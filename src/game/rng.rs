/// MSVC LCG random number generator (FUN_00406cda)
/// seed = seed * 0x343FD + 0x269EC3
/// output = (seed >> 16) & 0x7FFF
pub struct Rng {
    seed: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self { seed }
    }

    pub fn rand(&mut self) -> u16 {
        self.seed = self.seed.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
        ((self.seed >> 16) & 0x7FFF) as u16
    }

    /// Mirrors FUN_004020b0: rand() % n
    pub fn rand_range(&mut self, n: u16) -> u16 {
        if n == 0 {
            return 0;
        }
        self.rand() % n
    }
}
