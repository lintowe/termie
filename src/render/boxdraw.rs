//! Programmatic box-drawing and block-element glyphs that fill the whole cell.
//!
//! A font's box-drawing glyphs are only about one em tall, so with a line height
//! above 1.0 they leave vertical seams between stacked cells and box borders look
//! broken. These are drawn to the exact cell rectangle instead, so `│`, corners
//! and tees connect with no gaps at any line height. Codepoints outside the
//! supported set return `None` and fall back to the font glyph.

/// An alpha-coverage bitmap `w`×`h` for box/block char `c`, or `None` to fall
/// back to the font glyph. `stroke` is the light line thickness in pixels.
pub fn coverage(c: char, w: usize, h: usize, stroke: f32) -> Option<Vec<u8>> {
    // fast reject: only the box-drawing and (supported) block-element ranges
    if !matches!(c, '\u{2500}'..='\u{257F}' | '\u{2580}'..='\u{2593}') {
        return None;
    }
    let w = w.max(1);
    let h = h.max(1);
    let iw = w as i32;
    let ih = h as i32;
    let t = (stroke.round() as i32).clamp(1, ih.max(1)); // light line
    let th = ((stroke * 2.0).round() as i32).clamp(2, ih.max(2)); // heavy line
    let cx = iw / 2;
    let cy = ih / 2;
    let mut buf = vec![0u8; w * h];

    let rect = |buf: &mut [u8], x0: i32, y0: i32, x1: i32, y1: i32, a: u8| {
        for y in y0.max(0)..y1.min(ih) {
            for x in x0.max(0)..x1.min(iw) {
                let i = y as usize * w + x as usize;
                if a > buf[i] {
                    buf[i] = a;
                }
            }
        }
    };

    // ---- block elements & shades -------------------------------------------
    match c {
        '\u{2588}' => { rect(&mut buf, 0, 0, iw, ih, 255); return Some(buf); } // █ full block
        '\u{2580}' => { rect(&mut buf, 0, 0, iw, cy, 255); return Some(buf); } // ▀ upper half
        '\u{2584}' => { rect(&mut buf, 0, cy, iw, ih, 255); return Some(buf); } // ▄ lower half
        '\u{258C}' => { rect(&mut buf, 0, 0, cx, ih, 255); return Some(buf); } // ▌ left half
        '\u{2590}' => { rect(&mut buf, cx, 0, iw, ih, 255); return Some(buf); } // ▐ right half
        '\u{2591}' => { rect(&mut buf, 0, 0, iw, ih, 64); return Some(buf); }  // ░ light shade
        '\u{2592}' => { rect(&mut buf, 0, 0, iw, ih, 128); return Some(buf); } // ▒ medium shade
        '\u{2593}' => { rect(&mut buf, 0, 0, iw, ih, 191); return Some(buf); } // ▓ dark shade
        _ => {}
    }

    // ---- lines, corners, tees, cross ---------------------------------------
    let (l, r, u, d, heavy, rounded) = spec(c)?;
    let thick = if heavy { th } else { t };
    let half = thick / 2;

    if rounded {
        // exactly one horizontal + one vertical direction, joined by a quarter
        // arc so the bend stays rounded the way the font draws it
        let rad = cx.min(cy).max(thick);
        let ax = if r { cx + rad } else { cx - rad };
        let ay = if d { cy + rad } else { cy - rad };
        // straight stems run from the cell edge to where the arc begins
        if r {
            rect(&mut buf, ax, cy - half, iw, cy - half + thick, 255);
        }
        if l {
            rect(&mut buf, 0, cy - half, ax + 1, cy - half + thick, 255);
        }
        if d {
            rect(&mut buf, cx - half, ay, cx - half + thick, ih, 255);
        }
        if u {
            rect(&mut buf, cx - half, 0, cx - half + thick, ay + 1, 255);
        }
        // arc band of width `thick` centred on radius `rad`, in the quadrant
        // between the cell centre and the arc centre
        let (qx0, qx1) = (cx.min(ax), cx.max(ax));
        let (qy0, qy1) = (cy.min(ay), cy.max(ay));
        let lo = (rad as f32 - half as f32).max(0.0);
        let hi = lo + thick as f32;
        for y in qy0.max(0)..=qy1.min(ih - 1) {
            for x in qx0.max(0)..=qx1.min(iw - 1) {
                let dx = (x - ax) as f32 + 0.5;
                let dy = (y - ay) as f32 + 0.5;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist >= lo && dist < hi {
                    buf[y as usize * w + x as usize] = 255;
                }
            }
        }
        return Some(buf);
    }

    // straight stems meeting at the centre cross
    if l {
        rect(&mut buf, 0, cy - half, cx - half + thick, cy - half + thick, 255);
    }
    if r {
        rect(&mut buf, cx - half, cy - half, iw, cy - half + thick, 255);
    }
    if u {
        rect(&mut buf, cx - half, 0, cx - half + thick, cy - half + thick, 255);
    }
    if d {
        rect(&mut buf, cx - half, cy - half, cx - half + thick, ih, 255);
    }
    Some(buf)
}

