# Credits

## 3D controller models (CC BY 4.0)

The controller models were created by the authors below and are used under the
[Creative Commons Attribution 4.0 International](assets/models/LICENSES/CC-BY-4.0.txt) licence.
They were downloaded from Sketchfab and then **modified** for DriftLab: scaled to real-world size, origins re-centred,
moving parts (sticks, triggers, buttons, d-pad, touchpad, lightbar, paddles) separated into named nodes with correct pivots,
brand logos removed or replaced by plain surfaces, meshes decimated and textures resized (see `tools/models/`).

| Model in DriftLab | Original work | Author | Licence |
|---|---|---|---|
| DualSense | [Sony PS5 DualSense Controller](https://sketchfab.com/3d-models/b1d054ee67204ec1984d5fbebd707cde) | be the knight (jayhystic22) | CC BY 4.0 |
| DualSense Edge | [Dualsense](https://sketchfab.com/3d-models/14a6fa6d7078463380cc72fe0bc5acb0) | miwaVV (mishel1337) | CC BY 4.0 |
| DualShock 4 | [DualShock 4 PlayStation Controller](https://sketchfab.com/3d-models/e3c2f0dc16524fc19cdde45bad1de1a9) | shaielwolf | CC BY 4.0 |
| Xbox One / Series | [Xbox One X/S (Controller)](https://sketchfab.com/3d-models/294f13ab554f4d4d832b271045bda4fa) | TdoubleU8 | CC BY 4.0 |
| Xbox Elite | [XBOX ELITE 2](https://sketchfab.com/3d-models/8a9ef66d68be4fb9a1f320ce716a2017) | Vrah (Wee.Patt) | CC BY 4.0 |

Per-model attribution notes are in [`assets/models/LICENSES/`](assets/models/LICENSES/). The names and shapes of the
depicted products are trademarks / designs of their respective owners; DriftLab is not affiliated with them.

## Environment lighting (CC0)

* [`studio_small_03`](https://polyhaven.com/a/studio_small_03) HDRI (1k) by Poly Haven – [CC0 1.0](assets/models/LICENSES/CC0-1.0.txt).

## Fonts and icons

* [Inter](https://rsms.me/inter/) typeface – SIL Open Font License 1.1 (`assets/fonts/NOTICE-Inter.md`)
* [Phosphor Icons](https://phosphoricons.com) – MIT (through `egui-pop-phosphor`)

## Protocol references

* [dualshock-tools](https://github.com/dualshock-tools/dualshock-tools.github.io) (MIT) – DS4/DualSense calibration protocol
* Linux `hid-sony.c` / `hid-playstation.c` and SDL `SDL_hidapi_ps4.c` / `SDL_hidapi_ps5.c` – report layout documentation (no code copied)

## Libraries

egui / eframe, wgpu, gltf, glam, hidapi, gilrs, image and the other crates listed in `Cargo.lock` – see their respective licences (checked with `cargo deny`).
