#!/usr/bin/env python3
"""Strict ELF32/RV32I audit and Intel HEX export, Python standard library only."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import struct

RAM_END = 0xC00
MRET = 0x30200073

def supported(word: int) -> bool:
    """Virgule computational RV32I instructions plus its sole SYSTEM op, MRET."""
    op, f3, f7 = word & 127, (word >> 12) & 7, word >> 25
    if word & 3 != 3:
        return False
    if op in (0x37, 0x17, 0x6F):
        return True
    if op == 0x67:
        return f3 == 0
    if op == 0x63:
        return f3 in (0, 1, 4, 5, 6, 7)
    if op == 0x03:
        return f3 in (0, 1, 2, 4, 5)
    if op == 0x23:
        return f3 in (0, 1, 2)
    if op == 0x13:
        return f7 == 0 if f3 == 1 else (f7 in (0, 32) if f3 == 5 else True)
    if op == 0x33:
        return f7 == 0 or (f7 == 32 and f3 in (0, 5))
    return word == MRET

class Elf32:
    def __init__(self, raw: bytes):
        self.raw = raw
        if len(raw) < 52:
            raise ValueError("truncated ELF header")
        h = struct.unpack_from("<16sHHIIIIIHHHHHH", raw)
        ident, kind, machine, version, self.entry, phoff, shoff, flags, eh, pe, pn, se, sn, ss = h
        if ident[:7] != b"\x7fELF\x01\x01\x01" or kind != 2 or machine != 243 or version != 1:
            raise ValueError("need ELF32 little-endian RISC-V executable")
        if self.entry != 0 or flags & 0xF:
            raise ValueError("need entry 0, RV32I/ILP32 without compressed, float ABI or RVE flags")
        if eh != 52 or pe != 32 or se != 40 or not pn or not sn or ss >= sn:
            raise ValueError("unsupported ELF table layout")
        self.slice(phoff, pe * pn)
        self.slice(shoff, se * sn)
        self.programs = [struct.unpack_from("<8I", raw, phoff + i * pe) for i in range(pn)]
        headers = [struct.unpack_from("<10I", raw, shoff + i * se) for i in range(sn)]
        names = self.slice(headers[ss][4], headers[ss][5])
        self.sections = []
        for sh in headers:
            name = self.string(names, sh[0])
            if sh[1] != 8:
                self.slice(sh[4], sh[5])
            self.sections.append(dict(name=name, kind=sh[1], flags=sh[2], address=sh[3],
                offset=sh[4], size=sh[5], link=sh[6], entsize=sh[9]))
        self.symbols: dict[str, int] = {}
        for sh in self.sections:
            if sh["kind"] != 2:
                continue
            if sh["entsize"] != 16 or sh["size"] % 16 or sh["link"] >= sn:
                raise ValueError("bad symbol table")
            strings = self.sections[sh["link"]]
            table = self.slice(strings["offset"], strings["size"])
            for off in range(sh["offset"], sh["offset"] + sh["size"], 16):
                name, value, _, _, _, section = struct.unpack_from("<IIIBBH", raw, off)
                if section:
                    self.symbols[self.string(table, name)] = value

    def slice(self, offset: int, size: int) -> bytes:
        if offset < 0 or size < 0 or offset + size > len(self.raw):
            raise ValueError("ELF range outside file")
        return self.raw[offset:offset + size]

    @staticmethod
    def string(table: bytes, index: int) -> str:
        if index >= len(table):
            raise ValueError("bad string index")
        end = table.find(b"\0", index)
        if end < 0:
            raise ValueError("unterminated ELF string")
        return table[index:end].decode("utf-8")

    def audit(self) -> tuple[dict[int, int], dict]:
        sy = self.symbols
        required = ("__image_end", "__stack_bottom", "__stack_top", "__bss_start", "__bss_end")
        if not all(k in sy for k in required):
            raise ValueError("keep the linker symbols required for a memory audit")
        if not (8 <= sy["__image_end"] <= sy["__stack_bottom"] < sy["__stack_top"] == RAM_END):
            raise ValueError("image / stack / framebuffer overlap")
        if not (0 <= sy["__bss_start"] <= sy["__bss_end"] <= sy["__image_end"]):
            raise ValueError("invalid BSS extent")
        stack = sy["__stack_top"] - sy["__stack_bottom"]
        if stack < 128 or stack % 16:
            raise ValueError("invalid stack alignment/size")
        vectors = [s for s in self.sections if s["name"] == ".vectors"]
        if len(vectors) != 1 or vectors[0]["address"] != 0 or vectors[0]["size"] != 8:
            raise ValueError("missing fixed reset/IRQ vectors")
        count = 0
        for section in self.sections:
            if section["flags"] & 2 and section["size"] and section["address"] + section["size"] > RAM_END:
                raise ValueError(f"allocated section outside ordinary RAM: {section['name']}")
            if section["flags"] & 4:
                if section["size"] % 4 or section["address"] % 4:
                    raise ValueError("non-word-aligned executable section")
                for off in range(0, section["size"], 4):
                    word, = struct.unpack_from("<I", self.raw, section["offset"] + off)
                    if not supported(word):
                        raise ValueError(f"unsupported instruction 0x{word:08x} at 0x{section['address']+off:x}")
                    count += 1
        image: dict[int, int] = {}
        for typ, off, va, pa, filesz, memsz, _, _ in self.programs:
            if typ != 1:
                continue
            if va != pa or filesz > memsz or pa + memsz > RAM_END:
                raise ValueError("invalid PT_LOAD address/size, or ROM-style LMA != VMA")
            for i, byte in enumerate(self.slice(off, filesz)):
                a = pa + i
                if a in image and image[a] != byte:
                    raise ValueError("conflicting load segments")
                image[a] = byte
        for a in (0, 4):
            if not all(a + i in image for i in range(4)):
                raise ValueError("vector is not in the loaded image")
            insn = sum(image[a+i] << (8*i) for i in range(4))
            if insn & 0xFFF != 0x06F:
                raise ValueError("vectors must be unconditional JAL x0 instructions")
        report = dict(elf_sha256=hashlib.sha256(self.raw).hexdigest(), entry=self.entry,
            load_bytes=len(image), instruction_words=count, image_end=sy["__image_end"],
            bss_bytes=sy["__bss_end"]-sy["__bss_start"], stack_reserved=stack,
            free_before_stack=sy["__stack_bottom"]-sy["__image_end"], ram_bytes=RAM_END,
            framebuffer_base=RAM_END, static_layout_ok=True, runtime_stack_high_water="not measured")
        return image, report

def record(address: int, kind: int, payload: bytes) -> str:
    data = bytes((len(payload), address >> 8, address & 255, kind)) + payload
    return ":" + (data + bytes((-sum(data) & 255,))).hex().upper()

def to_hex(image: dict[int, int]) -> str:
    addresses = sorted(image)
    if any(a < 0 or a >= 0x10000 or not 0 <= image[a] <= 255 for a in addresses):
        raise ValueError("HEX writer expects byte values in the first 64 KiB")
    lines, i = [], 0
    while i < len(addresses):
        start = addresses[i]
        chunk = bytearray([image[start]])
        i += 1
        while i < len(addresses) and addresses[i] == start + len(chunk) and len(chunk) < 16:
            chunk.append(image[addresses[i]])
            i += 1
        lines.append(record(start, 0, bytes(chunk)))
    return "\n".join(lines + [record(0, 1, b"")]) + "\n"

def from_hex(text: str) -> dict[int, int]:
    image, base, ended = {}, 0, False
    for line in text.splitlines():
        if not line.strip():
            continue
        if ended or not line.startswith(":"):
            raise ValueError("invalid HEX record order")
        try:
            data = bytes.fromhex(line[1:])
        except ValueError as exc:
            raise ValueError("non-hex character") from exc
        if len(data) < 5 or len(data) != data[0] + 5 or sum(data) & 255:
            raise ValueError("bad HEX length/checksum")
        size, high, low, kind = data[:4]
        address = high * 256 + low
        payload = data[4:-1]
        if kind == 0:
            if address + size > 65536:
                raise ValueError("HEX record crosses segment")
            for i, byte in enumerate(payload):
                a = base + address + i
                if a in image:
                    raise ValueError("overlapping HEX records")
                image[a] = byte
        elif kind == 1 and size == 0 and address == 0:
            ended = True
        elif kind == 4 and size == 2 and address == 0:
            base = int.from_bytes(payload, "big") << 16
        else:
            raise ValueError("unsupported HEX record")
    if not ended:
        raise ValueError("HEX is missing EOF")
    return image

def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("elf", type=Path)
    ap.add_argument("-o", "--output", required=True, type=Path)
    ap.add_argument("--report", type=Path)
    args = ap.parse_args()
    try:
        image, report = Elf32(args.elf.read_bytes()).audit()
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(to_hex(image))
        if args.report:
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))
    except (ValueError, OSError, struct.error) as exc:
        ap.exit(1, f"audit failed: {exc}\n")

if __name__ == "__main__":
    main()
