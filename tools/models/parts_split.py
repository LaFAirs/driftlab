"""Partition meshes into named, animatable groups using island positions (millimetres, normalised frame)."""
import bmesh
import bpy
from mathutils import Vector

import prep_lib as P


def build_groups(rules):
    """rules: callable(mesh_name, island) -> group name or None. Returns {group: [Object, ...]} of new split objects."""
    created = {}
    for src in list(P.meshes()):
        isl = P.islands(src)
        label = {}
        for i in isl:
            g = rules(src.name, i)
            if g:
                for f in i["faces"]:
                    label[f] = g
        if not label:
            continue
        bm = bmesh.new()
        bm.from_mesh(src.data)
        bm.faces.ensure_lookup_table()
        uv_src = bm.loops.layers.uv.active
        groups = {}
        for fi, g in label.items():
            groups.setdefault(g, []).append(fi)
        for g, faces in groups.items():
            nb = bmesh.new()
            vmap = {}
            uv_dst = nb.loops.layers.uv.new("UVMap") if uv_src else None
            for fi in faces:
                f = bm.faces[fi]
                nv = []
                for v in f.verts:
                    if v.index not in vmap:
                        vmap[v.index] = nb.verts.new(v.co)
                    nv.append(vmap[v.index])
                try:
                    nf = nb.faces.new(nv)
                except ValueError:
                    continue
                nf.material_index = f.material_index
                nf.smooth = f.smooth
                if uv_src:
                    for lo, ln in zip(f.loops, nf.loops):
                        ln[uv_dst].uv = lo[uv_src].uv
            me = bpy.data.meshes.new(f"{g}_{src.name[:12]}")
            nb.to_mesh(me)
            nb.free()
            for m in src.data.materials:
                me.materials.append(m)
            ob = bpy.data.objects.new(me.name, me)
            bpy.context.collection.objects.link(ob)
            created.setdefault(g, []).append(ob)
        bmesh.ops.delete(bm, geom=[bm.faces[fi] for fi in label], context="FACES")
        bm.to_mesh(src.data)
        bm.free()
    return created


def merge(objs, name):
    """Join several objects into one named object."""
    for o in bpy.context.scene.objects:
        o.select_set(False)
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    bpy.ops.object.join()
    ob = bpy.context.view_layer.objects.active
    ob.name = name
    ob.data.name = name
    return ob


def bbox(ob):
    pts = [v.co for v in ob.data.vertices]
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    return lo, hi


def cleanup_empty():
    for o in list(P.meshes()):
        if len(o.data.polygons) == 0:
            bpy.data.objects.remove(o)
