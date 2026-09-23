#!/usr/bin/env python3
"""Independent, strict RV32I/Virgule reference runner, NOT the upstream JS emulator."""
from __future__ import annotations
import argparse
from pathlib import Path
from elf2hex import from_hex, supported, MRET

MASK = 0xFFFFFFFF

def signed(value: int, bits: int = 32) -> int:
    value &= (1 << bits) - 1
    return value - (1 << bits) if value & (1 << (bits - 1)) else value

class Machine:
    def __init__(self) -> None:
        self.ram = bytearray(4096)
        self.reg = [0] * 32
        self.pc = self.mepc = self.steps = 0
        self.interruptible = True
        self.irq_count = 0
        self.text_ctrl = self.text_data = self.input_overruns = 0
        self.output = bytearray()
        self.gpio = [MASK, 0, 0, 0, 0]
        self.inputs = 0
        self.min_sp: int | None = None

    def load(self, image: dict[int, int]) -> None:
        for a, byte in image.items():
            if not 0 <= a < 0xC00:
                raise ValueError("program image outside ordinary RAM")
            self.ram[a] = byte

    def receive(self, byte: int) -> None:
        if not 0 <= byte <= 255:
            raise ValueError("input is one byte")
        self.input_overruns += bool(self.text_ctrl & 0x40)
        self.text_data = byte
        self.text_ctrl |= 0x40

    def set_inputs(self, value: int) -> None:
        value &= MASK
        self.gpio[2] |= ~self.inputs & value & MASK
        self.gpio[3] |= self.inputs & ~value & MASK
        self.inputs = value

    def pending(self) -> bool:
        return self.text_ctrl & 0xC0 == 0xC0 or bool((self.gpio[2] | self.gpio[3]) & self.gpio[1])

    def read(self, a: int, n: int) -> int:
        a &= MASK
        if n not in (1, 2, 4) or a % n:
            raise ValueError(f"misaligned read: {a:x}/{n}")
        if a + n <= len(self.ram):
            return int.from_bytes(self.ram[a:a+n], "little")
        if n == 1 and a in (0xB0000000, 0xB0000001):
            return self.text_ctrl if a == 0xB0000000 else self.text_data
        if 0xD0000000 <= a and a + n <= 0xD0000014:
            idx, shift = (a - 0xD0000000) // 4, (a % 4) * 8
            value = (self.inputs & self.gpio[0]) | (self.gpio[4] & ~self.gpio[0]) if idx == 4 else self.gpio[idx]
            return (value >> shift) & ((1 << (n*8)) - 1)
        raise ValueError(f"unmapped read: {a:08x}/{n}")

    def write(self, a: int, n: int, value: int) -> None:
        a &= MASK
        if n not in (1, 2, 4) or a % n:
            raise ValueError(f"misaligned write: {a:x}/{n}")
        value &= (1 << (n*8)) - 1
        if a + n <= len(self.ram):
            self.ram[a:a+n] = value.to_bytes(n, "little")
            return
        if a == 0xB0000000 and n == 1:
            self.text_ctrl = value & 0xC0
            return
        if a == 0xC0000000 and n == 1:
            self.output.append(value)
            return
        if 0xD0000000 <= a and a + n <= 0xD0000014:
            idx, shift = (a - 0xD0000000) // 4, (a % 4) * 8
            mask = ((1 << (n*8)) - 1) << shift
            self.gpio[idx] = ((self.gpio[idx] & ~mask) | (value << shift)) & MASK
            return
        raise ValueError(f"unmapped write: {a:08x}/{n}")

    def step(self) -> None:
        if self.interruptible and self.pending():
            self.mepc, self.pc, self.interruptible = self.pc, 4, False
            self.irq_count += 1
        if self.pc % 4 or not 0 <= self.pc < 0xC00:
            raise ValueError(f"invalid instruction address: {self.pc:08x}")
        pc, word = self.pc, self.read(self.pc, 4)
        if not supported(word):
            raise ValueError(f"unsupported instruction {word:08x} at {pc:x}")
        op, rd, f3, rs1, rs2, f7 = word & 127, (word >> 7) & 31, (word >> 12) & 7, (word >> 15) & 31, (word >> 20) & 31, word >> 25
        a, b, imm = self.reg[rs1], self.reg[rs2], signed(word >> 20, 12)
        next_pc, result = (pc + 4) & MASK, None
        if word == MRET:
            next_pc, self.interruptible = self.mepc, True
        elif op == 0x37:
            result = word & 0xFFFFF000
        elif op == 0x17:
            result = pc + (word & 0xFFFFF000)
        elif op == 0x6F:
            off = signed(((word >> 31) << 20) | (((word >> 12) & 255) << 12) | (((word >> 20) & 1) << 11) | (((word >> 21) & 1023) << 1), 21)
            result, next_pc = next_pc, (pc + off) & MASK
        elif op == 0x67:
            result, next_pc = next_pc, (a + imm) & MASK & ~1
        elif op == 0x63:
            off = signed(((word >> 31) << 12) | (((word >> 7) & 1) << 11) | (((word >> 25) & 63) << 5) | (((word >> 8) & 15) << 1), 13)
            take = {0: a == b, 1: a != b, 4: signed(a) < signed(b), 5: signed(a) >= signed(b), 6: a < b, 7: a >= b}[f3]
            if take:
                next_pc = (pc + off) & MASK
        elif op == 0x03:
            n = {0: 1, 1: 2, 2: 4, 4: 1, 5: 2}[f3]
            result = self.read(a + imm, n)
            if f3 in (0, 1):
                result = signed(result, n * 8)
        elif op == 0x23:
            off = signed(((word >> 25) << 5) | ((word >> 7) & 31), 12)
            self.write(a + off, {0: 1, 1: 2, 2: 4}[f3], b)
        elif op == 0x13:
            if f3 == 0: result = a + imm
            elif f3 == 1: result = a << (rs2 & 31)
            elif f3 == 2: result = int(signed(a) < imm)
            elif f3 == 3: result = int(a < (imm & MASK))
            elif f3 == 4: result = a ^ imm
            elif f3 == 5: result = signed(a) >> rs2 if f7 == 32 else a >> rs2
            elif f3 == 6: result = a | imm
            elif f3 == 7: result = a & imm
        elif op == 0x33:
            if f3 == 0: result = a - b if f7 == 32 else a + b
            elif f3 == 1: result = a << (b & 31)
            elif f3 == 2: result = int(signed(a) < signed(b))
            elif f3 == 3: result = int(a < b)
            elif f3 == 4: result = a ^ b
            elif f3 == 5: result = signed(a) >> (b & 31) if f7 == 32 else a >> (b & 31)
            elif f3 == 6: result = a | b
            elif f3 == 7: result = a & b
        if result is not None and rd:
            self.reg[rd] = result & MASK
            if rd == 2:
                self.min_sp = self.reg[2] if self.min_sp is None else min(self.min_sp, self.reg[2])
        self.reg[0], self.pc = 0, next_pc
        self.steps += 1

    def run(self, steps: int) -> None:
        for _ in range(steps):
            self.step()

    def run_until(self, predicate, limit: int = 100000) -> None:
        for _ in range(limit):
            if predicate(self):
                return
            self.step()
        if not predicate(self):
            raise TimeoutError(f"condition not reached in {limit} steps")

def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("hex", type=Path)
    ap.add_argument("--steps", type=int, default=100000)
    ap.add_argument("--input", default="")
    ap.add_argument("--boot-steps", type=int, default=10000)
    args = ap.parse_args()
    if args.steps < 0 or args.boot_steps < 0:
        ap.error("step counts must be nonnegative")
    m = Machine()
    try:
        m.load(from_hex(args.hex.read_text()))
        m.run(args.boot_steps)
        for byte in args.input.encode("ascii"):
            m.receive(byte)
            m.run_until(lambda cpu: not cpu.text_ctrl & 0x40)
        m.run(args.steps)
    except (ValueError, OSError, TimeoutError, UnicodeError) as exc:
        ap.exit(1, f"simulation failed: {exc}\n")
    print(m.output.decode("ascii", errors="replace"), end="")
    print(f"\n[reference runner: steps={m.steps}, IRQs={m.irq_count}, min_sp={m.min_sp}, overruns={m.input_overruns}]")

if __name__ == "__main__":
    main()
