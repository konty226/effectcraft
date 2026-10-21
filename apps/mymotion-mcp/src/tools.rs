//! The motion-design tools this proxy adds, built on EffectCraft's own MCP tools.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::brand::{self, Brands};
use crate::{Proxy, b64, draw, exr, lock, project, vector};

pub const INSTRUCTIONS: &str = "mymotion tools (motion design with Claude): render_filmstrip shows a stretch of the animation as a captioned contact sheet, so judge timing and spacing on it rather than on one frame. plot_curves draws a property's value and speed over time with its keyframes and reports overshoot and peak speed, to judge easing. focus_context returns what the user is looking at in the live app (active comp, current time, selected layers, properties and keys) plus an image, call it first when the user says \"this\" or \"the selected layer\". checkpoint {label} before a large change, restore_checkpoint {label} to go back. brand_kit (also resource mymotion://brand) returns the brand kit to follow (colours, fonts, timing), merged studio -> client -> project; the user has several clients: check `client` in its reply, switch with brand_use, list with brand_list, create one with brand_new, and save each client's projects under clients/<client>/projets/ so the right kit applies. Every job starts from a project brief: project_new creates clients/<client>/projets/<project>/ with BRIEF.md, SCRIPT.md, assets/, references/, moodboard/ (fill what the user said, ask for the rest); project_brief reads it with a sheet of the mood board and references; brief_update records answers and feedback; project_list shows projects and what their brief lacks.";

pub struct Checkpoint {
    pub label: String,
    pub state: String,
    pub history_label: String,
}

struct Def {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    props: fn() -> Value,
    required: &'static [&'static str],
    read_only: bool,
}

const DEFS: &[Def] = &[
    Def {
        name: "render_filmstrip",
        title: "Render filmstrip",
        description: "Render evenly spaced frames of a composition into one captioned contact sheet (time in seconds and frame number under each), to judge an animation's timing, spacing and composition over time. Range defaults to the work area.",
        props: || {
            json!({
                "comp": {"description": "Composition id or name (default: the active comp)."},
                "from": {"type": "number", "description": "Start, comp seconds (default: work area start)."},
                "to": {"type": "number", "description": "End, comp seconds (default: last frame of the work area)."},
                "frames": {"type": "integer", "description": "Number of frames, 2–36 (default 8)."},
                "columns": {"type": "integer", "description": "Frames per row (default: up to 6, or 4 for landscape comps)."},
                "max_side": {"type": "integer", "description": "Longest side of each frame in pixels, 64–1024 (default 300)."},
                "transparent": {"type": "boolean", "description": "Keep alpha (shown over a checkerboard)."}
            })
        },
        required: &[],
        read_only: true,
    },
    Def {
        name: "plot_curves",
        title: "Plot animation curves",
        description: "Sample an animated property over time and draw its value graph and speed graph (as a graph editor does), with keyframes as dashed lines. The text reply gives start/end/min/max per dimension, overshoot in percent of the move and the peak speed, to judge easing and bounce.",
        props: || {
            json!({
                "layer": {"description": "Layer id, \"#n\" or name."},
                "path": {"type": "string", "description": "Property path, e.g. transform/position, transform/scale, effects/#1/blurriness."},
                "comp": {"description": "Composition id or name (default: the active comp)."},
                "from": {"type": "number", "description": "Start, comp seconds (default: work area start)."},
                "to": {"type": "number", "description": "End, comp seconds (default: work area end)."},
                "samples": {"type": "integer", "description": "Samples, 16–400 (default 120)."},
                "width": {"type": "integer", "description": "Image width (default 900)."},
                "height": {"type": "integer", "description": "Image height (default 520)."}
            })
        },
        required: &["layer", "path"],
        read_only: true,
    },
    Def {
        name: "focus_context",
        title: "Focus context",
        description: "What the user is looking at right now: active comp (size, fps, duration, work area, layers with the selected ones marked), current time, selected properties and keyframes, plus images: the rendered frame at the current time and, in the live app, the Composition panel (zoom, overlays, handles).",
        props: || {
            json!({
                "max_side": {"type": "integer", "description": "Longest side of each image (default 720)."},
                "image": {"type": "boolean", "description": "Include the images (default true)."}
            })
        },
        required: &[],
        read_only: true,
    },
    Def {
        name: "checkpoint",
        title: "Checkpoint",
        description: "Remember the current undo-history state under a label (for this server session), before a large change. restore_checkpoint goes back to it; the history keeps every branch, so nothing is lost.",
        props: || json!({"label": {"type": "string", "description": "Name of the checkpoint."}}),
        required: &["label"],
        read_only: false,
    },
    Def {
        name: "checkpoints",
        title: "List checkpoints",
        description: "The checkpoints of this session, oldest first, and the current history state.",
        props: || json!({}),
        required: &[],
        read_only: true,
    },
    Def {
        name: "restore_checkpoint",
        title: "Restore checkpoint",
        description: "Jump the project back (or forward) to a checkpoint's history state. Later states stay in the history and can be restored too.",
        props: || json!({"label": {"type": "string", "description": "Name given to checkpoint."}}),
        required: &["label"],
        read_only: false,
    },
    Def {
        name: "brand_kit",
        title: "Brand kit",
        description: "The brand kit to follow (colours, fonts, logo, safe areas, durations, easing) as JSON, merged in cascade: studio kit, then the client's, then the project folder's and the project's own (<project>.brand.json). The client is the one set with brand_use, else the one whose folder (clients/<client>/…) holds the open project. The reply lists `client` and the `sources` merged. Use its values when creating or restyling anything.",
        props: || json!({"client": {"type": "string", "description": "Use this client's kit instead of the active / detected one."}}),
        required: &[],
        read_only: true,
    },
    Def {
        name: "brand_list",
        title: "List clients",
        description: "The clients that have a brand kit (folders clients/<client>/ with a brand_kit.json), the studio kit, the client in use and the open project.",
        props: || json!({}),
        required: &[],
        read_only: true,
    },
    Def {
        name: "brand_use",
        title: "Use client",
        description: "Use this client's brand kit for the rest of the session (whatever project is open); null goes back to detecting the client from the open project's folder.",
        props: || json!({"client": {"type": ["string", "null"], "description": "Client folder name, or null."}}),
        required: &["client"],
        read_only: false,
    },
    Def {
        name: "brand_new",
        title: "New client",
        description: "Create a client: clients/<client>/ with projets/, medias/, out/ and a brand_kit.json holding only what this client changes (anything missing comes from the studio kit), or a copy of another client's kit (`from`). Then fill it with the client's colours, fonts and logo, and save the client's projects in clients/<client>/projets/.",
        props: || {
            json!({
                "client": {"type": "string", "description": "Client name (used as the folder name)."},
                "from": {"type": "string", "description": "Copy this existing client's kit instead of starting empty."}
            })
        },
        required: &["client"],
        read_only: false,
    },
    Def {
        name: "project_new",
        title: "New project",
        description: "Create a client's project folder clients/<client>/projets/<project>/ with BRIEF.md (sections: Infos, Objectif et message clé, Public cible, Script, Assets fournis, Références et exemples, Mood board, Style, Contraintes et mentions obligatoires, Livrables, Notes de suivi), SCRIPT.md (scene table) and the folders assets/, references/, moodboard/, out/. Fill `brief` with what the user already said (keys are section names, loosely matched: \"objectif\", \"style\", \"livrables\"…; values are text or lists), `script` with the scene breakdown. Then ask the user for the missing essentials, tell them to drop files in assets/, references/ and moodboard/, and save the .ecproj in the folder (path in the reply).",
        props: || {
            json!({
                "client": {"type": "string", "description": "Existing client (brand_list); create it with brand_new first."},
                "project": {"type": "string", "description": "Project name (folder name)."},
                "brief": {"type": "object", "description": "Section name → text or list of lines."},
                "script": {"type": "string", "description": "Scene breakdown for SCRIPT.md (Markdown table or text)."}
            })
        },
        required: &["client", "project"],
        read_only: false,
    },
    Def {
        name: "project_brief",
        title: "Project brief",
        description: "Read a project's brief before creating anything: BRIEF.md by section, SCRIPT.md, what is still missing, every supplied file (assets/, references/, moodboard/ with kind and image size), notes in references/*.txt, plus a visual sheet of the mood board and reference images to derive the style from. Defaults to the project holding the open .ecproj.",
        props: || {
            json!({
                "client": {"type": "string", "description": "Client (with `project`); default: from the open project."},
                "project": {"type": "string", "description": "Project name; default: the open project's."},
                "images": {"type": "boolean", "description": "Include the mood board / references sheet (default true)."}
            })
        },
        required: &[],
        read_only: true,
    },
    Def {
        name: "brief_update",
        title: "Update brief",
        description: "Write one section of a project's BRIEF.md (or SCRIPT.md with section \"script-file\"), e.g. to record what the user answered, a decision or feedback (section \"Notes de suivi\", append true). Section names are loosely matched; an unknown name adds a new section.",
        props: || {
            json!({
                "section": {"type": "string", "description": "Section name (\"style\", \"objectif\", \"notes\"…) or \"script-file\" for SCRIPT.md."},
                "content": {"type": "string", "description": "Markdown text."},
                "append": {"type": "boolean", "description": "Add below the current text instead of replacing it."},
                "client": {"type": "string", "description": "Default: from the open project."},
                "project": {"type": "string", "description": "Default: the open project's."}
            })
        },
        required: &["section", "content"],
        read_only: false,
    },
    Def {
        name: "project_list",
        title: "List projects",
        description: "Projects of a client (or of every client) with their folder, number of files and the brief sections still missing.",
        props: || json!({"client": {"type": "string", "description": "Only this client."}}),
        required: &[],
        read_only: true,
    },
    Def {
        name: "import_render",
        title: "Import 3D render",
        description: "Import a 3D render (an image sequence, typically Blender's multi-part / multilayer OpenEXR) and stack its passes in a composition ready to finish: the Shadow Catcher pass multiplied onto what is below (if rendered), optionally Ambient Occlusion, then the beauty (Combined, with alpha) on top. Lists every pass found (Combined, Depth, Mist, AO, Shadow Catcher, Cryptomatte) so others can be extracted with the EXtractoR effect (channel names like ViewLayer.Mist.Z). `path` is the first frame, a sequence pattern from project_brief (name_[0001-0090].exr) or a folder.",
        props: || {
            json!({
                "path": {"type": "string", "description": "First frame, sequence pattern or folder of the render."},
                "fps": {"type": "number", "description": "Frame rate of the render (default 30)."},
                "comp": {"type": "string", "description": "Add to this existing composition; default: a new comp sized like the render."},
                "name": {"type": "string", "description": "Name of the new comp and layer prefix (default: the sequence name)."},
                "shadow": {"type": "number", "description": "Shadow Catcher opacity 0-100 (default 85; 0 = skip)."},
                "ao": {"type": "number", "description": "Ambient Occlusion opacity 0-100 (default 0 = skip)."}
            })
        },
        required: &["path"],
        read_only: false,
    },
    Def {
        name: "vectorize_logo",
        title: "Vectorize logo",
        description: "Turn a raster logo (PNG with transparency, or dark logo on light background) into a clean SVG: sub-pixel contours, smoothed, holes kept (even-odd). Use it to extrude a logo in 3D in Blender (rebuild curves from its path) or as a sharp shape. Writes next to the source unless `output` is given.",
        props: || {
            json!({
                "input": {"type": "string", "description": "PNG / JPEG of the logo."},
                "output": {"type": "string", "description": "SVG to write (default: <input>_vectorise.svg)."},
                "precision": {"type": "integer", "description": "Upsampling 2-12 (default 6; higher = finer small letters)."},
                "color": {"type": "string", "description": "Fill colour of the SVG (default #000000)."}
            })
        },
        required: &["input"],
        read_only: false,
    },
    Def {
        name: "camera_from_blender",
        title: "Camera from Blender",
        description: "Recreate a Blender camera in the active (or given) composition from the JSON written by mymotion_blender.export_camera: a two-node camera keyed on every frame (position, point of interest, zoom, roll), plus 3D nulls for the exported markers (Blender objects followed frame by frame, and fixed points), so 2D titles, shapes and footage placed in 3D line up with the Blender render (import_render) and move with its perspective. Time 0 of the comp = Blender frame_start.",
        props: || {
            json!({
                "json": {"type": "string", "description": "Camera JSON written by export_camera (usually <project>/3d/camera_<shot>.json)."},
                "comp": {"type": "string", "description": "Composition to add the camera to (default: the active one)."},
                "name": {"type": "string", "description": "Camera layer name (default Camera Blender)."}
            })
        },
        required: &["json"],
        read_only: false,
    },
    Def {
        name: "replace_screen",
        title: "Replace screen",
        description: "Screen replacement on footage in one call: finds the green (or blue) screen of a phone, laptop or TV on a reference frame, puts a 4-point perspective tracker on its corners (pushed `expand` px outward to hide rounded corners under the bezel), analyses the whole shot, applies the corner pin to the content (a comp, footage item or layer, placed under the footage) and keys the screen colour out with Key Light. The footage layer must fill its comp 1:1 (work in a comp made from the footage, then nest it).",
        props: || {
            json!({
                "layer": {"description": "Footage layer showing the screen."},
                "content": {"description": "What goes on the screen: a composition or footage item name/id, or a layer of this comp."},
                "comp": {"type": "string", "description": "Composition holding the footage (default: the active one)."},
                "color": {"type": "string", "description": "green (default) or blue."},
                "time": {"type": "number", "description": "Reference frame where the screen is fully visible, comp seconds (default 0)."},
                "expand": {"type": "number", "description": "Push the corners outward by this many px (default 10)."},
                "feature": {"type": "number", "description": "Tracker feature size px (default 56)."},
                "search": {"type": "number", "description": "Tracker search size px (default 150)."}
            })
        },
        required: &["layer", "content"],
        read_only: false,
    },
    Def {
        name: "render_before_after",
        title: "Before / after",
        description: "A contact sheet of the same frames with the effects of `layers` off (AVANT, top row) and on (APRES, bottom row), to judge a key, a track, a clean-up or a grade.",
        props: || {
            json!({
                "layers": {"type": "array", "description": "Layers whose effects make the VFX (their fx switch is toggled)."},
                "comp": {"description": "Composition (default: the active one)."},
                "times": {"type": "array", "description": "Comp seconds (default: 3 spread over the work area)."},
                "max_side": {"type": "integer", "description": "Longest side of each frame (default 420)."}
            })
        },
        required: &["layers"],
        read_only: true,
    },
    Def {
        name: "camera_to_blender",
        title: "Camera to Blender",
        description: "Write an EffectCraft camera (typically the 3D Camera Tracker's solved camera, one-node and keyed per frame) as a JSON that mymotion_blender.import_camera turns into an animated Blender camera, so 3D objects rendered in Blender sit in the filmed shot. The world is re-based on the tracked ground: `origin` (EffectCraft world point, e.g. the solve's ground plane centre) becomes Blender's origin and `normal` its +Z, with 1 m = `scale` px. Frames span the comp's work area.",
        props: || {
            json!({
                "camera": {"description": "Camera layer (id, name or #n)."},
                "json": {"type": "string", "description": "Output JSON path (usually <project>/3d/camera_<shot>_solve.json)."},
                "comp": {"type": "string", "description": "Composition (default: the active one)."},
                "origin": {"type": "array", "description": "Ground origin [x, y, z] in EffectCraft world px (default: comp centre at z 0)."},
                "normal": {"type": "array", "description": "Ground normal [x, y, z] (default [0, -1, 0], i.e. world up)."},
                "scale": {"type": "number", "description": "EffectCraft px per Blender metre (default 100)."},
                "twoNode": {"type": "boolean", "description": "Use the point of interest (two-node camera); default false."}
            })
        },
        required: &["camera", "json"],
        read_only: false,
    },
];

