## What this changes

A short summary of the change and why. Link any issue it closes.

## Checklist

- [ ] `cargo test` passes
- [ ] `cargo clippy --all-targets` is clean (the main crate, and the example plugins if you touched them)
- [ ] Terminal/rendering changes come with a golden scenario or unit test; if rendering intentionally changed, the snapshots are re-blessed and the diff reviewed (`$env:BLESS=1; cargo test golden`)
- [ ] Commits use the short-prefix style (`fix:`, `perf:`, `feat:`, `test:`, `polish:`)
