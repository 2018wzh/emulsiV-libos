//! Release gates and archive integrity. No script interpreter is used.
use crate::{
    image::{self, Elf},
    machine::{Machine, Rig},
    project::{Project, REPOSITORY, TARGET, UPSTREAM_PIN},
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Read, Write},
    path::{Component, Path, PathBuf},
};
use zip::{write::FileOptions, CompressionMethod, ZipArchive, ZipWriter};

#[derive(Debug, Serialize, Deserialize)]
pub struct Receipt {
    pub status: String,
    pub source_commit: String,
    pub source_sha256: String,
    pub clean_source: bool,
    pub version: String,
    pub upstream_commit: String,
    pub artifacts: BTreeMap<String, String>,
    pub browser_ui_tested: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub tag: String,
    pub source_commit: String,
    pub files: BTreeMap<String, String>,
}
fn relative(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('\\')
        && Path::new(name)
            .components()
            .all(|p| matches!(p, Component::Normal(_)))
}
fn validate_receipt(
    r: &Receipt,
    head: &str,
    source: &str,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    ensure!(
        r.status == "passed" && r.clean_source,
        "verification did not pass on clean source"
    );
    ensure!(
        r.source_commit == head && r.source_sha256 == source,
        "source changed after verification"
    );
    ensure!(!r.artifacts.is_empty(), "empty verification evidence");
    for (name, digest) in &r.artifacts {
        ensure!(relative(name), "invalid evidence path");
        ensure!(
            files.get(name).is_some_and(|b| image::hash(b) == *digest),
            "missing or modified evidence: {name}"
        );
    }
    Ok(())
}
fn archive(payload: &BTreeMap<String, Vec<u8>>, manifest: &Manifest) -> Result<Vec<u8>> {
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    let options = FileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    for (name, bytes) in payload {
        ensure!(
            relative(name) && name != "manifest.json",
            "invalid archive member"
        );
        z.start_file(name, options)?;
        z.write_all(bytes)?;
    }
    z.start_file("manifest.json", options)?;
    z.write_all((serde_json::to_string_pretty(manifest)? + "\n").as_bytes())?;
    Ok(z.finish()?.into_inner())
}
pub fn verify_archive(bytes: &[u8]) -> Result<Manifest> {
    let mut z = ZipArchive::new(Cursor::new(bytes))?;
    let manifest: Manifest = serde_json::from_reader(z.by_name("manifest.json")?)?;
    ensure!(
        z.len() == manifest.files.len() + 1,
        "archive member count mismatch"
    );
    let mut names = std::collections::BTreeSet::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i)?;
        let name = f.name().to_owned();
        ensure!(
            relative(&name) && names.insert(name.clone()),
            "unsafe or duplicate archive member"
        );
        ensure!(f.size() <= 16 * 1024 * 1024, "archive member is too large");
        let mut data = Vec::new();
        f.read_to_end(&mut data)?;
        if name != "manifest.json" {
            ensure!(
                manifest.files.get(&name) == Some(&image::hash(&data)),
                "archive digest mismatch: {name}"
            );
        }
    }
    Ok(manifest)
}

