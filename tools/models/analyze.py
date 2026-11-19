"""Auto-orient a model and list islands of selected meshes.
blender -b -P analyze.py -- --in f.glb --width 0.162 --sticks Object_16 --triggers Object_22 [--only Object_19,Object_22] [--keep names] [--drop names] [--min 40]"""
import os, sys

sys.path.insert(0, os.path.dirname(__file__))
import prep_lib as P

P.import_glb(P.arg("--in"))
P.flatten()
if P.arg("--keep"):
    keep = P.arg("--keep").split(",")
    for o in P.meshes():
        if not any(k in o.name for k in keep):
            import bpy
            bpy.data.objects.remove(o)
if P.arg("--drop"):
    P.drop(P.arg("--drop").split(","))
print(P.auto_orient(float(P.arg("--width", "0.16")), P.arg("--sticks", "stick").split(","), P.arg("--triggers", "trigger").split(",")))
lo, hi = P.bounds()
print(f"size(mm)=({(hi.x-lo.x)*1000:.1f},{(hi.y-lo.y)*1000:.1f},{(hi.z-lo.z)*1000:.1f})")
only = P.arg("--only")
only = only.split(",") if only else None
limit = int(P.arg("--limit", "30"))
for o in P.meshes():
    tris = sum(len(p.vertices) - 2 for p in o.data.polygons)
    if only and not any(k in o.name for k in only):
        print(f"## {o.name} tris={tris} mats={[m.name for m in o.data.materials]}")
        continue
    isl = P.islands(o)
    print(f"## {o.name} islands={len(isl)} tris={tris} mats={[m.name for m in o.data.materials]}")
    for k, i in sorted(enumerate(isl), key=lambda t: -t[1]["tris"])[:limit]:
        c, sz = i["c"] * 1000, i["size"] * 1000
        print(f"   #{k:3d} tris={i['tris']:6d} c=({c.x:7.1f},{c.y:7.1f},{c.z:7.1f}) size=({sz.x:5.1f},{sz.y:5.1f},{sz.z:5.1f})")
