//! Deterministic, dependency-free random number generation.
//!
//! * `Rng` is xoshiro256** seeded through SplitMix64 (sequential streams).
//! * `hash_combine` / `unit_f64` give *counter-based* randomness: the draw for a
//!   given (seed, link, tick, salt) never depends on event order, which lets all
//!   variants share identical loss/latency draws (common random numbers).

pub fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Stateless 64-bit mixer.
pub fn mix64(x: u64) -> u64 {
    let mut s = x;
    splitmix64(&mut s)
}

pub fn hash_combine(a: u64, b: u64) -> u64 {
    mix64(a ^ mix64(b.wrapping_add(0x632B_E59B_D9B4_E019)))
}

/// Map a 64-bit hash to a uniform f64 in [0, 1).
pub fn unit_f64(h: u64) -> f64 {
    (h >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
}

#[derive(Clone, Debug)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut sm = seed;
        let s = [
            splitmix64(&mut sm),
            splitmix64(&mut sm),
            splitmix64(&mut sm),
            splitmix64(&mut sm),
        ];
        Rng { s }
    }

    /// Independent sub-stream derived from a master seed and a stream id.
    pub fn stream(master: u64, stream_id: u64) -> Self {
        Rng::new(hash_combine(master, stream_id))
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    pub fn next_f64(&mut self) -> f64 {
        unit_f64(self.next_u64())
    }

    /// Uniform integer in [0, n). `n` must be > 0.
    pub fn below(&mut self, n: usize) -> usize {
        debug_assert!(n > 0);
        (((self.next_u64() as u128) * (n as u128)) >> 64) as usize
    }

    /// Standard normal via Box-Muller.
    pub fn normal(&mut self) -> f64 {
        let u1 = loop {
            let u = self.next_f64();
            if u > 1e-300 {
                break u;
            }
        };
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    /// Fisher-Yates shuffle.
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        let n = v.len();
        if n < 2 {
            return;
        }
        for i in (1..n).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix_reference_value() {
        let mut s = 0u64;
        assert_eq!(splitmix64(&mut s), 0xE220_A839_7B1D_CDAF);
    }

    #[test]
    fn xoshiro_reference_values() {
        // Reference values computed independently (Python) from the same algorithm.
        let mut r = Rng::new(42);
        assert_eq!(r.next_u64(), 0x1578_0B2E_0C2E_C716);
        assert_eq!(r.next_u64(), 0x6104_D986_6D11_3A7E);
        assert_eq!(r.next_u64(), 0xAE17_5332_39E4_99A1);
    }

    #[test]
    fn deterministic_and_stream_independent() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        let mut s1 = Rng::stream(7, 1);
        let mut s2 = Rng::stream(7, 2);
        assert_ne!(s1.next_u64(), s2.next_u64());
    }

    #[test]
    fn uniform_range_and_mean() {
        let mut r = Rng::new(1);
        let mut sum = 0.0;
        for _ in 0..20000 {
            let x = r.next_f64();
            assert!((0.0..1.0).contains(&x));
            sum += x;
        }
        assert!((sum / 20000.0 - 0.5).abs() < 0.01);
        for _ in 0..1000 {
            assert!(r.below(7) < 7);
        }
    }

    #[test]
    fn normal_moments() {
        let mut r = Rng::new(3);
        let n = 20000;
        let xs: Vec<f64> = (0..n).map(|_| r.normal()).collect();
        let mean = xs.iter().sum::<f64>() / n as f64;
        let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
        assert!(mean.abs() < 0.03);
        assert!((var - 1.0).abs() < 0.05);
    }

    #[test]
    fn shuffle_is_permutation() {
        let mut r = Rng::new(9);
        let mut v: Vec<usize> = (0..50).collect();
        r.shuffle(&mut v);
        let mut sorted = v.clone();
        sorted.sort();
        assert_eq!(sorted, (0..50).collect::<Vec<_>>());
    }
}
