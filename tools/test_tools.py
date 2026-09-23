"""Executable tests for the independent reference runner and ELF/HEX toolchain."""
import random
import struct
import unittest
from elf2hex import Elf32, from_hex, to_hex, record, supported
from rv32 import Machine, signed


def tiny_elf(word=0x0000006F):
    shnames = b"\0.vectors\0.text\0.symtab\0.strtab\0.shstrtab\0.stack\0"
    symbols = {"__image_end": 12, "__stack_bottom": 0xA00, "__stack_top": 0xC00,
               "__bss_start": 12, "__bss_end": 12}
    strings, symdata = bytearray(b"\0"), bytearray(16)
    for name, value in symbols.items():
        index = len(strings)
        strings.extend(name.encode() + b"\0")
        symdata.extend(struct.pack("<IIIBBH", index, value, 0, 0x10, 0, 0xFFF1))
    codeoff, symoff = 0x100, 0x10C
    stroff, shstroff = symoff + len(symdata), symoff + len(symdata) + len(strings)
    shoff = (shstroff + len(shnames) + 3) & ~3
    raw = bytearray(shoff + 7 * 40)
    ident = b"\x7fELF\x01\x01\x01" + bytes(9)
    struct.pack_into("<16sHHIIIIIHHHHHH", raw, 0, ident, 2, 243, 1, 0, 52, shoff, 0, 52, 32, 1, 40, 7, 5)
    struct.pack_into("<8I", raw, 52, 1, codeoff, 0, 0, 12, 12, 5, 4)
    struct.pack_into("<3I", raw, codeoff, 0x0080006F, 0x0000006F, word)
    raw[symoff:symoff+len(symdata)] = symdata
    raw[stroff:stroff+len(strings)] = strings
    raw[shstroff:shstroff+len(shnames)] = shnames
    def section(i, name, kind, flags, addr, off, size, link=0, entsize=0):
        struct.pack_into("<10I", raw, shoff+i*40, shnames.index(name.encode()+b"\0"), kind, flags,
                         addr, off, size, link, 0, 4, entsize)
    section(1,".vectors",1,6,0,codeoff,8)
    section(2,".text",1,6,8,codeoff+8,4)
    section(3,".symtab",2,0,0,symoff,len(symdata),4,16)
    section(4,".strtab",3,0,0,stroff,len(strings))
    section(5,".shstrtab",3,0,0,shstroff,len(shnames))
    section(6,".stack",8,3,0xA00,0,512)
    return bytes(raw)


def i_insn(op, rd, f3, rs1, imm):
    return ((imm & 4095) << 20) | (rs1 << 15) | (f3 << 12) | (rd << 7) | op


def one(word, a=0, b=0):
    m = Machine()
    m.reg[1], m.reg[2] = a & 0xFFFFFFFF, b & 0xFFFFFFFF
    m.write(0, 4, word)
    m.step()
    return m


class HexTests(unittest.TestCase):
    def test_empty_roundtrip(self):
        self.assertEqual(from_hex(to_hex({})), {})

    def test_sparse_roundtrip(self):
        image = {0: 0xAA, 1: 0x55, 0xBFF: 0xFF}
        self.assertEqual(from_hex(to_hex(image)), image)

    def test_random_roundtrip(self):
        rng = random.Random(2026)
        for _ in range(40):
            image = {a: rng.randrange(256) for a in rng.sample(range(3072), 80)}
            self.assertEqual(from_hex(to_hex(image)), image)

    def test_checksum_rejected(self):
        with self.assertRaises(ValueError):
            from_hex(":0100000001FF\n:00000001FF\n")

    def test_eof_required(self):
        with self.assertRaises(ValueError):
            from_hex(record(0,0,b"a"))

    def test_records_after_eof_rejected(self):
        with self.assertRaises(ValueError):
            from_hex(":00000001FF\n:00000001FF\n")

    def test_overlap_rejected(self):
        with self.assertRaises(ValueError):
            from_hex(record(0,0,b"a")+"\n"+record(0,0,b"a")+"\n:00000001FF\n")

    def test_extended_address(self):
        text = record(0,4,b"\x00\x01")+"\n"+record(2,0,b"a")+"\n:00000001FF\n"
        self.assertEqual(from_hex(text), {0x10002: 97})

    def test_writer_range(self):
        for image in ({-1:0},{65536:0},{0:256}):
            with self.assertRaises(ValueError):
                to_hex(image)


class ElfTests(unittest.TestCase):
    def test_valid(self):
        image, report = Elf32(tiny_elf()).audit()
        self.assertEqual(len(image), 12)
        self.assertEqual(report["stack_reserved"], 512)
        self.assertEqual(report["free_before_stack"], 2548)

    def test_reject_unsupported_instruction(self):
        for word in (0x00000073,0x00100073,0x0000100F,0x02000033,0x30001073,0x10500073,0x00000001):
            with self.subTest(word=word), self.assertRaises(ValueError):
                Elf32(tiny_elf(word)).audit()

    def test_reject_compressed_flag(self):
        raw=bytearray(tiny_elf());struct.pack_into("<I",raw,36,1)
        with self.assertRaises(ValueError): Elf32(bytes(raw))

    def test_reject_nonzero_entry(self):
        raw=bytearray(tiny_elf());struct.pack_into("<I",raw,24,8)
        with self.assertRaises(ValueError): Elf32(bytes(raw))

    def test_reject_over_budget(self):
        elf=Elf32(tiny_elf());elf.symbols["__image_end"]=0xB00
        with self.assertRaises(ValueError): elf.audit()

    def test_reject_missing_symbols(self):
        elf=Elf32(tiny_elf());elf.symbols.clear()
        with self.assertRaises(ValueError): elf.audit()

    def test_reject_bad_load_address(self):
        raw=bytearray(tiny_elf());struct.pack_into("<I",raw,52+12,4)
        with self.assertRaises(ValueError): Elf32(bytes(raw)).audit()

    def test_truncated_headers(self):
        raw=tiny_elf()
        for n in (0,1,4,51,52,70,len(raw)-1):
            with self.subTest(size=n), self.assertRaises(ValueError): Elf32(raw[:n])


