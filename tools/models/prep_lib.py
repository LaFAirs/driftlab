"""Shared helpers to turn a downloaded (CC BY) controller model into a DriftLab asset.

Conventions of the output (Blender space): +X right, +Y towards the triggers (away from the player),
+Z up, origin at the middle of the housing, real-world size in metres. The glTF exporter converts to Y-up.
"""
import math
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector


def arg(name, default=None):
    a = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    return a[a.index(name) + 1] if name in a else default


def import_glb(path):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=path)


def meshes():
    return [o for o in bpy.context.scene.objects if o.type == "MESH"]


def flatten():
    """Bake world transforms into the meshes and drop the node hierarchy / empties."""
    for o in meshes():
        mw = o.matrix_world.copy()
        o.parent = None
        o.data.transform(mw)
        o.matrix_world = Matrix.Identity(4)
        if mw.determinant() < 0:
            # Mirrored node: baking the transform inverts the surface orientation, so reverse the winding.
            bm = bmesh.new()
            bm.from_mesh(o.data)
            bmesh.ops.reverse_faces(bm, faces=list(bm.faces))
            bm.to_mesh(o.data)
            bm.free()
            print("flatten: reversed winding of", o.name)
    for o in list(bpy.context.scene.objects):
        if o.type != "MESH":
            bpy.data.objects.remove(o)


def keep_only(names):
    for o in meshes():
        if o.name not in names:
            bpy.data.objects.remove(o)


def drop(names_contains):
    for o in meshes():
        if any(s in o.name for s in names_contains):
            bpy.data.objects.remove(o)


def bounds(objs=None):
    objs = objs or meshes()
    lo = Vector((1e9,) * 3)
    hi = Vector((-1e9,) * 3)
    for o in objs:
        for v in o.data.vertices:
            c = o.matrix_world @ v.co
            lo = Vector((min(lo.x, c.x), min(lo.y, c.y), min(lo.z, c.z)))
            hi = Vector((max(hi.x, c.x), max(hi.y, c.y), max(hi.z, c.z)))
    return lo, hi


def normalise(width_m, rot_z_deg=0.0, rot_x_deg=0.0):
    """Rotate (to put triggers at +Y, up at +Z), scale to the real width, centre the housing."""
    R = Matrix.Rotation(math.radians(rot_z_deg), 4, "Z") @ Matrix.Rotation(math.radians(rot_x_deg), 4, "X")
    for o in meshes():
        o.data.transform(R)
    lo, hi = bounds()
    s = width_m / (hi.x - lo.x)
    centre = (lo + hi) / 2
    T = Matrix.Scale(s, 4) @ Matrix.Translation(-centre)
    for o in meshes():
        o.data.transform(T)
    return s


def auto_orient(width_m, stick_names, trigger_names):
    """Rotate/scale the whole model so that width -> +X, trigger side -> +Y and up -> +Z, centred.
    Up is the thinnest axis (towards the sticks), width the longest (the housing is wider than deep).
    Returns a description of what was done."""
    lo, hi = bounds()
    ext = [hi.x - lo.x, hi.y - lo.y, hi.z - lo.z]
    order = sorted(range(3), key=lambda i: ext[i])
    ax_h, ax_d, ax_w = order[0], order[1], order[2]
    centre = (lo + hi) / 2

    def centroid(names):
        pts = [o.matrix_world @ v.co for o in meshes() if any(n in o.name for n in names) for v in o.data.vertices]
        return sum(pts, Vector()) / max(len(pts), 1)

    sc, tc = centroid(stick_names), centroid(trigger_names)
    up_sign = 1.0 if (sc - centre)[ax_h] >= 0 else -1.0
    front_sign = 1.0 if (tc - centre)[ax_d] >= 0 else -1.0
    ex, ey, ez = Vector((0, 0, 0)), Vector((0, 0, 0)), Vector((0, 0, 0))
    ez[ax_h] = up_sign
    ey[ax_d] = front_sign
    ex = ey.cross(ez)  # right-handed: X = Y x Z
    # rows map old axes to new ones
    R = Matrix(((ex.x, ex.y, ex.z, 0), (ey.x, ey.y, ey.z, 0), (ez.x, ez.y, ez.z, 0), (0, 0, 0, 1)))
    for o in meshes():
        o.data.transform(R)
    lo, hi = bounds()
    s = width_m / (hi.x - lo.x)
    T = Matrix.Scale(s, 4) @ Matrix.Translation(-(lo + hi) / 2)
    for o in meshes():
        o.data.transform(T)
    return f"extent(raw)={[round(e,3) for e in ext]} axes w/d/h={ax_w}/{ax_d}/{ax_h} up_sign={up_sign} front_sign={front_sign} scale={s:.4f}"


