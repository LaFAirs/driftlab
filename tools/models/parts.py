"""Reusable controller components (sticks, d-pad, buttons, triggers, ribbons). Dimensions in millimetres."""
import math

import bmesh
from mathutils import Vector

from common import *  # noqa: F401,F403
from common import MM, adopt, apply_mods, bevel, extrude_poly, finish, hexc, lathe, material, set_origin_world, subsurf, surface_z, text_mesh

PS_COL = {"triangle": 0x2FBF9B, "circle": 0xE0413A, "cross": 0x4F7BD9, "square": 0xD97AB6}
XB_COL = {"a": 0x62B246, "b": 0xE5352B, "x": 0x2F86D6, "y": 0xF2B51D}


def stick(body, side, x, y, mats, radius=9.6, rim_knurl=True):
    """Stick cap (animated `stick_l/r`, origin = ball-joint pivot) plus a static bezel ring."""
    sz, _ = surface_z(body, x, y)
    sfx = "l" if side < 0 else "r"
    # Static bezel / well around the stick.
    ring = [(radius + 1.0, -1.5), (radius + 3.8, -1.5), (radius + 3.8, 0.6), (radius + 3.2, 1.3), (radius + 1.5, 1.4), (radius + 1.0, 0.4)]
    lathe(f"stick_bezel_{sfx}", ring + [ring[0]], [mats["bezel"]], (x, y, sz), steps=56)
    # Cap: shaft + dished disc with ridged rim. Origin 3 mm below the surface (ball joint).
    r = radius
    prof = [
        (0, 0), (4.4, 0), (4.4, 8.6), (r - 0.9, 9.2), (r, 10.2), (r + 0.1, 11.7), (r - 0.5, 13.1),
        (r - 1.4, 13.3), (r - 3.0, 12.6), (r * 0.5, 11.9), (0, 11.55),
    ]
    cap = lathe(f"stick_{sfx}", prof, [mats["rubber"]], (x, y, sz - 3.0), steps=192, knurl=(r - 1.6, 48, 0.2) if rim_knurl else None)
    return cap


def dpad(body, cx, cy, mats, arm_w=10.4, arm_len=15.5, style="ps"):
    """Four separate arms named dpad_up/down/left/right (origin at the hub centre)."""
    sz, _ = surface_z(body, cx, cy)
    hub = [(0, 0), (arm_len * 0.62, 0)]
    lathe("dpad_hub", [(0, -1), (arm_len * 0.7, -1), (arm_len * 0.7, 1.0), (0, 1.0)], [mats["bezel"]], (cx, cy, sz), steps=40)
    w = arm_w / 2
    names = {"up": 0, "right": -90, "down": 180, "left": 90}
    for n, ang in names.items():
        pts = [(-w, -w * 0.6), (w, -w * 0.6), (w, arm_len - 2.8), (w - 1.3, arm_len - 0.6), (-w + 1.3, arm_len - 0.6), (-w, arm_len - 2.8)]
        a = math.radians(ang)
        rot = [(px * math.cos(a) - py * math.sin(a), px * math.sin(a) + py * math.cos(a)) for px, py in pts]
        ob = extrude_poly(f"dpad_{n}", rot, 3.4, [mats["dpad"]], loc_mm=(cx, cy, sz), z0_mm=-0.4, bev_mm=0.9)
        # small chevron on the tip
        tx, ty = -math.sin(a) * (arm_len - 4.5), math.cos(a) * (arm_len - 4.5)
        tri = [(-1.8, -1.0), (1.8, -1.0), (0, 1.6)]
        tri_r = [(px * math.cos(a) - py * math.sin(a) + tx, px * math.sin(a) + py * math.cos(a) + ty) for px, py in tri]
        ch = extrude_poly(f"dpad_{n}_mark", tri_r, 0.3, [mats["dpad_mark"]], loc_mm=(cx, cy, sz + 3.0), bev_mm=0.0)
        adopt(ch, ob)


def _symbol(kind, size):
    """Outline ring polygon points (mm) for a PlayStation-style symbol, centred on the origin."""
    s = size
    if kind == "triangle":
        h = s * 0.95
        return [(-s * 0.55, -h * 0.4), (s * 0.55, -h * 0.4), (0, h * 0.62)]
    if kind == "square":
        return [(-s * 0.5, -s * 0.5), (s * 0.5, -s * 0.5), (s * 0.5, s * 0.5), (-s * 0.5, s * 0.5)]
    if kind == "circle":
        return [(s * 0.55 * math.cos(t * math.tau / 32), s * 0.55 * math.sin(t * math.tau / 32)) for t in range(32)]
    return None


