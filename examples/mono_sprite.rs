#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

use os::bitmap::{self, Bitmap, Color, MonoBuffer};
fn main() -> ! {
    let mut bus = unsafe { Mmio::new() };
    let mut mono = MonoBuffer::new();
    let sprite = [0x3c, 0x42, 0xa5, 0x81, 0xa5, 0x99, 0x42, 0x3c];
    let _ = bitmap::blit_mono(&mut mono, 12, 12, 8, 8, &sprite, Color::WHITE, None);
    mono.present(&mut Bitmap::new(&mut bus), Color::YELLOW, Color::BLUE);
    os::runtime::halt()
}
