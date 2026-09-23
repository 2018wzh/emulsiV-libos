//! Explicit, zeroing bump arena. No global allocator and no heap dependency.
//! Returned allocations are disjoint. No reset API can invalidate live borrows.
pub struct Arena<'a> {
    rest: &'a mut [u8],
    used: usize,
}
impl<'a> Arena<'a> {
    pub fn new(storage: &'a mut [u8]) -> Self {
        Self {
            rest: storage,
            used: 0,
        }
    }
    pub fn used(&self) -> usize {
        self.used
    }
    pub fn remaining(&self) -> usize {
        self.rest.len()
    }
    /// Alignment must be a nonzero power of two. Failed requests change nothing.
    /// Padding counts toward used bytes. Zero-length allocations are rejected.
    pub fn allocate(&mut self, len: usize, align: usize) -> Option<&'a mut [u8]> {
        if len == 0 || !align.is_power_of_two() {
            return None;
        }
        let address = self.rest.as_ptr() as usize;
        let pad = address.wrapping_neg() & (align - 1);
        let end = pad.checked_add(len)?;
        if end > self.rest.len() {
            return None;
        }
        let rest = core::mem::take(&mut self.rest);
        let (allocated, remainder) = rest.split_at_mut(end);
        self.rest = remainder;
        self.used += end;
        let result = &mut allocated[pad..];
        result.fill(0);
        Some(result)
    }
}