def face_buttons_ps(body, cx, cy, mats, spacing=12.2, r=6.2, edge_style="light"):
    layout = {"triangle": (0, 1), "circle": (1, 0), "cross": (0, -1), "square": (-1, 0)}
    for kind, (dx, dy) in layout.items():
        x, y = cx + dx * spacing, cy + dy * spacing
        sz, _ = surface_z(body, x, y)
        prof = [(0, 0), (r - 0.6, 0), (r, 0.7), (r, 2.4), (r - 0.7, 3.2), (r - 2.5, 3.5), (0, 3.2)]
        cap = lathe(f"face_{kind}", prof, [mats["face_cap"]], (x, y, sz - 0.3), steps=48)
        col = material(f"sym_{kind}", hexc(PS_COL[kind]), 0.35)
        z = 3.25
        if kind == "cross":
            L, W = 3.4, 0.62
            plus = [(W, W), (L, W), (L, -W), (W, -W), (W, -L), (-W, -L), (-W, -W), (-L, -W), (-L, W), (-W, W), (-W, L), (W, L)]
            a = math.radians(45)
            pts = [(px * math.cos(a) - py * math.sin(a), px * math.sin(a) + py * math.cos(a)) for px, py in plus]
            pts = [(px * 1.55, py * 1.55) for px, py in pts]
            o = extrude_poly(f"face_{kind}_sym", pts, 0.5, [col], loc_mm=(x, y, sz - 0.3 + z), bev_mm=0.0)
            adopt(o, cap)
        else:
            pts = _symbol(kind, 6.6)
            o = extrude_poly(f"face_{kind}_sym", pts, 0.5, [col], loc_mm=(x, y, sz - 0.3 + z), hole_inset_mm=1.15, bev_mm=0.0)
            adopt(o, cap)


def face_buttons_xbox(body, cx, cy, mats, spacing=11.4, r=6.7):
    layout = {"y": (0, 1), "b": (1, 0), "a": (0, -1), "x": (-1, 0)}
    for k, (dx, dy) in layout.items():
        x, y = cx + dx * spacing, cy + dy * spacing
        sz, _ = surface_z(body, x, y)
        prof = [(0, 0), (r - 0.6, 0), (r, 0.7), (r, 2.6), (r - 0.8, 3.4), (r - 2.5, 3.8), (0, 3.5)]
        cap = lathe(f"face_{k}", prof, [mats["face_cap"]], (x, y, sz - 0.3), steps=48)
        col = material(f"letter_{k}", hexc(XB_COL[k]), 0.4)
        t = text_mesh(f"face_{k}_letter", k.upper(), 6.4, [col], (x, y, sz - 0.3 + 3.5), 0.5)
        adopt(t, cap)


def disc_button(name, body, x, y, r, mats, mat_key, h=2.2, z_off=-0.3, dish=0.3):
    sz, _ = surface_z(body, x, y)
    prof = [(0, 0), (r - 0.5, 0), (r, 0.5), (r, h - 0.6), (r - 0.6, h), (r * 0.5, h + 0.1), (0, h - dish)]
    return lathe(name, prof, [mats[mat_key]], (x, y, sz + z_off), steps=40)


def oval_button(name, body, x, y, w, h, mats, mat_key, rot_deg=0.0, height=1.6):
    sz, _ = surface_z(body, x, y)
    pts = []
    for i in range(24):
        t = i * math.tau / 24
        px, py = (w / 2) * math.cos(t), (h / 2) * math.sin(t)
        a = math.radians(rot_deg)
        pts.append((px * math.cos(a) - py * math.sin(a), px * math.sin(a) + py * math.cos(a)))
    return extrude_poly(name, pts, height, [mats[mat_key]], loc_mm=(x, y, sz - 0.4), bev_mm=0.5)


def rounded_rect(w, h, r, n=8):
    pts = []
    for cx, cy, a0 in ((w / 2 - r, h / 2 - r, 0), (-w / 2 + r, h / 2 - r, 90), (-w / 2 + r, -h / 2 + r, 180), (w / 2 - r, -h / 2 + r, 270)):
        for i in range(n + 1):
            a = math.radians(a0 + 90 * i / n)
            pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    return pts


def touchpad(body, x, y, w, h, mats, mat_key="touchpad", r=4.0, height=0.8, name="touchpad"):
    sz, _ = surface_z(body, x, y)
    return extrude_poly(name, rounded_rect(w, h, r), height + 1.0, [mats[mat_key]], loc_mm=(x, y, sz - 1.0), bev_mm=0.35)


