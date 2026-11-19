"""List the loose parts (islands) of every mesh after normalising: blender -b -P islands.py -- --in f.glb --width 0.16 --rotz 180 [--only substr]"""
import sys, os

sys.path.insert(0, os.path.dirname(__file__))
import prep_lib as P

P.import_glb(P.arg("--in"))
P.flatten()
drop = P.arg("--drop")
if drop:
    P.drop(drop.split(","))
s = P.normalise(float(P.arg("--width", "0.16")), float(P.arg("--rotz", "0")), float(P.arg("--rotx", "0")))
only = P.arg("--only")
lo, hi = P.bounds()
print(f"scale={s:.5f}  size(mm)=({(hi.x-lo.x)*1000:.1f},{(hi.y-lo.y)*1000:.1f},{(hi.z-lo.z)*1000:.1f})")
for o in P.meshes():
    if only and only not in o.name:
        continue
    isl = P.islands(o)
    print(f"## {o.name}  islands={len(isl)} tris={sum(i['tris'] for i in isl)}")
    for k, i in sorted(enumerate(isl), key=lambda t: -t[1]["tris"])[:40]:
        c, sz = i["c"] * 1000, i["size"] * 1000
        print(f"   #{k:3d} tris={i['tris']:6d} c=({c.x:7.1f},{c.y:7.1f},{c.z:7.1f}) size=({sz.x:5.1f},{sz.y:5.1f},{sz.z:5.1f})")
