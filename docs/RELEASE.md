# emulsiV-libos v0.1.0-preview.2

This preview adds an optional reclaiming heap and a Rust-only task implementation.
The target library still has no third-party dependencies.
The default configuration still uses no global heap.

The heap supports aligned allocation, release, zeroing, reallocation, usage statistics, and recoverable OOM.
Five new examples exercise Box, Vec, String, repeated reuse, and failed allocation recovery.
All 25 examples retain the stock RAM map and a 512-byte stack reservation.

`cargo xtask` now performs ELF/HEX conversion and audit, reference execution, official-core validation, assembly ABI checks, documentation checks, packaging, and GitHub publication.
The official JavaScript modules execute through the Rust Boa engine without Node.
The previous scripts remain only as archived historical text.

English and Chinese README files cover setup, feature selection, heap limits, examples, integration, troubleshooting, validation, and publication.
The distributed Cargo package is checked with a separate downstream heap application.
The firmware ZIP includes 25 HEX images, ELF files, verification evidence, and a SHA256 manifest.
The `.crate` attachment is verified but has not been uploaded to crates.io by this command.

Limitations: this is an educational preview, not a production OS.
Tests do not cover the full browser UI or prove worst-case stack usage or memory safety.
Heap allocation rounds to 16-byte units. Fragmentation and temporary reallocation space can cause OOM.
TextIO is still a one-byte latch, and software ticks are not milliseconds.

中文：本版新增可回收堆、五个堆示例和纯 Rust xtask，保留默认无堆模式及原内存边界。
发布包包含 25 个固件、验证证据和摘要清单，双语 README 已补齐完整使用流程。
官方核心测试由 Rust Boa 执行，不依赖 Node。Cargo 包包含独立下游应用验证。
这仍是教学预览版，不声称完整浏览器 UI 验收、形式化安全证明或 crates.io 已发布。
