//! A reclaiming, first-fit heap with 16-byte allocation units.
//!
//! `Heap` owns an exclusive borrow of its backing memory. The target global
//! allocator uses the linker extent between `__heap_start` and `__heap_end`.
//! No object header is stored in user memory. Free adjacent units form a larger
//! free run automatically. Fragmentation can still prevent a large allocation.
//!
//! ```
//! use core::alloc::Layout;
//! use emulsiv_libos::heap::Heap;
//!
//! let mut storage = [0u8; 96];
//! let mut heap = Heap::new(&mut storage);
//! let layout = Layout::from_size_align(16, 16).unwrap();
//! let block = heap.allocate_zeroed(layout).unwrap();
//! assert_eq!(heap.stats().used, 16);
//! // SAFETY: block is live in this heap and layout is its original layout.
//! unsafe { heap.deallocate(block, layout); }
//! assert_eq!(heap.stats().used, 0);
//! ```
use core::{
    alloc::Layout,
    marker::PhantomData,
    ptr::{self, NonNull},
};

/// The size of one allocation unit, in bytes.
pub const GRANULE: usize = 16;
/// Maximum managed memory. The stock platform cannot provide a larger heap.
pub const MAX_CAPACITY: usize = 2560;
const UNITS: usize = MAX_CAPACITY / GRANULE;

/// Byte counts rounded to allocation units, not requested payload lengths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeapStats {
    pub capacity: usize,
    pub used: usize,
    pub free: usize,
    pub largest_free: usize,
    pub peak_used: usize,
}

struct State {
    base: *mut u8,
    units: usize,
    bits: [u32; UNITS / 32],
    used: usize,
    peak: usize,
}
impl State {
    const fn empty() -> Self {
        Self {
            base: ptr::null_mut(),
            units: 0,
            bits: [0; UNITS / 32],
            used: 0,
            peak: 0,
        }
    }
    // The caller supplies exclusive, writable memory with this lifetime.
    #[inline(never)]
    unsafe fn init(&mut self, base: *mut u8, bytes: usize) {
        let skip = (base as usize).wrapping_neg() & (GRANULE - 1);
        if bytes < skip {
            return;
        }
        // Use wrapping_add so an empty input never requires an in-bounds offset.
        self.base = base.wrapping_add(skip);
        self.units = ((bytes - skip) / GRANULE).min(UNITS);
    }
    fn occupied(&self, i: usize) -> bool {
        // SAFETY: all callers restrict i to 0..units, and units <= UNITS.
        unsafe { self.bits.get_unchecked(i / 32) & (1 << (i % 32)) != 0 }
    }
    fn mark(&mut self, start: usize, count: usize, on: bool) {
        for i in start..start + count {
            // SAFETY: allocate validates the range. deallocate requires a live
            // allocation and its exact Layout, so it supplies the same range.
            let word = unsafe { self.bits.get_unchecked_mut(i / 32) };
            let bit = 1 << (i % 32);
            if on {
                *word |= bit;
            } else {
                *word &= !bit;
            }
        }
    }
    #[inline(never)]
    fn allocate(&mut self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 || layout.size() > self.units * GRANULE {
            return ptr::null_mut();
        }
        let need = layout.size().div_ceil(GRANULE);
        let mut run = 0;
        let mut start = 0;
        for i in 0..self.units {
            if self.occupied(i) {
                run = 0;
                continue;
            }
            if run == 0 {
                if (self.base as usize + i * GRANULE) & (layout.align() - 1) != 0 {
                    continue;
                }
                start = i;
            }
            run += 1;
            if run == need {
                self.mark(start, need, true);
                self.used += need;
                self.peak = self.peak.max(self.used);
                return self.base.wrapping_add(start * GRANULE);
            }
        }
        ptr::null_mut()
    }
    #[inline(never)]
    unsafe fn deallocate(&mut self, p: *mut u8, layout: Layout) {
        let start = (p as usize - self.base as usize) / GRANULE;
        let need = layout.size().div_ceil(GRANULE);
        self.mark(start, need, false);
        self.used -= need;
    }
    #[inline(never)]
    unsafe fn resize(&mut self, p: *mut u8, old: Layout, new_size: usize) -> *mut u8 {
        let Ok(new) = Layout::from_size_align(new_size, old.align()) else {
            return ptr::null_mut();
        };
        let q = self.allocate(new);
        if !q.is_null() {
            // SAFETY: q is disjoint live storage. p and old satisfy this API's
            // contract. Failure leaves p, its bytes, and its allocation intact.
            unsafe {
                ptr::copy_nonoverlapping(p, q, old.size().min(new_size));
                self.deallocate(p, old);
            }
        }
        q
    }
    fn stats(&self) -> HeapStats {
        let (mut run, mut largest) = (0, 0);
        for i in 0..self.units {
            if self.occupied(i) {
                run = 0;
            } else {
                run += 1;
                largest = largest.max(run);
            }
        }
        HeapStats {
            capacity: self.units * GRANULE,
            used: self.used * GRANULE,
            free: (self.units - self.used) * GRANULE,
            largest_free: largest * GRANULE,
            peak_used: self.peak * GRANULE,
        }
    }
}

