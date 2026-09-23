//! Register access abstraction. Drivers can run against a test bus on the host.
use core::{marker::PhantomData, ptr::{read_volatile, write_volatile}};
pub const TEXT_CTRL: usize = 0xB000_0000;
pub const TEXT_DATA: usize = 0xB000_0001;
pub const TEXT_OUT: usize = 0xC000_0000;
pub const GPIO_BASE: usize = 0xD000_0000;
pub const FRAMEBUFFER: usize = 0x0000_0C00;

pub trait RegisterIo {
    fn read8(&mut self, address: usize) -> u8;
    fn write8(&mut self, address: usize, value: u8);
    fn read32(&mut self, address: usize) -> u32;
    fn write32(&mut self, address: usize, value: u32);
}

/// A raw MMIO handle, intentionally neither Send nor Sync.
pub struct Mmio { _single_hart: PhantomData<*mut ()> }
impl Mmio {
    /// # Safety
    /// Execute only on emulsiV with the documented address map. No Rust allocation
    /// may overlap the MMIO/framebuffer region. All accesses must stay on one hart.
    /// Multiple handles do not provide read-modify-write synchronization.
    pub const unsafe fn new() -> Self { Self { _single_hart: PhantomData } }
}
impl RegisterIo for Mmio {
    fn read8(&mut self, a: usize) -> u8 {
        assert!(a == TEXT_CTRL || a == TEXT_DATA || (FRAMEBUFFER..FRAMEBUFFER+1024).contains(&a));
        // SAFETY: constructor contract and the checked address range.
        unsafe { read_volatile(a as *const u8) }
    }
    fn write8(&mut self, a: usize, v: u8) {
        assert!(a == TEXT_CTRL || a == TEXT_OUT || (FRAMEBUFFER..FRAMEBUFFER+1024).contains(&a));
        unsafe { write_volatile(a as *mut u8, v) }
    }
    fn read32(&mut self, a: usize) -> u32 {
        assert!((GPIO_BASE..=GPIO_BASE+16).contains(&a) && a & 3 == 0);
        unsafe { read_volatile(a as *const u32) }
    }
    fn write32(&mut self, a: usize, v: u32) {
        assert!((GPIO_BASE..=GPIO_BASE+16).contains(&a) && a & 3 == 0);
        unsafe { write_volatile(a as *mut u32, v) }
    }
}
