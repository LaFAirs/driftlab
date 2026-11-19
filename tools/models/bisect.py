"""Debug: import -> (optional steps) -> export, to find which prep step breaks a model.
blender -b -P bisect.py -- --in f.glb --out o.glb --steps flatten,normalise,decimate"""
import os, sys

sys.path.insert(0, os.path.dirname(__file__))
import prep_lib as P

steps = (P.arg("--steps") or "").split(",")
P.import_glb(P.arg("--in"))
if "flatten" in steps:
    P.flatten()
if "droplogo" in steps:
    P.drop(['Logo'])
if "normalise" in steps:
    P.normalise(0.16, rot_z_deg=180)
if "decimate" in steps:
    for o in P.meshes():
        n = len(o.data.polygons)
        if n > 12000:
            P.decimate(o, 0.42)
        elif n > 4000:
            P.decimate(o, 0.6)
if "emission" in steps:
    P.fix_emission(keep=("Emmissive",))
if "shrink" in steps:
    P.shrink_images(2048)
if "rename" in steps:
    for o in P.meshes():
        o.name = 'part_' + o.name[-6:]
print("TRIANGLES", P.tri_count())
P.export_glb(P.arg("--out"))
