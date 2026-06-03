# Third-party notices

termie is dual-licensed under MIT OR Apache-2.0 (see `LICENSE-MIT` and
`LICENSE-APACHE`). It bundles and depends on third-party work covered by their
own licenses, noted below.

## Bundled fonts (`assets/fonts/`)

### Maple Mono (Nerd Font variant)

Copyright (c) 2022 subframe7536.

Licensed under the SIL Open Font License, Version 1.1, reproduced in
`assets/fonts/OFL.txt`. Under the OFL the font may be bundled and redistributed
with software provided this notice and the license are included; it must not be
sold by itself.

The "NF" variant is patched with icon glyphs aggregated by the Nerd Fonts
project (https://github.com/ryanoasis/nerd-fonts). The constituent icon sets
carry their own permissive licenses (MIT, OFL, and others) as documented by
that project.

## Rust dependencies

termie links a number of Rust crates (anyhow, bytemuck, cosmic-text, log,
pollster, portable-pty, vte, wgpu, windows, winit), each under permissive
licenses (MIT, Apache-2.0, and similar). Their full license texts can be
regenerated from the dependency tree with `cargo about` or `cargo deny`.
