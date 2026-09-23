#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

fn main() -> ! {
    let mut bus = unsafe { Mmio::new() };
    let mut gpio = os::gpio::Gpio::new(&mut bus);
    gpio.configure_outputs(1);
    let pwm = os::input::Pwm::new(16, 4).unwrap();
    let mut tick = 0u32;
    loop {
        gpio.write_masked(1, u32::from(pwm.level(tick)));
        tick = tick.wrapping_add(1);
        os::runtime::delay_iterations(50);
    }
}