/// An exclusive heap over a borrowed buffer. Raw allocation pointers must not
/// outlive this heap or its backing buffer. This type is neither Send nor Sync.
pub struct Heap<'a> {
    state: State,
    _memory: PhantomData<&'a mut [u8]>,
}
impl<'a> Heap<'a> {
    /// Align the start and limit the managed extent to MAX_CAPACITY bytes.
    pub fn new(memory: &'a mut [u8]) -> Self {
        let mut state = State::empty();
        // SAFETY: the exclusive borrow remains held for the lifetime of Heap.
        unsafe {
            state.init(memory.as_mut_ptr(), memory.len());
        }
        Self {
            state,
            _memory: PhantomData,
        }
    }
    /// Return aligned storage, or None for zero size, OOM, or unavailable alignment.
    pub fn allocate(&mut self, layout: Layout) -> Option<NonNull<u8>> {
        NonNull::new(self.state.allocate(layout))
    }
    /// Allocate and clear the requested payload bytes.
    pub fn allocate_zeroed(&mut self, layout: Layout) -> Option<NonNull<u8>> {
        let p = self.allocate(layout)?;
        // SAFETY: this allocation contains layout.size() writable bytes.
        unsafe {
            p.as_ptr().write_bytes(0, layout.size());
        }
        Some(p)
    }
    /// Release storage for reuse.
    ///
    /// # Safety
    /// p must be a live allocation from this heap with the exact supplied layout.
    /// Do not use p or its derived references after this call.
    pub unsafe fn deallocate(&mut self, p: NonNull<u8>, layout: Layout) {
        unsafe {
            self.state.deallocate(p.as_ptr(), layout);
        }
    }
    /// Allocate a replacement, copy the common prefix, and release the old block.
    /// None leaves the old block valid and unchanged. Zero size returns None.
    ///
    /// # Safety
    /// p and old must describe a live allocation from this heap. On success,
    /// discard p and use only the returned pointer with the new size.
    pub unsafe fn reallocate(
        &mut self,
        p: NonNull<u8>,
        old: Layout,
        size: usize,
    ) -> Option<NonNull<u8>> {
        NonNull::new(unsafe { self.state.resize(p.as_ptr(), old, size) })
    }
    pub fn stats(&self) -> HeapStats {
        self.state.stats()
    }
}

#[cfg(all(target_arch = "riscv32", target_os = "none"))]
mod global {
    use super::*;
    use core::{alloc::GlobalAlloc, cell::UnsafeCell};
    pub struct Allocator(UnsafeCell<State>);
    // SAFETY: stock Virgule has one hart. Every state access masks both device
    // IRQ sources before borrowing, and restores them after the borrow ends.
    // No allocation, user callback, or unwinding occurs inside this operation.
    unsafe impl Sync for Allocator {}
    impl Allocator {
        fn access<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
            crate::runtime::with_device_irqs_masked(|| {
                // SAFETY: both known IRQ sources are masked. The reference
                // cannot escape because R cannot borrow from the input lifetime.
                let state = unsafe { &mut *self.0.get() };
                if state.base.is_null() {
                    extern "C" {
                        static __heap_start: u8;
                        static __heap_end: u8;
                    }
                    let start = ptr::addr_of!(__heap_start) as usize;
                    let end = ptr::addr_of!(__heap_end) as usize;
                    // SAFETY: link.x reserves this exclusive ordinary RAM extent.
                    unsafe {
                        state.init(start as *mut u8, end.saturating_sub(start));
                    }
                }
                f(state)
            })
        }
    }
    unsafe impl GlobalAlloc for Allocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            self.access(|s| s.allocate(layout))
        }
        unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
            self.access(|s| unsafe { s.deallocate(p, layout) });
        }
        unsafe fn realloc(&self, p: *mut u8, old: Layout, size: usize) -> *mut u8 {
            self.access(|s| unsafe { s.resize(p, old, size) })
        }
    }
    #[global_allocator]
    static GLOBAL: Allocator = Allocator(UnsafeCell::new(State::empty()));
    pub fn stats() -> HeapStats {
        GLOBAL.access(|s| s.stats())
    }
}

/// Report the target global heap. This function also initializes the heap lazily.
#[cfg(all(target_arch = "riscv32", target_os = "none"))]
pub use global::stats;
