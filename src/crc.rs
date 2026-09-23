//! Table-free checksums. Not authentication or cryptography.
pub fn crc16_ccitt(data:&[u8])->u16 {
    let mut crc=0xffffu16;
    for &byte in data {
        crc^=u16::from(byte)<<8;
        for _ in 0..8 {crc=if crc&0x8000!=0 {(crc<<1)^0x1021} else {crc<<1};}
    }
    crc
}
pub fn crc32(data:&[u8])->u32 {
    let mut crc=0xffff_ffffu32;
    for &byte in data {
        crc^=u32::from(byte);
        for _ in 0..8 {crc=if crc&1!=0 {(crc>>1)^0xedb8_8320} else {crc>>1};}
    }
    !crc
}
