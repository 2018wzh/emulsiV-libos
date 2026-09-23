#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

use os::{console::{LineEditor,LineEvent},shell::Command,bitmap::{Bitmap,Color,PixelTarget}};
fn execute(bus:&mut Mmio,line:&str) {
    match os::shell::parse(line) {
        Ok(Command::Help) => os::textio::TextIo::new(bus).write_str("r | w N | c N | p X Y N\n"),
        Ok(Command::ReadGpio) => {
            let value = os::gpio::Gpio::new(bus).read();
            let mut io = os::textio::TextIo::new(bus);io.write_hex32(value);io.put_byte(b'\n');
        }
        Ok(Command::WriteGpio(value)) => os::gpio::Gpio::new(bus).write_masked(0xffff,value),
        Ok(Command::Clear(color)) => Bitmap::new(bus).clear(Color::from_bits(color)),
        Ok(Command::Pixel{x,y,color}) => Bitmap::new(bus).pixel(i16::from(x),i16::from(y),Color::from_bits(color)),
        Err(_) => os::textio::TextIo::new(bus).write_str("error\n"),
    }
}
fn main() -> ! {
    let mut bus = unsafe { Mmio::new() };
    let mut editor = LineEditor::<32>::new();
    os::gpio::Gpio::new(&mut bus).set_direction(0xffff_0000);
    os::textio::TextIo::new(&mut bus).write_str("? for help\n> ");
    loop {
        if let Some(byte) = os::textio::TextIo::new(&mut bus).try_read() {
            match editor.feed(byte) {
                LineEvent::Complete => {
                    os::textio::TextIo::new(&mut bus).put_byte(b'\n');
                    execute(&mut bus,editor.line());
                    os::textio::TextIo::new(&mut bus).write_str("> ");
                }
                LineEvent::Echo(b) => os::textio::TextIo::new(&mut bus).put_byte(b),
                LineEvent::Overflow => os::textio::TextIo::new(&mut bus).write_str("\noverflow\n> "),
                LineEvent::Cleared|LineEvent::Cancelled => os::textio::TextIo::new(&mut bus).write_str("\n> "),
                LineEvent::Erase => os::textio::TextIo::new(&mut bus).put_byte(b'<'),
                LineEvent::None => {}
            }
        }
    }
}
