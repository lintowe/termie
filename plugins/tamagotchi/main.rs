#![cfg_attr(windows, windows_subsystem = "windows")]

//! termie reference plugin: a small animated tamagotchi pet.
//!
//! demonstrates the plugin protocol end to end with zero dependencies:
//! - declares a widget, then animates it on a timer: the pet idles, blinks,
//!   hops, and naps, and its stats (food, joy) drift over time
//! - reacts to host events: a `bell` startles it into a sparkly bounce,
//!   `focus_changed` cheers it up, and a `widget_clicked` pets/feeds it (click
//!   the card to interact)
//! - draws a pixel creature plus segmented gauges from rect/text primitives on a
//!   Tier-2 host (api_version >= 2, learned from the `hello` handshake); on an
//!   older host it falls back to the Tier-1 text view
//! - only sends a frame when the drawing actually changes, so an idle pet lets
//!   the terminal idle too
//! - exits cleanly when stdin closes or a `shutdown` event arrives
//!
//! protocol: newline-delimited json. host events arrive on stdin; commands go
//! out on stdout. see plugins/README.md for the full contract.

use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// animation clock and pacing, in ticks
const TICK_MS: u64 = 130;
const DECAY_EVERY: u64 = 16; // stat drift cadence (~2s)
const NAP_AFTER: u64 = 460; // ticks of no interaction before the pet naps (~60s)

// creature palette: fixed, tasteful colors so the pet reads on any theme; the
// gauges use palette roles so their track/label blend into the dock
const BODY: &str = "#7fd1a6";
const BODY_SHADE: &str = "#5cb98a";
const BELLY: &str = "#d6f1e4";
const LINE: &str = "#243430";
const SHINE: &str = "#ffffff";
const CHEEK: &str = "#f3a6b0";
const HEART: &str = "#ef5d72";
const SPARK: &str = "#ffd166";
const FOOD: &str = "#83a06d";
const JOY: &str = "#6486a6";
const TRACK: &str = "ink3";
const LABEL: &str = "mute";

/// minimal json string escaper (the only json we emit is widget text)
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out
}

/// pull the host's api version out of a `hello` line without a json dependency
fn host_api_version(line: &str) -> u32 {
    let Some(i) = line.find("\"api_version\"") else {
        return 0;
    };
    line[i..]
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

#[derive(Clone, Copy, PartialEq)]
enum Mood {
    Excited,
    Petted,
    Sleepy,
    Hungry,
    Sad,
    Happy,
    Neutral,
}

impl Mood {
    fn ord(self) -> u64 {
        match self {
            Mood::Excited => 0,
            Mood::Petted => 1,
            Mood::Sleepy => 2,
            Mood::Hungry => 3,
            Mood::Sad => 4,
            Mood::Happy => 5,
            Mood::Neutral => 6,
        }
    }
    /// the short status word shown after the name in the card title
    fn word(self) -> &'static str {
        match self {
            Mood::Excited => "yay",
            Mood::Petted => "<3",
            Mood::Sleepy => "zzz",
            Mood::Hungry => "hungry",
            Mood::Sad => "glum",
            Mood::Happy => "happy",
            Mood::Neutral => "ok",
        }
    }
    /// the Tier-1 fallback face (ascii, no backslashes so it needs no escaping)
    fn face(self) -> &'static str {
        match self {
            Mood::Excited => "(^o^)",
            Mood::Petted => "(^.^)",
            Mood::Sleepy => "(-_-)",
            Mood::Hungry => "(o~o)",
            Mood::Sad => "(T_T)",
            Mood::Happy => "(^_^)",
            Mood::Neutral => "(._.)",
        }
    }
}

/// the pet's whole state. one mutex guards it; the tick thread and the stdin
/// reader both lock it, build a frame, then release it before touching stdout so
/// the two locks are never held at once
struct Pet {
    hunger: u8, // 0..=100, higher = hungrier (food gauge shows 100 - hunger)
    joy: u8,    // 0..=100
    frame: u64,
    blink_until: u64,
    next_blink: u64,
    action_at: u64,  // next frame an idle hop may start
    action_end: u64, // frame the current idle hop ends (0 = none)
    excite_end: u64, // frame a bell's excited reaction ends
    pet_end: u64,    // frame a focus/click reaction ends
    idle_since: u64, // last frame with an interaction, for the nap timer
    tier2: bool,
    rng: u64,
    last_sig: u64,
}

