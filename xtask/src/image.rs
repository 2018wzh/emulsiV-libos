//! Strict ELF32 and Intel HEX handling. No objcopy or script runtime is used.
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub type Image = BTreeMap<u32, u8>;
pub const RAM_END: u32 = 0xc00;
pub const MRET: u32 = 0x30200073;
pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn supported(w: u32) -> bool {
    let (op, f3, f7) = (w & 127, (w >> 12) & 7, w >> 25);
    if w & 3 != 3 {
        return false;
    }
    match op {
        0x37 | 0x17 | 0x6f => true,
        0x67 => f3 == 0,
        0x63 => matches!(f3, 0 | 1 | 4..=7),
        0x03 => matches!(f3, 0..=2 | 4 | 5),
        0x23 => f3 <= 2,
        0x13 => match f3 {
            1 => f7 == 0,
            5 => matches!(f7, 0 | 32),
            _ => true,
        },
        0x33 => f7 == 0 || f7 == 32 && matches!(f3, 0 | 5),
        _ => w == MRET,
    }
}
fn range(b: &[u8], off: usize, len: usize) -> Result<&[u8]> {
    b.get(off..off.checked_add(len).context("ELF range overflow")?)
        .context("truncated ELF range")
}
fn u16at(b: &[u8], off: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(range(b, off, 2)?.try_into()?))
}
fn u32at(b: &[u8], off: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(range(b, off, 4)?.try_into()?))
}
fn string(b: &[u8], off: u32) -> Result<String> {
    let tail = b.get(off as usize..).context("invalid ELF string offset")?;
    let end = tail
        .iter()
        .position(|&v| v == 0)
        .context("unterminated ELF string")?;
    Ok(std::str::from_utf8(&tail[..end])?.to_owned())
}
#[derive(Debug)]
struct Section {
    name: String,
    kind: u32,
    flags: u32,
    addr: u32,
    off: u32,
    size: u32,
    link: u32,
    ent: u32,
}
#[derive(Debug)]
pub struct Elf {
    pub symbols: BTreeMap<String, u32>,
    pub image: Image,
    pub report: Report,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub elf_sha256: String,
    pub hex_sha256: String,
    pub entry: u32,
    pub load_bytes: usize,
    pub instruction_words: usize,
    pub image_end: u32,
    pub bss_bytes: u32,
    pub stack_reserved: u32,
    pub free_before_stack: u32,
    pub heap_start: u32,
    pub heap_end: u32,
    pub heap_capacity: u32,
    pub heap_enabled: bool,
    pub static_layout_ok: bool,
}
impl Elf {
    pub fn parse(b: &[u8]) -> Result<Self> {
        ensure!(b.len() >= 52, "truncated ELF header");
        ensure!(
            &b[..7] == b"\x7fELF\x01\x01\x01"
                && u16at(b, 16)? == 2
                && u16at(b, 18)? == 243
                && u32at(b, 20)? == 1,
            "need ELF32 little-endian RISC-V executable"
        );
        ensure!(
            u32at(b, 24)? == 0 && u32at(b, 36)? == 0,
            "need entry zero and RV32I/ILP32 flags"
        );
        let (po, so) = (u32at(b, 28)? as usize, u32at(b, 32)? as usize);
        let (pn, sn, ss) = (
            u16at(b, 44)? as usize,
            u16at(b, 48)? as usize,
            u16at(b, 50)? as usize,
        );
        ensure!(
            u16at(b, 40)? == 52
                && u16at(b, 42)? == 32
                && u16at(b, 46)? == 40
                && pn > 0
                && sn > 0
                && ss < sn,
            "invalid ELF tables"
        );
        range(b, po, pn * 32)?;
        range(b, so, sn * 40)?;
        let names = range(
            b,
            u32at(b, so + ss * 40 + 16)? as usize,
            u32at(b, so + ss * 40 + 20)? as usize,
        )?;
        let mut sections = Vec::new();
        for i in 0..sn {
            let h = so + i * 40;
            let s = Section {
                name: string(names, u32at(b, h)?)?,
                kind: u32at(b, h + 4)?,
                flags: u32at(b, h + 8)?,
                addr: u32at(b, h + 12)?,
                off: u32at(b, h + 16)?,
                size: u32at(b, h + 20)?,
                link: u32at(b, h + 24)?,
                ent: u32at(b, h + 36)?,
            };
            if s.kind != 8 {
                range(b, s.off as usize, s.size as usize)?;
            }
            sections.push(s);
        }
        let mut symbols = BTreeMap::new();
        for s in &sections {
            if s.kind != 2 {
                continue;
            }
            ensure!(s.ent == 16 && s.size % 16 == 0, "invalid symbol table size");
            let st = sections
                .get(s.link as usize)
                .context("invalid symbol string table")?;
            ensure!(st.kind == 3, "symbol strings must be SHT_STRTAB");
            let strings = range(b, st.off as usize, st.size as usize)?;
            for i in 0..s.size / 16 {
                let h = s.off as usize + i as usize * 16;
                if u16at(b, h + 14)? != 0 {
                    symbols.insert(string(strings, u32at(b, h)?)?, u32at(b, h + 4)?);
                }
            }
        }
        let sym = |name: &str| {
            symbols
                .get(name)
                .copied()
                .with_context(|| format!("missing linker symbol {name}"))
        };
        let end = sym("__image_end")?;
        let bottom = sym("__stack_bottom")?;
        let top = sym("__stack_top")?;
        let bs = sym("__bss_start")?;
        let be = sym("__bss_end")?;
        let hs = sym("__heap_start")?;
        let he = sym("__heap_end")?;
        ensure!(
            8 <= end && end <= bottom && bottom < top && top == RAM_END,
            "image/stack/framebuffer overlap"
        );
        ensure!(
            bs <= be && be <= end && bs % 4 == 0 && be % 4 == 0,
            "invalid BSS extent"
        );
        ensure!(
            top - bottom >= 128 && (top - bottom) % 16 == 0,
            "invalid stack extent"
        );
        ensure!(
            hs == (end + 15) & !15 && hs <= he && he == bottom,
            "invalid heap extent"
        );
        let vectors: Vec<_> = sections.iter().filter(|s| s.name == ".vectors").collect();
        ensure!(
            vectors.len() == 1 && vectors[0].addr == 0 && vectors[0].size == 8,
            "missing fixed vectors"
        );
        let mut words = 0;
        let mut allocated = Vec::new();
        for s in &sections {
            if s.flags & 2 != 0 && s.size != 0 {
                let limit = s.addr.checked_add(s.size).context("section overflow")?;
                ensure!(
                    limit <= RAM_END,
                    "allocated section outside ordinary RAM: {}",
                    s.name
                );
                if s.name == ".stack" {
                    ensure!(
                        s.kind == 8 && s.addr == bottom && limit == top,
                        "invalid stack section"
                    );
                } else {
                    ensure!(limit <= end, "section exceeds static image");
                }
                for &(lo, hi) in &allocated {
                    ensure!(
                        limit <= lo || s.addr >= hi,
                        "overlapping allocated sections"
                    );
                }
                allocated.push((s.addr, limit));
            }
            if s.flags & 4 != 0 {
                ensure!(
                    s.flags & 2 != 0 && s.kind != 8 && s.addr % 4 == 0 && s.size % 4 == 0,
                    "invalid executable section"
                );
                for i in (0..s.size).step_by(4) {
                    let w = u32at(b, (s.off + i) as usize)?;
                    ensure!(
                        supported(w),
                        "unsupported instruction {w:08x} at {:x}",
                        s.addr + i
                    );
                    words += 1;
                }
            }
        }
        let mut image = Image::new();
        for i in 0..pn {
            let h = po + i * 32;
            if u32at(b, h)? != 1 {
                continue;
            }
            let (off, va, pa, fs, ms) = (
                u32at(b, h + 4)?,
                u32at(b, h + 8)?,
                u32at(b, h + 12)?,
                u32at(b, h + 16)?,
                u32at(b, h + 20)?,
            );
            ensure!(
                va == pa
                    && fs <= ms
                    && pa.checked_add(ms).is_some_and(|n| n <= RAM_END)
                    && pa + fs <= end,
                "invalid PT_LOAD extent"
            );
            for (j, &byte) in range(b, off as usize, fs as usize)?.iter().enumerate() {
                if let Some(old) = image.insert(pa + j as u32, byte) {
                    ensure!(old == byte, "conflicting load segments");
                }
            }
        }
        for addr in [0, 4] {
            let mut w = 0;
            for i in 0..4 {
                w |= u32::from(*image.get(&(addr + i)).context("unloaded vector")?) << (8 * i);
            }
            ensure!(w & 0xfff == 0x6f, "vector must be JAL x0");
        }
        let report = Report {
            elf_sha256: hash(b),
            hex_sha256: hash(to_hex(&image)?.as_bytes()),
            entry: 0,
            load_bytes: image.len(),
            instruction_words: words,
            image_end: end,
            bss_bytes: be - bs,
            stack_reserved: top - bottom,
            free_before_stack: bottom - end,
            heap_start: hs,
            heap_end: he,
            heap_capacity: he - hs,
            heap_enabled: false,
            static_layout_ok: true,
        };
        Ok(Self {
            symbols,
            image,
            report,
        })
    }
}
fn record(addr: u16, kind: u8, payload: &[u8]) -> String {
    let mut b = vec![payload.len() as u8, (addr >> 8) as u8, addr as u8, kind];
    b.extend_from_slice(payload);
    let sum = b.iter().fold(0u8, |a, &v| a.wrapping_add(v));
    b.push(sum.wrapping_neg());
    let mut s = String::from(":");
    for v in b {
        use std::fmt::Write;
        write!(s, "{v:02X}").unwrap();
    }
    s
}
pub fn to_hex(image: &Image) -> Result<String> {
    ensure!(
        image.keys().all(|&a| a < 65536),
        "HEX output exceeds 64 KiB"
    );
    let mut entries = image.iter().peekable();
    let mut output = String::new();
    while let Some((&start, &byte)) = entries.next() {
        let mut chunk = vec![byte];
        while chunk.len() < 16 {
            match entries.peek() {
                Some((&a, &v)) if a == start + chunk.len() as u32 => {
                    chunk.push(v);
                    entries.next();
                }
                _ => break,
            }
        }
        output.push_str(&record(start as u16, 0, &chunk));
        output.push('\n');
    }
    output.push_str(":00000001FF\n");
    Ok(output)
}
pub fn from_hex(text: &str) -> Result<Image> {
    let (mut image, mut base, mut eof) = (Image::new(), 0u32, false);
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        ensure!(
            !eof && line.starts_with(':') && line.len() % 2 == 1,
            "invalid HEX order or framing"
        );
        let mut b = Vec::new();
        for pair in line.as_bytes()[1..].chunks_exact(2) {
            b.push(
                u8::from_str_radix(std::str::from_utf8(pair)?, 16).context("invalid HEX digit")?,
            );
        }
        ensure!(
            b.len() >= 5
                && b.len() == usize::from(b[0]) + 5
                && b.iter().fold(0u8, |a, &v| a.wrapping_add(v)) == 0,
            "invalid HEX length or checksum"
        );
        let addr = u32::from(b[1]) * 256 + u32::from(b[2]);
        let p = &b[4..b.len() - 1];
        match (b[3], p.len(), addr) {
            (0, _, _) => {
                ensure!(addr + p.len() as u32 <= 65536, "HEX segment crossing");
                for (i, &v) in p.iter().enumerate() {
                    let a = base
                        .checked_add(addr)
                        .and_then(|a| a.checked_add(i as u32))
                        .context("HEX address overflow")?;
                    ensure!(image.insert(a, v).is_none(), "duplicate HEX address");
                }
            }
            (1, 0, 0) => eof = true,
            (4, 2, 0) => base = (u32::from(p[0]) * 256 + u32::from(p[1])) << 16,
            _ => bail!("unsupported HEX record"),
        }
    }
    ensure!(eof, "HEX missing EOF");
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn instruction_whitelist() {
        for w in [0x13, 0x6f, 0x8067, MRET, 0x002081b3] {
            assert!(supported(w));
        }
    }
    #[test]
    fn rejects_extensions() {
        for w in [
            0, 1, 0x73, 0x100073, 0x10500073, 0xf, 0x022081b3, 0x1000202f, 0x30001073,
        ] {
            assert!(!supported(w));
        }
    }
    #[test]
    fn empty_roundtrip() {
        assert!(from_hex(&to_hex(&Image::new()).unwrap())
            .unwrap()
            .is_empty());
    }
    #[test]
    fn sparse_roundtrip() {
        let i = Image::from([(0, 3), (31, 55), (65535, 255)]);
        assert_eq!(from_hex(&to_hex(&i).unwrap()).unwrap(), i);
    }
    #[test]
    fn random_roundtrip() {
        let i = (0..3072)
            .filter(|a| a % 7 != 0)
            .map(|a| (a, (a * 137) as u8))
            .collect();
        assert_eq!(from_hex(&to_hex(&i).unwrap()).unwrap(), i);
    }
    #[test]
    fn checksum_rejected() {
        assert!(from_hex(":0100000001FF\n:00000001FF").is_err());
    }
    #[test]
    fn eof_required() {
        assert!(from_hex(":0100000001FE").is_err());
    }
    #[test]
    fn trailing_record_rejected() {
        assert!(from_hex(":00000001FF\n:00000001FF").is_err());
    }
    #[test]
    fn duplicate_rejected() {
        assert!(from_hex(":0100000001FE\n:0100000001FE\n:00000001FF").is_err());
    }
    #[test]
    fn extended_address() {
        assert_eq!(
            from_hex(":020000040001F9\n:0100000001FE\n:00000001FF")
                .unwrap()
                .get(&65536),
            Some(&1)
        );
    }
    #[test]
    fn invalid_digit() {
        assert!(from_hex(":010000000ZFE\n:00000001FF").is_err());
    }
    #[test]
    fn writer_range() {
        assert!(to_hex(&Image::from([(65536, 0)])).is_err());
    }
    #[test]
    fn truncated_elf() {
        for n in 0..60 {
            assert!(Elf::parse(&vec![0; n]).is_err());
        }
    }
    pub fn fixture() -> Vec<u8> {
        fn w(b: &mut [u8], o: usize, v: u32) {
            b[o..o + 4].copy_from_slice(&v.to_le_bytes());
        }
        fn h(b: &mut [u8], o: usize, v: u16) {
            b[o..o + 2].copy_from_slice(&v.to_le_bytes());
        }
        let mut b = vec![0; 2048];
        b[..7].copy_from_slice(b"\x7fELF\x01\x01\x01");
        h(&mut b, 16, 2);
        h(&mut b, 18, 243);
        w(&mut b, 20, 1);
        w(&mut b, 28, 52);
        w(&mut b, 32, 1280);
        h(&mut b, 40, 52);
        h(&mut b, 42, 32);
        h(&mut b, 44, 1);
        h(&mut b, 46, 40);
        h(&mut b, 48, 8);
        h(&mut b, 50, 5);
        for (i, v) in [1, 128, 0, 0, 12, 3072, 7, 4].iter().enumerate() {
            w(&mut b, 52 + i * 4, *v);
        }
        w(&mut b, 128, 0x6f);
        w(&mut b, 132, 0x6f);
        w(&mut b, 136, MRET);
        let names = b"\0.vectors\0.text\0.symtab\0.strtab\0.shstrtab\0.bss\0.stack\0";
        b[1024..1024 + names.len()].copy_from_slice(names);
        let specs = [
            (1, 1, 6, 0, 128, 8, 0, 0),
            (10, 1, 6, 8, 136, 4, 0, 0),
            (16, 2, 0, 0, 512, 128, 4, 16),
            (24, 3, 0, 0, 768, 200, 0, 0),
            (32, 3, 0, 0, 1024, names.len() as u32, 0, 0),
            (42, 8, 3, 12, 0, 4, 0, 0),
            (47, 8, 3, 2560, 0, 512, 0, 0),
        ];
        for (i, (name, kind, flags, addr, off, size, link, ent)) in specs.into_iter().enumerate() {
            let o = 1280 + (i + 1) * 40;
            for (j, v) in [name, kind, flags, addr, off, size, link, 0, 4, ent]
                .iter()
                .enumerate()
            {
                w(&mut b, o + j * 4, *v);
            }
        }
        let sy = [
            ("__image_end", 16),
            ("__stack_bottom", 2560),
            ("__stack_top", 3072),
            ("__bss_start", 12),
            ("__bss_end", 16),
            ("__heap_start", 16),
            ("__heap_end", 2560),
        ];
        let mut off = 1;
        for (i, (name, value)) in sy.into_iter().enumerate() {
            b[768 + off..768 + off + name.len()].copy_from_slice(name.as_bytes());
            w(&mut b, 512 + (i + 1) * 16, off as u32);
            w(&mut b, 512 + (i + 1) * 16 + 4, value);
            h(&mut b, 512 + (i + 1) * 16 + 14, 0xfff1);
            off += name.len() + 1;
        }
        b
    }
    #[test]
    fn valid_elf() {
        let e = Elf::parse(&fixture()).unwrap();
        assert_eq!(e.report.heap_capacity, 2544);
    }
    #[test]
    fn bad_load_address() {
        let mut b = fixture();
        b[64..68].copy_from_slice(&4096u32.to_le_bytes());
        assert!(Elf::parse(&b).is_err());
    }
    #[test]
    fn compressed_flag() {
        let mut b = fixture();
        b[36] = 1;
        assert!(Elf::parse(&b).is_err());
    }
    #[test]
    fn nonzero_entry() {
        let mut b = fixture();
        b[24] = 4;
        assert!(Elf::parse(&b).is_err());
    }
    #[test]
    fn missing_symbols() {
        let mut b = fixture();
        b[1280 + 3 * 40 + 4] = 1;
        assert!(Elf::parse(&b).is_err());
    }
    #[test]
    fn bad_heap_extent() {
        let mut b = fixture();
        b[512 + 6 * 16 + 4] = 17;
        assert!(Elf::parse(&b).is_err());
    }
    #[test]
    fn over_budget() {
        let mut b = fixture();
        b[512 + 16 + 4..512 + 16 + 8].copy_from_slice(&3000u32.to_le_bytes());
        assert!(Elf::parse(&b).is_err());
    }
    #[test]
    fn unsupported_instruction() {
        let mut b = fixture();
        b[136..140].copy_from_slice(&0x73u32.to_le_bytes());
        assert!(Elf::parse(&b).is_err());
    }
    #[test]
    fn truncated_tables() {
        assert!(Elf::parse(&fixture()[..1400]).is_err());
    }
    #[test]
    fn sha256_known_vector() {
        assert_eq!(
            hash(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
