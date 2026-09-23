#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

fn main() -> ! {
    let mut bus = unsafe { Mmio::new() };
    let mut gpio = os::gpio::Gpio::new(&mut bus);
    gpio.set_direction(0xffff_0000);
    // Configure pins 16..31 as switches, 0..15 as LEDs in the emulator UI.
    loop { let input = gpio.read(); gpio.write(input >> 16); }
}
