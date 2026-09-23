#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

fn main() -> ! {
    let mut bus = unsafe { Mmio::new() };
    let mut io = os::textio::TextIo::new(&mut bus);
    io.write_str("Type to echo:\n");
    loop { if let Some(byte) = io.try_read() { io.put_byte(byte); } }
}
