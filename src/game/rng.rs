/// MSVC LCG random number generator (FUN_00406cda)
/// seed = seed * 0x343FD + 0x269EC3
/// output = (seed >> 16) & 0x7FFF
pub struct Rng {
    seed: u32,
    calls: u64,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self { seed, calls: 0 }
    }

    pub fn rand(&mut self) -> u16 {
        self.calls += 1;
        self.seed = self.seed.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
        ((self.seed >> 16) & 0x7FFF) as u16
    }

    pub fn call_count(&self) -> u64 {
        self.calls
    }

    /// Mirrors FUN_004020b0: rand() % n
    pub fn rand_range(&mut self, n: u16) -> u16 {
        if n == 0 {
            return 0;
        }
        self.rand() % n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference sequence for srand(1), the well-known MSVC rand() output.
    #[test]
    fn lcg_sequence_seed_1() {
        let mut rng = Rng::new(1);
        let expected = [41u16, 18467, 6334, 26500, 19169, 15724, 11478, 29358];
        for &want in &expected {
            assert_eq!(rng.rand(), want);
        }
    }

    /// Reference sequence for srand(0).
    #[test]
    fn lcg_sequence_seed_0() {
        let mut rng = Rng::new(0);
        let expected = [38u16, 7719, 21238, 2437, 8855, 11797, 8365, 32285];
        for &want in &expected {
            assert_eq!(rng.rand(), want);
        }
    }

    #[test]
    fn rand_range_is_modulo() {
        let mut rng = Rng::new(1);
        // First three outputs with seed 1: 41, 18467, 6334
        assert_eq!(rng.rand_range(10), 1);
        assert_eq!(rng.rand_range(10), 7);
        assert_eq!(rng.rand_range(10), 4);
    }

    #[test]
    fn rand_range_zero_is_zero() {
        let mut rng = Rng::new(1);
        assert_eq!(rng.rand_range(0), 0);
    }
}