def pca_orient(width_m, flip="", basis=None):
    """Align a tilted model by principal axes: widest -> +X, deepest -> +Y, thinnest -> +Z, then centre and scale.
    `flip` may contain 'x', 'y', 'z' to rotate 180 degrees about that axis afterwards (sign choice is made by eye)."""
    import numpy as np

    src = [o for o in meshes() if basis is None or any(b in o.name for b in basis)]
    pts = np.array([tuple(o.matrix_world @ v.co) for o in src for v in o.data.vertices])
    mean = pts.mean(axis=0)
    w, vecs = np.linalg.eigh(np.cov((pts - mean).T))  # ascending variance
    ez = vecs[:, 0]
    ey = vecs[:, 1]
    ex = vecs[:, 2]
    ez = np.cross(ex, ey)
    R = Matrix(((ex[0], ex[1], ex[2], 0), (ey[0], ey[1], ey[2], 0), (ez[0], ez[1], ez[2], 0), (0, 0, 0, 1)))
    R = R @ Matrix.Translation(-Vector(mean))
    for o in meshes():
        o.data.transform(R)
    for ax in flip:
        F = Matrix.Rotation(math.pi, 4, ax.upper())
        for o in meshes():
            o.data.transform(F)
    lo, hi = bounds()
    s = width_m / (hi.x - lo.x)
    T = Matrix.Scale(s, 4) @ Matrix.Translation(-(lo + hi) / 2)
    for o in meshes():
        o.data.transform(T)
    return f"pca variances={[round(float(x),5) for x in w]} scale={s:.4f}"


def lowest_z(x_m, y_m):
    """Lowest surface height of all meshes at (x, y): ray cast upwards from far below."""
    best = None
    for o in meshes():
        ok, loc, _n, _i = o.ray_cast(Vector((x_m, y_m, -1.0)), Vector((0, 0, 1)))
        if ok and (best is None or loc.z < best):
            best = loc.z
    return best


def add_paddle(name, x_mm, y_mm, length_mm, width_mm, thick_mm=2.2, tilt_deg=0.0, yaw_deg=0.0, drop_mm=0.4, metal_rgb=(0.16, 0.165, 0.175)):
    """Original rear paddle (our own modelling): a bevelled steel strip lying just below the rear surface of the housing."""
    MMs = 0.001
    z = lowest_z(x_mm * MMs, y_mm * MMs)
    z = (z if z is not None else -0.025) - drop_mm * MMs
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    bmesh.ops.scale(bm, vec=Vector((width_mm * MMs, length_mm * MMs, thick_mm * MMs)), verts=bm.verts)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me =bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    bpy.context.collection.objects.link(ob)
    ob.location = Vector((x_mm * MMs, y_mm * MMs, z - thick_mm * MMs / 2))
    ob.rotation_euler = (math.radians(tilt_deg), 0.0, math.radians(yaw_deg))
    mod = ob.modifiers.new("bevel", "BEVEL")
    mod.width = 0.8 * MMs
    mod.segments = 3
    bpy.context.view_layer.update()
    dg = bpy.context.evaluated_depsgraph_get()
    new = bpy.data.meshes.new_from_object(ob.evaluated_get(dg))
    old = ob.data
    ob.modifiers.clear()
    ob.data = new
    bpy.data.meshes.remove(old)
    for p in new.polygons:
        p.use_smooth = True
    m = bpy.data.materials.get("paddle_steel")
    if m is None:
        m = bpy.data.materials.new("paddle_steel")
        m.use_nodes = True
        b = m.node_tree.nodes["Principled BSDF"]
        b.inputs["Base Color"].default_value = (*metal_rgb, 1.0)
        b.inputs["Metallic"].default_value = 0.35  # satin gunmetal: fully metallic strips read as glass against a dark studio HDRI
        b.inputs["Roughness"].default_value = 0.5
    new.materials.append(m)
    # bake the object transform so origin = world origin like all other parts
    mw = ob.matrix_world.copy()
    ob.data.transform(mw)
    ob.matrix_world = Matrix.Identity(4)
    ob.rotation_euler = (0, 0, 0)
    ob.location = Vector((0, 0, 0))
    return ob


