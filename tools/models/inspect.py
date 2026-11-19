"""Inspect an imported GLB: object list, triangle counts, loose parts, bounding box, plus a quick test render.

blender -b -P tools/models/inspect.py -- --in assets/models/incoming/dualsense.glb --preview out.png [--angle top|persp|back|front]
"""
import sys

import bpy
from mathutils import Vector


def arg(name, default=None):
    a = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    return a[a.index(name) + 1] if name in a else default


bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=arg("--in"))

meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
lo = Vector((1e9, 1e9, 1e9))
hi = Vector((-1e9, -1e9, -1e9))
total = 0
print("=== OBJECTS ===")
for o in meshes:
    me = o.data
    tris = sum(len(p.vertices) - 2 for p in me.polygons)
    total += tris
    corners = [o.matrix_world @ Vector(c) for c in o.bound_box]
    for c in corners:
        lo = Vector((min(lo.x, c.x), min(lo.y, c.y), min(lo.z, c.z)))
        hi = Vector((max(hi.x, c.x), max(hi.y, c.y), max(hi.z, c.z)))
    size = [max(c[i] for c in corners) - min(c[i] for c in corners) for i in range(3)]
    mats = [m.name for m in me.materials if m]
    print(f"{o.name[:40]:40s} tris={tris:7d} size=({size[0]:.3f},{size[1]:.3f},{size[2]:.3f}) mats={mats} parent={o.parent.name if o.parent else None}")
print(f"TOTAL tris={total} bbox size=({hi.x-lo.x:.3f},{hi.y-lo.y:.3f},{hi.z-lo.z:.3f}) centre=({(hi.x+lo.x)/2:.3f},{(hi.y+lo.y)/2:.3f},{(hi.z+lo.z)/2:.3f})")

out = arg("--preview")
if out:
    sc = bpy.context.scene
    sc.render.engine = "CYCLES"
    sc.cycles.device = "CPU"
    sc.cycles.samples = 16
    sc.cycles.use_denoising = False
    sc.render.resolution_x, sc.render.resolution_y = 1000, 700
    sc.render.filepath = out
    w = bpy.data.worlds.new("w")
    w.use_nodes = True
    w.node_tree.nodes["Background"].inputs[0].default_value = (0.5, 0.52, 0.55, 1)
    w.node_tree.nodes["Background"].inputs[1].default_value = 0.8
    sc.world = w
    centre = (hi + lo) / 2
    diag = (hi - lo).length
    sun = bpy.data.lights.new("l", "SUN")
    sun.energy = 3.0
    so = bpy.data.objects.new("l", sun)
    bpy.context.collection.objects.link(so)
    so.rotation_euler = (0.7, 0.2, 0.6)
    cam = bpy.data.cameras.new("c")
    co = bpy.data.objects.new("c", cam)
    bpy.context.collection.objects.link(co)
    sc.camera = co
    cam.lens = 70
    cam.clip_start = diag / 100
    cam.clip_end = diag * 20
    # Models arrive Y-up (glTF -> Blender converts to Z-up). Look from above-front.
    ang = arg("--angle", "persp")
    offs = {"top": Vector((0, -0.05, 1.0)), "persp": Vector((0, -0.8, 0.8)), "front": Vector((0, -1.0, 0.25)), "back": Vector((0, 1.0, 0.5))}[ang]
    co.location = centre + offs.normalized() * diag * 2.6
    co.rotation_euler = (centre - co.location).to_track_quat("-Z", "Y").to_euler()
    bpy.ops.render.render(write_still=True)
