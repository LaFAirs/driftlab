"""Prepare 'Sony PS5 DualSense Controller' by be the knight (CC BY 4.0) for DriftLab.

blender -b -P tools/models/prep_dualsense.py -- --in assets/models/incoming/dualsense.glb --out assets/models/dualsense.glb [--preview x.png]
Changes: re-scaled to 160 mm, re-oriented (triggers +Y), PS logo removed, moving parts split into named nodes with real pivots,
static meshes decimated, textures limited to 2K.
"""
import math
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import bpy
from mathutils import Vector

import parts_split as S
import prep_lib as P

P.import_glb(P.arg("--in"))
P.flatten()
P.drop(["Logo"])  # PlayStation logo geometry
# Printed "SONY" lettering lives in the white base-colour texture: paint it out (no manufacturer lettering in the app).
P.whiten_image("Image_0", [(0.26, 0.500, 0.41, 0.580), (0.73, 0.435, 0.88, 0.515)])
P.normalise(0.16, rot_z_deg=180)

MM = 0.001

STICK = {"l": Vector((-25.9, -1.2)), "r": Vector((26.2, -1.2))}
FACE = {"face_triangle": Vector((49.4, 37.7)), "face_cross": Vector((49.4, 14.3)), "face_square": Vector((39.7, 25.8)), "face_circle": Vector((60.0, 25.5))}
DPAD = {"dpad_up": Vector((-49.0, 33.0)), "dpad_down": Vector((-49.0, 17.6)), "dpad_left": Vector((-57.6, 25.7)), "dpad_right": Vector((-41.7, 25.4))}


SKIP = (P.arg('--skip') or '').split(',')


def rules(mesh, i):
    c = i["c"] * 1000
    sz = i["size"] * 1000
    xy = Vector((c.x, c.y))
    # sticks: caps / shafts above the housing
    for side, ctr in STICK.items():
        if 'stick' not in SKIP and (xy - ctr).length < 12 and c.z > 22 and max(sz.x, sz.y) < 22 and any(k in mesh for k in ("Thumb_Top", "Front_Base")):
            return f"stick_{side}"
    # triggers vs bumpers
    if "trigger" not in SKIP and "Trigger_Buttons" in mesh and abs(c.x) > 40:
        side = "l" if c.x < 0 else "r"
        return f"trigger_{side}" if c.z < -2.5 else None  # upper parts stay with the (white) shoulder shell
    # face buttons and d-pad arms (caps, symbol strokes, glossy overlays)
    if 'face' not in SKIP and c.z > 4 and any(k in mesh for k in (("Buttons", "Transparent") if P.arg("--keepsp") is None else ("Side_Panels", "Buttons", "Transparent"))) and max(sz.x, sz.y) < 16:
        best = None
        for name, ctr in {**FACE, **DPAD}.items():
            d = (xy - ctr).length
            if d < 7.5 and (best is None or d < best[0]):
                best = (d, name)
        if best:
            return best[1]
    return None


