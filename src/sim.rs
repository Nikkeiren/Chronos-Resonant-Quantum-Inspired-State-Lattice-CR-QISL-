//! Tick-based asynchronous simulation engine.
//!
//! Order of operations in every tick `t`:
//!   0. network events (partition start / heal are logged)
//!   1. deliveries due at `t` are processed (validation, cache update, witnesses)
//!   2. scheduled evidence injections
//!   3. adversary nodes emit forged messages
//!   4. every honest node runs its local protocol step and sends (latency >= 1)
//!   5. the observer records telemetry (observer-level, not available to nodes)
//!
//! Variants run on identical `Setup` (graph, partition groups, adversary set,
//! initial states) and identical channel randomness for the same (seed, link, tick).

use crate::config::Config;
use crate::metrics::{first_run_start, EventRecord, RunSummary, Series, METRIC_COLUMNS};
use crate::model::{validate_state, wrap_phase, TWO_PI};
use crate::network::{Counters, Message, MsgKind, Network, DEC_CONFLICT, DEC_NONE};
use crate::node::{AdvKind, DecState, Node};
use crate::rng::{hash_combine, Rng};
use crate::stats;
use crate::topology::{self, Graph};
use std::collections::BTreeMap;
use std::f64::consts::PI;

/// Variant-independent experimental conditions derived from the seed.
#[derive(Clone, Debug)]
pub struct Setup {
    pub graph: Graph,
    /// Network partition labels; used exclusively for communication gating.
    pub groups: Vec<u8>,
    /// Evidence recipient labels; used exclusively for scheduled evidence injection.
    pub evidence_groups: Vec<u8>,
    pub adversaries: Vec<Option<AdvKind>>,
    pub init_amp: Vec<Vec<f64>>,
    pub init_phase: Vec<Vec<f64>>,
}

impl Setup {
    pub fn build(cfg: &Config) -> Result<Setup, String> {
        let n = cfg.nodes;
        let k = cfg.candidates;
        let kf = k as f64;
        let graph = topology::build(&cfg.topology, n, &mut Rng::stream(cfg.seed, 1))?;
        let groups = topology::partition_groups(&graph, &cfg.network.partition_mode, cfg.network.partition_fraction, &mut Rng::stream(cfg.seed, 2));
        let evidence_groups = build_evidence_groups(&groups, &cfg.evidence_assignment_mode, cfg.seed)?;

        let mut adversaries: Vec<Option<AdvKind>> = vec![None; n];
        let count = (cfg.adversary.fraction * n as f64).round() as usize;
        if cfg.adversary.behavior != "none" && count > 0 {
            let kind = match cfg.adversary.behavior.as_str() {
                "amplitude" => AdvKind::Amplitude,
                "phase" => AdvKind::Phase,
                _ => AdvKind::Invalid,
            };
            let mut rng = Rng::stream(cfg.seed, 3);
            let chosen: Vec<usize> = if cfg.adversary.placement == "clustered" {
                let start = rng.below(n);
                graph.bfs_order(start).into_iter().take(count).collect()
            } else {
                let mut v: Vec<usize> = (0..n).collect();
                rng.shuffle(&mut v);
                v.into_iter().take(count).collect()
            };
            for i in chosen {
                adversaries[i] = Some(kind);
            }
        }

        let mut rng_a = Rng::stream(cfg.seed, 4);
        let mut rng_p = Rng::stream(cfg.seed, 5);
        let bias = cfg.initial.bias_strength;
        let mut init_amp: Vec<Vec<f64>> = Vec::with_capacity(n);
        let mut init_phase: Vec<Vec<f64>> = Vec::with_capacity(n);
        for i in 0..n {
            let mut base = vec![1.0 / kf; k];
            match cfg.initial.amplitude_mode.as_str() {
                "random_bias" | "partition_aligned" => {
                    let c = if cfg.initial.amplitude_mode == "random_bias" {
                        rng_a.below(k)
                    } else {
                        groups[i] as usize % k
                    };
                    for (j, b) in base.iter_mut().enumerate() {
                        *b = if j == c { 1.0 / kf + bias } else { 1.0 / kf - bias / (kf - 1.0) };
                    }
                }
                "fixed" => {
                    let s: f64 = cfg.initial.amplitudes.iter().sum();
                    for (j, b) in base.iter_mut().enumerate() {
                        *b = cfg.initial.amplitudes[j] / s.max(1e-12);
                    }
                }
                _ => {}
            }
            for b in base.iter_mut() {
                let noise = if cfg.initial.amplitude_noise > 0.0 { cfg.initial.amplitude_noise * rng_a.normal() } else { 0.0 };
                *b = (*b + noise).max(1e-6);
            }
            let s: f64 = base.iter().sum();
            for b in base.iter_mut() {
                *b /= s;
            }
            init_amp.push(base);
            let ph: Vec<f64> = (0..k)
                .map(|j| match cfg.initial.phase_mode.as_str() {
                    "random" => rng_p.next_f64() * TWO_PI,
                    "candidate_offset" => TWO_PI * (j as f64) / kf,
                    "fixed" => wrap_phase(cfg.initial.phases[j]),
                    _ => 0.0,
                })
                .collect();
            init_phase.push(ph);
        }
        Ok(Setup {
            graph,
            groups,
            evidence_groups,
            adversaries,
            init_amp,
            init_phase,
        })
    }
}

