//! End-to-end invariants of the simulator. These tests deliberately assert
//! *engine invariants* (determinism, accounting, normalisation, scenario wiring),
//! not performance claims about CR-QISL.

use crqisl::config::{Config, Variant};
use crqisl::experiment;
use crqisl::metrics::{RunSummary, METRIC_COLUMNS};
use crqisl::sim::{self, Setup};
use std::path::PathBuf;

fn small() -> Config {
    let mut c = Config::default();
    c.nodes = 40;
    c.max_ticks = 160;
    c.network.partition_start = 30;
    c.network.partition_end = 80;
    for e in c.events.iter_mut() {
        e.tick = 31;
    }
    c.batch.seeds = 2;
    c.batch.threads = 2;
    c
}

fn run(c: &Config, v: Variant, seed: u64) -> sim::RunOutput {
    sim::run(&c.for_run(v, seed), false).expect("run must succeed")
}

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("crqisl_it_{}_{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&p);
    p
}

#[test]
fn identical_seed_gives_identical_results() {
    let c = small();
    let a = run(&c, Variant::FullCrQisl, 5);
    let b = run(&c, Variant::FullCrQisl, 5);
    assert_eq!(a.summary.csv_row(), b.summary.csv_row());
    assert_eq!(a.series.to_csv(), b.series.to_csv());
    assert_eq!(a.events.len(), b.events.len());
    assert_eq!(a.final_amp, b.final_amp);
}

#[test]
fn different_seeds_give_different_runs() {
    let c = small();
    let a = run(&c, Variant::FullCrQisl, 1);
    let b = run(&c, Variant::FullCrQisl, 2);
    assert_ne!(a.summary.metrics, b.summary.metrics);
}

#[test]
fn summary_contains_every_declared_metric() {
    let c = small();
    let a = run(&c, Variant::WeightedGossip, 1);
    for col in METRIC_COLUMNS {
        assert!(a.summary.metrics.contains_key(*col), "missing metric {}", col);
    }
}

#[test]
fn variants_share_identical_experimental_conditions() {
    let c = small();
    let base = Setup::build(&c.for_run(Variant::WeightedGossip, 3)).unwrap();
    for v in Variant::all() {
        let s = Setup::build(&c.for_run(v, 3)).unwrap();
        assert_eq!(s.graph, base.graph);
        assert_eq!(s.groups, base.groups);
        assert_eq!(s.adversaries, base.adversaries);
        assert_eq!(s.init_amp, base.init_amp);
        assert_eq!(s.init_phase, base.init_phase);
    }
}

#[test]
fn message_accounting_identity_holds_for_all_variants() {
    let mut c = small();
    c.adversary.fraction = 0.1;
    c.adversary.behavior = "invalid".into();
    for v in Variant::all() {
        let o = run(&c, v, 4);
        let g = |k: &str| o.summary.get(k);
        let lhs = g("messages_sent");
        let rhs = g("messages_delivered") + g("dropped_loss") + g("dropped_partition") + g("undelivered_end");
        assert_eq!(lhs, rhs, "variant {:?}", v);
        assert_eq!(g("signature_ops"), 0.0);
        assert_eq!(g("retransmissions"), 0.0);
    }
}

#[test]
fn partition_drops_only_when_enabled() {
    let mut c = small();
    let with = run(&c, Variant::WeightedGossip, 1);
    assert!(with.summary.get("dropped_partition") > 0.0);
    c.network.partition_enabled = false;
    let without = run(&c, Variant::WeightedGossip, 1);
    assert_eq!(without.summary.get("dropped_partition"), 0.0);
    assert!(without.summary.get("divergence_post_heal").is_nan(), "no healing, no post-heal window");
}

#[test]
fn zero_loss_has_no_loss_drops() {
    let mut c = small();
    c.network.packet_loss = 0.0;
    let o = run(&c, Variant::FullCrQisl, 1);
    assert_eq!(o.summary.get("dropped_loss"), 0.0);
}

#[test]
fn honest_amplitudes_stay_l1_normalised_and_finite() {
    let c = small();
    for v in Variant::all() {
        let o = run(&c, v, 2);
        for amp in &o.final_amp {
            let s: f64 = amp.iter().sum();
            assert!(amp.iter().all(|a| a.is_finite() && *a >= 0.0), "variant {:?}", v);
            assert!((s - 1.0).abs() < 1e-9, "variant {:?} sum {}", v, s);
        }
        assert_eq!(o.summary.get("nan_corrupted_nodes"), 0.0);
    }
}