impl Pet {
    fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9e3779b97f4a7c15)
            | 1;
        Pet {
            hunger: 20,
            joy: 80,
            frame: 0,
            blink_until: 0,
            next_blink: 16,
            action_at: 40,
            action_end: 0,
            excite_end: 0,
            pet_end: 0,
            idle_since: 0,
            tier2: false,
            rng: seed,
            last_sig: u64::MAX,
        }
    }

    fn rand(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }
    fn rand_range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.rand() % (hi - lo).max(1)
    }

    fn sleeping(&self) -> bool {
        self.frame.saturating_sub(self.idle_since) > NAP_AFTER && self.hunger < 80 && self.joy > 25
    }
    fn blinking(&self) -> bool {
        self.frame <= self.blink_until
    }
    fn mood(&self) -> Mood {
        if self.frame < self.excite_end {
            Mood::Excited
        } else if self.frame < self.pet_end {
            Mood::Petted
        } else if self.sleeping() {
            Mood::Sleepy
        } else if self.hunger >= 78 {
            Mood::Hungry
        } else if self.joy <= 25 {
            Mood::Sad
        } else if self.joy >= 68 {
            Mood::Happy
        } else {
            Mood::Neutral
        }
    }

    fn food_fill(&self) -> u8 {
        ((100u16.saturating_sub(self.hunger as u16) + 5) / 10).min(10) as u8
    }
    fn joy_fill(&self) -> u8 {
        ((self.joy as u16 + 5) / 10).min(10) as u8
    }

    /// vertical offset for the whole creature (positive = down), giving idle
    /// bob, an occasional hop, reaction bounces, and slow sleep breathing
    fn bob(&self) -> f32 {
        let f = self.frame;
        if f < self.excite_end {
            hop_curve(self.excite_end - f) * 1.3
        } else if f < self.pet_end {
            hop_curve(self.pet_end - f) * 0.7
        } else if self.action_end != 0 {
            hop_curve(self.action_end - f)
        } else if self.sleeping() {
            [0.0, 0.012, 0.022, 0.012][((f / 8) % 4) as usize]
        } else {
            [0.0, 0.010, 0.020, 0.010][((f / 6) % 4) as usize]
        }
    }

    /// animation phase for moods with a moving overlay (sparkles, hearts, zzz)
    fn overlay_phase(&self) -> u64 {
        match self.mood() {
            Mood::Excited | Mood::Petted | Mood::Sleepy => (self.frame / 4) % 4,
            _ => 0,
        }
    }

    /// a compact hash of everything currently visible, used to skip emitting a
    /// frame when nothing changed (so an idle pet doesn't keep the dock redrawing)
    fn sig(&self) -> u64 {
        let mut s = self.mood().ord() | ((self.food_fill() as u64) << 4) | ((self.joy_fill() as u64) << 9);
        if self.tier2 {
            let bobb = (((self.bob() + 0.1) * 200.0) as u64) & 0x3ff;
            s |= ((self.blinking() as u64) << 14) | (bobb << 16) | (self.overlay_phase() << 28);
        }
        s
    }

    /// advance one tick: drift stats, schedule blinks and idle hops, wake naps
    fn tick(&mut self) {
        self.frame += 1;
        let f = self.frame;
        if f % DECAY_EVERY == 0 {
            if self.sleeping() {
                self.hunger = (self.hunger + 1).min(100);
                self.joy = (self.joy + 1).min(100);
            } else {
                self.hunger = (self.hunger + 2).min(100);
                let mut j = self.joy.saturating_sub(1);
                if self.hunger >= 80 {
                    j = j.saturating_sub(1);
                }
                self.joy = j;
            }
        }
        if self.sleeping() {
            return;
        }
        if f >= self.next_blink {
            self.blink_until = f + 1;
            self.next_blink = f + self.rand_range(18, 46);
        }
        if self.action_end != 0 && f >= self.action_end {
            self.action_end = 0;
        }
        if self.action_end == 0 && f >= self.action_at && f >= self.excite_end && f >= self.pet_end {
            self.action_end = f + 6;
            self.action_at = f + self.rand_range(48, 110);
        }
    }

    fn on_bell(&mut self) {
        self.joy = 100;
        self.hunger = self.hunger.saturating_sub(15);
        self.excite_end = self.frame + 14;
        self.idle_since = self.frame;
    }
    fn on_focus(&mut self) {
        self.joy = (self.joy + 5).min(100);
        self.pet_end = self.frame + 6;
        self.idle_since = self.frame;
    }
    fn on_pet(&mut self) {
        self.joy = (self.joy + 12).min(100);
        self.hunger = self.hunger.saturating_sub(10);
        self.pet_end = self.frame + 10;
        self.idle_since = self.frame;
    }

    fn widget_line(&self) -> String {
        if self.tier2 {
            self.v2_line()
        } else {
            self.v1_line()
        }
    }

    /// the Tier-1 fallback: a face plus two text bars
    fn v1_line(&self) -> String {
        let bar = |v: u8| {
            let filled = (v as usize).div_ceil(10);
            (0..10).map(|i| if i < filled { '#' } else { '.' }).collect::<String>()
        };
        let m = self.mood();
        format!(
            "{{\"t\":\"update_widget\",\"widget\":{{\"id\":\"pet\",\"title\":\"tama · {}\",\"lines\":[\"{}\",\"food {}\",\"joy  {}\"]}}}}\n",
            m.word(),
            m.face(),
            bar(100u8.saturating_sub(self.hunger)),
            bar(self.joy),
        )
    }

    /// the Tier-2 view: a pixel creature drawn from rects + text, plus two
    /// segmented gauges. coordinates are normalized 0..1 within the canvas box
    fn v2_line(&self) -> String {
        let mut d: Vec<String> = Vec::with_capacity(48);
        macro_rules! rect {
            ($x:expr, $y:expr, $w:expr, $h:expr, $c:expr) => {
                d.push(format!(
                    "{{\"t\":\"rect\",\"x\":{:.4},\"y\":{:.4},\"w\":{:.4},\"h\":{:.4},\"color\":\"{}\"}}",
                    $x as f32, $y as f32, $w as f32, $h as f32, $c
                ))
            };
        }
        macro_rules! text {
            ($x:expr, $y:expr, $s:expr, $c:expr) => {
                d.push(format!(
                    "{{\"t\":\"text\",\"x\":{:.4},\"y\":{:.4},\"text\":\"{}\",\"color\":\"{}\"}}",
                    $x as f32, $y as f32, esc($s), $c
                ))
            };
        }

        let mood = self.mood();
        let dy = self.bob();
        let asleep = mood == Mood::Sleepy;
        let closed = asleep || self.blinking();
        let happy = matches!(mood, Mood::Happy | Mood::Excited | Mood::Petted);

        // ears
        rect!(0.40, 0.085 + dy, 0.06, 0.06, BODY);
        rect!(0.54, 0.085 + dy, 0.06, 0.06, BODY);
        rect!(0.415, 0.10 + dy, 0.03, 0.035, BELLY);
        rect!(0.555, 0.10 + dy, 0.03, 0.035, BELLY);
        // body (stacked slabs make a rounded blob), then belly highlight
        rect!(0.40, 0.12 + dy, 0.20, 0.07, BODY);
        rect!(0.345, 0.175 + dy, 0.31, 0.07, BODY);
        rect!(0.30, 0.235 + dy, 0.40, 0.20, BODY);
        rect!(0.335, 0.43 + dy, 0.33, 0.075, BODY);
        rect!(0.385, 0.50 + dy, 0.23, 0.05, BODY_SHADE);
        rect!(0.41, 0.31 + dy, 0.18, 0.14, BELLY);
        // feet
        rect!(0.40, 0.545 + dy, 0.07, 0.03, BODY_SHADE);
        rect!(0.53, 0.545 + dy, 0.07, 0.03, BODY_SHADE);

        // cheeks
        if happy {
            rect!(0.35, 0.35 + dy, 0.045, 0.03, CHEEK);
            rect!(0.605, 0.35 + dy, 0.045, 0.03, CHEEK);
        }
        // eyes
        if closed {
            rect!(0.405, 0.305 + dy, 0.06, 0.014, LINE);
            rect!(0.545, 0.305 + dy, 0.06, 0.014, LINE);
        } else {
            rect!(0.405, 0.265 + dy, 0.06, 0.08, LINE);
            rect!(0.545, 0.265 + dy, 0.06, 0.08, LINE);
            rect!(0.44, 0.277 + dy, 0.018, 0.022, SHINE);
            rect!(0.58, 0.277 + dy, 0.018, 0.022, SHINE);
        }
        // mouth, by mood
        match mood {
            Mood::Excited => {
                rect!(0.455, 0.40 + dy, 0.09, 0.055, LINE);
                rect!(0.475, 0.43 + dy, 0.05, 0.02, CHEEK);
            }
            Mood::Hungry => {
                rect!(0.46, 0.40 + dy, 0.08, 0.06, LINE);
            }
            Mood::Happy | Mood::Petted => {
                rect!(0.46, 0.425 + dy, 0.08, 0.014, LINE);
                rect!(0.445, 0.41 + dy, 0.02, 0.014, LINE);
                rect!(0.535, 0.41 + dy, 0.02, 0.014, LINE);
            }
            Mood::Sad => {
                rect!(0.46, 0.415 + dy, 0.08, 0.014, LINE);
                rect!(0.445, 0.43 + dy, 0.02, 0.014, LINE);
                rect!(0.535, 0.43 + dy, 0.02, 0.014, LINE);
                rect!(0.43, 0.34 + dy, 0.02, 0.03, JOY);
            }
            Mood::Sleepy => {
                rect!(0.485, 0.42 + dy, 0.03, 0.012, LINE);
            }
            Mood::Neutral => {
                rect!(0.47, 0.415 + dy, 0.06, 0.014, LINE);
            }
        }
        // animated overlays
        let p = self.overlay_phase();
        match mood {
            Mood::Excited => {
                if p % 2 == 0 {
                    text!(0.20, 0.10, "*", SPARK);
                    text!(0.72, 0.04, "*", SPARK);
                } else {
                    text!(0.74, 0.16, "+", SPARK);
                    text!(0.18, 0.22, "+", SPARK);
                }
                text!(0.62, 0.12_f32 - (p as f32) * 0.02, "<3", HEART);
            }
            Mood::Petted => {
                text!(0.60, 0.14_f32 - (p as f32) * 0.02, "<3", HEART);
            }
            Mood::Sleepy => {
                if p % 2 == 0 {
                    text!(0.64, 0.12, "z", LABEL);
                } else {
                    text!(0.70, 0.05, "Z", LABEL);
                }
            }
            _ => {}
        }

        // gauges: label + ten segments each
        let seg = |d: &mut Vec<String>, y: f32, fill: u8, color: &str| {
            for i in 0..10u8 {
                let sx = 0.31 + (i as f32) * (0.045 + 0.013);
                let c = if i < fill { color } else { TRACK };
                d.push(format!(
                    "{{\"t\":\"rect\",\"x\":{:.4},\"y\":{:.4},\"w\":0.0450,\"h\":0.0500,\"color\":\"{}\"}}",
                    sx, y, c
                ));
            }
        };
        text!(0.03, 0.69, "food", LABEL);
        seg(&mut d, 0.71, self.food_fill(), FOOD);
        text!(0.03, 0.85, "joy", LABEL);
        seg(&mut d, 0.87, self.joy_fill(), JOY);

        format!(
            "{{\"t\":\"update_widget\",\"widget\":{{\"id\":\"pet\",\"title\":\"tama · {}\",\"lines\":[],\"canvas_h\":150,\"draw\":[{}]}}}}\n",
            mood.word(),
            d.join(",")
        )
    }
}

