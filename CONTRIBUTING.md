# Contributing to DriftLab

Thanks for helping! Bug reports, controller IDs, translations and PRs are welcome.

## Quick start

```bash
git clone https://github.com/LaFAirs/driftlab && cd driftlab
cargo run -p driftlab-app -- --demo        # no controller needed
cargo test --workspace
```

Linux needs `libudev-dev libusb-1.0-0-dev libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev libgtk-3-dev`.

## Before opening a PR

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny check
```

* Conventional Commits (`feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`), small and focused.
* No `unwrap()` / `expect()` in production paths; typed errors with `thiserror`.
* New protocol code needs parser tests with recorded fixtures. Anything that writes to a controller must abort to a safe state on failure.
* UI strings go through `crates/driftlab-app/src/i18n.rs` (English **and** German).
* Update `CHANGELOG.md` under *Unreleased*.

## Safety

Never merge changes that write to controller flash without: a backup path, a temporary-first apply, and a manual test entry in `docs/HARDWARE_TESTING.md`.
