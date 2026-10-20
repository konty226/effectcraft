"""Recettes Blender (5.2) pour le pipeline mymotion : à exécuter via le MCP Blender
(execute_blender_code) avec `exec(open(r"<chemin>/mymotion_blender.py", encoding="utf-8").read())`,
puis appeler les fonctions.

    setup_render(out_dir, w=1080, h=1920, fps=30, frames=(1, 90), samples=96, engine="CYCLES")
    logo_from_svg(svg_path, width_m=3.2, depth_m=0.11, bevel_m=0.012, color="#0957a2")
    studio_lights(sweep=True, frames=(28, 78))
    camera_push(start=(-2.2, -10.5, 1.9), end=(0.9, -8.6, 1.3), target=(0, 0, 1.0), frames=(1, 90), lens=50)
    export_camera(json_path, objects=("Repere titre",), points={"sol_centre": (0, 0, 0)}, scale=100)
        → caméra + repères pour l'outil camera_from_blender de mymotion (titres 2D dans la scène 3D)

Pièges Blender 5.2 déjà traités :
- l'EXR multicouche se choisit avec image_settings.media_type = "MULTI_LAYER_IMAGE" puis
  file_format = "OPEN_EXR_MULTILAYER" (il n'y a plus de "OPEN_EXR_MULTILAYER" sans media_type) ;
- la passe Shadow Catcher est dans view_layer.cycles.use_pass_shadow_catcher ;
- les EXR sont multipart (une partie par passe) : EffectCraft les lit, la passe Combined est l'image ;
- rendre l'animation dans un Blender séparé en arrière-plan : blender -b fichier.blend -a.
"""
import math
import re

import bpy


def _lin(c):
    c = c / 255
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def hex_linear(h, a=1.0):
    h = h.lstrip("#")
    return (_lin(int(h[0:2], 16)), _lin(int(h[2:4], 16)), _lin(int(h[4:6], 16)), a)


def setup_render(out_dir, w=1080, h=1920, fps=30, frames=(1, 90), samples=96, engine="CYCLES", name="rendu_"):
    """Cycles sur GPU (OptiX si dispo), débruitage, fond transparent, passes utiles au compositing,
    EXR multicouche 16 bits DWAA dans out_dir."""
    sc = bpy.context.scene
    r = sc.render
    r.engine = engine
    if engine == "CYCLES":
        prefs = bpy.context.preferences.addons["cycles"].preferences
        for t in ("OPTIX", "CUDA", "HIP", "METAL", "ONEAPI"):
            try:
                prefs.compute_device_type = t
                prefs.refresh_devices()
                if any(d.type == t for d in prefs.devices):
                    break
            except Exception:
                continue
        for d in prefs.devices:
            d.use = d.type == prefs.compute_device_type
        cy = sc.cycles
        cy.device = "GPU"
        cy.samples = samples
        cy.use_adaptive_sampling = True
        cy.adaptive_threshold = 0.02
        cy.use_denoising = True
        try:
            cy.denoiser = "OPTIX"
        except Exception:
            pass
    r.resolution_x, r.resolution_y, r.resolution_percentage = w, h, 100
    r.fps = fps
    sc.frame_start, sc.frame_end = frames
    r.film_transparent = True
    sc.view_settings.view_transform = "Standard"
    sc.view_settings.look = "None"
    vl = sc.view_layers[0]
    vl.use_pass_z = True
    vl.use_pass_mist = True
    vl.use_pass_ambient_occlusion = True
    vl.use_pass_cryptomatte_object = True
    try:
        vl.cycles.use_pass_shadow_catcher = True
    except Exception:
        pass
    im = r.image_settings
    if "media_type" in [p.identifier for p in im.bl_rna.properties]:
        im.media_type = "MULTI_LAYER_IMAGE"
    im.file_format = "OPEN_EXR_MULTILAYER"
    im.color_depth = "16"
    codecs = [i.identifier for i in im.bl_rna.properties["exr_codec"].enum_items]
    im.exr_codec = "DWAA" if "DWAA" in codecs else "ZIP"
    r.filepath = out_dir.rstrip("/\\") + "/" + name
    return {"engine": r.engine, "device": getattr(sc.cycles, "device", None), "out": r.filepath}


