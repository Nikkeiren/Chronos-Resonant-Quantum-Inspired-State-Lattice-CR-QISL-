//! Batch experiments: variants x seeds under identical conditions, parallel but
//! deterministic (each run is independent and results are ordered by job index),
//! with failure capture, exports and the statistical analysis.
//!
//! Output layout (no timestamps, so reruns are byte-identical):
//!   config.resolved.json / config.resolved.toml   exact configuration + hash
//!   manifest.json                                 provenance
//!   runs.csv / runs.json                          one row per (variant, seed)
//!   series/<variant>_seed<N>.csv                  observer-level per-tick series
//!   events/<variant>_seed<N>.csv                  network / decision event history
//!   traces/<variant>_seed<N>.trace.json           per-node amplitude/phase/state history
//!   analysis/{descriptive.csv,paired_tests.csv,report.md}

use crate::analysis::{self, Report};
use crate::config::Config;
use crate::metrics::{EventRecord, RunSummary};
use crate::sim;
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

struct Job {
    variant: crate::config::Variant,
    seed: u64,
}

struct JobResult {
    label: String,
    summary: RunSummary,
    series_csv: String,
    events_csv: String,
    trace_json: Option<String>,
}

pub struct BatchResult {
    pub summaries: Vec<RunSummary>,
    pub report: Report,
}

pub fn events_csv(events: &[EventRecord]) -> String {
    let mut s = String::from("tick,kind,node,a,b\n");
    for e in events {
        s.push_str(&format!("{},{},{},{},{}\n", e.tick, e.kind, e.node, e.a, e.b));
    }
    s
}

fn write(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {}", parent.display(), e))?;
    }
    fs::write(path, content).map_err(|e| format!("cannot write {}: {}", path.display(), e))
}

fn panic_message(p: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        format!("panic: {}", s)
    } else if let Some(s) = p.downcast_ref::<String>() {
        format!("panic: {}", s)
    } else {
        "panic: unknown".to_string()
    }
}

fn execute(cfg: &Config, job: &Job, trace_seeds: &[u64]) -> JobResult {
    let c = cfg.for_run(job.variant, job.seed);
    let label = format!("{}_seed{}", job.variant.label(), job.seed);
    let record = trace_seeds.contains(&job.seed);
    let res = catch_unwind(AssertUnwindSafe(|| sim::run(&c, record)));
    match res {
        Ok(Ok(out)) => {
            let trace_json = if record { sim::trace_json(&c, &out).map(|v| v.to_string()) } else { None };
            JobResult {
                label,
                series_csv: out.series.to_csv(),
                events_csv: events_csv(&out.events),
                trace_json,
                summary: out.summary,
            }
        }
        Ok(Err(e)) => JobResult {
            label,
            summary: RunSummary::failed(job.variant.label(), job.seed, &e),
            series_csv: String::new(),
            events_csv: String::new(),
            trace_json: None,
        },
        Err(p) => JobResult {
            label,
            summary: RunSummary::failed(job.variant.label(), job.seed, &panic_message(p)),
            series_csv: String::new(),
            events_csv: String::new(),
            trace_json: None,
        },
    }
}

