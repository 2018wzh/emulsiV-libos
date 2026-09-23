#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

use os::bitmap::{Bitmap, Color, PixelTarget};
fn main() -> ! {
    let mut bus = unsafe { Mmio::new() };
    let mut bitmap = Bitmap::new(&mut bus);
    for y in 0..32 {
        for x in 0..32 {
            // 16 by 16 color swatches, each two pixels wide and high.
            bitmap.pixel(x, y, Color::from_bits(((y / 2) * 16 + x / 2) as u8));
        }
    }
    os::runtime::halt()
}
