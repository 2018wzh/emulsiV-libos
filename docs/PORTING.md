# Application integration / 应用集成

## Create a separate application

Use Rust 1.85.1 and target `riscv32i-unknown-none-elf`.
Place the following files in a new application directory outside this workspace.
Copy `link.x` from the same emulsiV-libos release into that directory.

`Cargo.toml`:

```toml
[package]
name = "my-emulsiv-app"
version = "0.1.0"
edition = "2021"
build = "build.rs"

[dependencies]
emulsiv-libos = { git = "https://github.com/2018wzh/emulsiV-libos.git", tag = "v0.1.0-preview.2", features = ["heap"] }

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
```

`rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.85.1"
profile = "minimal"
targets = ["riscv32i-unknown-none-elf"]
```

`build.rs`:

```rust
fn main() {
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::copy("link.x", out.join("application.x")).unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rustc-link-arg=-Tapplication.x");
    println!("cargo:rerun-if-changed=link.x");
}
```

Use the complete `no_std` application from the README as `src/main.rs`.
The final application must select the linker script. A dependency's `rustc-link-arg` is not a substitute.
Remove the `heap` feature when the application does not use heap allocation.
Do not provide another global allocator or panic handler when the library runtime supplies them.

Build the application:

```bash
cargo build --release --target riscv32i-unknown-none-elf
```

From the emulsiV-libos Git checkout, audit the resulting ELF:

```bash
cargo xtask audit /absolute/path/to/my-emulsiv-app/target/riscv32i-unknown-none-elf/release/my-emulsiv-app
```

For automatic HEX output, add the program under this repository's `examples/` directory and use `cargo xtask build NAME`.
To convert an external ELF, use `cargo xtask hex INPUT_ELF OUTPUT_HEX`.
The command audits the ELF first and refuses to overwrite an existing output file.
The supplied build command always audits an image before it writes the HEX file.

## Change the hardware abstraction

Keep drivers behind `RegisterIo` and graphics behind `PixelTarget`.
Add a host test before changing a device register rule.
A new interrupt source requires a new review of the global allocator's serialization.
A new target requires startup, ABI, instruction, and memory validation.

Do not enlarge the linker RAM region to suppress a size failure.
Do not assume `riscv-rt` is compatible with Virgule's nonstandard interrupt model.
Do not use `spin_loop`, WFI, atomics, or CSR operations without auditing the generated instructions.

## Verify the distributed package

`cargo xtask crate-check` runs Cargo's package verification.
It then builds a separate heap application against the extracted package.
The test checks that the distributed library and linker script work outside the original workspace.
The check does not upload to crates.io.

## 中文说明

在工作区外创建独立应用，并复制同一发布版本的 `link.x`。
使用上述 Cargo、工具链和 `build.rs` 配置，再将 README 中的完整裸机程序保存为 `src/main.rs`。
应用必须明确指定最终链接脚本，不能只依赖库的构建参数。
不用堆时，应移除依赖中的 `heap` feature。

修改外设映射时，应先增加主机测试。
增加中断源时，应重新检查全局分配器的串行化条件。
不要通过放宽 RAM 范围或关闭指令检查来掩盖构建失败。
`crate-check` 验证独立下游应用，不会发布到 crates.io。
