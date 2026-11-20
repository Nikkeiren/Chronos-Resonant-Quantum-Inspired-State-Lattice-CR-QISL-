//! Per-node protocol logic for honest nodes: neighbour cache, integrity weights,
//! local update, local stability decision (with CONFLICT_PENDING), conflict
//! witnesses, reconciliation hooks, delta gating and heartbeat.
//!
//! IMPORTANT: a `Stable` state is a *local* decision ("my own state has been
//! stable for a window"). It is NOT global agreement and NOT safe finality.

use crate::config::{Config, Features};
use crate::model::{circ_dist, integrity_targets, step_state, top_two, wrap_phase, IntegrityParams, Sample, StepParams};
use crate::network::{MsgKind, DEC_CONFLICT, DEC_NONE};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecState {
    Undecided = 0,
    Tentative = 1,
    Stable = 2,
    ConflictPending = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdvKind {
    Amplitude,
    Phase,
    Invalid,
}

#[derive(Clone, Debug)]
pub struct CacheEntry {
    pub amp: Vec<f64>,
    pub phase: Vec<f64>,
    pub decision: u8,
    pub recv_tick: u64,
    pub send_tick: u64,
    pub seq: u64,
}

#[derive(Clone, Debug)]
pub struct NodeEvent {
    pub kind: &'static str,
    pub a: i64,
    pub b: i64,
}

#[derive(Clone, Debug)]
pub struct Outgoing {
    pub kind: MsgKind,
    pub amp: Vec<f64>,
    pub phase: Vec<f64>,
    pub decision: u8,
    pub seq: u64,
}

#[derive(Clone, Debug)]
pub struct WitnessOut {
    pub id: u64,
    pub a: u8,
    pub b: u8,
    pub ttl: u8,
}

#[derive(Default)]
pub struct TickOutput {
    pub state_msg: Option<Outgoing>,
    pub witness: Option<WitnessOut>,
    pub events: Vec<NodeEvent>,
    pub reset: bool,
}

pub struct Node {
    pub id: usize,
    pub neighbors: Vec<usize>,
    pub adversary: Option<AdvKind>,
    pub amp: Vec<f64>,
    pub phase: Vec<f64>,
    pub cache: Vec<Option<CacheEntry>>,
    pub weights: Vec<f64>,
    pub seq: u64,
    pub last_sent_amp: Vec<f64>,
    pub last_sent_phase: Vec<f64>,
    pub last_sent_dec: u8,
    pub last_sent_tick: Option<u64>,
    pub last_reconcile_tick: Option<u64>,
    pub state: DecState,
    pub dec_cand: u8,
    pub dominant: u8,
    pub last_dom: u8,
    pub stable_counter: u32,
    pub conflict_since: Option<u64>,
    release_counter: u32,
    release_dom: u8,
    timeout_flagged: bool,
    seen_witness: HashSet<(usize, u64)>,
    next_wid: u64,
    witness_cooldown_until: u64,
    pub first_stable_tick: Option<u64>,
    pub flips: u32,
    pub revocations: u32,
    pub conflict_entries: u32,
    pub reconcile_timeouts: u32,
    pub resets: u32,
}

impl Node {
    pub fn new(id: usize, neighbors: Vec<usize>, amp: Vec<f64>, phase: Vec<f64>, adversary: Option<AdvKind>) -> Node {
        let deg = neighbors.len();
        let k = amp.len();
        Node {
            id,
            neighbors,
            adversary,
            last_sent_amp: vec![f64::NAN; k],
            last_sent_phase: vec![0.0; k],
            amp,
            phase,
            cache: vec![None; deg],
            weights: vec![1.0; deg],
            seq: 0,
            last_sent_dec: DEC_NONE,
            last_sent_tick: None,
            last_reconcile_tick: None,
            state: DecState::Undecided,
            dec_cand: DEC_NONE,
            dominant: DEC_NONE,
            last_dom: DEC_NONE,
            stable_counter: 0,
            conflict_since: None,
            release_counter: 0,
            release_dom: DEC_NONE,
            timeout_flagged: false,
            seen_witness: HashSet::new(),
            next_wid: 0,
            witness_cooldown_until: 0,
            first_stable_tick: None,
            flips: 0,
            revocations: 0,
            conflict_entries: 0,
            reconcile_timeouts: 0,
            resets: 0,
        }
    }

    /// Store a received state if it is newer than the cached one. Returns false if stale.
    #[allow(clippy::too_many_arguments)]
    pub fn receive_state(&mut self, from: usize, amp: &[f64], phase: &[f64], decision: u8, send_tick: u64, tick: u64, seq: u64) -> bool {
        let idx = match self.neighbors.binary_search(&from) {
            Ok(i) => i,
            Err(_) => return false,
        };
        if let Some(c) = &self.cache[idx] {
            if c.seq >= seq {
                return false;
            }
        }
        self.cache[idx] = Some(CacheEntry {
            amp: amp.to_vec(),
            phase: phase.to_vec(),
            decision,
            recv_tick: tick,
            send_tick,
            seq,
        });
        true
    }

    fn enter_conflict(&mut self, tick: u64, a: u8, b: u8, events: &mut Vec<NodeEvent>) {
        self.state = DecState::ConflictPending;
        self.conflict_since = Some(tick);
        self.conflict_entries += 1;
        self.release_counter = 0;
        self.release_dom = DEC_NONE;
        self.timeout_flagged = false;
        self.dec_cand = DEC_NONE;
        self.stable_counter = 0;
        events.push(NodeEvent {
            kind: "conflict_enter",
            a: a as i64,
            b: b as i64,
        });
    }

    /// Handle a conflict witness. Returns the TTL to forward with (None = do not forward).
    #[allow(clippy::too_many_arguments)]
    pub fn on_witness(&mut self, feat: &Features, tick: u64, origin: usize, id: u64, a: u8, b: u8, ttl: u8, events: &mut Vec<NodeEvent>) -> Option<u8> {
        if !self.seen_witness.insert((origin, id)) {
            return None;
        }
        if feat.witness && self.state == DecState::Stable && (self.dec_cand == a || self.dec_cand == b) {
            self.enter_conflict(tick, a, b, events);
        }
        if ttl > 1 {
            Some(ttl - 1)
        } else {
            None
        }
    }

    /// Run one tick of the honest-node protocol (after this tick's deliveries).
    pub fn tick(&mut self, cfg: &Config, feat: &Features, tick: u64) -> TickOutput {
        let mut out = TickOutput::default();
        let ttl = cfg.communication.cache_ttl;

        // 1. Fresh neighbour samples (ASSUMPTION A1 aggregation inputs).
        let mut idxs: Vec<usize> = Vec::new();
        let mut samples: Vec<Sample> = Vec::new();
        for (idx, entry) in self.cache.iter().enumerate() {
            if let Some(e) = entry {
                if tick.saturating_sub(e.recv_tick) <= ttl {
                    let mut ph = e.phase.clone();
                    if feat.phase && cfg.model.age_compensation && cfg.model.omega != 0.0 {
                        let dt = tick.saturating_sub(e.send_tick) as f64;
                        for p in ph.iter_mut() {
                            *p = wrap_phase(*p + cfg.model.omega * dt);
                        }
                    }
                    let w = if feat.integrity { self.weights[idx] } else { 1.0 };
                    samples.push(Sample {
                        amp: e.amp.clone(),
                        phase: ph,
                        weight: w,
                    });
                    idxs.push(idx);
                }
            }
        }

        // 2. Integrity-weight adjustment (ASSUMPTION A6).
        if feat.integrity && samples.len() >= 2 {
            let ip = IntegrityParams {
                kappa: cfg.integrity.kappa,
                phase_gamma: cfg.integrity.phase_gamma,
                phase_on: feat.phase,
            };
            let targets = integrity_targets(&self.amp, &self.phase, &samples, &ip);
            let eta = cfg.integrity.eta;
            let min_w = cfg.integrity.min_weight;
            for (n, &idx) in idxs.iter().enumerate() {
                let mut w = (1.0 - eta) * self.weights[idx] + eta * targets[n];
                if !w.is_finite() {
                    w = min_w;
                }
                let w = w.clamp(min_w, 1.0);
                self.weights[idx] = w;
                samples[n].weight = w;
            }
        }

        // 3. Local state update (ASSUMPTIONS A1-A5, A7).
        let mut step = cfg.model.rate * if feat.damping { 1.0 - cfg.model.alpha } else { 1.0 };
        if self.state == DecState::ConflictPending && feat.reconcile {
            step *= cfg.conflict.reconcile_rate_boost;
        }
        let step = step.min(1.0);
        let params = StepParams {
            step,
            beta: cfg.model.beta,
            phase_on: feat.phase,
            omega: cfg.model.omega,
            validate: cfg.communication.validate_inputs,
        };
        let prev_amp = self.amp.clone();
        let res = step_state(&self.amp, &self.phase, &samples, &params);
        self.amp = res.amp;
        self.phase = res.phase;
        if res.normalization_reset {
            self.resets += 1;
            out.reset = true;
        }

        // 4. Local stability decision.
        let (new_dom, margin, prev_dom) = self.update_decision(cfg, tick, &prev_amp, &mut out.events);

        // 5. Conflict witness generation (direct observation of an opposing stable neighbour).
        if feat.witness && self.state == DecState::Stable && tick >= self.witness_cooldown_until {
            let mut conflicting: Option<u8> = None;
            for e in self.cache.iter().flatten() {
                if tick.saturating_sub(e.recv_tick) <= ttl && e.decision < DEC_CONFLICT && e.decision != self.dec_cand {
                    conflicting = Some(e.decision);
                    break;
                }
            }
            if let Some(other) = conflicting {
                let id = self.next_wid;
                self.next_wid += 1;
                self.seen_witness.insert((self.id, id));
                self.witness_cooldown_until = tick + cfg.conflict.witness_cooldown;
                let a = self.dec_cand;
                self.enter_conflict(tick, a, other, &mut out.events);
                out.witness = Some(WitnessOut {
                    id,
                    a,
                    b: other,
                    ttl: cfg.conflict.witness_ttl,
                });
                out.events.push(NodeEvent {
                    kind: "witness_generated",
                    a: a as i64,
                    b: other as i64,
                });
            }
        }

        // 6. Reconciliation: local release rule (ASSUMPTION R1). Never forces a decision.
        if self.state == DecState::ConflictPending {
            self.try_release(cfg, tick, new_dom, margin, &mut out.events);
        }
        let _ = prev_dom;

        // 7. Transmission policy: reconcile > initial > delta gate > heartbeat.
        let adv_decision = match self.state {
            DecState::Stable => self.dec_cand,
            DecState::ConflictPending => DEC_CONFLICT,
            _ => DEC_NONE,
        };
        let mut kind: Option<MsgKind> = None;
        if feat.reconcile
            && self.state == DecState::ConflictPending
            && self.last_reconcile_tick.map_or(true, |t| tick.saturating_sub(t) >= cfg.conflict.reconcile_period)
        {
            kind = Some(MsgKind::Reconcile);
            self.last_reconcile_tick = Some(tick);
        } else if self.last_sent_tick.is_none() {
            kind = Some(MsgKind::State);
        } else if self.gate_exceeded(cfg, feat) || adv_decision != self.last_sent_dec {
            kind = Some(MsgKind::State);
        } else if let Some(t) = self.last_sent_tick {
            if tick.saturating_sub(t) >= cfg.communication.heartbeat_period {
                kind = Some(MsgKind::Heartbeat);
            }
        }
        if let Some(k) = kind {
            self.seq += 1;
            self.last_sent_amp = self.amp.clone();
            self.last_sent_phase = self.phase.clone();
            self.last_sent_dec = adv_decision;
            self.last_sent_tick = Some(tick);
            out.state_msg = Some(Outgoing {
                kind: k,
                amp: self.amp.clone(),
                phase: self.phase.clone(),
                decision: adv_decision,
                seq: self.seq,
            });
        }
        out
    }

    fn gate_exceeded(&self, cfg: &Config, feat: &Features) -> bool {
        for c in 0..self.amp.len() {
            if (self.amp[c] - self.last_sent_amp[c]).abs() > cfg.communication.amplitude_threshold {
                return true;
            }
            if feat.phase && circ_dist(self.phase[c], self.last_sent_phase[c]) > cfg.communication.phase_threshold {
                return true;
            }
        }
        false
    }

    /// Returns (tentative dominant candidate or DEC_NONE, margin, previous dominant).
    fn update_decision(&mut self, cfg: &Config, tick: u64, prev_amp: &[f64], events: &mut Vec<NodeEvent>) -> (u8, f64, u8) {
        let d = &cfg.decision;
        let (top, margin) = top_two(&self.amp);
        let new_dom = if top != DEC_NONE && margin >= d.tentative_margin { top } else { DEC_NONE };
        if new_dom != DEC_NONE {
            if self.last_dom != DEC_NONE && new_dom != self.last_dom {
                self.flips += 1;
            }
            self.last_dom = new_dom;
        }
        let max_delta = self.amp.iter().zip(prev_amp.iter()).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
        let prev_dom = self.dominant;
        self.dominant = new_dom;
        match self.state {
            DecState::ConflictPending => {}
            DecState::Stable => {
                if new_dom != self.dec_cand || margin < d.exit_margin {
                    self.revocations += 1;
                    events.push(NodeEvent {
                        kind: "stable_revoke",
                        a: self.dec_cand as i64,
                        b: new_dom as i64,
                    });
                    self.state = if new_dom != DEC_NONE { DecState::Tentative } else { DecState::Undecided };
                    self.dec_cand = DEC_NONE;
                    self.stable_counter = 0;
                }
            }
            DecState::Undecided | DecState::Tentative => {
                if new_dom == DEC_NONE {
                    self.state = DecState::Undecided;
                    self.stable_counter = 0;
                } else {
                    self.state = DecState::Tentative;
                    if margin >= d.stable_margin && new_dom == prev_dom && max_delta <= d.stable_eps {
                        self.stable_counter += 1;
                    } else {
                        self.stable_counter = 0;
                    }
                    if self.stable_counter >= d.stable_window {
                        self.state = DecState::Stable;
                        self.dec_cand = new_dom;
                        if self.first_stable_tick.is_none() {
                            self.first_stable_tick = Some(tick);
                        }
                        events.push(NodeEvent {
                            kind: "stable_enter",
                            a: new_dom as i64,
                            b: 0,
                        });
                    }
                }
            }
        }
        (new_dom, margin, prev_dom)
    }

    /// ASSUMPTION[R1] A conflicted node returns to TENTATIVE when, after a minimum
    /// time, its own dominant candidate has a stable margin for `reconcile_hold`
    /// ticks and no fresh neighbour still advertises a *different* stable decision.
    /// If this never happens the node stays CONFLICT_PENDING (timeout is only logged).
    fn try_release(&mut self, cfg: &Config, tick: u64, new_dom: u8, margin: f64, events: &mut Vec<NodeEvent>) {
        let since = self.conflict_since.unwrap_or(tick);
        let age = tick.saturating_sub(since);
        if age >= cfg.conflict.reconcile_timeout && !self.timeout_flagged {
            self.timeout_flagged = true;
            self.reconcile_timeouts += 1;
            events.push(NodeEvent {
                kind: "conflict_timeout",
                a: new_dom as i64,
                b: 0,
            });
        }
        if new_dom != DEC_NONE && margin >= cfg.decision.stable_margin && age >= cfg.conflict.reconcile_min_ticks {
            if new_dom == self.release_dom {
                self.release_counter += 1;
            } else {
                self.release_dom = new_dom;
                self.release_counter = 1;
            }
        } else {
            self.release_counter = 0;
            self.release_dom = DEC_NONE;
        }
        if self.release_counter >= cfg.conflict.reconcile_hold {
            let ttl = cfg.communication.cache_ttl;
            let blocked = self
                .cache
                .iter()
                .flatten()
                .any(|e| tick.saturating_sub(e.recv_tick) <= ttl && e.decision < DEC_CONFLICT && e.decision != new_dom);
            if !blocked {
                self.state = DecState::Tentative;
                self.stable_counter = 0;
                self.dec_cand = DEC_NONE;
                self.conflict_since = None;
                self.release_counter = 0;
                self.release_dom = DEC_NONE;
                events.push(NodeEvent {
                    kind: "conflict_release",
                    a: new_dom as i64,
                    b: age as i64,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gossip_feat() -> Features {
        Features {
            damping: false,
            phase: false,
            integrity: false,
            witness: false,
            reconcile: false,
        }
    }

    fn full_feat() -> Features {
        Features {
            damping: true,
            phase: false,
            integrity: true,
            witness: true,
            reconcile: true,
        }
    }

    fn node(amp: [f64; 2]) -> Node {
        Node::new(0, vec![1], amp.to_vec(), vec![0.0, 0.0], None)
    }

    fn feed(n: &mut Node, tick: u64, amp: [f64; 2], decision: u8, seq: u64) {
        n.receive_state(1, &amp, &[0.0, 0.0], decision, tick, tick, seq);
    }

    #[test]
    fn reaches_local_stability_under_consensus() {
        let cfg = Config::default();
        let feat = gossip_feat();
        let mut n = node([0.8, 0.2]);
        for t in 0..30u64 {
            feed(&mut n, t, [0.8, 0.2], DEC_NONE, t + 1);
            n.tick(&cfg, &feat, t);
        }
        assert_eq!(n.state, DecState::Stable);
        assert_eq!(n.dec_cand, 0);
        assert!(n.first_stable_tick.is_some());
    }

    #[test]
    fn small_margin_never_stable() {
        let cfg = Config::default();
        let feat = gossip_feat();
        let mut n = node([0.51, 0.49]);
        for t in 0..40u64 {
            feed(&mut n, t, [0.51, 0.49], DEC_NONE, t + 1);
            n.tick(&cfg, &feat, t);
        }
        assert_eq!(n.state, DecState::Undecided);
    }

    #[test]
    fn stable_decision_is_revoked_when_margin_collapses() {
        let cfg = Config::default();
        let feat = gossip_feat();
        let mut n = node([0.8, 0.2]);
        for t in 0..30u64 {
            feed(&mut n, t, [0.8, 0.2], DEC_NONE, t + 1);
            n.tick(&cfg, &feat, t);
        }
        assert_eq!(n.state, DecState::Stable);
        // Clear the fresh neighbour cache so this test isolates local revocation
        // behavior instead of allowing the cached consensus sample to restore
        // the candidate-0 margin during the tick update.
        n.cache[0] = None;
        n.amp = vec![0.5, 0.5];
        n.tick(&cfg, &feat, 30);
        assert_ne!(n.state, DecState::Stable);
        assert_eq!(n.revocations, 1);
    }

    #[test]
    fn heartbeat_and_initial_transmission() {
        let mut cfg = Config::default();
        cfg.decision.stable_window = 10_000;
        let feat = gossip_feat();
        let mut n = node([0.5, 0.5]);
        let mut kinds = Vec::new();
        for t in 0..25u64 {
            if let Some(m) = n.tick(&cfg, &feat, t).state_msg {
                kinds.push((t, m.kind));
            }
        }
        assert_eq!(kinds, vec![(0, MsgKind::State), (10, MsgKind::Heartbeat), (20, MsgKind::Heartbeat)]);
    }

    #[test]
    fn delta_gating_triggers_state_update() {
        let mut cfg = Config::default();
        cfg.decision.stable_window = 10_000;
        let feat = gossip_feat();
        let mut n = node([0.5, 0.5]);
        assert!(n.tick(&cfg, &feat, 0).state_msg.is_some());
        assert!(n.tick(&cfg, &feat, 1).state_msg.is_none());
        n.amp = vec![0.6, 0.4];
        let m = n.tick(&cfg, &feat, 2).state_msg.expect("gate must fire");
        assert_eq!(m.kind, MsgKind::State);
        n.amp = vec![0.6005, 0.3995];
        assert!(n.tick(&cfg, &feat, 3).state_msg.is_none(), "below threshold must be gated");
    }

    #[test]
    fn opposing_stable_neighbour_creates_witness_and_conflict() {
        let cfg = Config::default();
        let feat = full_feat();
        let mut n = node([0.8, 0.2]);
        for t in 0..30u64 {
            feed(&mut n, t, [0.8, 0.2], DEC_NONE, t + 1);
            n.tick(&cfg, &feat, t);
        }
        assert_eq!(n.state, DecState::Stable);
        feed(&mut n, 30, [0.8, 0.2], 1, 100);
        let out = n.tick(&cfg, &feat, 30);
        assert!(out.witness.is_some());
        assert_eq!(n.state, DecState::ConflictPending);
        assert_eq!(n.conflict_entries, 1);
        // Without the witness feature nothing happens.
        let mut m = node([0.8, 0.2]);
        let g = gossip_feat();
        for t in 0..30u64 {
            feed(&mut m, t, [0.8, 0.2], DEC_NONE, t + 1);
            m.tick(&cfg, &g, t);
        }
        feed(&mut m, 30, [0.8, 0.2], 1, 100);
        assert!(m.tick(&cfg, &g, 30).witness.is_none());
        assert_eq!(m.state, DecState::Stable);
    }

    #[test]
    fn received_witness_triggers_conflict_once_and_forwards_with_decremented_ttl() {
        let cfg = Config::default();
        let feat = full_feat();
        let mut n = node([0.8, 0.2]);
        for t in 0..30u64 {
            feed(&mut n, t, [0.8, 0.2], DEC_NONE, t + 1);
            n.tick(&cfg, &feat, t);
        }
        let mut ev = Vec::new();
        let fwd = n.on_witness(&feat, 31, 5, 0, 0, 1, 4, &mut ev);
        assert_eq!(fwd, Some(3));
        assert_eq!(n.state, DecState::ConflictPending);
        let dup = n.on_witness(&feat, 32, 5, 0, 0, 1, 4, &mut ev);
        assert_eq!(dup, None, "duplicate witness must not be forwarded again");
        assert_eq!(n.conflict_entries, 1);
        assert_eq!(n.on_witness(&feat, 33, 6, 0, 0, 1, 1, &mut ev), None, "ttl exhausted");
    }

    #[test]
    fn conflict_is_released_only_after_min_time_and_stays_local() {
        let cfg = Config::default();
        let feat = full_feat();
        let mut n = node([0.8, 0.2]);
        for t in 0..30u64 {
            feed(&mut n, t, [0.8, 0.2], DEC_NONE, t + 1);
            n.tick(&cfg, &feat, t);
        }
        let mut ev = Vec::new();
        n.on_witness(&feat, 30, 5, 0, 0, 1, 4, &mut ev);
        assert_eq!(n.state, DecState::ConflictPending);
        for t in 31..40u64 {
            feed(&mut n, t, [0.8, 0.2], DEC_NONE, t + 1);
            n.tick(&cfg, &feat, t);
        }
        assert_eq!(n.state, DecState::ConflictPending, "must not release before reconcile_min_ticks + hold");
        for t in 40..90u64 {
            feed(&mut n, t, [0.8, 0.2], DEC_NONE, t + 1);
            n.tick(&cfg, &feat, t);
        }
        assert!(matches!(n.state, DecState::Tentative | DecState::Stable));
        assert!(n.conflict_since.is_none());
    }

    #[test]
    fn stale_or_reordered_messages_are_ignored() {
        let mut n = node([0.5, 0.5]);
        assert!(n.receive_state(1, &[0.6, 0.4], &[0.0, 0.0], DEC_NONE, 5, 6, 10));
        assert!(!n.receive_state(1, &[0.9, 0.1], &[0.0, 0.0], DEC_NONE, 3, 7, 9));
        assert!((n.cache[0].as_ref().unwrap().amp[0] - 0.6).abs() < 1e-12);
        assert!(!n.receive_state(7, &[0.5, 0.5], &[0.0, 0.0], DEC_NONE, 1, 1, 1), "non-neighbour ignored");
    }
}
