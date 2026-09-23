#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

use os::bitmap::{self, Bitmap, Color};
fn main() -> ! {
    let mut bus = unsafe { Mmio::new() };
    let mut b = Bitmap::new(&mut bus);
    b.clear(Color::BLACK);
    bitmap::text(&mut b, 0, 1, "RUST\nLIBOS\n01234567", Color::CYAN);
    os::runtime::halt()
}