pub fn crate_check(p: &Project) -> Result<()> {
    let clean = p.git(&["status", "--porcelain"])?.trim().is_empty();
    let mut args = vec!["package", "--locked", "-p", "emulsiv-libos"];
    if !clean {
        args.push("--allow-dirty");
    }
    print!("{}", p.cargo(&args)?);
    let version = p.version()?;
    // Cargo's package verification extracts and builds this exact .crate file.
    let unpacked = p
        .root
        .join("target/package")
        .join(format!("emulsiv-libos-{version}"));
    ensure!(
        unpacked.join("Cargo.toml").is_file(),
        "Cargo did not verify the extracted package"
    );
    let downstream = p.dist.join("downstream");
    fs::create_dir_all(downstream.join("src"))?;
    let manifest = format!(
        r#"[package]
name = "downstream-heap-check"
version = "0.0.0"
edition = "2021"
build = "build.rs"
[workspace]
[dependencies]
emulsiv-libos = {{ path = {:?}, features = ["heap"] }}
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
"#,
        unpacked.to_str().context("package path")?
    );
    fs::write(downstream.join("Cargo.toml"), manifest)?;
    fs::copy(unpacked.join("link.x"), downstream.join("link.x"))?;
    fs::write(
        downstream.join("build.rs"),
        r#"fn main() {
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::copy("link.x", out.join("application.x")).unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rustc-link-arg=-Tapplication.x");
}
"#,
    )?;
    fs::copy(
        unpacked.join("examples/heap_box.rs"),
        downstream.join("src/main.rs"),
    )?;
    let manifest_path = downstream.join("Cargo.toml");
    let m = manifest_path.to_str().unwrap();
    p.cargo(&["generate-lockfile", "--offline", "--manifest-path", m])?;
    p.cargo(&[
        "build",
        "--locked",
        "--offline",
        "--manifest-path",
        m,
        "--release",
        "--target",
        TARGET,
    ])?;
    let elf = Elf::parse(&fs::read(
        downstream
            .join("target")
            .join(TARGET)
            .join("release/downstream-heap-check"),
    )?)?;
    let mut cpu = Machine::new(&elf.image)?;
    cpu.run(100000)?;
    ensure!(
        cpu.text.starts_with("box=42 address=0x") && !cpu.text.contains("panic"),
        "packaged downstream heap application failed"
    );
    let readme = fs::read_to_string(unpacked.join("README.md"))?;
    let snippet = readme
        .split_once("```rust\n")
        .and_then(|(_, tail)| tail.split_once("```"))
        .map(|(code, _)| code)
        .context("README Rust application")?;
    fs::write(downstream.join("src/main.rs"), snippet)?;
    p.cargo(&[
        "build",
        "--locked",
        "--offline",
        "--manifest-path",
        m,
        "--release",
        "--target",
        TARGET,
    ])?;
    let readme_elf = Elf::parse(&fs::read(
        downstream
            .join("target")
            .join(TARGET)
            .join("release/downstream-heap-check"),
    )?)?;
    let mut readme_cpu = Machine::new(&readme_elf.image)?;
    readme_cpu.run(100000)?;
    ensure!(
        readme_cpu.text == "42",
        "README application output mismatch"
    );
    let file = format!("emulsiv-libos-{version}.crate");
    let raw = fs::read(p.root.join("target/package").join(&file))?;
    fs::write(p.dist.join(&file), &raw)?;
    p.write_json("crate-verification.json",&json!({"cargo_package_verified":true,"downstream_target":TARGET,"downstream_heap_box":true,"readme_application_compiled_and_executed":true,"crate_file":file,"crate_sha256":image::hash(&raw),"downstream_static_bytes":elf.report.image_end,"downstream_heap_bytes":elf.report.heap_capacity,"published_to_crates_io":false}))?;
    println!("PASS Cargo package and an independent downstream heap application");
    Ok(())
}

