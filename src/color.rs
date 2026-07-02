#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Rgb { r, g, b }
    }

    /// per-channel blend toward `other` by `t` in [0,1] for ui colour eases
    pub fn lerp(self, other: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgb::new(mix(self.r, other.r), mix(self.g, other.g), mix(self.b, other.b))
    }

    pub fn to_linear_f32(self) -> [f32; 4] {
        // srgb -> linear via a precomputed 256-entry table: this runs per cell
        // per color every paint, so the per-channel powf was the hot path's main
        // transcendental cost. the table is bit-identical to the closed form
        let t = srgb_linear_lut();
        [t[self.r as usize], t[self.g as usize], t[self.b as usize], 1.0]
    }
}

/// srgb -> linear for all 256 channel values, built once on first use
fn srgb_linear_lut() -> &'static [f32; 256] {
    use std::sync::OnceLock;
    static LUT: OnceLock<[f32; 256]> = OnceLock::new();
    LUT.get_or_init(|| {
        let mut t = [0.0f32; 256];
        let mut i = 0;
        while i < 256 {
            let s = i as f32 / 255.0;
            t[i] = if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            };
            i += 1;
        }
        t
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Color {
    #[default]
    Default,
    DefaultBg,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeId {
    Instrument,
    Koi,
    Paper,
    Catppuccin,
    Gruvbox,
    TokyoNight,
    Nord,
}

impl ThemeId {
    pub const ALL: [ThemeId; 7] = [
        ThemeId::Instrument,
        ThemeId::Koi,
        ThemeId::Paper,
        ThemeId::Catppuccin,
        ThemeId::Gruvbox,
        ThemeId::TokyoNight,
        ThemeId::Nord,
    ];

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|&t| t == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    pub fn name(self) -> &'static str {
        match self {
            ThemeId::Instrument => "instrument",
            ThemeId::Koi => "koi",
            ThemeId::Paper => "paper",
            ThemeId::Catppuccin => "catppuccin",
            ThemeId::Gruvbox => "gruvbox",
            ThemeId::TokyoNight => "tokyo night",
            ThemeId::Nord => "nord",
        }
    }

    pub fn from_name(s: &str) -> Self {
        match s {
            "koi" => ThemeId::Koi,
            "paper" => ThemeId::Paper,
            "catppuccin" => ThemeId::Catppuccin,
            "gruvbox" => ThemeId::Gruvbox,
            "tokyo night" | "tokyonight" => ThemeId::TokyoNight,
            "nord" => ThemeId::Nord,
            _ => ThemeId::Instrument,
        }
    }
}

/// full palette: chrome ladder + terminal colors for one theme
pub struct Palette {
    // chrome ladder (dark→light for dark themes)
    pub ink0: Rgb,
    pub ink1: Rgb,
    pub ink3: Rgb,
    pub ink4: Rgb,
    pub rule: Rgb,
    pub rule2: Rgb,
    pub mute: Rgb,
    pub text2: Rgb,
    pub paper: Rgb, // high-contrast accent (white on dark, dark on light, ember on koi)
    // terminal content
    pub fg: Rgb,
    pub bg: Rgb,
    pub bg2: Rgb,  // gradient companion to bg (subtle vertical wash)
    pub cursor: Rgb,
    pub sel: Rgb,  // text-selection tint
    ansi: [Rgb; 256],
}

impl Palette {
    pub fn from_theme(id: ThemeId) -> Self {
        match id {
            ThemeId::Instrument => Self::instrument(),
            ThemeId::Koi => Self::koi(),
            ThemeId::Paper => Self::paper(),
            ThemeId::Catppuccin => Self::catppuccin(),
            ThemeId::Gruvbox => Self::gruvbox(),
            ThemeId::TokyoNight => Self::tokyo_night(),
            ThemeId::Nord => Self::nord(),
        }
    }

    pub fn resolve_fg(&self, c: Color) -> Rgb {
        match c {
            Color::Default => self.fg,
            Color::DefaultBg => self.bg,
            Color::Indexed(i) => self.ansi[i as usize],
            Color::Rgb(r, g, b) => Rgb::new(r, g, b),
        }
    }

    pub fn resolve_bg(&self, c: Color) -> Rgb {
        match c {
            Color::Default | Color::DefaultBg => self.bg,
            Color::Indexed(i) => self.ansi[i as usize],
            Color::Rgb(r, g, b) => Rgb::new(r, g, b),
        }
    }

    /// bold text in a basic ansi color (0-7) maps to its bright variant (8-15)
    /// when enabled — the common xterm "bold = bright" behavior
    pub fn bold_bright(fg: Color, bold: bool, enabled: bool) -> Color {
        if enabled
            && bold
            && let Color::Indexed(i) = fg
            && i < 8
        {
            Color::Indexed(i + 8)
        } else {
            fg
        }
    }

    pub fn ansi_color(&self, i: u8) -> Rgb {
        self.ansi[i as usize]
    }

    /// patch palette fields by name from user color overrides (ansi0..ansi255
    /// target the 256-color table); unknown keys are ignored
    pub fn apply_overrides(&mut self, overrides: &[(String, Rgb)]) {
        for (k, c) in overrides {
            let c = *c;
            match k.as_str() {
                "fg" => self.fg = c,
                "bg" => self.bg = c,
                "bg2" => self.bg2 = c,
                "cursor" => self.cursor = c,
                "sel" => self.sel = c,
                "paper" => self.paper = c,
                "mute" => self.mute = c,
                "text2" => self.text2 = c,
                "ink0" => self.ink0 = c,
                "ink1" => self.ink1 = c,
                "ink3" => self.ink3 = c,
                "ink4" => self.ink4 = c,
                "rule" => self.rule = c,
                "rule2" => self.rule2 = c,
                _ => {
                    if let Some(n) = k.strip_prefix("ansi").and_then(|n| n.parse::<usize>().ok())
                        && n < 256
                    {
                        self.ansi[n] = c;
                    }
                }
            }
        }
    }

    fn instrument() -> Self {
        // restrained "instrument panel" signal colors on a hard greyscale ground:
        // clay / sage / brass / steel / mauve / teal, paired normal→bright
        let base16 = [
            Rgb::new(0x1a, 0x1a, 0x1a),
            Rgb::new(0xbf, 0x63, 0x60),
            Rgb::new(0x83, 0xa0, 0x6d),
            Rgb::new(0xc6, 0xa6, 0x67),
            Rgb::new(0x64, 0x86, 0xa6),
            Rgb::new(0xa0, 0x74, 0x8f),
            Rgb::new(0x66, 0xa3, 0xa0),
            Rgb::new(0xc8, 0xc8, 0xc8),
            Rgb::new(0x4f, 0x4f, 0x4f),
            Rgb::new(0xe0, 0x8a, 0x86),
            Rgb::new(0x9d, 0xbe, 0x86),
            Rgb::new(0xe6, 0xcd, 0x86),
            Rgb::new(0x8a, 0xa8, 0xcc),
            Rgb::new(0xc5, 0x96, 0xb4),
            Rgb::new(0x84, 0xc6, 0xc2),
            Rgb::new(0xf0, 0xf0, 0xf0),
        ];
        Palette {
            ink0: Rgb::new(0x05, 0x05, 0x05),
            ink1: Rgb::new(0x0d, 0x0d, 0x0d),
            ink3: Rgb::new(0x1c, 0x1c, 0x1c),
            ink4: Rgb::new(0x26, 0x26, 0x26),
            rule: Rgb::new(0x2a, 0x2a, 0x2a),
            rule2: Rgb::new(0x3a, 0x3a, 0x3a),
            mute: Rgb::new(0x6f, 0x6f, 0x6f),
            text2: Rgb::new(0xed, 0xed, 0xed),
            paper: Rgb::new(0xf5, 0xf5, 0xf5),
            fg: Rgb::new(0xc8, 0xc8, 0xc8),
            bg: Rgb::new(0x14, 0x14, 0x14),
            bg2: Rgb::new(0x0d, 0x0e, 0x10),
            cursor: Rgb::new(0xf5, 0xf5, 0xf5),
            sel: Rgb::new(0x35, 0x52, 0x7a),
            ansi: fill_ansi(base16),
        }
    }

    fn koi() -> Self {
        // warm ember world: every hue pulled toward the fire, greens olive,
        // blues dusty, so the ff5b22 accent feels native
        let base16 = [
            Rgb::new(0x24, 0x1e, 0x16),
            Rgb::new(0xd1, 0x51, 0x2f),
            Rgb::new(0x7c, 0x96, 0x56),
            Rgb::new(0xd4, 0x9a, 0x3f),
            Rgb::new(0x5f, 0x82, 0x90),
            Rgb::new(0xa9, 0x72, 0x8c),
            Rgb::new(0x4f, 0x9b, 0x95),
            Rgb::new(0xd9, 0xce, 0xb9),
            Rgb::new(0x58, 0x4d, 0x3e),
            Rgb::new(0xff, 0x6a, 0x33),
            Rgb::new(0x99, 0xb5, 0x6e),
            Rgb::new(0xe8, 0xbe, 0x63),
            Rgb::new(0x84, 0xa0, 0xad),
            Rgb::new(0xc7, 0x94, 0xa6),
            Rgb::new(0x6c, 0xb0, 0xa8),
            Rgb::new(0xee, 0xe3, 0xce),
        ];
        Palette {
            ink0: Rgb::new(0x0a, 0x08, 0x06),
            ink1: Rgb::new(0x14, 0x11, 0x0d),
            ink3: Rgb::new(0x22, 0x1d, 0x17),
            ink4: Rgb::new(0x2e, 0x27, 0x1f),
            rule: Rgb::new(0x2e, 0x28, 0x20),
            rule2: Rgb::new(0x40, 0x37, 0x2c),
            mute: Rgb::new(0x8a, 0x7d, 0x6a),
            text2: Rgb::new(0xec, 0xe2, 0xcd),
            paper: Rgb::new(0xff, 0x5b, 0x22),
            fg: Rgb::new(0xd8, 0xcd, 0xb8),
            bg: Rgb::new(0x1a, 0x16, 0x12),
            bg2: Rgb::new(0x12, 0x0d, 0x09),
            cursor: Rgb::new(0xff, 0x5b, 0x22),
            sel: Rgb::new(0x5a, 0x3a, 0x24),
            ansi: fill_ansi(base16),
        }
    }

    fn paper() -> Self {
        // printed-ink colors on warm paper: vermilion / moss / ochre / indigo /
        // plum / pine, saturated enough to read as ink, not pastel
        let base16 = [
            Rgb::new(0x39, 0x34, 0x2b),
            Rgb::new(0xa8, 0x3a, 0x28),
            Rgb::new(0x4f, 0x6e, 0x34),
            Rgb::new(0x9a, 0x6e, 0x1c),
            Rgb::new(0x35, 0x60, 0x8a),
            Rgb::new(0x85, 0x46, 0x70),
            Rgb::new(0x2f, 0x6e, 0x6a),
            Rgb::new(0x4a, 0x46, 0x3e),
            Rgb::new(0x6a, 0x64, 0x56),
            Rgb::new(0xc2, 0x4a, 0x30),
            Rgb::new(0x5f, 0x85, 0x40),
            Rgb::new(0xb5, 0x86, 0x2a),
            Rgb::new(0x3f, 0x6e, 0xa0),
            Rgb::new(0x9a, 0x56, 0x80),
            Rgb::new(0x3a, 0x84, 0x80),
            Rgb::new(0x26, 0x20, 0x19),
        ];
        Palette {
            // on a light theme the "ink ladder" runs light→darker for chrome surfaces
            ink0: Rgb::new(0xd2, 0xcb, 0xbc), // statusbar
            ink1: Rgb::new(0xde, 0xd8, 0xcb), // titlebar
            ink3: Rgb::new(0xdd, 0xd6, 0xc7),
            ink4: Rgb::new(0xd0, 0xc8, 0xb6),
            rule: Rgb::new(0xc4, 0xbb, 0xa6),
            rule2: Rgb::new(0xb0, 0xa6, 0x8e),
            mute: Rgb::new(0x8a, 0x80, 0x6c),
            text2: Rgb::new(0x1c, 0x1a, 0x14),
            paper: Rgb::new(0x1c, 0x1a, 0x14), // dark accent on light
            fg: Rgb::new(0x33, 0x30, 0x2a),
            bg: Rgb::new(0xe8, 0xe3, 0xd6),
            bg2: Rgb::new(0xde, 0xd7, 0xc6),
            cursor: Rgb::new(0xb5, 0x53, 0x2a),
            sel: Rgb::new(0x9a, 0xb0, 0xd2),
            ansi: fill_ansi(base16),
        }
    }
}

impl Palette {
    fn catppuccin() -> Self {
        // catppuccin mocha, the canonical terminal mapping: surfaces for the
        // dim pair, subtext for white, lavender as the accent
        let base16 = [
            Rgb::new(0x45, 0x47, 0x5a),
            Rgb::new(0xf3, 0x8b, 0xa8),
            Rgb::new(0xa6, 0xe3, 0xa1),
            Rgb::new(0xf9, 0xe2, 0xaf),
            Rgb::new(0x89, 0xb4, 0xfa),
            Rgb::new(0xf5, 0xc2, 0xe7),
            Rgb::new(0x94, 0xe2, 0xd5),
            Rgb::new(0xba, 0xc2, 0xde),
            Rgb::new(0x58, 0x5b, 0x70),
            Rgb::new(0xf3, 0x8b, 0xa8),
            Rgb::new(0xa6, 0xe3, 0xa1),
            Rgb::new(0xf9, 0xe2, 0xaf),
            Rgb::new(0x89, 0xb4, 0xfa),
            Rgb::new(0xf5, 0xc2, 0xe7),
            Rgb::new(0x94, 0xe2, 0xd5),
            Rgb::new(0xa6, 0xad, 0xc8),
        ];
        Palette {
            ink0: Rgb::new(0x11, 0x11, 0x1b), // crust
            ink1: Rgb::new(0x18, 0x18, 0x25), // mantle
            ink3: Rgb::new(0x31, 0x32, 0x44), // surface0
            ink4: Rgb::new(0x45, 0x47, 0x5a), // surface1
            rule: Rgb::new(0x31, 0x32, 0x44),
            rule2: Rgb::new(0x45, 0x47, 0x5a),
            mute: Rgb::new(0x6c, 0x70, 0x86), // overlay0
            text2: Rgb::new(0xcd, 0xd6, 0xf4),
            paper: Rgb::new(0xb4, 0xbe, 0xfe), // lavender
            fg: Rgb::new(0xcd, 0xd6, 0xf4),
            bg: Rgb::new(0x1e, 0x1e, 0x2e),
            bg2: Rgb::new(0x18, 0x18, 0x25),
            cursor: Rgb::new(0xf5, 0xe0, 0xdc), // rosewater
            sel: Rgb::new(0x58, 0x5b, 0x70),
            ansi: fill_ansi(base16),
        }
    }

    fn gruvbox() -> Self {
        // gruvbox dark medium; the orange accent is the theme's signature
        let base16 = [
            Rgb::new(0x28, 0x28, 0x28),
            Rgb::new(0xcc, 0x24, 0x1d),
            Rgb::new(0x98, 0x97, 0x1a),
            Rgb::new(0xd7, 0x99, 0x21),
            Rgb::new(0x45, 0x85, 0x88),
            Rgb::new(0xb1, 0x62, 0x86),
            Rgb::new(0x68, 0x9d, 0x6a),
            Rgb::new(0xa8, 0x99, 0x84),
            Rgb::new(0x92, 0x83, 0x74),
            Rgb::new(0xfb, 0x49, 0x34),
            Rgb::new(0xb8, 0xbb, 0x26),
            Rgb::new(0xfa, 0xbd, 0x2f),
            Rgb::new(0x83, 0xa5, 0x98),
            Rgb::new(0xd3, 0x86, 0x9b),
            Rgb::new(0x8e, 0xc0, 0x7c),
            Rgb::new(0xeb, 0xdb, 0xb2),
        ];
        Palette {
            ink0: Rgb::new(0x14, 0x16, 0x17),
            ink1: Rgb::new(0x1d, 0x20, 0x21), // bg0_h
            ink3: Rgb::new(0x32, 0x30, 0x2f), // bg1
            ink4: Rgb::new(0x3c, 0x38, 0x36), // bg2
            rule: Rgb::new(0x3c, 0x38, 0x36),
            rule2: Rgb::new(0x50, 0x49, 0x45),
            mute: Rgb::new(0x92, 0x83, 0x74),
            text2: Rgb::new(0xfb, 0xf1, 0xc7),
            paper: Rgb::new(0xfe, 0x80, 0x19), // orange
            fg: Rgb::new(0xeb, 0xdb, 0xb2),
            bg: Rgb::new(0x28, 0x28, 0x28),
            bg2: Rgb::new(0x1f, 0x21, 0x21),
            cursor: Rgb::new(0xeb, 0xdb, 0xb2),
            sel: Rgb::new(0x50, 0x49, 0x45),
            ansi: fill_ansi(base16),
        }
    }

    fn tokyo_night() -> Self {
        // tokyo night (storm-leaning darks for the chrome ladder)
        let base16 = [
            Rgb::new(0x15, 0x16, 0x1e),
            Rgb::new(0xf7, 0x76, 0x8e),
            Rgb::new(0x9e, 0xce, 0x6a),
            Rgb::new(0xe0, 0xaf, 0x68),
            Rgb::new(0x7a, 0xa2, 0xf7),
            Rgb::new(0xbb, 0x9a, 0xf7),
            Rgb::new(0x7d, 0xcf, 0xff),
            Rgb::new(0xa9, 0xb1, 0xd6),
            Rgb::new(0x41, 0x48, 0x68),
            Rgb::new(0xf7, 0x76, 0x8e),
            Rgb::new(0x9e, 0xce, 0x6a),
            Rgb::new(0xe0, 0xaf, 0x68),
            Rgb::new(0x7a, 0xa2, 0xf7),
            Rgb::new(0xbb, 0x9a, 0xf7),
            Rgb::new(0x7d, 0xcf, 0xff),
            Rgb::new(0xc0, 0xca, 0xf5),
        ];
        Palette {
            ink0: Rgb::new(0x0f, 0x0f, 0x17),
            ink1: Rgb::new(0x16, 0x16, 0x1e), // bg_dark
            ink3: Rgb::new(0x1f, 0x23, 0x35),
            ink4: Rgb::new(0x29, 0x2e, 0x42),
            rule: Rgb::new(0x29, 0x2e, 0x42),
            rule2: Rgb::new(0x3b, 0x42, 0x61),
            mute: Rgb::new(0x56, 0x5f, 0x89),
            text2: Rgb::new(0xc0, 0xca, 0xf5),
            paper: Rgb::new(0x7a, 0xa2, 0xf7), // the signature blue
            fg: Rgb::new(0xc0, 0xca, 0xf5),
            bg: Rgb::new(0x1a, 0x1b, 0x26),
            bg2: Rgb::new(0x16, 0x16, 0x1e),
            cursor: Rgb::new(0xc0, 0xca, 0xf5),
            sel: Rgb::new(0x33, 0x46, 0x7c),
            ansi: fill_ansi(base16),
        }
    }

    fn nord() -> Self {
        // nord: polar night ground, snow storm text, frost accent
        let base16 = [
            Rgb::new(0x3b, 0x42, 0x52),
            Rgb::new(0xbf, 0x61, 0x6a),
            Rgb::new(0xa3, 0xbe, 0x8c),
            Rgb::new(0xeb, 0xcb, 0x8b),
            Rgb::new(0x81, 0xa1, 0xc1),
            Rgb::new(0xb4, 0x8e, 0xad),
            Rgb::new(0x88, 0xc0, 0xd0),
            Rgb::new(0xe5, 0xe9, 0xf0),
            Rgb::new(0x4c, 0x56, 0x6a),
            Rgb::new(0xbf, 0x61, 0x6a),
            Rgb::new(0xa3, 0xbe, 0x8c),
            Rgb::new(0xeb, 0xcb, 0x8b),
            Rgb::new(0x81, 0xa1, 0xc1),
            Rgb::new(0xb4, 0x8e, 0xad),
            Rgb::new(0x8f, 0xbc, 0xbb),
            Rgb::new(0xec, 0xef, 0xf4),
        ];
        Palette {
            ink0: Rgb::new(0x23, 0x28, 0x31),
            ink1: Rgb::new(0x2a, 0x2f, 0x3a),
            ink3: Rgb::new(0x3b, 0x42, 0x52),
            ink4: Rgb::new(0x43, 0x4c, 0x5e),
            rule: Rgb::new(0x3b, 0x42, 0x52),
            rule2: Rgb::new(0x4c, 0x56, 0x6a),
            mute: Rgb::new(0x61, 0x6e, 0x88),
            text2: Rgb::new(0xe5, 0xe9, 0xf0),
            paper: Rgb::new(0x88, 0xc0, 0xd0), // frost
            fg: Rgb::new(0xd8, 0xde, 0xe9),
            bg: Rgb::new(0x2e, 0x34, 0x40),
            bg2: Rgb::new(0x29, 0x2e, 0x39),
            cursor: Rgb::new(0xd8, 0xde, 0xe9),
            sel: Rgb::new(0x43, 0x4c, 0x5e),
            ansi: fill_ansi(base16),
        }
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::instrument()
    }
}

fn fill_ansi(base16: [Rgb; 16]) -> [Rgb; 256] {
    let mut ansi = [Rgb::new(0, 0, 0); 256];
    ansi[..16].copy_from_slice(&base16);
    // 216-color cube (16..232)
    let steps = [0u8, 95, 135, 175, 215, 255];
    let mut i = 16;
    for r in 0..6 {
        for g in 0..6 {
            for b in 0..6 {
                ansi[i] = Rgb::new(steps[r], steps[g], steps[b]);
                i += 1;
            }
        }
    }
    // grayscale ramp (232..256)
    for j in 0..24 {
        let v = 8 + j as u8 * 10;
        ansi[232 + j] = Rgb::new(v, v, v);
    }
    ansi
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bold_bright_promotes_indexed_0_7_only_when_enabled() {
        // bold + enabled: the 8 base ansi colors promote to their bright pair (+8)
        assert_eq!(Palette::bold_bright(Color::Indexed(1), true, true), Color::Indexed(9));
        assert_eq!(Palette::bold_bright(Color::Indexed(7), true, true), Color::Indexed(15));
        // already-bright (>=8) is left alone
        assert_eq!(Palette::bold_bright(Color::Indexed(9), true, true), Color::Indexed(9));
        // not bold, disabled, or non-indexed: unchanged
        assert_eq!(Palette::bold_bright(Color::Indexed(1), false, true), Color::Indexed(1));
        assert_eq!(Palette::bold_bright(Color::Indexed(1), true, false), Color::Indexed(1));
        assert_eq!(Palette::bold_bright(Color::Default, true, true), Color::Default);
    }
}
