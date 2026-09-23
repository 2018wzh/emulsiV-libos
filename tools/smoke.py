#!/usr/bin/env python3
"""Smoke-test selected COMPILED Rust HEX images in the independent reference CPU."""
import json
from pathlib import Path
from elf2hex import from_hex
from rv32 import Machine
ROOT=Path(__file__).resolve().parents[1]

def boot(name,steps=100000):
    cpu=Machine();cpu.load(from_hex((ROOT/"dist"/f"{name}.hex").read_text()));cpu.run(steps)
    return cpu

def send(cpu, data):
    for byte in data:
        assert not cpu.text_ctrl & 0x40
        cpu.receive(byte)
        cpu.run_until(lambda c:not c.text_ctrl&0x40)
        cpu.run(10000)

def main():
    checks={}
    cpu=boot("hello");assert cpu.output==b"Hello, emulsiV Rust!\n";checks["hello"]=cpu.steps
    cpu=boot("text_echo");cpu.receive(65);cpu.run_until(lambda c:not c.text_ctrl&0x40)
    cpu.run(1000);assert cpu.output.endswith(b"A");checks["text_echo"]=cpu.steps
    cpu=boot("gpio_mirror");cpu.set_inputs(0xA55A0000);cpu.run(1000)
    assert cpu.read(0xD0000010,4)&0xFFFF==0xA55A;checks["gpio_mirror"]=cpu.steps
    cpu=boot("bitmap_palette")
    for y in range(32):
        for x in range(32):assert cpu.ram[0xC00+y*32+x]==(y//2)*16+x//2
    checks["bitmap_palette"]=cpu.steps
    cpu=boot("irq_echo");cpu.receive(90)
    cpu.run_until(lambda c:c.irq_count>=1 and c.interruptible)
    assert cpu.output.endswith(b"Z");checks["irq_echo"]=cpu.steps
    assert cpu.min_sp is not None and cpu.min_sp>=0xA00
    cpu=boot("irq_gpio");cpu.set_inputs(0x80000000)
    cpu.run_until(lambda c:c.irq_count>=1 and c.interruptible)
    assert cpu.read(0xD0000010,4)&1==1;checks["irq_gpio"]=cpu.steps
    cpu=boot("paint")
    for byte in b"4d":
        cpu.receive(byte);cpu.run_until(lambda c:not c.text_ctrl&0x40);cpu.run(10000)
    assert cpu.ram[0xC00+16*32+17]==0xe0;checks["paint"]=cpu.steps
    cpu=boot("line_console");send(cpu,b"Rust;")
    assert b"=Rust\n" in cpu.output;checks["line_console"]=cpu.steps
    cpu=boot("shell");send(cpu,b"?;w 0x1234;r;c 224;p 31 31 3;")
    assert b"0x00001234\n" in cpu.output
    assert cpu.read(0xD0000010,4)&0xffff==0x1234
    assert all(cpu.ram[0xC00+i]==(3 if i==1023 else 224) for i in range(1024))
    send(cpu,b"p 32 0 1;w 4294967296;w 2 3;")
    assert cpu.output.endswith(b"error\nerror\nerror\n")
    send(cpu,b"w 1"+b" "*30+b";")
    assert cpu.output.endswith(b"overflow\n") and cpu.read(0xD0000010,4)&0xffff==0x1234
    send(cpu,b"w 7;r;");assert cpu.output.endswith(b"0x00000007\n")
    assert cpu.min_sp is not None and cpu.min_sp>=0xA00
    checks["shell"]=cpu.steps
    report={"runner":"independent Python RV32I reference model", "rust_firmware_smoke":checks,
            "shell_observed_stack_bytes":3072-cpu.min_sp,
            "upstream_browser_emulsiv":"not tested by this script; see upstream_smoke.mjs"}
    (ROOT/"dist"/"rust-smoke.json").write_text(json.dumps(report,indent=2)+"\n")
    print(json.dumps(report,indent=2))

if __name__=="__main__":main()
