//! Dependency-free statistics used by the analysis pipeline.
//!
//! * median / quartiles / IQR (linear interpolation, "type 7")
//! * Wilcoxon signed-rank test (zero differences dropped; exact distribution when
//!   n <= 30 and no ties, otherwise normal approximation with tie and continuity
//!   correction) — validated offline against SciPy 1.x
//! * matched-pairs rank-biserial correlation and Cohen's d_z
//! * Holm-Bonferroni step-down adjustment

use std::f64::consts::PI;

fn finite_sorted(xs: &[f64]) -> Vec<f64> {
    let mut v: Vec<f64> = xs.iter().cloned().filter(|x| x.is_finite()).collect();
    v.sort_by(|a, b| a.total_cmp(b));
    v
}

/// Quantile with linear interpolation between order statistics (non-finite values ignored).
pub fn quantile(xs: &[f64], q: f64) -> f64 {
    let v = finite_sorted(xs);
    if v.is_empty() {
        return f64::NAN;
    }
    let q = q.clamp(0.0, 1.0);
    let h = (v.len() - 1) as f64 * q;
    let lo = h.floor() as usize;
    let hi = h.ceil() as usize;
    v[lo] + (h - lo as f64) * (v[hi] - v[lo])
}

pub fn median(xs: &[f64]) -> f64 {
    quantile(xs, 0.5)
}

pub fn iqr(xs: &[f64]) -> f64 {
    quantile(xs, 0.75) - quantile(xs, 0.25)
}

pub fn mean(xs: &[f64]) -> f64 {
    let v: Vec<f64> = xs.iter().cloned().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        f64::NAN
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

/// Sample standard deviation (n - 1 denominator).
pub fn std_dev(xs: &[f64]) -> f64 {
    let v: Vec<f64> = xs.iter().cloned().filter(|x| x.is_finite()).collect();
    if v.len() < 2 {
        return f64::NAN;
    }
    let m = v.iter().sum::<f64>() / v.len() as f64;
    (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() as f64 - 1.0)).sqrt()
}

/// Complementary error function (Taylor series for x < 2.5, continued fraction above).
/// Relative error below ~1e-10 on [0, 8] (checked against SciPy).
pub fn erfc(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x < 0.0 {
        return 2.0 - erfc(-x);
    }
    if x < 2.5 {
        let x2 = x * x;
        let mut term = x;
        let mut sum = x;
        let mut n = 0.0;
        loop {
            n += 1.0;
            term *= -x2 / n;
            let add = term / (2.0 * n + 1.0);
            sum += add;
            if add.abs() < 1e-17 * sum.abs().max(1e-300) || n > 200.0 {
                break;
            }
        }
        1.0 - 2.0 / PI.sqrt() * sum
    } else {
        let mut t = x;
        for n in (1..=80).rev() {
            t = x + (n as f64 / 2.0) / t;
        }
        (-x * x).exp() / (PI.sqrt() * t)
    }
}

#[derive(Clone, Debug)]
pub struct PairedTest {
    /// Pairs with both values finite.
    pub n_pairs: usize,
    /// Pairs with non-zero difference (the Wilcoxon sample size).
    pub n_nonzero: usize,
    pub w_plus: f64,
    pub w_minus: f64,
    pub p_value: f64,
    /// "exact" | "normal" | "degenerate"
    pub method: &'static str,
    /// Signed z statistic (NaN for the exact method).
    pub z: f64,
    /// Matched-pairs rank-biserial correlation in [-1, 1]; positive means first > second.
    pub rank_biserial: f64,
    /// Cohen's d_z of the paired differences (first - second).
    pub cohens_dz: f64,
    pub median_diff: f64,
}

