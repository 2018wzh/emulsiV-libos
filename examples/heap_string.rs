#![no_std]
#![no_main]
extern crate alloc;
use alloc::string::String;
use emulsiv_libos as os;
os::entry!(main);
fn main() -> ! {
    let mut bus = unsafe { os::bus::Mmio::new() };
    let mut text = String::new();
    if text.try_reserve_exact(16).is_err() {
        panic!();
    }
    text.push_str("heap ");
    text.push_str("Rust");
    os::textio::TextIo::new(&mut bus).write_str(core::hint::black_box(&text));
    drop(text);
    os::runtime::halt()
}