/// Run the full variants x seeds matrix and write all outputs below `out_dir`.
pub fn run_batch(cfg: &Config, out_dir: &Path) -> Result<BatchResult, String> {
    let errs = cfg.validate();
    if !errs.is_empty() {
        return Err(format!("invalid configuration:\n  - {}", errs.join("\n  - ")));
    }
    let b = &cfg.batch;
    let mut jobs: Vec<Job> = Vec::new();
    for &v in &b.variants {
        for s in 0..b.seeds {
            jobs.push(Job {
                variant: v,
                seed: b.seed_start + s,
            });
        }
    }
    let trace_seeds: Vec<u64> = if b.trace_seeds.is_empty() { vec![b.seed_start] } else { b.trace_seeds.clone() };
    let hw = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let requested = if b.threads == 0 { hw } else { b.threads };
    let n_threads = requested.max(1).min(jobs.len().max(1));

    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<JobResult>>> = Mutex::new((0..jobs.len()).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..n_threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= jobs.len() {
                    break;
                }
                let r = execute(cfg, &jobs[i], &trace_seeds);
                if let Ok(mut guard) = results.lock() {
                    guard[i] = Some(r);
                }
            });
        }
    });
    let collected: Vec<JobResult> = results
        .into_inner()
        .map_err(|_| "result lock poisoned".to_string())?
        .into_iter()
        .enumerate()
        .map(|(i, r)| r.unwrap_or_else(|| JobResult {
            label: format!("job{}", i),
            summary: RunSummary::failed(jobs[i].variant.label(), jobs[i].seed, "job did not complete"),
            series_csv: String::new(),
            events_csv: String::new(),
            trace_json: None,
        }))
        .collect();

    // ---- exports ----
    write(&out_dir.join("config.resolved.json"), &serde_json::to_string_pretty(cfg).unwrap_or_default())?;
    if let Ok(t) = toml::to_string_pretty(cfg) {
        write(&out_dir.join("config.resolved.toml"), &t)?;
    }
    let summaries: Vec<RunSummary> = collected.iter().map(|r| r.summary.clone()).collect();
    write(&out_dir.join("runs.csv"), &RunSummary::to_csv(&summaries))?;
    let runs_json: Vec<serde_json::Value> = summaries
        .iter()
        .map(|s| serde_json::json!({"variant": s.variant, "seed": s.seed, "status": s.status, "error": s.error, "metrics": s.metrics}))
        .collect();
    write(&out_dir.join("runs.json"), &serde_json::to_string_pretty(&runs_json).unwrap_or_default())?;
    for r in &collected {
        if !r.series_csv.is_empty() {
            write(&out_dir.join("series").join(format!("{}.csv", r.label)), &r.series_csv)?;
        }
        if !r.events_csv.is_empty() {
            write(&out_dir.join("events").join(format!("{}.csv", r.label)), &r.events_csv)?;
        }
        if let Some(t) = &r.trace_json {
            write(&out_dir.join("traces").join(format!("{}.trace.json", r.label)), t)?;
        }
    }
    let failed = summaries.iter().filter(|s| s.status != "ok").count();
    let manifest = serde_json::json!({
        "tool": "crqisl",
        "version": env!("CARGO_PKG_VERSION"),
        "experiment": cfg.name,
        "config_hash": cfg.hash_hex(),
        "variants": b.variants.iter().map(|v| v.label()).collect::<Vec<_>>(),
        "seed_start": b.seed_start,
        "seeds": b.seeds,
        "trace_seeds": trace_seeds,
        "runs": summaries.len(),
        "failed_runs": failed,
        "notes": [
            "Quantum-inspired classical simulation; not a quantum implementation, not a proven consensus protocol.",
            "Equations are implementation assumptions (docs/ASSUMPTIONS.md); local stability is not global agreement or finality.",
            "Output contains no timestamps: identical config and seeds reproduce identical files."
        ]
    });
    write(&out_dir.join("manifest.json"), &serde_json::to_string_pretty(&manifest).unwrap_or_default())?;

    let report = analysis::analyze(&summaries, 0.05);
    write(&out_dir.join("analysis").join("descriptive.csv"), &report.descriptive_csv())?;
    write(&out_dir.join("analysis").join("paired_tests.csv"), &report.tests_csv())?;
    write(&out_dir.join("analysis").join("report.md"), &report.to_markdown())?;
    Ok(BatchResult { summaries, report })
}

/// Run a single (variant, seed) configuration and export its artefacts.
pub fn run_single(cfg: &Config, out_dir: &Path, record_trace: bool) -> Result<sim::RunOutput, String> {
    let out = sim::run(cfg, record_trace)?;
    let label = format!("{}_seed{}", cfg.variant.label(), cfg.seed);
    write(&out_dir.join("config.resolved.json"), &serde_json::to_string_pretty(cfg).unwrap_or_default())?;
    write(&out_dir.join("runs.csv"), &RunSummary::to_csv(std::slice::from_ref(&out.summary)))?;
    write(&out_dir.join("series").join(format!("{}.csv", label)), &out.series.to_csv())?;
    write(&out_dir.join("events").join(format!("{}.csv", label)), &events_csv(&out.events))?;
    if let Some(t) = sim::trace_json(cfg, &out) {
        write(&out_dir.join("traces").join(format!("{}.trace.json", label)), &t.to_string())?;
    }
    Ok(out)
}