def logo_from_svg(svg_path, width_m=3.2, depth_m=0.11, bevel_m=0.012, color="#0957a2", metallic=0.85, roughness=0.22, name="Logo"):
    """Logo vectorisé (vectorize_logo de mymotion : un seul <path> fait de polygones « M…Z »)
    extrudé et biseauté, debout face à -Y, posé sur le sol."""
    txt = open(svg_path, encoding="utf-8").read()
    d = " ".join(re.findall(r' d="([^"]+)"', txt))
    xs, ys, loops = [], [], []
    for part in d.split("M")[1:]:
        pts = [tuple(map(float, p.split(","))) for p in part.replace("Z", "").split()]
        if len(pts) >= 3:
            loops.append(pts)
            xs += [p[0] for p in pts]
            ys += [p[1] for p in pts]
    cx, cy = (min(xs) + max(xs)) / 2, (min(ys) + max(ys)) / 2
    s = width_m / (max(xs) - min(xs))
    cu = bpy.data.curves.new(name, "CURVE")
    cu.dimensions = "2D"
    cu.fill_mode = "BOTH"
    for pts in loops:
        sp = cu.splines.new("POLY")
        sp.points.add(len(pts) - 1)
        for i, (x, y) in enumerate(pts):
            sp.points[i].co = ((x - cx) * s, (cy - y) * s, 0, 1)
        sp.use_cyclic_u = True
    cu.extrude, cu.bevel_depth, cu.bevel_resolution = depth_m, bevel_m, 3
    ob = bpy.data.objects.new(name, cu)
    bpy.context.scene.collection.objects.link(ob)
    ob.rotation_euler = (math.radians(90), 0, 0)
    ob.location = (0, 0, (max(ys) - min(ys)) * s / 2 + 0.2)
    m = bpy.data.materials.new(name + " matiere")
    m.use_nodes = True
    b = m.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = hex_linear(color)
    b.inputs["Metallic"].default_value = metallic
    b.inputs["Roughness"].default_value = roughness
    if "Coat Weight" in b.inputs:
        b.inputs["Coat Weight"].default_value = 0.7
    cu.materials.append(m)
    return ob


def studio_lights(sweep=True, frames=(28, 78), floor=True, world="#03081a"):
    """Clé chaude, contre-jour bleu, bande de lumière qui balaie (reflet sur le métal), sol attrapeur d'ombres."""
    sc = bpy.context.scene

    def area(name, loc, rot, energy, color, size, size_y=None):
        light = bpy.data.lights.new(name, "AREA")
        light.energy, light.color, light.shape = energy, color, "RECTANGLE"
        light.size, light.size_y = size, size_y or size
        o = bpy.data.objects.new(name, light)
        sc.collection.objects.link(o)
        o.location, o.rotation_euler = loc, [math.radians(a) for a in rot]
        return o

    area("Cle", (-3, -4, 4), (55, 0, -35), 900, (1, 0.97, 0.92), 3)
    area("Contre", (3.5, 3, 3), (-60, 0, 140), 1200, (0.17, 0.56, 0.88), 2.5)
    if sweep:
        sw = area("Balayage", (-6, -2.5, 1.2), (90, 0, 0), 600, (1, 1, 1), 0.25, 5)
        sw.keyframe_insert("location", frame=frames[0])
        sw.location.x = 6
        sw.keyframe_insert("location", frame=frames[1])
    if floor:
        bpy.ops.mesh.primitive_plane_add(size=30, location=(0, 0, 0))
        bpy.context.active_object.name = "Sol (shadow catcher)"
        bpy.context.active_object.is_shadow_catcher = True
    w = bpy.data.worlds.new("Monde")
    w.use_nodes = True
    w.node_tree.nodes["Background"].inputs[0].default_value = hex_linear(world)
    sc.world = w


