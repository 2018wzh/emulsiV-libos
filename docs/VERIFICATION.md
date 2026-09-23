# Verification / 验证

Version: 0.1.0-preview.2.
This document specifies the release gates and evidence format.
The generated evidence and the Actions run establish whether a particular source commit passed.
Do not apply a previous commit's result to modified source.

## Reproduce the gates

```bash
cargo xtask fetch-upstream
cargo xtask verify
```

Use the fixed Rust 1.85.1 toolchain on Linux x86_64.
The official emulator must be a clean checkout at `9e15421cd33511d4d2911fea1ae41cd65f33dae9`.
The tools do not invoke Python, Node, Clang, objcopy, or the archived scripts.

## Validation layers

| Layer | Coverage |
| :--- | :--- |
| Documentation | Required bilingual sections, matching code blocks, local links, and all example names |
| Source quality | Rust formatting, Clippy for the library and xtask, and Git whitespace checks |
| Host tests | Default, format, heap, heap+format, and no-default feature configurations |
| Heap tests | Alignment, zeroing, exhaustion, failure preservation, fragmentation, adjacent reuse, random live-block integrity |
| Rust task tests | ELF/HEX errors, ISA whitelist, CPU arithmetic, MMIO, IRQ, archive integrity, stale evidence rejection |
| Assembly ABI | Fixed vectors, poisoned BSS, preserved framebuffer, 100 IRQ register roundtrips, memory functions, strong handler override |
| Firmware builds | All 25 examples, correct required features, ELF audit, static RAM and heap extents, HEX roundtrip |
| Rust reference CPU | Every example's behavior, not just startup |
| Official core in Boa | The same Rust assertions against the unmodified upstream CPU and devices |
| Display protocol | The upstream Bitmap view converts all 256 RGB332 encodings through Rust-native canvas callbacks |
| Cargo distribution | Standard package verification and a separate application using the extracted package |

Each firmware begins with 100000 executed instructions in each engine.
Interactive tests add input and run additional instructions.
TextIO IRQ and GPIO IRQ tests each complete 100 entries and returns.
The heap examples verify a real Box address, Vec growth, String content, 100 reuse cycles, and OOM recovery.

The independent Rust CPU and the official CPU are separate implementations.
The official ES modules are loaded as external source files through Boa's module API.
Rust controls execution and assertions. No project JavaScript harness is evaluated.
This preserves the upstream comparison without requiring a Node installation.

## Evidence and release binding

Output is written to `dist/preview2/`.
`verification.json` records the source commit, source fingerprint, version, clean-tree state, and artifact hashes.
A build invalidates an earlier verification receipt.
Packaging checks the receipt against the current source and every recorded file.
It also requires complete build and behavior coverage.

The firmware ZIP includes ELF files, HEX files, per-image reports, both engine reports, package checks, and the frozen command log.
`manifest.json` stores SHA256 values for every other archive member.
The ZIP uses sorted member names, fixed ZIP timestamps, and the Stored compression method.
The same inputs produce the same ZIP bytes. Logs and host paths can differ between separate validation runs.
A ZIP checksum is an integrity check, not an authenticity signature.

`cargo xtask release` additionally verifies the remote commit and successful CI before creating the release.
It refuses an existing tag or release instead of overwriting it.
It does not change repository visibility or upload to crates.io.

## Limits

No complete browser UI, DOM, keyboard, or mouse end-to-end test is included.
The canvas adapter tests the upstream conversion method, not a browser renderer.
The project has not completed the full RISC-V conformance suite or a formal memory-safety proof.
It does not claim an exhaustive heap/interrupt interleaving test.

Recorded stack usage applies only to executed paths.
It is not a worst-case bound for arbitrary applications or inputs.
Host `format` tests do not imply that every target example fits when all features are enabled.
Software ticks are not wall-clock time. TextIO remains a one-byte hardware latch.

## 中文说明

本文件说明发布检查和证据格式，具体提交是否通过以生成报告和 Actions 结果为准。
所有测试工具均为 Rust，不调用历史 Python、Node 或 Shell 工具。
两种 CPU 使用相同的 Rust 行为断言，但 CPU 实现相互独立。
官方 JavaScript 模块保持原样，由 Rust Boa 引擎执行。

发布记录绑定源码提交、源码指纹和每个产物摘要。
重新构建会使旧记录失效。源码或产物变化后，必须重新验证。
ZIP 内的 manifest 验证每个文件，外部 SHA256 验证整个 ZIP，但它们不是数字签名。

测试不包含完整浏览器交互，也不是形式化内存安全或最坏情况栈上界证明。
默认栈仍为 512 字节，堆不得侵入栈或帧缓冲。
历史版本的故障和验证记录保留在 Git 历史及历史目录中，不代表本版本的结果。