/// (left, right, up, down, heavy, rounded) stems for a line/corner/tee char,
/// or `None` to fall back to the font (double, dashed and mixed-weight glyphs)
fn spec(c: char) -> Option<(bool, bool, bool, bool, bool, bool)> {
    Some(match c {
        '\u{2500}' => (true, true, false, false, false, false), // ─
        '\u{2501}' => (true, true, false, false, true, false),  // ━
        '\u{2502}' => (false, false, true, true, false, false), // │
        '\u{2503}' => (false, false, true, true, true, false),  // ┃
        '\u{250C}' => (false, true, false, true, false, false), // ┌
        '\u{250F}' => (false, true, false, true, true, false),  // ┏
        '\u{2510}' => (true, false, false, true, false, false), // ┐
        '\u{2513}' => (true, false, false, true, true, false),  // ┓
        '\u{2514}' => (false, true, true, false, false, false), // └
        '\u{2517}' => (false, true, true, false, true, false),  // ┗
        '\u{2518}' => (true, false, true, false, false, false), // ┘
        '\u{251B}' => (true, false, true, false, true, false),  // ┛
        '\u{251C}' => (false, true, true, true, false, false),  // ├
        '\u{2523}' => (false, true, true, true, true, false),   // ┣
        '\u{2524}' => (true, false, true, true, false, false),  // ┤
        '\u{252B}' => (true, false, true, true, true, false),   // ┫
        '\u{252C}' => (true, true, false, true, false, false),  // ┬
        '\u{2533}' => (true, true, false, true, true, false),   // ┳
        '\u{2534}' => (true, true, true, false, false, false),  // ┴
        '\u{253B}' => (true, true, true, false, true, false),   // ┻
        '\u{253C}' => (true, true, true, true, false, false),   // ┼
        '\u{254B}' => (true, true, true, true, true, false),    // ╋
        '\u{256D}' => (false, true, false, true, false, true),  // ╭
        '\u{256E}' => (true, false, false, true, false, true),  // ╮
        '\u{256F}' => (true, false, true, false, false, true),  // ╯
        '\u{2570}' => (false, true, true, false, false, true),  // ╰
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::coverage;

    #[test]
    fn unsupported_falls_back() {
        assert!(coverage('A', 8, 16, 1.0).is_none());
        assert!(coverage('\u{2550}', 8, 16, 1.0).is_none()); // ═ double — font glyph
    }

    #[test]
    fn full_block_fills_cell() {
        let b = coverage('\u{2588}', 8, 16, 1.0).unwrap();
        assert!(b.iter().all(|&p| p == 255));
    }

    #[test]
    fn vertical_line_spans_full_height() {
        let (w, h) = (9usize, 17usize);
        let b = coverage('\u{2502}', w, h, 1.0).unwrap(); // │
        // the centre column is lit on both the first and last row (no seam)
        let cx = w / 2;
        assert!(b[cx] > 0, "top of vertical stem must be lit");
        assert!(b[(h - 1) * w + cx] > 0, "bottom of vertical stem must be lit");
    }

    #[test]
    fn horizontal_line_spans_full_width() {
        let (w, h) = (9usize, 17usize);
        let b = coverage('\u{2500}', w, h, 1.0).unwrap(); // ─
        let cy = h / 2;
        assert!(b[cy * w] > 0, "left end of horizontal stem must be lit");
        assert!(b[cy * w + (w - 1)] > 0, "right end must be lit");
    }
}
