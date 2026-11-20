//! Experiment configuration. Every tunable lives here; nothing is hard-coded in
//! the engine. `Config::default()` reproduces the *initial experiment*
//! (n = 100, 2 candidates, 20% loss, asynchronous latency, partition at tick 60,
//! healing at tick 140, horizon 300, 30 seeds).
//!
//! Parameters marked [ASSUMPTION] belong to operational assumptions introduced to
//! make the simulator runnable; they are NOT claimed to be CR-QISL mathematics.
//! See docs/ASSUMPTIONS.md.

use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Variant {
    WeightedGossip,
    WeightedGossipDamping,
    PhaseAblation,
    FullCrQisl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Features {
    pub damping: bool,
    pub phase: bool,
    pub integrity: bool,
    pub witness: bool,
    pub reconcile: bool,
}

impl Variant {
    pub fn all() -> Vec<Variant> {
        vec![
            Variant::WeightedGossip,
            Variant::WeightedGossipDamping,
            Variant::PhaseAblation,
            Variant::FullCrQisl,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Variant::WeightedGossip => "weighted_gossip",
            Variant::WeightedGossipDamping => "weighted_gossip_damping",
            Variant::PhaseAblation => "phase_ablation",
            Variant::FullCrQisl => "full_cr_qisl",
        }
    }

    pub fn parse(s: &str) -> Option<Variant> {
        match s.trim().to_ascii_lowercase().as_str() {
            "a" | "weighted_gossip" => Some(Variant::WeightedGossip),
            "b" | "weighted_gossip_damping" => Some(Variant::WeightedGossipDamping),
            "c" | "phase_ablation" => Some(Variant::PhaseAblation),
            "d" | "full_cr_qisl" | "full" => Some(Variant::FullCrQisl),
            _ => None,
        }
    }

    /// Feature preset of the variant. All variants share the same communication
    /// policy (gating, heartbeat, validation) and the same local decision rule.
    pub fn preset(&self) -> Features {
        match self {
            Variant::WeightedGossip => Features {
                damping: false,
                phase: false,
                integrity: false,
                witness: false,
                reconcile: false,
            },
            Variant::WeightedGossipDamping => Features {
                damping: true,
                phase: false,
                integrity: false,
                witness: false,
                reconcile: false,
            },
            Variant::PhaseAblation => Features {
                damping: true,
                phase: false,
                integrity: true,
                witness: true,
                reconcile: true,
            },
            Variant::FullCrQisl => Features {
                damping: true,
                phase: true,
                integrity: true,
                witness: true,
                reconcile: true,
            },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TopologyConfig {
    /// "ring" | "erdos_renyi" | "small_world" | "scale_free" | "grid" | "complete"
    pub kind: String,
    /// Degree for ring / small_world (even).
    pub k: usize,
    /// Edge probability (erdos_renyi) or rewiring probability (small_world).
    pub p: f64,
    /// Attachment count for scale_free.
    pub m: usize,
    /// If true, disconnected components are joined by random bridge edges.
    pub ensure_connected: bool,
}

impl Default for TopologyConfig {
    fn default() -> Self {
        TopologyConfig {
            kind: "small_world".into(),
            k: 6,
            p: 0.1,
            m: 3,
            ensure_connected: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ModelConfig {
    /// [ASSUMPTION A5] Damping coefficient alpha in [0,1): the update step is scaled by (1 - alpha).
    pub alpha: f64,
    /// [ASSUMPTION A3] Resonance / phase-coupling beta in [0,1]: blend between arithmetic amplitude
    /// mean (0) and interference magnitude |complex mean| (1). Used only when phase dynamics are on.
    pub beta: f64,
    /// [ASSUMPTION A4] Base gossip step size epsilon in (0,1].
    pub rate: f64,
    /// [ASSUMPTION A7] Optional intrinsic phase rotation per tick (0 disables).
    pub omega: f64,
    /// [ASSUMPTION A7] Rotate cached neighbour phases forward by omega * age.
    pub age_compensation: bool,
}

impl Default for ModelConfig {
    fn default() -> Self {
        ModelConfig {
            alpha: 0.5,
            beta: 0.5,
            rate: 0.5,
            omega: 0.0,
            age_compensation: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InitialConfig {
    /// "uniform" | "random_bias" | "partition_aligned" | "fixed"
    pub amplitude_mode: String,
    /// Gaussian noise added to initial amplitudes before L1 normalisation.
    pub amplitude_noise: f64,
    /// Bias strength for random_bias / partition_aligned.
    pub bias_strength: f64,
    /// Explicit amplitude vector (length = candidates) for mode "fixed".
    pub amplitudes: Vec<f64>,
    /// "zero" | "random" | "candidate_offset" | "fixed"
    pub phase_mode: String,
    /// Explicit phase vector (length = candidates) for phase_mode "fixed".
    pub phases: Vec<f64>,
}

impl Default for InitialConfig {
    fn default() -> Self {
        InitialConfig {
            amplitude_mode: "uniform".into(),
            amplitude_noise: 0.02,
            bias_strength: 0.3,
            amplitudes: vec![],
            phase_mode: "zero".into(),
            phases: vec![],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NetworkConfig {
    pub packet_loss: f64,
    /// "fixed" | "uniform" | "exponential"
    pub latency_kind: String,
    pub latency_min: f64,
    pub latency_max: f64,
    /// Mean of the exponential extra delay (kind "exponential").
    pub latency_mean_extra: f64,
    pub latency_cap: u64,
    /// Latency multiplier for the direction from higher to lower node id (1.0 = symmetric).
    pub asymmetry: f64,
    pub partition_enabled: bool,
    pub partition_start: u64,
    pub partition_end: u64,
    /// "bfs" | "random" | "index"
    pub partition_mode: String,
    /// Fraction of nodes in group 0 (the larger side carries more evidence mass).
    pub partition_fraction: f64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig {
            packet_loss: 0.2,
            latency_kind: "uniform".into(),
            latency_min: 1.0,
            latency_max: 5.0,
            latency_mean_extra: 2.0,
            latency_cap: 50,
            asymmetry: 1.0,
            partition_enabled: true,
            partition_start: 60,
            partition_end: 140,
            partition_mode: "bfs".into(),
            partition_fraction: 0.7,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CommConfig {
    pub heartbeat_period: u64,
    /// Delta gating: transmit when any |amplitude change| exceeds this since the last transmission.
    pub amplitude_threshold: f64,
    /// Delta gating on circular phase change (radians); only used when phase dynamics are on.
    pub phase_threshold: f64,
    /// Cached neighbour states older than this (ticks since receipt) are ignored.
    pub cache_ttl: u64,
    /// Reject incoming messages with non-finite / negative / non-normalised values.
    pub validate_inputs: bool,
}

impl Default for CommConfig {
    fn default() -> Self {
        CommConfig {
            heartbeat_period: 10,
            amplitude_threshold: 0.005,
            phase_threshold: 0.05,
            cache_ttl: 30,
            validate_inputs: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DecisionConfig {
    pub tentative_margin: f64,
    pub stable_margin: f64,
    pub exit_margin: f64,
    pub stable_window: u32,
    pub stable_eps: f64,
}

impl Default for DecisionConfig {
    fn default() -> Self {
        DecisionConfig {
            tentative_margin: 0.04,
            stable_margin: 0.08,
            exit_margin: 0.04,
            stable_window: 12,
            stable_eps: 0.01,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct IntegrityConfig {
    /// [ASSUMPTION A6] Learning rate of the integrity weight.
    pub eta: f64,
    /// [ASSUMPTION A6] Sensitivity: target weight = exp(-kappa * deviation).
    pub kappa: f64,
    pub min_weight: f64,
    /// [ASSUMPTION A6] Weight of the phase-deviation term.
    pub phase_gamma: f64,
}

impl Default for IntegrityConfig {
    fn default() -> Self {
        IntegrityConfig {
            eta: 0.1,
            kappa: 8.0,
            min_weight: 0.05,
            phase_gamma: 0.5,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConflictConfig {
    pub witness_ttl: u8,
    pub witness_cooldown: u64,
    pub reconcile_period: u64,
    /// [ASSUMPTION R2] Step multiplier (clamped to 1) while CONFLICT_PENDING and reconciliation is on.
    pub reconcile_rate_boost: f64,
    pub reconcile_min_ticks: u64,
    pub reconcile_hold: u32,
    pub reconcile_timeout: u64,
}

impl Default for ConflictConfig {
    fn default() -> Self {
        ConflictConfig {
            witness_ttl: 8,
            witness_cooldown: 10,
            reconcile_period: 5,
            reconcile_rate_boost: 1.5,
            reconcile_min_ticks: 10,
            reconcile_hold: 8,
            reconcile_timeout: 80,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AdversaryConfig {
    pub fraction: f64,
    /// "none" | "amplitude" | "phase" | "invalid"
    pub behavior: String,
    /// "random" | "clustered"
    pub placement: String,
    pub target_candidate: usize,
    /// Amplitude put on the target candidate by the amplitude attack.
    pub forged_amplitude: f64,
    pub start_tick: u64,
    /// 0 = attack lasts until the end of the run.
    pub end_tick: u64,
    pub period: u64,
    /// If true, forged messages also claim a stable decision for `target_candidate`
    /// (this lets the attacker try to trigger conflict witnesses).
    pub claim_decision: bool,
}

impl Default for AdversaryConfig {
    fn default() -> Self {
        AdversaryConfig {
            fraction: 0.0,
            behavior: "none".into(),
            placement: "random".into(),
            target_candidate: 1,
            forged_amplitude: 0.95,
            start_tick: 0,
            end_tick: 0,
            period: 1,
            claim_decision: false,
        }
    }
}

/// "auto" follows the variant preset; "on" / "off" force a feature.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FeatureOverrides {
    pub damping: String,
    pub phase: String,
    pub integrity: String,
    pub witness: String,
    pub reconcile: String,
}

impl Default for FeatureOverrides {
    fn default() -> Self {
        FeatureOverrides {
            damping: "auto".into(),
            phase: "auto".into(),
            integrity: "auto".into(),
            witness: "auto".into(),
            reconcile: "auto".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MetricsConfig {
    /// Amplitude-spread threshold for "amplitude convergence".
    pub convergence_eps: f64,
    /// Number of consecutive ticks a convergence / agreement condition must hold.
    pub hold_ticks: u64,
    /// Fraction of honest nodes that must hold the same Stable decision for "quorum agreement"
    /// (observer-level, secondary to the strict all-nodes agreement metric).
    pub quorum_fraction: f64,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        MetricsConfig {
            convergence_eps: 0.02,
            hold_ticks: 10,
            quorum_fraction: 0.9,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TelemetryConfig {
    /// Record per-node amplitude/phase/state frames every `history_stride` ticks.
    pub history_stride: u64,
    pub max_events: usize,
    /// For the `run` command: record per-node history (batch uses `batch.trace_seeds`).
    pub per_node_history: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        TelemetryConfig {
            history_stride: 1,
            max_events: 200_000,
            per_node_history: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BatchConfig {
    pub seeds: u64,
    pub seed_start: u64,
    pub variants: Vec<Variant>,
    /// 0 = use available parallelism.
    pub threads: usize,
    /// Seeds for which full per-node traces are exported (empty = first seed only).
    pub trace_seeds: Vec<u64>,
}

impl Default for BatchConfig {
    fn default() -> Self {
        BatchConfig {
            seeds: 30,
            seed_start: 1,
            variants: Variant::all(),
            threads: 0,
            trace_seeds: vec![],
        }
    }
}

/// Scheduled exogenous evidence ("competing states" arriving at nodes).
/// [ASSUMPTION A8] The brief does not define how two competing states become
/// locally dominant on each side of a partition; scheduled injections do.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EventConfig {
    /// Currently only "inject".
    pub kind: String,
    pub tick: u64,
    /// Evidence-recipient group receiving the injection; -1 = all honest nodes.
    pub group: i64,
    pub candidate: usize,
    /// amplitude <- (1 - strength) * amplitude + strength * onehot(candidate)
    pub strength: f64,
    /// Phase assigned to the injected candidate (only if the phase feature is on).
    pub phase: f64,
}

impl Default for EventConfig {
    fn default() -> Self {
        EventConfig {
            kind: "inject".into(),
            tick: 61,
            group: 0,
            candidate: 0,
            strength: 0.8,
            phase: 0.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub name: String,
    pub nodes: usize,
    pub candidates: usize,
    pub max_ticks: u64,
    pub seed: u64,
    pub variant: Variant,
    pub topology: TopologyConfig,
    pub model: ModelConfig,
    pub initial: InitialConfig,
    pub network: NetworkConfig,
    /// "aligned" | "reversed" | "independent_balanced"; evidence recipients are separate from network partitions.
    pub evidence_assignment_mode: String,
    pub communication: CommConfig,
    pub decision: DecisionConfig,
    pub integrity: IntegrityConfig,
    pub conflict: ConflictConfig,
    pub adversary: AdversaryConfig,
    pub features: FeatureOverrides,
    pub metrics: MetricsConfig,
    pub telemetry: TelemetryConfig,
    pub batch: BatchConfig,
    /// Keep last: TOML arrays of tables must follow scalar/table fields.
    pub events: Vec<EventConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            name: "initial_experiment".into(),
            nodes: 100,
            candidates: 2,
            max_ticks: 300,
            seed: 1,
            variant: Variant::FullCrQisl,
            topology: TopologyConfig::default(),
            model: ModelConfig::default(),
            initial: InitialConfig::default(),
            network: NetworkConfig::default(),
            evidence_assignment_mode: "aligned".into(),
            communication: CommConfig::default(),
            decision: DecisionConfig::default(),
            integrity: IntegrityConfig::default(),
            conflict: ConflictConfig::default(),
            adversary: AdversaryConfig::default(),
            features: FeatureOverrides::default(),
            metrics: MetricsConfig::default(),
            telemetry: TelemetryConfig::default(),
            batch: BatchConfig::default(),
            events: vec![
                EventConfig {
                    kind: "inject".into(),
                    tick: 61,
                    group: 0,
                    candidate: 0,
                    strength: 0.8,
                    phase: 0.0,
                },
                EventConfig {
                    kind: "inject".into(),
                    tick: 61,
                    group: 1,
                    candidate: 1,
                    strength: 0.8,
                    phase: 1.5707963,
                },
            ],
        }
    }
}

fn flag(setting: &str, preset: bool) -> bool {
    match setting {
        "on" => true,
        "off" => false,
        _ => preset,
    }
}

impl Config {
    /// Effective feature set: variant preset with explicit overrides applied.
    pub fn effective_features(&self) -> Features {
        let p = self.variant.preset();
        Features {
            damping: flag(&self.features.damping, p.damping),
            phase: flag(&self.features.phase, p.phase),
            integrity: flag(&self.features.integrity, p.integrity),
            witness: flag(&self.features.witness, p.witness),
            reconcile: flag(&self.features.reconcile, p.reconcile),
        }
    }

    pub fn for_run(&self, variant: Variant, seed: u64) -> Config {
        let mut c = self.clone();
        c.variant = variant;
        c.seed = seed;
        c
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// FNV-1a hash of the canonical JSON; recorded in every output for provenance.
    pub fn hash_hex(&self) -> String {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in self.to_json().bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        format!("{:016x}", h)
    }

    pub fn from_toml_str(s: &str) -> Result<Config, String> {
        toml::from_str::<Config>(s).map_err(|e| format!("config parse error: {}", e))
    }

    pub fn load(path: &str) -> Result<Config, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("cannot read {}: {}", path, e))?;
        let cfg = if path.ends_with(".json") {
            serde_json::from_str::<Config>(&text).map_err(|e| format!("config parse error: {}", e))?
        } else {
            Config::from_toml_str(&text)?
        };
        let errs = cfg.validate();
        if errs.is_empty() {
            Ok(cfg)
        } else {
            Err(format!("invalid configuration:\n  - {}", errs.join("\n  - ")))
        }
    }

    pub fn validate(&self) -> Vec<String> {
        let mut e: Vec<String> = Vec::new();
        let prob = |name: &str, v: f64, e: &mut Vec<String>| {
            if !(v.is_finite() && (0.0..=1.0).contains(&v)) {
                e.push(format!("{} must be in [0,1], got {}", name, v));
            }
        };
        if self.nodes < 2 {
            e.push("nodes must be >= 2".into());
        }
        if self.candidates < 2 || self.candidates > 16 {
            e.push("candidates must be in [2,16]".into());
        }
        if self.max_ticks < 1 {
            e.push("max_ticks must be >= 1".into());
        }
        let m = &self.model;
        if !(m.alpha.is_finite() && m.alpha >= 0.0 && m.alpha < 1.0) {
            e.push("model.alpha must be in [0,1)".into());
        }
        prob("model.beta", m.beta, &mut e);
        if !(m.rate.is_finite() && m.rate > 0.0 && m.rate <= 1.0) {
            e.push("model.rate must be in (0,1]".into());
        }
        if !m.omega.is_finite() {
            e.push("model.omega must be finite".into());
        }
        // topology
        let t = &self.topology;
        match t.kind.as_str() {
            "ring" | "small_world" => {
                if t.k < 2 || t.k % 2 != 0 || t.k >= self.nodes {
                    e.push("topology.k must be even, >= 2 and < nodes".into());
                }
                if t.kind == "small_world" {
                    prob("topology.p", t.p, &mut e);
                }
            }
            "erdos_renyi" => prob("topology.p", t.p, &mut e),
            "scale_free" => {
                if t.m < 1 || t.m >= self.nodes {
                    e.push("topology.m must be >= 1 and < nodes".into());
                }
            }
            "grid" | "complete" => {}
            other => e.push(format!("unknown topology.kind '{}'", other)),
        }
        // initial
        let i = &self.initial;
        match i.amplitude_mode.as_str() {
            "uniform" | "random_bias" | "partition_aligned" => {}
            "fixed" => {
                if i.amplitudes.len() != self.candidates || i.amplitudes.iter().any(|x| !x.is_finite() || *x < 0.0) {
                    e.push("initial.amplitudes must have one non-negative value per candidate".into());
                }
            }
            other => e.push(format!("unknown initial.amplitude_mode '{}'", other)),
        }
        match i.phase_mode.as_str() {
            "zero" | "random" | "candidate_offset" => {}
            "fixed" => {
                if i.phases.len() != self.candidates || i.phases.iter().any(|x| !x.is_finite()) {
                    e.push("initial.phases must have one finite value per candidate".into());
                }
            }
            other => e.push(format!("unknown initial.phase_mode '{}'", other)),
        }
        if !(i.amplitude_noise.is_finite() && i.amplitude_noise >= 0.0) {
            e.push("initial.amplitude_noise must be >= 0".into());
        }
        // network
        let n = &self.network;
        prob("network.packet_loss", n.packet_loss, &mut e);
        match n.latency_kind.as_str() {
            "fixed" | "uniform" | "exponential" => {}
            other => e.push(format!("unknown network.latency_kind '{}'", other)),
        }
        if !(n.latency_min.is_finite() && n.latency_min >= 1.0) {
            e.push("network.latency_min must be >= 1 tick (messages never arrive in the tick they are sent)".into());
        }
        if !(n.latency_max.is_finite() && n.latency_max >= n.latency_min) {
            e.push("network.latency_max must be >= latency_min".into());
        }
        if !(n.latency_mean_extra.is_finite() && n.latency_mean_extra >= 0.0) {
            e.push("network.latency_mean_extra must be >= 0".into());
        }
        if n.latency_cap < 1 {
            e.push("network.latency_cap must be >= 1".into());
        }
        if !(n.asymmetry.is_finite() && n.asymmetry > 0.0) {
            e.push("network.asymmetry must be > 0".into());
        }
        if n.partition_enabled {
            if n.partition_end <= n.partition_start {
                e.push("network.partition_end must be > partition_start".into());
            }
            if !(n.partition_fraction > 0.0 && n.partition_fraction < 1.0) {
                e.push("network.partition_fraction must be in (0,1)".into());
            }
            match n.partition_mode.as_str() {
                "bfs" | "random" | "bfs_random_root" | "index" => {}
                other => e.push(format!("unknown network.partition_mode '{}'", other)),
            }
        }
        match self.evidence_assignment_mode.as_str() {
            "aligned" | "reversed" | "independent_balanced" => {}
            other => e.push(format!("unknown evidence_assignment_mode '{}'", other)),
        }
        // communication
        let c = &self.communication;
        if c.heartbeat_period < 1 {
            e.push("communication.heartbeat_period must be >= 1".into());
        }
        if c.cache_ttl < 1 {
            e.push("communication.cache_ttl must be >= 1".into());
        }
        if !(c.amplitude_threshold.is_finite() && c.amplitude_threshold >= 0.0) {
            e.push("communication.amplitude_threshold must be >= 0".into());
        }
        if !(c.phase_threshold.is_finite() && c.phase_threshold >= 0.0) {
            e.push("communication.phase_threshold must be >= 0".into());
        }
        // decision
        let d = &self.decision;
        if d.stable_window < 1 {
            e.push("decision.stable_window must be >= 1".into());
        }
        if !(d.exit_margin <= d.stable_margin && d.tentative_margin <= d.stable_margin) {
            e.push("decision margins must satisfy tentative <= stable and exit <= stable".into());
        }
        if d.tentative_margin < 0.0 || d.exit_margin < 0.0 || d.stable_eps < 0.0 {
            e.push("decision margins / eps must be >= 0".into());
        }
        // integrity
        let g = &self.integrity;
        if !(g.eta > 0.0 && g.eta <= 1.0) {
            e.push("integrity.eta must be in (0,1]".into());
        }
        if !(g.min_weight > 0.0 && g.min_weight <= 1.0) {
            e.push("integrity.min_weight must be in (0,1]".into());
        }
        if !(g.kappa.is_finite() && g.kappa >= 0.0) {
            e.push("integrity.kappa must be >= 0".into());
        }
        if self.conflict.reconcile_period < 1 {
            e.push("conflict.reconcile_period must be >= 1".into());
        }
        if !(self.conflict.reconcile_rate_boost.is_finite() && self.conflict.reconcile_rate_boost > 0.0) {
            e.push("conflict.reconcile_rate_boost must be > 0".into());
        }
        // adversary
        let a = &self.adversary;
        if !(a.fraction.is_finite() && a.fraction >= 0.0 && a.fraction < 1.0) {
            e.push("adversary.fraction must be in [0,1)".into());
        }
        match a.behavior.as_str() {
            "none" | "amplitude" | "phase" | "invalid" => {}
            other => e.push(format!("unknown adversary.behavior '{}'", other)),
        }
        match a.placement.as_str() {
            "random" | "clustered" => {}
            other => e.push(format!("unknown adversary.placement '{}'", other)),
        }
        if a.target_candidate >= self.candidates {
            e.push("adversary.target_candidate out of range".into());
        }
        if a.period < 1 {
            e.push("adversary.period must be >= 1".into());
        }
        if !(a.forged_amplitude >= 0.0 && a.forged_amplitude <= 1.0) {
            e.push("adversary.forged_amplitude must be in [0,1]".into());
        }
        for (name, v) in [
            ("damping", &self.features.damping),
            ("phase", &self.features.phase),
            ("integrity", &self.features.integrity),
            ("witness", &self.features.witness),
            ("reconcile", &self.features.reconcile),
        ] {
            if !matches!(v.as_str(), "auto" | "on" | "off") {
                e.push(format!("features.{} must be auto/on/off", name));
            }
        }
        if self.metrics.hold_ticks < 1 {
            e.push("metrics.hold_ticks must be >= 1".into());
        }
        if !(self.metrics.quorum_fraction > 0.0 && self.metrics.quorum_fraction <= 1.0) {
            e.push("metrics.quorum_fraction must be in (0,1]".into());
        }
        if self.telemetry.history_stride < 1 {
            e.push("telemetry.history_stride must be >= 1".into());
        }
        if self.batch.seeds < 1 {
            e.push("batch.seeds must be >= 1".into());
        }
        if self.batch.variants.is_empty() {
            e.push("batch.variants must not be empty".into());
        }
        for ev in &self.events {
            if ev.kind != "inject" {
                e.push(format!("unknown event kind '{}'", ev.kind));
            }
            if ev.tick >= self.max_ticks {
                e.push(format!("event tick {} is beyond max_ticks", ev.tick));
            }
            if ev.candidate >= self.candidates {
                e.push("event candidate out of range".into());
            }
            if !(0.0..=1.0).contains(&ev.strength) {
                e.push("event strength must be in [0,1]".into());
            }
            if ev.group < -1 || ev.group > 1 {
                e.push("event group must be -1, 0 or 1".into());
            }
        }
        e
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_valid_and_matches_initial_experiment() {
        let c = Config::default();
        assert!(c.validate().is_empty(), "{:?}", c.validate());
        assert_eq!(c.nodes, 100);
        assert_eq!(c.candidates, 2);
        assert_eq!(c.max_ticks, 300);
        assert_eq!(c.batch.seeds, 30);
        assert!((c.network.packet_loss - 0.2).abs() < 1e-12);
        assert_eq!(c.network.partition_start, 60);
        assert_eq!(c.network.partition_end, 140);
    }

    #[test]
    fn toml_round_trip() {
        let c = Config::default();
        let s = toml::to_string_pretty(&c).expect("serialise");
        let back = Config::from_toml_str(&s).expect("parse");
        assert_eq!(c.hash_hex(), back.hash_hex());
    }

    #[test]
    fn partial_toml_uses_defaults() {
        let c = Config::from_toml_str("nodes = 40\n[network]\npacket_loss = 0.0\n").unwrap();
        assert_eq!(c.nodes, 40);
        assert_eq!(c.network.packet_loss, 0.0);
        assert_eq!(c.max_ticks, 300);
    }

    #[test]
    fn validation_catches_errors() {
        let mut c = Config::default();
        c.network.packet_loss = 1.5;
        c.model.alpha = 1.0;
        c.topology.kind = "nonsense".into();
        let errs = c.validate();
        assert!(errs.len() >= 3);
    }

    #[test]
    fn variant_presets_and_overrides() {
        assert!(!Variant::WeightedGossip.preset().damping);
        assert!(Variant::WeightedGossipDamping.preset().damping);
        assert!(!Variant::PhaseAblation.preset().phase);
        assert!(Variant::FullCrQisl.preset().phase);
        let mut c = Config::default();
        c.variant = Variant::FullCrQisl;
        c.features.phase = "off".into();
        assert!(!c.effective_features().phase);
        assert_eq!(Variant::parse("D"), Some(Variant::FullCrQisl));
    }
}
