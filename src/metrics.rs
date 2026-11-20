//! Run summaries, CSV (de)serialisation and time-series recording.
//!
//! METRIC SEMANTICS (see docs/METRICS.md):
//! * "stability" metrics are LOCAL: a node entered the Stable state.
//! * "agreement" / "convergence" metrics are OBSERVER-LEVEL: the simulator looks at
//!   all honest nodes at once. No node can compute them, and they are NOT finality.
//! * Times that never occurred are right-censored at the horizon and flagged.

use std::collections::BTreeMap;

pub const METRIC_COLUMNS: &[&str] = &[
    "honest_nodes",
    "adversarial_nodes",
    "horizon",
    "reference_tick",
    "network_group0_size",
    "network_group1_size",
    "evidence_group0_size",
    "evidence_group1_size",
    "evidence_overlap_rate",
    "group_n00",
    "group_n01",
    "group_n10",
    "group_n11",
    "graph_edge_count",
    "cross_network_edge_count",
    "network_cut_ratio",
    "agreement_time_after_heal",
    "agreement_censored",
    "quorum_agreement_time_after_heal",
    "quorum_censored",
    "amplitude_convergence_time",
    "convergence_censored",
    "final_amplitude_spread",
    "post_heal_amplitude_spread_mean",
    "post_heal_amplitude_spread_min",
    "stability_time_median",
    "stability_time_p90",
    "stability_censored_fraction",
    "nodes_never_stable",
    "divergence_pre_heal",
    "divergence_post_heal",
    "split_ticks_pre_heal",
    "split_ticks_post_heal",
    "oscillation_mean",
    "stable_revocations",
    "conflict_entries",
    "conflict_pending_final",
    "reconcile_timeouts",
    "normalization_resets",
    "final_stable_fraction",
    "final_agreement",
    "final_split",
    "adv_target_adoption_final",
    "nan_corrupted_nodes",
    "messages_sent",
    "messages_delivered",
    "dropped_loss",
    "dropped_partition",
    "dropped_invalid",
    "dropped_total",
    "undelivered_end",
    "stale_ignored",
    "bytes_sent",
    "bytes_delivered",
    "state_updates",
    "heartbeats",
    "witness_messages",
    "reconciliation_messages",
    "adversary_messages",
    "retransmissions",
    "signature_ops",
];

#[derive(Clone, Debug, PartialEq)]
pub struct RunSummary {
    pub variant: String,
    pub seed: u64,
    /// "ok" or "failed"
    pub status: String,
    pub error: String,
    pub metrics: BTreeMap<String, f64>,
}

fn clean(s: &str) -> String {
    s.replace(|c: char| c == ',' || c == '\n' || c == '\r', ";")
}

impl RunSummary {
    pub fn ok(variant: &str, seed: u64, metrics: BTreeMap<String, f64>) -> RunSummary {
        RunSummary {
            variant: variant.to_string(),
            seed,
            status: "ok".into(),
            error: String::new(),
            metrics,
        }
    }

    pub fn failed(variant: &str, seed: u64, error: &str) -> RunSummary {
        RunSummary {
            variant: variant.to_string(),
            seed,
            status: "failed".into(),
            error: clean(error),
            metrics: BTreeMap::new(),
        }
    }

    pub fn get(&self, name: &str) -> f64 {
        self.metrics.get(name).copied().unwrap_or(f64::NAN)
    }

    pub fn csv_header() -> String {
        let mut h = String::from("variant,seed,status,error");
        for c in METRIC_COLUMNS {
            h.push(',');
            h.push_str(c);
        }
        h
    }

    pub fn csv_row(&self) -> String {
        let mut r = format!("{},{},{},{}", clean(&self.variant), self.seed, self.status, clean(&self.error));
        for c in METRIC_COLUMNS {
            r.push(',');
            if let Some(v) = self.metrics.get(*c) {
                r.push_str(&format!("{}", v));
            }
        }
        r
    }

    pub fn to_csv(rows: &[RunSummary]) -> String {
        let mut s = RunSummary::csv_header();
        s.push('\n');
        for r in rows {
            s.push_str(&r.csv_row());
            s.push('\n');
        }
        s
    }

    pub fn from_csv(text: &str) -> Result<Vec<RunSummary>, String> {
        let mut lines = text.lines().filter(|l| !l.trim().is_empty());
        let header = lines.next().ok_or("empty CSV")?;
        let cols: Vec<&str> = header.split(',').collect();
        if cols.len() < 4 || cols[0] != "variant" || cols[1] != "seed" || cols[2] != "status" || cols[3] != "error" {
            return Err("unexpected CSV header (expected variant,seed,status,error,...)".into());
        }
        let mut out = Vec::new();
        for (ln, line) in lines.enumerate() {
            let f: Vec<&str> = line.split(',').collect();
            if f.len() != cols.len() {
                return Err(format!("row {} has {} fields, expected {}", ln + 2, f.len(), cols.len()));
            }
            let seed = f[1].parse::<u64>().map_err(|e| format!("row {}: bad seed: {}", ln + 2, e))?;
            let mut metrics = BTreeMap::new();
            for i in 4..cols.len() {
                if f[i].is_empty() {
                    continue;
                }
                let v = f[i].parse::<f64>().map_err(|e| format!("row {} col {}: {}", ln + 2, cols[i], e))?;
                metrics.insert(cols[i].to_string(), v);
            }
            out.push(RunSummary {
                variant: f[0].to_string(),
                seed,
                status: f[2].to_string(),
                error: f[3].to_string(),
                metrics,
            });
        }
        Ok(out)
    }
}

