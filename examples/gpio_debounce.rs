#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

fn main() -> ! {
    let mut bus = unsafe { Mmio::new() };
    let mut gpio = os::gpio::Gpio::new(&mut bus);
    gpio.configure_outputs(1);
    let mut button = os::input::Debouncer::new(false, 4);
    loop {
        if button.sample(gpio.read() & 0x8000_0000 != 0) == Some(os::input::Edge::Rising) {
            gpio.toggle(1);
        }
        // Four consecutive samples, not a guaranteed duration in milliseconds.
        os::runtime::delay_iterations(100);
    }
}
