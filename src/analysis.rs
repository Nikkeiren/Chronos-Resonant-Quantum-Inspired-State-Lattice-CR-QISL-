//! Statistical analysis of batch results.
//!
//! Design (also written into every generated report):
//! * Runs are paired by seed: for a given seed all variants share topology,
//!   partition groups, adversary set, initial states and channel randomness
//!   (common random numbers), so paired tests are appropriate.
//! * PRIMARY analysis keeps right-censored runs at the horizon value (a lower bound
//!   of the true time). A SENSITIVITY analysis drops pairs in which either run is
//!   censored. Both are reported side by side.
//! * Failed runs (status != "ok") are excluded from every pair they belong to;
//!   each exclusion is listed in the report.
//! * Holm-Bonferroni is applied to the whole family of primary tests (all metrics x
//!   all variant pairs), and separately to the sensitivity family.
//! * p-values describe simulation outcomes under the stated assumptions; they are
//!   not evidence about real distributed systems and say nothing about safety.

use crate::config::Variant;
use crate::metrics::RunSummary;
use crate::stats;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug)]
pub enum Censor {
    None,
    Flag(&'static str),
    FractionAtLeast(&'static str, f64),
}

#[derive(Clone, Copy, Debug)]
pub struct MetricSpec {
    pub name: &'static str,
    pub censor: Censor,
}

pub const ANALYZED: &[MetricSpec] = &[
    MetricSpec {
        name: "agreement_time_after_heal",
        censor: Censor::Flag("agreement_censored"),
    },
    MetricSpec {
        name: "quorum_agreement_time_after_heal",
        censor: Censor::Flag("quorum_censored"),
    },
    MetricSpec {
        name: "amplitude_convergence_time",
        censor: Censor::Flag("convergence_censored"),
    },
    MetricSpec { name: "final_amplitude_spread", censor: Censor::None },
    MetricSpec { name: "post_heal_amplitude_spread_mean", censor: Censor::None },
    MetricSpec { name: "post_heal_amplitude_spread_min", censor: Censor::None },
    MetricSpec {
        name: "stability_time_median",
        censor: Censor::FractionAtLeast("stability_censored_fraction", 0.5),
    },
    MetricSpec {
        name: "divergence_pre_heal",
        censor: Censor::None,
    },
    MetricSpec {
        name: "divergence_post_heal",
        censor: Censor::None,
    },
    MetricSpec {
        name: "oscillation_mean",
        censor: Censor::None,
    },
    MetricSpec {
        name: "stable_revocations",
        censor: Censor::None,
    },
    MetricSpec {
        name: "conflict_entries",
        censor: Censor::None,
    },
    MetricSpec {
        name: "final_stable_fraction",
        censor: Censor::None,
    },
    MetricSpec {
        name: "messages_sent",
        censor: Censor::None,
    },
    MetricSpec {
        name: "bytes_sent",
        censor: Censor::None,
    },
    MetricSpec {
        name: "dropped_total",
        censor: Censor::None,
    },
    MetricSpec {
        name: "heartbeats",
        censor: Censor::None,
    },
    MetricSpec {
        name: "reconciliation_messages",
        censor: Censor::None,
    },
];

fn is_censored(row: &RunSummary, c: Censor) -> bool {
    match c {
        Censor::None => false,
        Censor::Flag(f) => row.get(f) == 1.0,
        Censor::FractionAtLeast(f, th) => row.get(f) >= th,
    }
}

#[derive(Clone, Debug)]
pub struct DescRow {
    pub metric: String,
    pub variant: String,
    pub n: usize,
    pub n_censored: usize,
    pub median: f64,
    pub q25: f64,
    pub q75: f64,
    pub iqr: f64,
    pub min: f64,
    pub max: f64,
}

#[derive(Clone, Debug)]
pub struct TestRow {
    pub metric: String,
    pub variant_a: String,
    pub variant_b: String,
    pub n_pairs: usize,
    pub n_nonzero: usize,
    pub method: String,
    /// median of (a - b)
    pub median_diff: f64,
    pub rank_biserial: f64,
    pub cohens_dz: f64,
    pub p_raw: f64,
    pub p_holm: f64,
    /// Sensitivity analysis: pairs where neither run is censored.
    pub n_pairs_uncensored: usize,
    pub p_raw_uncensored: f64,
    pub p_holm_uncensored: f64,
}

#[derive(Clone, Debug)]
pub struct Report {
    pub variants: Vec<String>,
    pub n_runs: usize,
    pub alpha: f64,
    pub descriptive: Vec<DescRow>,
    pub tests: Vec<TestRow>,
    /// (variant, seed, error) of failed runs.
    pub failed: Vec<(String, u64, String)>,
    /// (variant_a, variant_b, number of seeds dropped from the pairing)
    pub dropped_pairs: Vec<(String, String, usize)>,
}

fn variant_order(rows: &[RunSummary]) -> Vec<String> {
    let present: BTreeSet<String> = rows.iter().map(|r| r.variant.clone()).collect();
    let mut out: Vec<String> = Vec::new();
    for v in Variant::all() {
        if present.contains(v.label()) {
            out.push(v.label().to_string());
        }
    }
    for p in present {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

pub fn analyze(rows: &[RunSummary], alpha: f64) -> Report {
    let variants = variant_order(rows);
    let failed: Vec<(String, u64, String)> = rows.iter().filter(|r| r.status != "ok").map(|r| (r.variant.clone(), r.seed, r.error.clone())).collect();

    // variant -> seed -> ok row
    let mut by: BTreeMap<String, BTreeMap<u64, &RunSummary>> = BTreeMap::new();
    let mut all_seeds: BTreeMap<String, BTreeSet<u64>> = BTreeMap::new();
    for r in rows {
        all_seeds.entry(r.variant.clone()).or_default().insert(r.seed);
        if r.status == "ok" {
            by.entry(r.variant.clone()).or_default().insert(r.seed, r);
        }
    }

    let mut descriptive = Vec::new();
    for spec in ANALYZED {
        for v in &variants {
            let mut vals: Vec<f64> = Vec::new();
            let mut cens = 0usize;
            if let Some(m) = by.get(v) {
                for r in m.values() {
                    let x = r.get(spec.name);
                    if x.is_finite() {
                        vals.push(x);
                        if is_censored(r, spec.censor) {
                            cens += 1;
                        }
                    }
                }
            }
            let min = vals.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            descriptive.push(DescRow {
                metric: spec.name.to_string(),
                variant: v.clone(),
                n: vals.len(),
                n_censored: cens,
                median: stats::median(&vals),
                q25: stats::quantile(&vals, 0.25),
                q75: stats::quantile(&vals, 0.75),
                iqr: stats::iqr(&vals),
                min: if vals.is_empty() { f64::NAN } else { min },
                max: if vals.is_empty() { f64::NAN } else { max },
            });
        }
    }

    let mut tests: Vec<TestRow> = Vec::new();
    let mut dropped_pairs = Vec::new();
    for i in 0..variants.len() {
        for j in (i + 1)..variants.len() {
            let (va, vb) = (&variants[i], &variants[j]);
            let empty: BTreeMap<u64, &RunSummary> = BTreeMap::new();
            let ma = by.get(va).unwrap_or(&empty);
            let mb = by.get(vb).unwrap_or(&empty);
            let common: Vec<u64> = ma.keys().filter(|s| mb.contains_key(*s)).cloned().collect();
            let mut union: BTreeSet<u64> = BTreeSet::new();
            if let Some(s) = all_seeds.get(va) {
                union.extend(s.iter().cloned());
            }
            if let Some(s) = all_seeds.get(vb) {
                union.extend(s.iter().cloned());
            }
            dropped_pairs.push((va.clone(), vb.clone(), union.len().saturating_sub(common.len())));
            for spec in ANALYZED {
                let mut x = Vec::new();
                let mut y = Vec::new();
                let mut xu = Vec::new();
                let mut yu = Vec::new();
                for s in &common {
                    let (ra, rb) = (ma[s], mb[s]);
                    let (a, b) = (ra.get(spec.name), rb.get(spec.name));
                    if !(a.is_finite() && b.is_finite()) {
                        continue;
                    }
                    x.push(a);
                    y.push(b);
                    if !is_censored(ra, spec.censor) && !is_censored(rb, spec.censor) {
                        xu.push(a);
                        yu.push(b);
                    }
                }
                let primary = stats::wilcoxon_signed_rank(&x, &y);
                let sens = stats::wilcoxon_signed_rank(&xu, &yu);
                let sens_p = if sens.n_pairs == 0 { f64::NAN } else { sens.p_value };
                tests.push(TestRow {
                    metric: spec.name.to_string(),
                    variant_a: va.clone(),
                    variant_b: vb.clone(),
                    n_pairs: primary.n_pairs,
                    n_nonzero: primary.n_nonzero,
                    method: primary.method.to_string(),
                    median_diff: primary.median_diff,
                    rank_biserial: primary.rank_biserial,
                    cohens_dz: primary.cohens_dz,
                    p_raw: if primary.n_pairs == 0 { f64::NAN } else { primary.p_value },
                    p_holm: f64::NAN,
                    n_pairs_uncensored: sens.n_pairs,
                    p_raw_uncensored: sens_p,
                    p_holm_uncensored: f64::NAN,
                });
            }
        }
    }
    let p1: Vec<f64> = tests.iter().map(|t| t.p_raw).collect();
    let p2: Vec<f64> = tests.iter().map(|t| t.p_raw_uncensored).collect();
    let h1 = stats::holm_bonferroni(&p1);
    let h2 = stats::holm_bonferroni(&p2);
    for (t, (a, b)) in tests.iter_mut().zip(h1.iter().zip(h2.iter())) {
        t.p_holm = *a;
        t.p_holm_uncensored = *b;
    }
    Report {
        variants,
        n_runs: rows.len(),
        alpha,
        descriptive,
        tests,
        failed,
        dropped_pairs,
    }
}

fn f(v: f64) -> String {
    if !v.is_finite() {
        "NaN".to_string()
    } else {
        format!("{}", v)
    }
}

fn pretty(v: f64) -> String {
    if !v.is_finite() {
        "n/a".to_string()
    } else if v != 0.0 && v.abs() < 1e-3 {
        format!("{:.2e}", v)
    } else {
        format!("{:.3}", v)
    }
}

impl Report {
    pub fn descriptive_csv(&self) -> String {
        let mut s = String::from("metric,variant,n,n_censored,median,q25,q75,iqr,min,max\n");
        for d in &self.descriptive {
            s.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{}\n",
                d.metric,
                d.variant,
                d.n,
                d.n_censored,
                f(d.median),
                f(d.q25),
                f(d.q75),
                f(d.iqr),
                f(d.min),
                f(d.max)
            ));
        }
        s
    }

    pub fn tests_csv(&self) -> String {
        let mut s = String::from("metric,variant_a,variant_b,n_pairs,n_nonzero,method,median_diff_a_minus_b,rank_biserial,cohens_dz,p_raw,p_holm,n_pairs_uncensored,p_raw_uncensored,p_holm_uncensored\n");
        for t in &self.tests {
            s.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                t.metric,
                t.variant_a,
                t.variant_b,
                t.n_pairs,
                t.n_nonzero,
                t.method,
                f(t.median_diff),
                f(t.rank_biserial),
                f(t.cohens_dz),
                f(t.p_raw),
                f(t.p_holm),
                t.n_pairs_uncensored,
                f(t.p_raw_uncensored),
                f(t.p_holm_uncensored)
            ));
        }
        s
    }

