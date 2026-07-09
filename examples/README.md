# termie examples

This directory contains minimal reference examples for writing termie plugins
and configuring window effects.

- `hello-plugin/` — the smallest possible plugin: announces `ready`, declares
  a text widget, then updates it every second with a counter and the current
  focus/tab. Shows the Newline-Delimited JSON (NDJSON) wire protocol without any
  external dependency. Any language that can read stdin / write stdout can do
  the same; the reference here is Python for readability.
- `acrylic.md` — notes on the `acrylic=true` config key (Win11 Mica backdrop).

See also `%APPDATA%\termie\config` keys documented in README.md's Configuration
section, and the plugin protocol docs in `src/plugin/proto.rs`.