/// Per-node history frames (only when trace recording is enabled).
#[derive(Clone, Debug, Default)]
pub struct Trace {
    pub ticks: Vec<u64>,
    pub amp: Vec<Vec<f64>>,
    pub phase: Vec<Vec<f64>>,
    pub state: Vec<Vec<u8>>,
    pub dec: Vec<Vec<u8>>,
}

pub struct RunOutput {
    pub summary: RunSummary,
    pub series: Series,
    pub events: Vec<EventRecord>,
    pub dropped_events: u64,
    pub trace: Option<Trace>,
    pub counters: Counters,
    pub final_amp: Vec<Vec<f64>>,
    pub final_state: Vec<u8>,
    pub setup: Setup,
}

struct EventLog {
    events: Vec<EventRecord>,
    cap: usize,
    dropped: u64,
}

impl EventLog {
    fn push(&mut self, tick: u64, kind: &str, node: i64, a: i64, b: i64) {
        if self.events.len() < self.cap {
            self.events.push(EventRecord {
                tick,
                kind: kind.to_string(),
                node,
                a,
                b,
            });
        } else {
            self.dropped += 1;
        }
    }
}

struct Obs {
    honest: usize,
    undecided: usize,
    tentative: usize,
    conflict: usize,
    stable: Vec<usize>,
    mean_amp: Vec<f64>,
    coherence: Vec<f64>,
    spread: f64,
    nan_nodes: usize,
}

fn observe(nodes: &[Node], k: usize) -> Obs {
    let mut o = Obs {
        honest: 0,
        undecided: 0,
        tentative: 0,
        conflict: 0,
        stable: vec![0; k],
        mean_amp: vec![0.0; k],
        coherence: vec![0.0; k],
        spread: 0.0,
        nan_nodes: 0,
    };
    let mut finite: Vec<&Node> = Vec::new();
    for nd in nodes.iter().filter(|x| x.adversary.is_none()) {
        o.honest += 1;
        match nd.state {
            DecState::Undecided => o.undecided += 1,
            DecState::Tentative => o.tentative += 1,
            DecState::ConflictPending => o.conflict += 1,
            DecState::Stable => {
                if (nd.dec_cand as usize) < k {
                    o.stable[nd.dec_cand as usize] += 1;
                }
            }
        }
        if nd.amp.iter().chain(nd.phase.iter()).all(|x| x.is_finite()) {
            finite.push(nd);
        } else {
            o.nan_nodes += 1;
        }
    }
    if !finite.is_empty() {
        let m = finite.len() as f64;
        for c in 0..k {
            let mut sa = 0.0;
            let mut re = 0.0;
            let mut im = 0.0;
            for nd in &finite {
                sa += nd.amp[c];
                re += nd.amp[c] * nd.phase[c].cos();
                im += nd.amp[c] * nd.phase[c].sin();
            }
            o.mean_amp[c] = sa / m;
            o.coherence[c] = if sa > 1e-12 { re.hypot(im) / sa } else { 0.0 };
        }
        let mut sp: f64 = 0.0;
        for nd in &finite {
            for c in 0..k {
                sp = sp.max((nd.amp[c] - o.mean_amp[c]).abs());
            }
        }
        o.spread = sp;
    }
    o
}

/// Omniscient adversary helper: honest mean amplitude and amplitude-weighted circular mean phase.
fn honest_mean(nodes: &[Node], k: usize) -> (Vec<f64>, Vec<f64>) {
    let mut amp = vec![0.0; k];
    let mut re = vec![0.0; k];
    let mut im = vec![0.0; k];
    let mut cnt = 0.0;
    for nd in nodes.iter().filter(|x| x.adversary.is_none()) {
        if nd.amp.iter().chain(nd.phase.iter()).all(|x| x.is_finite()) {
            cnt += 1.0;
            for c in 0..k {
                amp[c] += nd.amp[c];
                re[c] += nd.amp[c] * nd.phase[c].cos();
                im[c] += nd.amp[c] * nd.phase[c].sin();
            }
        }
    }
    if cnt == 0.0 {
        return (vec![1.0 / k as f64; k], vec![0.0; k]);
    }
    let s: f64 = amp.iter().sum();
    let mut pm = vec![0.0; k];
    for c in 0..k {
        amp[c] /= s.max(1e-12);
        pm[c] = if re[c].hypot(im[c]) > 1e-12 { wrap_phase(im[c].atan2(re[c])) } else { 0.0 };
    }
    (amp, pm)
}

