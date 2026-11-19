"""Prepare 'DualShock 4 PlayStation Controller' by shaielwolf (CC BY 4.0) for DriftLab.

blender -b -P tools/models/prep_ds4.py -- --in assets/models/incoming/dualshock4.glb --out assets/models/dualshock4.glb
Changes: re-scaled to 162 mm and re-oriented, PS-button logo replaced by a plain black surface, moving parts split into
named nodes with real pivots, textures limited to 2K.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import bpy
from mathutils import Vector

import parts_split as S
import prep_lib as P

P.import_glb(P.arg("--in"))
# Printed lettering (SHARE / OPTIONS / EXT, the "SONY" wordmark and the regulatory label) is painted out of the base colour maps.
P.whiten_image("Image_0", [(0.30, 0.30, 0.39, 0.36), (0.62, 0.30, 0.72, 0.36), (0.45, 0.60, 0.57, 0.66)])
P.whiten_image("Image_2", [(0.42, 0.03, 0.58, 0.08), (0.19, 0.50, 0.81, 0.67)])
P.flatten()
print(P.auto_orient(0.162, ["Object_16"], ["Object_22"]))
MM = 0.001

DPAD = {"dpad_up": Vector((-50.8, 28.1)), "dpad_down": Vector((-50.8, 13.0)), "dpad_left": Vector((-58.4, 20.5)), "dpad_right": Vector((-43.3, 20.5))}
FACE = {"face_triangle": Vector((50.9, 32.2)), "face_cross": Vector((50.9, 8.9)), "face_square": Vector((39.2, 20.5)), "face_circle": Vector((62.5, 20.5))}


def rules(mesh, i):
    c, sz = i["c"] * 1000, i["size"] * 1000
    xy = Vector((c.x, c.y))
    if "stick" not in (P.arg("--skip") or "") and "Object_16" in mesh:  # the whole stick (dome + dished top) tilts together
        return "stick_l" if c.x < 0 else "stick_r"
    if "Object_22" in mesh:
        side = "l" if c.x < 0 else "r"
        return f"bumper_{side}" if c.z > 2 else f"trigger_{side}"
    if "Object_13" in mesh:
        return "touchpad"
    if "Object_19" in mesh:
        if abs(c.x) < 4:
            return "btn_home"
        if abs(abs(c.x) - 33.1) < 3 and c.y > 30:
            return "btn_share" if c.x < 0 else "btn_options"
        best = None
        for name, ctr in {**DPAD, **FACE}.items():
            d = (xy - ctr).length
            if d < 7 and (best is None or d < best[0]):
                best = (d, name)
        return best[1] if best else None
    return None


groups = S.build_groups(rules)
parts = {}
for g, objs in groups.items():
    ob = S.merge(objs, g)
    lo, hi = S.bbox(ob)
    cx, cy = (lo.x + hi.x) / 2, (lo.y + hi.y) / 2
    if g.startswith("stick"):
        piv = Vector((cx, cy, lo.z + 2.0 * MM))
    elif g.startswith("trigger"):
        piv = Vector((cx, lo.y + 0.3 * (hi.y - lo.y), hi.z))
    elif g in ("touchpad", "btn_home") or g.startswith("bumper"):
        piv = Vector((cx, cy, lo.z))
    else:
        piv = Vector((cx, cy, lo.z))
    P.set_origin(ob, piv)
    parts[g] = ob
S.cleanup_empty()

# Plain black PS button (no logo): replace the material of that part.
if "btn_home" in parts:
    black = bpy.data.materials.new("ps_button_black")
    black.use_nodes = True
    bsdf = black.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (0.012, 0.012, 0.014, 1)
    bsdf.inputs["Roughness"].default_value = 0.32
    ob = parts["btn_home"]
    ob.data.materials.clear()
    ob.data.materials.append(black)
    for p in ob.data.polygons:
        p.material_index = 0

for g in ("stick_l", "stick_r"):
    if g in parts:
        P.solid_material(parts[g], 'stick_rubber_grip', (0.018, 0.018, 0.02), 0.95)

for o in P.meshes():
    if 'Object_16' in o.name and o not in parts.values():
        P.solid_material(o, 'stick_rubber_grip', (0.018, 0.018, 0.02), 0.95)

names = {"Object_7": "body_front", "Object_10": "body_back", "Object_16": "stick_bases", "Object_19": "buttons_static", "Object_22": "trigger_static", "Object_25": "headphone_jack", "Object_28": "screws_usb"}
for o in P.meshes():
    if o in parts.values():
        continue
    for k, v in names.items():
        if k in o.name:
            o.name = v

print("TRIANGLES", P.tri_count(), "parts:", sorted(parts))
if P.arg("--noemfix") is None:
    P.fix_emission(keep=(P.arg("--glow") or "NOGLOW",))
P.shrink_images(2048)
P.export_glb(P.arg("--out"))