/// Wilcoxon signed-rank test on paired samples; difference = x - y; two-sided.
pub fn wilcoxon_signed_rank(x: &[f64], y: &[f64]) -> PairedTest {
    let mut d: Vec<f64> = Vec::new();
    for (a, b) in x.iter().zip(y.iter()) {
        if a.is_finite() && b.is_finite() {
            d.push(a - b);
        }
    }
    let n_pairs = d.len();
    let median_diff = median(&d);
    let sd = std_dev(&d);
    let cohens_dz = if sd.is_finite() && sd > 0.0 { mean(&d) / sd } else { f64::NAN };
    let nz: Vec<f64> = d.iter().cloned().filter(|v| *v != 0.0).collect();
    let n = nz.len();
    if n == 0 {
        return PairedTest {
            n_pairs,
            n_nonzero: 0,
            w_plus: 0.0,
            w_minus: 0.0,
            p_value: 1.0,
            method: "degenerate",
            z: 0.0,
            rank_biserial: 0.0,
            cohens_dz,
            median_diff,
        };
    }
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&i, &j| nz[i].abs().total_cmp(&nz[j].abs()));
    let mut ranks = vec![0.0; n];
    let mut tie_term = 0.0;
    let mut has_ties = false;
    let mut i = 0;
    while i < n {
        let mut j = i;
        while j + 1 < n && nz[idx[j + 1]].abs() == nz[idx[i]].abs() {
            j += 1;
        }
        let avg = (i + j) as f64 / 2.0 + 1.0;
        for t in i..=j {
            ranks[idx[t]] = avg;
        }
        let t = (j - i + 1) as f64;
        if t > 1.0 {
            has_ties = true;
            tie_term += t * t * t - t;
        }
        i = j + 1;
    }
    let mut w_plus = 0.0;
    let mut w_minus = 0.0;
    for k in 0..n {
        if nz[k] > 0.0 {
            w_plus += ranks[k];
        } else {
            w_minus += ranks[k];
        }
    }
    let rank_biserial = (w_plus - w_minus) / (w_plus + w_minus);
    let (p_value, method, z) = if n <= 30 && !has_ties {
        // Exact null distribution of the rank sum via subset-sum counting.
        let max_sum = n * (n + 1) / 2;
        let mut counts = vec![0u64; max_sum + 1];
        counts[0] = 1;
        for r in 1..=n {
            for s in (r..=max_sum).rev() {
                counts[s] += counts[s - r];
            }
        }
        let w = w_plus.min(w_minus).round() as usize;
        let cum: u64 = counts[..=w.min(max_sum)].iter().sum();
        let p = (2.0 * cum as f64 / 2f64.powi(n as i32)).min(1.0);
        (p, "exact", f64::NAN)
    } else {
        let nf = n as f64;
        let mean_w = nf * (nf + 1.0) / 4.0;
        let var = nf * (nf + 1.0) * (2.0 * nf + 1.0) / 24.0 - tie_term / 48.0;
        let diff = w_plus - mean_w;
        if var <= 0.0 {
            (1.0, "normal", 0.0)
        } else {
            let zz = ((diff.abs() - 0.5).max(0.0)) / var.sqrt();
            let p = erfc(zz / 2f64.sqrt()).min(1.0);
            (p, "normal", if diff < 0.0 { -zz } else { zz })
        }
    };
    PairedTest {
        n_pairs,
        n_nonzero: n,
        w_plus,
        w_minus,
        p_value,
        method,
        z,
        rank_biserial,
        cohens_dz,
        median_diff,
    }
}

