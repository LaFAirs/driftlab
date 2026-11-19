"""Prepare 'Xbox One X/S' by TdoubleU8 (CC BY 4.0) for DriftLab.

blender -b -P tools/models/prep_xbox.py -- --in assets/models/incoming/xbox.glb --out assets/models/xbox.glb
The source file contains two consoles and two controllers; only the black controller (mesh 'Object_11') is kept.
Changes: consoles/white controller removed, tilted pose aligned by principal axes, scaled to 151 mm, Xbox logo on the guide
button replaced by a plain white glowing surface, moving parts split into named nodes with real pivots, textures <= 2K.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import bpy
from mathutils import Vector

import parts_split as S
import prep_lib as P

P.import_glb(P.arg("--in"))
P.flatten()
for o in P.meshes():
    if "Object_11" not in o.name:
        bpy.data.objects.remove(o)
print(P.pca_orient(0.151, flip="y"))
MM = 0.001
# The raised XBOX lettering/logo on the front edge between the triggers is made of small loose islands: delete them
# (no manufacturer lettering in the app).
import bmesh

for _o in P.meshes():
    _bm = bmesh.new()
    _bm.from_mesh(_o.data)
    _seen, _kill = set(), []
    for _f in _bm.faces:
        if _f.index in _seen:
            continue
        _seen.add(_f.index)
        _st, _isl = [_f], []
        while _st:
            _c = _st.pop()
            _isl.append(_c)
            for _e in _c.edges:
                for _n in _e.link_faces:
                    if _n.index not in _seen:
                        _seen.add(_n.index)
                        _st.append(_n)
        _vs = [v.co * 1000 for c in _isl for v in c.verts]
        _lo = [min(v[i] for v in _vs) for i in range(3)]
        _hi = [max(v[i] for v in _vs) for i in range(3)]
        if max(_hi[i] - _lo[i] for i in range(3)) < 24 and 37 < _lo[1] and _hi[1] < 44 and -14 < _lo[0] and _hi[0] < 14 and _hi[2] < 2:
            _kill += _isl
    if _kill:
        bmesh.ops.delete(_bm, geom=_kill, context="FACES")
        # close the cut-outs the letters sat in, so the cover is a plain closed surface again
        _edges = [
            e for e in _bm.edges
            if e.is_boundary and all(-18 < v.co.x * 1000 < 18 and 36 < v.co.y * 1000 < 46 and v.co.z * 1000 < 0 for v in e.verts)
        ]
        _res = bmesh.ops.holes_fill(_bm, edges=_edges, sides=0)
        bmesh.ops.recalc_face_normals(_bm, faces=_res["faces"])
        print("HOLE FACES", len(_res["faces"]))
        _bm.to_mesh(_o.data)
        print("LOGO FACES REMOVED", len(_kill))
    _bm.free()

FACE = {"face_y": Vector((38.8, 32.8)), "face_b": Vector((48.8, 22.1)), "face_x": Vector((28.4, 22.3)), "face_a": Vector((38.5, 11.9))}
STICK = {"stick_l": Vector((-37.1, 22.4)), "stick_r": Vector((19.5, -1.4))}
DPAD_C = Vector((-19.2, -2.4))
GUIDE_C = Vector((0.5, 38.5))
VIEW_C, MENU_C = Vector((-10.4, 22.7)), Vector((11.2, 22.7))


def rules(mesh, i):
    c, sz = i["c"] * 1000, i["size"] * 1000
    xy = Vector((c.x, c.y))
    if max(sz.x, sz.y) > 40:  # housing shells
        return None
    for name, ctr in STICK.items():
        if (xy - ctr).length < 13 and c.z > 20:
            return name
    if (xy - DPAD_C).length < 12 and c.z > 15:
        return "dpad"
    best = None
    for name, ctr in FACE.items():
        d = (xy - ctr).length
        if d < 7.5 and c.z > 12 and (best is None or d < best[0]):
            best = (d, name)
    if best:
        return best[1]
    if (xy - GUIDE_C).length < 8 and c.z > 12:
        return "btn_guide"
    if (xy - VIEW_C).length < 5 and c.z > 12:
        return "btn_view"
    if (xy - MENU_C).length < 5 and c.z > 12:
        return "btn_menu"
    if c.y > 38 and c.z > 3 and 30 < sz.x < 40 and abs(c.x) > 30:
        return "bumper_l" if c.x < 0 else "bumper_r"
    if c.y > 38 and c.z < -5 and sz.x < 25 and abs(c.x) > 35:
        return "trigger_l" if c.x < 0 else "trigger_r"
    return None


groups = S.build_groups(rules)
parts = {}
for g, objs in groups.items():
    ob = S.merge(objs, g)
    lo, hi = S.bbox(ob)
    cx, cy = (lo.x + hi.x) / 2, (lo.y + hi.y) / 2
    if g.startswith("stick"):
        piv = Vector((cx, cy, lo.z))
    elif g.startswith("trigger"):
        piv = Vector((cx, lo.y + 0.3 * (hi.y - lo.y), hi.z))
    else:
        piv = Vector((cx, cy, lo.z))
    P.set_origin(ob, piv)
    parts[g] = ob
S.cleanup_empty()

# Trigger / bumper meshes carry stacked duplicate layers that z-fight (white speckles): one plain black satin material hides it.
for g in ("trigger_l", "trigger_r", "bumper_l", "bumper_r"):
    if g in parts:
        P.solid_material(parts[g], "trigger_black", (0.012, 0.012, 0.014), 0.92)
        # inconsistent / doubled faces produce white sparkles: weld, orient consistently, flat-shade
        _bm = bmesh.new()
        _bm.from_mesh(parts[g].data)
        bmesh.ops.remove_doubles(_bm, verts=_bm.verts, dist=0.00005)
        bmesh.ops.recalc_face_normals(_bm, faces=_bm.faces)
        _bm.to_mesh(parts[g].data)
        _bm.free()
        for _p in parts[g].data.polygons:
            _p.use_smooth = True

if "btn_guide" in parts:  # plain white glowing disc, no logo
    m = bpy.data.materials.new("glow_guide")
    m.use_nodes = True
    b = m.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = (0.9, 0.92, 0.9, 1)
    b.inputs["Roughness"].default_value = 0.3
    b.inputs["Emission Color"].default_value = (1, 1, 1, 1)
    b.inputs["Emission Strength"].default_value = 2.0
    ob = parts["btn_guide"]
    ob.data.materials.clear()
    ob.data.materials.append(m)
    for p in ob.data.polygons:
        p.material_index = 0

for o in P.meshes():
    if o not in parts.values():
        o.name = "body"

# The grip tips have broken UVs (white speckle patterns): give those faces a plain matte-black material.
for o in P.meshes():
    if o.name.startswith("body"):
        # identical to the trigger material, so coincident layers (z-fighting) are indistinguishable
        tip = bpy.data.materials.get("trigger_black")
        if tip is None:
            tip = bpy.data.materials.new("trigger_black")
            tip.use_nodes = True
            tb = tip.node_tree.nodes["Principled BSDF"]
            tb.inputs["Base Color"].default_value = (0.012, 0.012, 0.014, 1)
            tb.inputs["Roughness"].default_value = 0.92
        o.data.materials.append(tip)
        idx = len(o.data.materials) - 1
        n = 0
        for p in o.data.polygons:
            c = p.center * 1000
            if P.arg("--tipfix") is not None and ((c.y < -33 and abs(c.x) > 42) or (c.y > 20 and abs(c.x) > 36 and c.z < -2)):
                p.material_index = idx
                n += 1
        print("GRIP TIP FACES", n)

if P.arg("--elite") is not None:
    # Elite additions (own modelling): four rear paddles. The matte, darker finish is applied by the renderer's style table.
    for s, n in ((-1, "l"), (1, "r")):
        P.add_paddle(f"paddle_{n}_inner", s * 51.0, -28.0, 38.0, 8.0, drop_mm=-0.9)
        P.add_paddle(f"paddle_{n}_outer", s * 62.0, -30.0, 34.0, 8.0, drop_mm=-0.9)

print("TRIANGLES", P.tri_count(), "parts:", sorted(parts))
P.fix_emission(keep=("glow_guide",))
P.shrink_images(2048)
P.export_glb(P.arg("--out"))


