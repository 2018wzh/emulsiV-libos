# Changelog

## 0.1.0-preview.1 (2026-09-23)

Restored the exact project history into MCPX PlayGround and validated Rust 1.85.1.
All 20 RV32I examples now compile within stock RAM and a reserved 512-byte stack.
Refactored Shell parsing and optimized constant-address MMIO calls; no RAM checks were weakened.
Corrected Bitmap to native RGB332 (256 colors), added RGB888 quantization, and fixed palette/paint colors.
Added semicolon line submission for the browser TextIO key filter.
Added default/format regression tests (47/48), nine independent firmware scenarios, and a pinned
upstream JavaScript-core runner covering all 20 boots and ten interaction/display checks.
Added unified CI/publication gates, bundled LLD detection and checksummed firmware packaging.
Browser full-UI end-to-end testing and exhaustive stack bounds remain outside this validation.

## 0.1.0 development snapshot (unreleased)

Added allocation-free Rust drivers for TextIO, GPIO and Bitmap, a custom Virgule runtime,
C ABI memory primitives, bounded containers, line editing, cooperative tasks, graphics,
checksums, logical timing, 20 example programs and host regression test source.
Added a strict ELF/HEX audit, independent RV32I reference runner, assembly verification,
CI configuration and a guarded GitHub publishing script.

Corrected GPIO event acknowledgement: event registers are ordinary writable memory,
not write-one-to-clear. Used a linker-provided default IRQ callback so strong user callbacks
can actually override it.

Verified Python tools and assembly fixtures only. Rust compilation, target example size,
upstream browser execution, Playground persistence and GitHub publication remain pending.