fn valid_decision(d: u8, k: usize) -> bool {
    (d as usize) < k || d == DEC_NONE || d == DEC_CONFLICT
}

fn forged_message(cfg: &Config, kind: AdvKind, tick: u64, nodes: &[Node], k: usize) -> (Vec<f64>, Vec<f64>, u8) {
    let a = &cfg.adversary;
    let decision = if a.claim_decision { a.target_candidate as u8 } else { DEC_NONE };
    match kind {
        AdvKind::Amplitude => {
            let mut amp = vec![(1.0 - a.forged_amplitude) / (k as f64 - 1.0); k];
            amp[a.target_candidate] = a.forged_amplitude;
            (amp, vec![0.0; k], decision)
        }
        AdvKind::Phase => {
            let (amp, pm) = honest_mean(nodes, k);
            let ph: Vec<f64> = pm.iter().map(|p| wrap_phase(p + PI)).collect();
            (amp, ph, decision)
        }
        AdvKind::Invalid => {
            let phase = vec![0.0; k];
            let mut amp = vec![1.0 / k as f64; k];
            let mut ph = phase;
            match (tick / a.period.max(1)) % 4 {
                0 => amp = vec![f64::NAN; k],
                1 => {
                    amp = vec![0.0; k];
                    amp[0] = f64::INFINITY;
                }
                2 => {
                    amp = vec![0.0; k];
                    amp[0] = -0.5;
                    amp[1] = 1.5;
                }
                _ => {
                    ph = vec![f64::NAN; k];
                }
            }
            (amp, ph, decision)
        }
    }
}

