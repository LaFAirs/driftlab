"""PCA-orient a model, list islands, optionally export an unsplit GLB for a visual sign check.
blender -b -P analyze_pca.py -- --in f.glb --width 0.151 --keep Object_11 [--flip xz] [--only Object_11] [--limit 40] [--out test.glb]"""
import os, sys

import bpy

sys.path.insert(0, os.path.dirname(__file__))
import prep_lib as P

P.import_glb(P.arg("--in"))
P.flatten()
if P.arg("--keep"):
    keep = P.arg("--keep").split(",")
    for o in P.meshes():
        if not any(k in o.name for k in keep):
            bpy.data.objects.remove(o)
if P.arg("--drop"):
    P.drop(P.arg("--drop").split(","))
print(P.pca_orient(float(P.arg("--width", "0.16")), P.arg("--flip", ""), (P.arg("--basis") or "").split(",") if P.arg("--basis") else None))
lo, hi = P.bounds()
print(f"size(mm)=({(hi.x-lo.x)*1000:.1f},{(hi.y-lo.y)*1000:.1f},{(hi.z-lo.z)*1000:.1f})")
if P.arg("--objects"):
    for o in P.meshes():
        pts = [v.co for v in o.data.vertices]
        if not pts:
            continue
        lo_ = [min(p[k] for p in pts) * 1000 for k in range(3)]
        hi_ = [max(p[k] for p in pts) * 1000 for k in range(3)]
        print(f"OBJ {o.name[:34]:34s} tris={sum(len(p.vertices)-2 for p in o.data.polygons):6d} c=({(lo_[0]+hi_[0])/2:7.1f},{(lo_[1]+hi_[1])/2:7.1f},{(lo_[2]+hi_[2])/2:7.1f}) size=({hi_[0]-lo_[0]:6.1f},{hi_[1]-lo_[1]:6.1f},{hi_[2]-lo_[2]:6.1f}) mats={[m.name for m in o.data.materials][:2]}")
if P.arg("--only"):
    only = P.arg("--only").split(",")
    for o in P.meshes():
        if not any(k in o.name for k in only):
            continue
        isl = P.islands(o)
        print(f"## {o.name} islands={len(isl)} tris={sum(i['tris'] for i in isl)} mats={[m.name for m in o.data.materials]}")
        for k, i in sorted(enumerate(isl), key=lambda t: -t[1]["tris"])[: int(P.arg("--limit", "40"))]:
            c, sz = i["c"] * 1000, i["size"] * 1000
            print(f"   #{k:3d} tris={i['tris']:6d} c=({c.x:7.1f},{c.y:7.1f},{c.z:7.1f}) size=({sz.x:5.1f},{sz.y:5.1f},{sz.z:5.1f})")
if P.arg("--out"):
    P.export_glb(P.arg("--out"))
