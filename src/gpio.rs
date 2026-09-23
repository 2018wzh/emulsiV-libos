//! GPIO direction, output, input and edge latches.
//! Event registers are ordinary read/write memory, NOT write-one-to-clear.
use crate::bus::{RegisterIo, GPIO_BASE};
pub const DIR: usize = GPIO_BASE;
pub const IEN: usize = GPIO_BASE + 4;
pub const RISING: usize = GPIO_BASE + 8;
pub const FALLING: usize = GPIO_BASE + 12;
pub const VALUE: usize = GPIO_BASE + 16;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pin(u8);
impl Pin {
    pub const fn new(index: u8) -> Option<Self> { if index < 32 { Some(Self(index)) } else { None } }
    pub const fn mask(self) -> u32 { 1u32 << self.0 }
    pub const fn index(self) -> u8 { self.0 }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Edges { pub rising: u32, pub falling: u32 }
pub struct Gpio<'a, B: RegisterIo> { bus: &'a mut B }
impl<'a, B: RegisterIo> Gpio<'a, B> {
    pub fn new(bus: &'a mut B) -> Self { Self { bus } }
    pub fn direction(&mut self) -> u32 { self.bus.read32(DIR) }
    /// Direction bit 1 is input; bit 0 is output.
    pub fn set_direction(&mut self, inputs: u32) { self.bus.write32(DIR, inputs); }
    pub fn configure_inputs(&mut self, mask: u32) {
        let v = self.direction(); self.set_direction(v | mask);
    }
    pub fn configure_outputs(&mut self, mask: u32) {
        let v = self.direction(); self.set_direction(v & !mask);
    }
    pub fn read(&mut self) -> u32 { self.bus.read32(VALUE) }
    pub fn read_pin(&mut self, pin: Pin) -> bool { self.read() & pin.mask() != 0 }
    pub fn write(&mut self, value: u32) { self.bus.write32(VALUE, value); }
    /// Only selected currently-output bits are updated. Not IRQ-atomic.
    pub fn write_masked(&mut self, mask: u32, value: u32) {
        let outputs = mask & !self.direction();
        let old = self.read(); self.write((old & !outputs) | (value & outputs));
    }
    pub fn set_high(&mut self, mask: u32) { self.write_masked(mask, mask); }
    pub fn set_low(&mut self, mask: u32) { self.write_masked(mask, 0); }
    pub fn toggle(&mut self, mask: u32) { let v = self.read(); self.write_masked(mask, !v); }
    pub fn set_interrupt_mask(&mut self, mask: u32) { self.bus.write32(IEN, mask); }
    pub fn edges(&mut self) -> Edges {
        Edges { rising: self.bus.read32(RISING), falling: self.bus.read32(FALLING) }
    }
    /// Preserve unselected latched events. Hardware edges arriving during this
    /// read/modify/write may still be lost; this peripheral has no atomic ACK.
    pub fn clear_edges(&mut self, mask: u32) {
        let r = self.bus.read32(RISING); self.bus.write32(RISING, r & !mask);
        let f = self.bus.read32(FALLING); self.bus.write32(FALLING, f & !mask);
    }
    pub fn clear_all_edges(&mut self) {
        self.bus.write32(RISING, 0); self.bus.write32(FALLING, 0);
    }
}
