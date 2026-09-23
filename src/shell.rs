//! Allocation-free command parsing, independent of the hardware execution layer.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ParseError { Empty, Unknown, Arguments, Number, Range }
pub fn number(s:&str)->Result<u32,ParseError> {
    let (digits,radix)=if let Some(s)=s.strip_prefix("0x").or_else(||s.strip_prefix("0X")) {(s,16)}
        else if let Some(s)=s.strip_prefix("0b").or_else(||s.strip_prefix("0B")) {(s,2)} else {(s,10)};
    if digits.is_empty() {return Err(ParseError::Number);}
    let mut value=0u32;
    for byte in digits.bytes() {
        let digit=match byte {b'0'..=b'9'=>u32::from(byte-b'0'),b'a'..=b'f'=>u32::from(byte-b'a'+10),
            b'A'..=b'F'=>u32::from(byte-b'A'+10),_=>return Err(ParseError::Number)};
        if digit>=radix {return Err(ParseError::Number);}
        value=value.checked_mul(radix).and_then(|v|v.checked_add(digit)).ok_or(ParseError::Range)?;
    }
    Ok(value)
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Command { Help, ReadGpio, WriteGpio(u32), Clear(u8), Pixel{x:u8,y:u8,color:u8} }
pub fn parse(line:&str)->Result<Command,ParseError> {
    let mut words=line.split_ascii_whitespace();
    let name=words.next().ok_or(ParseError::Empty)?;
    let command=match name {
        "help"|"?"=>Command::Help,
        "read"|"r"=>Command::ReadGpio,
        "write"|"w"=>Command::WriteGpio(number(words.next().ok_or(ParseError::Arguments)?)?),
        "clear"|"c"=>{
            let n=number(words.next().ok_or(ParseError::Arguments)?)?;
            if n>255 {return Err(ParseError::Range);} Command::Clear(n as u8)
        }
        "pixel"|"p"=>{
            let x=number(words.next().ok_or(ParseError::Arguments)?)?;
            let y=number(words.next().ok_or(ParseError::Arguments)?)?;
            let c=number(words.next().ok_or(ParseError::Arguments)?)?;
            if x>31||y>31||c>255 {return Err(ParseError::Range);}
            Command::Pixel{x:x as u8,y:y as u8,color:c as u8}
        }
        _=>return Err(ParseError::Unknown),
    };
    if words.next().is_some() {return Err(ParseError::Arguments);}
    Ok(command)
}
