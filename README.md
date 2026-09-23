# emulsiV-libos

[English](README.md) | [简体中文](README.zh-CN.md)

A Rust `no_std` library OS for the [emulsiV / Virgule simulator](https://eseo-tech.github.io/emulsiV/).
Version: **0.1.0-preview.2**. License: **MIT**.

The library provides TextIO, GPIO, RGB332 graphics, a bare-metal runtime, and an optional reclaiming heap.
A Rust `xtask` builds, audits, tests, and packages the firmware.
The repository includes 25 independent examples.

[Quick start](#quick-start) · [Heap](#heap) · [Examples](#examples) · [Commands](#xtask) · [Verification](#verification)

<a id="scope"></a>
## Scope

Use this project to learn bare-metal Rust or build small programs for stock emulsiV.
Applications call library functions directly. They do not use system calls.
The default build does not enable a global heap.

This is an educational preview, not a production operating system.
It does not provide POSIX, processes, virtual memory, a filesystem, networking, or preemptive threads.
The API can change before version 1.0.

A *firmware image* is one application linked with the library functions that it uses.
A *heap* is memory that the application can allocate and release at runtime.
A *tick* is an application-defined counter increment. It is not a millisecond.

<a id="requirements"></a>
## Requirements

To run the supplied HEX files, use a browser with emulsiV. A Rust installation is not necessary.

To build from source, install Rust with `rustup`, Git, and the standard linker for your host Rust installation.
The repository selects Rust **1.85.1** and the **`riscv32i-unknown-none-elf`** target.
The first build downloads the fixed Rust toolchain and the locked host dependencies.
The target library has no third-party crate dependencies.

The `xtask` implementation does not require Python, Node.js, a shell script, Clang, or a separate RISC-V toolchain.
It uses `rustc` and the bundled `rust-lld` for assembly checks.
It uses the Rust Boa engine to execute the official simulator's JavaScript modules as test inputs.
It does not replace those modules with another implementation.

GitHub publication also requires an authenticated `gh` CLI.
The full validation workflow is tested on Linux x86_64.
Other host platforms are not part of this release's validation.

<a id="quick-start"></a>
## Quick start

### Run a supplied firmware image

1. Open the [release page](https://github.com/2018wzh/emulsiV-libos/releases/tag/v0.1.0-preview.2).
2. Download the firmware ZIP and its SHA256 file.
3. Extract the ZIP.
4. Open emulsiV.
5. Load `firmware/hello.hex` from the extracted directory.
6. Reset the processor.
7. Start execution.

The TextIO output must show `Hello, emulsiV Rust!`.
Load `firmware/bitmap_palette.hex` to display all 256 RGB332 colors.
Load one image at a time. Each image is a separate application.

### Build an example

```bash
git clone https://github.com/2018wzh/emulsiV-libos.git
cd emulsiV-libos
git checkout v0.1.0-preview.2
rustup show
cargo xtask build hello
```

The output is `dist/preview2/hello.hex`.
The same directory contains the ELF file and its audit report.
The command fails if the image contains an unsupported instruction or exceeds the memory budget.

Run the example in the independent Rust CPU model:

```bash
cargo xtask run hello
cargo xtask run text_echo --input "Hello"
cargo xtask run heap_box
```

The model is a development tool. It does not test the browser interface.

<a id="features"></a>
## Features

| Area | Functions |
| :--- | :--- |
| TextIO | Byte input and output, polling, input interrupts, integer output, optional `core::fmt::Write` |
| GPIO | Direction, masked output, toggling, input sampling, edge events, interrupt masks |
| Bitmap | 32 × 32 RGB332 pixels, clipping, lines, rectangles, circles, 3 × 5 text, scrolling, monochrome sprites |
| Fixed storage | FIFO queues, typed events, UTF-8 strings, a 128-byte monochrome buffer, a borrowed arena |
| Tasks and input | Cooperative callbacks, deadlines, execution budgets, debounce, logical-tick PWM |
| Utilities | CRC16, CRC32, a non-cryptographic random generator, memory diagnostics |
| Runtime | Fixed reset and interrupt vectors, BSS initialization, register preservation, C memory functions |
| Optional heap | Aligned allocation, release, reuse, zeroing, reallocation, OOM handling, usage statistics |

| Cargo feature | Default | Effect |
| :--- | :--- | :--- |
| `runtime` | On | Include the target startup code and panic handler |
| `heap` | Off | Enable `alloc` and the target global allocator. Also enable `runtime` |
| `format` | Off | Implement `core::fmt::Write` for TextIO |

Use `--no-default-features` for the hardware-independent library components without the runtime.
Enabling `heap` on a host build exposes the borrowed-buffer allocator for tests.
It does not replace the host process's global allocator.

### Hardware limits

```text
0x00000000           reset vector
0x00000004           interrupt vector
0x00000008..         code, constants, initialized data, BSS
align16(image_end)   optional heap start
0x00000a00           heap end / stack bottom
0x00000a00..0x0bff   512-byte reserved stack
0x00000c00..0x0fff   1024-byte framebuffer
0xb0000000/1        TextIO status / input byte
0xc0000000          TextIO output byte
0xd0000000..0xd0000013   five 32-bit GPIO registers
```

Ordinary RAM is 3072 bytes. Code, data, heap, and stack share this space.
The framebuffer occupies another 1024 bytes.
The linker does not permit the heap to overlap the stack or framebuffer.

GPIO direction bit `1` means input. GPIO edge registers are ordinary writable registers, not write-one-to-clear registers.
RGB332 uses bits 7..5 for red, 4..2 for green, and 1..0 for blue.
The saturated colors are `RED=0xe0`, `GREEN=0x1c`, `BLUE=0x03`, and `WHITE=0xff`.

The firmware does not require CSR, ECALL, WFI, FENCE, atomic, compressed, or hardware multiply/divide instructions.
It uses the simulator's fixed interrupt entry and supported MRET instruction.

<a id="heap"></a>
## Heap support

Build a heap example with `xtask`. It selects the required feature automatically:

```bash
cargo xtask build heap_vec
cargo build --locked -p emulsiv-libos --release --target riscv32i-unknown-none-elf --features heap --example heap_vec
```

The global allocator initializes on its first use.
It manages the aligned space between `__heap_start` and `__heap_end`.
The end is the fixed stack bottom, not the current stack pointer.

The allocator uses a first-fit bitmap with 16-byte allocation units.
Released units become available for reuse. Adjacent free units can satisfy a larger request.
The state occupies 36 bytes on RV32I, in addition to any compiler-generated allocation flag.
Each allocation can waste up to 15 bytes because of rounding.
Alignment and fragmentation can also reduce usable space.

`Box`, `Vec`, and `String` use this allocator when `heap` is enabled on the target.
The following application allocates and releases a boxed integer:

```rust
#![no_std]
#![no_main]
extern crate alloc;

use alloc::boxed::Box;
use emulsiv_libos::{bus::Mmio, textio::TextIo};

emulsiv_libos::entry!(main);

fn main() -> ! {
    // SAFETY: this program runs on stock emulsiV with the supplied link.x.
    let mut bus = unsafe { Mmio::new() };
    let value = Box::new(42u32);
    TextIo::new(&mut bus).write_u32(**core::hint::black_box(&value));
    drop(value);
    emulsiv_libos::runtime::halt()
}
```

Use `Vec::try_reserve_exact` or `String::try_reserve_exact` when allocation failure must be recoverable.
Raw allocation returns a null pointer on failure.
Infallible allocation, such as `Box::new`, can enter the panic handler on OOM.
The panic handler prints `panic` and halts. It does not unwind.

Reallocation allocates another block, copies the common prefix, and releases the old block.
A failed reallocation leaves the original allocation unchanged.
This method needs temporary free space, even when some smaller sizes could fit in place.

`heap::stats()` reports capacity, used bytes, free bytes, the largest free run, and peak usage.
These are allocation-unit counts expressed in bytes, not exact payload sizes.
`Heap::new(&mut buffer)` provides the same allocator over an exclusively borrowed buffer.
Its raw-pointer APIs require the documented lifetime and deallocation rules.

| Heap example | Static image bytes | Available heap bytes | Reserved stack bytes |
| :--- | ---: | ---: | ---: |
| `heap_box` | 2336 | 224 | 512 |
| `heap_vec` | 2244 | 304 | 512 |
| `heap_string` | 2292 | 256 | 512 |
| `heap_reuse` | 1572 | 976 | 512 |
| `heap_oom` | 1484 | 1072 | 512 |

These values use the fixed toolchain and supplied examples. New code can change the budget.
Do not infer heap capacity from the no-heap `hello` image.
The default `shell` has only 32 spare static bytes. Adding a heap to that image needs further size work.
See [heap design and safety](docs/HEAP.md).

<a id="examples"></a>
## Examples

`xtask` builds heap examples with `heap`. It builds all other examples with the default features.
Configure simulator output pins as LEDs and input pins as buttons or switches before using GPIO examples.

| Example | Behavior or input |
| :--- | :--- |
| `hello` | Print a greeting |
| `text_echo` | Echo polled input bytes |
| `line_console` | Edit and echo a complete line. Use `;` to submit |
| `gpio_mirror` | Copy GPIO16..31 inputs to GPIO0..15 outputs |
| `gpio_debounce` | Debounce GPIO31 and toggle GPIO0 |
| `gpio_pwm` | Drive GPIO0 with a 16-tick period and four high ticks |
| `bitmap_palette` | Show all 256 RGB332 colors |
| `bitmap_shapes` | Draw lines, rectangles, blocks, and a circle |
| `bitmap_text` | Draw letters and digits |
| `mono_sprite` | Present a monochrome sprite through a 128-byte buffer |
| `cooperative` | Run LED and heartbeat callbacks |
| `event_loop` | Queue text events. Space toggles GPIO0 |
| `irq_echo` | Echo input from a TextIO interrupt |
| `irq_gpio` | Handle GPIO31 edge interrupts and toggle GPIO0 |
| `crc_demo` | Print known CRC check values |
| `arena_demo` | Allocate aligned storage from a borrowed arena |
| `shell` | Control GPIO and Bitmap with checked text commands |
| `random_pixels` | Draw deterministic pseudo-random pixels |
| `diagnostics` | Report static layout and current stack space |
| `paint` | Move with lowercase `w`, `a`, `s`, `d`. Select colors with `0`..`7`. Clear with `c` |
| `heap_box` | Allocate a Box and print its actual heap address |
| `heap_vec` | Grow a Vec and preserve its contents after a failed growth request |
| `heap_string` | Allocate, append, print, and release a String |
| `heap_reuse` | Allocate, zero, and release aligned storage 100 times |
| `heap_oom` | Reject an oversized allocation and then allocate again |

### Shell input

Load `shell.hex`. Type one command at a time in TextIO:

```text
?;
w 0x55aa;
r;
c 0;
p 16 16 0xe0;
p 17 16 0x03;
```

These commands show help, write GPIO, read GPIO, clear the screen, and draw red and blue pixels.
The official input view filters special keys such as Enter. The line examples therefore also accept `;` as a terminator.
Injected CR/LF bytes remain supported.
The input device holds only one byte. Pasting text is not a reliable serial transfer.
The parser rejects an oversized line as a whole. It does not execute a truncated command.

<a id="xtask"></a>
## Rust task commands

Run these commands from a Git checkout. The library `.crate` does not contain the host `xtask` workspace member.

| Command | Result |
| :--- | :--- |
| `cargo xtask doctor` | Show the toolchain and output paths |
| `cargo xtask build --all` | Build and audit all 25 firmware images |
| `cargo xtask build NAME` | Build one example with its required features |
| `cargo xtask audit ELF` | Check ELF structure, instructions, and memory limits |
| `cargo xtask hex ELF OUTPUT` | Audit an external ELF and create an Intel HEX file |
| `cargo xtask run NAME --input TEXT` | Build and run one example in the Rust CPU |
| `cargo xtask test` | Run library, heap, documentation, and task unit tests |
| `cargo xtask runtime` | Check assembly startup, IRQ preservation, and memory functions |
| `cargo xtask fetch-upstream` | Obtain the fixed official simulator revision |
| `cargo xtask smoke` | Run every firmware behavior test in the Rust CPU |
| `cargo xtask upstream` | Run the same behavior checks on the official core through Boa |
| `cargo xtask docs` | Check both README files and their example coverage |
| `cargo xtask crate-check` | Verify a Cargo package and a separate downstream heap application |
| `cargo xtask verify` | Run all release gates |
| `cargo xtask package` | Package unchanged, verified source and artifacts |
| `cargo xtask check-archive ZIP` | Verify archive paths, contents, and SHA256 values |
| `cargo xtask release` | Push `main` and create a GitHub preview after successful CI |

The `smoke` and `upstream` commands also accept one example name after a complete build.
Artifacts and reports are written to `dist/preview2/`.
Legacy scripts are retained as historical text under `legacy/`. No active command invokes them.

<a id="integration"></a>
## Use the library in another application

Use a path dependency during development, or select the Git release tag:

```toml
[dependencies]
emulsiv-libos = { git = "https://github.com/2018wzh/emulsiV-libos.git", tag = "v0.1.0-preview.2", features = ["heap"] }

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
```

Copy this release's `link.x` into the application directory.
Add an application `build.rs` that supplies the final linker script.
Do not assume that a dependency's build script supplies final binary arguments.
See the complete setup in [the integration guide](docs/PORTING.md).
The `crate-check` command builds and runs a separate application against Cargo's extracted package, not against this checkout.

<a id="verification"></a>
## Verification and safety limits

Obtain the official test dependency, then run all gates:

```bash
cargo xtask fetch-upstream
cargo xtask verify
```

The gates check both README files, Rust formatting, Clippy, feature combinations, heap behavior, ELF/HEX parsing, assembly ABI, and all firmware images.
They also check a standalone application against the packaged library.
Each firmware uses the same Rust assertions for the independent CPU and the official core.
The official revision is `9e15421cd33511d4d2911fea1ae41cd65f33dae9`.

Publication requires a clean commit and unchanged artifact digests.
A failed or incomplete verification does not authorize packaging.
The ZIP includes the evidence record, raw command log, reports, and file digests.
See [verification details](docs/VERIFICATION.md) and [GitHub Actions](https://github.com/2018wzh/emulsiV-libos/actions).

These checks do not constitute complete RISC-V conformance, a memory-safety proof, or browser UI testing.
Observed stack usage is not a worst-case bound for every input.
A program can still corrupt memory if its runtime stack exceeds the reserved space.
The emulator has no hardware protection between code, heap, and stack.

The global allocator masks both known device interrupt sources while it accesses allocator state.
This serialization is specific to the single-hart stock platform.
It is not a portable multi-hart lock or a guarantee of lossless device input.
Do not add another interrupt source without reviewing this contract.
Avoid allocation inside interrupt callbacks when predictable latency is required.

<a id="troubleshooting"></a>
## Troubleshooting

| Symptom | Action |
| :--- | :--- |
| `firmware exceeds stock 3 KiB RAM budget` | Remove unused output, formatting, or state. Do not enlarge RAM in the linker script |
| Allocation returns null or `try_reserve_exact` fails | Reduce the request. Check free space, alignment, fragmentation, and temporary reallocation space |
| `panic` appears | Stop execution. Check failed assertions, bounds, and infallible allocation requests |
| Enter does not submit a line | Use `;` in `shell` and `line_console` |
| Typed characters disappear | Reduce input rate. The hardware is a one-byte latch, not a FIFO |
| Upstream revision check fails | Restore a clean checkout at the pinned commit. Do not disable the check |
| Packaging reports changed source or evidence | Commit the intended changes, then rerun `cargo xtask verify` |
| CI is not complete | Inspect the Actions run. Rerun `cargo xtask release` after that commit passes |

To restore modified initialized data, reload the HEX image before a cold restart.
A jump to the reset vector alone does not restore the initial `.data` bytes.

<a id="release"></a>
## Release procedure

Update the package version, both README files, and the changelog before release.
Commit all intended source changes on `main` before the final verification.

```bash
cargo xtask fetch-upstream
cargo xtask verify
cargo xtask package
cargo xtask release
```

`release` accepts only the configured repository and the `2018wzh` GitHub account.
It preserves repository visibility. It does not force-push or overwrite an existing tag or release.
The first invocation can push the commit before CI completes. Rerun it after CI succeeds.
The command then uploads the firmware ZIP, its SHA256 file, and the verified `.crate` file.

GitHub publication is separate from crates.io publication.
The project verifies the Cargo package, but `xtask release` does not upload it to crates.io.
A crates.io owner must separately review and authorize registry publication.

<a id="contributing"></a>
## Contributing

Add a host regression test for each behavior change.
Add a Rust scenario when you add a firmware example.
Update both README files when a command, requirement, or limit changes.
Run all release gates before you submit a change.
See [CONTRIBUTING.md](CONTRIBUTING.md) and [the writing policy](docs/WRITING.md).

<a id="license"></a>
## License and references

Project code is available under the [MIT license](LICENSE).
The separately downloaded official emulator retains its own MPL-2.0 license.
The firmware archive does not redistribute that emulator's source.
The host dependencies retain their respective licenses.

Primary references: [emulsiV documentation](https://eseo-tech.github.io/emulsiV/doc/),
[Rust bare-metal targets](https://doc.rust-lang.org/rustc/platform-support/riscv32-unknown-none-elf.html),
[GlobalAlloc safety contract](https://doc.rust-lang.org/core/alloc/trait.GlobalAlloc.html),
and [Boa 0.20 API](https://docs.rs/boa_engine/0.20.0/boa_engine/).
