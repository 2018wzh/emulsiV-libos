//! Independent RV32I/Virgule model. This is not the upstream JavaScript CPU.
use crate::image::{supported, Image, MRET};
use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub text: String,
    pub ram: Vec<u8>,
    pub gpio_value: u32,
    pub gpio_events: u32,
    pub text_ctrl: u8,
    pub irq_entries: u64,
    pub in_irq: bool,
    pub steps: u64,
    pub observed_stack_bytes: u32,
    pub stack_started: bool,
    pub led_levels: u8,
}
pub trait Rig {
    fn run(&mut self, instructions: usize) -> Result<()>;
    fn receive(&mut self, byte: u8) -> Result<()>;
    fn toggle(&mut self, pin: u32) -> Result<()>;
    fn snapshot(&mut self) -> Result<Snapshot>;
    fn send(&mut self, text: &str) -> Result<()> {
        ensure!(text.is_ascii(), "test input must be ASCII");
        for b in text.bytes() {
            self.receive(b)?;
            let mut consumed = false;
            for _ in 0..50 {
                self.run(2000)?;
                if self.snapshot()?.text_ctrl & 0x40 == 0 {
                    consumed = true;
                    break;
                }
            }
            ensure!(consumed, "input consumption timeout");
            self.run(2000)?;
        }
        Ok(())
    }
}
pub struct Machine {
    pub ram: [u8; 4096],
    pub x: [u32; 32],
    pub pc: u32,
    pub mepc: u32,
    pub in_irq: bool,
    pub irqs: u64,
    pub steps: u64,
    pub ctrl: u8,
    pub data: u8,
    pub gpio: [u32; 5],
    pub inputs: u32,
    pub text: String,
    pub min_sp: u32,
    pub stack_started: bool,
    pub led_levels: u8,
}
impl Machine {
    pub fn new(image: &Image) -> Result<Self> {
        let mut m = Self {
            ram: [0; 4096],
            x: [0; 32],
            pc: 0,
            mepc: 0,
            in_irq: false,
            irqs: 0,
            steps: 0,
            ctrl: 0,
            data: 0,
            gpio: [u32::MAX, 0, 0, 0, 0],
            inputs: 0,
            text: String::new(),
            min_sp: 3072,
            stack_started: false,
            led_levels: 0,
        };
        for (&a, &b) in image {
            ensure!(a < 0xc00, "image outside ordinary RAM");
            m.ram[a as usize] = b;
        }
        Ok(m)
    }
    pub fn value(&self) -> u32 {
        self.inputs & self.gpio[0] | self.gpio[4] & !self.gpio[0]
    }
    pub fn set_inputs(&mut self, value: u32) {
        self.gpio[2] |= !self.inputs & value;
        self.gpio[3] |= self.inputs & !value;
        self.inputs = value;
    }
    pub fn pending(&self) -> bool {
        self.ctrl & 0xc0 == 0xc0 || (self.gpio[2] | self.gpio[3]) & self.gpio[1] != 0
    }
    pub fn read(&self, a: u32, n: usize) -> Result<u32> {
        ensure!(
            matches!(n, 1 | 2 | 4) && a % n as u32 == 0,
            "misaligned read {a:08x}/{n}"
        );
        if (a as u64 + n as u64) <= 4096 {
            return Ok(self.ram[a as usize..a as usize + n]
                .iter()
                .enumerate()
                .fold(0, |v, (i, b)| v | (u32::from(*b) << (8 * i))));
        }
        if n == 1 && a == 0xb0000000 {
            return Ok(u32::from(self.ctrl));
        }
        if n == 1 && a == 0xb0000001 {
            return Ok(u32::from(self.data));
        }
        if a >= 0xd0000000 && a as u64 + n as u64 <= 0xd0000014 {
            let i = ((a - 0xd0000000) / 4) as usize;
            let shift = (a % 4) * 8;
            let v = if i == 4 { self.value() } else { self.gpio[i] };
            return Ok((v >> shift) & if n == 4 { u32::MAX } else { (1 << (8 * n)) - 1 });
        }
        bail!("unmapped read {a:08x}/{n}")
    }
    pub fn write(&mut self, a: u32, n: usize, v: u32) -> Result<()> {
        ensure!(
            matches!(n, 1 | 2 | 4) && a % n as u32 == 0,
            "misaligned write {a:08x}/{n}"
        );
        if a as u64 + n as u64 <= 4096 {
            for i in 0..n {
                self.ram[a as usize + i] = (v >> (8 * i)) as u8;
            }
            return Ok(());
        }
        if a == 0xb0000000 && n == 1 {
            self.ctrl = v as u8;
            return Ok(());
        }
        if a == 0xc0000000 && n == 1 {
            let b = v as u8;
            self.text
                .push(if b == 10 || (32..127).contains(&b) || b >= 161 {
                    char::from(b)
                } else {
                    '\u{fffd}'
                });
            return Ok(());
        }
        if a >= 0xd0000000 && a as u64 + n as u64 <= 0xd0000014 {
            let i = ((a - 0xd0000000) / 4) as usize;
            let shift = (a % 4) * 8;
            let mask = if n == 4 {
                u32::MAX
            } else {
                ((1u32 << (8 * n)) - 1) << shift
            };
            self.gpio[i] = (self.gpio[i] & !mask) | ((v << shift) & mask);
            return Ok(());
        }
        bail!("unmapped write {a:08x}/{n}")
    }
    pub fn step(&mut self) -> Result<()> {
        ensure!(
            self.pc < 0xc00 && self.pc % 4 == 0,
            "invalid PC {:08x}",
            self.pc
        );
        let w = self.read(self.pc, 4)?;
        ensure!(
            supported(w),
            "unsupported instruction {w:08x} at {:x}",
            self.pc
        );
        let (op, rd, f, rs1, rs2, f7) = (
            w & 127,
            ((w >> 7) & 31) as usize,
            (w >> 12) & 7,
            ((w >> 15) & 31) as usize,
            ((w >> 20) & 31) as usize,
            w >> 25,
        );
        let (a, b, imm) = (self.x[rs1], self.x[rs2], sign(w >> 20, 12));
        let mut next = self.pc.wrapping_add(4);
        let mut result = None;
        match op {
            0x37 => result = Some(w & 0xfffff000),
            0x17 => result = Some(self.pc.wrapping_add(w & 0xfffff000)),
            0x6f => {
                let off = sign(
                    ((w >> 31) << 20)
                        | (((w >> 12) & 255) << 12)
                        | (((w >> 20) & 1) << 11)
                        | (((w >> 21) & 1023) << 1),
                    21,
                );
                result = Some(next);
                next = self.pc.wrapping_add(off);
            }
            0x67 => {
                result = Some(next);
                next = a.wrapping_add(imm) & !1;
            }
            0x63 => {
                let take = match f {
                    0 => a == b,
                    1 => a != b,
                    4 => (a as i32) < b as i32,
                    5 => (a as i32) >= b as i32,
                    6 => a < b,
                    7 => a >= b,
                    _ => unreachable!(),
                };
                let off = sign(
                    ((w >> 31) << 12)
                        | (((w >> 7) & 1) << 11)
                        | (((w >> 25) & 63) << 5)
                        | (((w >> 8) & 15) << 1),
                    13,
                );
                if take {
                    next = self.pc.wrapping_add(off);
                }
            }
            0x03 => {
                let n = match f {
                    0 | 4 => 1,
                    1 | 5 => 2,
                    2 => 4,
                    _ => unreachable!(),
                };
                let v = self.read(a.wrapping_add(imm), n)?;
                result = Some(if f < 2 { sign(v, (n * 8) as u32) } else { v });
            }
            0x23 => {
                let off = sign(((w >> 25) << 5) | ((w >> 7) & 31), 12);
                self.write(a.wrapping_add(off), 1 << f, b)?;
            }
            0x13 => {
                result = Some(match f {
                    0 => a.wrapping_add(imm),
                    1 => a << (rs2 & 31),
                    2 => u32::from((a as i32) < imm as i32),
                    3 => u32::from(a < imm),
                    4 => a ^ imm,
                    5 => {
                        if f7 == 32 {
                            ((a as i32) >> (rs2 & 31)) as u32
                        } else {
                            a >> (rs2 & 31)
                        }
                    }
                    6 => a | imm,
                    7 => a & imm,
                    _ => unreachable!(),
                })
            }
            0x33 => {
                result = Some(match f {
                    0 => {
                        if f7 == 32 {
                            a.wrapping_sub(b)
                        } else {
                            a.wrapping_add(b)
                        }
                    }
                    1 => a << (b & 31),
                    2 => u32::from((a as i32) < b as i32),
                    3 => u32::from(a < b),
                    4 => a ^ b,
                    5 => {
                        if f7 == 32 {
                            ((a as i32) >> (b & 31)) as u32
                        } else {
                            a >> (b & 31)
                        }
                    }
                    6 => a | b,
                    7 => a & b,
                    _ => unreachable!(),
                })
            }
            _ => ensure!(w == MRET, "invalid SYSTEM instruction"),
        }
        if let Some(v) = result {
            if rd != 0 {
                self.x[rd] = v;
            }
        }
        self.x[0] = 0;
        // Match the official CPU: accept a device IRQ at the end of an instruction.
        if self.pending() && !self.in_irq {
            self.mepc = next;
            next = 4;
            self.in_irq = true;
            self.irqs += 1;
        } else if w == MRET {
            next = self.mepc;
            self.in_irq = false;
        }
        self.pc = next;
        self.steps += 1;
        if self.x[2] == 0xc00 {
            self.stack_started = true;
        }
        if self.stack_started {
            ensure!(
                (0xa00..=0xc00).contains(&self.x[2]) && self.x[2] % 16 == 0,
                "stack escaped reservation: {:x}",
                self.x[2]
            );
            self.min_sp = self.min_sp.min(self.x[2]);
        }
        self.led_levels |= 1 << (self.value() & 1);
        Ok(())
    }
    pub fn until(&mut self, predicate: impl Fn(&Self) -> bool) -> Result<()> {
        for _ in 0..100000 {
            if predicate(self) {
                return Ok(());
            }
            self.step()?;
        }
        ensure!(predicate(self), "instruction budget exhausted");
        Ok(())
    }
}
fn sign(v: u32, bits: u32) -> u32 {
    (((v << (32 - bits)) as i32) >> (32 - bits)) as u32
}
impl Rig for Machine {
    fn run(&mut self, n: usize) -> Result<()> {
        for _ in 0..n {
            self.step()?;
        }
        Ok(())
    }
    fn receive(&mut self, b: u8) -> Result<()> {
        ensure!(self.ctrl & 0x40 == 0, "input latch overwrite");
        self.data = b;
        self.ctrl |= 0x40;
        Ok(())
    }
    fn toggle(&mut self, pin: u32) -> Result<()> {
        ensure!(pin < 32, "invalid GPIO pin");
        self.set_inputs(self.inputs ^ (1 << pin));
        Ok(())
    }
    fn snapshot(&mut self) -> Result<Snapshot> {
        Ok(Snapshot {
            text: self.text.clone(),
            ram: self.ram.to_vec(),
            gpio_value: self.value(),
            gpio_events: self.gpio[2] | self.gpio[3],
            text_ctrl: self.ctrl,
            irq_entries: self.irqs,
            in_irq: self.in_irq,
            steps: self.steps,
            observed_stack_bytes: 3072 - self.min_sp,
            stack_started: self.stack_started,
            led_levels: self.led_levels,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn program(words: &[u32]) -> Machine {
        let i = words
            .iter()
            .enumerate()
            .flat_map(|(a, w)| {
                w.to_le_bytes()
                    .into_iter()
                    .enumerate()
                    .map(move |(j, v)| ((a * 4 + j) as u32, v))
            })
            .collect();
        Machine::new(&i).unwrap()
    }
    #[test]
    fn addition_wrap_and_zero() {
        let mut m = program(&[0xfff00093, 0x00108093, 0xfff00013]);
        m.run(3).unwrap();
        assert_eq!(m.x[1], 0);
        assert_eq!(m.x[0], 0);
    }
    #[test]
    fn jalr_uses_old_source() {
        let mut m = program(&[0x000080e7, 0x13, 0x13]);
        m.x[1] = 8;
        m.step().unwrap();
        assert_eq!((m.x[1], m.pc), (4, 8));
    }
    #[test]
    fn signed_byte_load() {
        let mut m = program(&[0x04000083]);
        m.ram[64] = 0x80;
        m.step().unwrap();
        assert_eq!(m.x[1], 0xffffff80);
    }
    #[test]
    fn signed_shift() {
        let mut m = program(&[0x4010d113]);
        m.x[1] = 0x80000000;
        m.step().unwrap();
        assert_eq!(m.x[2], 0xc0000000);
    }
    #[test]
    fn shift_masks_register() {
        let mut m = program(&[0x002091b3]);
        m.x[1] = 7;
        m.x[2] = 33;
        m.step().unwrap();
        assert_eq!(m.x[3], 14);
    }
    #[test]
    fn signed_unsigned_compare() {
        let mut m = program(&[0x0020a1b3, 0x0020b233]);
        m.x[1] = u32::MAX;
        m.x[2] = 0;
        m.run(2).unwrap();
        assert_eq!((m.x[3], m.x[4]), (1, 0));
    }
    #[test]
    fn rejects_bad_pc() {
        let mut m = program(&[0x13]);
        m.pc = 2;
        assert!(m.step().is_err());
        m.pc = 0xc00;
        assert!(m.step().is_err());
    }
    #[test]
    fn rejects_bad_opcode() {
        assert!(program(&[0x73]).step().is_err());
    }
    #[test]
    fn rejects_unmapped_and_misaligned() {
        let mut m = program(&[]);
        assert!(m.read(1, 4).is_err());
        assert!(m.write(0x1000, 1, 0).is_err());
        assert!(m.read(0xa0000000, 1).is_err());
    }
    #[test]
    fn framebuffer_bounds() {
        let mut m = program(&[]);
        m.write(0xc00, 1, 1).unwrap();
        m.write(0xfff, 1, 255).unwrap();
        assert_eq!(m.ram[4095], 255);
        assert!(m.write(0xfff, 4, 0).is_err());
    }
    #[test]
    fn gpio_direction() {
        let mut m = program(&[]);
        m.gpio[0] = 0xffff0000;
        m.gpio[4] = 0xffff;
        m.set_inputs(0xabcd0000);
        assert_eq!(m.value(), 0xabcdffff);
    }
    #[test]
    fn gpio_events_and_clear() {
        let mut m = program(&[]);
        m.gpio[1] = 1;
        m.toggle(0).unwrap();
        assert!(m.pending());
        assert_eq!(m.gpio[2], 1);
        m.write(0xd0000008, 4, 0).unwrap();
        assert!(!m.pending());
        m.toggle(0).unwrap();
        assert_eq!(m.gpio[3], 1);
    }
    #[test]
    fn gpio_partial_register() {
        let mut m = program(&[]);
        m.write(0xd0000001, 1, 0x55).unwrap();
        assert_eq!(m.gpio[0], 0xffff55ff);
        assert_eq!(m.read(0xd0000001, 1).unwrap(), 0x55);
    }
    #[test]
    fn text_ack_and_irq_enable() {
        let mut m = program(&[]);
        m.ctrl = 0x80;
        m.receive(b'A').unwrap();
        assert!(m.pending());
        assert_eq!(m.read(0xb0000001, 1).unwrap(), 65);
        m.write(0xb0000000, 1, 0x80).unwrap();
        assert!(!m.pending());
    }
    #[test]
    fn detects_text_overwrite() {
        let mut m = program(&[]);
        m.receive(65).unwrap();
        assert!(m.receive(66).is_err());
    }
    #[test]
    fn irq_and_mret() {
        let mut m = program(&[0x13, MRET]);
        m.ctrl = 0xc0;
        m.step().unwrap();
        assert_eq!((m.pc, m.mepc, m.irqs), (4, 4, 1));
        m.ctrl = 0;
        m.step().unwrap();
        assert_eq!(m.pc, 4);
        assert!(!m.in_irq);
    }
    #[test]
    fn sign_extension() {
        assert_eq!(sign(0x800, 12), 0xfffff800);
        assert_eq!(sign(0x7ff, 12), 2047);
    }
}
