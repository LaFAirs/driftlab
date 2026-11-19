# Credits

## 3D controller models (CC BY 4.0)

The controller models were created by the authors below and are used under the
[Creative Commons Attribution 4.0 International](assets/models/LICENSES/CC-BY-4.0.txt) licence.
They were downloaded from Sketchfab and then **modified** for DriftLab: scaled to real-world size, origins re-centred,
moving parts (sticks, triggers, buttons, d-pad, touchpad, lightbar) separated into named nodes with correct pivots,
brand logos, manufacturer lettering and printed labels removed, meshes decimated, textures resized, and some materials
replaced (see the reproducible Blender scripts in `tools/models/`).

Only **three** source models are used. The two "Edge / Elite" variants are derived from them and extended with our own parts:

| Model in DriftLab | Source model | Author | Own additions |
|---|---|---|---|
| DualSense | [Sony PS5 DualSense Controller](https://sketchfab.com/3d-models/b1d054ee67204ec1984d5fbebd707cde) | be the knight (jayhystic22) | solid satin button caps and d-pad blocks, △ ○ ✕ □ symbols, black touchpad material |
| DualSense Edge | same DualSense model | be the knight (jayhystic22) | same as above **plus four rear paddles** (own modelling) |
| DualShock 4 | [DualShock 4 PlayStation Controller](https://sketchfab.com/3d-models/e3c2f0dc16524fc19cdde45bad1de1a9) | shaielwolf | plain black PS button, lettering painted out of the textures |
| Xbox One / Series | [Xbox One X/S (Controller)](https://sketchfab.com/3d-models/294f13ab554f4d4d832b271045bda4fa) | TdoubleU8 | logo removed, plain glowing guide button, repaired trigger meshes |
| Xbox Elite | same Xbox One model | TdoubleU8 | same as above **plus four rear paddles** (own modelling) |

**Not used:** the Sketchfab models "Dualsense" by miwaVV (mishel1337), "XBOX ELITE 2" by Vrah (Wee.Patt) and the Elite
alternative by Rusfort were evaluated but are not part of DriftLab; they are therefore not credited as used works.

The rear paddles, the face-button symbols and the d-pad blocks are original work made for DriftLab (also CC-compatible
with the project licence). Per-model attribution notes are in [`assets/models/LICENSES/`](assets/models/LICENSES/).
The names and shapes of the depicted products are trademarks / designs of their respective owners; DriftLab is not
affiliated with them. DriftLab Edge and Elite are **approximations** built from the standard-model shapes plus paddles,
not scans of those products.

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