def recalc_normals():
    """Make face orientation consistent on every connected part (outward for closed parts)."""
    for o in meshes():
        bm = bmesh.new()
        bm.from_mesh(o.data)
        bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
        bm.to_mesh(o.data)
        bm.free()


def set_metallic(value=0.0, keep=()):
    """Force the metallic input of all materials (except names in `keep`) to `value` and drop metallic links."""
    for m in bpy.data.materials:
        if not m.use_nodes or any(k in m.name for k in keep):
            continue
        for n in m.node_tree.nodes:
            if n.type == "BSDF_PRINCIPLED":
                for l in list(n.inputs["Metallic"].links):
                    m.node_tree.links.remove(l)
                n.inputs["Metallic"].default_value = value


def subdivide(ob, levels=1):
    """Smooth a low-poly object with Catmull-Clark subdivision (UVs are kept)."""
    m = ob.modifiers.new("subsurf", "SUBSURF")
    m.levels = levels
    m.render_levels = levels
    dg = bpy.context.evaluated_depsgraph_get()
    new = bpy.data.meshes.new_from_object(ob.evaluated_get(dg))
    old = ob.data
    mats = list(old.materials)
    ob.modifiers.clear()
    ob.data = new
    for mm in mats:
        if mm not in list(new.materials):
            new.materials.append(mm)
    bpy.data.meshes.remove(old)
    for p in new.polygons:
        p.use_smooth = True


def solid_material(ob, name, rgb, rough, metal=0.0):
    """Replace all materials of an object by one plain material (removes logos / unwanted textures)."""
    m = bpy.data.materials.get(name)
    if m is None:
        m = bpy.data.materials.new(name)
        m.use_nodes = True
        b = m.node_tree.nodes["Principled BSDF"]
        b.inputs["Base Color"].default_value = (*rgb, 1.0)
        b.inputs["Roughness"].default_value = rough
        b.inputs["Metallic"].default_value = metal
    ob.data.materials.clear()
    ob.data.materials.append(m)
    for p in ob.data.polygons:
        p.material_index = 0


def islands(obj):
    """Loose parts of a mesh: list of dicts with face indices, centroid and bbox size (metres)."""
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    bm.faces.ensure_lookup_table()
    seen = set()
    out = []
    for f in bm.faces:
        if f.index in seen:
            continue
        stack = [f]
        seen.add(f.index)
        faces = []
        while stack:
            cur = stack.pop()
            faces.append(cur.index)
            for e in cur.edges:
                for nf in e.link_faces:
                    if nf.index not in seen:
                        seen.add(nf.index)
                        stack.append(nf)
        vs = {v.index for i in faces for v in bm.faces[i].verts}
        pts = [obj.data.vertices[i].co for i in vs]
        lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
        hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
        out.append({"faces": faces, "tris": sum(len(bm.faces[i].verts) - 2 for i in faces), "lo": lo, "hi": hi, "c": (lo + hi) / 2, "size": hi - lo})
    bm.free()
    return out


def split_island(obj, isl, name):
    """Create a new object from one island of `obj` (and remove those faces from `obj`)."""
    me = obj.data
    bm = bmesh.new()
    bm.from_mesh(me)
    bm.faces.ensure_lookup_table()
    sel = set(isl["faces"])
    new_bm = bmesh.new()
    vmap = {}
    uv_src = bm.loops.layers.uv.active
    uv_dst = new_bm.loops.layers.uv.new("UVMap") if uv_src else None
    for i in sel:
        f = bm.faces[i]
        nv = []
        for v in f.verts:
            if v.index not in vmap:
                vmap[v.index] = new_bm.verts.new(v.co)
            nv.append(vmap[v.index])
        nf = new_bm.faces.new(nv)
        nf.material_index = f.material_index
        nf.smooth = f.smooth
        if uv_src:
            for lo, ln in zip(f.loops, nf.loops):
                ln[uv_dst].uv = lo[uv_src].uv
    bmesh.ops.delete(bm, geom=[bm.faces[i] for i in sel], context="FACES")
    bm.to_mesh(me)
    bm.free()
    new_me = bpy.data.meshes.new(name)
    new_bm.to_mesh(new_me)
    new_bm.free()
    for m in me.materials:
        new_me.materials.append(m)
    ob = bpy.data.objects.new(name, new_me)
    bpy.context.collection.objects.link(ob)
    return ob


def set_origin(ob, point):
    """Move the object's origin to `point` (world == local at this stage) keeping the geometry in place."""
    d = Vector(point)
    ob.data.transform(Matrix.Translation(-d))
    ob.location = d