pub fn verify(p: &Project) -> Result<()> {
    fs::write(p.dist.join("commands.log"), b"")?;
    p.write_json("verification.json", &json!({"status":"running"}))?;
    let source = p.source_hash()?;
    let head = p.head()?;
    let clean = p.git(&["status", "--porcelain"])?.trim().is_empty();
    let rust = p.command("rustc", &["--version"])?;
    ensure!(
        rust.starts_with("rustc 1.85.1 "),
        "use the fixed Rust 1.85.1 toolchain"
    );
    p.cargo(&["fmt", "--all", "--", "--check"])?;
    crate::docs::check(p)?;
    p.host_tests()?;
    p.cargo(&[
        "clippy",
        "--locked",
        "-p",
        "emulsiv-libos",
        "--lib",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ])?;
    p.cargo(&[
        "clippy",
        "--locked",
        "-p",
        "xtask",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ])?;
    crate::runtime_check::run(p)?;
    p.build(None)?;
    crate::firmware_tests(p, false, None)?;
    crate::firmware_tests(p, true, None)?;
    crate_check(p)?;
    p.git(&["diff", "--check"])?;
    ensure!(
        p.source_hash()? == source && p.head()? == head,
        "source changed during verification"
    );
    fs::copy(p.dist.join("commands.log"), p.dist.join("verification.log"))?;
    let mut artifacts = BTreeMap::new();
    for example in p.examples()? {
        for ext in ["elf", "hex", "json"] {
            let name = format!("{}.{ext}", example.name);
            artifacts.insert(name.clone(), image::hash(&fs::read(p.dist.join(name))?));
        }
    }
    for name in [
        "build-report.json",
        "runtime-verification.json",
        "reference-verification.json",
        "upstream-verification.json",
        "crate-verification.json",
        "docs-verification.json",
        "verification.log",
    ] {
        artifacts.insert(name.to_owned(), image::hash(&fs::read(p.dist.join(name))?));
    }
    let name = format!("emulsiv-libos-{}.crate", p.version()?);
    artifacts.insert(name.clone(), image::hash(&fs::read(p.dist.join(name))?));
    let receipt = Receipt {
        status: "passed".into(),
        source_commit: head,
        source_sha256: source,
        clean_source: clean,
        version: p.version()?,
        upstream_commit: UPSTREAM_PIN.into(),
        artifacts,
        browser_ui_tested: false,
    };
    p.write_json("verification.json", &receipt)?;
    println!("ALL_RELEASE_GATES_PASSED (clean source: {clean})");
    Ok(())
}

pub fn package(p: &Project) -> Result<PathBuf> {
    p.clean()?;
    let receipt: Receipt = serde_json::from_slice(&fs::read(p.dist.join("verification.json"))?)?;
    let mut evidence = BTreeMap::new();
    for name in receipt.artifacts.keys() {
        ensure!(relative(name), "unsafe evidence path");
        evidence.insert(name.clone(), fs::read(p.dist.join(name))?);
    }
    validate_receipt(&receipt, &p.head()?, &p.source_hash()?, &evidence)?;
    ensure!(
        receipt.version == p.version()? && receipt.upstream_commit == UPSTREAM_PIN,
        "version or upstream mismatch"
    );
    let reports = p.reports()?;
    let examples = p.examples()?;
    ensure!(
        reports.len() == examples.len() && examples.iter().all(|e| reports.contains_key(&e.name)),
        "build coverage mismatch"
    );
    for report_name in ["reference-verification.json", "upstream-verification.json"] {
        let v: Value = serde_json::from_slice(&evidence[report_name])?;
        let runs = v["firmware"].as_object().context("firmware runs")?;
        ensure!(
            runs.len() == examples.len()
                && examples
                    .iter()
                    .all(|e| runs.get(&e.name).is_some_and(|r| r["passed"] == true)),
            "behavior coverage mismatch"
        );
    }
    let mut payload = BTreeMap::new();
    for (name, data) in evidence {
        payload.insert(
            if name.ends_with(".hex") {
                format!("firmware/{name}")
            } else if name.ends_with(".crate") {
                name
            } else {
                format!("reports/{name}")
            },
            data,
        );
    }
    payload.insert(
        "reports/verification.json".into(),
        fs::read(p.dist.join("verification.json"))?,
    );
    for name in [
        "README.md",
        "README.zh-CN.md",
        "LICENSE",
        "CHANGELOG.md",
        "docs/HEAP.md",
        "docs/VERIFICATION.md",
        "docs/ARCHITECTURE.md",
        "docs/PORTING.md",
        "docs/WRITING.md",
        "docs/RELEASE.md",
        "CONTRIBUTING.md",
    ] {
        payload.insert(name.into(), fs::read(p.root.join(name))?);
    }
    let manifest = Manifest {
        tag: format!("v{}", receipt.version),
        source_commit: receipt.source_commit,
        files: payload
            .iter()
            .map(|(n, b)| (n.clone(), image::hash(b)))
            .collect(),
    };
    let raw = archive(&payload, &manifest)?;
    ensure!(
        verify_archive(&raw)?.source_commit == p.head()?,
        "archive source mismatch"
    );
    let path = p
        .dist
        .join(format!("emulsiV-libos-{}-firmware.zip", manifest.tag));
    fs::write(&path, &raw)?;
    fs::write(
        path.with_extension("zip.sha256"),
        format!(
            "{}  {}\n",
            image::hash(&raw),
            path.file_name().unwrap().to_str().unwrap()
        ),
    )?;
    println!("PACKAGE {}\nSHA256 {}", path.display(), image::hash(&raw));
    Ok(path)
}