#[test]
fn gossip_variants_have_no_conflict_machinery() {
    let c = small();
    for v in [Variant::WeightedGossip, Variant::WeightedGossipDamping] {
        let o = run(&c, v, 1);
        assert_eq!(o.summary.get("conflict_entries"), 0.0);
        assert_eq!(o.summary.get("witness_messages"), 0.0);
        assert_eq!(o.summary.get("reconciliation_messages"), 0.0);
    }
}

#[test]
fn partition_produces_split_decisions_before_healing() {
    // Whether a given seed splits depends on its random graph/cut, so require it for at least one of 5 seeds.
    let c = small();
    for v in Variant::all() {
        let split_somewhere = (1..=5u64).any(|seed| run(&c, v, seed).summary.get("split_ticks_pre_heal") > 0.0);
        assert!(split_somewhere, "variant {:?} never split during the partition for seeds 1..=5", v);
    }
}

#[test]
fn healing_triggers_witnesses_and_reconciliation_in_cr_qisl_variants() {
    let c = small();
    for v in [Variant::PhaseAblation, Variant::FullCrQisl] {
        let engaged = (1..=6u64).any(|seed| {
            let o = run(&c, v, seed);
            o.summary.get("conflict_entries") > 0.0 && o.summary.get("witness_messages") > 0.0 && o.summary.get("reconciliation_messages") > 0.0
        });
        assert!(engaged, "variant {:?}: conflict machinery never engaged for seeds 1..=6", v);
    }
}

#[test]
fn invalid_inputs_are_rejected_with_validation_and_corrupt_without() {
    let mut c = small();
    c.network.packet_loss = 0.0;
    c.adversary.fraction = 0.1;
    c.adversary.behavior = "invalid".into();
    c.communication.validate_inputs = true;
    let safe = run(&c, Variant::WeightedGossip, 1);
    assert!(safe.summary.get("dropped_invalid") > 0.0);
    assert_eq!(safe.summary.get("nan_corrupted_nodes"), 0.0);
    c.communication.validate_inputs = false;
    let bad = run(&c, Variant::WeightedGossip, 1);
    assert!(bad.summary.get("nan_corrupted_nodes") > 0.0, "without validation NaN must propagate");
}

#[test]
fn adversary_scenarios_run_for_all_variants_and_placements() {
    let mut base = small();
    base.nodes = 30;
    base.max_ticks = 100;
    base.network.partition_start = 20;
    base.network.partition_end = 50;
    for e in base.events.iter_mut() {
        e.tick = 21;
    }
    for behavior in ["amplitude", "phase", "invalid"] {
        for placement in ["random", "clustered"] {
            let mut c = base.clone();
            c.adversary.fraction = 0.2;
            c.adversary.behavior = behavior.into();
            c.adversary.placement = placement.into();
            for v in Variant::all() {
                let o = run(&c, v, 1);
                assert_eq!(o.summary.get("adversarial_nodes"), 6.0);
                assert!(o.summary.get("adversary_messages") > 0.0);
            }
        }
    }
}

#[test]
fn clustered_placement_forms_a_connected_ball() {
    let mut c = small();
    c.adversary.fraction = 0.25;
    c.adversary.behavior = "amplitude".into();
    c.adversary.placement = "clustered".into();
    let s = Setup::build(&c).unwrap();
    let adv: Vec<usize> = (0..c.nodes).filter(|&i| s.adversaries[i].is_some()).collect();
    assert_eq!(adv.len(), 10);
    let touching = adv.iter().filter(|&&i| s.graph.adj[i].iter().any(|j| adv.contains(j))).count();
    assert!(touching >= adv.len() - 1, "BFS ball must be (nearly) connected");
}

#[test]
fn every_topology_runs() {
    for kind in ["ring", "erdos_renyi", "small_world", "scale_free", "grid", "complete"] {
        let mut c = small();
        c.nodes = 30;
        c.max_ticks = 90;
        c.network.partition_end = 60;
        c.topology.kind = kind.into();
        if kind == "erdos_renyi" {
            c.topology.p = 0.1;
        }
        let o = run(&c, Variant::FullCrQisl, 1);
        assert_eq!(o.summary.get("honest_nodes"), 30.0, "{}", kind);
    }
}

#[test]
fn asymmetric_latency_and_exponential_latency_run() {
    let mut c = small();
    c.network.asymmetry = 3.0;
    c.network.latency_kind = "exponential".into();
    let o = run(&c, Variant::FullCrQisl, 1);
    assert!(o.summary.get("messages_delivered") > 0.0);
}