def fix_winding():
    """Make every closed loose part face outwards: parts with negative signed volume are flipped.
    (Mirrored node transforms in the source files can leave whole shells inside-out.)"""
    flipped = 0
    for o in meshes():
        bm = bmesh.new()
        bm.from_mesh(o.data)
        bm.faces.ensure_lookup_table()
        seen = set()
        for f in bm.faces:
            if f.index in seen:
                continue
            stack, comp = [f], []
            seen.add(f.index)
            while stack:
                cur = stack.pop()
                comp.append(cur)
                for e in cur.edges:
                    for nf in e.link_faces:
                        if nf.index not in seen:
                            seen.add(nf.index)
                            stack.append(nf)
            vol = 0.0
            for cf in comp:
                vs = [v.co for v in cf.verts]
                for k in range(1, len(vs) - 1):
                    vol += vs[0].dot(vs[k].cross(vs[k + 1])) / 6.0
            if vol < 0:
                for cf in comp:
                    cf.normal_flip()
                flipped += len(comp)
        bm.normal_update()
        bm.to_mesh(o.data)
        bm.free()
    return flipped


def tri_count():
    return sum(sum(len(p.vertices) - 2 for p in o.data.polygons) for o in meshes())


def decimate(ob, ratio):
    m = ob.modifiers.new("dec", "DECIMATE")
    m.ratio = ratio
    m.use_collapse_triangulate = True
    dg = bpy.context.evaluated_depsgraph_get()
    new = bpy.data.meshes.new_from_object(ob.evaluated_get(dg))
    old = ob.data
    mats = list(old.materials)
    ob.modifiers.clear()
    ob.data = new
    for m in mats:
        if m not in list(new.materials):
            new.materials.append(m)
    bpy.data.meshes.remove(old)


def fix_emission(keep=(), strength=1.0):
    """Imported FBX-style materials carry an emission slot with strength > 0 that makes everything glow in glTF.
    Zero it for every material except those whose name contains one of `keep`."""
    for m in bpy.data.materials:
        if not m.use_nodes:
            continue
        for n in m.node_tree.nodes:
            if n.type != 'BSDF_PRINCIPLED':
                continue
            if any(k in m.name for k in keep):
                n.inputs['Emission Strength'].default_value = strength
            else:
                n.inputs['Emission Strength'].default_value = 0.0
                for l in list(n.inputs['Emission Color'].links):
                    m.node_tree.links.remove(l)
                n.inputs['Emission Color'].default_value = (0, 0, 0, 1)


def make_opaque(names_contain):
    """Force materials whose name contains one of `names_contain` to be fully opaque (alpha = 1, no alpha links)."""
    for m in bpy.data.materials:
        if not m.use_nodes or not any(k in m.name for k in names_contain):
            continue
        for n in m.node_tree.nodes:
            if n.type == "BSDF_PRINCIPLED":
                for l in list(n.inputs["Alpha"].links):
                    m.node_tree.links.remove(l)
                n.inputs["Alpha"].default_value = 1.0
                bc = n.inputs["Base Color"]
                if not bc.links:
                    bc.default_value = (1.0, 1.0, 1.0, 1.0)


def shrink_images(max_px=2048):
    for im in bpy.data.images:
        if im.size[0] > max_px or im.size[1] > max_px:
            k = max_px / max(im.size)
            im.scale(max(1, int(im.size[0] * k)), max(1, int(im.size[1] * k)))


def export_glb(path, jpeg=True):
    bpy.ops.object.select_all(action="DESELECT")
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLB",
        export_yup=True,
        export_apply=True,
        export_materials="EXPORT",
        export_normals=True,
        export_texcoords=True,
        export_cameras=False,
        export_lights=False,
        export_image_format="JPEG" if jpeg else "AUTO",
        export_jpeg_quality=88,
    )


def preview(path, angle="persp", res=(1000, 700)):
    sc = bpy.context.scene
    sc.render.engine = "CYCLES"
    sc.cycles.device = "CPU"
    sc.cycles.samples = 16
    sc.cycles.use_denoising = False
    sc.render.resolution_x, sc.render.resolution_y = res
    sc.render.filepath = path
    w = bpy.data.worlds.new("w")
    w.use_nodes = True
    w.node_tree.nodes["Background"].inputs[0].default_value = (0.5, 0.52, 0.55, 1)
    w.node_tree.nodes["Background"].inputs[1].default_value = 0.8
    sc.world = w
    lo, hi = bounds()
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
    offs = {"top": Vector((0, -0.05, 1.0)), "persp": Vector((0, -0.8, 0.8)), "front": Vector((0, -1.0, 0.25)), "back": Vector((0, 1.0, 0.5))}[angle]
    co.location = centre + offs.normalized() * diag * 2.6
    co.rotation_euler = (centre - co.location).to_track_quat("-Z", "Y").to_euler()
    bpy.ops.render.render(write_still=True)


