//! Project folders with a brief: what the user wants, filled in when the project is created.
//!
//! ```text
//! <root>/clients/<client>/projets/<project>/
//!   BRIEF.md     infos, goal, audience, style, constraints, deliverables (## sections)
//!   SCRIPT.md    scene by scene: time, on-screen text, voice-over
//!   assets/      logos, photos, videos, music, voice-over, fonts supplied
//!   references/  examples to follow (images, videos, links.txt)
//!   moodboard/   mood images
//!   out/         renders
//! ```

use std::path::{Component, Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::brand::{self, Brands};

pub const PROJECTS: &str = "projets";
pub const BRIEF: &str = "BRIEF.md";
pub const SCRIPT: &str = "SCRIPT.md";
pub const FOLDERS: [&str; 7] = ["assets", "references", "moodboard", "footage", "3d", "renders", "out"];

/// The brief's sections, in order, with the hint shown under each empty one.
pub const SECTIONS: &[(&str, &str)] = &[
    ("Infos", "- Client :\n- Projet :\n- Date de livraison :\n- Durée :\n- Formats (ratio, résolution) :\n- Plateformes de diffusion :\n- Langue(s) :"),
    ("Objectif et message clé", "_Ce que le public doit retenir, en une ou deux phrases._"),
    ("Public cible", "_Qui regarde, où, dans quel contexte._"),
    ("Script", "_Résumé ici ; le découpage détaillé (temps, texte à l'écran, voix off) va dans SCRIPT.md._"),
    ("Assets fournis", "_Déposer les fichiers dans assets/ (logos, photos, vidéos, musique, voix off, polices) et préciser ici ce qui est obligatoire._"),
    ("Références et exemples", "_Déposer images/vidéos dans references/ ; coller les liens ici, avec ce qui plaît dans chacun._"),
    ("Mood board", "_Images d'ambiance dans moodboard/ ; mots-clés d'ambiance ici._"),
    (
        "Style",
        "- Direction artistique :\n- Couleurs propres au projet :\n- Typographie :\n- Rythme / énergie :\n- Transitions :\n- Niveau d'effets :\n- Musique / son :",
    ),
    (
        "3D et VFX",
        "- Modèles / logos à mettre en 3D :
- Plans tournés à truquer (footage/) :
- Simulations (particules, fumée, fluides…) :
- Moteur et qualité de rendu (EEVEE / Cycles, échantillons) :",
    ),
    ("Contraintes et mentions obligatoires", "_Mentions légales, logos partenaires, à éviter absolument._"),
    ("Livrables", "- MP4 (réseaux) :\n- WebM transparent :\n- GIF :\n- Autres :"),
    ("Notes de suivi", "_Retours, versions, décisions._"),
];

const SCRIPT_TEMPLATE: &str = "# Script\n\n| Scène | Temps | Texte à l'écran | Voix off / son | Visuel |\n|---|---|---|---|---|\n| 1 | 0 – 3 s |  |  |  |\n";

/// Lowercase ASCII letters and digits only, accents folded, for matching section names.
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        })
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

/// The section of `SECTIONS` that `name` designates (exact, prefix or any-word match).
pub fn section_name(name: &str) -> Option<&'static str> {
    let f = fold(name);
    if f.is_empty() {
        return None;
    }
    let names = SECTIONS.iter().map(|(n, _)| *n);
    names
        .clone()
        .find(|n| fold(n) == f)
        .or_else(|| names.clone().find(|n| fold(n).starts_with(&f)))
        .or_else(|| names.clone().find(|n| n.split_whitespace().filter(|w| w.chars().count() > 3).any(|w| fold(w).starts_with(&f) || f.starts_with(&fold(w)))))
}

/// A project folder name.
pub fn check_name(name: &str) -> Result<&str, String> {
    brand::check_client(name).map_err(|_| format!("invalid project name `{name}`: use letters, digits, spaces, - or _ (it is a folder name)"))
}

/// `# title` then `## section` blocks.
pub struct Brief {
    pub title: String,
    pub sections: Vec<(String, String)>,
}