pub fn is_ours(name: &str) -> bool {
    DEFS.iter().any(|d| d.name == name)
}

pub fn definitions() -> Vec<Value> {
    DEFS.iter()
        .map(|d| {
            json!({
                "name": d.name,
                "title": d.title,
                "description": d.description,
                "inputSchema": {"type": "object", "properties": (d.props)(), "required": d.required, "additionalProperties": false},
                "annotations": {"title": d.title, "readOnlyHint": d.read_only, "destructiveHint": false, "idempotentHint": d.read_only, "openWorldHint": false}
            })
        })
        .collect()
}

/// Reject unknown argument keys and missing required ones, as EffectCraft's own tools do.
pub fn check_args(name: &str, args: &Value) -> Result<(), String> {
    let Some(def) = DEFS.iter().find(|d| d.name == name) else { return Err(format!("unknown tool {name}")) };
    let props = (def.props)();
    let accepted: Vec<&str> = props.as_object().map(|o| o.keys().map(String::as_str).collect()).unwrap_or_default();
    let empty = Map::new();
    let given = match args {
        Value::Object(o) => o,
        Value::Null => &empty,
        _ => return Err(format!("{name}: arguments must be an object")),
    };
    if let Some(k) = given.keys().find(|k| !accepted.contains(&k.as_str())) {
        return Err(format!("{name}: unknown argument `{k}` (accepted: {})", accepted.join(", ")));
    }
    if let Some(r) = def.required.iter().find(|r| !given.contains_key(**r)) {
        return Err(format!("{name}: missing argument `{r}`"));
    }
    Ok(())
}

/// Run one of our tools; failures become an `isError` result.
pub fn run(p: &Proxy, name: &str, args: &Value) -> Value {
    let r = match name {
        "render_filmstrip" => filmstrip(p, args),
        "plot_curves" => plot_curves(p, args),
        "focus_context" => focus_context(p, args),
        "checkpoint" => checkpoint(p, args),
        "checkpoints" => checkpoints(p),
        "restore_checkpoint" => restore_checkpoint(p, args),
        "brand_kit" => brand_kit(p, a_str(args, "client")).map(|v| vec![text_content(&v)]),
        "brand_list" => brand_list(p).map(|v| vec![text_content(&v)]),
        "brand_use" => brand_use(p, args).map(|v| vec![text_content(&v)]),
        "brand_new" => brand_new(p, args).map(|v| vec![text_content(&v)]),
        "project_new" => project_new(p, args).map(|v| vec![text_content(&v)]),
        "project_brief" => project_brief(p, args),
        "brief_update" => brief_update(p, args).map(|v| vec![text_content(&v)]),
        "project_list" => project_list(p, args).map(|v| vec![text_content(&v)]),
        "import_render" => import_render(p, args).map(|v| vec![text_content(&v)]),
        "vectorize_logo" => vectorize_logo(args).map(|v| vec![text_content(&v)]),
        "camera_from_blender" => camera_from_blender(p, args).map(|v| vec![text_content(&v)]),
        "replace_screen" => replace_screen(p, args).map(|v| vec![text_content(&v)]),
        "render_before_after" => render_before_after(p, args),
        "camera_to_blender" => camera_to_blender(p, args).map(|v| vec![text_content(&v)]),
        _ => Err(format!("unknown tool {name}")),
    };
    match r {
        Ok(content) => json!({"content": content, "isError": false}),
        Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
    }
}