def ribbon(name, pts_mm, width_mm, mats, z_mm, height_mm=0.8, body=None):
    """Thin raised ribbon following a polyline (lightbar strips). Z follows the body surface if given."""
    bm = bmesh.new()
    left, right = [], []
    for i, (px, py) in enumerate(pts_mm):
        a = pts_mm[max(i - 1, 0)]
        b = pts_mm[min(i + 1, len(pts_mm) - 1)]
        tx, ty = b[0] - a[0], b[1] - a[1]
        L = math.hypot(tx, ty) or 1.0
        nx, ny = -ty / L, tx / L
        z = (surface_z(body, px, py)[0] if body else z_mm) + 0.1
        for sign, lst in ((1, left), (-1, right)):
            lx, ly = px + nx * width_mm / 2 * sign, py + ny * width_mm / 2 * sign
            zz = (surface_z(body, lx, ly)[0] if body else z_mm) + 0.05
            lst.append((lx, ly, zz))
    top_l = [bm.verts.new((x * MM, y * MM, (z + height_mm) * MM)) for x, y, z in left]
    top_r = [bm.verts.new((x * MM, y * MM, (z + height_mm) * MM)) for x, y, z in right]
    base_l = [bm.verts.new((x * MM, y * MM, (z - 0.4) * MM)) for x, y, z in left]
    base_r = [bm.verts.new((x * MM, y * MM, (z - 0.4) * MM)) for x, y, z in right]
    for i in range(len(pts_mm) - 1):
        bm.faces.new((top_l[i], top_l[i + 1], top_r[i + 1], top_r[i]))
        bm.faces.new((top_l[i], base_l[i], base_l[i + 1], top_l[i + 1]))
        bm.faces.new((top_r[i], top_r[i + 1], base_r[i + 1], base_r[i]))
    bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
    ob = finish(name, bm, (0, 0, 0), mats)
    return ob


def trigger(name, x, y, z, mats, mat_key, width=24.0, profile=None, hinge=(0, 0)):
    """Curved trigger paddle. Origin = hinge axis (along X). Profile in (y, z) mm relative to the hinge."""
    prof = profile or [(-5, 0), (6, 2.5), (14, -3), (19, -13), (18, -25), (11, -33), (1, -32), (-3, -20), (-5, -8)]
    bm = bmesh.new()
    vs = [bm.verts.new((-width / 2 * MM, py * MM, pz * MM)) for py, pz in prof]
    f = bm.faces.new(vs)
    ext = bmesh.ops.extrude_face_region(bm, geom=[f])
    moved = [v for v in ext["geom"] if isinstance(v, bmesh.types.BMVert)]
    bmesh.ops.translate(bm, verts=moved, vec=(width * MM, 0, 0))
    bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
    ob = finish(name, bm, (x * MM, y * MM, z * MM), [mats[mat_key]])
    bevel(ob, 2.6 * MM, 4, 50)
    subsurf(ob, 1)
    apply_mods(ob)
    return ob


def bumper(name, x, y, z, mats, mat_key, width=40.0, depth=11.0, height=7.0, tilt_deg=0.0):
    pts = rounded_rect(width, depth, 4.0)
    ob = extrude_poly(name, pts, height, [mats[mat_key]], loc_mm=(x, y, z), bev_mm=1.6)
    ob.rotation_euler = (math.radians(tilt_deg), 0, 0)
    return ob


def dots(name, centres_mm, r, z_mm, mats, mat_key):
    """A cluster of tiny dark discs merged into a single object (speaker grille)."""
    bm = bmesh.new()
    for cx, cy, cz in centres_mm:
        res = bmesh.ops.create_cone(bm, cap_ends=True, segments=12, radius1=r * MM, radius2=r * MM, depth=0.4 * MM)
        bmesh.ops.translate(bm, verts=res["verts"], vec=(cx * MM, cy * MM, cz * MM))
    return finish(name, bm, (0, 0, 0), [mats[mat_key]], smooth=False)


def slot(name, x, y, z, w, h, mats, mat_key, face="front"):
    """Dark rounded slot (USB-C port) on the front (+Y) or back (-Y) surface."""
    pts = rounded_rect(w, h, h / 2 - 0.05, 6)
    ob = extrude_poly(name, pts, 1.2, [mats[mat_key]], bev_mm=0.2)
    ob.rotation_euler = (math.radians(90), 0, math.radians(0 if face == "front" else 180))
    ob.location = Vector((x * MM, y * MM, z * MM))
    return ob
