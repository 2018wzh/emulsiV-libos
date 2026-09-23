#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

fn main() -> ! {
    let mut storage = [0u8;64];
    let mut arena = os::arena::Arena::new(&mut storage);
    let mut bus = unsafe { Mmio::new() };
    let mut io = os::textio::TextIo::new(&mut bus);
    if let Some(bytes) = arena.allocate(16,4) {
        bytes[..5].copy_from_slice(b"arena");
        io.write_bytes(&bytes[..5]);
        io.write_str(" used="); io.write_u32(arena.used() as u32); io.put_byte(b'\n');
    }
    os::runtime::halt()
}