/// Holm-Bonferroni adjusted p-values (same order as input). Non-finite inputs are
/// excluded from the family and returned as NaN.
pub fn holm_bonferroni(p: &[f64]) -> Vec<f64> {
    let mut idx: Vec<usize> = (0..p.len()).filter(|&i| p[i].is_finite()).collect();
    let m = idx.len();
    idx.sort_by(|&a, &b| p[a].total_cmp(&p[b]));
    let mut adj = vec![f64::NAN; p.len()];
    let mut running = 0.0f64;
    for (rank, &i) in idx.iter().enumerate() {
        let v = ((m - rank) as f64 * p[i]).min(1.0);
        running = running.max(v);
        adj[i] = running;
    }
    adj
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn quantiles_and_iqr() {
        let v = [1.0, 2.0, 3.0, 4.0];
        assert!(close(median(&v), 2.5, 1e-12));
        assert!(close(quantile(&v, 0.25), 1.75, 1e-12));
        assert!(close(quantile(&v, 0.75), 3.25, 1e-12));
        assert!(close(iqr(&v), 1.5, 1e-12));
        assert!(median(&[]).is_nan());
        assert!(close(median(&[3.0, f64::NAN, 1.0, 2.0]), 2.0, 1e-12));
        assert!(close(median(&[5.0]), 5.0, 1e-12));
    }

    #[test]
    fn erfc_matches_reference_values() {
        // Reference values from SciPy special.erfc
        let table = [
            (0.0, 1.0),
            (0.5, 0.4795001221869535),
            (1.0, 0.15729920705028516),
            (1.96, 0.005573724535172135),
            (2.4, 0.0006885138966450789),
            (2.6, 0.000236034416529349),
            (3.0, 2.2090496998585445e-05),
            (4.0, 1.541725790028002e-08),
            (6.0, 2.1519736712498913e-17),
        ];
        for (x, want) in table {
            let got = erfc(x);
            assert!(((got - want) / want).abs() < 1e-9, "erfc({}) = {} want {}", x, got, want);
        }
        assert!(close(erfc(-1.0), 2.0 - 0.15729920705028516, 1e-12));
    }

    #[test]
    fn wilcoxon_classic_exact_example() {
        // Hollander & Wolfe depression data (R documentation): V = 40, exact two-sided p = 0.0390625.
        let x = [1.83, 0.50, 1.62, 2.48, 1.68, 1.88, 1.55, 3.06, 1.30];
        let y = [0.878, 0.647, 0.598, 2.05, 1.06, 1.29, 1.06, 3.14, 1.29];
        let r = wilcoxon_signed_rank(&x, &y);
        assert_eq!(r.method, "exact");
        assert_eq!(r.n_nonzero, 9);
        assert!(close(r.w_plus, 40.0, 1e-12) && close(r.w_minus, 5.0, 1e-12));
        assert!(close(r.p_value, 0.0390625, 1e-12));
        assert!(close(r.rank_biserial, 35.0 / 45.0, 1e-12));
        // Swapping the samples keeps p and flips the sign of the effect size.
        let s = wilcoxon_signed_rank(&y, &x);
        assert!(close(s.p_value, r.p_value, 1e-12));
        assert!(close(s.rank_biserial, -r.rank_biserial, 1e-12));
    }

    #[test]
    fn wilcoxon_normal_approximation_with_ties_and_zeros() {
        // Reference computed with SciPy: wilcoxon(a, b, method="approx", correction=True)
        // -> p = 2.8923111812925393e-07, W+ = 724.5, W- = 16.5, 38 non-zero differences.
        let a = [
            10.0, 12.0, 9.0, 15.0, 14.0, 11.0, 13.0, 16.0, 12.0, 10.0, 18.0, 17.0, 9.0, 14.0, 13.0, 15.0, 12.0, 11.0, 10.0, 16.0, 17.0, 14.0, 13.0, 12.0, 11.0, 15.0, 14.0,
            10.0, 9.0, 13.0, 12.0, 14.0, 18.0, 17.0, 16.0, 15.0, 14.0, 13.0, 12.0, 11.0,
        ];
        let b: Vec<f64> = a
            .iter()
            .enumerate()
            .map(|(i, &ai)| ai - if i % 3 != 0 { 1.5 } else { -0.5 } - (i % 5) as f64 * 0.5 + if i % 2 == 1 { 0.25 } else { 0.0 })
            .collect();
        let r = wilcoxon_signed_rank(&a, &b);
        assert_eq!(r.method, "normal");
        assert_eq!(r.n_pairs, 40);
        assert_eq!(r.n_nonzero, 38);
        assert!(close(r.w_plus, 724.5, 1e-9) && close(r.w_minus, 16.5, 1e-9));
        assert!(close(r.p_value, 2.8923111812925393e-07, 1e-10), "p = {}", r.p_value);
        assert!(r.z > 0.0);
    }

    #[test]
    fn wilcoxon_identical_samples_are_degenerate() {
        let x = [1.0, 2.0, 3.0];
        let r = wilcoxon_signed_rank(&x, &x);
        assert_eq!(r.method, "degenerate");
        assert!(close(r.p_value, 1.0, 1e-12));
    }

    #[test]
    fn wilcoxon_skips_non_finite_pairs() {
        let x = [1.0, f64::NAN, 3.0, 4.0];
        let y = [0.0, 1.0, f64::INFINITY, 1.0];
        let r = wilcoxon_signed_rank(&x, &y);
        assert_eq!(r.n_pairs, 2);
    }

    #[test]
    fn holm_bonferroni_known_values() {
        let adj = holm_bonferroni(&[0.01, 0.04, 0.03, 0.005]);
        let want = [0.03, 0.06, 0.06, 0.02];
        for (a, w) in adj.iter().zip(want.iter()) {
            assert!(close(*a, *w, 1e-12), "{:?}", adj);
        }
        let with_nan = holm_bonferroni(&[0.01, f64::NAN, 0.02]);
        assert!(close(with_nan[0], 0.02, 1e-12) && close(with_nan[2], 0.02, 1e-12) && with_nan[1].is_nan());
        assert!(holm_bonferroni(&[0.9, 0.8]).iter().all(|&p| p <= 1.0));
    }

    #[test]
    fn effect_size_dz() {
        let x = [2.0, 3.0, 4.0, 5.0];
        let y = [1.0, 1.0, 2.0, 3.0];
        // differences 1, 2, 2, 2 -> mean 1.75, sd 0.5
        let r = wilcoxon_signed_rank(&x, &y);
        assert!(close(r.cohens_dz, 3.5, 1e-12));
        assert!(close(r.median_diff, 2.0, 1e-12));
    }
}
