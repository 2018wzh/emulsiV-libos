mod docs;
mod image;
mod machine;
mod project;
mod release;
mod runtime_check;
mod scenarios;
mod upstream;
use anyhow::{bail, ensure, Context, Result};
use machine::{Machine, Rig};
use project::Project;
use std::{collections::BTreeMap, fs};

fn main() {
    if let Err(error) = run() {
        eprintln!("xtask: {error:#}");
        std::process::exit(1);
    }
}
fn help() {
    println!("cargo xtask <command>\n\n  doctor                 Show the fixed toolchain\n  test                   Run host and xtask tests\n  build [NAME|--all]      Build and audit firmware\n  audit ELF              Audit an ELF without executing it\n  hex ELF OUTPUT         Audit an ELF and create an Intel HEX file\n  run NAME [--input TEXT] Build and run in the Rust reference CPU\n  smoke [NAME]           Check firmware behaviors in the Rust CPU\n  upstream [NAME]        Check the unmodified upstream core through Boa\n  fetch-upstream         Fetch the fixed official revision\n  runtime                Check startup, IRQ and memory ABI\n  docs                   Check bilingual documentation\n  crate-check            Verify the crate and a downstream heap app\n  verify                 Run every release gate\n  package                Package an unchanged verified commit\n  check-archive ZIP      Verify every archive member digest\n  release                Push main and publish after successful CI\n");
}
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first() else {
        help();
        return Ok(());
    };
    if command == "help" || command == "--help" {
        help();
        return Ok(());
    }
    let p = Project::new()?;
    match command.as_str() {
        "doctor" => {
            ensure!(args.len() == 1, "doctor takes no arguments");
            print!("{}", p.command("rustc", &["-vV"])?);
            print!("{}", p.cargo(&["--version"])?);
            println!(
                "workspace={}\noutput={}",
                p.root.display(),
                p.dist.display()
            );
        }
        "test" => {
            ensure!(args.len() == 1, "test takes no arguments");
            p.host_tests()?;
        }
        "build" => {
            ensure!(args.len() <= 2, "build takes one example or --all");
            p.build(args.get(1).map(String::as_str).filter(|&s| s != "--all"))?;
        }
        "audit" => {
            ensure!(args.len() == 2, "usage: audit ELF");
            let elf = image::Elf::parse(&fs::read(&args[1])?)?;
            println!("{}", serde_json::to_string_pretty(&elf.report)?);
        }
        "hex" => {
            ensure!(args.len() == 3, "usage: hex ELF OUTPUT");
            let elf = image::Elf::parse(&fs::read(&args[1])?)?;
            let text = image::to_hex(&elf.image)?;
            ensure!(image::from_hex(&text)? == elf.image, "HEX roundtrip failed");
            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&args[2])
                .context("output must not already exist")?;
            file.write_all(text.as_bytes())?;
            println!("HEX {}: {} loaded bytes", args[2], elf.image.len());
        }
        "run" => {
            ensure!(
                args.len() == 2 || args.len() == 4 && args[2] == "--input",
                "usage: run NAME [--input TEXT]"
            );
            p.build(Some(&args[1]))?;
            let hex = fs::read_to_string(p.dist.join(format!("{}.hex", args[1])))?;
            let mut cpu = Machine::new(&image::from_hex(&hex)?)?;
            cpu.run(100000)?;
            if let Some(input) = args.get(3) {
                cpu.send(input)?;
            }
            print!("{}", cpu.text);
            println!(
                "\n[{} instructions, observed stack {} bytes]",
                cpu.steps,
                3072 - cpu.min_sp
            );
        }
        "docs" => {
            ensure!(args.len() == 1, "docs takes no arguments");
            docs::check(&p)?;
        }
        "crate-check" => {
            ensure!(args.len() == 1, "crate-check takes no arguments");
            release::crate_check(&p)?;
        }
        "verify" => {
            ensure!(args.len() == 1, "verify takes no arguments");
            release::verify(&p)?;
        }
        "package" => {
            ensure!(args.len() == 1, "package takes no arguments");
            release::package(&p)?;
        }
        "release" => {
            ensure!(args.len() == 1, "release takes no arguments");
            release::publish(&p)?;
        }
        "check-archive" => {
            ensure!(args.len() == 2, "usage: check-archive ZIP");
            let m = release::verify_archive(&fs::read(&args[1])?)?;
            println!("VALID {} {}", m.tag, m.source_commit);
        }
        "runtime" => {
            ensure!(args.len() == 1, "runtime takes no arguments");
            runtime_check::run(&p)?;
        }
        "fetch-upstream" => {
            ensure!(args.len() == 1, "fetch-upstream takes no arguments");
            p.fetch_upstream()?;
        }
        "smoke" | "upstream" => {
            ensure!(args.len() <= 2, "one optional example name is allowed");
            firmware_tests(&p, command == "upstream", args.get(1).map(String::as_str))?;
        }
        _ => bail!("unknown command {command}; use cargo xtask help"),
    }
    Ok(())
}
pub fn firmware_tests(p: &Project, official: bool, selection: Option<&str>) -> Result<()> {
    let mut reports = p.reports()?;
    if let Some(name) = selection {
        let report = reports
            .remove(name)
            .with_context(|| format!("missing build for {name}"))?;
        reports = BTreeMap::from([(name.to_owned(), report)]);
    }
    if official {
        p.check_upstream(&p.upstream())?;
    }
    let jobs: Vec<_> = reports.into_iter().collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let workers = if official {
        std::thread::available_parallelism()
            .map_or(1, usize::from)
            .min(8)
    } else {
        1
    };
    let groups = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..workers {
            let next = &next;
            let jobs = &jobs;
            handles.push(
                scope.spawn(move || -> Result<BTreeMap<String, serde_json::Value>> {
                    let mut results = BTreeMap::new();
                    loop {
                        let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some((name, build)) = jobs.get(index) else {
                            break;
                        };
                        let text = fs::read_to_string(p.dist.join(format!("{name}.hex")))?;
                        ensure!(
                            image::hash(text.as_bytes()) == build.hex_sha256,
                            "firmware changed after audit: {name}"
                        );
                        let snapshot = if official {
                            let mut rig = upstream::Upstream::new(&p.upstream(), &text)?;
                            let snapshot = scenarios::check(name, &mut rig, build)
                                .with_context(|| format!("upstream {name}"))?;
                            if name == "bitmap_palette" {
                                rig.check_colors()?;
                            }
                            snapshot
                        } else {
                            let mut rig = Machine::new(&image::from_hex(&text)?)?;
                            scenarios::check(name, &mut rig, build)
                                .with_context(|| format!("reference {name}"))?
                        };
                        println!(
                            "PASS {} {name}: {} instructions, observed stack {}/512",
                            if official { "upstream" } else { "reference" },
                            snapshot.steps,
                            snapshot.observed_stack_bytes
                        );
                        let result = serde_json::json!({
                            "instructions": snapshot.steps,
                            "irq_entries": snapshot.irq_entries,
                            "observed_stack_bytes": snapshot.observed_stack_bytes,
                            "text": snapshot.text,
                            "ram_sha256": image::hash(&snapshot.ram),
                            "gpio_value": snapshot.gpio_value,
                            "passed": true
                        });
                        results.insert(name.clone(), result);
                    }
                    Ok(results)
                }),
            );
        }
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .map_err(|_| anyhow::anyhow!("firmware worker panicked"))?
            })
            .collect::<Result<Vec<_>>>()
    })?;
    let results: BTreeMap<_, _> = groups.into_iter().flatten().collect();
    let name = match (official, selection.is_some()) {
        (true, false) => "upstream-verification.json",
        (false, false) => "reference-verification.json",
        (true, true) => "selected-upstream.json",
        (false, true) => "selected-reference.json",
    };
    p.write_json(name,&serde_json::json!({"engine":if official{"Boa 0.20 Rust engine, unmodified upstream ES modules"}else{"independent Rust RV32I model"},"upstream_commit":if official{Some(project::UPSTREAM_PIN)}else{None},"browser_ui_tested":false,"stack_note":"observed paths, not a worst-case proof","firmware":results}))?;
    Ok(())
}
