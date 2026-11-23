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
    #[track_caller]
    pub fn rand_range(&mut self, n: u16) -> u16 {
        if n == 0 {
            return 0;
        }
        #[cfg(feature = "rng_sites")]
        self::rng_sites::record(n, std::panic::Location::caller());
        let r = self.rand() % n;
        #[cfg(feature = "rng_sites")]
        if self::rng_sites::log_calls() {
            let loc = std::panic::Location::caller();
            eprintln!("rng {} {} {}:{}", self.calls, n, loc.file(), loc.line());
        }
        r
    }
}

/// Per-callsite RNG consumption accounting (build with `--features rng_sites`,
/// dump with `rng_sites::dump()`).
#[cfg(feature = "rng_sites")]
pub mod rng_sites {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    /// (range `n`, file, line) of a `rand_range` call site.
    type Site = (u16, &'static str, u32);

    static SITES: OnceLock<Mutex<HashMap<Site, u64>>> = OnceLock::new();
    static LOG_CALLS: OnceLock<bool> = OnceLock::new();

    /// `SKIFREE_LOG_RNG=1`: print every call in order as
    /// `rng <stream index> <n> <file>:<line>` (for aligning against an
    /// original-side Frida rand() trace).
    pub fn log_calls() -> bool {
        *LOG_CALLS.get_or_init(|| std::env::var_os("SKIFREE_LOG_RNG").is_some())
    }

    pub fn record(n: u16, loc: &'static std::panic::Location<'static>) {
        let map = SITES.get_or_init(|| Mutex::new(HashMap::new()));
        *map.lock().unwrap().entry((n, loc.file(), loc.line())).or_insert(0) += 1;
    }

    pub fn dump() {
        if let Some(map) = SITES.get() {
            let map = map.lock().unwrap();
            let mut rows: Vec<_> = map.iter().collect();
            rows.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
            let total: u64 = map.values().sum();
            eprintln!("rng calls by site (total {total}):");
            for ((n, file, line), count) in rows {
                eprintln!("  {file}:{line} rand_range({n}): {count}");
            }
        }
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
