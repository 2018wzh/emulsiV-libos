//! Small deterministic PRNG for graphics and demos. NOT cryptographically secure.
pub struct XorShift32(u32);
impl XorShift32 {
    pub const fn new(seed: u32) -> Self {
        Self(if seed == 0 { 0x6d2b_79f5 } else { seed })
    }
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
}
