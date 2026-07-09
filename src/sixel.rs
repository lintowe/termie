//! sixel graphics decoder (DCS q). bytes stream in through the vte DCS hooks
//! and paint a growing RGBA canvas; `finish` hands back the pixels for the
//! image store. pixels a stream never touches stay transparent — over the
//! cell background that reads the same for both P2 background modes, so P2
//! isn't tracked. P1 (the legacy pixel aspect ratio) is ignored like every
//! modern encoder assumes

/// per-axis cap; 4096x4096 RGBA = 64 MB, the same ceiling the kitty path has,
/// so a hostile stream can't grow the canvas past one image budget. public
/// because XTSMGRAPHICS reports these as the max sixel geometry
pub const MAX_W: usize = 4096;
pub const MAX_H: usize = 4096;

/// the VT340 default color registers 0-15 as percentages, the palette a
/// stream inherits before its own `#` definitions land
const VT340: [(u32, u32, u32); 16] = [
    (0, 0, 0),
    (20, 20, 80),
    (80, 13, 13),
    (20, 80, 20),
    (80, 20, 80),
    (20, 80, 80),
    (80, 80, 20),
    (53, 53, 53),
    (26, 26, 26),
    (33, 33, 60),
    (60, 26, 26),
    (33, 60, 33),
    (60, 33, 60),
    (33, 60, 60),
    (60, 60, 33),
    (80, 80, 80),
];

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Ground,
    /// `!` repeat introducer, collecting the count
    Repeat,
    /// `#` color introducer, collecting register / colorspace params
    Color,
    /// `"` raster attributes, collecting Pan;Pad;Ph;Pv
    Raster,
}

pub struct SixelDecoder {
    palette: [(u8, u8, u8); 256],
    color: usize,
    /// straight RGBA, `alloc_w * alloc_h`, grown geometrically as bands land
    canvas: Vec<u8>,
    alloc_w: usize,
    alloc_h: usize,
    x: usize,
    /// top row of the current six-pixel band
    y: usize,
    max_x: usize,
    max_y: usize,
    /// size the stream declared via raster attributes (`"`)
    raster_w: usize,
    raster_h: usize,
    /// pending `!` count, consumed by the next data character
    repeat: u32,
    mode: Mode,
    params: Vec<u32>,
    cur: u32,
}

impl Default for SixelDecoder {
    fn default() -> Self {
        let mut palette = [(0u8, 0u8, 0u8); 256];
        for (i, &(r, g, b)) in VT340.iter().enumerate() {
            palette[i] = (pct(r), pct(g), pct(b));
        }
        SixelDecoder {
            palette,
            color: 0,
            canvas: Vec::new(),
            alloc_w: 0,
            alloc_h: 0,
            x: 0,
            y: 0,
            max_x: 0,
            max_y: 0,
            raster_w: 0,
            raster_h: 0,
            repeat: 0,
            mode: Mode::Ground,
            params: Vec::new(),
            cur: 0,
        }
    }
}

impl SixelDecoder {
    pub fn put(&mut self, byte: u8) {
        if self.mode != Mode::Ground {
            match byte {
                b'0'..=b'9' => {
                    self.cur = self.cur.saturating_mul(10).saturating_add((byte - b'0') as u32);
                    return;
                }
                b';' => {
                    self.push_param();
                    return;
                }
                _ => {
                    self.push_param();
                    self.apply_params();
                    // the terminator is itself the next command byte
                }
            }
        }
        match byte {
            b'!' => self.begin_params(Mode::Repeat),
            b'#' => self.begin_params(Mode::Color),
            b'"' => self.begin_params(Mode::Raster),
            b'$' => self.x = 0,
            b'-' => {
                self.y = (self.y + 6).min(MAX_H);
                self.x = 0;
            }
            0x3f..=0x7e => self.draw(byte),
            _ => {}
        }
    }

    /// decode is over (ST): flush any half-collected params and hand back the
    /// pixels as straight RGBA; None when the stream drew nothing
    pub fn finish(mut self) -> Option<(u32, u32, Vec<u8>)> {
        if self.mode != Mode::Ground {
            self.push_param();
            self.apply_params();
        }
        let w = self.max_x.max(self.raster_w).min(MAX_W);
        let h = self.max_y.max(self.raster_h).min(MAX_H);
        if w == 0 || h == 0 {
            return None;
        }
        let mut out = vec![0u8; w * h * 4];
        let copy_w = self.alloc_w.min(w);
        for row in 0..self.max_y.min(h) {
            let src = row * self.alloc_w * 4;
            let dst = row * w * 4;
            out[dst..dst + copy_w * 4].copy_from_slice(&self.canvas[src..src + copy_w * 4]);
        }
        Some((w as u32, h as u32, out))
    }

