"""DualSense / DualSense Edge model (original modelling; no brand logos).
Dimensions from public specs: ~160 x 66 x 106 mm (W x H x D). Run via Blender, see common.py.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from parts import *  # noqa: F401,F403
import common as C
from common import MM, hexc, material
import parts as P

A = C.args()
EDGE = "edge" in os.path.basename(A["out"] or "") if A["out"] else False
C.reset()

m = {
    "white": material("shell_white", hexc(0xE8EAEE), 0.52),
    "black": material("shell_black_gloss", hexc(0x0D0E11), 0.16),
    "bezel": material("bezel_black", hexc(0x111215), 0.45),
    "rubber": material("stick_rubber_grip", hexc(0x18191C), 0.62),
    "face_cap": material("face_cap_light", hexc(0xDADCE1), 0.32) if not EDGE else material("face_cap_dark", hexc(0x1B1C20), 0.3),
    "dpad": material("dpad_light", hexc(0xDADCE1), 0.35) if not EDGE else material("dpad_dark", hexc(0x1B1C20), 0.32),
    "dpad_mark": material("dpad_mark", hexc(0x2A2B30), 0.5),
    "touchpad": material("touchpad_black", hexc(0x131416), 0.28),
    "btn_dark": material("btn_dark_gloss", hexc(0x16171A), 0.2),
    "btn_light": material("btn_light", hexc(0xCFD1D6), 0.4),
    "trigger": material("trigger_white", hexc(0xEDEFF2), 0.45) if not EDGE else material("trigger_black", hexc(0x131417), 0.4),
    "port": material("port_dark", hexc(0x050506), 0.8),
    "glow": material("glow_lightbar", (0.05, 0.3, 1.0), 0.3, emission=(0.1, 0.4, 1.0), strength=4.0),
    "metal": material("metal_steel", hexc(0x9AA0A8), 0.3, metal=1.0),
}
if EDGE:
    m["white"] = material("shell_white", hexc(0xE1E3E7), 0.5)

half = [(0, 37), (30, 38), (52, 36), (66, 30), (75, 18), (80, 2), (80, -14), (77, -32), (70, -52), (60, -66), (50, -66), (43, -55), (36, -38), (26, -30), (12, -28), (0, -28)]
outline = C.mirror_outline(half)
blobs = []
for s in (-1, 1):
    blobs.append(((s * 63, -36, -3), (15, 36, 19), (0, 0, s * -14)))
body = C.build_body("body", outline, 29, blobs, [m["white"], m["black"]], voxel_mm=0.95, round_mm=9.5)


K = 4.0 / 36.0  # border slope below y = 6 mm


def band(y):
    # Black central bridge: straight at the touchpad, slightly wider around the sticks (piecewise linear).
    return 31.0 if y > 6 else min(31.0 + (6 - y) * K, 35.0)


def region(x, y, z, nz):
    return 1 if abs(x) < band(y) else 0


import math as _m

_len = _m.hypot(1.0, K)
cuts = []
for s in (-1, 1):
    cuts.append(((s * 31.0, 0, 0), (1, 0, 0)))
    # plane through (s*31, 6) running towards (s*35, -30): normal is perpendicular to that direction
    cuts.append(((s * 31.0, 6.0, 0), (1.0 / _len, s * K / _len * 1.0, 0)))
C.bisect(body, cuts)
C.assign_regions(body, region)

# Sticks, d-pad, face buttons
P.stick(body, -1, -21.5, -14, m, radius=9.7)
P.stick(body, 1, 21.5, -14, m, radius=9.7)
P.dpad(body, -46, 12, m, arm_w=10.2, arm_len=15.0)
P.face_buttons_ps(body, 46, 12, m, spacing=12.0, r=6.1)

# Centre cluster: touchpad, PS button (smooth black, no logo), mute, create / options
P.touchpad(body, 0, 19, 52, 27, m, r=5.0)
P.disc_button("btn_home", body, 0, -5.5, 5.4, m, "btn_dark", h=2.0)
P.oval_button("btn_mute", body, 0, -19.5, 9.0, 4.4, m, "btn_dark")
P.oval_button("btn_create", body, -35.5, 25, 8.5, 4.0, m, "btn_light", rot_deg=-40)
P.oval_button("btn_options", body, 35.5, 25, 8.5, 4.0, m, "btn_light", rot_deg=40)

# Lightbar strips along both sides of the touchpad
for s in (-1, 1):
    pts = [(s * 29.5, 8), (s * 30.5, 14), (s * 30.5, 24), (s * 28.5, 31), (s * 24.5, 35)]
    P.ribbon("lightbar_l" if s < 0 else "lightbar_r", pts, 1.6, [m["glow"]], 0, height_mm=0.9, body=body)

# Speaker grille and ports
dots_c = [(dx, -23.5 - r * 1.8, C.surface_z(body, dx, -23.5 - r * 1.8)[0] + 0.1) for r in range(2) for dx in (-6, -3, 0, 3, 6)]
P.dots("speaker", dots_c, 0.55, 0, m, "port")
P.slot("port_usbc", 0, 37.4, 0, 8.4, 2.8, m, "port", "front")
P.disc_button("jack", body, 0, -26.0, 1.9, m, "port", h=0.6, z_off=-1.4)

# Bumpers and triggers (white; Edge uses black triggers)
for s, n in ((-1, "l"), (1, "r")):
    bz = C.surface_z(body, s * 52, 29)[0]
    P.bumper(f"bumper_{n}", s * 52, 29, bz - 2.0, m, "trigger", width=42, depth=10.5, height=7.5)
    P.trigger(f"trigger_{n}", s * 52, 36.5, bz - 6.0, m, "trigger", width=26)

if EDGE:
    paddle = material("paddle_black", hexc(0x141518), 0.45)
    for s, n in ((-1, "l"), (1, "r")):
        ob = C.ellipsoid(f"paddle_{n}", (s * 58, -52, -16), (4.5, 20, 3.0), [paddle], (0, 0, s * -18))
        for p in ob.data.polygons:
            p.use_smooth = True
    # Edge: small function-button strip under the sticks
    P.oval_button("btn_fn_l", body, -17, -28.5, 7.0, 3.4, m, "btn_dark")
    P.oval_button("btn_fn_r", body, 17, -28.5, 7.0, 3.4, m, "btn_dark")

# Centre the whole controller on the origin (so the app can rotate it around its middle).
if A["preview"]:
    C.preview(A["preview"], A["angle"])
if A["out"]:
    C.export_glb(A["out"])
