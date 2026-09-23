#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

os::interrupt!(on_interrupt);
fn on_interrupt() {
    let mut bus = unsafe { Mmio::new() };
    let mut gpio = os::gpio::Gpio::new(&mut bus);
    let events = gpio.edges();
    gpio.clear_all_edges();
    if events.rising & 0x8000_0000 != 0 {
        gpio.toggle(1);
    }
}
fn main() -> ! {
    let mut bus = unsafe { Mmio::new() };
    let mut gpio = os::gpio::Gpio::new(&mut bus);
    gpio.configure_outputs(1);
    gpio.clear_all_edges();
    gpio.set_interrupt_mask(0x8000_0000);
    os::runtime::halt()
}