    fn begin_params(&mut self, mode: Mode) {
        self.mode = mode;
        self.params.clear();
        self.cur = 0;
    }

    fn push_param(&mut self) {
        if self.params.len() < 8 {
            self.params.push(self.cur);
        }
        self.cur = 0;
    }

    fn apply_params(&mut self) {
        match self.mode {
            Mode::Repeat => self.repeat = self.params.first().copied().unwrap_or(0),
            Mode::Color => {
                let idx = self.params.first().copied().unwrap_or(0) as usize % self.palette.len();
                if self.params.len() >= 5 {
                    let (pu, a, b, c) = (self.params[1], self.params[2], self.params[3], self.params[4]);
                    match pu {
                        2 => self.palette[idx] = (pct(a), pct(b), pct(c)),
                        1 => self.palette[idx] = hls_to_rgb(a, b, c),
                        _ => {}
                    }
                }
                self.color = idx;
            }
            Mode::Raster => {
                // Pan;Pad (aspect) ignored; Ph;Pv declare the image box, which
                // pads the final image with transparency if nothing draws there
                self.raster_w = (self.params.get(2).copied().unwrap_or(0) as usize).min(MAX_W);
                self.raster_h = (self.params.get(3).copied().unwrap_or(0) as usize).min(MAX_H);
            }
            Mode::Ground => {}
        }
        self.mode = Mode::Ground;
    }

    /// one data character: six vertical pixels (bit 0 on top), repeated
    /// `repeat` columns. an empty char ('?') still advances and widens the
    /// image, it just leaves the columns transparent
    fn draw(&mut self, ch: u8) {
        let bits = ch - 0x3f;
        let n = std::mem::take(&mut self.repeat).max(1) as usize;
        let x0 = self.x.min(MAX_W);
        let x1 = self.x.saturating_add(n).min(MAX_W);
        if bits != 0 && x1 > x0 && self.y < MAX_H {
            let top_bit = 7 - (bits.leading_zeros() as usize);
            let y1 = (self.y + top_bit + 1).min(MAX_H);
            self.ensure(x1, y1);
            let (r, g, b) = self.palette[self.color];
            let px = [r, g, b, 255];
            for j in 0..6 {
                if bits & (1 << j) == 0 {
                    continue;
                }
                let py = self.y + j;
                if py >= self.alloc_h {
                    break;
                }
                let row = py * self.alloc_w;
                for xi in x0..x1 {
                    let o = (row + xi) * 4;
                    self.canvas[o..o + 4].copy_from_slice(&px);
                }
            }
            self.max_y = self.max_y.max(y1);
        }
        self.x = x1;
        self.max_x = self.max_x.max(self.x);
    }

    /// grow the canvas (geometrically, capped) so `w x h` is addressable
    fn ensure(&mut self, w: usize, h: usize) {
        let w = w.min(MAX_W);
        let h = h.min(MAX_H);
        if w <= self.alloc_w && h <= self.alloc_h {
            return;
        }
        let nw = self.alloc_w.max(w).max(64).next_power_of_two().min(MAX_W);
        let nh = self.alloc_h.max(h).max(64).next_power_of_two().min(MAX_H);
        let mut next = vec![0u8; nw * nh * 4];
        for row in 0..self.max_y {
            let src = row * self.alloc_w * 4;
            let dst = row * nw * 4;
            next[dst..dst + self.alloc_w * 4].copy_from_slice(&self.canvas[src..src + self.alloc_w * 4]);
        }
        self.canvas = next;
        self.alloc_w = nw;
        self.alloc_h = nh;
    }
}

/// a DEC percentage (0-100) widened to a byte
fn pct(v: u32) -> u8 {
    ((v.min(100) * 255 + 50) / 100) as u8
}

