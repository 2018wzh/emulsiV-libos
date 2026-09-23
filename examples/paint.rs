#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

use os::bitmap::{Bitmap,Color,PixelTarget};
fn main()->! {
    let mut bus=unsafe {Mmio::new()};
    os::textio::TextIo::new(&mut bus).write_str("WASD paint, 0-7 color, c clear\n");
    os::gpio::Gpio::new(&mut bus).configure_outputs(1);
    Bitmap::new(&mut bus).clear(Color::BLACK);
    let (mut x,mut y,mut color)=(16i16,16i16,Color::WHITE);
    loop {
        if let Some(b)=os::textio::TextIo::new(&mut bus).try_read() {
            match b {
                b'w'=>y=(y-1)&31,b's'=>y=(y+1)&31,
                b'a'=>x=(x-1)&31,b'd'=>x=(x+1)&31,
                b'0'..=b'7'=>{let n=b-b'0';color=Color::from_rgb(n&4!=0,n&2!=0,n&1!=0);},
                b'c'=>Bitmap::new(&mut bus).clear(Color::BLACK),_=>{}
            }
            Bitmap::new(&mut bus).pixel(x,y,color);
            os::gpio::Gpio::new(&mut bus).toggle(1);
        }
    }
}
