//! windows-specific window effects: acrylic backdrop + rounded corners

/// suppress the OS "application was unable to start correctly" / crash dialogs
/// for this process and the children it spawns. without this, a pre-warmed pool
/// pwsh that loses its ConPTY during termie's exit pops a 0xc0000142 dialog.
/// child processes inherit the error mode set here.
#[cfg(windows)]
pub fn suppress_child_error_dialogs() {
    // SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX
    unsafe extern "system" {
        fn SetErrorMode(mode: u32) -> u32;
    }
    unsafe {
        SetErrorMode(0x0001 | 0x0002);
    }
}

#[cfg(not(windows))]
pub fn suppress_child_error_dialogs() {}

/// procedurally drawn app/taskbar icon (RGBA, premultiplied not required by winit):
/// a dark rounded square with the instrument ">_" prompt mark in white
pub fn app_icon() -> (Vec<u8>, u32, u32) {
    const N: i32 = 64;
    let mut px = vec![0u8; (N * N * 4) as usize];
    let mut set = |x: i32, y: i32, c: [u8; 4]| {
        if x < 0 || y < 0 || x >= N || y >= N {
            return;
        }
        let i = ((y * N + x) * 4) as usize;
        px[i] = c[0];
        px[i + 1] = c[1];
        px[i + 2] = c[2];
        px[i + 3] = c[3];
    };

    // charcoal rounded-square background via a signed-distance test (matches
    // the installer .ico: inset 0.06, corner radius 0.225, flat #1a1a1a)
    let inset = N as f32 * 0.06;
    let radius = N as f32 * 0.225;
    let center = N as f32 / 2.0;
    let half = N as f32 / 2.0 - inset;
    for y in 0..N {
        for x in 0..N {
            let pxf = x as f32 + 0.5 - center;
            let pyf = y as f32 + 0.5 - center;
            let qx = pxf.abs() - (half - radius);
            let qy = pyf.abs() - (half - radius);
            let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius;
            if d <= 0.0 {
                set(x, y, [0x1a, 0x1a, 0x1a, 0xff]);
            }
        }
    }

    // optically centered ">_" prompt mark in near-white, drawn as thick strokes;
    // coordinates mirror the installer .ico (normalized, baked-in optical nudge)
    let ink = [0xf4u8, 0xf4, 0xf4, 0xff];
    let mut line = |x0: i32, y0: i32, x1: i32, y1: i32, t: i32| {
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for s in 0..=steps {
            let fx = x0 + (x1 - x0) * s / steps;
            let fy = y0 + (y1 - y0) * s / steps;
            for dy in -t..=t {
                for dx in -t..=t {
                    if dx * dx + dy * dy <= t * t {
                        set(fx + dx, fy + dy, ink);
                    }
                }
            }
        }
    };
    line(19, 20, 32, 30, 3); // chevron upper
    line(32, 30, 19, 40, 3); // chevron lower
    line(34, 40, 46, 40, 3); // underscore

    (px, N as u32, N as u32)
}

#[cfg(windows)]
pub fn apply_window_effects(hwnd_handle: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
        DWM_WINDOW_CORNER_PREFERENCE,
    };

    let hwnd = HWND(hwnd_handle as *mut core::ffi::c_void);
    unsafe {
        // rounded corners + shadow even though we're undecorated
        let corner: DWM_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );
    }
}

#[cfg(not(windows))]
pub fn apply_window_effects(_hwnd_handle: isize) {}

#[cfg(windows)]
pub fn local_hm() -> String {
    use windows::Win32::System::SystemInformation::GetLocalTime;
    let st = unsafe { GetLocalTime() };
    format!("{:02}:{:02}", st.wHour, st.wMinute)
}

#[cfg(not(windows))]
pub fn local_hm() -> String {
    String::new()
}

/// open an http(s) URL in the default browser via the shell. the scheme is
/// re-checked here so only web links can ever be launched, never a file path
/// or a custom protocol handler that could start an arbitrary app
#[cfg(windows)]
pub fn open_url(url: &str) {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::PCWSTR;
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return;
    }
    let verb: Vec<u16> = "open\0".encode_utf16().collect();
    let file: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

#[cfg(not(windows))]
pub fn open_url(_url: &str) {}
