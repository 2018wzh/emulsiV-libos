#![no_std]
#![no_main]
extern crate alloc;
use alloc::alloc::{alloc, dealloc};
use core::alloc::Layout;
use emulsiv_libos as os;
os::entry!(main);
fn main() -> ! {
    let mut bus = unsafe { os::bus::Mmio::new() };
    let huge = Layout::from_size_align(4096, 16).unwrap();
    if !unsafe { alloc(huge) }.is_null() {
        panic!();
    }
    let small = Layout::from_size_align(16, 16).unwrap();
    let p = unsafe { alloc(small) };
    if p.is_null() {
        panic!();
    }
    unsafe {
        p.write_volatile(7);
        dealloc(p, small);
    }
    os::textio::TextIo::new(&mut bus).write_str("oom=recovered\n");
    os::runtime::halt()
}