#[test]
fn short_horizon_is_flagged_as_censored_not_silently_dropped() {
    let mut c = small();
    c.max_ticks = 85; // only 5 ticks after healing; hold_ticks = 10 cannot be satisfied
    let o = run(&c, Variant::FullCrQisl, 1);
    assert_eq!(o.summary.get("agreement_censored"), 1.0);
    assert_eq!(o.summary.get("quorum_censored"), 1.0);
    assert_eq!(o.summary.get("convergence_censored"), 1.0);
    assert_eq!(o.summary.get("agreement_time_after_heal"), 5.0);
    assert_eq!(o.summary.get("reference_tick"), 80.0);
}

#[test]
fn phase_ablation_equals_full_model_with_phase_forced_off() {
    let c = small();
    let ablation = run(&c, Variant::PhaseAblation, 3);
    let mut forced = c.clone();
    forced.features.phase = "off".into();
    let full_no_phase = run(&forced, Variant::FullCrQisl, 3);
    assert_eq!(ablation.summary.metrics, full_no_phase.summary.metrics);
}

#[test]
fn invalid_configuration_is_rejected() {
    let mut c = small();
    c.nodes = 1;
    assert!(sim::run(&c, false).is_err());
    let mut d = small();
    d.network.packet_loss = 2.0;
    assert!(sim::run(&d, false).is_err());
}

#[test]
fn trace_has_expected_schema_and_shapes() {
    let c = small().for_run(Variant::FullCrQisl, 1);
    let o = sim::run(&c, true).unwrap();
    let v = sim::trace_json(&c, &o).expect("trace recorded");
    assert_eq!(v["schema"], "crqisl-trace/1");
    let ticks = v["frames"]["ticks"].as_array().unwrap().len();
    assert_eq!(ticks, c.max_ticks as usize);
    assert_eq!(v["frames"]["amp"][0].as_array().unwrap().len(), c.nodes * c.candidates);
    assert_eq!(v["frames"]["state"][0].as_array().unwrap().len(), c.nodes);
    assert_eq!(v["nodes"]["group"].as_array().unwrap().len(), c.nodes);
    assert!(v["series"]["stable_0"].as_array().unwrap().len() == ticks);
    assert!(!v["edges"].as_array().unwrap().is_empty());
    assert!(v["events"].as_array().unwrap().iter().any(|e| e["kind"] == "partition_heal"));
    // Without recording there is no trace.
    let o2 = sim::run(&c, false).unwrap();
    assert!(sim::trace_json(&c, &o2).is_none());
}

#[test]
fn batch_is_reproducible_and_exports_everything() {
    let mut c = small();
    c.nodes = 30;
    c.max_ticks = 100;
    c.network.partition_start = 20;
    c.network.partition_end = 50;
    for e in c.events.iter_mut() {
        e.tick = 21;
    }
    c.batch.seeds = 3;
    c.batch.seed_start = 10;
    let d1 = tmp("batch1");
    let d2 = tmp("batch2");
    let r1 = experiment::run_batch(&c, &d1).unwrap();
    c.batch.threads = 1; // thread count must not change results
    let r2 = experiment::run_batch(&c, &d2).unwrap();
    assert_eq!(r1.summaries.len(), 12);
    assert_eq!(std::fs::read_to_string(d1.join("runs.csv")).unwrap(), std::fs::read_to_string(d2.join("runs.csv")).unwrap());
    assert_eq!(
        std::fs::read_to_string(d1.join("analysis/report.md")).unwrap(),
        std::fs::read_to_string(d2.join("analysis/report.md")).unwrap()
    );
    for f in ["config.resolved.json", "manifest.json", "runs.csv", "runs.json", "analysis/descriptive.csv", "analysis/paired_tests.csv", "analysis/report.md"] {
        assert!(d1.join(f).exists(), "missing {}", f);
    }
    assert!(d1.join("series/full_cr_qisl_seed10.csv").exists());
    assert!(d1.join("events/weighted_gossip_seed11.csv").exists());
    assert!(d1.join("traces/full_cr_qisl_seed10.trace.json").exists(), "first seed is traced by default");
    assert!(!d1.join("traces/full_cr_qisl_seed11.trace.json").exists());
    // runs.csv round-trips into the analysis pipeline
    let rows = RunSummary::from_csv(&std::fs::read_to_string(d1.join("runs.csv")).unwrap()).unwrap();
    assert_eq!(rows.len(), 12);
    let again = crqisl::analysis::analyze(&rows, 0.05);
    assert_eq!(again.to_markdown(), r1.report.to_markdown());
    let _ = std::fs::remove_dir_all(&d1);
    let _ = std::fs::remove_dir_all(&d2);
}
