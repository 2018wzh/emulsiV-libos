//! Clipped graphics, a compact 3x5 font, and a 128-byte monochrome back buffer.
use crate::bus::{RegisterIo, FRAMEBUFFER};
pub const WIDTH: u16 = 32;
pub const HEIGHT: u16 = 32;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct Color(u8);
impl Color {
    pub const BLACK: Self = Self(0x00);
    pub const BLUE: Self = Self(0x03);
    pub const GREEN: Self = Self(0x1c);
    pub const CYAN: Self = Self(0x1f);
    pub const RED: Self = Self(0xe0);
    pub const MAGENTA: Self = Self(0xe3);
    pub const YELLOW: Self = Self(0xfc);
    pub const WHITE: Self = Self(0xff);
    /// Native RGB332: red bits 7..5, green bits 4..2, blue bits 1..0.
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }
    /// Select the eight saturated primary/secondary colors.
    pub const fn from_rgb(red: bool, green: bool, blue: bool) -> Self {
        Self(
            (if red { 0xe0 } else { 0 })
                | (if green { 0x1c } else { 0 })
                | (if blue { 3 } else { 0 }),
        )
    }
    /// Quantize 8-bit channels to the native 3/3/2-bit representation.
    pub const fn from_rgb888(red: u8, green: u8, blue: u8) -> Self {
        Self((red & 0xe0) | ((green >> 3) & 0x1c) | (blue >> 6))
    }
    pub const fn bits(self) -> u8 {
        self.0
    }
}
pub trait PixelTarget {
    fn size(&self) -> (u16, u16);
    /// Coordinates outside the surface must be ignored.
    fn pixel(&mut self, x: i16, y: i16, color: Color);
}
pub struct Bitmap<'a, B: RegisterIo> {
    bus: &'a mut B,
}
impl<'a, B: RegisterIo> Bitmap<'a, B> {
    pub fn new(bus: &'a mut B) -> Self {
        Self { bus }
    }
    pub fn clear(&mut self, color: Color) {
        for offset in 0..1024 {
            self.bus.write8(FRAMEBUFFER + offset, color.bits());
        }
    }
    pub fn get_pixel(&mut self, x: i16, y: i16) -> Option<Color> {
        if !(0..32).contains(&x) || !(0..32).contains(&y) {
            return None;
        }
        Some(Color::from_bits(
            self.bus.read8(FRAMEBUFFER + y as usize * 32 + x as usize),
        ))
    }
    /// Scroll upward in place without allocating a second framebuffer.
    pub fn scroll_up(&mut self, rows: u8, background: Color) {
        let shift = usize::from(rows.min(32)) * 32;
        for i in 0..1024 - shift {
            let value = self.bus.read8(FRAMEBUFFER + i + shift);
            self.bus.write8(FRAMEBUFFER + i, value);
        }
        for i in 1024 - shift..1024 {
            self.bus.write8(FRAMEBUFFER + i, background.bits());
        }
    }
}
impl<B: RegisterIo> PixelTarget for Bitmap<'_, B> {
    fn size(&self) -> (u16, u16) {
        (WIDTH, HEIGHT)
    }
    fn pixel(&mut self, x: i16, y: i16, c: Color) {
        if (0..32).contains(&x) && (0..32).contains(&y) {
            self.bus
                .write8(FRAMEBUFFER + y as usize * 32 + x as usize, c.bits());
        }
    }
}
fn plot<T: PixelTarget>(t: &mut T, x: i32, y: i32, c: Color) {
    let (w, h) = t.size();
    if x >= 0
        && y >= 0
        && x < i32::from(w)
        && y < i32::from(h)
        && x <= i32::from(i16::MAX)
        && y <= i32::from(i16::MAX)
    {
        t.pixel(x as i16, y as i16, c);
    }
}
/// Bresenham for every octant. Offscreen samples are clipped; extreme i16
/// endpoints can take up to 65536 iterations, so prefer near-screen coordinates.
pub fn line<T: PixelTarget>(t: &mut T, x0: i16, y0: i16, x1: i16, y1: i16, c: Color) {
    let (mut x, mut y, x1, y1) = (i32::from(x0), i32::from(y0), i32::from(x1), i32::from(y1));
    let dx = (x1 - x).abs();
    let dy = -(y1 - y).abs();
    let sx = if x < x1 { 1 } else { -1 };
    let sy = if y < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        plot(t, x, y, c);
        if x == x1 && y == y1 {
            break;
        }
        let twice = err * 2;
        if twice >= dy {
            err += dy;
            x += sx;
        }
        if twice <= dx {
            err += dx;
            y += sy;
        }
    }
}
pub fn fill_rect<T: PixelTarget>(t: &mut T, x: i16, y: i16, w: u16, h: u16, c: Color) {
    let (tw, th) = t.size();
    let right = (i32::from(x) + i32::from(w)).min(i32::from(tw)).min(32768);
    let bottom = (i32::from(y) + i32::from(h)).min(i32::from(th)).min(32768);
    for yy in i32::from(y).max(0)..bottom {
        for xx in i32::from(x).max(0)..right {
            t.pixel(xx as i16, yy as i16, c);
        }
    }
}
pub fn rect<T: PixelTarget>(t: &mut T, x: i16, y: i16, w: u16, h: u16, c: Color) {
    if w == 0 || h == 0 {
        return;
    }
    fill_rect(t, x, y, w, 1, c);
    fill_rect(t, x, y, 1, h, c);
    let bottom = i32::from(y) + i32::from(h) - 1;
    let right = i32::from(x) + i32::from(w) - 1;
    if bottom <= i32::from(i16::MAX) {
        fill_rect(t, x, bottom as i16, w, 1, c);
    }
    if right <= i32::from(i16::MAX) {
        fill_rect(t, right as i16, y, 1, h, c);
    }
}
pub fn circle<T: PixelTarget>(t: &mut T, cx: i16, cy: i16, radius: u8, c: Color) {
    let (cx, cy) = (i32::from(cx), i32::from(cy));
    let (mut x, mut y, mut err) = (i32::from(radius), 0, 1 - i32::from(radius));
    while x >= y {
        for (dx, dy) in [
            (x, y),
            (y, x),
            (-y, x),
            (-x, y),
            (-x, -y),
            (-y, -x),
            (y, -x),
            (x, -y),
        ] {
            plot(t, cx + dx, cy + dy, c);
        }
        y += 1;
        if err < 0 {
            err += 2 * y + 1;
        } else {
            x -= 1;
            err += 2 * (y - x) + 1;
        }
    }
}
/// Original 3x5 glyphs, row-major, most-significant row first.
fn glyph(ch: u8) -> u16 {
    match ch.to_ascii_uppercase() {
        b'0' => 0b111_101_101_101_111,
        b'1' => 0b010_110_010_010_111,
        b'2' => 0b110_001_010_100_111,
        b'3' => 0b110_001_010_001_110,
        b'4' => 0b101_101_111_001_001,
        b'5' => 0b111_100_110_001_110,
        b'6' => 0b011_100_111_101_111,
        b'7' => 0b111_001_010_010_010,
        b'8' => 0b111_101_111_101_111,
        b'9' => 0b111_101_111_001_110,
        b'A' => 0b010_101_111_101_101,
        b'B' => 0b110_101_110_101_110,
        b'C' => 0b011_100_100_100_011,
        b'D' => 0b110_101_101_101_110,
        b'E' => 0b111_100_110_100_111,
        b'F' => 0b111_100_110_100_100,
        b'G' => 0b011_100_101_101_011,
        b'H' => 0b101_101_111_101_101,
        b'I' => 0b111_010_010_010_111,
        b'J' => 0b001_001_001_101_010,
        b'K' => 0b101_101_110_101_101,
        b'L' => 0b100_100_100_100_111,
        b'M' => 0b101_111_111_101_101,
        b'N' => 0b101_111_111_111_101,
        b'O' => 0b010_101_101_101_010,
        b'P' => 0b110_101_110_100_100,
        b'Q' => 0b010_101_101_111_011,
        b'R' => 0b110_101_110_101_101,
        b'S' => 0b011_100_010_001_110,
        b'T' => 0b111_010_010_010_010,
        b'U' => 0b101_101_101_101_111,
        b'V' => 0b101_101_101_101_010,
        b'W' => 0b101_101_111_111_101,
        b'X' => 0b101_101_010_101_101,
        b'Y' => 0b101_101_010_010_010,
        b'Z' => 0b111_001_010_100_111,
        b' ' => 0,
        b'-' => 0b000_000_111_000_000,
        b'_' => 0b000_000_000_000_111,
        b'.' => 1,
        b':' => 0b000_010_000_010_000,
        b'!' => 0b010_010_010_000_010,
        _ => 0b110_001_010_000_010,
    }
}
pub fn draw_char<T: PixelTarget>(t: &mut T, x: i16, y: i16, ch: u8, c: Color) {
    let bits = glyph(ch);
    for row in 0..5 {
        for col in 0..3 {
            if bits & (1 << (14 - row * 3 - col)) != 0 {
                plot(t, i32::from(x) + col, i32::from(y) + row, c);
            }
        }
    }
}
/// ASCII text. Newline advances six rows; glyphs are four columns apart.
pub fn text<T: PixelTarget>(t: &mut T, x: i16, y: i16, s: &str, c: Color) {
    let (mut xx, mut yy) = (x, y);
    for ch in s.bytes() {
        if ch == b'\n' {
            xx = x;
            yy = yy.saturating_add(6);
        } else {
            draw_char(t, xx, yy, ch, c);
            xx = xx.saturating_add(4);
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortSprite;
/// Row-padded, MSB-first 1-bit sprite. `None` background means transparency.
#[allow(clippy::too_many_arguments)]
pub fn blit_mono<T: PixelTarget>(
    t: &mut T,
    x: i16,
    y: i16,
    w: u8,
    h: u8,
    data: &[u8],
    foreground: Color,
    background: Option<Color>,
) -> Result<(), ShortSprite> {
    let stride = usize::from(w).div_ceil(8);
    if data.len() < stride * usize::from(h) {
        return Err(ShortSprite);
    }
    for row in 0..usize::from(h) {
        for col in 0..usize::from(w) {
            let on = data[row * stride + col / 8] & (0x80 >> (col % 8)) != 0;
            if let Some(c) = if on { Some(foreground) } else { background } {
                plot(t, i32::from(x) + col as i32, i32::from(y) + row as i32, c);
            }
        }
    }
    Ok(())
}
/// One bit per pixel, 128 bytes. Non-black colors become set bits.
pub struct MonoBuffer {
    bits: [u8; 128],
}
impl Default for MonoBuffer {
    fn default() -> Self {
        Self::new()
    }
}
impl MonoBuffer {
    pub const fn new() -> Self {
        Self { bits: [0; 128] }
    }
    pub fn clear(&mut self) {
        self.bits.fill(0);
    }
    pub fn bytes(&self) -> &[u8; 128] {
        &self.bits
    }
    pub fn present<T: PixelTarget>(&self, t: &mut T, fg: Color, bg: Color) {
        for y in 0..32 {
            for x in 0..32 {
                let index = y * 32 + x;
                let on = self.bits[index / 8] & (0x80 >> (index % 8)) != 0;
                t.pixel(x as i16, y as i16, if on { fg } else { bg });
            }
        }
    }
}
impl PixelTarget for MonoBuffer {
    fn size(&self) -> (u16, u16) {
        (32, 32)
    }
    fn pixel(&mut self, x: i16, y: i16, c: Color) {
        if !(0..32).contains(&x) || !(0..32).contains(&y) {
            return;
        }
        let index = y as usize * 32 + x as usize;
        let mask = 0x80 >> (index % 8);
        if c == Color::BLACK {
            self.bits[index / 8] &= !mask;
        } else {
            self.bits[index / 8] |= mask;
        }
    }
}
