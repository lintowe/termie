# acrylic / mica backdrop

termie operates with flat per-pixel alpha today (the `opacity` setting). On
Windows 11 (build >= 22000, 22H2+) the window can additionally opt into the
system backdrop so the desktop shows through the chrome as Mica.

## config

Add to `%APPDATA%\termie\config`:

    acrylic=true

The alias `mica=true` also works for this key. On older Windows the call
returns `E_INVALIDARG` and is ignored — the window still shows with rounded
corners and your configured opacity. No code change is required for lighting
changes; the system tints the backdrop itself.

## how it works (code)

- `src/win.rs` defines the raw attribute number `DWMWA_SYSTEMBACKDROP_TYPE = 38`
  (not all windows-rs versions export it) and an enum mirroring
  `DWM_SYSTEMBACKDROP_TYPE`:
  `DWMSBT_AUTO=0, DWMSBT_NONE=1, DWMSBT_MAINWINDOW=2, DWMSBT_TRANSIENTWINDOW=3,
   DWMSBT_TABBEDWINDOW=4`.
- `apply_backdrop(hwnd, use_acrylic)` builds a `DWMWINDOWATTRIBUTE(38)` and calls
  `DwmSetWindowAttribute`. The HRESULT is discarded — cosmetic only.
- `apply_acrylic(hwnd, opacity)` is a legacy alias per the task description: it
  calls `apply_backdrop(hwnd, true)` and ignores `opacity`; flat opacity is
  handled by the renderer's `set_opacity_pct` path instead.
- `apply_window_effects(hwnd)` still sets `DWMWCP_ROUND` for rounded corners;
  it is kept unchanged. `boot` in `src/main.rs` now calls both helpers: corners
  always, backdrop only when `persisted.acrylic` is true.

## future: true Acrylic blur vs Mica

- The system treats MAINWINDOW as Mica on Win11. If a true Acrylic (blur + tint)
  blur is desired, one additional call `DwmExtendFrameIntoClientArea` with
  a negative margin plus rendering the background fully transparent is needed.
  Persisted alpha must be 1.0 for that path to show correctly — otherwise the
  wrender's bg_alpha composes over the blurred system texture a second time.
  The stub above intentionally keeps the two paths separate.
