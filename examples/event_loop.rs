#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

use os::event::{Event, EventQueue};
fn main() -> ! {
    let mut bus = unsafe { Mmio::new() };
    let mut queue = EventQueue::<4>::new();
    os::gpio::Gpio::new(&mut bus).configure_outputs(1);
    loop {
        if let Some(b) = os::textio::TextIo::new(&mut bus).try_read() {
            // Single owner: one producer followed by a complete drain.
            let _ = queue.push(Event::Text(b));
        }
        while let Some(event) = queue.pop() {
            if let Event::Text(b) = event {
                os::textio::TextIo::new(&mut bus).put_byte(b);
                if b == b' ' {
                    os::gpio::Gpio::new(&mut bus).toggle(1);
                }
            }
        }
    }
}
