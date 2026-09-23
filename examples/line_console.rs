#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

use os::console::{LineEditor, LineEvent};
fn main() -> ! {
    let mut bus = unsafe { Mmio::new() };
    let mut io = os::textio::TextIo::new(&mut bus);
    let mut editor = LineEditor::<32>::new();
    io.write_str("> ");
    loop {
        if let Some(byte) = io.try_read() {
            // Semicolon submits a line even when the browser filters Enter.
            match editor.feed(if byte == b';' { b'\n' } else { byte }) {
                LineEvent::Echo(b) => io.put_byte(b),
                // TextIO is a text area, not a VT100 terminal. Echo a printable
                // edit marker instead of assuming backspace or ANSI support.
                LineEvent::Erase => io.write_str("<"),
                LineEvent::Cleared | LineEvent::Cancelled => io.write_str("\n> "),
                LineEvent::Complete => {
                    io.write_str("\n=");
                    io.write_str(editor.line());
                    io.write_str("\n> ");
                }
                LineEvent::Overflow => io.write_str("\nline too long\n> "),
                LineEvent::None => {}
            }
        }
    }
}
