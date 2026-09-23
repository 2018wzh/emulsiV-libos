#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

use os::scheduler::{Scheduler,TaskControl};
fn led(_:u32)->TaskControl {
    let mut bus = unsafe { Mmio::new() };
    os::gpio::Gpio::new(&mut bus).toggle(1);
    TaskControl::Continue
}
fn heartbeat(_:u32)->TaskControl {
    let mut bus = unsafe { Mmio::new() };
    os::textio::TextIo::new(&mut bus).put_byte(b'.');
    TaskControl::Continue
}
fn main() -> ! {
    let mut bus = unsafe { Mmio::new() };
    os::gpio::Gpio::new(&mut bus).configure_outputs(1);
    let mut scheduler = Scheduler::<2>::new();
    let _ = scheduler.add(0,50,led);
    let _ = scheduler.add(0,200,heartbeat);
    let mut tick = 0u32;
    loop {
        scheduler.run_ready(tick,2);
        tick = tick.wrapping_add(1);
        os::runtime::delay_iterations(50);
    }
}