/// Run one simulation. `record_trace` enables per-node frames (amplitude, phase, state).
pub fn run(cfg: &Config, record_trace: bool) -> Result<RunOutput, String> {
    let errs = cfg.validate();
    if !errs.is_empty() {
        return Err(format!("invalid configuration: {}", errs.join("; ")));
    }
    let setup = Setup::build(cfg)?;
    let n = cfg.nodes;
    let k = cfg.candidates;
    let horizon = cfg.max_ticks as usize;
    let feat = cfg.effective_features();
    let validate = cfg.communication.validate_inputs;

    let mut nodes: Vec<Node> = (0..n)
        .map(|i| {
            let phase = if feat.phase { setup.init_phase[i].clone() } else { vec![0.0; k] };
            Node::new(i, setup.graph.adj[i].clone(), setup.init_amp[i].clone(), phase, setup.adversaries[i])
        })
        .collect();
    let mut net = Network::new(n, k, cfg.seed, cfg.network.clone(), setup.groups.clone(), cfg.max_ticks);
    let mut log = EventLog {
        events: Vec::new(),
        cap: cfg.telemetry.max_events,
        dropped: 0,
    };

    // Series layout.
    let mut names: Vec<String> = vec!["tick".into(), "partition".into(), "undecided".into(), "tentative".into(), "conflict".into()];
    for c in 0..k {
        names.push(format!("stable_{}", c));
    }
    for c in 0..k {
        names.push(format!("mean_amp_{}", c));
    }
    for c in 0..k {
        names.push(format!("coherence_{}", c));
    }
    for s in ["spread", "divergence", "agree", "sent", "delivered", "dropped_loss", "dropped_partition", "dropped_invalid", "bytes_sent", "nan_nodes"] {
        names.push(s.to_string());
    }
    let mut series = Series::new(names);
    let mut trace = if record_trace { Some(Trace::default()) } else { None };

    let mut agree_flags: Vec<bool> = Vec::with_capacity(horizon);
    let mut conv_flags: Vec<bool> = Vec::with_capacity(horizon);
    let mut quorum_flags: Vec<bool> = Vec::with_capacity(horizon);
    let mut div_values: Vec<f64> = Vec::with_capacity(horizon);
    let mut last_obs: Option<Obs> = None;

    for t in 0..horizon {
        let tick = t as u64;
        // 0. network events
        if cfg.network.partition_enabled {
            if tick == cfg.network.partition_start {
                log.push(tick, "partition_start", -1, 0, 0);
            }
            if tick == cfg.network.partition_end {
                log.push(tick, "partition_heal", -1, 0, 0);
            }
        }

        // 1. deliveries
        let msgs = net.take_deliveries(tick);
        for m in msgs {
            let to = m.to;
            if nodes[to].adversary.is_some() {
                continue;
            }
            match m.kind {
                MsgKind::State | MsgKind::Heartbeat | MsgKind::Reconcile => {
                    if validate && (!validate_state(&m.amp, &m.phase, k) || !valid_decision(m.decision, k)) {
                        net.counters.dropped_invalid += 1;
                        continue;
                    }
                    if m.amp.len() != k || m.phase.len() != k {
                        net.counters.dropped_invalid += 1;
                        continue;
                    }
                    if !nodes[to].receive_state(m.from, &m.amp, &m.phase, m.decision, m.send_tick, tick, m.seq) {
                        net.counters.stale_ignored += 1;
                    }
                }
                MsgKind::Witness => {
                    if (m.w_a as usize) >= k || (m.w_b as usize) >= k {
                        net.counters.dropped_invalid += 1;
                        continue;
                    }
                    let mut evs = Vec::new();
                    let fwd = nodes[to].on_witness(&feat, tick, m.w_origin, m.w_id, m.w_a, m.w_b, m.ttl, &mut evs);
                    for e in evs {
                        log.push(tick, e.kind, to as i64, e.a, e.b);
                    }
                    if let Some(ttl) = fwd {
                        let salt = hash_combine(m.w_origin as u64, m.w_id);
                        for &nb in setup.graph.adj[to].iter() {
                            if nb != m.from {
                                net.send(Message::witness(to, nb, tick, m.w_origin, m.w_id, m.w_a, m.w_b, ttl), tick, salt);
                            }
                        }
                    }
                }
            }
        }

        // 2. scheduled evidence injections (ASSUMPTION A8)
        for ev in cfg.events.iter().filter(|e| e.tick == tick) {
            for i in 0..n {
                if nodes[i].adversary.is_some() {
                    continue;
                }
                if ev.group >= 0 && setup.evidence_groups[i] as i64 != ev.group {
                    continue;
                }
                let nd = &mut nodes[i];
                for c in 0..k {
                    let target = if c == ev.candidate { 1.0 } else { 0.0 };
                    nd.amp[c] = (1.0 - ev.strength) * nd.amp[c] + ev.strength * target;
                }
                let s: f64 = nd.amp.iter().sum();
                if s.is_finite() && s > 0.0 {
                    for a in nd.amp.iter_mut() {
                        *a /= s;
                    }
                }
                if feat.phase {
                    nd.phase[ev.candidate] = wrap_phase(ev.phase);
                }
            }
            log.push(tick, "inject", -1, ev.group, ev.candidate as i64);
        }

        // 3. adversaries
        let adv_active = tick >= cfg.adversary.start_tick && (cfg.adversary.end_tick == 0 || tick < cfg.adversary.end_tick);
        if adv_active && setup.adversaries.iter().any(|a| a.is_some()) {
            if tick == cfg.adversary.start_tick {
                log.push(tick, "adversary_active", -1, 0, 0);
            }
            if (tick - cfg.adversary.start_tick) % cfg.adversary.period.max(1) == 0 {
                for i in 0..n {
                    let kind = match nodes[i].adversary {
                        Some(kd) => kd,
                        None => continue,
                    };
                    let (amp, ph, dec) = forged_message(cfg, kind, tick, &nodes, k);
                    nodes[i].seq += 1;
                    let seq = nodes[i].seq;
                    if amp.iter().chain(ph.iter()).all(|x| x.is_finite()) {
                        nodes[i].amp = amp.clone();
                        nodes[i].phase = ph.clone();
                    }
                    for &nb in setup.graph.adj[i].iter() {
                        net.counters.adversary_messages += 1;
                        net.send(Message::state(i, nb, MsgKind::State, tick, seq, &amp, &ph, dec), tick, 0);
                    }
                }
            }
        }

        // 4. honest node steps
        for i in 0..n {
            if nodes[i].adversary.is_some() {
                continue;
            }
            let out = nodes[i].tick(cfg, &feat, tick);
            for e in out.events {
                log.push(tick, e.kind, i as i64, e.a, e.b);
            }
            if let Some(msg) = out.state_msg {
                for &nb in setup.graph.adj[i].iter() {
                    net.send(Message::state(i, nb, msg.kind, tick, msg.seq, &msg.amp, &msg.phase, msg.decision), tick, 0);
                }
            }
            if let Some(w) = out.witness {
                let salt = hash_combine(i as u64, w.id);
                for &nb in setup.graph.adj[i].iter() {
                    net.send(Message::witness(i, nb, tick, i, w.id, w.a, w.b, w.ttl), tick, salt);
                }
            }
        }

        // 5. observation
        let obs = observe(&nodes, k);
        let stable_total: usize = obs.stable.iter().sum();
        let max_stable = obs.stable.iter().cloned().max().unwrap_or(0);
        let divergence = if stable_total > 0 { 1.0 - max_stable as f64 / stable_total as f64 } else { 0.0 };
        let agree = obs.honest > 0 && max_stable == obs.honest;
        let converged = obs.honest > 0 && obs.nan_nodes == 0 && obs.spread < cfg.metrics.convergence_eps;
        let quorum = obs.honest > 0 && (max_stable as f64) >= cfg.metrics.quorum_fraction * obs.honest as f64;
        agree_flags.push(agree);
        quorum_flags.push(quorum);
        conv_flags.push(converged);
        div_values.push(divergence);

        let mut row: Vec<f64> = vec![
            tick as f64,
            if net.partition_active(tick) { 1.0 } else { 0.0 },
            obs.undecided as f64,
            obs.tentative as f64,
            obs.conflict as f64,
        ];
        row.extend(obs.stable.iter().map(|&x| x as f64));
        row.extend(obs.mean_amp.iter().cloned());
        row.extend(obs.coherence.iter().cloned());
        row.push(obs.spread);
        row.push(divergence);
        row.push(if agree { 1.0 } else { 0.0 });
        row.push(net.counters.sent as f64);
        row.push(net.counters.delivered as f64);
        row.push(net.counters.dropped_loss as f64);
        row.push(net.counters.dropped_partition as f64);
        row.push(net.counters.dropped_invalid as f64);
        row.push(net.counters.bytes_sent as f64);
        row.push(obs.nan_nodes as f64);
        series.push_row(&row);

        if let Some(tr) = trace.as_mut() {
            if tick % cfg.telemetry.history_stride == 0 || t + 1 == horizon {
                tr.ticks.push(tick);
                let mut amp = Vec::with_capacity(n * k);
                let mut ph = Vec::with_capacity(n * k);
                let mut st = Vec::with_capacity(n);
                let mut dc = Vec::with_capacity(n);
                for nd in &nodes {
                    amp.extend(nd.amp.iter().cloned());
                    ph.extend(nd.phase.iter().cloned());
                    st.push(if nd.adversary.is_some() { 4u8 } else { nd.state as u8 });
                    dc.push(if nd.state == DecState::Stable { nd.dec_cand } else { DEC_NONE });
                }
                tr.amp.push(amp);
                tr.phase.push(ph);
                tr.state.push(st);
                tr.dec.push(dc);
            }
        }
        last_obs = Some(obs);
    }
    log.push(cfg.max_ticks.saturating_sub(1), "run_end", -1, 0, 0);

    // ---- summary metrics (observer-level unless stated) ----
    let hold = cfg.metrics.hold_ticks as usize;
    let reference = if cfg.network.partition_enabled { (cfg.network.partition_end as usize).min(horizon) } else { 0 };
    let horizon_f = horizon as f64;

    // Continuous spread diagnostics complement threshold-based convergence time.
    // Spread is measured across finite honest nodes; post-heal summaries start
    // at the healing reference tick (inclusive) and exclude ticks with NaN nodes.
    let spread_values = series.column("spread").map(|v| v.as_slice()).unwrap_or(&[]);
    let nan_values = series.column("nan_nodes").map(|v| v.as_slice()).unwrap_or(&[]);
    let (final_amplitude_spread, post_heal_amplitude_spread_mean, post_heal_amplitude_spread_min) =
        amplitude_spread_diagnostics(spread_values, nan_values, reference);

    let mut m: BTreeMap<String, f64> = BTreeMap::new();
    let honest: Vec<&Node> = nodes.iter().filter(|x| x.adversary.is_none()).collect();
    let honest_n = honest.len() as f64;
    m.insert("honest_nodes".into(), honest_n);
    m.insert("adversarial_nodes".into(), (n - honest.len()) as f64);
    m.insert("horizon".into(), horizon_f);
    m.insert("reference_tick".into(), reference as f64);

    // Per-seed structural diagnostics: network groups gate communication;
    // evidence groups determine which nodes receive candidate evidence.
    let mut network_sizes = [0usize; 2];
    let mut evidence_sizes = [0usize; 2];
    let mut contingency = [[0usize; 2]; 2];
    let mut overlap = 0usize;
    for i in 0..n {
        let ng = setup.groups[i] as usize;
        let eg = setup.evidence_groups[i] as usize;
        network_sizes[ng] += 1;
        evidence_sizes[eg] += 1;
        contingency[ng][eg] += 1;
        if ng == eg { overlap += 1; }
    }
    let edges = setup.graph.edges();
    let cross_edges = edges.iter().filter(|(a, b)| setup.groups[*a] != setup.groups[*b]).count();
    let edge_count = edges.len();
    m.insert("network_group0_size".into(), network_sizes[0] as f64);
    m.insert("network_group1_size".into(), network_sizes[1] as f64);
    m.insert("evidence_group0_size".into(), evidence_sizes[0] as f64);
    m.insert("evidence_group1_size".into(), evidence_sizes[1] as f64);
    m.insert("evidence_overlap_rate".into(), overlap as f64 / n.max(1) as f64);
    m.insert("group_n00".into(), contingency[0][0] as f64);
    m.insert("group_n01".into(), contingency[0][1] as f64);
    m.insert("group_n10".into(), contingency[1][0] as f64);
    m.insert("group_n11".into(), contingency[1][1] as f64);
    m.insert("graph_edge_count".into(), edge_count as f64);
    m.insert("cross_network_edge_count".into(), cross_edges as f64);
    m.insert("network_cut_ratio".into(), if edge_count > 0 { cross_edges as f64 / edge_count as f64 } else { f64::NAN });

    // Agreement / convergence times measured from the reference tick (healing, or 0).
    match first_run_start(&agree_flags, reference, hold) {
        Some(s) => {
            m.insert("agreement_time_after_heal".into(), (s - reference) as f64);
            m.insert("agreement_censored".into(), 0.0);
        }
        None => {
            m.insert("agreement_time_after_heal".into(), (horizon - reference) as f64);
            m.insert("agreement_censored".into(), 1.0);
        }
    }
    match first_run_start(&quorum_flags, reference, hold) {
        Some(s) => {
            m.insert("quorum_agreement_time_after_heal".into(), (s - reference) as f64);
            m.insert("quorum_censored".into(), 0.0);
        }
        None => {
            m.insert("quorum_agreement_time_after_heal".into(), (horizon - reference) as f64);
            m.insert("quorum_censored".into(), 1.0);
        }
    }
    match first_run_start(&conv_flags, reference, hold) {
        Some(s) => {
            m.insert("amplitude_convergence_time".into(), (s - reference) as f64);
            m.insert("convergence_censored".into(), 0.0);
        }
        None => {
            m.insert("amplitude_convergence_time".into(), (horizon - reference) as f64);
            m.insert("convergence_censored".into(), 1.0);
        }
    }
    m.insert("final_amplitude_spread".into(), final_amplitude_spread);
    m.insert("post_heal_amplitude_spread_mean".into(), post_heal_amplitude_spread_mean);
    m.insert("post_heal_amplitude_spread_min".into(), post_heal_amplitude_spread_min);

    // Local stability: first tick each honest node entered Stable (never = censored at horizon).
    let stab: Vec<f64> = honest.iter().map(|x| x.first_stable_tick.map(|t| t as f64).unwrap_or(horizon_f)).collect();
    let never = honest.iter().filter(|x| x.first_stable_tick.is_none()).count() as f64;
    m.insert("stability_time_median".into(), stats::median(&stab));
    m.insert("stability_time_p90".into(), stats::quantile(&stab, 0.9));
    m.insert("nodes_never_stable".into(), never);
    m.insert("stability_censored_fraction".into(), if honest_n > 0.0 { never / honest_n } else { f64::NAN });

    // Divergence (minority share among nodes in Stable, per tick), windows relative to healing.
    let window_mean = |lo: usize, hi: usize| -> f64 {
        let hi = hi.min(div_values.len());
        if lo >= hi {
            f64::NAN
        } else {
            div_values[lo..hi].iter().sum::<f64>() / (hi - lo) as f64
        }
    };
    let window_split = |lo: usize, hi: usize| -> f64 {
        let hi = hi.min(div_values.len());
        if lo >= hi {
            f64::NAN
        } else {
            div_values[lo..hi].iter().filter(|&&d| d > 0.0).count() as f64
        }
    };
    if cfg.network.partition_enabled {
        let (ps, pe) = (cfg.network.partition_start as usize, cfg.network.partition_end as usize);
        m.insert("divergence_pre_heal".into(), window_mean(ps, pe));
        m.insert("divergence_post_heal".into(), window_mean(pe, horizon));
        m.insert("split_ticks_pre_heal".into(), window_split(ps, pe));
        m.insert("split_ticks_post_heal".into(), window_split(pe, horizon));
    } else {
        m.insert("divergence_pre_heal".into(), window_mean(0, horizon));
        m.insert("divergence_post_heal".into(), f64::NAN);
        m.insert("split_ticks_pre_heal".into(), window_split(0, horizon));
        m.insert("split_ticks_post_heal".into(), f64::NAN);
    }

    let sum_u32 = |f: &dyn Fn(&Node) -> u32| -> f64 { honest.iter().map(|x| f(x) as f64).sum() };
    m.insert("oscillation_mean".into(), if honest_n > 0.0 { sum_u32(&|x| x.flips) / honest_n } else { f64::NAN });
    m.insert("stable_revocations".into(), sum_u32(&|x| x.revocations));
    m.insert("conflict_entries".into(), sum_u32(&|x| x.conflict_entries));
    m.insert("reconcile_timeouts".into(), sum_u32(&|x| x.reconcile_timeouts));
    m.insert("normalization_resets".into(), sum_u32(&|x| x.resets));

    let (fin_stable, fin_conflict, fin_split, fin_agree, nan_nodes) = match &last_obs {
        Some(o) => {
            let st: usize = o.stable.iter().sum();
            let mx = o.stable.iter().cloned().max().unwrap_or(0);
            (st as f64, o.conflict as f64, if st > 0 && mx < st { 1.0 } else { 0.0 }, if o.honest > 0 && mx == o.honest { 1.0 } else { 0.0 }, o.nan_nodes as f64)
        }
        None => (0.0, 0.0, 0.0, 0.0, 0.0),
    };
    m.insert("conflict_pending_final".into(), fin_conflict);
    m.insert("final_stable_fraction".into(), if honest_n > 0.0 { fin_stable / honest_n } else { f64::NAN });
    m.insert("final_agreement".into(), fin_agree);
    m.insert("final_split".into(), fin_split);
    m.insert("nan_corrupted_nodes".into(), nan_nodes);
    let adopted = if cfg.adversary.behavior != "none" && honest_n > 0.0 {
        honest.iter().filter(|x| x.state == DecState::Stable && x.dec_cand as usize == cfg.adversary.target_candidate).count() as f64 / honest_n
    } else {
        0.0
    };
    m.insert("adv_target_adoption_final".into(), adopted);

    let c = net.counters.clone();
    m.insert("messages_sent".into(), c.sent as f64);
    m.insert("messages_delivered".into(), c.delivered as f64);
    m.insert("dropped_loss".into(), c.dropped_loss as f64);
    m.insert("dropped_partition".into(), c.dropped_partition as f64);
    m.insert("dropped_invalid".into(), c.dropped_invalid as f64);
    m.insert("dropped_total".into(), (c.dropped_loss + c.dropped_partition + c.dropped_invalid) as f64);
    m.insert("undelivered_end".into(), (c.undelivered_end + net.in_flight()) as f64);
    m.insert("stale_ignored".into(), c.stale_ignored as f64);
    m.insert("bytes_sent".into(), c.bytes_sent as f64);
    m.insert("bytes_delivered".into(), c.bytes_delivered as f64);
    m.insert("state_updates".into(), c.state_updates as f64);
    m.insert("heartbeats".into(), c.heartbeats as f64);
    m.insert("witness_messages".into(), c.witness_messages as f64);
    m.insert("reconciliation_messages".into(), c.reconciliation_messages as f64);
    m.insert("adversary_messages".into(), c.adversary_messages as f64);
    m.insert("retransmissions".into(), c.retransmissions as f64);
    m.insert("signature_ops".into(), c.signature_ops as f64);
    debug_assert!(METRIC_COLUMNS.iter().all(|c| m.contains_key(*c)));

    let final_amp: Vec<Vec<f64>> = nodes.iter().map(|x| x.amp.clone()).collect();
    let final_state: Vec<u8> = nodes.iter().map(|x| if x.adversary.is_some() { 4 } else { x.state as u8 }).collect();
    let summary = RunSummary::ok(cfg.variant.label(), cfg.seed, m);
    Ok(RunOutput {
        summary,
        series,
        events: log.events,
        dropped_events: log.dropped,
        trace,
        counters: c,
        final_amp,
        final_state,
        setup,
    })
}

