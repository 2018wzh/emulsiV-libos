use emulsiv_libos::{bitmap::Color, shell::{number, parse, Command}};

#[test]
fn native_rgb332_preserves_all_256_values() {
    for bits in 0..=255u8 { assert_eq!(Color::from_bits(bits).bits(), bits); }
    assert_eq!([Color::RED.bits(), Color::GREEN.bits(), Color::BLUE.bits(), Color::WHITE.bits()], [0xe0, 0x1c, 3, 255]);
    assert_eq!(Color::from_rgb888(255, 0, 0), Color::RED);
    assert_eq!(Color::from_rgb888(0, 255, 0), Color::GREEN);
    assert_eq!(Color::from_rgb888(0, 0, 255), Color::BLUE);
    assert_eq!(Color::from_rgb888(255, 255, 255), Color::WHITE);
}

#[test]
fn parser_matches_host_formatting() {
    let mut value = 1u32;
    for _ in 0..4096 {
        value ^= value << 13; value ^= value >> 17; value ^= value << 5;
        for text in [format!("{value}"), format!("0x{value:x}"), format!("0X{value:X}"), format!("0b{value:b}")] {
            assert_eq!(number(&text), Ok(value), "{text}");
        }
    }
}

#[test]
fn parser_boundaries_and_malformed_numbers() {
    for text in ["4294967295", "0XFFFFFFFF", "0B11111111111111111111111111111111"] {
        assert_eq!(number(text), Ok(u32::MAX));
    }
    for text in ["4294967296", "0x100000000", "0b100000000000000000000000000000000", "-0", "+1", "1 2", "１２", "0X", "0b", "0o77"] {
        assert!(number(text).is_err(), "{text}");
    }
}

#[test]
fn shell_aliases_and_argument_validation() {
    assert_eq!(parse(" help \t"), Ok(Command::Help));
    assert_eq!(parse("read"), Ok(Command::ReadGpio));
    assert_eq!(parse("write 0XFFFFFFFF"), Ok(Command::WriteGpio(u32::MAX)));
    assert_eq!(parse("clear 255"), Ok(Command::Clear(255)));
    assert_eq!(parse(" pixel\t31 31 0xE0 "), Ok(Command::Pixel {x:31,y:31,color:224}));
    for text in ["? x", "read 0", "w 1 2", "pixel 31 31", "pixel 0 32 1", "c 256", "p 0 0 256", "p -1 0 1"] {
        assert!(parse(text).is_err(), "{text}");
    }
}

#[test]
fn zero_capacity_editor_rejects_nonempty_line() {
    use emulsiv_libos::console::{LineEditor, LineEvent};
    let mut editor = LineEditor::<0>::new();
    editor.feed(b'x');
    assert_eq!(editor.feed(b'\n'), LineEvent::Overflow);
    assert_eq!(editor.line(), "");
}

#[cfg(feature="format")]
#[test]
fn textio_fmt_write_is_exercised() {
    use std::fmt::Write;
    use emulsiv_libos::bus::{RegisterIo, TEXT_OUT};
    #[derive(Default)] struct Bus(Vec<u8>);
    impl RegisterIo for Bus {
        fn read8(&mut self, _:usize)->u8 { panic!("unexpected read") }
        fn read32(&mut self, _:usize)->u32 { panic!("unexpected read") }
        fn write32(&mut self, _:usize, _:u32) { panic!("unexpected write") }
        fn write8(&mut self, a:usize, v:u8) { assert_eq!(a,TEXT_OUT); self.0.push(v); }
    }
    let mut bus = Bus::default();
    write!(emulsiv_libos::textio::TextIo::new(&mut bus), "{} {:08x}", -42, 0x1234u32).unwrap();
    assert_eq!(bus.0, b"-42 00001234");
}
