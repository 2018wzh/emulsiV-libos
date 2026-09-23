#![cfg(feature = "heap")]
use core::{alloc::Layout, ptr::NonNull};
use emulsiv_libos::heap::{Heap, MAX_CAPACITY};
#[repr(align(256))]
struct Memory([u8; MAX_CAPACITY + 256]);
fn layout(n: usize, a: usize) -> Layout {
    Layout::from_size_align(n, a).unwrap()
}

#[test]
fn empty_and_zero_size() {
    let mut bytes = [];
    let mut heap = Heap::new(&mut bytes);
    assert_eq!(heap.stats().capacity, 0);
    assert!(heap.allocate(layout(1, 1)).is_none());
    let mut memory = Memory([0; MAX_CAPACITY + 256]);
    let mut heap = Heap::new(&mut memory.0);
    assert!(heap.allocate(layout(0, 1)).is_none());
    assert_eq!(heap.stats().capacity, MAX_CAPACITY);
}
#[test]
fn alignment_and_rounding() {
    let mut memory = Memory([0; MAX_CAPACITY + 256]);
    let mut heap = Heap::new(&mut memory.0[1..]);
    let mut pointers = Vec::new();
    for a in [1, 2, 4, 8, 16, 32, 64, 128, 256] {
        let l = layout(3, a);
        let p = heap.allocate(l).unwrap();
        assert_eq!(p.as_ptr() as usize % a, 0);
        pointers.push((p, l));
    }
    assert_eq!(heap.stats().used, 9 * 16);
    for (p, l) in pointers {
        unsafe {
            heap.deallocate(p, l);
        }
    }
    assert_eq!(heap.stats().used, 0);
    assert_eq!(heap.stats().largest_free, heap.stats().capacity);
}
#[test]
fn zeroed_payload_and_canaries() {
    let mut memory = Memory([0xa5; MAX_CAPACITY + 256]);
    {
        let mut heap = Heap::new(&mut memory.0[256..512]);
        let p = heap.allocate_zeroed(layout(31, 16)).unwrap();
        assert!(unsafe { core::slice::from_raw_parts(p.as_ptr(), 31) }
            .iter()
            .all(|&b| b == 0));
        unsafe {
            heap.deallocate(p, layout(31, 16));
        }
    }
    assert!(memory.0[..256].iter().all(|&b| b == 0xa5));
    assert!(memory.0[512..].iter().all(|&b| b == 0xa5));
}
#[test]
fn fragmentation_and_adjacent_reuse() {
    let mut memory = Memory([0; MAX_CAPACITY + 256]);
    let mut heap = Heap::new(&mut memory.0[..64]);
    let l = layout(16, 16);
    let p: Vec<_> = (0..4).map(|_| heap.allocate(l).unwrap()).collect();
    unsafe {
        heap.deallocate(p[0], l);
        heap.deallocate(p[2], l);
    }
    assert_eq!(heap.stats().free, 32);
    assert_eq!(heap.stats().largest_free, 16);
    assert!(heap.allocate(layout(32, 16)).is_none());
    unsafe {
        heap.deallocate(p[1], l);
    }
    let q = heap.allocate(layout(48, 16)).unwrap();
    assert_eq!(q, p[0]);
    unsafe {
        heap.deallocate(q, layout(48, 16));
        heap.deallocate(p[3], l);
    }
    assert_eq!(heap.stats().free, 64);
}
#[test]
fn failed_allocation_preserves_stats() {
    let mut memory = Memory([0; MAX_CAPACITY + 256]);
    let mut heap = Heap::new(&mut memory.0[..64]);
    let p = heap.allocate(layout(64, 16)).unwrap();
    let before = heap.stats();
    assert!(heap.allocate(layout(1, 1)).is_none());
    assert!(heap.allocate(layout(isize::MAX as usize, 1)).is_none());
    assert_eq!(heap.stats(), before);
    unsafe {
        heap.deallocate(p, layout(64, 16));
    }
}
#[test]
fn realloc_copies_and_failure_preserves_original() {
    let mut memory = Memory([0; MAX_CAPACITY + 256]);
    let mut heap = Heap::new(&mut memory.0[..128]);
    let p = heap.allocate(layout(16, 16)).unwrap();
    unsafe {
        p.as_ptr().write_bytes(0x5a, 16);
    }
    let q = unsafe { heap.reallocate(p, layout(16, 16), 48) }.unwrap();
    assert_eq!(
        unsafe { core::slice::from_raw_parts(q.as_ptr(), 16) },
        &[0x5a; 16]
    );
    let before = heap.stats();
    assert!(unsafe { heap.reallocate(q, layout(48, 16), 128) }.is_none());
    assert!(unsafe { heap.reallocate(q, layout(48, 16), 0) }.is_none());
    assert_eq!(heap.stats(), before);
    assert_eq!(unsafe { q.as_ptr().read() }, 0x5a);
    let r = unsafe { heap.reallocate(q, layout(48, 16), 8) }.unwrap();
    assert_eq!(
        unsafe { core::slice::from_raw_parts(r.as_ptr(), 8) },
        &[0x5a; 8]
    );
    unsafe {
        heap.deallocate(r, layout(8, 16));
    }
    assert_eq!(heap.stats().used, 0);
}
#[test]
fn repeated_allocation_is_not_a_bump_leak() {
    let mut memory = Memory([0; MAX_CAPACITY + 256]);
    let mut heap = Heap::new(&mut memory.0[..32]);
    for _ in 0..10000 {
        let p = heap.allocate(layout(19, 4)).unwrap();
        unsafe {
            heap.deallocate(p, layout(19, 4));
        }
    }
    assert_eq!(heap.stats().peak_used, 32);
    assert_eq!(heap.stats().free, 32);
}
#[test]
fn random_live_blocks_remain_disjoint_and_intact() {
    let mut memory = Memory([0; MAX_CAPACITY + 256]);
    let mut heap = Heap::new(&mut memory.0);
    let mut live: Vec<(NonNull<u8>, Layout, u8)> = Vec::new();
    let mut rng = 0x55aa7733u32;
    for iteration in 0..5000 {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        if !live.is_empty() && rng & 3 == 0 {
            let (p, l, byte) = live.swap_remove(rng as usize % live.len());
            assert!(unsafe { core::slice::from_raw_parts(p.as_ptr(), l.size()) }
                .iter()
                .all(|&v| v == byte));
            unsafe {
                heap.deallocate(p, l);
            }
        } else {
            let l = layout(1 + (rng as usize % 127), 1 << ((rng >> 8) % 8));
            if let Some(p) = heap.allocate(l) {
                let start = p.as_ptr() as usize;
                for (other, ol, _) in &live {
                    let os = other.as_ptr() as usize;
                    assert!(start + l.size() <= os || os + ol.size() <= start);
                }
                let byte = iteration as u8;
                unsafe {
                    p.as_ptr().write_bytes(byte, l.size());
                }
                live.push((p, l, byte));
            }
        }
        assert_eq!(
            heap.stats().used,
            live.iter()
                .map(|(_, l, _)| l.size().div_ceil(16) * 16)
                .sum()
        );
    }
    for (p, l, byte) in live {
        assert!(unsafe { core::slice::from_raw_parts(p.as_ptr(), l.size()) }
            .iter()
            .all(|&v| v == byte));
        unsafe {
            heap.deallocate(p, l);
        }
    }
    assert_eq!(heap.stats().free, MAX_CAPACITY);
}