// ---- helpers over child results ----

/// The text of a tool result's text parts.
pub fn text_of(result: &Value) -> String {
    result.get("content").and_then(Value::as_array).into_iter().flatten().filter_map(|c| c.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join("\n")
}

/// A tool result's first JSON text part.
fn json_of(result: &Value) -> Value {
    result
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| c.get("text").and_then(Value::as_str))
        .find_map(|t| serde_json::from_str::<Value>(t).ok())
        .unwrap_or(Value::Null)
}

fn image_of(result: &Value) -> Result<image::RgbaImage, String> {
    let data = result
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|c| c.get("type").and_then(Value::as_str) == Some("image"))
        .and_then(|c| c.get("data").and_then(Value::as_str))
        .ok_or("the reply holds no image")?;
    let bytes = b64::decode(data).ok_or("the reply's image is not valid base64")?;
    Ok(image::load_from_memory(&bytes).map_err(|e| format!("cannot read the image: {e}"))?.to_rgba8())
}

fn text_content(v: &Value) -> Value {
    json!({"type": "text", "text": v.to_string()})
}

fn image_content(img: &image::RgbaImage) -> Result<Value, String> {
    Ok(json!({"type": "image", "mimeType": "image/png", "data": b64::encode(&draw::png(img)?)}))
}

fn num(a: &Value, key: &str) -> Option<f64> {
    a.get(key).and_then(Value::as_f64).filter(|v| v.is_finite())
}

fn int(a: &Value, key: &str, default: i64, lo: i64, hi: i64) -> i64 {
    a.get(key).and_then(Value::as_i64).unwrap_or(default).clamp(lo, hi)
}

/// Optional `comp` argument forwarded as-is.
fn with_comp(a: &Value, mut v: Value) -> Value {
    if let Some(c) = a.get("comp").filter(|c| !c.is_null()) {
        v["comp"] = c.clone();
    }
    v
}

struct CompInfo {
    id: Value,
    name: String,
    width: f64,
    height: f64,
    fps: f64,
    work: (f64, f64),
    raw: Value,
}

fn comp_info(p: &Proxy, a: &Value) -> Result<CompInfo, String> {
    let c = json_of(&p.call("get_comp", with_comp(a, json!({})))?);
    let fps = c.get("frameRate").and_then(Value::as_f64).filter(|f| *f > 0.0).unwrap_or(30.0);
    let duration = c.get("duration").and_then(Value::as_f64).unwrap_or(0.0);
    let work = match c.get("workArea").and_then(Value::as_array).map(Vec::as_slice) {
        Some([s, e]) => (s.as_f64().unwrap_or(0.0), e.as_f64().unwrap_or(duration)),
        _ => (0.0, duration),
    };
    Ok(CompInfo {
        id: c.get("id").cloned().unwrap_or(Value::Null),
        name: c.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
        width: c.get("width").and_then(Value::as_f64).unwrap_or(1.0),
        height: c.get("height").and_then(Value::as_f64).unwrap_or(1.0),
        fps,
        work,
        raw: c,
    })
}

fn snap(t: f64, fps: f64) -> f64 {
    (t * fps).round() / fps
}

// ---- tools ----

fn filmstrip(p: &Proxy, a: &Value) -> Result<Vec<Value>, String> {
    let info = comp_info(p, a)?;
    let n = int(a, "frames", 8, 2, 36) as usize;
    let last = (info.work.1 - 1.0 / info.fps).max(info.work.0);
    let (from, to) = (num(a, "from").unwrap_or(info.work.0), num(a, "to").unwrap_or(last));
    if to < from {
        return Err(format!("render_filmstrip: `to` ({to}) is before `from` ({from})"));
    }
    let landscape = info.width > info.height;
    let columns = int(a, "columns", if landscape { 4 } else { 6 }, 1, 12) as u32;
    let max_side = int(a, "max_side", 300, 64, 1024);
    let transparent = a.get("transparent").and_then(Value::as_bool).unwrap_or(false);
    let mut cells = Vec::with_capacity(n);
    let mut times = Vec::with_capacity(n);
    for i in 0..n {
        let t = snap(from + (to - from) * i as f64 / (n - 1) as f64, info.fps);
        let mut args = json!({"comp": info.id, "time": t, "max_side": max_side});
        if transparent {
            args["transparent"] = json!(true);
        }
        let image = image_of(&p.call("render_frame", args)?)?;
        let frame = (t * info.fps).round() as i64;
        cells.push(draw::Cell { image, caption: format!("{t:.2}S F{frame}") });
        times.push(json!({"time": t, "frame": frame}));
    }
    let title = format!("{} {:.0}X{:.0} {}FPS  {:.2}S-{:.2}S", info.name, info.width, info.height, info.fps, from, to);
    let sheet = draw::filmstrip(&cells, columns, transparent, &title);
    let summary = json!({"comp": info.id, "name": info.name, "from": from, "to": to, "frames": times, "columns": columns.min(n as u32), "sheet": [sheet.width(), sheet.height()]});
    Ok(vec![text_content(&summary), image_content(&sheet)?])
}

fn numbers(v: &Value) -> Option<Vec<f64>> {
    match v {
        Value::Number(n) => n.as_f64().map(|f| vec![f]),
        Value::Bool(b) => Some(vec![if *b { 1.0 } else { 0.0 }]),
        Value::Array(a) if !a.is_empty() => a.iter().map(Value::as_f64).collect(),
        _ => None,
    }
}

fn plot_curves(p: &Proxy, a: &Value) -> Result<Vec<Value>, String> {
    let info = comp_info(p, a)?;
    let (from, to) = (num(a, "from").unwrap_or(info.work.0), num(a, "to").unwrap_or(info.work.1));
    if to <= from {
        return Err(format!("plot_curves: `to` ({to}) must be after `from` ({from})"));
    }
    let n = int(a, "samples", 120, 16, 400) as usize;
    let layer = a.get("layer").cloned().unwrap_or(Value::Null);
    let path = a.get("path").and_then(Value::as_str).ok_or("plot_curves: `path` must be a string")?.to_string();
    let mut times = Vec::with_capacity(n);
    let mut values: Vec<Vec<f64>> = Vec::with_capacity(n);
    let mut first = Value::Null;
    for i in 0..n {
        let t = from + (to - from) * i as f64 / (n - 1) as f64;
        let r = json_of(&p.call("get_property", json!({"comp": info.id, "layer": layer, "path": path, "time": t}))?);
        let v = r.get("evaluated").filter(|v| !v.is_null()).or_else(|| r.get("value")).cloned().unwrap_or(Value::Null);
        let v = numbers(&v).ok_or_else(|| format!("plot_curves: {path} is not numeric ({v}); plot a number, a point or a colour"))?;
        if i == 0 {
            first = r;
        }
        times.push(t);
        values.push(v);
    }
    let dims_n = values.iter().map(Vec::len).min().unwrap_or(0);
    let labels = ["X", "Y", "Z", "W"];
    let mut dims: Vec<(String, Vec<f64>)> = (0..dims_n).map(|d| (labels.get(d).unwrap_or(&"V").to_string(), values.iter().map(|v| v[d]).collect())).collect();
    if dims_n == 1 {
        dims[0].0 = "VALUE".into();
    }
    // Hide dimensions that never move (the z of a 2D scale) when another one does.
    let moving = |v: &[f64]| v.iter().any(|x| (x - v[0]).abs() > 1e-9);
    if dims.iter().any(|(_, v)| moving(v)) {
        dims.retain(|(_, v)| moving(v));
    }
    // Uniform scale and the like: one curve labelled "X=Y" instead of curves drawn on top of each other.
    if dims.len() > 1 && dims.iter().all(|(_, v)| v.iter().zip(&dims[0].1).all(|(a, b)| (a - b).abs() < 1e-9)) {
        let name = dims.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join("=");
        dims.truncate(1);
        dims[0].0 = name;
    }
    let dt = (to - from) / (n - 1) as f64;
    let speed: Vec<f64> = (0..n)
        .map(|i| {
            let (a, b) = (i.saturating_sub(1), (i + 1).min(n - 1));
            let span = (b - a) as f64 * dt;
            let sq: f64 = dims.iter().map(|(_, v)| ((v[b] - v[a]) / span).powi(2)).sum();
            sq.sqrt()
        })
        .collect();
    let keys: Vec<(f64, Value)> = first
        .get("keys")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|k| Some((k.get("compTime").or_else(|| k.get("time"))?.as_f64()?, k.get("value").cloned().unwrap_or(Value::Null))))
        .collect();
    let key_times: Vec<f64> = keys.iter().map(|(t, _)| *t).filter(|t| *t >= from - 1e-9 && *t <= to + 1e-9).collect();

    let per_dim: Vec<Value> = dims
        .iter()
        .map(|(name, v)| {
            let (start, end) = (v[0], v[v.len() - 1]);
            let (min, max) = v.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), x| (lo.min(*x), hi.max(*x)));
            let change = end - start;
            let beyond = if change >= 0.0 { max - end } else { end - min };
            let overshoot = if change.abs() > 1e-9 { (beyond.max(0.0) / change.abs() * 100.0 * 10.0).round() / 10.0 } else { 0.0 };
            json!({"dim": name, "start": start, "end": end, "min": min, "max": max, "overshootPct": overshoot})
        })
        .collect();
    let (peak_i, peak) = speed.iter().enumerate().fold((0, 0.0f64), |(bi, bv), (i, v)| if *v > bv { (i, *v) } else { (bi, bv) });
    let summary = json!({
        "layer": layer, "path": path, "name": first.get("name"), "from": from, "to": to, "samples": n,
        "keys": keys.iter().map(|(t, v)| json!({"time": t, "value": v})).collect::<Vec<_>>(),
        "expression": first.get("expression"),
        "dims": per_dim,
        "peakSpeed": {"value": (peak * 100.0).round() / 100.0, "time": (times[peak_i] * 1000.0).round() / 1000.0, "unitsPerSecond": true}
    });
    let title = format!("{} {}", layer.as_str().map(str::to_string).unwrap_or_else(|| layer.to_string()), path);
    let series = draw::Series { times, dims, speed, keys: key_times };
    let img = draw::curves(&series, &title, int(a, "width", 900, 320, 2400) as u32, int(a, "height", 520, 240, 1600) as u32)
        .ok_or("plot_curves: cannot allocate the image")?;
    Ok(vec![text_content(&summary), image_content(&img)?])
}

