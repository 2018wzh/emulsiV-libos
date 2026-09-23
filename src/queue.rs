//! Fixed-capacity, single-owner FIFO. Not an interrupt-safe shared queue.
#[derive(Debug)]
pub struct Queue<T: Copy, const N: usize> {
    slots: [Option<T>; N],
    head: usize,
    len: usize,
}
impl<T: Copy, const N: usize> Default for Queue<T, N> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T: Copy, const N: usize> Queue<T, N> {
    pub const fn new() -> Self {
        Self {
            slots: [None; N],
            head: 0,
            len: 0,
        }
    }
    pub const fn len(&self) -> usize {
        self.len
    }
    pub const fn capacity(&self) -> usize {
        N
    }
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub const fn is_full(&self) -> bool {
        self.len == N
    }
    pub fn push(&mut self, value: T) -> Result<(), T> {
        if self.is_full() {
            return Err(value);
        }
        let tail = (self.head + self.len) % N;
        self.slots[tail] = Some(value);
        self.len += 1;
        Ok(())
    }
    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        let result = self.slots[self.head].take();
        self.head = (self.head + 1) % N;
        self.len -= 1;
        result
    }
    pub fn front(&self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            self.slots[self.head]
        }
    }
    pub fn clear(&mut self) {
        self.slots.fill(None);
        self.head = 0;
        self.len = 0;
    }
}