impl Brief {
    pub fn template(client: &str, project: &str) -> Self {
        let mut sections: Vec<(String, String)> = SECTIONS.iter().map(|(n, hint)| (n.to_string(), hint.to_string())).collect();
        if let Some(info) = sections.first_mut() {
            info.1 = info.1.replace("- Client :", &format!("- Client : {client}")).replace("- Projet :", &format!("- Projet : {project}"));
        }
        Brief { title: format!("Brief — {project}"), sections }
    }

    pub fn parse(text: &str) -> Self {
        let mut title = String::new();
        let mut sections: Vec<(String, String)> = vec![];
        for line in text.lines() {
            if let Some(h) = line.strip_prefix("## ") {
                sections.push((h.trim().to_string(), String::new()));
            } else if let Some(t) = line.strip_prefix("# ").filter(|_| sections.is_empty() && title.is_empty()) {
                title = t.trim().to_string();
            } else if let Some((_, body)) = sections.last_mut() {
                body.push_str(line);
                body.push('\n');
            }
        }
        for (_, b) in &mut sections {
            *b = b.trim().to_string();
        }
        Brief { title, sections }
    }

    pub fn render(&self) -> String {
        let mut out = format!("# {}\n", self.title);
        for (n, b) in &self.sections {
            out.push_str(&format!("\n## {n}\n\n{b}\n"));
        }
        out
    }

    /// Replace (or append to) a section; unknown names are added at the end.
    pub fn set(&mut self, name: &str, content: &str, append: bool) -> String {
        let canonical = section_name(name).map(str::to_string).unwrap_or_else(|| name.trim().to_string());
        let content = content.trim();
        match self.sections.iter_mut().find(|(n, _)| fold(n) == fold(&canonical)) {
            Some((_, body)) if append && !is_placeholder(body) => {
                body.push_str("\n\n");
                body.push_str(content);
            }
            Some((_, body)) => *body = content.to_string(),
            None => self.sections.push((canonical.clone(), content.to_string())),
        }
        canonical
    }

    /// Sections still empty or holding only the template hint.
    pub fn missing(&self) -> Vec<String> {
        self.sections.iter().filter(|(n, b)| is_placeholder(b) && fold(n) != fold("Notes de suivi")).map(|(n, _)| n.clone()).collect()
    }
}

/// Empty, an `_italic hint_`, or a list of `- label :` lines with nothing after the colon.
fn is_placeholder(body: &str) -> bool {
    let t = body.trim();
    t.is_empty()
        || (t.starts_with('_') && t.ends_with('_') && !t.contains('\n'))
        || t.lines().all(|l| {
            let l = l.trim();
            l.is_empty() || (l.starts_with("- ") && l.ends_with(':'))
        })
}

/// What the project still lacks: empty brief sections, except those whose material is already in
/// the folder (files in assets/, references/, moodboard/; a SCRIPT.md beyond the template).
pub fn missing(dir: &Path, brief: &Brief) -> Vec<String> {
    let has_files = |f: &str| std::fs::read_dir(dir.join(f)).map(|mut e| e.any(|x| x.is_ok_and(|x| x.path().is_file()))).unwrap_or(false);
    let script_filled = std::fs::read_to_string(dir.join(SCRIPT)).is_ok_and(|s| s.trim() != SCRIPT_TEMPLATE.trim() && !s.trim().is_empty());
    brief
        .missing()
        .into_iter()
        .filter(|n| match fold(n).as_str() {
            "assetsfournis" => !has_files("assets"),
            "referencesetexemples" => !has_files("references"),
            "moodboard" => !has_files("moodboard"),
            "3detvfx" => !(has_files("3d") || has_files("footage") || has_files("renders")),
            "script" => !script_filled,
            _ => true,
        })
        .collect()
}

pub fn dir(b: &Brands, client: &str, project: &str) -> Result<PathBuf, String> {
    Ok(b.client_dir(brand::check_client(client)?).join(PROJECTS).join(check_name(project)?))
}

