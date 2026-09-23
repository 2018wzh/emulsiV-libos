//! Assembly ABI verification with rustc's integrated assembler and bundled LLD.
use crate::{
    image::Elf,
    machine::{Machine, Rig},
    project::{Project, TARGET},
};
use anyhow::{ensure, Context, Result};
use serde_json::json;
use std::{fs, path::PathBuf};

fn linker(p: &Project) -> Result<PathBuf> {
    let sysroot = p.command("rustc", &["--print", "sysroot"])?;
    let version = p.command("rustc", &["-vV"])?;
    let host = version
        .lines()
        .find_map(|l| l.strip_prefix("host: "))
        .context("rustc host")?;
    let path = PathBuf::from(sysroot.trim())
        .join("lib/rustlib")
        .join(host)
        .join("bin/rust-lld");
    ensure!(path.is_file(), "missing bundled rust-lld");
    Ok(path)
}
fn assemble(p: &Project, name: &str, extra: bool) -> Result<PathBuf> {
    let mut source = String::from("#![no_std]\ncore::arch::global_asm!(\n");
    let mut files = vec!["src/startup.S", "src/memory.S", "tests/runtime_fixture.S"];
    if extra {
        files.push("tests/irq_override.S");
    }
    for path in files {
        source.push_str(&format!(
            "include_str!({:?}),\n",
            p.root.join(path).to_str().context("source path")?
        ));
    }
    source.push_str(");\n");
    let rs = p.dist.join(format!("{name}.rs"));
    let object = p.dist.join(format!("{name}.o"));
    fs::write(&rs, source)?;
    p.command(
        "rustc",
        &[
            "--edition=2021",
            "--crate-type=lib",
            "--emit=obj",
            "--target",
            TARGET,
            "-C",
            "panic=abort",
            "-C",
            "opt-level=z",
            rs.to_str().context("fixture path")?,
            "-o",
            object.to_str().context("object path")?,
        ],
    )?;
    Ok(object)
}
pub fn run(p: &Project) -> Result<()> {
    let ld = linker(p)?;
    let ld = ld.to_str().context("linker path")?;
    let object = assemble(p, "runtime-fixture", false)?;
    let elf_path = p.dist.join("runtime-fixture.elf");
    p.command(
        ld,
        &[
            "-flavor",
            "gnu",
            "-m",
            "elf32lriscv",
            "-T",
            "link.x",
            object.to_str().unwrap(),
            "-o",
            elf_path.to_str().unwrap(),
        ],
    )?;
    let elf = Elf::parse(&fs::read(elf_path)?)?;
    let mut m = Machine::new(&elf.image)?;
    let (bs, be) = (
        elf.symbols["__bss_start"] as usize,
        elf.symbols["__bss_end"] as usize,
    );
    let idle = elf.symbols["fixture_idle"];
    m.ram[bs..be].fill(0xa5);
    m.ram[0xc00..].fill(0x55);
    m.until(|m| m.pc == idle)?;
    ensure!(
        m.text == "R" && m.ram[bs..be].iter().all(|&v| v == 0),
        "reset and BSS initialization"
    );
    ensure!(
        m.x[2] == 0xc00 && m.ram[0xc00..].iter().all(|&v| v == 0x55),
        "stack initialization and framebuffer guard"
    );
    let mut random = 55u32;
    for round in 0..100 {
        for reg in 1..32 {
            if reg != 2 {
                random ^= random << 13;
                random ^= random >> 17;
                random ^= random << 5;
                m.x[reg] = random;
            }
        }
        let before = m.x;
        let count = m.irqs;
        m.ctrl = 0x80;
        m.receive(b'A' + (round % 26) as u8)?;
        m.until(|m| m.irqs == count + 1 && !m.in_irq)?;
        ensure!(
            m.x == before && m.pc == idle && m.ctrl == 0,
            "IRQ corrupted integer state"
        );
    }
    ensure!(
        m.min_sp == 0xbc0 && m.ram[0xc00..].iter().all(|&v| v == 0x55),
        "IRQ stack size/framebuffer guard"
    );
    fn call(m: &mut Machine, e: &Elf, name: &str, args: [u32; 3]) -> Result<u32> {
        m.pc = e.symbols[name];
        m.x[1] = e.symbols["fixture_idle"];
        m.x[10..13].copy_from_slice(&args);
        m.until(|m| m.pc == e.symbols["fixture_idle"])?;
        Ok(m.x[10])
    }
    for n in [0, 1, 7, 16, 31] {
        let original: Vec<u8> = (0..128).collect();
        m.ram[0x600..0x680].copy_from_slice(&original);
        ensure!(
            call(&mut m, &elf, "memcpy", [0x681, 0x601, n])? == 0x681,
            "memcpy return"
        );
        ensure!(
            m.ram[0x681..0x681 + n as usize] == original[1..1 + n as usize],
            "memcpy bytes"
        );
        ensure!(
            call(&mut m, &elf, "memset", [0x700, 0xab, n])? == 0x700,
            "memset return"
        );
        ensure!(
            m.ram[0x700..0x700 + n as usize].iter().all(|&v| v == 0xab),
            "memset bytes"
        );
        for (dst, src) in [(0x603, 0x600), (0x600, 0x603), (0x600, 0x600)] {
            m.ram[0x600..0x680].copy_from_slice(&original);
            let mut expected = original.clone();
            expected.copy_within(src - 0x600..src - 0x600 + n as usize, dst - 0x600);
            ensure!(
                call(&mut m, &elf, "memmove", [dst as u32, src as u32, n])? == dst as u32,
                "memmove return"
            );
            ensure!(m.ram[0x600..0x680] == expected, "memmove overlap bytes");
        }
    }
    m.ram[0x600..0x604].copy_from_slice(b"abc\0");
    m.ram[0x610..0x614].copy_from_slice(b"abd\0");
    for (a, b, n, value) in [
        (0x600, 0x610, 3, u32::MAX),
        (0x610, 0x600, 3, 1),
        (0x600, 0x610, 2, 0),
        (0, 0, 0, 0),
    ] {
        ensure!(
            call(&mut m, &elf, "memcmp", [a, b, n])? == value,
            "memcmp result"
        );
    }
    let bad = p.dist.join("must-not-fit.elf");
    let out = p.output(
        ld,
        &[
            "-flavor",
            "gnu",
            "-m",
            "elf32lriscv",
            "-T",
            "link.x",
            "--defsym=__stack_size_override=3072",
            object.to_str().unwrap(),
            "-o",
            bad.to_str().unwrap(),
        ],
    )?;
    ensure!(
        !out.status.success() && String::from_utf8_lossy(&out.stderr).contains("budget"),
        "oversized image was not rejected"
    );
    let object = assemble(p, "irq-override", true)?;
    let path = p.dist.join("irq-override.elf");
    p.command(
        ld,
        &[
            "-flavor",
            "gnu",
            "-m",
            "elf32lriscv",
            "-T",
            "link.x",
            object.to_str().unwrap(),
            "-o",
            path.to_str().unwrap(),
        ],
    )?;
    let e = Elf::parse(&fs::read(path)?)?;
    let mut cpu = Machine::new(&e.image)?;
    cpu.until(|m| m.pc == e.symbols["fixture_idle"])?;
    cpu.ctrl = 0x80;
    cpu.receive(65)?;
    cpu.until(|m| m.irqs == 1 && !m.in_irq)?;
    ensure!(cpu.text == "R!", "strong IRQ override not selected");
    p.write_json("runtime-verification.json",&json!({"compiler":"rustc integrated assembler + bundled rust-lld","runner":"Rust RV32I model","irq_roundtrips":100,"irq_frame_bytes":64,"bss_zeroed":true,"fixed_vectors":true,"framebuffer_guard":true,"memory_abi":true,"linker_rejects_oversize":true,"strong_irq_override":true,"elf":elf.report}))?;
    println!("PASS runtime: reset, BSS, 100 IRQ roundtrips, memory ABI, linker guards");
    Ok(())
}
