use std::{env, fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-changed=link.x");
    println!("cargo:rerun-if-changed=src/startup.S");
    println!("cargo:rerun-if-changed=src/memory.S");
    if env::var("TARGET").as_deref() == Ok("riscv32i-unknown-none-elf") {
        let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
        fs::copy("link.x", out.join("link.x")).expect("copy linker script");
        println!("cargo:rustc-link-search={}", out.display());
        println!("cargo:rustc-link-arg=-Tlink.x");
    }
}