def camera_push(start=(-2.2, -10.5, 1.9), end=(0.9, -8.6, 1.3), target=(0, 0, 1.0), frames=(1, 90), lens=50):
    sc = bpy.context.scene
    cd = bpy.data.cameras.new("Camera")
    cd.lens = lens
    cam = bpy.data.objects.new("Camera", cd)
    sc.collection.objects.link(cam)
    sc.camera = cam
    tgt = bpy.data.objects.new("Cible", None)
    sc.collection.objects.link(tgt)
    tgt.location = target
    c = cam.constraints.new("TRACK_TO")
    c.target, c.track_axis, c.up_axis = tgt, "TRACK_NEGATIVE_Z", "UP_Y"
    cam.location = start
    cam.keyframe_insert("location", frame=frames[0])
    cam.location = end
    cam.keyframe_insert("location", frame=frames[1])
    return cam


def to_effectcraft(p, w, h, scale):
    """Point Blender (Z en haut, mètres) → EffectCraft / After Effects (Y vers le bas, Z en profondeur,
    pixels). L'origine du monde Blender tombe au centre de la comp, à la profondeur 0."""
    return [round(w / 2 + scale * p[0], 3), round(h / 2 - scale * p[2], 3), round(scale * p[1], 3)]


def export_camera(json_path, objects=(), points=None, scale=100.0, camera=None, frames=None, aim=10.0):
    """Échantillonne la caméra active à chaque image (position, point visé, zoom en pixels, roulis) et la
    position monde de `objects` (empties, objets) ; `points` = repères fixes {nom: (x, y, z)}.
    `scale` = pixels par mètre dans EffectCraft (100 : un objet de 1 m fait 100 px à la profondeur 0)."""
    import json
    from mathutils import Vector

    sc = bpy.context.scene
    cam = camera or sc.camera
    r = sc.render
    w = round(r.resolution_x * r.resolution_percentage / 100)
    h = round(r.resolution_y * r.resolution_percentage / 100)
    f0, f1 = frames or (sc.frame_start, sc.frame_end)
    out = {"source": bpy.data.filepath, "camera": cam.name, "width": w, "height": h, "fps": r.fps / r.fps_base,
           "frame_start": f0, "frame_end": f1, "scale": scale, "frames": [], "objects": {}, "points": {}}
    obs = [bpy.data.objects[n] for n in objects]
    for n in objects:
        out["objects"][n] = []
    keep = sc.frame_current
    for f in range(f0, f1 + 1):
        sc.frame_set(f)
        mw = cam.matrix_world
        pos = mw.translation.copy()
        rot = mw.to_3x3()
        fwd = (rot @ Vector((0, 0, -1))).normalized()
        up = (rot @ Vector((0, 1, 0))).normalized()
        # Roulis : écart entre le haut de la caméra et le haut « sans roulis » (monde Z projeté).
        ref = Vector((0, 0, 1)) - fwd * fwd.dot(Vector((0, 0, 1)))
        roll = 0.0
        if ref.length > 1e-6:
            ref.normalize()
            roll = math.degrees(math.atan2(fwd.dot(ref.cross(up)), ref.dot(up)))
        # Zoom After Effects = distance (px) à laquelle la largeur de la comp couvre le champ horizontal.
        corners = cam.data.view_frame(scene=sc)
        half_w = max(abs(v.x) for v in corners)
        depth = abs(corners[0].z) or 1.0
        zoom = (w / 2) / (half_w / depth)
        out["frames"].append({"f": f, "position": to_effectcraft(pos, w, h, scale),
                              "poi": to_effectcraft(pos + fwd * aim, w, h, scale),
                              "zoom": round(zoom, 3), "roll": round(roll, 4)})
        for o in obs:
            out["objects"][o.name].append(to_effectcraft(o.matrix_world.translation, w, h, scale))
    sc.frame_set(keep)
    for n, p in (points or {}).items():
        out["points"][n] = to_effectcraft(p, w, h, scale)
    with open(json_path, "w", encoding="utf-8") as fh:
        json.dump(out, fh, ensure_ascii=False, indent=1)
    return {"json": json_path, "frames": len(out["frames"]), "zoom_first": out["frames"][0]["zoom"] if out["frames"] else None}