fn r4(x: f64) -> f64 {
    if x.is_finite() {
        (x * 1e4).round() / 1e4
    } else {
        x
    }
}

/// Computes continuous amplitude-spread diagnostics from aligned tick arrays.
fn amplitude_spread_diagnostics(spread: &[f64], nan_nodes: &[f64], reference: usize) -> (f64, f64, f64) {
    let final_spread = if nan_nodes.last().copied().unwrap_or(1.0) == 0.0 {
        spread.last().copied().unwrap_or(f64::NAN)
    } else { f64::NAN };
    let valid: Vec<f64> = spread.iter().enumerate()
        .filter(|(i, x)| *i >= reference && x.is_finite() && nan_nodes.get(*i).copied().unwrap_or(1.0) == 0.0)
        .map(|(_, x)| *x).collect();
    if valid.is_empty() { return (final_spread, f64::NAN, f64::NAN); }
    (final_spread, valid.iter().sum::<f64>() / valid.len() as f64,
     valid.iter().copied().fold(f64::INFINITY, f64::min))
}

/// Build evidence-recipient labels independently of network partition labels when requested.
fn build_evidence_groups(network_groups: &[u8], mode: &str, seed: u64) -> Result<Vec<u8>, String> {
    match mode {
        "aligned" => Ok(network_groups.to_vec()),
        "reversed" => Ok(network_groups.iter().map(|g| 1u8.saturating_sub(*g)).collect()),
        "independent_balanced" => {
            let n = network_groups.len();
            if n < 2 { return Err("independent_balanced evidence assignment requires at least two nodes".into()); }
            let size0 = network_groups.iter().filter(|&&g| g == 0).count();
            if size0 == 0 || size0 == n {
                return Err("network groups must both be non-empty for independent_balanced evidence assignment".into());
            }
            let mut order: Vec<usize> = (0..n).collect();
            Rng::stream(seed, 6).shuffle(&mut order);
            let mut labels = vec![1u8; n];
            for &idx in order.iter().take(size0) { labels[idx] = 0; }
            Ok(labels)
        }
        other => Err(format!("unknown evidence_assignment_mode '{}'", other)),
    }
}

