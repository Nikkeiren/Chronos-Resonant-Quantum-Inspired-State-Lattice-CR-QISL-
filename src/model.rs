//! State-update equations.
//!
//! HONESTY NOTE. The CR-QISL specification (v0.1.0-v0.1.2) is not available to
//! this implementation as an explicit equation set. Every equation below is
//! therefore an *operational assumption* (tag `ASSUMPTION[Axx]`), documented in
//! `docs/ASSUMPTIONS.md`, kept configurable, and NOT presented as established
//! CR-QISL mathematics. Replace the bodies of `step_state`, `integrity_targets`
//! and `l1_normalize` with the specification equations once they are available;
//! the rest of the engine only depends on their signatures.
//!
//! Notation: node i holds, for every candidate k, a complex amplitude
//! psi_ik = a_ik * exp(i * theta_ik) with a_ik >= 0 and sum_k a_ik = 1 (L1).

use crate::network::DEC_NONE;
use std::f64::consts::PI;

pub const EPS: f64 = 1e-12;
pub const TWO_PI: f64 = 2.0 * PI;

/// Wrap an angle to [0, 2*pi). NaN stays NaN (so corruption stays visible).
pub fn wrap_phase(x: f64) -> f64 {
    x.rem_euclid(TWO_PI)
}

/// Circular distance in [0, pi].
pub fn circ_dist(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(TWO_PI);
    if d > PI {
        TWO_PI - d
    } else {
        d
    }
}

/// A neighbour state as seen by the aggregation (phases already age-compensated).
#[derive(Clone, Debug)]
pub struct Sample {
    pub amp: Vec<f64>,
    pub phase: Vec<f64>,
    pub weight: f64,
}

#[derive(Clone, Debug)]
pub struct StepParams {
    /// Effective step size: rate * (1 - alpha) [* reconcile boost], clamped to (0, 1].
    pub step: f64,
    pub beta: f64,
    pub phase_on: bool,
    pub omega: f64,
    pub validate: bool,
}

#[derive(Clone, Debug)]
pub struct StepResult {
    pub amp: Vec<f64>,
    pub phase: Vec<f64>,
    pub normalization_reset: bool,
}

/// ASSUMPTION[A2] L1 normalisation: a_k <- a_k / sum_j a_j.
/// Degenerate sums (<= EPS or non-finite) reset to the uniform distribution only
/// when input validation is on; otherwise corrupted values are left untouched so
/// that "invalid numerical input" experiments can expose the corruption.
/// Returns true when a uniform reset happened.
pub fn l1_normalize(amp: &mut [f64], validate: bool) -> bool {
    let sum: f64 = amp.iter().sum();
    let ok = sum.is_finite() && sum > EPS && amp.iter().all(|a| a.is_finite() && *a >= 0.0);
    if ok {
        for a in amp.iter_mut() {
            *a /= sum;
        }
        false
    } else if validate {
        let u = 1.0 / amp.len() as f64;
        for a in amp.iter_mut() {
            *a = u;
        }
        true
    } else {
        false
    }
}

