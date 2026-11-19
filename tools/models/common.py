"""Shared Blender (>= 4.x) helpers for the DriftLab controller models.

Run a model script headless, e.g.:
    blender -b -P tools/models/dualsense.py -- --out assets/models/dualsense.glb [--preview out.png]

Everything here is original modelling work from public dimensions and reference knowledge of the
product category. No ripped or downloaded 3D assets, textures or logos are used.

Coordinate convention (Blender): +X right, +Y towards the triggers (away from the player),
+Z up. The glTF exporter converts to Y-up, so triggers end up at -Z in the app (player = +Z).
Units: metres (helpers take millimetres via MM).
"""
import math
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

MM = 0.001


# --------------------------------------------------------------------------- scene / materials
def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.context.scene.unit_settings.system = "METRIC"


def args():
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    out = {"out": None, "preview": None, "angle": "top"}
    i = 0
    while i < len(argv):
        if argv[i] == "--out":
            out["out"] = argv[i + 1]
            i += 2
        elif argv[i] == "--preview":
            out["preview"] = argv[i + 1]
            i += 2
        elif argv[i] == "--angle":
            out["angle"] = argv[i + 1]
            i += 2
        else:
            i += 1
    return out


_MATS = {}


def material(name, color, rough=0.5, metal=0.0, emission=None, strength=0.0, alpha=1.0):
    """Principled material. Names starting with 'glow' / 'accent' are recoloured by the app."""
    if name in _MATS:
        return _MATS[name]
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    b = m.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = (*color, 1.0)
    b.inputs["Roughness"].default_value = rough
    b.inputs["Metallic"].default_value = metal
    if emission is not None:
        b.inputs["Emission Color"].default_value = (*emission, 1.0)
        b.inputs["Emission Strength"].default_value = strength
    if alpha < 1.0:
        b.inputs["Alpha"].default_value = alpha
    _MATS[name] = m
    return m


def hexc(h):
    """sRGB hex -> linear RGB tuple (Blender stores linear values)."""
    def lin(c):
        c /= 255.0
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

    return (lin((h >> 16) & 255), lin((h >> 8) & 255), lin(h & 255))


# --------------------------------------------------------------------------- mesh plumbing
def finish(name, bm, loc=(0, 0, 0), mats=(), smooth=True, parent=None):
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    for m in mats:
        me.materials.append(m)
    ob = bpy.data.objects.new(name, me)
    bpy.context.collection.objects.link(ob)
    ob.location = Vector(loc)
    if smooth:
        for p in me.polygons:
            p.use_smooth = True
    if parent is not None:
        adopt(ob, parent)
    return ob


def adopt(child, parent):
    """Parent without moving the child (refreshes the depsgraph so matrix_world is current)."""
    bpy.context.view_layer.update()
    child.parent = parent
    child.matrix_parent_inverse = parent.matrix_world.inverted()


def apply_mods(ob):
    """Bake all modifiers of an object into its mesh (via the evaluated depsgraph)."""
    dg = bpy.context.evaluated_depsgraph_get()
    ev = ob.evaluated_get(dg)
    new = bpy.data.meshes.new_from_object(ev)
    mats = list(ob.data.materials)
    old = ob.data
    ob.modifiers.clear()
    ob.data = new
    for m in mats:
        if m not in list(new.materials):
            new.materials.append(m)
    bpy.data.meshes.remove(old)
    for p in new.polygons:
        p.use_smooth = True
    return ob


def bevel(ob, width, segments=3, angle=40):
    m = ob.modifiers.new("bevel", "BEVEL")
    m.width = width
    m.segments = segments
    m.limit_method = "ANGLE"
    m.angle_limit = math.radians(angle)
    return m


def subsurf(ob, levels=2):
    m = ob.modifiers.new("subsurf", "SUBSURF")
    m.levels = levels
    m.render_levels = levels
    return m


def set_origin_world(ob, world_pt):
    """Move the object's origin to `world_pt` without moving the geometry."""
    d = Vector(world_pt) - ob.location
    ob.data.transform(Matrix.Translation(-d))
    ob.location = Vector(world_pt)


# --------------------------------------------------------------------------- 2D outline helpers
def catmull_closed(pts, per_seg=10):
    """Closed Catmull-Rom spline through 2D points (list of (x, y))."""
    n = len(pts)
    out = []
    for i in range(n):
        p0, p1, p2, p3 = pts[(i - 1) % n], pts[i], pts[(i + 1) % n], pts[(i + 2) % n]
        for s in range(per_seg):
            t = s / per_seg
            t2, t3 = t * t, t * t * t
            out.append(
                tuple(
                    0.5 * ((2 * p1[k]) + (-p0[k] + p2[k]) * t + (2 * p0[k] - 5 * p1[k] + 4 * p2[k] - p3[k]) * t2 + (-p0[k] + 3 * p1[k] - 3 * p2[k] + p3[k]) * t3)
                    for k in range(2)
                )
            )
    return out