/// The project folder holding `file` (an open .ecproj): `<root>/clients/<c>/projets/<p>/…`.
pub fn of_file(b: &Brands, file: &Path) -> Option<(String, String, PathBuf)> {
    let root = std::fs::canonicalize(&b.root).ok()?;
    let parent = std::fs::canonicalize(file.parent()?).ok()?;
    let rel = parent.strip_prefix(&root).ok()?;
    let parts: Vec<String> =
        rel.components().filter_map(|c| if let Component::Normal(p) = c { Some(p.to_string_lossy().into_owned()) } else { None }).collect();
    match parts.as_slice() {
        [c, client, p, project, ..] if c == brand::CLIENTS && p == PROJECTS => {
            let d = b.client_dir(client).join(PROJECTS).join(project);
            d.join(BRIEF).is_file().then(|| (client.clone(), project.clone(), d))
        }
        _ => None,
    }
}

/// Create the folder, its sub-folders, BRIEF.md (with `fields` filled in) and SCRIPT.md.
pub fn create(b: &Brands, client: &str, project: &str, fields: &Map<String, Value>, script: Option<&str>) -> Result<(PathBuf, Brief), String> {
    let client = brand::check_client(client)?;
    if !b.client_dir(client).join(brand::FILE).is_file() {
        return Err(format!("client `{client}` has no brand kit yet: create it first with brand_new"));
    }
    let d = dir(b, client, project)?;
    if d.join(BRIEF).exists() {
        return Err(format!("project `{project}` already exists for `{client}`: {}", d.display()));
    }
    for f in FOLDERS {
        std::fs::create_dir_all(d.join(f)).map_err(|e| format!("cannot create {}: {e}", d.join(f).display()))?;
    }
    let mut brief = Brief::template(client, project.trim());
    for (k, v) in fields {
        let text = match v {
            Value::String(s) => s.clone(),
            Value::Array(items) => {
                items.iter().map(|i| format!("- {}", i.as_str().map(str::to_string).unwrap_or_else(|| i.to_string()))).collect::<Vec<_>>().join("\n")
            }
            Value::Null => continue,
            other => other.to_string(),
        };
        brief.set(k, &text, false);
    }
    write(&d.join(BRIEF), &brief.render())?;
    let script = match script {
        Some(s) if !s.trim().is_empty() => format!("# Script\n\n{}\n", s.trim()),
        _ => SCRIPT_TEMPLATE.to_string(),
    };
    write(&d.join(SCRIPT), &script)?;
    Ok((d, brief))
}

pub fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

pub fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

pub fn kind(path: &Path) -> &'static str {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "bmp" | "gif" | "tif" | "tiff" | "webp" | "tga" => "image",
        "exr" => "exr",
        "blend" => "blender",
        "svg" | "ai" | "eps" | "pdf" => "vector",
        "psd" | "psb" => "photoshop",
        "mp4" | "mov" | "webm" | "mkv" | "avi" | "mxf" | "m4v" => "video",
        "wav" | "mp3" | "aac" | "m4a" | "aif" | "aiff" | "flac" | "ogg" | "opus" => "audio",
        "ttf" | "otf" | "woff" | "woff2" => "font",
        "json" | "lottie" => "lottie/json",
        "txt" | "md" | "url" | "docx" | "doc" | "rtf" => "text",
        "glb" | "gltf" | "obj" | "fbx" | "stl" => "3d",
        "ecproj" | "ecprojx" => "project",
        _ => "other",
    }
}

/// Every file under `dir` (depth-limited), skipping `out/`, as `{path, folder, kind, bytes, size?}`.
pub fn inventory(dir: &Path) -> Vec<Value> {
    fn walk(base: &Path, d: &Path, depth: u32, out: &mut Vec<Value>) {
        let Ok(entries) = std::fs::read_dir(d) else { return };
        let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let p = e.path();
            let rel = p.strip_prefix(base).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            if p.is_dir() {
                if depth < 4 && rel != "out" {
                    walk(base, &p, depth + 1, out);
                }
                continue;
            }
            let folder = rel.split('/').next().filter(|_| rel.contains('/')).unwrap_or(".").to_string();
            let k = kind(&p);
            let mut v = json!({"path": rel, "folder": folder, "kind": k, "bytes": e.metadata().map(|m| m.len()).unwrap_or(0)});
            if k == "image"
                && let Ok((w, h)) = image::image_dimensions(&p)
            {
                v["size"] = json!([w, h]);
            }
            out.push(v);
        }
    }
    let mut out = vec![];
    walk(dir, dir, 0, &mut out);
    group_sequences(out)
}

