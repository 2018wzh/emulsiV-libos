//! Target-only startup and peripheral interrupt masking.
use crate::{
    bus::{Mmio, RegisterIo, TEXT_CTRL},
    gpio::IEN,
};
use core::{
    arch::global_asm,
    panic::PanicInfo,
    sync::atomic::{compiler_fence, Ordering},
};
global_asm!(include_str!("startup.S"));
global_asm!(include_str!("memory.S"));
#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    let mut io = unsafe { Mmio::new() };
    io.write8(TEXT_CTRL, 0);
    io.write32(IEN, 0);
    crate::textio::TextIo::new(&mut io).write_str("panic\n");
    halt()
}
pub fn halt() -> ! {
    loop {
        unsafe { core::arch::asm!("j .", options(nomem, nostack)) }
    }
}
/// Instruction-loop delay, not calibrated time. Does not use WFI or spin_loop.
pub fn delay_iterations(mut count: u32) {
    while count != 0 {
        unsafe { core::arch::asm!("nop", options(nomem, nostack, preserves_flags)) }
        count -= 1;
    }
}
#[no_mangle]
pub extern "C" fn __libos_default_irq() {
    // A default handler must not immediately re-enter on a pending level IRQ.
    let mut bus = unsafe { Mmio::new() };
    let ctrl = bus.read8(TEXT_CTRL);
    bus.write8(TEXT_CTRL, ctrl & 0x40);
    bus.write32(IEN, 0);
}
/// Mask the two known device IRQ sources, run a closure, then restore them.
/// This is NOT a CSR-based, architecture-wide critical-section implementation.
/// A source arriving during entry may run before both masks have been stored.
/// Does not prevent hardware input changes or guarantee lossless edge capture.
pub fn with_device_irqs_masked<R>(f: impl FnOnce() -> R) -> R {
    let mut bus = unsafe { Mmio::new() };
    let ctrl = bus.read8(TEXT_CTRL);
    let ien = bus.read32(IEN);
    bus.write8(TEXT_CTRL, ctrl & 0x40);
    bus.write32(IEN, 0);
    compiler_fence(Ordering::SeqCst);
    let result = f();
    compiler_fence(Ordering::SeqCst);
    let pending = bus.read8(TEXT_CTRL) & 0x40;
    bus.write8(TEXT_CTRL, pending | (ctrl & 0x80));
    bus.write32(IEN, ien);
    result
}

/// Linker layout plus instantaneous SP diagnostics, not stack-overflow protection.
#[derive(Clone, Copy, Debug)]
pub struct MemoryInfo {
    pub image_end: usize,
    pub stack_bottom: usize,
    pub stack_top: usize,
}
pub fn memory_info() -> MemoryInfo {
    extern "C" {
        static __image_end: u8;
        static __stack_bottom: u8;
        static __stack_top: u8;
    }
    // Only take symbol addresses. The linker symbols are not dereferenced.
    MemoryInfo {
        image_end: core::ptr::addr_of!(__image_end) as usize,
        stack_bottom: core::ptr::addr_of!(__stack_bottom) as usize,
        stack_top: core::ptr::addr_of!(__stack_top) as usize,
    }
}
/// Remaining reserved stack at the instant of this call, including this frame.
/// Does not measure the worst-case foreground-plus-IRQ stack usage.
pub fn stack_remaining() -> usize {
    let sp: usize;
    unsafe { core::arch::asm!("mv {}, sp",out(reg) sp,options(nomem,nostack,preserves_flags)) }
    sp.saturating_sub(memory_info().stack_bottom)
}
