#!/usr/bin/env python3
"""Package verified HEX images and reports, with per-file and ZIP SHA256 hashes."""
import hashlib
import json
from pathlib import Path
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[1]
DIST = ROOT / "dist"
TAG = "v0.1.0-preview.1"

def main():
    log = (DIST / "verification.log").read_text()
    if not log.rstrip().endswith("ALL_RELEASE_GATES_PASSED"):
        raise SystemExit("Run a successful tools/verify.sh before packaging")
    report = json.loads((DIST / "build-report.json").read_text())
    names = sorted(p.stem for p in (ROOT / "examples").glob("*.rs"))
    if set(report) != set(names) or not all(report[n].get("static_layout_ok") for n in names):
        raise SystemExit("Build report does not cover every example successfully")
    source = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if f"SOURCE_COMMIT={source}" not in log.splitlines():
        raise SystemExit("Verification log belongs to a different source commit; rerun tools/verify.sh")
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip():
        raise SystemExit("Commit/review source changes before packaging")
    payload = {f"firmware/{n}.hex":(DIST/f"{n}.hex").read_bytes() for n in names}
    for filename in ["build-report.json", "runtime-verification.json", "rust-smoke.json", "upstream-smoke.json", "upstream-verification.json", "verification.log"]:
        payload[f"reports/{filename}"] = (DIST/filename).read_bytes()
    payload["VERIFICATION.md"] = (ROOT/"docs/VERIFICATION.md").read_bytes()
    payload["README.md"] = (ROOT/"README.md").read_bytes()
    manifest = {"tag":TAG,"source_commit":source,"files":{n:hashlib.sha256(v).hexdigest() for n,v in payload.items()}}
    payload["manifest.json"] = (json.dumps(manifest,indent=2)+"\n").encode()
    path = DIST/f"emulsiV-libos-{TAG}-firmware.zip"
    with zipfile.ZipFile(path,"w",compression=zipfile.ZIP_DEFLATED) as archive:
        for name,data in sorted(payload.items()):
            info=zipfile.ZipInfo(name,date_time=(2026,9,23,0,0,0))
            info.compress_type=zipfile.ZIP_DEFLATED
            info.external_attr=0o100644<<16
            archive.writestr(info,data)
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    checksum=path.with_suffix(path.suffix+".sha256")
    checksum.write_text(f"{digest}  {path.name}\n")
    print(json.dumps({"zip":str(path),"sha256":digest,"commit":source,"firmware_count":len(names)},indent=2))

if __name__=="__main__":main()
