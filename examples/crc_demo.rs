#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

fn main() -> ! {
    let mut bus = unsafe { Mmio::new() };
    let mut io = os::textio::TextIo::new(&mut bus);
    io.write_str("CRC32=");
    io.write_hex32(os::crc::crc32(b"123456789"));
    io.write_str("\nCRC16=");
    io.write_hex32(u32::from(os::crc::crc16_ccitt(b"123456789")));
    io.put_byte(b'\n');
    os::runtime::halt()
}