/// One local update of the node state.
///
/// ASSUMPTION[A1] (aggregation): weighted complex mean of fresh neighbour states
///   N_k = sum_j w_j psi_jk / sum_j w_j.
/// ASSUMPTION[A3] (resonance / phase interaction, phase on only):
///   target magnitude  r_k = (1 - beta) * (sum_j w_j a_jk / sum_j w_j) + beta * |N_k|
///   target phase      phi_k = arg N_k   (own phase if |N_k| ~ 0)
///   T_k = r_k * exp(i * phi_k). beta = 0 ignores interference, beta = 1 is fully
///   interference-driven: aligned phases reinforce, opposed phases cancel.
/// ASSUMPTION[A4]+[A5] (gossip step and damping):
///   psi_k <- psi_k + step * (T_k - psi_k),  step = rate * (1 - alpha).
/// Phase off: T_k = arithmetic weighted mean amplitude and phases stay 0
///   (this is the plain weighted-gossip update).
/// ASSUMPTION[A7] optional intrinsic rotation: theta_k <- theta_k + omega.
/// Finally ASSUMPTION[A2] L1 normalisation.
pub fn step_state(own_amp: &[f64], own_phase: &[f64], samples: &[Sample], p: &StepParams) -> StepResult {
    let k = own_amp.len();
    let mut amp = vec![0.0; k];
    let mut phase = vec![0.0; k];
    let wsum: f64 = samples.iter().map(|s| s.weight).sum();
    if samples.is_empty() || !(wsum > 0.0) {
        amp.copy_from_slice(own_amp);
        phase.copy_from_slice(own_phase);
    } else {
        for c in 0..k {
            let mut amean = 0.0;
            let mut re = 0.0;
            let mut im = 0.0;
            for s in samples {
                let w = s.weight / wsum;
                amean += w * s.amp[c];
                if p.phase_on {
                    re += w * s.amp[c] * s.phase[c].cos();
                    im += w * s.amp[c] * s.phase[c].sin();
                }
            }
            if p.phase_on {
                let mag = re.hypot(im);
                let target_amp = (1.0 - p.beta) * amean + p.beta * mag;
                let target_phase = if mag > EPS { im.atan2(re) } else { own_phase[c] };
                let ore = own_amp[c] * own_phase[c].cos();
                let oim = own_amp[c] * own_phase[c].sin();
                let tre = target_amp * target_phase.cos();
                let tim = target_amp * target_phase.sin();
                let nre = ore + p.step * (tre - ore);
                let nim = oim + p.step * (tim - oim);
                let nmag = nre.hypot(nim);
                amp[c] = nmag;
                phase[c] = if nmag > EPS { nim.atan2(nre) } else { target_phase };
            } else {
                amp[c] = own_amp[c] + p.step * (amean - own_amp[c]);
                phase[c] = 0.0;
            }
        }
    }
    if p.phase_on {
        for ph in phase.iter_mut() {
            *ph = wrap_phase(*ph + p.omega);
        }
    }
    let reset = l1_normalize(&mut amp, p.validate);
    StepResult {
        amp,
        phase,
        normalization_reset: reset,
    }
}

/// Receiver-side numerical validation of a state message.
pub fn validate_state(amp: &[f64], phase: &[f64], k: usize) -> bool {
    amp.len() == k
        && phase.len() == k
        && amp.iter().all(|a| a.is_finite() && *a >= 0.0 && *a <= 1.0 + 1e-6)
        && phase.iter().all(|p| p.is_finite())
        && (amp.iter().sum::<f64>() - 1.0).abs() <= 1e-3
}

#[derive(Clone, Debug)]
pub struct IntegrityParams {
    pub kappa: f64,
    pub phase_gamma: f64,
    pub phase_on: bool,
}

/// ASSUMPTION[A6] Integrity target per neighbour. The deviation of neighbour j is
/// measured against the coordinate-wise median of {neighbours, own state}:
///   d_j = 0.5 * sum_k |a_jk - med_k|  [+ gamma * sum_k med_k * circdist(theta_jk, phimed_k) / pi]
/// and the target trust is exp(-kappa * d_j) in (0, 1].
/// The weight then moves toward the target: w <- (1 - eta) * w + eta * target.
/// This is an outlier heuristic. It penalises honest-but-divergent neighbours and
/// is defeated by a local adversarial majority; it gives NO protection guarantee.
pub fn integrity_targets(own_amp: &[f64], own_phase: &[f64], neigh: &[Sample], p: &IntegrityParams) -> Vec<f64> {
    let k = own_amp.len();
    let m = neigh.len();
    if m < 2 {
        return vec![1.0; m];
    }
    let mut med = vec![0.0; k];
    for c in 0..k {
        let mut v: Vec<f64> = neigh.iter().map(|s| s.amp[c]).collect();
        v.push(own_amp[c]);
        v.sort_by(|a, b| a.total_cmp(b));
        let l = v.len();
        med[c] = if l % 2 == 1 { v[l / 2] } else { 0.5 * (v[l / 2 - 1] + v[l / 2]) };
    }
    let mut pmed = vec![0.0; k];
    if p.phase_on {
        for c in 0..k {
            let mut cands: Vec<f64> = neigh.iter().map(|s| s.phase[c]).collect();
            cands.push(own_phase[c]);
            let mut best = cands[0];
            let mut best_cost = f64::INFINITY;
            for &x in &cands {
                let cost: f64 = cands.iter().map(|&y| circ_dist(x, y)).sum();
                if cost < best_cost {
                    best_cost = cost;
                    best = x;
                }
            }
            pmed[c] = best;
        }
    }
    neigh
        .iter()
        .map(|s| {
            let mut d = 0.0;
            for c in 0..k {
                d += (s.amp[c] - med[c]).abs();
            }
            d *= 0.5;
            if p.phase_on {
                let mut pd = 0.0;
                for c in 0..k {
                    pd += med[c] * circ_dist(s.phase[c], pmed[c]) / PI;
                }
                d += p.phase_gamma * pd;
            }
            let t = (-p.kappa * d).exp();
            if t.is_finite() {
                t
            } else {
                0.0
            }
        })
        .collect()
}