def mirror_outline(half):
    """Right-half control points (x >= 0, ordered from top-centre clockwise to bottom-centre) -> full closed loop."""
    left = [(-x, y) for (x, y) in reversed(half[1:-1])]
    return list(half) + left


def point_in_poly(x, y, poly):
    inside = False
    j = len(poly) - 1
    for i in range(len(poly)):
        xi, yi = poly[i]
        xj, yj = poly[j]
        if (yi > y) != (yj > y) and x < (xj - xi) * (y - yi) / (yj - yi + 1e-12) + xi:
            inside = not inside
        j = i
    return inside


# --------------------------------------------------------------------------- primitives
def extrude_poly(name, pts_mm, depth_mm, mats, loc_mm=(0, 0, 0), z0_mm=0.0, bev_mm=0.35, hole_inset_mm=None):
    """Prism from a 2D polygon in the XY plane. `hole_inset_mm` makes an outline ring instead of a solid."""
    bm = bmesh.new()
    vs = [bm.verts.new((x * MM, y * MM, z0_mm * MM)) for x, y in pts_mm]
    f = bm.faces.new(vs)
    if hole_inset_mm:
        r = bmesh.ops.inset_individual(bm, faces=[f], thickness=hole_inset_mm * MM)
        bmesh.ops.delete(bm, geom=[f], context="FACES")
    geom = list(bm.faces)
    ext = bmesh.ops.extrude_face_region(bm, geom=geom)
    moved = [v for v in ext["geom"] if isinstance(v, bmesh.types.BMVert)]
    bmesh.ops.translate(bm, verts=moved, vec=(0, 0, depth_mm * MM))
    bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
    ob = finish(name, bm, [c * MM for c in loc_mm], mats)
    if bev_mm:
        bevel(ob, bev_mm * MM, 2)
        apply_mods(ob)
    return ob


def lathe(name, profile_mm, mats, loc_mm=(0, 0, 0), steps=48, knurl=None):
    """Surface of revolution about +Z. profile_mm: [(radius, z), ...] from axis outwards, closed back to the axis.
    knurl=(r_min_mm, count, amp_mm) adds ridges to vertices beyond r_min."""
    bm = bmesh.new()
    vs = [bm.verts.new((r * MM, 0, z * MM)) for r, z in profile_mm]
    for a, b in zip(vs, vs[1:]):
        bm.edges.new((a, b))
    bmesh.ops.spin(bm, geom=list(bm.verts) + list(bm.edges), cent=(0, 0, 0), axis=(0, 0, 1), angle=math.tau, steps=steps, use_merge=True)
    bmesh.ops.remove_doubles(bm, verts=list(bm.verts), dist=1e-6)
    bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
    if knurl:
        r_min, count, amp = knurl
        for v in bm.verts:
            r = math.hypot(v.co.x, v.co.y)
            if r > r_min * MM:
                th = math.atan2(v.co.y, v.co.x)
                k = 1.0 + (amp * MM / r) * math.cos(count * th)
                v.co.x *= k
                v.co.y *= k
    return finish(name, bm, [c * MM for c in loc_mm], mats)


def ellipsoid(name, center_mm, radii_mm, mats, rot_deg=(0, 0, 0)):
    bm = bmesh.new()
    bmesh.ops.create_uvsphere(bm, u_segments=48, v_segments=24, radius=1.0)
    bmesh.ops.scale(bm, vec=Vector(radii_mm) * MM, verts=bm.verts)
    ob = finish(name, bm, [c * MM for c in center_mm], mats)
    ob.rotation_euler = [math.radians(a) for a in rot_deg]
    return ob


def text_mesh(name, text, size_mm, mats, loc_mm, depth_mm=0.5):
    cu = bpy.data.curves.new(name, "FONT")
    cu.body = text
    cu.size = size_mm * MM
    cu.extrude = depth_mm * MM / 2
    cu.align_x = "CENTER"
    cu.align_y = "CENTER"
    ob = bpy.data.objects.new(name, cu)
    bpy.context.collection.objects.link(ob)
    ob.location = Vector([c * MM for c in loc_mm])
    bpy.context.view_layer.objects.active = ob
    ob.select_set(True)
    bpy.ops.object.convert(target="MESH")
    ob = bpy.context.view_layer.objects.active
    ob.data.materials.append(mats[0])
    for p in ob.data.polygons:
        p.use_smooth = True
    ob.select_set(False)
    return ob


