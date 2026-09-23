#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

os::interrupt!(on_interrupt);
fn on_interrupt() {
    let mut bus = unsafe { Mmio::new() };
    let mut io = os::textio::TextIo::new(&mut bus);
    if let Some(byte) = io.try_read() { io.put_byte(byte); }
}
fn main() -> ! {
    let mut bus = unsafe { Mmio::new() };
    let mut io = os::textio::TextIo::new(&mut bus);
    io.write_str("IRQ echo:\n");
    io.set_interrupts(true);
    // No shared mutable Rust state; foreground never touches TextIO again.
    os::runtime::halt()
}
