//! Behavior checks shared by both execution engines. Every example needs a case.
use crate::{
    image::Report,
    machine::{Rig, Snapshot},
};
use anyhow::{bail, ensure, Result};

fn pixel(s: &Snapshot, x: usize, y: usize) -> u8 {
    s.ram[0xc00 + y * 32 + x]
}
pub fn check(name: &str, rig: &mut impl Rig, build: &Report) -> Result<Snapshot> {
    // Both engines execute the same number of instructions and the same input.
    rig.run(100000)?;
    match name {
        "hello" => ensure!(
            rig.snapshot()?.text == "Hello, emulsiV Rust!\n",
            "hello output"
        ),
        "text_echo" => {
            rig.send("Hello")?;
            ensure!(rig.snapshot()?.text.ends_with("Hello"), "text echo");
        }
        "line_console" => {
            rig.send("Rust;")?;
            ensure!(rig.snapshot()?.text.contains("\n=Rust\n"), "line console");
        }
        "gpio_mirror" => {
            rig.toggle(16)?;
            rig.toggle(31)?;
            rig.run(1000)?;
            ensure!(rig.snapshot()?.gpio_value & 0xffff == 0x8001, "GPIO mirror");
        }
        "gpio_debounce" => {
            rig.toggle(31)?;
            rig.run(5000)?;
            ensure!(rig.snapshot()?.gpio_value & 1 == 1, "debounce rising");
            rig.toggle(31)?;
            rig.run(5000)?;
            rig.toggle(31)?;
            rig.run(5000)?;
            ensure!(rig.snapshot()?.gpio_value & 1 == 0, "debounce repeat");
        }
        "gpio_pwm" => ensure!(rig.snapshot()?.led_levels == 3, "PWM did not change output"),
        "bitmap_palette" => {
            let s = rig.snapshot()?;
            for y in 0..32 {
                for x in 0..32 {
                    ensure!(
                        pixel(&s, x, y) == ((y / 2) * 16 + x / 2) as u8,
                        "palette mismatch {x}/{y}"
                    );
                }
            }
        }
        "bitmap_shapes" => {
            let s = rig.snapshot()?;
            ensure!(
                pixel(&s, 0, 0) == 3 && pixel(&s, 4, 4) == 0x1c && pixel(&s, 16, 7) == 0xfc,
                "shape pixels"
            );
        }
        "bitmap_text" => {
            let s = rig.snapshot()?;
            let fb = &s.ram[0xc00..];
            ensure!(
                fb.iter().filter(|&&p| p == 0x1f).count() > 50
                    && fb.iter().all(|&p| p == 0 || p == 0x1f),
                "font output"
            );
        }
        "mono_sprite" => {
            let s = rig.snapshot()?;
            ensure!(
                pixel(&s, 0, 0) == 3 && pixel(&s, 14, 12) == 0xfc,
                "sprite output"
            );
        }
        "cooperative" => {
            let s = rig.snapshot()?;
            ensure!(
                s.text.contains('.') && s.led_levels == 3,
                "cooperative tasks"
            );
        }
        "event_loop" => {
            rig.send("A ")?;
            let s = rig.snapshot()?;
            ensure!(
                s.text == "A " && s.gpio_value & 1 == 1,
                "event queue behavior"
            );
        }
        "irq_echo" => {
            rig.send(&"Z".repeat(100))?;
            let s = rig.snapshot()?;
            ensure!(
                s.irq_entries == 100
                    && !s.in_irq
                    && s.text == format!("IRQ echo:\n{}", "Z".repeat(100)),
                "100 TextIO IRQ roundtrips"
            );
        }
        "irq_gpio" => {
            for _ in 0..100 {
                rig.toggle(31)?;
                rig.run(500)?;
            }
            let s = rig.snapshot()?;
            ensure!(
                s.irq_entries == 100 && !s.in_irq && s.gpio_events == 0 && s.gpio_value & 1 == 0,
                "100 GPIO IRQ roundtrips"
            );
        }
        "crc_demo" => ensure!(
            rig.snapshot()?.text == "CRC32=0xcbf43926\nCRC16=0x000029b1\n",
            "CRC vectors"
        ),
        "arena_demo" => ensure!(rig.snapshot()?.text == "arena used=16\n", "arena output"),
        "diagnostics" => {
            let s = rig.snapshot()?;
            ensure!(
                s.text.starts_with("image end=0x") && s.text.contains("stack reserved=512\n"),
                "memory diagnostics"
            );
        }
        "random_pixels" => ensure!(
            rig.snapshot()?.ram[0xc00..]
                .iter()
                .filter(|&&p| p != 0)
                .count()
                > 5,
            "random pixels"
        ),
        "paint" => {
            rig.send("4d")?;
            ensure!(pixel(&rig.snapshot()?, 17, 16) == 0xe0, "red brush");
            rig.send("2s")?;
            ensure!(pixel(&rig.snapshot()?, 17, 17) == 0x1c, "green brush");
        }
        "shell" => {
            rig.send("?;w 0x1234;r;c 224;p 31 31 3;")?;
            let s = rig.snapshot()?;
            ensure!(
                s.text.contains("r | w N | c N | p X Y N")
                    && s.text.contains("0x00001234\n")
                    && s.gpio_value & 0xffff == 0x1234,
                "shell commands"
            );
            for i in 0..1024 {
                ensure!(
                    s.ram[0xc00 + i] == if i == 1023 { 3 } else { 224 },
                    "shell drawing"
                );
            }
            rig.send("p 32 0 1;w 4294967296;w 2 3;")?;
            let s = rig.snapshot()?;
            ensure!(
                s.text.ends_with("error\nerror\nerror\n") && s.gpio_value & 0xffff == 0x1234,
                "invalid command rejection"
            );
            rig.send(&format!("w 1{};", " ".repeat(30)))?;
            let s = rig.snapshot()?;
            ensure!(
                s.text.ends_with("overflow\n") && s.gpio_value & 0xffff == 0x1234,
                "whole-line overflow rejection"
            );
            rig.send("w 7;r;")?;
            ensure!(
                rig.snapshot()?.text.ends_with("0x00000007\n"),
                "overflow recovery"
            );
        }
        "heap_box" => {
            let s = rig.snapshot()?;
            let address = s
                .text
                .strip_prefix("box=42 address=0x")
                .and_then(|s| s.strip_suffix('\n'))
                .ok_or_else(|| anyhow::anyhow!("Box output: {}", s.text))?;
            let address = u32::from_str_radix(address, 16)?;
            ensure!(
                address >= build.heap_start && address + 4 <= build.heap_end && address % 16 == 0,
                "Box was not allocated inside heap"
            );
        }
        "heap_vec" => ensure!(
            rig.snapshot()?.text == "vec=grown oom=preserved\n",
            "Vec growth or values"
        ),
        "heap_string" => ensure!(rig.snapshot()?.text == "heap Rust", "String contents"),
        "heap_reuse" => ensure!(
            rig.snapshot()?.text == "reuse=100 free=all\n",
            "heap zeroing/alignment/reuse"
        ),
        "heap_oom" => ensure!(
            rig.snapshot()?.text == "oom=recovered\n",
            "recoverable heap OOM"
        ),
        _ => bail!("no behavior test for example {name}"),
    }
    let s = rig.snapshot()?;
    ensure!(
        s.stack_started && s.observed_stack_bytes <= build.stack_reserved,
        "stack not initialized or out of range"
    );
    ensure!(!s.text.contains("panic"), "{name} panicked: {}", s.text);
    Ok(s)
}