# --------------------------------------------------------------------------- body
def build_body(name, outline_ctrl_mm, thickness_mm, grip_blobs, mats, voxel_mm=1.0, round_mm=9.0, smooth_iter=8):
    """Organic one-piece housing: extruded outline + grip ellipsoids, voxel-remeshed and smoothed.
    grip_blobs: list of (center_mm, radii_mm, rot_deg)."""
    pts = catmull_closed(outline_ctrl_mm, 10)
    slab = extrude_poly(name + "_slab", pts, thickness_mm, [], z0_mm=-thickness_mm / 2, bev_mm=round_mm)
    parts = [slab]
    for i, (c, r, rot) in enumerate(grip_blobs):
        parts.append(ellipsoid(f"{name}_blob{i}", c, r, [], rot))
    for ob in bpy.context.scene.objects:
        ob.select_set(ob in parts)
    bpy.context.view_layer.objects.active = slab
    bpy.ops.object.join()
    body = bpy.context.view_layer.objects.active
    body.name = name
    rm = body.modifiers.new("remesh", "REMESH")
    rm.mode = "VOXEL"
    rm.voxel_size = voxel_mm * MM
    sm = body.modifiers.new("smooth", "SMOOTH")
    sm.factor = 0.6
    sm.iterations = smooth_iter * 3
    apply_mods(body)
    for m in mats:
        body.data.materials.append(m)
    return body


def bisect(body, planes_mm):
    """Cut the mesh along planes [(point, normal), ...] (mm) so material borders follow clean straight lines."""
    bm = bmesh.new()
    bm.from_mesh(body.data)
    for co, no in planes_mm:
        geom = list(bm.verts) + list(bm.edges) + list(bm.faces)
        bmesh.ops.bisect_plane(bm, geom=geom, plane_co=Vector(co) * MM, plane_no=Vector(no), clear_inner=False, clear_outer=False)
    bm.to_mesh(body.data)
    bm.free()
    for p in body.data.polygons:
        p.use_smooth = True


def assign_regions(body, region_fn):
    """Per-face material index from region_fn(x_mm, y_mm, z_mm, nz) -> int."""
    me = body.data
    me.calc_loop_triangles()
    for p in me.polygons:
        c = p.center
        p.material_index = region_fn(c.x / MM, c.y / MM, c.z / MM, p.normal.z)


def surface_z(body, x_mm, y_mm, z_from_mm=80.0):
    """Height (mm) of the body's top surface at (x, y) via ray cast."""
    ok, loc, nrm, _ = body.ray_cast(Vector((x_mm * MM, y_mm * MM, z_from_mm * MM)), Vector((0, 0, -1)))
    return (loc.z / MM, nrm) if ok else (0.0, Vector((0, 0, 1)))


# --------------------------------------------------------------------------- export / preview
def export_glb(path):
    bpy.ops.object.select_all(action="DESELECT")
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLB",
        export_yup=True,
        export_apply=True,
        export_materials="EXPORT",
        export_normals=True,
        export_texcoords=False,
        export_cameras=False,
        export_lights=False,
        export_image_format="NONE",
    )


def preview(path, angle="top", res=(1100, 760)):
    """Cycles CPU test render (not shipped) to inspect the geometry."""
    sc = bpy.context.scene
    sc.render.engine = "CYCLES"
    sc.cycles.device = "CPU"
    sc.cycles.samples = 24
    sc.cycles.use_denoising = False
    sc.render.resolution_x, sc.render.resolution_y = res
    sc.render.filepath = path
    w = bpy.data.worlds.new("w")
    w.use_nodes = True
    w.node_tree.nodes["Background"].inputs[0].default_value = (0.55, 0.57, 0.6, 1)
    w.node_tree.nodes["Background"].inputs[1].default_value = 0.45
    sc.world = w
    sun = bpy.data.lights.new("sun", "AREA")
    sun.energy = 18
    sun.size = 0.3
    so = bpy.data.objects.new("sun", sun)
    bpy.context.collection.objects.link(so)
    so.location = (-0.15, -0.1, 0.35)
    so.rotation_euler = (0.5, -0.3, 0)
    cam = bpy.data.cameras.new("cam")
    co = bpy.data.objects.new("cam", cam)
    bpy.context.collection.objects.link(co)
    sc.camera = co
    cam.lens = 85
    views = {
        "top": ((0, -0.05, 0.62), (0, 0, 0)),
        "front": ((0.0, -0.45, 0.28), (0, 0, 0)),
        "persp": ((0.0, -0.32, 0.34), (0, 0, 0)),
        "back": ((0.0, 0.42, 0.24), (0, 0, 0)),
        "side": ((0.45, 0.0, 0.06), (0, 0, 0)),
    }
    loc, tgt = views.get(angle, views["persp"])
    co.location = loc
    d = Vector(tgt) - Vector(loc)
    co.rotation_euler = d.to_track_quat("-Z", "Y").to_euler()
    bpy.ops.render.render(write_still=True)
