//! Deterministic pseudo-random number generator for reproducible montage planning.
//!
//! Uses SplitMix64 to guarantee identical cut plans on all platforms given the same seed.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    /// Creates a new generator from a 64-bit seed.
    pub fn new(seed: u64) -> Self {
        // If seed is 0, give it a non-zero initialization constant
        let initial = if seed == 0 { 0x5851f42d4c957f2d } else { seed };
        Self { state: initial }
    }

    /// Generates the next pseudo-random 64-bit integer.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    /// Generates a float in the range `[0.0, 1.0)`.
    pub fn next_f32(&mut self) -> f32 {
        let val = (self.next_u64() >> 40) as u32;
        val as f32 / (1u32 << 24) as f32
    }

    /// Generates an integer in the range `[low, high)` (or `low` if high <= low).
    pub fn gen_range_i64(&mut self, low: i64, high: i64) -> i64 {
        if high <= low {
            return low;
        }
        let range = (high - low) as u64;
        let offset = self.next_u64() % range;
        low + (offset as i64)
    }

    /// Generates an integer in the range `[low, high)` (or `low` if high <= low).
    pub fn gen_range_usize(&mut self, low: usize, high: usize) -> usize {
        if high <= low {
            return low;
        }
        let range = high - low;
        let offset = (self.next_u64() as usize) % range;
        low + offset
    }

    /// Shuffles a slice in place deterministically (Fisher-Yates).
    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        for i in (1..slice.len()).rev() {
            let j = (self.next_u64() as usize) % (i + 1);
            slice.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rng_determinism() {
        let mut rng1 = DeterministicRng::new(12345);
        let mut rng2 = DeterministicRng::new(12345);

        for _ in 0..100 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
            assert_eq!(rng1.next_f32(), rng2.next_f32());
        }
    }

    #[test]
    fn test_different_seeds_differ() {
        let mut rng1 = DeterministicRng::new(12345);
        let mut rng2 = DeterministicRng::new(67890);

        let vals1: Vec<u64> = (0..20).map(|_| rng1.next_u64()).collect();
        let vals2: Vec<u64> = (0..20).map(|_| rng2.next_u64()).collect();

        assert_ne!(vals1, vals2);
    }
}