fn focus_context(p: &Proxy, a: &Value) -> Result<Vec<Value>, String> {
    let state = json_of(&p.call("get_state", json!({}))?);
    let st = state.get("state").cloned().unwrap_or(Value::Null);
    let active = st.get("active_comp").cloned().unwrap_or(Value::Null);
    let pick = |k: &str| st.get(k).cloned().unwrap_or(Value::Null);
    let mut out = json!({
        "activeComp": active,
        "times": pick("times"),
        "selectedLayers": pick("selected_layers"),
        "selectedProps": pick("selected_props"),
        "selectedKeys": pick("selected_keys"),
        "tool": pick("tool"),
    });
    if !active.is_null() {
        let info = comp_info(p, &json!({"comp": active}))?;
        let selected: Vec<Value> = match out["selectedLayers"].clone() {
            Value::Array(v) => v,
            Value::Object(o) => o.get(&active.to_string()).and_then(Value::as_array).cloned().unwrap_or_default(),
            _ => vec![],
        };
        let layers: Vec<Value> = info
            .raw
            .get("layers")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|l| {
                let id = l.get("id").cloned().unwrap_or(Value::Null);
                json!({"index": l.get("index"), "id": id, "name": l.get("name"), "type": l.get("type"), "in": l.get("in"), "out": l.get("out"), "parent": l.get("parent"), "selected": selected.contains(&id)})
            })
            .collect();
        out["comp"] = json!({"id": info.id, "name": info.name, "size": [info.width, info.height], "fps": info.fps, "workArea": [info.work.0, info.work.1], "duration": info.raw.get("duration"), "layers": layers});
    }
    let mut content = vec![];
    if a.get("image").and_then(Value::as_bool).unwrap_or(true) {
        let max_side = int(a, "max_side", 720, 128, 2400);
        // The rendered frame at the current time, then (live app only) the Composition panel with
        // its overlays, zoom and selection handles.
        let mut images = vec![];
        if !active.is_null() {
            images.push(("frame at the current time", p.call("render_frame", json!({"max_side": max_side}))));
        }
        images.push(("Composition panel", p.call("screenshot", json!({"panel": "Composition", "max_side": max_side}))));
        let mut shown = vec![];
        for (what, r) in images {
            if let Ok(img) = r.and_then(|r| image_of(&r)) {
                content.push(image_content(&img)?);
                shown.push(what);
            }
        }
        out["images"] = json!(shown);
    }
    content.insert(0, text_content(&out));
    Ok(content)
}

fn current_state(p: &Proxy) -> Result<(String, String), String> {
    let h = json_of(&p.call("history", json!({}))?);
    let states = h.get("states").and_then(Value::as_array).cloned().unwrap_or_default();
    let cur = states.iter().find(|s| s.get("current").and_then(Value::as_bool) == Some(true)).ok_or("history: no current state")?;
    let id = cur.get("id").and_then(Value::as_str).ok_or("history: state without id")?.to_string();
    Ok((id, cur.get("label").and_then(Value::as_str).unwrap_or_default().to_string()))
}

fn checkpoint(p: &Proxy, a: &Value) -> Result<Vec<Value>, String> {
    let label =
        a.get("label").and_then(Value::as_str).map(str::trim).filter(|l| !l.is_empty()).ok_or("checkpoint: `label` must be a non-empty string")?.to_string();
    let (state, history_label) = current_state(p)?;
    let mut list = lock(&p.checkpoints);
    list.retain(|c| c.label != label);
    list.push(crate::tools::Checkpoint { label: label.clone(), state: state.clone(), history_label: history_label.clone() });
    Ok(vec![text_content(&json!({"checkpoint": label, "state": state, "after": history_label, "count": list.len()}))])
}

fn checkpoints(p: &Proxy) -> Result<Vec<Value>, String> {
    let current = current_state(p).map(|(id, label)| json!({"state": id, "label": label})).unwrap_or(Value::Null);
    let list: Vec<Value> = lock(&p.checkpoints).iter().map(|c| json!({"label": c.label, "state": c.state, "after": c.history_label})).collect();
    Ok(vec![text_content(&json!({"checkpoints": list, "current": current}))])
}

fn restore_checkpoint(p: &Proxy, a: &Value) -> Result<Vec<Value>, String> {
    let label = a.get("label").and_then(Value::as_str).unwrap_or_default();
    let state = {
        let list = lock(&p.checkpoints);
        let found = list.iter().find(|c| c.label == label).map(|c| c.state.clone());
        found.ok_or_else(|| {
            let names: Vec<&str> = list.iter().map(|c| c.label.as_str()).collect();
            format!("restore_checkpoint: no checkpoint `{label}` (have: {})", if names.is_empty() { "none".into() } else { names.join(", ") })
        })?
    };
    let r = p.call("history", json!({"goto": state}))?;
    Ok(vec![text_content(&json!({"restored": label, "state": state, "history": json_of(&r)}))])
}

fn a_str<'a>(a: &'a Value, key: &str) -> Option<&'a str> {
    a.get(key).and_then(Value::as_str)
}

fn brands(p: &Proxy) -> Result<&Brands, String> {
    p.brands.as_ref().ok_or_else(|| "no brand kit: start mymotion-mcp with --brand FILE.json (the studio kit; see mymotion/brand_kit.example.json)".to_string())
}

/// The open project's file, if it has been saved.
fn project_path(p: &Proxy) -> Option<PathBuf> {
    let r = json_of(&p.call("get_project", json!({})).ok()?);
    r.get("path").and_then(Value::as_str).filter(|s| !s.is_empty()).map(PathBuf::from)
}

fn brand_kit(p: &Proxy, client: Option<&str>) -> Result<Value, String> {
    let b = brands(p)?;
    let project = project_path(p);
    let forced = client.map(str::to_string).or_else(|| lock(&p.client).clone());
    let r = b.resolve(project.as_deref(), forced.as_deref())?;
    Ok(json!({
        "client": r.client,
        "project": project,
        "sources": r.sources,
        "kit": r.kit,
    }))
}

fn brand_list(p: &Proxy) -> Result<Value, String> {
    let b = brands(p)?;
    let project = project_path(p);
    let detected = project.as_deref().and_then(|pp| b.client_of(pp));
    Ok(json!({
        "studio": b.default,
        "clientsFolder": b.root.join(brand::CLIENTS),
        "clients": b.list(),
        "active": *lock(&p.client),
        "detected": detected,
        "project": project,
    }))
}

fn brand_use(p: &Proxy, a: &Value) -> Result<Value, String> {
    let b = brands(p)?;
    let client = match a.get("client") {
        Some(Value::String(c)) => {
            let c = brand::check_client(c)?.to_string();
            if !b.client_dir(&c).join(brand::FILE).is_file() {
                let names: Vec<String> = b.list().iter().filter_map(|v| v["client"].as_str().map(str::to_string)).collect();
                return Err(format!(
                    "brand_use: no client `{c}` (have: {}); create it with brand_new",
                    if names.is_empty() { "none".into() } else { names.join(", ") }
                ));
            }
            Some(c)
        }
        Some(Value::Null) | None => None,
        Some(other) => return Err(format!("brand_use: `client` must be a string or null, not {other}")),
    };
    *lock(&p.client) = client.clone();
    Ok(
        json!({"active": client, "note": if client.is_none() { "client detected from the open project's folder" } else { "this client's kit applies whatever project is open" }}),
    )
}

fn brand_new(p: &Proxy, a: &Value) -> Result<Value, String> {
    let b = brands(p)?;
    let client = a_str(a, "client").ok_or("brand_new: `client` must be a string")?;
    let kit = b.create(client, a_str(a, "from"))?;
    let dir = kit.parent().map(Path::to_path_buf).unwrap_or_default();
    Ok(json!({"client": client.trim(), "kit": kit, "projects": dir.join("projets"), "media": dir.join("medias"), "out": dir.join("out")}))
}

// ---- projects and briefs ----

/// (client, project, folder) from the arguments, else from the open .ecproj.
fn project_target(p: &Proxy, a: &Value) -> Result<(String, String, PathBuf), String> {
    let b = brands(p)?;
    match (a_str(a, "client"), a_str(a, "project")) {
        (Some(c), Some(n)) => {
            let d = project::dir(b, c, n)?;
            if !d.join(project::BRIEF).is_file() {
                return Err(format!("no project `{n}` for client `{c}` (project_list shows them; project_new creates one)"));
            }
            Ok((c.trim().to_string(), n.trim().to_string(), d))
        }
        (None, None) => {
            let open = project_path(p).ok_or("no project given and the open project is not saved: pass `client` and `project`")?;
            project::of_file(b, &open).ok_or_else(|| {
                format!(
                    "the open project {} is not in a project folder (clients/<client>/projets/<project>/ with a BRIEF.md): pass `client` and `project`",
                    open.display()
                )
            })
        }
        _ => Err("pass both `client` and `project`, or neither (the open project)".into()),
    }
}