pub fn publish(p: &Project) -> Result<()> {
    // This publishes a GitHub preview. It does not publish to crates.io.
    let zip = package(p)?;
    p.require_origin()?;
    ensure!(
        p.git(&["branch", "--show-current"])?.trim() == "main",
        "release from main only"
    );
    let login = p.command("gh", &["api", "user", "--jq", ".login"])?;
    ensure!(login.trim() == "2018wzh", "unexpected GitHub account");
    let head = p.head()?;
    let tag = format!("v{}", p.version()?);
    let existing: Value = serde_json::from_str(&p.command(
        "gh",
        &["api", &format!("repos/{REPOSITORY}/releases?per_page=100")],
    )?)?;
    ensure!(
        !existing
            .as_array()
            .context("release list")?
            .iter()
            .any(|r| r["tag_name"] == tag),
        "release exists; refusing to overwrite it"
    );
    ensure!(
        p.git(&["ls-remote", "origin", &format!("refs/tags/{tag}")])?
            .trim()
            .is_empty(),
        "tag exists; refusing to move it"
    );
    p.git(&["push", "--set-upstream", "origin", "main"])?;
    ensure!(
        p.command(
            "gh",
            &[
                "api",
                &format!("repos/{REPOSITORY}/commits/main"),
                "--jq",
                ".sha"
            ]
        )?
        .trim()
            == head,
        "remote main does not match verified source"
    );
    // Let the caller rerun after CI completes. Never create an unverified release.
    let runs: Value = serde_json::from_str(&p.command(
        "gh",
        &[
            "run",
            "list",
            "--repo",
            REPOSITORY,
            "--commit",
            &head,
            "--workflow",
            "ci.yml",
            "--limit",
            "5",
            "--json",
            "databaseId,status,conclusion,url",
        ],
    )?)?;
    let run = runs
        .as_array()
        .context("workflow runs")?
        .first()
        .context("CI has not appeared yet; run cargo xtask release after CI completes")?;
    ensure!(
        run["status"] == "completed" && run["conclusion"] == "success",
        "CI is not successful yet; rerun cargo xtask release after CI completes"
    );
    let checksum = zip.with_extension("zip.sha256");
    let notes = p.root.join("docs/RELEASE.md");
    let crate_file = p.dist.join(format!("emulsiv-libos-{}.crate", p.version()?));
    p.command(
        "gh",
        &[
            "release",
            "create",
            &tag,
            "--repo",
            REPOSITORY,
            "--target",
            &head,
            "--prerelease",
            "--title",
            &format!("emulsiV-libos {tag}"),
            "--notes-file",
            notes.to_str().unwrap(),
            zip.to_str().unwrap(),
            checksum.to_str().unwrap(),
            crate_file.to_str().unwrap(),
        ],
    )?;
    let remote: Value = serde_json::from_str(&p.command(
        "gh",
        &[
            "release",
            "view",
            &tag,
            "--repo",
            REPOSITORY,
            "--json",
            "url,assets,isDraft,targetCommitish",
        ],
    )?)?;
    ensure!(
        remote["isDraft"] == false && remote["targetCommitish"] == head,
        "unexpected remote release state"
    );
    let digest = format!("sha256:{}", image::hash(&fs::read(&zip)?));
    ensure!(
        remote["assets"]
            .as_array()
            .context("release assets")?
            .iter()
            .any(|a| a["name"] == zip.file_name().unwrap().to_str().unwrap()
                && a["state"] == "uploaded"
                && a["digest"] == digest),
        "remote ZIP digest mismatch"
    );
    p.write_json(
        "publication.json",
        &json!({"release":remote,"ci":run,"commit":head,"zip_digest":digest}),
    )?;
    println!("PUBLISHED {}", remote["url"].as_str().unwrap_or(""));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn evidence() -> (Receipt, BTreeMap<String, Vec<u8>>) {
        let files: BTreeMap<String, Vec<u8>> =
            BTreeMap::from([("hello.hex".into(), b"firmware".to_vec())]);
        (
            Receipt {
                status: "passed".into(),
                source_commit: "head".into(),
                source_sha256: "tree".into(),
                clean_source: true,
                version: "1".into(),
                upstream_commit: UPSTREAM_PIN.into(),
                artifacts: files
                    .iter()
                    .map(|(n, b)| (n.clone(), image::hash(b)))
                    .collect(),
                browser_ui_tested: false,
            },
            files,
        )
    }
    #[test]
    fn evidence_matches() {
        let (r, f) = evidence();
        validate_receipt(&r, "head", "tree", &f).unwrap();
    }
    #[test]
    fn stale_commit_is_rejected() {
        let (r, f) = evidence();
        assert!(validate_receipt(&r, "new", "tree", &f).is_err());
    }
    #[test]
    fn stale_source_is_rejected() {
        let (r, f) = evidence();
        assert!(validate_receipt(&r, "head", "new", &f).is_err());
    }
    #[test]
    fn dirty_source_is_rejected() {
        let (mut r, f) = evidence();
        r.clean_source = false;
        assert!(validate_receipt(&r, "head", "tree", &f).is_err());
    }
    #[test]
    fn failed_gate_is_rejected() {
        let (mut r, f) = evidence();
        r.status = "failed".into();
        assert!(validate_receipt(&r, "head", "tree", &f).is_err());
    }
    #[test]
    fn changed_firmware_is_rejected() {
        let (r, mut f) = evidence();
        f.insert("hello.hex".into(), vec![0]);
        assert!(validate_receipt(&r, "head", "tree", &f).is_err());
    }
    #[test]
    fn missing_evidence_is_rejected() {
        let (r, _) = evidence();
        assert!(validate_receipt(&r, "head", "tree", &BTreeMap::new()).is_err());
    }
    #[test]
    fn unsafe_paths_are_rejected() {
        for s in ["../x", "/x", "a/../x", "a\\b", ""] {
            assert!(!relative(s));
        }
    }
    #[test]
    fn zip_roundtrip_and_deterministic_layout() {
        let payload: BTreeMap<String, Vec<u8>> =
            BTreeMap::from([("firmware/hello.hex".into(), b"abc".to_vec())]);
        let m = Manifest {
            tag: "v1".into(),
            source_commit: "head".into(),
            files: payload
                .iter()
                .map(|(n, b)| (n.clone(), image::hash(b)))
                .collect(),
        };
        let a = archive(&payload, &m).unwrap();
        assert_eq!(a, archive(&payload, &m).unwrap());
        assert_eq!(verify_archive(&a).unwrap().source_commit, "head");
    }
    #[test]
    fn incorrect_zip_manifest_is_rejected() {
        let payload = BTreeMap::from([("file".into(), b"abc".to_vec())]);
        let m = Manifest {
            tag: "v1".into(),
            source_commit: "head".into(),
            files: BTreeMap::from([("file".into(), "wrong".into())]),
        };
        assert!(verify_archive(&archive(&payload, &m).unwrap()).is_err());
    }
}