class IsaTests(unittest.TestCase):
    def test_basic_whitelist(self):
        for word in (0x00000013,0x30200073,0x0000006F,0x00000037):
            self.assertTrue(supported(word))
        for word in (0,0x00000073,0x00100073,0x02000033,0x30001073,0x10500073,0x0000000F,0x0000100F,0x0100000F):
            self.assertFalse(supported(word))

    def test_sign_extension(self):
        self.assertEqual(signed(0x80,8),-128)
        self.assertEqual(signed(0x7FFFFFFF),2147483647)
        self.assertEqual(signed(0xFFFFFFFF),-1)

    def test_addi_wrap_and_x0(self):
        m=one(i_insn(0x13,3,0,1,1),0xFFFFFFFF)
        self.assertEqual(m.reg[3],0)
        m=one(i_insn(0x13,0,0,1,1),12)
        self.assertEqual(m.reg[0],0)

    def test_signed_unsigned_comparison(self):
        self.assertEqual(one(i_insn(0x13,3,2,1,1),0xFFFFFFFF).reg[3],1)
        self.assertEqual(one(i_insn(0x13,3,3,1,1),0xFFFFFFFF).reg[3],0)

    def test_shift_register_mask(self):
        word=(2<<20)|(1<<15)|(1<<12)|(3<<7)|0x33
        self.assertEqual(one(word,1,33).reg[3],2)

    def test_arithmetic_right_shift(self):
        word=i_insn(0x13,3,5,1,(32<<5)|4)
        self.assertEqual(one(word,0x80000000).reg[3],0xF8000000)

    def test_signed_byte_load(self):
        m=Machine();m.write(0,4,i_insn(3,3,0,1,0));m.reg[1]=100;m.ram[100]=0xFF;m.step()
        self.assertEqual(m.reg[3],0xFFFFFFFF)

    def test_jalr_uses_old_source(self):
        m=one(i_insn(0x67,1,0,1,4),8)
        self.assertEqual(m.pc,12);self.assertEqual(m.reg[1],4)

    def test_pc_alignment(self):
        m=Machine();m.pc=2
        with self.assertRaises(ValueError): m.step()

    def test_invalid_opcode(self):
        with self.assertRaises(ValueError): one(0x00000073)

    def test_alignment_and_unmapped_io(self):
        m=Machine()
        for a,n in ((1,4),(3,2),(0xA0000000,1)):
            with self.assertRaises(ValueError): m.read(a,n)
            with self.assertRaises(ValueError): m.write(a,n,0)


class PeripheralTests(unittest.TestCase):
    def test_text_latch_overwrite_and_ack(self):
        m=Machine();m.receive(65);m.receive(66)
        self.assertEqual(m.input_overruns,1);self.assertEqual(m.read(0xB0000001,1),66)
        m.write(0xB0000000,1,0x80);self.assertFalse(m.pending())
        m.receive(67);self.assertTrue(m.pending())

    def test_gpio_rw_not_w1c(self):
        m=Machine();m.set_inputs(3);self.assertEqual(m.gpio[2],3)
        m.write(0xD0000008,4,2);self.assertEqual(m.gpio[2],2)
        m.write(0xD0000008,4,0);self.assertEqual(m.gpio[2],0)

    def test_gpio_value_direction(self):
        m=Machine();m.write(0xD0000000,4,0xFFFFFFFE);m.set_inputs(0x80000000);m.write(0xD0000010,4,1)
        self.assertEqual(m.read(0xD0000010,4),0x80000001)

    def test_gpio_edges_and_irq(self):
        m=Machine();m.set_inputs(1);m.set_inputs(0);m.write(0xD0000004,4,1)
        self.assertEqual(m.gpio[2:4],[1,1]);self.assertTrue(m.pending())
        m.write(0xD0000008,4,0);m.write(0xD000000C,4,0);self.assertFalse(m.pending())

    def test_framebuffer_exact_range(self):
        m=Machine();m.write(0xFFF,1,0xE0);self.assertEqual(m.read(0xFFF,1),0xE0)
        with self.assertRaises(ValueError): m.write(0x1000,1,0)

    def test_gpio_partial_access(self):
        m=Machine();m.write(0xD0000000,1,0xAA)
        self.assertEqual(m.read(0xD0000000,4),0xFFFFFFAA)

    def test_interrupt_mepc_and_mret(self):
        m=Machine();m.write(4,4,0x30200073);m.pc=8;m.text_ctrl=0xC0
        m.step();self.assertEqual(m.mepc,8);self.assertEqual(m.pc,8)
        self.assertTrue(m.interruptible);self.assertEqual(m.irq_count,1)

if __name__ == "__main__":
    unittest.main()
