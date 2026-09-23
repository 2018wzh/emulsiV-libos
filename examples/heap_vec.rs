#![no_std]
#![no_main]
extern crate alloc;
use alloc::vec::Vec;
use emulsiv_libos as os;
os::entry!(main);
fn main() -> ! {
    let mut bus = unsafe { os::bus::Mmio::new() };
    let mut values = Vec::from([1u8, 2, 3, 4]);
    if values.try_reserve_exact(12).is_err() {
        panic!();
    }
    if values.try_reserve_exact(4096).is_ok() {
        panic!();
    }
    let values = core::hint::black_box(values);
    if values.as_slice() != [1, 2, 3, 4] || values.capacity() < 16 {
        panic!();
    }
    os::textio::TextIo::new(&mut bus).write_str("vec=grown oom=preserved\n");
    drop(values);
    os::runtime::halt()
}
