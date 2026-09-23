#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
//! Bare-metal building blocks for stock emulsiV, with an optional reclaiming heap.
//! Enable `heap` to use the Rust `alloc` crate on the RV32I target.
#[cfg(feature = "heap")]
extern crate alloc;
pub mod arena;
pub mod bitmap;
pub mod bus;
pub mod console;
pub mod crc;
pub mod event;
pub mod fixed;
pub mod gpio;
#[cfg(feature = "heap")]
pub mod heap;
pub mod input;
pub mod queue;
pub mod rng;
#[cfg(all(feature = "runtime", target_arch = "riscv32", target_os = "none"))]
pub mod runtime;
pub mod scheduler;
pub mod shell;
pub mod textio;
pub mod time;

/// Define the bare-metal Rust entry point. `main` must have signature `fn() -> !`.
#[macro_export]
macro_rules! entry {
    ($main:path) => {
        #[no_mangle]
        pub extern "C" fn __libos_main() -> ! {
            $main()
        }
    };
}
/// Override the default IRQ callback. The assembly wrapper preserves the ABI.
/// Never block, allocate or use unsynchronized shared Rust state in the callback.
#[macro_export]
macro_rules! interrupt {
    ($handler:path) => {
        #[no_mangle]
        pub extern "C" fn __libos_irq_handler() {
            $handler()
        }
    };
}
