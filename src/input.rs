//! Debounce measured in consecutive samples, independent of any hardware timer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Rising,
    Falling,
}
pub struct Debouncer {
    threshold: u16,
    count: u16,
    stable: bool,
    candidate: bool,
}
impl Debouncer {
    pub const fn new(initial: bool, threshold: u16) -> Self {
        Self {
            threshold: if threshold == 0 { 1 } else { threshold },
            count: 0,
            stable: initial,
            candidate: initial,
        }
    }
    pub const fn value(&self) -> bool {
        self.stable
    }
    pub fn sample(&mut self, value: bool) -> Option<Edge> {
        if value == self.stable {
            self.count = 0;
            self.candidate = value;
            return None;
        }
        if value != self.candidate {
            self.candidate = value;
            self.count = 0;
        }
        self.count = self.count.saturating_add(1);
        if self.count < self.threshold {
            return None;
        }
        self.stable = value;
        self.count = 0;
        Some(if value { Edge::Rising } else { Edge::Falling })
    }
}
/// Software PWM phase calculation. Caller supplies logical ticks and writes GPIO.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pwm {
    period: u16,
    duty: u16,
}
impl Pwm {
    pub const fn new(period: u16, duty: u16) -> Option<Self> {
        if period == 0 || duty > period {
            None
        } else {
            Some(Self { period, duty })
        }
    }
    pub fn level(self, tick: u32) -> bool {
        tick % u32::from(self.period) < u32::from(self.duty)
    }
    pub fn set_duty(&mut self, duty: u16) -> bool {
        if duty > self.period {
            false
        } else {
            self.duty = duty;
            true
        }
    }
}