/// DEC HLS to RGB. DEC hue 0 is blue (120 red, 240 green), so rotate onto the
/// standard HSL wheel first; L and S are percentages
fn hls_to_rgb(h: u32, l: u32, s: u32) -> (u8, u8, u8) {
    let h = ((h % 360) + 240) % 360;
    let l = l.min(100) as f32 / 100.0;
    let s = s.min(100) as f32 / 100.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h as f32 / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to = |v: f32| ((v + m).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    (to(r1), to(g1), to(b1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(data: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
        let mut d = SixelDecoder::default();
        for &b in data {
            d.put(b);
        }
        d.finish()
    }

    fn px(rgba: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
        let o = ((y * w + x) * 4) as usize;
        rgba[o..o + 4].try_into().unwrap()
    }

    #[test]
    fn draws_a_column_in_a_defined_color() {
        // define register 1 as pure red, select it, draw '~' (all six bits)
        let (w, h, rgba) = decode(b"#1;2;100;0;0~").expect("image");
        assert_eq!((w, h), (1, 6));
        for y in 0..6 {
            assert_eq!(px(&rgba, w, 0, y), [255, 0, 0, 255]);
        }
    }

    #[test]
    fn repeat_spans_columns_and_blank_advances() {
        // !3 repeats '@' (bit 0 only) across three columns; '?' advances one
        // blank column that still counts toward the width
        let (w, h, rgba) = decode(b"#1;2;0;100;0!3@?").expect("image");
        assert_eq!((w, h), (4, 1));
        for x in 0..3 {
            assert_eq!(px(&rgba, w, x, 0), [0, 255, 0, 255]);
        }
        assert_eq!(px(&rgba, w, 3, 0), [0, 0, 0, 0], "blank column is transparent");
    }

    #[test]
    fn dollar_rewinds_and_dash_advances_a_band() {
        // draw bit0 in col 0, CR, overdraw bit1 same column, then next band
        let (w, h, rgba) = decode(b"#1;2;100;0;0@$#2;2;0;0;100A-@").expect("image");
        assert_eq!((w, h), (1, 7));
        assert_eq!(px(&rgba, w, 0, 0), [255, 0, 0, 255]); // band 0 bit 0
        assert_eq!(px(&rgba, w, 0, 1), [0, 0, 255, 255]); // band 0 bit 1 after $
        assert_eq!(px(&rgba, w, 0, 6), [0, 0, 255, 255]); // band 1 bit 0, register 2 still selected
    }

    #[test]
    fn raster_attributes_pad_the_image() {
        // declares 4x8 but only draws one pixel; the rest pads transparent
        let (w, h, rgba) = decode(b"\"1;1;4;8#1;2;100;100;100@").expect("image");
        assert_eq!((w, h), (4, 8));
        assert_eq!(px(&rgba, w, 0, 0), [255, 255, 255, 255]);
        assert_eq!(px(&rgba, w, 3, 7), [0, 0, 0, 0]);
    }

    #[test]
    fn dec_hls_hue_is_blue_rotated() {
        // DEC hue 120 = red at L=50, S=100
        let (_, _, rgba) = decode(b"#0;1;120;50;100~").expect("image");
        assert_eq!(px(&rgba, 1, 0, 0), [255, 0, 0, 255]);
        // DEC hue 0 = blue
        let (_, _, rgba) = decode(b"#0;1;0;50;100~").expect("image");
        assert_eq!(px(&rgba, 1, 0, 0), [0, 0, 255, 255]);
    }

    #[test]
    fn vt340_default_palette_register_2_is_red() {
        let (_, _, rgba) = decode(b"#2~").expect("image");
        let [r, g, b, a] = px(&rgba, 1, 0, 0);
        assert!(r > 150 && g < 80 && b < 80 && a == 255, "got {:?}", (r, g, b, a));
    }

    #[test]
    fn hostile_repeat_clamps_at_the_cap() {
        // a 4-billion repeat must clamp to MAX_W, not OOM or panic
        let (w, h, _) = decode(b"#1;2;100;0;0!4000000000~").expect("image");
        assert_eq!((w as usize, h), (MAX_W, 6));
    }

    #[test]
    fn hostile_band_count_clamps_at_the_cap() {
        let mut d = SixelDecoder::default();
        for _ in 0..(MAX_H / 6 + 50) {
            d.put(b'~');
            d.put(b'-');
        }
        let (_, h, _) = d.finish().expect("image");
        assert_eq!(h as usize, MAX_H);
    }

    #[test]
    fn empty_stream_yields_nothing() {
        assert!(decode(b"").is_none());
        assert!(decode(b"#1;2;100;0;0").is_none(), "color def alone draws nothing");
    }

    #[test]
    fn params_split_across_puts_and_unterminated_raster_flushes() {
        // raster attrs with no trailing data char still apply via finish()
        let (w, h, _) = decode(b"\"1;1;7;9").expect("image");
        assert_eq!((w, h), (7, 9));
    }
}
