//! crqisl command line interface.

use crqisl::analysis;
use crqisl::config::{Config, Variant};
use crqisl::experiment;
use crqisl::metrics::RunSummary;
use std::path::PathBuf;
use std::process::ExitCode;

const HELP: &str = "crqisl - CR-QISL experimental simulation platform (quantum-inspired CLASSICAL simulation;
not a quantum implementation and not a proven consensus protocol)

USAGE:
  crqisl run      [--config FILE] [--variant V] [--seed N] [--out DIR] [--trace]
  crqisl batch    [--config FILE] [--out DIR] [--threads N] [--seeds N] [--seed-start N] [--variants A,B,..]
  crqisl analyze  --runs runs.csv [--out DIR] [--alpha 0.05]
  crqisl validate [--config FILE]
  crqisl default-config

Variants: weighted_gossip (A), weighted_gossip_damping (B), phase_ablation (C), full_cr_qisl (D).
Without --config the built-in initial experiment is used (n=100, 2 states, 30 seeds, 20% loss,
asynchronous latency, partition at tick 60, healing at tick 140, horizon 300).
";

struct Args {
    items: Vec<String>,
}

impl Args {
    fn flag(&self, name: &str) -> bool {
        self.items.iter().any(|a| a == name)
    }

    fn value(&self, name: &str) -> Option<String> {
        let prefix = format!("{}=", name);
        for (i, a) in self.items.iter().enumerate() {
            if a == name {
                return self.items.get(i + 1).cloned();
            }
            if let Some(v) = a.strip_prefix(&prefix) {
                return Some(v.to_string());
            }
        }
        None
    }
}

fn load_config(args: &Args) -> Result<Config, String> {
    match args.value("--config") {
        Some(p) => Config::load(&p),
        None => Ok(Config::default()),
    }
}

fn parse_u64(args: &Args, name: &str) -> Result<Option<u64>, String> {
    match args.value(name) {
        None => Ok(None),
        Some(v) => v.parse::<u64>().map(Some).map_err(|e| format!("{} expects an integer: {}", name, e)),
    }
}

fn print_headline(report: &analysis::Report) {
    println!("\nHeadline (observer-level; local stability is NOT global agreement or finality):");
    for metric in ["agreement_time_after_heal", "stability_time_median", "divergence_post_heal", "messages_sent"] {
        for d in report.descriptive.iter().filter(|d| d.metric == metric) {
            println!(
                "  {:<28} {:<24} n={:<3} censored={:<3} median={:.3} IQR={:.3}",
                d.metric, d.variant, d.n, d.n_censored, d.median, d.iqr
            );
        }
    }
}

fn real_main() -> Result<(), String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let cmd = argv.first().cloned().unwrap_or_else(|| "help".to_string());
    let args = Args { items: argv.iter().skip(1).cloned().collect() };
    match cmd.as_str() {
        "help" | "--help" | "-h" => {
            print!("{}", HELP);
            Ok(())
        }
        "default-config" => {
            let c = Config::default();
            match toml::to_string_pretty(&c) {
                Ok(s) => print!("{}", s),
                Err(e) => {
                    eprintln!("note: TOML serialisation failed ({}); printing JSON", e);
                    println!("{}", serde_json::to_string_pretty(&c).unwrap_or_default());
                }
            }
            Ok(())
        }
        "validate" => {
            let c = load_config(&args)?;
            println!("configuration '{}' is valid (hash {})", c.name, c.hash_hex());
            Ok(())
        }
        "run" => {
            let mut c = load_config(&args)?;
            if let Some(v) = args.value("--variant") {
                c.variant = Variant::parse(&v).ok_or(format!("unknown variant '{}'", v))?;
            }
            if let Some(s) = parse_u64(&args, "--seed")? {
                c.seed = s;
            }
            let out_dir = PathBuf::from(args.value("--out").unwrap_or_else(|| format!("out/{}_run", c.name)));
            let trace = args.flag("--trace") || c.telemetry.per_node_history;
            let out = experiment::run_single(&c, &out_dir, trace)?;
            println!("variant={} seed={} -> {}", c.variant.label(), c.seed, out_dir.display());
            for key in [
                "agreement_time_after_heal",
                "agreement_censored",
                "stability_time_median",
                "divergence_pre_heal",
                "divergence_post_heal",
                "conflict_entries",
                "final_stable_fraction",
                "messages_sent",
                "bytes_sent",
                "dropped_total",
            ] {
                println!("  {:<28} {}", key, out.summary.get(key));
            }
            if trace {
                println!("  trace: {}/traces/{}_seed{}.trace.json (open viewer/index.html)", out_dir.display(), c.variant.label(), c.seed);
            }
            Ok(())
        }
        "batch" => {
            let mut c = load_config(&args)?;
            if let Some(n) = parse_u64(&args, "--seeds")? {
                c.batch.seeds = n;
            }
            if let Some(n) = parse_u64(&args, "--seed-start")? {
                c.batch.seed_start = n;
            }
            if let Some(n) = parse_u64(&args, "--threads")? {
                c.batch.threads = n as usize;
            }
            if let Some(v) = args.value("--variants") {
                let mut list = Vec::new();
                for p in v.split(',') {
                    list.push(Variant::parse(p).ok_or(format!("unknown variant '{}'", p))?);
                }
                c.batch.variants = list;
            }
            let out_dir = PathBuf::from(args.value("--out").unwrap_or_else(|| format!("out/{}", c.name)));
            eprintln!(
                "running {} variants x {} seeds (n={}, horizon={}) ...",
                c.batch.variants.len(),
                c.batch.seeds,
                c.nodes,
                c.max_ticks
            );
            let res = experiment::run_batch(&c, &out_dir)?;
            let failed = res.summaries.iter().filter(|s| s.status != "ok").count();
            println!("{} runs written to {} ({} failed)", res.summaries.len(), out_dir.display(), failed);
            println!("report: {}", out_dir.join("analysis").join("report.md").display());
            print_headline(&res.report);
            Ok(())
        }
        "analyze" => {
            let runs = args.value("--runs").ok_or("analyze requires --runs <runs.csv>")?;
            let text = std::fs::read_to_string(&runs).map_err(|e| format!("cannot read {}: {}", runs, e))?;
            let rows = RunSummary::from_csv(&text)?;
            let alpha = match args.value("--alpha") {
                Some(a) => a.parse::<f64>().map_err(|e| format!("--alpha: {}", e))?,
                None => 0.05,
            };
            let report = analysis::analyze(&rows, alpha);
            let out_dir = PathBuf::from(args.value("--out").unwrap_or_else(|| "analysis".to_string()));
            std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
            std::fs::write(out_dir.join("descriptive.csv"), report.descriptive_csv()).map_err(|e| e.to_string())?;
            std::fs::write(out_dir.join("paired_tests.csv"), report.tests_csv()).map_err(|e| e.to_string())?;
            std::fs::write(out_dir.join("report.md"), report.to_markdown()).map_err(|e| e.to_string())?;
            println!("analysis of {} runs written to {}", rows.len(), out_dir.display());
            print_headline(&report);
            Ok(())
        }
        other => Err(format!("unknown command '{}'\n\n{}", other, HELP)),
    }
}

fn main() -> ExitCode {
    match real_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::from(2)
        }
    }
}
