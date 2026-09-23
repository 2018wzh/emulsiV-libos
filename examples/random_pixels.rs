#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

use os::bitmap::{Bitmap, Color, PixelTarget};
fn main() -> ! {
    let mut bus = unsafe { Mmio::new() };
    let mut rng = os::rng::XorShift32::new(42);
    let mut display = Bitmap::new(&mut bus);
    loop {
        let value = rng.next_u32();
        display.pixel(
            (value & 31) as i16,
            ((value >> 5) & 31) as i16,
            Color::from_bits((value >> 10) as u8),
        );
        os::runtime::delay_iterations(100);
    }
}
