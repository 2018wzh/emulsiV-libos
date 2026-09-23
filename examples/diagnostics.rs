#![no_std]
#![no_main]
use emulsiv_libos as os;
use os::bus::Mmio;
os::entry!(main);

fn main() -> ! {
    let mut bus = unsafe { Mmio::new() };
    let memory = os::runtime::memory_info();
    let remaining = os::runtime::stack_remaining();
    let mut io = os::textio::TextIo::new(&mut bus);
    io.write_str("image end=");
    io.write_hex32(memory.image_end as u32);
    io.write_str("\nstack reserved=");
    io.write_u32((memory.stack_top - memory.stack_bottom) as u32);
    io.write_str("\nstack now remaining=");
    io.write_u32(remaining as u32);
    io.put_byte(b'\n');
    os::runtime::halt()
}
