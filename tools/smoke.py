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

def main():
    checks={}
    cpu=boot("hello");assert cpu.output==b"Hello, emulsiV Rust!\n";checks["hello"]=cpu.steps
    cpu=boot("text_echo");cpu.receive(65);cpu.run_until(lambda c:not c.text_ctrl&0x40)
    cpu.run(1000);assert cpu.output.endswith(b"A");checks["text_echo"]=cpu.steps
    cpu=boot("gpio_mirror");cpu.set_inputs(0xA55A0000);cpu.run(1000)
    assert cpu.read(0xD0000010,4)&0xFFFF==0xA55A;checks["gpio_mirror"]=cpu.steps
    cpu=boot("bitmap_palette")
    for y in range(32):
        for x in range(32):assert cpu.ram[0xC00+y*32+x]==(x//4)<<5
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
    assert cpu.ram[0xC00+16*32+17]==0x80;checks["paint"]=cpu.steps
    report={"runner":"independent Python RV32I reference model", "rust_firmware_smoke":checks,
            "upstream_browser_emulsiv":"not tested by this script"}
    (ROOT/"dist"/"rust-smoke.json").write_text(json.dumps(report,indent=2)+"\n")
    print(json.dumps(report,indent=2))

if __name__=="__main__":main()
