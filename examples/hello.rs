#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

fn main() -> ! {
    let mut bus = unsafe { Mmio::new() };
    os::textio::TextIo::new(&mut bus).write_str("Hello, emulsiV Rust!\n");
    os::runtime::halt()
}
