pub trait RandomSource {
    fn uniform(&mut self) -> f64;
    fn normal(&mut self) -> f64;

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn index(&mut self, upper_bound: usize) -> usize {
        debug_assert!(upper_bound > 0);
        ((self.uniform() * upper_bound as f64) as usize).min(upper_bound - 1)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DeterministicRng {
    state: u64,
    spare_normal: Option<f64>,
}

impl DeterministicRng {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            state: seed,
            spare_normal: None,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }
}

impl RandomSource for DeterministicRng {
    fn uniform(&mut self) -> f64 {
        const SCALE: f64 = 1.0 / ((1_u64 << 53) as f64);
        ((self.next_u64() >> 11) as f64) * SCALE
    }

    fn normal(&mut self) -> f64 {
        if let Some(value) = self.spare_normal.take() {
            return value;
        }
        let radius = (-2.0 * self.uniform().max(f64::MIN_POSITIVE).ln()).sqrt();
        let angle = std::f64::consts::TAU * self.uniform();
        self.spare_normal = Some(radius * angle.sin());
        radius * angle.cos()
    }
}
