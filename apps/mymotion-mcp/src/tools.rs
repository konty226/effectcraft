//! The motion-design tools this proxy adds, built on EffectCraft's own MCP tools.

use serde_json::{Map, Value, json};

use crate::{Proxy, b64, draw, lock};

pub const INSTRUCTIONS: &str = "mymotion tools (motion design with Claude): render_filmstrip shows a stretch of the animation as a captioned contact sheet, so judge timing and spacing on it rather than on one frame. plot_curves draws a property's value and speed over time with its keyframes and reports overshoot and peak speed, to judge easing. focus_context returns what the user is looking at in the live app (active comp, current time, selected layers, properties and keys) plus an image, call it first when the user says \"this\" or \"the selected layer\". checkpoint {label} before a large change, restore_checkpoint {label} to go back. brand_kit (also resource mymotion://brand) returns the user's colours, fonts and timing rules: follow it.";

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
        description: "The user's brand kit (colours, fonts, logo, safe areas, preferred durations and easing) as JSON. Use its values when creating or restyling anything.",
        props: || json!({}),
        required: &[],
        read_only: true,
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
        "brand_kit" => brand_kit(p).map(|v| vec![text_content(&v)]),
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

fn brand_kit(p: &Proxy) -> Result<Value, String> {
    let path = p.brand.as_ref().ok_or("no brand kit: start mymotion-mcp with --brand FILE.json (see mymotion/brand_kit.example.json)")?;
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read the brand kit {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("brand kit {} is not valid JSON: {e}", path.display()))
}

pub const BRAND_URI: &str = "mymotion://brand";

pub fn resources(p: &Proxy) -> Vec<Value> {
    if p.brand.is_none() {
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
    let text = match brand_kit(p) {
        Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_default(),
        Err(e) => json!({"error": e}).to_string(),
    };
    Some(json!({"contents": [{"uri": BRAND_URI, "mimeType": "application/json", "text": text}]}))
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
    fn numeric_values() {
        assert_eq!(numbers(&json!(2.5)), Some(vec![2.5]));
        assert_eq!(numbers(&json!([1, 2, 3])), Some(vec![1.0, 2.0, 3.0]));
        assert_eq!(numbers(&json!("text")), None);
        assert_eq!(numbers(&json!([])), None);
        assert_eq!(snap(0.51, 30.0), 0.5);
    }
}
