# Audit (v0.2.0)

Method: full read of the code base, docs and CI; search for `TODO`/`FIXME`/`todo!`/`unimplemented!` (none found); GitHub issues
(none were open); every screen of the app captured in demo mode (MockDriver) and inspected; 8.5-minute demo soak test with
memory/CPU sampling; unit and integration tests (`cargo test --workspace`).

## Findings

| # | Prio | Finding | Status |
|---|---|---|---|
| 1 | high | **Memory growth while the window does not repaint** (minimised / occluded): the input queue between device threads and the UI was unbounded; ≈ 2 MB/min at 250 Hz (308 → 326 MB in 8 min). | Fixed: queue back-pressure (`MAX_QUEUED_EVENTS`), regression test `event_queue_stays_bounded_when_ui_does_not_drain`. After the fix memory stayed flat at 314 MB over 4 min of the same run. |
| 2 | high | **Corrupt `profiles.json` / `settings.toml` were silently replaced by defaults and then overwritten on the next save** (user profiles lost). | Fixed: unreadable files are moved to `*.corrupt`; saves are atomic (temp file + rename). Tests added, including an unwritable data directory. |
| 3 | high | Light themes: the 3D view showed a grey rectangle with hard edges (ground shadow was not faded out inside the viewport). | Fixed in `shader.wgsl` (falloff). |
| 4 | medium | Dark themes: slider rails and checkbox frames nearly invisible (low contrast). | Fixed (`theme.rs`), sliders show a filled trailing part. |
| 5 | medium | Model licence texts / About dialog named models that are not used (miwaVV, Vrah) and an unverified author name. | Fixed. Authors and licences verified through the public Sketchfab API (`user.displayName`, `license`); all three used models are CC BY 4.0. |
| 6 | medium | Manufacturer lettering ("SONY", "XBOX", SHARE/OPTIONS, regulatory label) visible on the models. | Removed from textures / geometry. |
| 7 | medium | PowerShell `Set-Content -Encoding utf8` adds a BOM; a BOM in `shader.wgsl` broke shader compilation during this audit. | Fixed; no tracked file starts with a BOM. |
| 8 | low | DualSense bumpers belong to the shell mesh in the source model and are not separately animated; Edge/Elite are derived models. | Documented (README, DECISIONS 15–19), issue opened. |
| 9 | low | Hardware calibration write paths unverified on real hardware. | Documented; needs community testing (`docs/HARDWARE_TESTING.md`), issue opened. |
| 10 | low | No automated test at 150 % / 200 % display scaling. | Documented, issue opened. |
| 11 | low | No animated GIF in the README. | Issue opened. |

## Edge cases checked

| Case | Result |
|---|---|
| Controller disconnected during the wizard | Handled (`wizard::on_disconnect`, safe cancel, covered by `tests/wizard.rs`) |
| Fast plug / unplug | Device list fades entries out; removal is idempotent; mock lifecycle test |
| No controller | Empty state shown |
| Corrupt profile / settings file | Quarantined, defaults used (see finding 2) |
| Unwritable data directory | Errors are returned, no panic (test) |
| Language | DE/EN key parity enforced by a test; all screens captured in German |

## Performance (demo mode, release build, AMD Radeon 760M iGPU, Windows 11)

| Metric | Result |
|---|---|
| CPU while animating | ≈ 1.5–3 % of total CPU (8 logical cores) |
| CPU while idle / occluded | ≈ 0.2 % |
| Memory, 8.5 min before fix | 308 → 326 MB (growing) |
| Memory, 4 min after fix | 314 MB (flat) |
| Frame rate | not measured with an on-screen counter; rendering is vsync-limited (60 Hz) and CPU load stays low |


# v0.2.1 – why did the DualSense look like grey chrome?

**Cause: the lighting environment, not the material data.** The v0.2.0 studio HDRI (`studio_small_03`) has dark surroundings; a
satin white plastic with a roughness of 0.4–0.5 reflects those at glancing angles and in its highlights, which reads as grey chrome.
A product photo uses a white cyclorama with large softboxes.

What was checked (and found correct):

| Check | Result |
|---|---|
| Textures loaded (baseColor / metallicRoughness / normal / emissive) | all present for every material of `dualsense.glb` (dump of the glTF JSON) |
| Colour space | base colour and emissive are uploaded as `Rgba8UnormSrgb`, metallicRoughness / normal / occlusion as linear `Rgba8Unorm` (`gltf_load.rs`, `lib.rs`) |
| Channels | roughness = G, metallic = B, occlusion = R (shader); the shell texture reads metallic 0, roughness ≈ 0.5 (channel means B = 0, G = 127) |
| Factors | `metallicFactor` / `roughnessFactor` are multiplied with the texture values |
| Extensions | the model uses `KHR_materials_specular` on one material (ignored, default specular is equivalent); no clearcoat |
| Normal maps | derivative-based tangent frame, OpenGL convention, correct UV set |

Real defects found on the way (fixed):

1. dark environment (see above) → procedural bright studio with key / fill / rim softboxes;
2. the white shell consists of overlapping layers (`Back_Baseq`, `Side_Panels`, the static `Trigger_Buttons` part) whose grey
   textures showed through as camouflage patches → one plain warm-white dielectric (albedo 0.80, roughness 0.5, metallic 0), faces
   oriented consistently, `Side_Panels` lifted by 0.25 mm, black centre section shrunk by 0.75 %;
3. black centre section: glossy (roughness ≈ 0.22 via a patched `roughnessFactor`), touchpad black glass;
4. camera: 22° telephoto instead of 30°, aspect-aware distance, 23° default pitch, slow idle sway and float;
5. contact shadow tightened, cheap ground-contact ambient occlusion, exposure raised so white reads as white;
6. the Windows executable had no icon resource (generic icon on shortcuts) → `build.rs` + `driftlab.rc`;
7. screenshot mode could pick up a real, connected controller (a real DualSense showed up in a test run) → it now only accepts the simulated one.

Not done / limits: the Sketchfab viewer and real manufacturer product photos were not placed next to our renders (no licence for
the photos; the viewer needs a browser session); the alternative model "Playstation 5 Dualsense" by AHarmlessPotato was **not**
evaluated because downloads need a logged-in Sketchfab session – with the fixed renderer the current model is acceptable, so it was not
needed. Bumpers of the DualSense are still part of the shell mesh.