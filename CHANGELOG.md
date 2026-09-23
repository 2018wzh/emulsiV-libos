# Changelog

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
