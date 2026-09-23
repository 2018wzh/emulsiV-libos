//! A UTF-8 preserving bounded string. Appends fail without partial writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityError;
#[derive(Clone)]
pub struct FixedString<const N: usize> {
    data: [u8; N],
    len: usize,
}
impl<const N: usize> Default for FixedString<N> {
    fn default() -> Self {
        Self::new()
    }
}
impl<const N: usize> FixedString<N> {
    pub const fn new() -> Self {
        Self {
            data: [0; N],
            len: 0,
        }
    }
    pub fn as_str(&self) -> &str {
        // SAFETY: only valid UTF-8 appends and character-boundary pops change len.
        unsafe { core::str::from_utf8_unchecked(&self.data[..self.len]) }
    }
    pub const fn len(&self) -> usize {
        self.len
    }
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub const fn capacity(&self) -> usize {
        N
    }
    pub fn clear(&mut self) {
        self.len = 0;
    }
    pub fn push_str(&mut self, s: &str) -> Result<(), CapacityError> {
        if s.len() > N - self.len {
            return Err(CapacityError);
        }
        self.data[self.len..self.len + s.len()].copy_from_slice(s.as_bytes());
        self.len += s.len();
        Ok(())
    }
    pub fn push(&mut self, ch: char) -> Result<(), CapacityError> {
        let mut buf = [0; 4];
        self.push_str(ch.encode_utf8(&mut buf))
    }
    pub fn pop(&mut self) -> Option<char> {
        let ch = self.as_str().chars().next_back()?;
        self.len -= ch.len_utf8();
        Some(ch)
    }
}
impl<const N: usize> core::fmt::Write for FixedString<N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.push_str(s).map_err(|_| core::fmt::Error)
    }
}