fn project_new(p: &Proxy, a: &Value) -> Result<Value, String> {
    let b = brands(p)?;
    let client = a_str(a, "client").ok_or("project_new: `client` must be a string")?;
    let name = a_str(a, "project").ok_or("project_new: `project` must be a string")?;
    let empty = Map::new();
    let fields = match a.get("brief") {
        Some(Value::Object(o)) => o,
        None | Some(Value::Null) => &empty,
        Some(_) => return Err("project_new: `brief` must be an object (section → text)".into()),
    };
    let (d, brief) = project::create(b, client, name, fields, a_str(a, "script"))?;
    let ecproj = d.join(format!("{}.ecproj", name.trim()));
    Ok(json!({
        "client": client.trim(), "project": name.trim(), "folder": d,
        "brief": d.join(project::BRIEF), "script": d.join(project::SCRIPT),
        "dropFilesIn": {"assets": d.join("assets"), "references": d.join("references"), "moodboard": d.join("moodboard")},
        "saveProjectAs": ecproj, "renders": d.join("out"),
        "missing": project::missing(&d, &brief),
    }))
}

fn project_brief(p: &Proxy, a: &Value) -> Result<Vec<Value>, String> {
    let (client, name, d) = project_target(p, a)?;
    let brief = project::Brief::parse(&project::read(&d.join(project::BRIEF))?);
    let script = project::read(&d.join(project::SCRIPT)).unwrap_or_default();
    let sections: Map<String, Value> = brief.sections.iter().map(|(n, b)| (n.clone(), json!(b))).collect();
    let files = project::inventory(&d);
    let out = json!({
        "client": client, "project": name, "folder": d,
        "brief": sections, "script": script, "missing": project::missing(&d, &brief),
        "files": files, "referenceNotes": project::notes(&d, 2000),
    });
    let mut content = vec![text_content(&out)];
    if a.get("images").and_then(Value::as_bool).unwrap_or(true) {
        let paths = project::images(&d, &["moodboard", "references"], 24);
        let cells: Vec<draw::Cell> = paths
            .iter()
            .filter_map(|path| {
                let img = image::open(path).ok()?.thumbnail(320, 320).to_rgba8();
                let folder = path.parent().and_then(Path::file_name).map(|f| f.to_string_lossy().to_uppercase()).unwrap_or_default();
                let file: String = path.file_stem().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default().chars().take(16).collect();
                Some(draw::Cell { image: img, caption: format!("{}/{}", folder.chars().take(4).collect::<String>(), file) })
            })
            .collect();
        if !cells.is_empty() {
            let sheet = draw::filmstrip(&cells, 5, false, &format!("MOODBOARD + REFERENCES  {}", cells.len()));
            content.push(image_content(&sheet)?);
        }
    }
    Ok(content)
}

fn brief_update(p: &Proxy, a: &Value) -> Result<Value, String> {
    let (client, name, d) = project_target(p, a)?;
    let section = a_str(a, "section").ok_or("brief_update: `section` must be a string")?;
    let content = a_str(a, "content").ok_or("brief_update: `content` must be a string")?;
    let append = a.get("append").and_then(Value::as_bool).unwrap_or(false);
    if project::fold(section) == "scriptfile" {
        let path = d.join(project::SCRIPT);
        let text =
            if append { format!("{}\n{}\n", project::read(&path).unwrap_or_default().trim_end(), content.trim()) } else { format!("{}\n", content.trim()) };
        project::write(&path, &text)?;
        return Ok(json!({"client": client, "project": name, "updated": path}));
    }
    let path = d.join(project::BRIEF);
    let mut brief = project::Brief::parse(&project::read(&path)?);
    let canonical = brief.set(section, content, append);
    project::write(&path, &brief.render())?;
    Ok(json!({"client": client, "project": name, "section": canonical, "missing": project::missing(&d, &brief)}))
}

fn project_list(p: &Proxy, a: &Value) -> Result<Value, String> {
    let b = brands(p)?;
    let client = a_str(a, "client").map(brand::check_client).transpose()?;
    Ok(json!({"projects": project::list(b, client)}))
}

pub const BRAND_URI: &str = "mymotion://brand";

pub fn resources(p: &Proxy) -> Vec<Value> {
    if p.brands.is_none() {
        return vec![];
    }
    vec![
        json!({"uri": BRAND_URI, "name": "brand", "title": "Brand kit", "mimeType": "application/json", "description": "Colours, fonts, logo, safe areas and timing rules to follow."}),
    ]
}

/// `resources/read` for our URIs; `None` passes the request to EffectCraft.
pub fn read_resource(p: &Proxy, uri: &str) -> Option<Value> {
    if uri != BRAND_URI {
        return None;
    }
    let text = match brand_kit(p, None) {
        Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_default(),
        Err(e) => json!({"error": e}).to_string(),
    };
    Some(json!({"contents": [{"uri": BRAND_URI, "mimeType": "application/json", "text": text}]}))
}

// ---- 3D renders ----

/// The first frame and frame count of a render given as a frame, a `name_[0001-0090].exr` pattern or a folder.
fn render_frames(path: &str) -> Result<(PathBuf, usize), String> {
    let p = PathBuf::from(path);
    let first = if p.is_dir() {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&p)
            .map_err(|e| format!("cannot read {}: {e}", p.display()))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|f| f.is_file() && matches!(project::kind(f), "exr" | "image"))
            .collect();
        files.sort();
        files.into_iter().next().ok_or_else(|| format!("no image in {}", p.display()))?
    } else if let (Some(open), Some(close)) = (path.find('['), path.rfind(']')) {
        let range = path.get(open + 1..close).unwrap_or_default();
        let start = range.split('-').next().unwrap_or_default();
        PathBuf::from(format!("{}{start}{}", path.get(..open).unwrap_or_default(), path.get(close + 1..).unwrap_or_default()))
    } else {
        p
    };
    if !first.is_file() {
        return Err(format!("render frame not found: {}", first.display()));
    }
    // Frames of the same sequence: same name with only the trailing digits changing.
    let stem_of = |f: &Path| f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = stem_of(&first);
    let head = stem.trim_end_matches(|c: char| c.is_ascii_digit()).to_string();
    let ext = first.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
    let dir = first.parent().map(Path::to_path_buf).unwrap_or_default();
    let count = std::fs::read_dir(&dir)
        .map(|it| {
            it.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|n| {
                    let st = stem_of(n);
                    n.extension().is_some_and(|x| x.to_string_lossy() == ext.as_str())
                        && st.len() > head.len()
                        && st.starts_with(&head)
                        && st.chars().skip(head.chars().count()).all(|c| c.is_ascii_digit())
                })
                .count()
        })
        .unwrap_or(1);
    Ok((first, count.max(1)))
}

fn top_layer_id(p: &Proxy) -> Result<Value, String> {
    let c = json_of(&p.call("get_comp", json!({}))?);
    c.get("layers")
        .and_then(Value::as_array)
        .and_then(|l| l.first())
        .and_then(|l| l.get("id"))
        .cloned()
        .ok_or_else(|| "the composition has no layer".to_string())
}