#[cfg(test)]
mod evidence_assignment_tests {
    use super::*;

    #[test]
    fn aligned_and_reversed_assignments_are_exact() {
        let network = vec![0, 0, 1, 1];
        assert_eq!(build_evidence_groups(&network, "aligned", 7).unwrap(), network);
        assert_eq!(build_evidence_groups(&network, "reversed", 7).unwrap(), vec![1, 1, 0, 0]);
    }

    #[test]
    fn independent_assignment_is_deterministic_and_balanced() {
        let network = vec![0, 0, 0, 1, 1, 1];
        let a = build_evidence_groups(&network, "independent_balanced", 17).unwrap();
        let b = build_evidence_groups(&network, "independent_balanced", 17).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.iter().filter(|&&g| g == 0).count(), 3);
    }
}

/// Trace document consumed by `viewer/index.html` (schema `crqisl-trace/1`).
pub fn trace_json(cfg: &Config, out: &RunOutput) -> Option<serde_json::Value> {
    let tr = out.trace.as_ref()?;
    let n = cfg.nodes;
    let adv: Vec<u8> = out.setup.adversaries.iter().map(|a| if a.is_some() { 1 } else { 0 }).collect();
    let edges: Vec<[usize; 2]> = out.setup.graph.edges().into_iter().map(|(a, b)| [a, b]).collect();
    let amp: Vec<Vec<f64>> = tr.amp.iter().map(|f| f.iter().map(|&x| r4(x)).collect()).collect();
    let phase: Vec<Vec<f64>> = tr.phase.iter().map(|f| f.iter().map(|&x| r4(x)).collect()).collect();
    let events: Vec<serde_json::Value> = out
        .events
        .iter()
        .map(|e| serde_json::json!({"tick": e.tick, "kind": e.kind, "node": e.node, "a": e.a, "b": e.b}))
        .collect();
    Some(serde_json::json!({
        "schema": "crqisl-trace/1",
        "meta": {
            "name": cfg.name,
            "variant": cfg.variant.label(),
            "seed": cfg.seed,
            "nodes": n,
            "candidates": cfg.candidates,
            "max_ticks": cfg.max_ticks,
            "history_stride": cfg.telemetry.history_stride,
            "partition": {
                "enabled": cfg.network.partition_enabled,
                "start": cfg.network.partition_start,
                "end": cfg.network.partition_end
            },
            "config_hash": cfg.hash_hex(),
            "synthetic": false,
            "note": "Local stability is NOT global agreement or finality. Observer-level series only."
        },
        "nodes": {
            "group": out.setup.groups,
            "network_group": out.setup.groups,
            "evidence_group": out.setup.evidence_groups,
            "adversary": adv
        },
        "edges": edges,
        "frames": { "ticks": tr.ticks, "amp": amp, "phase": phase, "state": tr.state, "dec": tr.dec },
        "series": out.series.to_json(),
        "events": events
    }))
}

#[cfg(test)]
mod amplitude_diagnostic_tests {
    use super::*;
    #[test]
    fn filters_nan_ticks_and_respects_reference_tick() {
        let (f, mean, min) = amplitude_spread_diagnostics(&[0.1, 0.4, 0.2, 0.3], &[0.0, 1.0, 0.0, 0.0], 2);
        assert!((f - 0.3).abs() < 1e-12);
        assert!((mean - 0.25).abs() < 1e-12);
        assert!((min - 0.2).abs() < 1e-12);
    }
    #[test]
    fn empty_valid_window_returns_nan_for_aggregate_metrics() {
        let (_, mean, min) = amplitude_spread_diagnostics(&[0.2, 0.3], &[1.0, 1.0], 1);
        assert!(mean.is_nan() && min.is_nan());
    }
}
