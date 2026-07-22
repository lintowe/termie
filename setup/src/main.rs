//! termie's installer. one small frameless window in the app's own style —
//! not a wizard. modes:
//!   (none)        interactive install
//!   /update       silent update: straight to progress, relaunch when done
//!   /uninstall    confirm + remove (what the Add/Remove Programs entry runs)
//!   --preview P.png [--page NAME] [--scale N]   headless design render
#![windows_subsystem = "windows"]

mod engine;
mod payload;
mod ui;

use std::sync::{Arc, Mutex};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |f: &str| args.iter().any(|a| a.eq_ignore_ascii_case(f));
    let val = |f: &str| -> Option<String> {
        args.iter()
            .position(|a| a.eq_ignore_ascii_case(f))
            .and_then(|i| args.get(i + 1))
            .cloned()
    };

    if has("--cleanup") {
        let delay_ms = val("--cleanup-delay-ms")
            .and_then(|value| value.parse().ok())
            .unwrap_or(3_000)
            .clamp(1_000, 15_000);
        engine::cleanup_install_dir(delay_ms);
        return;
    }

    // extraction-only debug mode: unpack the payload somewhere harmless so the
    // packaging pipeline can be verified without touching registry/shortcuts
    if let Some(dir) = val("--extract-to") {
        let mut n = 0usize;
        for e in payload::entries() {
            let raw = e.decompress().expect("decompress");
            let target = std::path::Path::new(&dir).join(e.name.replace('/', "\\"));
            if let Some(p) = target.parent() {
                std::fs::create_dir_all(p).expect("mkdir");
            }
            std::fs::write(&target, &raw).expect("write");
            n += 1;
        }
        // windows-subsystem binary: report through a file the caller can read
        let _ = std::fs::write(
            std::path::Path::new(&dir).join("EXTRACT_REPORT.txt"),
            format!("{n} files, version {}", payload::APP_VERSION),
        );
        return;
    }

    if let Some(out) = val("--preview") {
        let page = match val("--page").as_deref() {
            Some("progress") => ui::Page::Progress,
            Some("done") => ui::Page::Done,
            Some("error") => ui::Page::Error,
            Some("remove") => ui::Page::RemoveConfirm,
            Some("removed") => ui::Page::Removed,
            _ => ui::Page::Options,
        };
        let scale: f32 = val("--scale").and_then(|v| v.parse().ok()).unwrap_or(2.0);
        let mut s = ui::State::new(page);
        s.progress = 0.62;
        s.detail = "assets/fonts/MapleMono-NF-Regular.ttf".into();
        s.error = "write C:\\Users\\you\\AppData\\Local\\Programs\\termie\\termie.exe: access denied".into();
        s.old_msi = true;
        let _ = ui::preview(&out, &s, scale);
        return;
    }

    let mut state = ui::State::new(ui::Page::Options);
    if has("/uninstall") || has("--uninstall") {
        state.page = ui::Page::RemoveConfirm;
    } else if has("/update") || has("--update") {
        state.auto_update = true;
        state.page = ui::Page::Progress;
        state.opts = engine::opts_from_existing();
    } else {
        state.old_msi = engine::find_machine_msi().is_some();
    }
    ui::run(Arc::new(Mutex::new(state)));
}
