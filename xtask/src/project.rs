//! Workspace operations. Commands use argument arrays, never a shell interpreter.
use crate::image::{self, Elf, Report};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output},
};

pub const TARGET: &str = "riscv32i-unknown-none-elf";
pub const UPSTREAM_PIN: &str = "9e15421cd33511d4d2911fea1ae41cd65f33dae9";
pub const REPOSITORY: &str = "2018wzh/emulsiV-libos";
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Example {
    pub name: String,
    pub features: Vec<String>,
}
pub struct Project {
    pub root: PathBuf,
    pub dist: PathBuf,
}
impl Project {
    pub fn new() -> Result<Self> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .context("workspace root")?
            .canonicalize()?;
        let dist = root.join("dist/preview2");
        fs::create_dir_all(&dist)?;
        Ok(Self { root, dist })
    }
    pub fn output(&self, program: &str, args: &[&str]) -> Result<Output> {
        let output = Command::new(program)
            .args(args)
            .current_dir(&self.root)
            .output()
            .with_context(|| format!("start {program}"))?;
        let mut log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dist.join("commands.log"))?;
        writeln!(log, "$ {program} {}", args.join(" "))?;
        log.write_all(&output.stdout)?;
        log.write_all(&output.stderr)?;
        writeln!(log, "[exit {}]", output.status)?;
        Ok(output)
    }
    pub fn command(&self, program: &str, args: &[&str]) -> Result<String> {
        let output = self.output(program, args)?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        ensure!(
            output.status.success(),
            "{program} {} failed: {}\n{stdout}\n{stderr}",
            args.join(" "),
            output.status
        );
        if !stderr.trim().is_empty() {
            eprint!("{stderr}");
        }
        Ok(stdout.into_owned())
    }
    pub fn cargo(&self, args: &[&str]) -> Result<String> {
        self.command("cargo", args)
    }
    pub fn git(&self, args: &[&str]) -> Result<String> {
        self.command("git", args)
    }
    pub fn head(&self) -> Result<String> {
        Ok(self.git(&["rev-parse", "HEAD"])?.trim().to_owned())
    }
    pub fn clean(&self) -> Result<()> {
        ensure!(
            self.git(&["status", "--porcelain"])?.trim().is_empty(),
            "commit or review all source changes first"
        );
        Ok(())
    }
    pub fn metadata(&self) -> Result<Value> {
        Ok(serde_json::from_str(&self.cargo(&[
            "metadata",
            "--locked",
            "--no-deps",
            "--format-version",
            "1",
        ])?)?)
    }
    pub fn package_metadata(&self) -> Result<Value> {
        let metadata = self.metadata()?;
        metadata["packages"]
            .as_array()
            .context("packages")?
            .iter()
            .find(|p| p["name"] == "emulsiv-libos")
            .cloned()
            .context("emulsiv-libos package")
    }
    pub fn version(&self) -> Result<String> {
        Ok(self.package_metadata()?["version"]
            .as_str()
            .context("version")?
            .to_owned())
    }
    pub fn examples(&self) -> Result<Vec<Example>> {
        let package = self.package_metadata()?;
        let mut examples = Vec::new();
        for target in package["targets"].as_array().context("targets")? {
            if !target["kind"]
                .as_array()
                .context("target kinds")?
                .iter()
                .any(|v| v == "example")
            {
                continue;
            }
            let name = target["name"].as_str().context("example name")?.to_owned();
            let features = target["required-features"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            examples.push(Example { name, features });
        }
        examples.sort_by(|a, b| a.name.cmp(&b.name));
        ensure!(!examples.is_empty(), "no examples");
        Ok(examples)
    }
    pub fn write_json(&self, name: &str, value: &impl Serialize) -> Result<()> {
        fs::write(
            self.dist.join(name),
            serde_json::to_string_pretty(value)? + "\n",
        )?;
        Ok(())
    }
    pub fn build(&self, selection: Option<&str>) -> Result<BTreeMap<String, Report>> {
        let examples = self.examples()?;
        if let Some(name) = selection {
            ensure!(
                examples.iter().any(|e| e.name == name),
                "unknown example {name}"
            );
        }
        let mut reports = BTreeMap::new();
        // An in-progress receipt can never authorize packaging of old firmware.
        self.write_json(
            "verification.json",
            &serde_json::json!({"status":"invalidated by build"}),
        )?;
        for example in examples
            .iter()
            .filter(|e| selection.is_none_or(|n| n == e.name))
        {
            let features = example.features.join(",");
            let mut args = vec![
                "build",
                "--locked",
                "-p",
                "emulsiv-libos",
                "--release",
                "--target",
                TARGET,
                "--example",
                &example.name,
            ];
            if !features.is_empty() {
                args.extend(["--features", &features]);
            }
            self.cargo(&args)?;
            let raw = fs::read(
                self.root
                    .join("target")
                    .join(TARGET)
                    .join("release/examples")
                    .join(&example.name),
            )?;
            let mut elf = Elf::parse(&raw).with_context(|| format!("audit {}", example.name))?;
            elf.report.heap_enabled = example.features.iter().any(|f| f == "heap");
            if elf.report.heap_enabled {
                ensure!(
                    elf.report.heap_capacity >= 32,
                    "{} has no useful heap space",
                    example.name
                );
            }
            let text = image::to_hex(&elf.image)?;
            ensure!(image::from_hex(&text)? == elf.image, "HEX roundtrip failed");
            fs::write(self.dist.join(format!("{}.elf", example.name)), &raw)?;
            fs::write(self.dist.join(format!("{}.hex", example.name)), text)?;
            self.write_json(&format!("{}.json", example.name), &elf.report)?;
            println!(
                "BUILD {}: static={} heap extent={} stack={}",
                example.name,
                elf.report.image_end,
                elf.report.heap_capacity,
                elf.report.stack_reserved
            );
            reports.insert(example.name.clone(), elf.report);
        }
        self.write_json(
            if selection.is_none() {
                "build-report.json"
            } else {
                "selected-build.json"
            },
            &reports,
        )?;
        Ok(reports)
    }
    pub fn reports(&self) -> Result<BTreeMap<String, Report>> {
        Ok(serde_json::from_slice(&fs::read(
            self.dist.join("build-report.json"),
        )?)?)
    }
    pub fn source_files(&self) -> Result<Vec<String>> {
        let out = self.git(&[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ])?;
        let mut files: Vec<_> = out
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        files.sort();
        files.dedup();
        Ok(files)
    }
    pub fn source_hash(&self) -> Result<String> {
        let mut data = Vec::new();
        for file in self.source_files()? {
            let path = self.root.join(&file);
            let metadata = fs::symlink_metadata(&path)?;
            ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "source must be a regular file: {file}"
            );
            let bytes = fs::read(path)?;
            data.extend_from_slice(&(file.len() as u64).to_le_bytes());
            data.extend_from_slice(file.as_bytes());
            data.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            data.extend(bytes);
        }
        Ok(image::hash(&data))
    }
    pub fn upstream(&self) -> PathBuf {
        self.root.parent().unwrap().join(".emulsiv-upstream")
    }
    pub fn check_upstream(&self, path: &Path) -> Result<()> {
        let p = path.to_str().context("non-UTF8 upstream path")?;
        ensure!(
            self.git(&["-C", p, "rev-parse", "HEAD"])?.trim() == UPSTREAM_PIN,
            "unexpected upstream commit"
        );
        ensure!(
            self.git(&["-C", p, "status", "--porcelain"])?
                .trim()
                .is_empty(),
            "upstream checkout must be unmodified"
        );
        Ok(())
    }
    pub fn fetch_upstream(&self) -> Result<()> {
        let path = self.upstream();
        if path.exists() {
            return self.check_upstream(&path);
        }
        let p = path.to_str().context("upstream path")?;
        self.git(&["init", p])?;
        self.git(&[
            "-C",
            p,
            "fetch",
            "--depth",
            "1",
            "https://github.com/ESEO-Tech/emulsiV.git",
            UPSTREAM_PIN,
        ])?;
        self.git(&["-C", p, "checkout", "--detach", "FETCH_HEAD"])?;
        self.check_upstream(&path)
    }
    pub fn host_tests(&self) -> Result<()> {
        for features in [None, Some("format"), Some("heap"), Some("heap,format")] {
            let mut args = vec![
                "test",
                "--locked",
                "-p",
                "emulsiv-libos",
                "--lib",
                "--tests",
            ];
            if let Some(f) = features {
                args.extend(["--features", f]);
            }
            print!("{}", self.cargo(&args)?);
        }
        print!(
            "{}",
            self.cargo(&[
                "test",
                "--locked",
                "-p",
                "emulsiv-libos",
                "--no-default-features",
                "--lib",
                "--tests"
            ])?
        );
        print!("{}", self.cargo(&["test", "--locked", "-p", "xtask"])?);
        print!(
            "{}",
            self.cargo(&[
                "test",
                "--locked",
                "-p",
                "emulsiv-libos",
                "--all-features",
                "--doc"
            ])?
        );
        Ok(())
    }
    pub fn require_origin(&self) -> Result<()> {
        let origin = self.git(&["remote", "get-url", "origin"])?;
        if ![
            format!("https://github.com/{REPOSITORY}.git"),
            format!("https://github.com/{REPOSITORY}"),
            format!("git@github.com:{REPOSITORY}.git"),
        ]
        .contains(&origin.trim().to_owned())
        {
            bail!("refusing unrelated origin");
        }
        Ok(())
    }
}