/// `name_0001.exr` … `name_0090.exr` → one `{path: "name_[0001-0090].exr", kind: "sequence", frames}`.
fn split_frame(path: &str) -> Option<(String, String, String)> {
    let (stem, ext) = path.rsplit_once('.')?;
    let digits = stem.chars().rev().take_while(char::is_ascii_digit).count();
    if digits < 3 || digits == stem.len() {
        return None;
    }
    let (head, num) = stem.split_at(stem.len() - digits);
    Some((head.to_string(), num.to_string(), ext.to_string()))
}

fn group_sequences(files: Vec<Value>) -> Vec<Value> {
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<(String, String), Vec<(String, Value)>> = BTreeMap::new();
    let mut out = vec![];
    for v in files {
        let path = v["path"].as_str().unwrap_or_default().to_string();
        match split_frame(&path) {
            Some((head, num, ext)) => groups.entry((head, ext)).or_default().push((num, v)),
            None => out.push(v),
        }
    }
    for ((head, ext), mut frames) in groups {
        if frames.len() < 3 {
            out.extend(frames.into_iter().map(|(_, v)| v));
            continue;
        }
        frames.sort_by(|a, b| a.0.cmp(&b.0));
        let (first, last) = (frames[0].0.clone(), frames[frames.len() - 1].0.clone());
        let bytes: u64 = frames.iter().map(|(_, v)| v["bytes"].as_u64().unwrap_or(0)).sum();
        let mut v = frames[0].1.clone();
        v["path"] = json!(format!("{head}[{first}-{last}].{ext}"));
        v["firstFrame"] = json!(format!("{head}{first}.{ext}"));
        v["kind"] = json!(format!("sequence ({})", v["kind"].as_str().unwrap_or("file")));
        v["frames"] = json!(frames.len());
        v["bytes"] = json!(bytes);
        out.push(v);
    }
    out.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    out
}

/// Image files in `folders` of the project, for the visual sheet.
pub fn images(dir: &Path, folders: &[&str], limit: usize) -> Vec<PathBuf> {
    let mut out = vec![];
    for f in folders {
        let Ok(entries) = std::fs::read_dir(dir.join(f)) else { continue };
        let mut files: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_file() && matches!(kind(p), "image")).collect();
        files.sort();
        out.extend(files);
    }
    out.truncate(limit);
    out
}

/// Text notes in `references/` (links, comments), each cut to `max` characters.
pub fn notes(dir: &Path, max: usize) -> Vec<Value> {
    let Ok(entries) = std::fs::read_dir(dir.join("references")) else { return vec![] };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            matches!(ext.as_str(), "txt" | "md" | "url")
        })
        .collect();
    files.sort();
    files
        .iter()
        .filter_map(|p| {
            let t = std::fs::read_to_string(p).ok()?;
            let cut: String = t.chars().take(max).collect();
            Some(json!({"file": p.file_name().map(|n| n.to_string_lossy().into_owned()), "text": cut}))
        })
        .collect()
}