groups = S.build_groups(rules)
parts = {}
for g, objs in groups.items():
    ob = S.merge(objs, g)
    lo, hi = S.bbox(ob)
    if g.startswith("stick"):
        piv = Vector(((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, lo.z + 1.0 * MM))
    elif g.startswith("trigger"):
        piv = Vector(((lo.x + hi.x) / 2, lo.y + 0.25 * (hi.y - lo.y), hi.z))
    elif g.startswith("bumper"):
        piv = Vector(((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, (lo.z + hi.z) / 2))
    else:  # face / dpad
        piv = Vector(((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, lo.z))
    P.set_origin(ob, piv)
    parts[g] = ob
S.cleanup_empty()

# The source's glass-like cap overlays read as clear blobs: use solid satin caps and our own thin symbols in the original colours.
if P.arg("--keepglass") is None:
    SYM = {
        "face_triangle": ("triangle", (0.10, 0.62, 0.50)),
        "face_circle": ("circle", (0.80, 0.18, 0.24)),
        "face_cross": ("cross", (0.20, 0.38, 0.85)),
        "face_square": ("square", (0.78, 0.40, 0.70)),
    }
    for g in list(parts):
        if g.startswith("dpad"):
            # The source d-pad arms are open shells with their recess walls: rebuild each arm as a clean bevelled block (own modelling).
            import bmesh

            ob = parts[g]
            lo, hi = S.bbox(ob)
            vertical = g in ("dpad_up", "dpad_down")
            sx, sy = (8.6, 10.6) if vertical else (10.6, 8.6)
            bm = bmesh.new()
            vs = bmesh.ops.create_cube(bm, size=1.0)["verts"]
            bmesh.ops.scale(bm, vec=Vector((sx * MM, sy * MM, (hi.z - lo.z))), verts=vs)
            bmesh.ops.translate(bm, vec=Vector(((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, (lo.z + hi.z) / 2)), verts=vs)
            top_edges = [e for e in bm.edges if all(v.co.z > (lo.z + hi.z) / 2 for v in e.verts)]
            bmesh.ops.bevel(bm, geom=top_edges, offset=1.1 * MM, segments=3, affect="EDGES")
            ob.data.clear_geometry()
            bm.to_mesh(ob.data)
            bm.free()
            for p in ob.data.polygons:
                p.use_smooth = True
            P.solid_material(ob, "cap_ps5", (0.80, 0.81, 0.84), 0.38)
        elif g in SYM:
            ob = parts[g]
            lo, hi = P.world_bbox(ob)
            P.solid_material(ob, "cap_ps5", (0.80, 0.81, 0.84), 0.38)
            kind, rgb = SYM[g]
            sym = P.add_symbol(kind, kind, (lo.x + hi.x) / 2 / MM, (lo.y + hi.y) / 2 / MM, hi.z / MM, 7.4, rgb, w_mm=1.0)
            parts[g] = S.merge([ob, sym], g)

# Decimate the static (non-animated) meshes to stay well below 150k triangles.
animated = set(parts.values())
for o in ([] if P.arg('--nodec') is not None else P.meshes()):
    if o in animated:
        continue
    n = len(o.data.polygons)
    if n > 12000:
        P.decimate(o, 0.42)
    elif n > 4000:
        P.decimate(o, 0.6)
for g in ("trigger_l", "trigger_r"):
    if g in parts and len(parts[g].data.polygons) > 8000:
        P.decimate(parts[g], 0.5)

# Rename static meshes to something readable.
names = {"Back_Baseq": "body_back", "Front_Base": "body_front", "Side_Panels": "body_side", "Default_Material": "body_screws", "Emmissive": "lightbar", "Transparent": "overlay", "Buttons": "buttons_static", "Port": "port", "SPCB": "btn_side", "FBCB": "btn_fb", "Gold": "contacts", "Front_Panel": "touchpad_frame"}
for o in P.meshes():
    if o in animated:
        continue
    for k, v in names.items():
        if k in o.name:
            o.name = v
            break

for _g in ("trigger_l", "trigger_r"):
    if _g in parts:
        P.solid_material(parts[_g], "cap_ps5", (0.80, 0.81, 0.84), 0.38)

# Touchpad as black glass (the source renders it as white plastic).
if P.arg("--whitepad") is None:
    for o in P.meshes():
        if o.name == "touchpad_frame":
            P.solid_material(o, "touchpad_black", (0.008, 0.008, 0.010), 0.16)

if P.arg("--edge") is not None:
    # DualSense Edge additions (own modelling): two rear paddles on each side.
    for s, n in ((-1, "l"), (1, "r")):
        P.add_paddle(f"paddle_{n}_inner", s * 58.0, -30.0, 42.0, 9.0, drop_mm=-0.9)
        P.add_paddle(f"paddle_{n}_outer", s * 68.5, -32.0, 38.0, 9.0, drop_mm=-0.9)

print("TRIANGLES", P.tri_count(), "parts:", sorted(parts))
if P.arg('--noemfix') is None:
    P.fix_emission(keep=('Emmissive',))
if P.arg('--opaque') is not None:
    P.make_opaque(['Transparent'])
if P.arg('--noshrink') is None:
    P.shrink_images(2048)
if P.arg("--preview"):
    P.preview(P.arg("--preview"), P.arg("--angle", "persp"))
P.export_glb(P.arg("--out"))




