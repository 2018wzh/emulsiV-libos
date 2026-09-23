#!/usr/bin/env python3
"""Compile and execute the shared startup/linker with an assembly-only fixture.
This does not compile Rust and does not certify Rust examples or upstream emulsiV.
"""
import json
from pathlib import Path
import random
import shutil
import subprocess
from elf2hex import Elf32, to_hex
from rv32 import Machine

ROOT = Path(__file__).resolve().parents[1]

def main():
    clang = shutil.which("clang")
    linker = [shutil.which("ld.lld")] if shutil.which("ld.lld") else []
    if not linker and shutil.which("rustc"):
        sysroot = Path(subprocess.check_output(["rustc", "--print", "sysroot"], cwd=ROOT, text=True).strip())
        host = next(line.split(": ", 1)[1] for line in subprocess.check_output(["rustc", "-vV"], cwd=ROOT, text=True).splitlines() if line.startswith("host: "))
        bundled = sysroot / "lib" / "rustlib" / host / "bin" / "rust-lld"
        if bundled.is_file():
            linker = [str(bundled), "-flavor", "gnu"]
    if not clang or not linker:
        raise SystemExit("clang and either ld.lld or Rust's bundled rust-lld are required")
    dist=ROOT/"dist";dist.mkdir(exist_ok=True)
    for source,target in (("src/startup.S","startup.o"),("tests/runtime_fixture.S","fixture.o"),("src/memory.S","memory.o")):
        subprocess.run([clang,"--target=riscv32-unknown-elf","-march=rv32i","-mabi=ilp32","-c",source,"-o",str(dist/target)],cwd=ROOT,check=True)
    elfpath=dist/"runtime-fixture.elf"
    subprocess.run([*linker,"-m","elf32lriscv","-T","link.x",str(dist/"startup.o"),str(dist/"fixture.o"),str(dist/"memory.o"),"-o",str(elfpath)],cwd=ROOT,check=True)
    elf=Elf32(elfpath.read_bytes());image,report=elf.audit()
    (dist/"runtime-fixture.hex").write_text(to_hex(image))
    checks=[]
    m=Machine();m.load(image)
    lo,hi=elf.symbols["__bss_start"],elf.symbols["__bss_end"]
    m.ram[lo:hi]=bytes([0xA5])*(hi-lo)
    m.ram[0xC00:]=bytes([0x55])*1024
    m.run_until(lambda cpu: cpu.pc==elf.symbols["fixture_idle"])
    assert m.output==b"R";checks.append("reset vector reaches main and TextIO output")
    assert m.ram[lo:hi]==bytes(hi-lo);checks.append("startup clears poisoned BSS")
    assert m.reg[2]==0xC00 and m.reg[2]%16==0;checks.append("SP starts aligned immediately below framebuffer")
    assert m.ram[0xC00:]==bytes([0x55])*1024;checks.append("startup does not touch framebuffer")
    rng=random.Random(55)
    for iteration in range(100):
        for reg in range(1,32):
            if reg!=2: m.reg[reg]=rng.getrandbits(32)
        before=m.reg.copy();pc=m.pc
        m.text_ctrl=0x80;m.receive(65+(iteration%26));count=m.irq_count
        m.run_until(lambda cpu: cpu.irq_count==count+1 and cpu.interruptible)
        assert m.reg==before, "IRQ wrapper corrupted a register"
        assert m.pc==pc, "MRET did not resume the interrupted PC"
        assert m.text_ctrl==0, "fixture did not acknowledge pending input"
    checks.append("100 IRQ entries preserve all integer registers and return PC")
    assert m.min_sp==0xBC0;checks.append("IRQ wrapper uses exactly 64 bytes in assembly fixture")
    assert m.ram[0xC00:]==bytes([0x55])*1024;checks.append("IRQ fixture leaves framebuffer intact")

    def call_memory(name, a0, a1, a2):
        m.pc=elf.symbols[name];m.reg[1]=elf.symbols["fixture_idle"]
        m.reg[10:13]=[a0,a1,a2]
        m.run_until(lambda cpu: cpu.pc==elf.symbols["fixture_idle"])
        return m.reg[10]
    for n in (0,1,7,16,31):
        m.ram[0x600:0x680]=bytes(range(128))
        original=bytes(m.ram[0x600:0x680])
        assert call_memory("memcpy",0x681,0x601,n)==0x681
        assert m.ram[0x681:0x681+n]==original[1:1+n]
        assert call_memory("memset",0x700,0xAB,n)==0x700
        assert m.ram[0x700:0x700+n]==bytes([0xAB])*n
        for dst,src in ((0x603,0x600),(0x600,0x603),(0x600,0x600)):
            m.ram[0x600:0x680]=original
            expected=bytearray(original)
            expected[dst-0x600:dst-0x600+n]=original[src-0x600:src-0x600+n]
            assert call_memory("memmove",dst,src,n)==dst
            assert m.ram[0x600:0x680]==expected
    m.ram[0x600:0x604]=b"abc\0";m.ram[0x610:0x614]=b"abd\0"
    assert call_memory("memcmp",0x600,0x610,3)==0xFFFFFFFF
    assert call_memory("memcmp",0x610,0x600,3)==1
    assert call_memory("memcmp",0x600,0x610,2)==0
    assert call_memory("memcmp",0,0,0)==0
    checks.append("C ABI memory primitives handle zero lengths, alignment and both overlap directions")
    negative=subprocess.run([*linker,"-m","elf32lriscv","-T","link.x",
        "--defsym=__stack_size_override=3072",str(dist/"startup.o"),str(dist/"fixture.o"),
        "-o",str(dist/"must-not-fit.elf")],cwd=ROOT,capture_output=True,text=True)
    assert negative.returncode!=0 and "budget" in negative.stderr
    checks.append("linker rejects a deliberately oversized RAM reservation")
    subprocess.run([clang,"--target=riscv32-unknown-elf","-march=rv32i","-mabi=ilp32","-c",
        "tests/irq_override.S","-o",str(dist/"override.o")],cwd=ROOT,check=True)
    override_elf=dist/"irq-override.elf"
    subprocess.run([*linker,"-m","elf32lriscv","-T","link.x",str(dist/"startup.o"),
        str(dist/"fixture.o"),str(dist/"override.o"),"-o",str(override_elf)],cwd=ROOT,check=True)
    override=Elf32(override_elf.read_bytes());override_image,_=override.audit()
    cpu=Machine();cpu.load(override_image)
    cpu.run_until(lambda c:c.pc==override.symbols["fixture_idle"])
    cpu.text_ctrl=0x80;cpu.receive(65)
    cpu.run_until(lambda c:c.irq_count==1 and c.interruptible)
    assert cpu.output==b"R!", "strong user IRQ callback was not selected"
    checks.append("strong application IRQ callback overrides linker-provided default")
    report.update(kind="assembly-only runtime verification",checks=checks,irq_roundtrips=100,
                  observed_min_sp=m.min_sp,rust_compilation="not performed by this test",
                  upstream_emulsiv="not executed by this test")
    (dist/"runtime-verification.json").write_text(json.dumps(report,indent=2)+"\n")
    print(json.dumps(report,indent=2))

if __name__=="__main__":
    main()
