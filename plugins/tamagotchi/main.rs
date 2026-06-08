//! termie reference plugin: a tiny tamagotchi pet.
//!
//! demonstrates the plugin protocol end to end with zero dependencies:
//! - declares a widget, then updates it on a timer (the pet gets hungrier /
//!   sleepier over time)
//! - reacts to host events: a `bell` startles it happy; `focus_changed` pets it
//! - on a Tier-2 host (api_version >= 2, learned from the `hello` handshake) it
//!   draws graphical meters via an immediate-mode draw list; on an older host it
//!   falls back to the Tier-1 text bars
//! - exits cleanly when stdin closes or a `shutdown` event arrives
//!
//! protocol: newline-delimited json. host events arrive on stdin; commands go
//! out on stdout. see plugins/README.md for the full contract.

use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

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

/// the pet's face, derived from its stats
fn face(hunger: u8, joy: u8) -> &'static str {
    if joy >= 70 {
        ">  w  <"
    } else if hunger >= 80 {
        ">  n  <"
    } else if joy <= 25 {
        ">  ;  <"
    } else {
        ">  -  <"
    }
}

/// the Tier-1 view: a face + two text meters
fn render(hunger: u8, joy: u8) -> (String, Vec<String>) {
    let bar = |v: u8| {
        let filled = (v as usize).div_ceil(10);
        let mut s = String::new();
        for i in 0..10 {
            s.push(if i < filled { '#' } else { '.' });
        }
        s
    };
    (
        "tama".to_string(),
        vec![
            face(hunger, joy).to_string(),
            format!("joy   {}", bar(joy)),
            format!("food  {}", bar(100u8.saturating_sub(hunger))),
        ],
    )
}

fn emit_widget(out: &mut impl Write, hunger: u8, joy: u8) {
    let (title, lines) = render(hunger, joy);
    let lines_json: Vec<String> = lines.iter().map(|l| format!("\"{}\"", esc(l))).collect();
    let _ = writeln!(
        out,
        "{{\"t\":\"update_widget\",\"widget\":{{\"id\":\"pet\",\"title\":\"{}\",\"lines\":[{}]}}}}",
        esc(&title),
        lines_json.join(",")
    );
    let _ = out.flush();
}

/// the Tier-2 view: the face plus two graphical meters drawn as track+fill
/// rects, coordinates normalized 0..1 within a 76px canvas
fn emit_widget_v2(out: &mut impl Write, hunger: u8, joy: u8) {
    let food = 100u8.saturating_sub(hunger) as f32 / 100.0;
    let joyf = joy as f32 / 100.0;
    // a labeled meter: a full-width track and a proportional fill on one row
    let meter = |y: f32, frac: f32, color: &str| {
        format!(
            "{{\"t\":\"rect\",\"x\":0.30,\"y\":{y},\"w\":0.70,\"h\":0.14,\"color\":\"ink3\"}},\
             {{\"t\":\"rect\",\"x\":0.30,\"y\":{y},\"w\":{fw:.4},\"h\":0.14,\"color\":\"{color}\"}}",
            fw = 0.70 * frac.clamp(0.0, 1.0)
        )
    };
    let draw = format!(
        "{{\"t\":\"text\",\"x\":0.0,\"y\":0.0,\"text\":\"{face}\",\"color\":\"paper\"}},\
         {{\"t\":\"text\",\"x\":0.0,\"y\":0.30,\"text\":\"food\",\"color\":\"mute\"}},{food_bar},\
         {{\"t\":\"text\",\"x\":0.0,\"y\":0.62,\"text\":\"joy\",\"color\":\"mute\"}},{joy_bar}",
        face = esc(face(hunger, joy)),
        food_bar = meter(0.30, food, "#83a06d"),
        joy_bar = meter(0.62, joyf, "#6486a6"),
    );
    let _ = writeln!(
        out,
        "{{\"t\":\"update_widget\",\"widget\":{{\"id\":\"pet\",\"title\":\"tama\",\"lines\":[],\"canvas_h\":76,\"draw\":[{draw}]}}}}"
    );
    let _ = out.flush();
}

fn emit(out: &mut impl Write, hunger: u8, joy: u8, tier2: bool) {
    if tier2 {
        emit_widget_v2(out, hunger, joy);
    } else {
        emit_widget(out, hunger, joy);
    }
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

fn main() {
    let stdout = Arc::new(Mutex::new(std::io::stdout()));

    // shared stats (0..=100), nudged by both the tick thread and host events
    let hunger = Arc::new(AtomicU8::new(20));
    let joy = Arc::new(AtomicU8::new(80));
    // set once the `hello` handshake reports a Tier-2 host; selects the view
    let tier2 = Arc::new(AtomicBool::new(false));

    // announce ourselves and declare the widget once. we are built against v2,
    // but the view we emit is chosen per host from the handshake below
    {
        let mut o = stdout.lock().unwrap();
        let _ = writeln!(o, "{{\"t\":\"ready\",\"name\":\"tamagotchi\",\"api_version\":2}}");
        let _ = writeln!(
            o,
            "{{\"t\":\"declare_widget\",\"widget\":{{\"id\":\"pet\",\"title\":\"tama\",\"lines\":[]}}}}"
        );
        let _ = o.flush();
        emit(&mut *o, hunger.load(Ordering::Relaxed), joy.load(Ordering::Relaxed), false);
    }

    // tick thread: the pet slowly gets hungrier and a touch less joyful, and we
    // repaint the widget every couple of seconds
    {
        let (stdout, hunger, joy, tier2) = (stdout.clone(), hunger.clone(), joy.clone(), tier2.clone());
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(2));
            let h = (hunger.load(Ordering::Relaxed) + 2).min(100);
            hunger.store(h, Ordering::Relaxed);
            let mut j = joy.load(Ordering::Relaxed).saturating_sub(1);
            if h >= 80 {
                j = j.saturating_sub(2); // hungry pets sulk
            }
            joy.store(j, Ordering::Relaxed);
            let mut o = stdout.lock().unwrap();
            emit(&mut *o, h, j, tier2.load(Ordering::Relaxed));
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
        // dependency-free: match on the event type substring. the protocol
        // guarantees one compact json object per line with a "t" tag
        if line.contains("\"shutdown\"") {
            break;
        } else if line.contains("\"hello\"") {
            // upgrade to the graphical view if the host speaks Tier-2
            if host_api_version(line) >= 2 {
                tier2.store(true, Ordering::Relaxed);
            }
            let mut o = stdout.lock().unwrap();
            emit(&mut *o, hunger.load(Ordering::Relaxed), joy.load(Ordering::Relaxed), tier2.load(Ordering::Relaxed));
        } else if line.contains("\"bell\"") {
            // a bell startles the pet into delight and shakes off hunger a bit
            joy.store(100, Ordering::Relaxed);
            hunger.store(hunger.load(Ordering::Relaxed).saturating_sub(15), Ordering::Relaxed);
            let mut o = stdout.lock().unwrap();
            emit(&mut *o, hunger.load(Ordering::Relaxed), joy.load(Ordering::Relaxed), tier2.load(Ordering::Relaxed));
        } else if line.contains("\"focus_changed\"") {
            // attention cheers it up slightly
            let j = (joy.load(Ordering::Relaxed) + 5).min(100);
            joy.store(j, Ordering::Relaxed);
            let mut o = stdout.lock().unwrap();
            emit(&mut *o, hunger.load(Ordering::Relaxed), j, tier2.load(Ordering::Relaxed));
        }
    }
}
