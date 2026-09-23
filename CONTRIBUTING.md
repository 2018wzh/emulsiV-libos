# Contributing / 参与开发

Use the fixed Rust toolchain and keep the target instruction set at RV32I.
Run `cargo xtask test` before testing firmware changes.
Run `cargo xtask build NAME` for the changed example.
Run `cargo xtask verify` before release.

For a new heap example, add `required-features = ["heap"]` in Cargo.toml.
Add its behavior case in `xtask/src/scenarios.rs`.
Document the example in both README files.
The release gate rejects examples without behavior coverage.

Use `RegisterIo` for testable drivers and `PixelTarget` for graphics.
Document the safety contract of every new unsafe API.
Do not weaken ISA or RAM checks to obtain a successful build.
Do not add an allocation inside allocator internals or diagnostic formatting used by the allocator.
Keep fallible allocation failure observable to the caller.

Use `cargo fmt --all` to format Rust source before committing.
Preserve the same commands and code blocks in both README files.
Follow `docs/WRITING.md` and record API changes in CHANGELOG.md.

中文：使用固定工具链。新增行为必须增加回归测试，新增固件必须增加 Rust 场景并同步双语文档。
不要通过关闭指令或内存检查来掩盖失败。每个 unsafe API 都应说明调用前提。
提交前格式化源码，发布前执行完整验证。发布不得强推或覆盖既有标签。
