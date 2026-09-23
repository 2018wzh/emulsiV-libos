#!/usr/bin/env python3
"""Build selected Rust firmware and fail on unsupported ISA or memory overflow."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
from elf2hex import Elf32, to_hex

ROOT = Path(__file__).resolve().parents[1]
TARGET = "riscv32i-unknown-none-elf"

def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    choice = ap.add_mutually_exclusive_group(required=True)
    choice.add_argument("--example", choices=sorted(p.stem for p in (ROOT/"examples").glob("*.rs")))
    choice.add_argument("--all", action="store_true")
    args = ap.parse_args()
    if not shutil.which("cargo"):
        ap.exit(1, "cargo is unavailable; install the toolchain in rust-toolchain.toml\n")
    names = sorted(p.stem for p in (ROOT/"examples").glob("*.rs")) if args.all else [args.example]
    dist = ROOT / "dist"
    dist.mkdir(exist_ok=True)
    reports, failures = {}, []
    for name in names:
        try:
            subprocess.run(["cargo", "build", "--locked", "--release", "--target", TARGET, "--example", name], cwd=ROOT, check=True)
            elf = ROOT / "target" / TARGET / "release" / "examples" / name
            image, report = Elf32(elf.read_bytes()).audit()
            (dist/f"{name}.hex").write_text(to_hex(image))
            (dist/f"{name}.json").write_text(json.dumps(report, indent=2)+"\n")
            reports[name] = report
            print(f"{name}: image end={report['image_end']}, stack={report['stack_reserved']}, spare={report['free_before_stack']}")
        except (subprocess.CalledProcessError, OSError, ValueError) as exc:
            failures.append(name)
            reports[name] = {"error": str(exc)}
    (dist/"build-report.json").write_text(json.dumps(reports, indent=2)+"\n")
    if failures:
        ap.exit(1, f"Build/audit failed for: {', '.join(failures)}\n")

if __name__ == "__main__":
    main()
