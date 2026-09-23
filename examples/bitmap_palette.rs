#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

use os::bitmap::{Bitmap,Color,PixelTarget};
fn main() -> ! {
    let mut bus = unsafe { Mmio::new() };
    let mut bitmap = Bitmap::new(&mut bus);
    for y in 0..32 { for x in 0..32 {
        bitmap.pixel(x,y,Color::from_bits(((x / 4) as u8) << 5));
    }}
    os::runtime::halt()
}