/// Index of the largest amplitude and the margin to the runner-up.
/// Non-finite amplitudes yield (DEC_NONE, 0).
pub fn top_two(amp: &[f64]) -> (u8, f64) {
    if amp.iter().any(|a| !a.is_finite()) {
        return (DEC_NONE, 0.0);
    }
    let mut best = 0usize;
    for (i, a) in amp.iter().enumerate() {
        if *a > amp[best] {
            best = i;
        }
    }
    let mut second = f64::NEG_INFINITY;
    for (i, a) in amp.iter().enumerate() {
        if i != best && *a > second {
            second = *a;
        }
    }
    if second == f64::NEG_INFINITY {
        second = 0.0;
    }
    (best as u8, amp[best] - second)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(phase_on: bool, beta: f64, step: f64) -> StepParams {
        StepParams {
            step,
            beta,
            phase_on,
            omega: 0.0,
            validate: true,
        }
    }

    fn sample(amp: &[f64], phase: &[f64], w: f64) -> Sample {
        Sample {
            amp: amp.to_vec(),
            phase: phase.to_vec(),
            weight: w,
        }
    }

    #[test]
    fn l1_normalization_sums_to_one() {
        let mut a = vec![2.0, 1.0, 1.0];
        assert!(!l1_normalize(&mut a, true));
        assert!((a.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!((a[0] - 0.5).abs() < 1e-12);
    }

    #[test]
    fn degenerate_sum_resets_only_with_validation() {
        let mut a = vec![0.0, 0.0];
        assert!(l1_normalize(&mut a, true));
        assert_eq!(a, vec![0.5, 0.5]);
        let mut b = vec![f64::NAN, 0.5];
        assert!(!l1_normalize(&mut b, false));
        assert!(b[0].is_nan());
    }

    #[test]
    fn wrap_and_circular_distance() {
        assert!((wrap_phase(-0.5) - (TWO_PI - 0.5)).abs() < 1e-12);
        assert!((wrap_phase(TWO_PI + 0.25) - 0.25).abs() < 1e-12);
        assert!((circ_dist(0.1, TWO_PI - 0.1) - 0.2).abs() < 1e-12);
        assert!((circ_dist(0.0, PI) - PI).abs() < 1e-12);
    }

    #[test]
    fn no_neighbours_keeps_state() {
        let r = step_state(&[0.7, 0.3], &[0.0, 0.0], &[], &params(false, 0.0, 0.5));
        assert!((r.amp[0] - 0.7).abs() < 1e-12 && (r.amp[1] - 0.3).abs() < 1e-12);
    }

    #[test]
    fn consensus_is_a_fixed_point() {
        let s = vec![sample(&[0.6, 0.4], &[0.3, 1.0], 1.0), sample(&[0.6, 0.4], &[0.3, 1.0], 2.0)];
        for phase_on in [false, true] {
            let r = step_state(&[0.6, 0.4], &[0.3, 1.0], &s, &params(phase_on, 0.7, 0.5));
            assert!((r.amp[0] - 0.6).abs() < 1e-9, "phase_on={}", phase_on);
            assert!((r.amp[1] - 0.4).abs() < 1e-9);
        }
    }

    #[test]
    fn phase_off_is_weighted_average_step() {
        let s = vec![sample(&[0.9, 0.1], &[0.0, 0.0], 1.0), sample(&[0.5, 0.5], &[0.0, 0.0], 3.0)];
        let r = step_state(&[0.2, 0.8], &[0.0, 0.0], &s, &params(false, 0.9, 0.5));
        let mean0 = 0.25 * 0.9 + 0.75 * 0.5;
        let expect0 = 0.2 + 0.5 * (mean0 - 0.2);
        assert!((r.amp[0] - expect0).abs() < 1e-12);
        assert!((r.amp.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn destructive_interference_reduces_candidate_support() {
        // Candidate 0: neighbours in anti-phase (cancel). Candidate 1: aligned (reinforce).
        let s = vec![sample(&[0.5, 0.5], &[0.0, 0.0], 1.0), sample(&[0.5, 0.5], &[PI, 0.0], 1.0)];
        let with_beta = step_state(&[0.5, 0.5], &[0.0, 0.0], &s, &params(true, 1.0, 1.0));
        assert!(with_beta.amp[0] < 1e-6, "got {:?}", with_beta.amp);
        let without = step_state(&[0.5, 0.5], &[0.0, 0.0], &s, &params(true, 0.0, 1.0));
        assert!((without.amp[0] - 0.5).abs() < 1e-9, "beta=0 must ignore interference");
    }

    #[test]
    fn output_always_normalised_and_phases_wrapped() {
        let s = vec![sample(&[0.3, 0.7], &[5.0, 2.0], 0.4), sample(&[0.8, 0.2], &[0.1, 6.0], 1.0)];
        let r = step_state(&[0.5, 0.5], &[1.0, 4.0], &s, &params(true, 0.5, 0.35));
        assert!((r.amp.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!(r.phase.iter().all(|p| (0.0..TWO_PI).contains(p)));
    }

    #[test]
    fn validation_rejects_bad_numbers() {
        assert!(validate_state(&[0.5, 0.5], &[0.0, 1.0], 2));
        assert!(!validate_state(&[f64::NAN, 0.5], &[0.0, 0.0], 2));
        assert!(!validate_state(&[f64::INFINITY, 0.0], &[0.0, 0.0], 2));
        assert!(!validate_state(&[-0.1, 1.1], &[0.0, 0.0], 2));
        assert!(!validate_state(&[0.5, 0.2], &[0.0, 0.0], 2));
        assert!(!validate_state(&[0.5, 0.5], &[f64::NAN, 0.0], 2));
        assert!(!validate_state(&[0.5, 0.5, 0.0], &[0.0, 0.0, 0.0], 2));
    }

    #[test]
    fn integrity_penalises_outlier() {
        let honest = |a: f64| sample(&[a, 1.0 - a], &[0.0, 0.0], 1.0);
        let neigh = vec![honest(0.60), honest(0.62), honest(0.58), honest(0.61), sample(&[0.0, 1.0], &[0.0, 0.0], 1.0)];
        let ip = IntegrityParams {
            kappa: 8.0,
            phase_gamma: 0.5,
            phase_on: false,
        };
        let t = integrity_targets(&[0.6, 0.4], &[0.0, 0.0], &neigh, &ip);
        assert_eq!(t.len(), 5);
        assert!(t[4] < 0.1 && t[0] > 0.8, "{:?}", t);
    }

    #[test]
    fn integrity_detects_phase_outlier() {
        let mk = |ph: f64| sample(&[0.5, 0.5], &[ph, ph], 1.0);
        let neigh = vec![mk(0.0), mk(0.05), mk(0.1), mk(PI)];
        let ip = IntegrityParams {
            kappa: 8.0,
            phase_gamma: 0.5,
            phase_on: true,
        };
        let t = integrity_targets(&[0.5, 0.5], &[0.0, 0.0], &neigh, &ip);
        assert!(t[3] < t[0] && t[3] < 0.5, "{:?}", t);
    }

    #[test]
    fn top_two_margin() {
        assert_eq!(top_two(&[0.7, 0.3]).0, 0);
        assert!((top_two(&[0.2, 0.5, 0.3]).1 - 0.2).abs() < 1e-12);
        assert_eq!(top_two(&[f64::NAN, 0.5]).0, DEC_NONE);
    }
}
