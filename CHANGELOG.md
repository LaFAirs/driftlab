# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.0] - 2026-11-19

### Changed
- Controller models are now real, licensed `.glb` models (CC BY 4.0, see `CREDITS.md`) instead of code-generated geometry: DualSense, DualShock 4 and Xbox One/Series; DualSense Edge and Xbox Elite are derived from them with own rear paddles.
- New PBR renderer: image-based lighting (CC0 HDRI), soft key-light shadow, ACES tone mapping, 4x MSAA, textured materials, emissive parts, alpha-blended parts, ground shadow and reflection.
- Moving parts (sticks, triggers, bumpers, d-pad, face buttons, touchpad, buttons) are animated through named glTF nodes with real pivots.
- Manufacturer lettering and logos are removed from all models; face symbols and d-pad blocks of the DualSense models were rebuilt as own work.

### Fixed
- Memory growth while the window was minimised (unbounded input queue).
- Corrupt `profiles.json` / `settings.toml` are quarantined instead of overwritten; atomic saves.
- Hard-edged shadow rectangle on light themes; invisible slider rails on dark themes.
- Author/licence data verified against the Sketchfab API.

### Added
- `docs/AUDIT.md`.
- `tools/models/`: reproducible Blender preparation scripts for every model.
- Credits for models and HDRI in `CREDITS.md`, `assets/models/LICENSES/` and the About dialog.

### Known limitations
- Edge and Elite are approximations (standard-model shapes plus paddles), not scans of those products.
- Bumpers and triggers of the Edge and Elite models are not separately animated.
- The DualSense touchpad is black by design choice (the real one is white).

## [0.1.0] - 2026-11-19

First public release.

### Added
- Native Rust app (egui + wgpu) with live 3D controller models, 2D vector fallback and animated per-controller themes (PS5, PS5 Edge, PS4, Xbox, neutral) with OKLab colour transitions.
- Controller detection with hotplug for DualSense, DualSense Edge, DualShock 4 (v1/v2), Xbox One/Series/Elite and generic gamepads.
- Drift diagnosis: 5-second auto test (median/MAD filtering), noise, jitter, drift score 0–100, circularity test, trigger test, CSV/JSON export, PNG report.
- Calibration wizard with hardware calibration for DualShock 4 and DualSense/Edge: pre-flight safety checks, automatic JSON backup, temporary-first apply, before/after comparison, optional permanent save, backup restore.
- Software correction for every controller: centre offset, four deadzone types, inner/outer/anti deadzone, response curves (linear, exponential, S, Bézier), circularity gain, auto-optimise, per-controller profiles, import/export, Steam Input export.
- Button test, vibration, lightbar, gyro/accelerometer graphs, touchpad view, backups screen, settings with HID inspector (expert mode), update check, German and English UI.
- `driftlab-cli` with `list`, `monitor`, `drift-test` and `dump-calibration`.
- Installers: Windows setup + portable zip, Linux AppImage + .deb (with udev rules), macOS universal .dmg, SHA256 checksums.

### Known limitations
- System-wide virtual controller is not available yet; use the Steam Input profile export (see README).
- Hardware calibration write paths could not be verified on physical hardware before release; see `docs/HARDWARE_TESTING.md`.
- Xbox controllers have no open calibration protocol (software correction only).

[Unreleased]: https://github.com/LaFAirs/driftlab/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/LaFAirs/driftlab/releases/tag/v0.1.0

