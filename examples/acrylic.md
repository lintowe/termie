# acrylic / mica backdrop

termie operates with flat per-pixel alpha (the `opacity` setting). On
Windows 11 (build >= 22000) the window can additionally opt into the system
backdrop so the desktop shows through the chrome as Mica.

## config

Add to `%APPDATA%\termie\config`:

    acrylic=true

The alias `mica=true` also works for this key. On older Windows the call
fails and is ignored — the window still shows with rounded corners and your
configured opacity. The system tints the backdrop itself, so lighting and
wallpaper changes track automatically.

The backdrop is only visible when `opacity` is below 100; at full opacity the
terminal paints over it.

## how it works (code)

- `src/win.rs` spells the attribute number raw (`DWMWINDOWATTRIBUTE(38)`,
  `DWMWA_SYSTEMBACKDROP_TYPE`) because not every windows-rs release exports
  the enum, and passes `DWMSBT_MAINWINDOW` (2, Mica) to
  `DwmSetWindowAttribute`. The HRESULT is discarded — cosmetic only.
- `apply_window_effects(hwnd)` sets `DWMWCP_ROUND` for rounded corners as
  before; window creation in `src/main.rs` calls `apply_backdrop(hwnd)` right
  after it when `acrylic=true` is set.

## true Acrylic blur vs Mica

The system treats `DWMSBT_MAINWINDOW` as Mica on Win11. A true Acrylic
(blur + tint) backdrop would additionally need `DwmExtendFrameIntoClientArea`
with a negative margin and a fully transparent rendered background — at which
point the renderer's `bg_alpha` would compose over the blurred system texture
a second time, so the flat-opacity path and a blur path can't share settings.
Mica sidesteps that interaction, which is why it's the shipped variant.
