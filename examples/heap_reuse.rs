#![no_std]
#![no_main]
extern crate alloc;
use alloc::alloc::{alloc_zeroed, dealloc};
use core::alloc::Layout;
use emulsiv_libos as os;
os::entry!(main);
fn main() -> ! {
    let mut bus = unsafe { os::bus::Mmio::new() };
    let layout = Layout::from_size_align(32, 32).unwrap();
    for _ in 0..100 {
        let p = unsafe { alloc_zeroed(layout) };
        if p.is_null() {
            panic!();
        }
        if p as usize & 31 != 0 || unsafe { p.read_volatile() } != 0 {
            panic!();
        }
        unsafe {
            p.write_volatile(0x5a);
            dealloc(p, layout);
        }
    }
    let stats = os::heap::stats();
    if stats.used != 0 {
        panic!();
    }
    os::textio::TextIo::new(&mut bus).write_str("reuse=100 free=all\n");
    os::runtime::halt()
}