/// Start index of the first run of at least `hold` consecutive `true` flags,
/// scanning from index `from`.
pub fn first_run_start(flags: &[bool], from: usize, hold: usize) -> Option<usize> {
    let hold = hold.max(1);
    let mut run = 0usize;
    for (t, &f) in flags.iter().enumerate().skip(from) {
        if f {
            run += 1;
            if run >= hold {
                return Some(t + 1 - hold);
            }
        } else {
            run = 0;
        }
    }
    None
}

/// Column-oriented time series (one value per tick per column).
#[derive(Clone, Debug, Default)]
pub struct Series {
    pub names: Vec<String>,
    pub cols: Vec<Vec<f64>>,
}

impl Series {
    pub fn new(names: Vec<String>) -> Series {
        let cols = vec![Vec::new(); names.len()];
        Series { names, cols }
    }

    pub fn push_row(&mut self, row: &[f64]) {
        debug_assert_eq!(row.len(), self.cols.len());
        for (c, v) in self.cols.iter_mut().zip(row.iter()) {
            c.push(*v);
        }
    }

    pub fn column(&self, name: &str) -> Option<&Vec<f64>> {
        self.names.iter().position(|n| n == name).map(|i| &self.cols[i])
    }

    pub fn to_csv(&self) -> String {
        let mut s = self.names.join(",");
        s.push('\n');
        let rows = self.cols.first().map(|c| c.len()).unwrap_or(0);
        for r in 0..rows {
            let line: Vec<String> = self.cols.iter().map(|c| format!("{}", c[r])).collect();
            s.push_str(&line.join(","));
            s.push('\n');
        }
        s
    }

    pub fn to_json(&self) -> serde_json::Value {
        let mut m = serde_json::Map::new();
        for (n, c) in self.names.iter().zip(self.cols.iter()) {
            let arr: Vec<serde_json::Value> = c.iter().map(|v| serde_json::json!(*v)).collect();
            m.insert(n.clone(), serde_json::Value::Array(arr));
        }
        serde_json::Value::Object(m)
    }
}

#[derive(Clone, Debug)]
pub struct EventRecord {
    pub tick: u64,
    pub kind: String,
    pub node: i64,
    pub a: i64,
    pub b: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_detection() {
        let f = [false, true, true, false, true, true, true, true];
        assert_eq!(first_run_start(&f, 0, 3), Some(4));
        assert_eq!(first_run_start(&f, 0, 2), Some(1));
        assert_eq!(first_run_start(&f, 5, 2), Some(5));
        assert_eq!(first_run_start(&f, 0, 5), None);
        assert_eq!(first_run_start(&[], 0, 1), None);
    }

    #[test]
    fn csv_round_trip_including_failed_rows() {
        let mut m = BTreeMap::new();
        for (i, c) in METRIC_COLUMNS.iter().enumerate() {
            m.insert(c.to_string(), i as f64 * 0.5);
        }
        m.insert("divergence_post_heal".into(), f64::NAN);
        let a = RunSummary::ok("full_cr_qisl", 7, m);
        let b = RunSummary::failed("weighted_gossip", 8, "panic: bad, thing\nline2");
        let text = RunSummary::to_csv(&[a.clone(), b.clone()]);
        let back = RunSummary::from_csv(&text).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].variant, "full_cr_qisl");
        assert_eq!(back[0].seed, 7);
        assert!(back[0].get("divergence_post_heal").is_nan());
        assert!((back[0].get("horizon") - 1.0).abs() < 1e-12);
        assert_eq!(back[1].status, "failed");
        assert!(back[1].metrics.is_empty());
        assert!(!back[1].error.contains(','));
    }

    #[test]
    fn csv_rejects_malformed_input() {
        assert!(RunSummary::from_csv("").is_err());
        assert!(RunSummary::from_csv("a,b,c\n1,2,3\n").is_err());
        let h = RunSummary::csv_header();
        assert!(RunSummary::from_csv(&format!("{}\nx,1,ok\n", h)).is_err());
    }

    #[test]
    fn series_columns() {
        let mut s = Series::new(vec!["a".into(), "b".into()]);
        s.push_row(&[1.0, 2.0]);
        s.push_row(&[3.0, 4.0]);
        assert_eq!(s.column("b").unwrap(), &vec![2.0, 4.0]);
        assert_eq!(s.to_csv(), "a,b\n1,2\n3,4\n");
    }
}