/// hop easing: given frames remaining in a 0..=6 hop, return the height (up is
/// negative). a quick rise and a soft landing
fn hop_curve(remaining: u64) -> f32 {
    match remaining {
        6 => -0.020,
        5 => -0.050,
        4 => -0.060,
        3 => -0.050,
        2 => -0.030,
        1 => -0.012,
        _ => 0.0,
    }
}

fn main() {
    let out = Arc::new(Mutex::new(std::io::stdout()));
    let pet = Arc::new(Mutex::new(Pet::new()));

    // announce ourselves and declare the widget once
    {
        let mut o = out.lock().unwrap();
        let _ = writeln!(o, "{{\"t\":\"ready\",\"name\":\"tamagotchi\",\"api_version\":2}}");
        let _ = writeln!(
            o,
            "{{\"t\":\"declare_widget\",\"widget\":{{\"id\":\"pet\",\"title\":\"tama\",\"lines\":[]}}}}"
        );
        let _ = o.flush();
    }

    // tick thread: animate + drift stats, emitting a frame only when the drawing
    // changes so an idle pet lets the terminal idle too
    {
        let (out, pet) = (out.clone(), pet.clone());
        thread::spawn(move || loop {
            thread::sleep(Duration::from_millis(TICK_MS));
            let mut p = pet.lock().unwrap();
            p.tick();
            let sig = p.sig();
            if sig == p.last_sig {
                continue;
            }
            p.last_sig = sig;
            let line = p.widget_line();
            drop(p);
            let mut o = out.lock().unwrap();
            let _ = o.write_all(line.as_bytes());
            let _ = o.flush();
        });
    }

    // main thread: read host events line by line until stdin closes
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.contains("\"shutdown\"") {
            break;
        }
        // dependency-free: match on the event type substring. the protocol
        // guarantees one compact json object per line with a "t" tag
        let mut p = pet.lock().unwrap();
        let handled = if line.contains("\"hello\"") {
            p.tier2 = host_api_version(line) >= 2;
            true
        } else if line.contains("\"bell\"") {
            p.on_bell();
            true
        } else if line.contains("\"widget_clicked\"") {
            p.on_pet();
            true
        } else if line.contains("\"focus_changed\"") {
            p.on_focus();
            true
        } else {
            false
        };
        if handled {
            p.last_sig = p.sig();
            let l = p.widget_line();
            drop(p);
            let mut o = out.lock().unwrap();
            let _ = o.write_all(l.as_bytes());
            let _ = o.flush();
        }
    }
}