def smooth_region(box_mm, nz_max=-0.5, iters=12, factor=0.6):
    """Smooth vertices of downward-facing surface inside box (xmin,xmax,ymin,ymax) in mm: erases shallow engraved lettering."""
    x0, x1, y0, y1 = [v * 0.001 for v in box_mm]
    for o in meshes():
        bm = bmesh.new()
        bm.from_mesh(o.data)
        bm.normal_update()
        vs = [v for v in bm.verts if x0 <= v.co.x <= x1 and y0 <= v.co.y <= y1 and v.normal.z <= nz_max]
        if vs:
            for _ in range(iters):
                bmesh.ops.smooth_vert(bm, verts=vs, factor=factor, use_axis_x=True, use_axis_y=True, use_axis_z=True)
            bm.to_mesh(o.data)
        bm.free()
        o.data.update()


def whiten_image(name, rects):
    """Paint rectangles (x0,y0,x1,y1 as top-down fractions) of a texture near-white: removes printed lettering."""
    import numpy as np
    im = bpy.data.images.get(name)
    w, h = im.size
    px = np.array(im.pixels[:], dtype=np.float32).reshape(h, w, 4)
    for x0, y0, x1, y1 in rects:
        r0, r1 = int((1 - y1) * h), int((1 - y0) * h)
        c0, c1 = int(x0 * w), int(x1 * w)
        px[r0:r1, c0:c1, :3] = np.median(px[r0:r1, c0:c1, :3].reshape(-1, 3), axis=0)  # dominant background of the rect
    im.pixels = px.ravel().tolist()
    im.update()




def add_symbol(name, kind, cx_mm, cy_mm, z_mm, size_mm, rgb, rough=0.3, w_mm=0.75, t_mm=0.3):
    """Own-built PlayStation face symbol (triangle / circle / cross / square) as thin bars lying on a button cap."""
    MMs = 0.001
    bm = bmesh.new()

    def bar(p0, p1):
        (x0, y0), (x1, y1) = p0, p1
        length = math.hypot(x1 - x0, y1 - y0) + w_mm * 0.9
        ang = math.atan2(y1 - y0, x1 - x0)
        vs = bmesh.ops.create_cube(bm, size=1.0)["verts"]
        bmesh.ops.scale(bm, vec=Vector((length, w_mm, t_mm)), verts=vs)
        bmesh.ops.rotate(bm, cent=(0, 0, 0), matrix=Matrix.Rotation(ang, 3, "Z"), verts=vs)
        bmesh.ops.translate(bm, vec=Vector(((x0 + x1) / 2, (y0 + y1) / 2, 0)), verts=vs)

    s = size_mm
    if kind == "cross":
        bar((-s / 2, -s / 2), (s / 2, s / 2))
        bar((-s / 2, s / 2), (s / 2, -s / 2))
    elif kind == "square":
        h = s * 0.44
        pts = [(-h, -h), (h, -h), (h, h), (-h, h)]
        for i in range(4):
            bar(pts[i], pts[(i + 1) % 4])
    elif kind == "triangle":
        r = s * 0.62
        pts = [(r * math.cos(math.radians(a)), r * math.sin(math.radians(a)) - r * 0.12) for a in (90, 210, 330)]
        for i in range(3):
            bar(pts[i], pts[(i + 1) % 3])
    else:  # circle
        r, n = s * 0.5, 20
        pts = [(r * math.cos(2 * math.pi * i / n), r * math.sin(2 * math.pi * i / n)) for i in range(n)]
        for i in range(n):
            bar(pts[i], pts[(i + 1) % n])
    bmesh.ops.translate(bm, vec=Vector((cx_mm, cy_mm, z_mm + t_mm / 2 - 0.05)), verts=bm.verts)
    bmesh.ops.scale(bm, vec=Vector((MMs, MMs, MMs)), verts=bm.verts)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    bpy.context.collection.objects.link(ob)
    solid_material(ob, "sym_" + name, rgb, rough)
    return ob


def world_bbox(ob):
    pts = [ob.matrix_world @ v.co for v in ob.data.vertices]
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    return lo, hi
