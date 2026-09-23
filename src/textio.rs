//! Byte-oriented console. The hardware input is a one-byte latch, not a FIFO.
use crate::bus::{RegisterIo, TEXT_CTRL, TEXT_DATA, TEXT_OUT};
pub const RECEIVED: u8 = 0x40;
pub const IRQ_ENABLE: u8 = 0x80;
pub struct TextIo<'a, B: RegisterIo> {
    bus: &'a mut B,
}
impl<'a, B: RegisterIo> TextIo<'a, B> {
    pub fn new(bus: &'a mut B) -> Self {
        Self { bus }
    }
    pub fn put_byte(&mut self, byte: u8) {
        self.bus.write8(TEXT_OUT, byte);
    }
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.put_byte(b);
        }
    }
    pub fn write_str(&mut self, s: &str) {
        self.write_bytes(s.as_bytes());
    }
    pub fn available(&mut self) -> bool {
        self.bus.read8(TEXT_CTRL) & RECEIVED != 0
    }
    pub fn try_read(&mut self) -> Option<u8> {
        let status = self.bus.read8(TEXT_CTRL);
        if status & RECEIVED == 0 {
            return None;
        }
        let byte = self.bus.read8(TEXT_DATA);
        self.bus.write8(TEXT_CTRL, status & IRQ_ENABLE);
        Some(byte)
    }
    /// Blocking read; only use where waiting for user input is intended.
    pub fn read(&mut self) -> u8 {
        loop {
            if let Some(b) = self.try_read() {
                return b;
            }
        }
    }
    /// Bounded polling, measured in attempts rather than milliseconds.
    pub fn read_with_budget(&mut self, attempts: usize) -> Option<u8> {
        for _ in 0..attempts {
            if let Some(b) = self.try_read() {
                return Some(b);
            }
        }
        None
    }
    pub fn set_interrupts(&mut self, enabled: bool) {
        let received = self.bus.read8(TEXT_CTRL) & RECEIVED;
        self.bus
            .write8(TEXT_CTRL, received | if enabled { IRQ_ENABLE } else { 0 });
    }
    /// Fixed-width, small-code hexadecimal output without core::fmt.
    pub fn write_hex32(&mut self, value: u32) {
        self.write_str("0x");
        for shift in (0..8).rev() {
            let n = ((value >> (shift * 4)) & 15) as u8;
            self.put_byte(if n < 10 { b'0' + n } else { b'a' + n - 10 });
        }
    }
    pub fn write_u32(&mut self, mut value: u32) {
        let mut digits = [0u8; 10];
        let mut i = digits.len();
        loop {
            i -= 1;
            digits[i] = b'0' + (value % 10) as u8;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        self.write_bytes(&digits[i..]);
    }
    pub fn write_i32(&mut self, value: i32) {
        if value < 0 {
            self.put_byte(b'-');
        }
        self.write_u32(value.unsigned_abs());
    }
}
#[cfg(feature = "format")]
impl<B: RegisterIo> core::fmt::Write for TextIo<'_, B> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        TextIo::write_str(self, s);
        Ok(())
    }
}
