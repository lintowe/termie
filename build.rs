// embed a windows application manifest declaring per-monitor-v2 dpi awareness
// and long-path support. declaring dpi awareness in the manifest sets it at
// process load, before any window exists, which is more reliable than winit's
// runtime SetProcessDpiAwarenessContext call ordering. the CARGO_CFG_WINDOWS
// check (not cfg!(windows)) is required because build scripts run on the host

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        use embed_manifest::manifest::{DpiAwareness, Setting};
        use embed_manifest::{embed_manifest, new_manifest};
        embed_manifest(
            new_manifest("termie")
                .dpi_awareness(DpiAwareness::PerMonitorV2)
                .long_path_aware(Setting::Enabled),
        )
        .expect("failed to embed application manifest");
    }
}