/// Projects of a client (or all clients), with what their brief still lacks.
pub fn list(b: &Brands, client: Option<&str>) -> Vec<Value> {
    let clients: Vec<String> = match client {
        Some(c) => vec![c.to_string()],
        None => b.list().iter().filter_map(|v| v["client"].as_str().map(str::to_string)).collect(),
    };
    let mut out = vec![];
    for c in clients {
        let Ok(entries) = std::fs::read_dir(b.client_dir(&c).join(PROJECTS)) else { continue };
        let mut dirs: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.join(BRIEF).is_file()).collect();
        dirs.sort();
        for d in dirs {
            let brief = read(&d.join(BRIEF)).map(|t| Brief::parse(&t)).ok();
            out.push(json!({
                "client": c,
                "project": d.file_name().map(|n| n.to_string_lossy().into_owned()),
                "folder": d,
                "missing": brief.map(|b| missing(&d, &b)).unwrap_or_default(),
                "files": inventory(&d).len(),
            }));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_match_loosely() {
        assert_eq!(section_name("objectif"), Some("Objectif et message clé"));
        assert_eq!(section_name("Message"), Some("Objectif et message clé"));
        assert_eq!(section_name("moodboard"), Some("Mood board"));
        assert_eq!(section_name("références"), Some("Références et exemples"));
        assert_eq!(section_name("exemples"), Some("Références et exemples"));
        assert_eq!(section_name("STYLE"), Some("Style"));
        assert_eq!(section_name("contraintes"), Some("Contraintes et mentions obligatoires"));
        assert_eq!(section_name("zzz"), None);
        assert_eq!(fold("Clé É"), "clee");
    }

    #[test]
    fn brief_round_trip_and_missing() {
        let mut b = Brief::template("Acme", "Teaser");
        let text = b.render();
        assert!(text.contains("- Client : Acme") && text.contains("## Mood board"));
        let parsed = Brief::parse(&text);
        assert_eq!(parsed.sections.len(), SECTIONS.len());
        assert!(parsed.missing().contains(&"Style".to_string()));
        assert!(!parsed.missing().contains(&"Infos".to_string()), "client and project are filled");
        b.set("style", "Élégant, lent, photos bichromes", false);
        b.set("style", "Pas de rebonds", true);
        b.set("Budget", "serré", false);
        let p = Brief::parse(&b.render());
        assert!(!p.missing().contains(&"Style".to_string()));
        assert!(p.render().contains("Élégant, lent, photos bichromes\n\nPas de rebonds"));
        assert_eq!(p.sections.last().map(|s| s.0.as_str()), Some("Budget"));
    }

    #[test]
    fn create_list_and_locate() {
        let root = std::env::temp_dir().join(format!("mymotion-project-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::create_dir_all(&root);
        let b = Brands::new(root.join(brand::FILE));
        assert!(create(&b, "Acme", "Teaser", &Map::new(), None).is_err(), "client must exist");
        assert!(b.create("Acme", None).is_ok());
        let mut fields = Map::new();
        fields.insert("objectif".into(), json!("Annoncer le lancement"));
        fields.insert("livrables".into(), json!(["MP4 9:16", "GIF"]));
        let r = create(&b, "Acme", "Teaser", &fields, Some("| 1 | 0–3 s | Bonjour | | |"));
        assert!(r.is_ok());
        let d = root.join("clients/Acme/projets/Teaser");
        assert!(d.join("moodboard").is_dir() && d.join(SCRIPT).is_file());
        let brief = read(&d.join(BRIEF)).unwrap_or_default();
        assert!(brief.contains("Annoncer le lancement") && brief.contains("- MP4 9:16"));
        assert!(create(&b, "Acme", "Teaser", &Map::new(), None).is_err(), "no overwrite");
        let _ = std::fs::write(d.join("moodboard/a.png"), b"not really a png");
        let _ = std::fs::write(d.join("references/liens.txt"), "https://example.com super rythme");
        assert_eq!(images(&d, &["moodboard"], 10).len(), 1);
        let parsed = Brief::parse(&read(&d.join(BRIEF)).unwrap_or_default());
        let m = missing(&d, &parsed);
        assert!(!m.contains(&"Mood board".to_string()) && !m.contains(&"Références et exemples".to_string()) && !m.contains(&"Script".to_string()));
        assert!(m.contains(&"Style".to_string()));
        assert_eq!(notes(&d, 100).len(), 1);
        assert!(inventory(&d).iter().any(|v| v["path"] == "moodboard/a.png" && v["kind"] == "image"));
        let found = of_file(&b, &d.join("Teaser.ecproj"));
        assert_eq!(found.map(|f| f.1), Some("Teaser".to_string()));
        assert_eq!(list(&b, None).len(), 1);
        for i in 1..=5 {
            let _ = std::fs::write(d.join(format!("renders/logo_{i:04}.exr")), b"x");
        }
        let inv = inventory(&d);
        let seq = inv.iter().find(|v| v["path"] == "renders/logo_[0001-0005].exr");
        assert!(seq.is_some_and(|v| v["frames"] == 5 && v["firstFrame"] == "renders/logo_0001.exr"), "{inv:?}");
        assert!(check_name("../x").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