fn import_render(p: &Proxy, a: &Value) -> Result<Value, String> {
    let path = a_str(a, "path").ok_or("import_render: `path` must be a string")?;
    let (first, frames) = render_frames(path)?;
    let fps = num(a, "fps").filter(|f| *f > 0.0).unwrap_or(30.0);
    let header = if project::kind(&first) == "exr" { Some(exr::read(&first)?) } else { None };
    let (w, h) = match &header {
        Some(hd) if hd.width > 0 => (hd.width, hd.height),
        _ => image::image_dimensions(&first).map_err(|e| format!("cannot read {}: {e}", first.display()))?,
    };
    let imported = json_of(&p.call("command_run", json!({"id": "file.import", "params": {"paths": [first.to_string_lossy()], "sequence": frames > 1}}))?);
    let item = imported.get("items").and_then(Value::as_array).and_then(|i| i.first()).cloned().ok_or_else(|| format!("import failed: {imported}"))?;
    if frames > 1 {
        p.call("command_run", json!({"id": "file.interpretFootage", "params": {"items": [item], "frameRate": fps}}))?;
    }
    let base = a_str(a, "name").map(str::to_string).unwrap_or_else(|| {
        let s = first.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let t = s.trim_end_matches(|c: char| c.is_ascii_digit() || c == '_' || c == '-' || c == '.').to_string();
        if t.is_empty() { "Rendu 3D".to_string() } else { t }
    });
    let comp = match a_str(a, "comp") {
        Some(c) => {
            p.call("command_run", json!({"id": "comp.open", "params": {"comp": c}}))?;
            json!(c)
        }
        None => {
            let duration = (frames as f64 / fps * 1000.0).round() / 1000.0;
            let r = json_of(&p.call(
                "command_run",
                json!({"id": "comp.new", "params": {"name": base, "width": w, "height": h, "frameRate": fps, "duration": duration, "open": true}}),
            )?);
            r.get("comp").cloned().unwrap_or(json!(base))
        }
    };
    let mut layers = Map::new();
    let pass = |suffix: &str| header.as_ref().and_then(|h| exr::find(h, suffix)).map(|p| p.layer.clone());
    let mut multiply = |label: &str, layer_name: &str, opacity: f64| -> Result<(), String> {
        p.call("command_run", json!({"id": "layer.addItem", "params": {"item": item, "index": 1}}))?;
        let id = top_layer_id(p)?;
        p.call("add_effect", json!({"layer": id, "effect": "EXtractoR", "values": {"red": format!("{layer_name}.R"), "green": format!("{layer_name}.G"), "blue": format!("{layer_name}.B"), "alpha": ""}}))?;
        p.call("command_run", json!({"id": "layer.setBlendMode", "params": {"layers": [id], "mode": "Multiply"}}))?;
        p.call("set_property", json!({"layer": id, "path": "transform/opacity", "value": opacity}))?;
        p.call("command_run", json!({"id": "layer.rename", "params": {"layer": id, "name": format!("{base} {label}")}}))?;
        layers.insert(label.to_string(), id);
        Ok(())
    };
    let shadow = num(a, "shadow").unwrap_or(85.0).clamp(0.0, 100.0);
    if shadow > 0.0
        && let Some(l) = pass("Shadow Catcher")
    {
        multiply("ombre", &l, shadow)?;
    }
    let ao = num(a, "ao").unwrap_or(0.0).clamp(0.0, 100.0);
    if ao > 0.0
        && let Some(l) = pass("Ambient Occlusion")
    {
        multiply("AO", &l, ao)?;
    }
    p.call("command_run", json!({"id": "layer.addItem", "params": {"item": item, "index": 1}}))?;
    let beauty = top_layer_id(p)?;
    p.call("command_run", json!({"id": "layer.rename", "params": {"layer": beauty, "name": format!("{base} beauty")}}))?;
    layers.insert("beauty".into(), beauty);
    let passes: Vec<Value> = header
        .as_ref()
        .map(|h| h.passes.iter().filter(|p| !p.layer.to_lowercase().contains("noisy")).map(|p| json!({"layer": p.layer, "channels": p.channels})).collect())
        .unwrap_or_default();
    Ok(json!({
        "comp": comp, "item": item, "firstFrame": first, "frames": frames, "fps": fps, "size": [w, h],
        "layers": layers, "passes": passes,
        "cryptomatte": header.as_ref().map(|h| h.cryptomatte.clone()).unwrap_or_default(),
        "hint": "Other passes: layer.addItem the same item, then add_effect EXtractoR with red/green/blue set to <layer>.Z (depth, mist) or <layer>.R/G/B.",
    }))
}

fn vectorize_logo(a: &Value) -> Result<Value, String> {
    let input = PathBuf::from(a_str(a, "input").ok_or("vectorize_logo: `input` must be a string")?);
    let img = image::open(&input).map_err(|e| format!("cannot read {}: {e}", input.display()))?.to_rgba8();
    let k = int(a, "precision", 6, 2, 12) as usize;
    let color = a_str(a, "color").unwrap_or("#000000");
    let (svg, contours, points) = vector::to_svg(&vector::coverage(&img), k, color);
    if contours == 0 {
        return Err(format!("no shape found in {} (transparent or uniform image?)", input.display()));
    }
    let output = a_str(a, "output").map(PathBuf::from).unwrap_or_else(|| {
        let stem = input.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "logo".into());
        input.with_file_name(format!("{stem}_vectorise.svg"))
    });
    project::write(&output, &svg)?;
    Ok(json!({"svg": output, "contours": contours, "points": points, "size": [img.width(), img.height()]}))
}

// ---- Blender camera ----

fn xyz(v: &Value) -> Option<[f64; 3]> {
    let a = v.as_array()?;
    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?])
}

/// Keys for a per-frame track, or one static value when it never moves.
fn track_keys(times: &[f64], values: &[[f64; 3]]) -> Option<Vec<(f64, Value)>> {
    let first = values.first()?;
    let moves = values.iter().any(|v| v.iter().zip(first).any(|(a, b)| (a - b).abs() > 1e-3));
    if !moves {
        return None;
    }
    Some(times.iter().zip(values).map(|(t, v)| (*t, json!(v))).collect())
}

fn camera_from_blender(p: &Proxy, a: &Value) -> Result<Value, String> {
    let path = PathBuf::from(a_str(a, "json").ok_or("camera_from_blender: `json` must be a string")?);
    let data: Value = serde_json::from_str(&project::read(&path)?).map_err(|e| format!("{} is not valid JSON: {e}", path.display()))?;
    let fps = data.get("fps").and_then(Value::as_f64).filter(|f| *f > 0.0).ok_or("camera JSON without `fps`")?;
    let f0 = data.get("frame_start").and_then(Value::as_f64).unwrap_or(1.0);
    let frames = data.get("frames").and_then(Value::as_array).filter(|f| !f.is_empty()).ok_or("camera JSON without `frames`")?;
    let times: Vec<f64> = frames.iter().map(|f| ((f.get("f").and_then(Value::as_f64).unwrap_or(f0) - f0) / fps * 1000.0).round() / 1000.0).collect();
    let pos: Vec<[f64; 3]> = frames.iter().filter_map(|f| f.get("position").and_then(xyz)).collect();
    let poi: Vec<[f64; 3]> = frames.iter().filter_map(|f| f.get("poi").and_then(xyz)).collect();
    let zoom: Vec<f64> = frames.iter().filter_map(|f| f.get("zoom").and_then(Value::as_f64)).collect();
    let roll: Vec<f64> = frames.iter().map(|f| f.get("roll").and_then(Value::as_f64).unwrap_or(0.0)).collect();
    if pos.len() != frames.len() || poi.len() != frames.len() || zoom.len() != frames.len() {
        return Err("camera JSON: every frame needs position, poi and zoom".into());
    }
    if let Some(c) = a_str(a, "comp") {
        p.call("command_run", json!({"id": "comp.open", "params": {"comp": c}}))?;
    }
    let name = a_str(a, "name").unwrap_or("Camera Blender");
    let made = json_of(&p.call(
        "command_run",
        json!({"id": "layer.newCamera", "params": {"name": name, "type": "twoNode", "zoom": zoom[0], "position": pos[0], "poi": poi[0]}}),
    )?);
    let cam = made.get("layer").cloned().unwrap_or(json!(name));
    let add = |layer: &Value, path: &str, keys: Vec<(f64, Value)>| -> Result<(), String> {
        let keys: Vec<Value> = keys.into_iter().map(|(t, v)| json!({"time": t, "value": v})).collect();
        p.call("add_keyframe", json!({"layer": layer, "path": path, "keys": keys, "interpolation": "linear"}))?;
        Ok(())
    };
    if let Some(k) = track_keys(&times, &pos) {
        add(&cam, "transform/position", k)?;
    }
    if let Some(k) = track_keys(&times, &poi) {
        add(&cam, "transform/poi", k)?;
    }
    if zoom.iter().any(|z| (z - zoom[0]).abs() > 0.5) {
        add(&cam, "cameraOptions/zoom", times.iter().zip(&zoom).map(|(t, z)| (*t, json!(z))).collect())?;
    }
    // Blender roll is counter-clockwise seen from behind the camera; After Effects' Z rotation is clockwise on screen.
    if roll.iter().any(|r| r.abs() > 0.01) {
        add(&cam, "transform/rotation", times.iter().zip(&roll).map(|(t, r)| (*t, json!(-r))).collect())?;
    }
    // Markers: 3D nulls that follow the Blender objects, and fixed points.
    let mut nulls = Map::new();
    let mut make_null = |label: &str, values: &[[f64; 3]]| -> Result<(), String> {
        let Some(first) = values.first() else { return Ok(()) };
        let made = json_of(&p.call("command_run", json!({"id": "layer.newNull", "params": {"name": format!("3D {label}")}}))?);
        let id = made.get("layer").cloned().unwrap_or(json!(format!("3D {label}")));
        p.call("command_run", json!({"id": "layer.setSwitch", "params": {"layers": [id], "switch": "threeD", "value": true}}))?;
        p.call("set_property", json!({"layer": id, "path": "transform/position", "value": first}))?;
        if let Some(k) = track_keys(&times, values) {
            add(&id, "transform/position", k)?;
        }
        nulls.insert(label.to_string(), json!({"layer": id, "start": first, "end": values.last()}));
        Ok(())
    };
    if let Some(objs) = data.get("objects").and_then(Value::as_object) {
        for (n, v) in objs {
            let vals: Vec<[f64; 3]> = v.as_array().into_iter().flatten().filter_map(xyz).collect();
            make_null(n, &vals)?;
        }
    }
    if let Some(pts) = data.get("points").and_then(Value::as_object) {
        for (n, v) in pts {
            if let Some(pt) = xyz(v) {
                make_null(n, &[pt])?;
            }
        }
    }
    Ok(json!({
        "camera": cam, "frames": frames.len(), "fps": fps, "zoom": zoom[0], "scale": data.get("scale"),
        "size": [data.get("width"), data.get("height")], "markers": nulls,
        "hint": "To put a title in the scene: layer.newText, layer.setSwitch threeD, then either set transform/position to a marker's coordinates or layer.setParent {layers, parent: <marker null>} and set its position to [0,0,0] relative offset. 1 Blender metre = `scale` px at depth 0; keep the comp's time 0 = Blender frame_start.",
    }))
}

// ---- VFX on footage ----