    pub fn to_markdown(&self) -> String {
        let mut s = String::new();
        s.push_str("# CR-QISL experiment analysis\n\n");
        s.push_str("> Simulation results of an experimental quantum-inspired *classical* model. They are not evidence of safety, Byzantine tolerance or consensus,\n");
        s.push_str("> and \"agreement\" below is an observer-level measurement, not finality. Equations are implementation assumptions (docs/ASSUMPTIONS.md).\n\n");
        s.push_str(&format!("Runs: {} | variants: {} | family-wise alpha (Holm): {}\n\n", self.n_runs, self.variants.join(", "), self.alpha));
        s.push_str("## Method\n\n");
        s.push_str("- Paired by seed; variants share topology, partition groups, adversary set, initial state and channel randomness.\n");
        s.push_str("- Wilcoxon signed-rank (two-sided; zero differences dropped; exact distribution for n <= 30 without ties, otherwise normal approximation with tie and continuity correction).\n");
        s.push_str("- Effect sizes: matched-pairs rank-biserial correlation (positive = first variant larger) and Cohen's d_z; also the median paired difference.\n");
        s.push_str("- Holm-Bonferroni over all metric x variant-pair tests (primary and sensitivity families separately).\n");
        s.push_str("- Censoring: times that never occurred are right-censored at the horizon and kept in the primary analysis (lower bound); the sensitivity columns drop pairs with a censored run.\n\n");
        s.push_str("## Descriptive statistics (median [Q1, Q3], censored runs)\n\n");
        s.push_str("| metric | variant | n | censored | median | Q1 | Q3 | IQR |\n|---|---|---|---|---|---|---|---|\n");
        for d in &self.descriptive {
            s.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
                d.metric,
                d.variant,
                d.n,
                d.n_censored,
                pretty(d.median),
                pretty(d.q25),
                pretty(d.q75),
                pretty(d.iqr)
            ));
        }
        s.push_str("\n## Paired comparisons (a vs b, difference = a - b)\n\n");
        s.push_str("| metric | a | b | pairs | median diff | r_rb | d_z | p | p_Holm | pairs (uncens.) | p_Holm (uncens.) | sig. |\n|---|---|---|---|---|---|---|---|---|---|---|---|\n");
        for t in &self.tests {
            let sig = if t.p_holm.is_finite() && t.p_holm < self.alpha { "yes" } else { "no" };
            s.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                t.metric,
                t.variant_a,
                t.variant_b,
                t.n_pairs,
                pretty(t.median_diff),
                pretty(t.rank_biserial),
                pretty(t.cohens_dz),
                pretty(t.p_raw),
                pretty(t.p_holm),
                t.n_pairs_uncensored,
                pretty(t.p_holm_uncensored),
                sig
            ));
        }
        s.push_str("\n## Exclusions and failed runs\n\n");
        if self.failed.is_empty() {
            s.push_str("No failed runs. Exclusion criterion: a run with status != ok is dropped from every pair it belongs to; non-finite metric values drop only that metric of that pair.\n");
        } else {
            s.push_str("Criterion: a run with status != ok is dropped from every pair it belongs to.\n\n");
            for (v, seed, e) in &self.failed {
                s.push_str(&format!("- {} seed {}: {}\n", v, seed, e));
            }
        }
        s.push_str("\nSeeds dropped from pairing per variant pair:\n\n");
        for (a, b, n) in &self.dropped_pairs {
            s.push_str(&format!("- {} vs {}: {}\n", a, b, n));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(variant: &str, seed: u64, agree_t: f64, cens: f64, msgs: f64) -> RunSummary {
        let mut m = BTreeMap::new();
        m.insert("agreement_time_after_heal".to_string(), agree_t);
        m.insert("agreement_censored".to_string(), cens);
        m.insert("messages_sent".to_string(), msgs);
        RunSummary::ok(variant, seed, m)
    }

    fn rows() -> Vec<RunSummary> {
        let mut v = Vec::new();
        for s in 1..=14u64 {
            // weighted_gossip is consistently slower and cheaper than full_cr_qisl.
            v.push(row("weighted_gossip", s, 100.0 + s as f64, if s <= 2 { 1.0 } else { 0.0 }, 1000.0 + s as f64));
            v.push(row("full_cr_qisl", s, 50.0 + (s % 3) as f64, 0.0, 2000.0 + 3.0 * s as f64));
        }
        v
    }

    #[test]
    fn variants_are_ordered_canonically() {
        let r = analyze(&rows(), 0.05);
        assert_eq!(r.variants, vec!["weighted_gossip".to_string(), "full_cr_qisl".to_string()]);
    }

    #[test]
    fn descriptive_statistics_and_censoring_counts() {
        let r = analyze(&rows(), 0.05);
        let d = r.descriptive.iter().find(|d| d.metric == "agreement_time_after_heal" && d.variant == "weighted_gossip").unwrap();
        assert_eq!(d.n, 14);
        assert_eq!(d.n_censored, 2);
        assert!((d.median - 107.5).abs() < 1e-9);
    }

    #[test]
    fn paired_test_detects_systematic_difference_and_holm_is_conservative() {
        let r = analyze(&rows(), 0.05);
        let t = r.tests.iter().find(|t| t.metric == "messages_sent").unwrap();
        assert_eq!(t.n_pairs, 14);
        assert!(t.p_raw < 0.001, "p = {}", t.p_raw);
        assert!(t.p_holm >= t.p_raw);
        assert!(t.median_diff < 0.0);
        assert!(t.rank_biserial < -0.99);
        let a = r.tests.iter().find(|t| t.metric == "agreement_time_after_heal").unwrap();
        assert!(a.median_diff > 0.0);
        // Sensitivity analysis removes the two censored seeds.
        assert_eq!(a.n_pairs_uncensored, 12);
    }

    #[test]
    fn failed_runs_are_excluded_and_documented() {
        let mut v = rows();
        v.push(RunSummary::failed("full_cr_qisl", 99, "panic: test"));
        v.push(row("weighted_gossip", 99, 120.0, 0.0, 1000.0));
        let r = analyze(&v, 0.05);
        assert_eq!(r.failed.len(), 1);
        let t = r.tests.iter().find(|t| t.metric == "messages_sent").unwrap();
        assert_eq!(t.n_pairs, 14, "seed 99 must not be paired");
        assert_eq!(r.dropped_pairs[0].2, 1);
        let md = r.to_markdown();
        assert!(md.contains("seed 99: panic: test"));
        assert!(md.contains("Holm"));
    }

    #[test]
    fn csv_outputs_have_consistent_columns() {
        let r = analyze(&rows(), 0.05);
        let csv = r.tests_csv();
        let cols = csv.lines().next().unwrap().split(',').count();
        assert!(csv.lines().all(|l| l.split(',').count() == cols));
        let d = r.descriptive_csv();
        let dcols = d.lines().next().unwrap().split(',').count();
        assert!(d.lines().all(|l| l.split(',').count() == dcols));
    }
}
