//! Allocation-free command parsing, independent of the hardware execution layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    Unknown,
    Arguments,
    Number,
    Range,
}

pub fn number(s: &str) -> Result<u32, ParseError> {
    // One byte-prefix match avoids four separate string-prefix calls on RV32I.
    let (digits, radix, shift, limit) = match s.as_bytes() {
        [b'0', b'x' | b'X', rest @ ..] => (rest, 16, 4, u32::MAX >> 4),
        [b'0', b'b' | b'B', rest @ ..] => (rest, 2, 1, u32::MAX >> 1),
        bytes => (bytes, 10, 0, u32::MAX / 10),
    };
    if digits.is_empty() {
        return Err(ParseError::Number);
    }
    let mut value = 0u32;
    for &byte in digits {
        let digit = match byte {
            b'0'..=b'9' => u32::from(byte - b'0'),
            b'a'..=b'f' => u32::from(byte - b'a' + 10),
            b'A'..=b'F' => u32::from(byte - b'A' + 10),
            _ => return Err(ParseError::Number),
        };
        if digit >= radix {
            return Err(ParseError::Number);
        }
        if value > limit {
            return Err(ParseError::Range);
        }
        let scaled = if shift == 0 {
            (value << 3) + (value << 1)
        } else {
            value << shift
        };
        value = scaled.checked_add(digit).ok_or(ParseError::Range)?;
    }
    Ok(value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Help,
    ReadGpio,
    WriteGpio(u32),
    Clear(u8),
    Pixel { x: u8, y: u8, color: u8 },
}

pub fn parse(line: &str) -> Result<Command, ParseError> {
    let mut words = line.split_ascii_whitespace();
    let name = words.next().ok_or(ParseError::Empty)?;
    let (kind, count) = match name {
        "help" | "?" => (0, 0),
        "read" | "r" => (1, 0),
        "write" | "w" => (2, 1),
        "clear" | "c" => (3, 1),
        "pixel" | "p" => (4, 3),
        _ => return Err(ParseError::Unknown),
    };
    // Parse all operands through one shared loop instead of duplicated arms.
    let mut args = [0u32; 3];
    for arg in args.iter_mut().take(count) {
        *arg = number(words.next().ok_or(ParseError::Arguments)?)?;
    }
    if words.next().is_some() {
        return Err(ParseError::Arguments);
    }
    match kind {
        0 => Ok(Command::Help),
        1 => Ok(Command::ReadGpio),
        2 => Ok(Command::WriteGpio(args[0])),
        3 if args[0] <= 255 => Ok(Command::Clear(args[0] as u8)),
        4 if args[0] <= 31 && args[1] <= 31 && args[2] <= 255 => Ok(Command::Pixel {
            x: args[0] as u8,
            y: args[1] as u8,
            color: args[2] as u8,
        }),
        _ => Err(ParseError::Range),
    }
}
