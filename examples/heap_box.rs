#![no_std]
#![no_main]
extern crate alloc;
use alloc::boxed::Box;
use emulsiv_libos as os;
os::entry!(main);
fn main() -> ! {
    let mut bus = unsafe { os::bus::Mmio::new() };
    let mut value = Box::new(42u32);
    // Expose the allocation to an optimization barrier so the firmware test
    // observes a real heap address instead of an elided allocation.
    let p = core::hint::black_box(&mut *value as *mut u32);
    let mut io = os::textio::TextIo::new(&mut bus);
    io.write_str("box=");
    io.write_u32(unsafe { p.read_volatile() });
    io.write_str(" address=");
    io.write_hex32(p as u32);
    io.put_byte(b'\n');
    drop(value);
    os::runtime::halt()
}