/// Corners (top-left, top-right, bottom-left, bottom-right) of the largest chroma area of `img`
/// ("green" or "blue"), sampled every other pixel; `None` when too few pixels match.
fn chroma_quad(img: &image::RgbaImage, color: &str) -> Option<[[f64; 2]; 4]> {
    let blue = color.eq_ignore_ascii_case("blue");
    let mut pts: Vec<(i64, i64)> = vec![];
    for y in (0..img.height()).step_by(2) {
        for x in (0..img.width()).step_by(2) {
            let p = img.get_pixel(x, y);
            let (r, g, b) = (i64::from(p[0]), i64::from(p[1]), i64::from(p[2]));
            let hit = if blue { b > r + 45 && b > g + 30 } else { g > r + 45 && g > b + 45 };
            if hit {
                pts.push((i64::from(x), i64::from(y)));
            }
        }
    }
    if pts.len() < 200 {
        return None;
    }
    let pick = |f: &dyn Fn(&(i64, i64)) -> i64, max: bool| -> [f64; 2] {
        let p = if max { pts.iter().max_by_key(|p| f(p)) } else { pts.iter().min_by_key(|p| f(p)) };
        p.map(|p| [p.0 as f64, p.1 as f64]).unwrap_or([0.0, 0.0])
    };
    Some([pick(&|p| p.0 + p.1, false), pick(&|p| p.0 - p.1, true), pick(&|p| p.0 - p.1, false), pick(&|p| p.0 + p.1, true)])
}

/// Whether the layer is placed 1:1 on the comp (so comp pixels = layer pixels for the tracker).
fn layer_is_identity(p: &Proxy, layer: &Value, w: f64, h: f64) -> Result<bool, String> {
    let get = |path: &str| -> Result<Vec<f64>, String> {
        let r = json_of(&p.call("get_property", json!({"layer": layer, "path": path}))?);
        Ok(numbers(r.get("value").unwrap_or(&Value::Null)).unwrap_or_default())
    };
    let (pos, scale, anchor) = (get("transform/position")?, get("transform/scale")?, get("transform/anchor")?);
    let near = |a: Option<&f64>, b: f64| a.is_some_and(|a| (a - b).abs() < 0.5);
    Ok(near(scale.first(), 100.0)
        && near(scale.get(1), 100.0)
        && near(pos.first(), w / 2.0)
        && near(pos.get(1), h / 2.0)
        && near(anchor.first(), w / 2.0)
        && near(anchor.get(1), h / 2.0))
}

fn replace_screen(p: &Proxy, a: &Value) -> Result<Value, String> {
    if let Some(c) = a_str(a, "comp") {
        p.call("command_run", json!({"id": "comp.open", "params": {"comp": c}}))?;
    }
    let layer = a.get("layer").cloned().ok_or("replace_screen: `layer` (the footage layer) is required")?;
    let content = a.get("content").cloned().ok_or("replace_screen: `content` (comp, footage item or layer) is required")?;
    let color = a_str(a, "color").unwrap_or("green");
    let t = num(a, "time").unwrap_or(0.0);
    let expand = num(a, "expand").unwrap_or(10.0);
    let feature = num(a, "feature").unwrap_or(56.0);
    let search = num(a, "search").unwrap_or(150.0);
    let comp = json_of(&p.call("get_comp", json!({}))?);
    let (w, h) = (comp.get("width").and_then(Value::as_f64).unwrap_or(0.0), comp.get("height").and_then(Value::as_f64).unwrap_or(0.0));
    if !layer_is_identity(p, &layer, w, h)? {
        return Err("replace_screen: the footage layer must fill the comp 1:1 (scale 100 %, centred, comp size = footage size). Make a comp from the footage (its 'plan' comp), replace the screen there, then nest it in the delivery comp.".into());
    }
    // Find the screen on the reference frame (full resolution).
    let shot = image_of(&p.call("render_frame", json!({"time": t, "max_side": w.max(h) as i64}))?)?;
    let q = chroma_quad(&shot, color).ok_or_else(|| format!("replace_screen: no {color} screen found at {t} s"))?;
    // The content goes under the footage; add it when it is an item rather than a layer of this comp.
    let names: Vec<String> =
        comp.get("layers").and_then(Value::as_array).into_iter().flatten().filter_map(|l| l.get("name").and_then(Value::as_str).map(str::to_string)).collect();
    let content_layer = match content.as_str() {
        Some(n) if names.iter().any(|x| x == n) => json!(n),
        _ => {
            p.call("command_run", json!({"id": "layer.addItem", "params": {"item": content, "index": 1}}))?;
            top_layer_id(p)?
        }
    };
    p.call("command_run", json!({"id": "layer.arrange", "params": {"layers": [content_layer], "to": "back"}}))?;
    let tracker = json_of(&p.call("command_run", json!({"id": "track.new", "params": {"layer": layer, "kind": "perspective", "target": content_layer}}))?);
    let offsets = [[-expand, -expand], [expand, -expand], [-expand, expand], [expand, expand]];
    for (i, (c, off)) in q.iter().zip(offsets).enumerate() {
        p.call(
            "command_run",
            json!({"id": "track.setPoint", "params": {"layer": layer, "point": i + 1, "center": c, "featureSize": [feature, feature], "searchSize": [search, search], "attachOffset": off, "time": t}}),
        )?;
    }
    let mut analysed = json!(null);
    for dir in ["forward", "backward"] {
        if dir == "backward" && t <= 0.0 {
            continue;
        }
        analysed = json_of(&p.call("command_run", json!({"id": "track.analyze", "params": {"layer": layer, "direction": dir, "wait": true}}))?);
    }
    p.call("command_run", json!({"id": "track.apply", "params": {"layer": layer}}))?;
    // Key the screen colour out of the footage, sampled at the screen's centre.
    let fx = json_of(&p.call("add_effect", json!({"layer": layer, "effect": "Key Light", "values": {}}))?);
    let idx = fx.get("index").cloned().unwrap_or(json!(1));
    let centre = [(q[0][0] + q[3][0]) / 2.0, (q[0][1] + q[3][1]) / 2.0];
    p.call("command_run", json!({"id": "effect.pickColor", "params": {"layer": layer, "effect": idx, "param": "screenColour", "x": centre[0], "y": centre[1], "average": true, "time": t}}))?;
    for (path, v) in [("screenMatte/clipBlack", 12.0), ("screenMatte/clipWhite", 88.0), ("screenMatte/screenShrinkGrow", -0.5)] {
        p.call("set_property", json!({"layer": layer, "path": format!("effects/#{idx}/{path}"), "value": v}))?;
    }
    Ok(json!({
        "screen": {"topLeft": q[0], "topRight": q[1], "bottomLeft": q[2], "bottomRight": q[3], "time": t},
        "tracker": tracker, "analysis": analysed, "content": content_layer, "keyEffect": idx,
        "hint": "Check with render_before_after {layers: [<footage layer>]} and render_filmstrip; widen `expand` if a coloured rim shows, lower it if content spills past the bezel.",
    }))
}

fn render_before_after(p: &Proxy, a: &Value) -> Result<Vec<Value>, String> {
    let info = comp_info(p, a)?;
    if a.get("comp").is_some() {
        p.call("command_run", json!({"id": "comp.open", "params": {"comp": info.id}}))?;
    }
    let layers = a
        .get("layers")
        .and_then(Value::as_array)
        .cloned()
        .filter(|l| !l.is_empty())
        .ok_or("render_before_after: `layers` (whose effects make the VFX) is required")?;
    let times: Vec<f64> = match a.get("times").and_then(Value::as_array) {
        Some(t) => t.iter().filter_map(Value::as_f64).collect(),
        None => (0..3).map(|i| snap(info.work.0 + (info.work.1 - info.work.0) * (0.15 + 0.35 * f64::from(i)), info.fps)).collect(),
    };
    let max_side = int(a, "max_side", 420, 96, 1200);
    let mut before = vec![];
    let mut after = vec![];
    let fx = |on: bool| p.call("command_run", json!({"id": "layer.setSwitch", "params": {"layers": layers, "switch": "fx", "value": on}}));
    for &t in &times {
        fx(false)?;
        let b = image_of(&p.call("render_frame", json!({"comp": info.id, "time": t, "max_side": max_side}))?);
        fx(true)?;
        let b = b?;
        let c = image_of(&p.call("render_frame", json!({"comp": info.id, "time": t, "max_side": max_side}))?)?;
        before.push(draw::Cell { image: b, caption: format!("AVANT {t:.2}S") });
        after.push(draw::Cell { image: c, caption: format!("APRES {t:.2}S") });
    }
    let n = times.len() as u32;
    before.extend(after);
    let sheet = draw::filmstrip(&before, n.max(1), false, &format!("{} AVANT / APRES", info.name));
    Ok(vec![text_content(&json!({"comp": info.id, "times": times, "layers": layers})), image_content(&sheet)?])
}

// ---- camera to Blender ----

type V3 = [f64; 3];

