//! ASCII line editing: CR/LF, backspace, Delete, Ctrl-U and Ctrl-C.
//! Overflowed lines are rejected as a whole, never executed as truncated commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEvent {
    None,
    Echo(u8),
    Erase,
    Cleared,
    Complete,
    Overflow,
    Cancelled,
}
pub struct LineEditor<const N: usize> {
    data: [u8; N],
    len: usize,
    overflow: bool,
    ready: bool,
    after_cr: bool,
}
impl<const N: usize> Default for LineEditor<N> {
    fn default() -> Self {
        Self::new()
    }
}
impl<const N: usize> LineEditor<N> {
    pub const fn new() -> Self {
        Self {
            data: [0; N],
            len: 0,
            overflow: false,
            ready: false,
            after_cr: false,
        }
    }
    pub fn line(&self) -> &str {
        // SAFETY: only printable ASCII is stored.
        unsafe { core::str::from_utf8_unchecked(self.data.get(..self.len).unwrap_or_default()) }
    }
    pub fn clear(&mut self) {
        self.len = 0;
        self.overflow = false;
        self.ready = false;
    }
    pub fn feed(&mut self, b: u8) -> LineEvent {
        if b == b'\n' && self.after_cr {
            self.after_cr = false;
            return LineEvent::None;
        }
        self.after_cr = b == b'\r';
        if self.ready {
            self.clear();
        }
        match b {
            b'\r' | b'\n' => {
                self.ready = true;
                if self.overflow {
                    LineEvent::Overflow
                } else {
                    LineEvent::Complete
                }
            }
            3 => {
                self.clear();
                LineEvent::Cancelled
            }
            21 => {
                self.clear();
                LineEvent::Cleared
            }
            8 | 127 => {
                if !self.overflow && self.len > 0 {
                    self.len -= 1;
                    LineEvent::Erase
                } else {
                    LineEvent::None
                }
            }
            32..=126 => {
                if self.len >= N || self.overflow {
                    self.overflow = true;
                    LineEvent::None
                } else {
                    self.data[self.len] = b;
                    self.len += 1;
                    LineEvent::Echo(b)
                }
            }
            _ => LineEvent::None,
        }
    }
}
