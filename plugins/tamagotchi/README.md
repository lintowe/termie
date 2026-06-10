# tamagotchi — termie reference plugin

A small animated pet that lives in termie's plugin dock. It's the first-party
reference for the Tier-2 plugin protocol: zero dependencies, demonstrates an
immediate-mode draw list and reacting to host events.

## What it does

- Declares a `pet` widget, then animates it: the creature idles, blinks, hops,
  and naps when left alone, and its food/joy gauges drift over time.
- Reacts to host events: a terminal `bell` startles it into a sparkly bounce,
  switching pane focus cheers it up, and clicking the card pets and feeds it.
- Draws a pixel creature plus segmented gauges from `rect`/`text` primitives on a
  Tier-2 host, and falls back to a face + text bars on an older Tier-1 host
  (chosen from the `hello` handshake).
- Only sends a frame when the drawing changes, so an idle pet lets the terminal
  idle too.
- Exits cleanly when termie closes its stdin or sends `shutdown`.

## Build & install

This is an independent crate (its own `[workspace]`), so it does not affect
termie's build.

```powershell
cd plugins/tamagotchi
cargo build --release
# install into termie's plugin dir
$dst = "$env:APPDATA\termie\plugins\tamagotchi"
New-Item -ItemType Directory -Force $dst | Out-Null
Copy-Item target/release/tamagotchi.exe $dst
Copy-Item plugin.json $dst
```

Relaunch termie — the pet appears in the right-side dock. (termie spawns enabled
plugins after the window is shown, so it never slows startup.)

## How it talks to termie

Newline-delimited JSON: host events arrive on stdin, commands go out on stdout,
stderr is for logs. See `../README.md` for the full protocol.
