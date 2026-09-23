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
    bitmap::rect(&mut b, 0, 0, 32, 32, Color::BLUE);
    bitmap::line(&mut b, -4, 31, 31, -4, Color::RED);
    bitmap::fill_rect(&mut b, 3, 3, 5, 5, Color::GREEN);
    bitmap::circle(&mut b, 16, 16, 9, Color::YELLOW);
    os::runtime::halt()
}