fn sub3(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot3(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross3(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm3(a: V3) -> V3 {
    let l = dot3(a, a).sqrt();
    if l < 1e-12 { a } else { [a[0] / l, a[1] / l, a[2] / l] }
}
fn mat_mul(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    r
}
fn rot(axis: usize, deg: f64) -> [[f64; 3]; 3] {
    let (s, c) = deg.to_radians().sin_cos();
    match axis {
        0 => [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]],
        1 => [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]],
        _ => [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
    }
}
fn col(m: &[[f64; 3]; 3], j: usize) -> V3 {
    [m[0][j], m[1][j], m[2][j]]
}

/// EffectCraft (After Effects) camera basis in world space: right, down, forward. One-node cameras:
/// orientation (X, then Y, then Z) × Z × Y × X rotations, looking along local +z (crates/geom
/// `layer_3d`); two-node cameras look at their point of interest with up = −y.
fn ae_camera_basis(orientation: V3, rot_xyz: V3, poi: Option<V3>, pos: V3) -> (V3, V3, V3) {
    if let Some(poi) = poi {
        let f = norm3(sub3(poi, pos));
        let up = if cross3(f, [0.0, -1.0, 0.0]).iter().all(|v| v.abs() < 1e-9) { [0.0, 0.0, 1.0] } else { [0.0, -1.0, 0.0] };
        let r = norm3(cross3(up, f));
        let u = cross3(f, r);
        let (x, y) = ([-r[0], -r[1], -r[2]], [-u[0], -u[1], -u[2]]);
        // Two-node roll is the Z rotation about the view axis.
        let (s, c) = (-rot_xyz[2]).to_radians().sin_cos();
        let xr = [x[0] * c + y[0] * s, x[1] * c + y[1] * s, x[2] * c + y[2] * s];
        let yr = [y[0] * c - x[0] * s, y[1] * c - x[1] * s, y[2] * c - x[2] * s];
        return (xr, yr, f);
    }
    let o = mat_mul(mat_mul(rot(2, orientation[2]), rot(1, orientation[1])), rot(0, orientation[0]));
    let m = mat_mul(mat_mul(mat_mul(o, rot(2, rot_xyz[2])), rot(1, rot_xyz[1])), rot(0, rot_xyz[0]));
    (col(&m, 0), col(&m, 1), col(&m, 2))
}

/// One property of a layer over time: its keys `(time, value)`, or one static value at time 0.
fn prop_track(p: &Proxy, layer: &Value, path: &str) -> Result<Vec<(f64, Vec<f64>)>, String> {
    let r = json_of(&p.call("get_property", json!({"layer": layer, "path": path, "time": 0}))?);
    let keys: Vec<(f64, Vec<f64>)> = r
        .get("keys")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|k| Some((k.get("compTime").or_else(|| k.get("time"))?.as_f64()?, numbers(k.get("value")?)?)))
        .collect();
    if keys.is_empty() {
        let v = numbers(r.get("value").unwrap_or(&Value::Null)).ok_or_else(|| format!("{path} is not numeric"))?;
        return Ok(vec![(0.0, v)]);
    }
    Ok(keys)
}

/// Value of a sampled track at `t` (nearest earlier key; tracks from the 3D tracker are keyed per frame).
fn at(track: &[(f64, Vec<f64>)], t: f64) -> Vec<f64> {
    track.iter().rev().find(|(kt, _)| *kt <= t + 1e-6).or(track.first()).map(|(_, v)| v.clone()).unwrap_or_default()
}

fn v3(v: &[f64]) -> V3 {
    [v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0), v.get(2).copied().unwrap_or(0.0)]
}

fn camera_to_blender(p: &Proxy, a: &Value) -> Result<Value, String> {
    if let Some(c) = a_str(a, "comp") {
        p.call("command_run", json!({"id": "comp.open", "params": {"comp": c}}))?;
    }
    let info = comp_info(p, &json!({}))?;
    let camera = a.get("camera").cloned().ok_or("camera_to_blender: `camera` (the camera layer) is required")?;
    let out = PathBuf::from(a_str(a, "json").ok_or("camera_to_blender: `json` (output path) is required")?);
    let scale = num(a, "scale").filter(|s| *s > 0.0).unwrap_or(100.0);
    let pos_t = prop_track(p, &camera, "transform/position")?;
    let ori_t = prop_track(p, &camera, "transform/orientation").unwrap_or_else(|_| vec![(0.0, vec![0.0; 3])]);
    let rx_t = prop_track(p, &camera, "transform/rotationX").unwrap_or_else(|_| vec![(0.0, vec![0.0])]);
    let ry_t = prop_track(p, &camera, "transform/rotationY").unwrap_or_else(|_| vec![(0.0, vec![0.0])]);
    let rz_t = prop_track(p, &camera, "transform/rotation").unwrap_or_else(|_| vec![(0.0, vec![0.0])]);
    let zoom_t = prop_track(p, &camera, "cameraOptions/zoom")?;
    let poi_t = prop_track(p, &camera, "transform/poi").ok();
    let two_node = a.get("twoNode").and_then(Value::as_bool).unwrap_or(false);
    // Ground: origin and "up" normal (pointing towards the camera), in EffectCraft world coordinates.
    let origin = a.get("origin").and_then(xyz).unwrap_or([info.width / 2.0, info.height / 2.0, 0.0]);
    let mut up = a.get("normal").and_then(xyz).map(norm3).unwrap_or([0.0, -1.0, 0.0]);
    let cam0 = v3(&at(&pos_t, info.work.0));
    if dot3(up, sub3(cam0, origin)) < 0.0 {
        up = [-up[0], -up[1], -up[2]];
    }
    // Blender basis expressed in EffectCraft world: z_b = up, x_b = world +x made perpendicular, y_b = z × x.
    let mut xb = sub3([1.0, 0.0, 0.0], [up[0] * up[0], up[0] * up[1], up[0] * up[2]]);
    if dot3(xb, xb) < 1e-6 {
        xb = sub3([0.0, 0.0, 1.0], [up[2] * up[0], up[2] * up[1], up[2] * up[2]]);
    }
    let xb = norm3(xb);
    let yb = cross3(up, xb);
    let to_b = |v: V3| [dot3(v, xb), dot3(v, yb), dot3(v, up)];
    let n = ((info.work.1 - info.work.0) * info.fps).round().max(1.0) as usize;
    let mut frames = vec![];
    for i in 0..n {
        let t = info.work.0 + i as f64 / info.fps;
        let pos = v3(&at(&pos_t, t));
        let poi = if two_node { poi_t.as_ref().map(|pt| v3(&at(pt, t))) } else { None };
        let (r, d, f) = ae_camera_basis(
            v3(&at(&ori_t, t)),
            [at(&rx_t, t).first().copied().unwrap_or(0.0), at(&ry_t, t).first().copied().unwrap_or(0.0), at(&rz_t, t).first().copied().unwrap_or(0.0)],
            poi,
            pos,
        );
        let pb = to_b(sub3(pos, origin));
        let (rb, db, fb) = (to_b(r), to_b(d), to_b(f));
        // Blender camera: local X = right, local Y = up (= −down), local Z = −forward.
        let m = [[rb[0], -db[0], -fb[0], pb[0] / scale], [rb[1], -db[1], -fb[1], pb[1] / scale], [rb[2], -db[2], -fb[2], pb[2] / scale], [0.0, 0.0, 0.0, 1.0]];
        let zoom = at(&zoom_t, t).first().copied().unwrap_or(1.0);
        frames.push(json!({"f": i + 1, "time": t, "matrix_world": m, "lens_mm": 36.0 * zoom / info.width, "zoom": zoom}));
    }
    let data = json!({
        "source": "EffectCraft", "comp": info.name, "width": info.width, "height": info.height, "fps": info.fps,
        "frame_start": 1, "frame_end": n, "scale": scale, "sensor_width_mm": 36.0, "sensor_fit": "HORIZONTAL",
        "ground": {"origin": origin, "normal": up}, "frames": frames,
    });
    project::write(&out, &serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?)?;
    Ok(json!({"json": out, "frames": n, "fps": info.fps, "size": [info.width, info.height], "ground": {"origin": origin, "normal": up},
        "hint": "In Blender: import_camera(json) from mymotion_blender.py, then build the 3D on the ground plane z = 0 around the origin (1 m = `scale` px), render with film transparent + shadow catcher, and import_render the result over the footage."}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_are_checked() {
        assert!(check_args("render_filmstrip", &json!({})).is_ok());
        assert!(check_args("render_filmstrip", &Value::Null).is_ok());
        assert!(check_args("render_filmstrip", &json!({"frame": 3})).is_err_and(|e| e.contains("frames")));
        assert!(check_args("plot_curves", &json!({"layer": 1})).is_err_and(|e| e.contains("path")));
        assert!(check_args("plot_curves", &json!([])).is_err());
        assert!(check_args("nope", &json!({})).is_err());
    }

    #[test]
    fn definitions_are_complete() {
        let defs = definitions();
        assert_eq!(defs.len(), DEFS.len());
        assert!(defs.iter().all(|d| d["inputSchema"]["additionalProperties"] == json!(false)));
        assert!(is_ours("plot_curves") && !is_ours("render_frame"));
    }

    #[test]
    fn camera_basis() {
        let (r, d, f) = ae_camera_basis([0.0; 3], [0.0; 3], None, [0.0; 3]);
        assert!((r[0] - 1.0).abs() < 1e-9 && (d[1] - 1.0).abs() < 1e-9 && (f[2] - 1.0).abs() < 1e-9);
        // Pan 90° about Y: forward turns to +x.
        let (_, _, f) = ae_camera_basis([0.0, 90.0, 0.0], [0.0; 3], None, [0.0; 3]);
        assert!((f[0] - 1.0).abs() < 1e-9);
        // Two-node looking down +z from the default position matches the one-node basis.
        let (r2, d2, f2) = ae_camera_basis([0.0; 3], [0.0; 3], Some([0.0, 0.0, 10.0]), [0.0; 3]);
        assert!((r2[0] - 1.0).abs() < 1e-9 && (d2[1] - 1.0).abs() < 1e-9 && (f2[2] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn numeric_values() {
        assert_eq!(numbers(&json!(2.5)), Some(vec![2.5]));
        assert_eq!(numbers(&json!([1, 2, 3])), Some(vec![1.0, 2.0, 3.0]));
        assert_eq!(numbers(&json!("text")), None);
        assert_eq!(numbers(&json!([])), None);
        assert_eq!(snap(0.51, 30.0), 0.5);
    }
}
