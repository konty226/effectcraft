//! Brand kits per client and per project, merged in cascade.
//!
//! ```text
//! <root>/brand_kit.json                              studio defaults (the --brand file)
//! <root>/clients/<client>/brand_kit.json             what this client changes
//! <root>/clients/<client>/<folders…>/brand_kit.json  what a project folder changes
//! <project>.brand.json (next to the .ecproj)         what one project changes
//! ```
//!
//! Each level holds only what differs: objects merge key by key, anything else (numbers, strings,
//! arrays) replaces the value above. The client comes from the open project's location under
//! `<root>/clients/`, or from `brand_use` / the `client` argument.

use std::path::{Component, Path, PathBuf};

use serde_json::{Value, json};

pub const FILE: &str = "brand_kit.json";
pub const CLIENTS: &str = "clients";

pub struct Brands {
    /// The studio kit (`--brand`).
    pub default: PathBuf,
    /// Its folder: clients live in `<root>/clients/`.
    pub root: PathBuf,
}

/// The merged kit and where it came from.
pub struct Resolved {
    pub kit: Value,
    pub client: Option<String>,
    pub sources: Vec<PathBuf>,
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Objects merge key by key; anything else in `over` replaces `base`. Keys starting with `_`
/// (comments) are not inherited.
pub fn merge(base: &mut Value, over: Value) {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            b.retain(|k, _| !k.starts_with('_'));
            for (k, v) in o {
                match b.get_mut(&k) {
                    Some(slot) if slot.is_object() && v.is_object() => merge(slot, v),
                    _ => {
                        b.insert(k, v);
                    }
                }
            }
        }
        (b, o) => *b = o,
    }
}

/// A client name usable as a folder name.
pub fn check_client(name: &str) -> Result<&str, String> {
    let n = name.trim();
    let bad = n.is_empty()
        || n.len() > 80
        || n.starts_with('.')
        || n.ends_with('.')
        || n.chars().any(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control());
    if bad {
        return Err(format!("invalid client name `{name}`: use letters, digits, spaces, - or _ (it is a folder name)"));
    }
    Ok(n)
}

fn read(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("{} is not valid JSON: {e}", path.display()))?;
    if v.is_object() { Ok(v) } else { Err(format!("{} must hold a JSON object", path.display())) }
}

impl Brands {
    pub fn new(default: PathBuf) -> Self {
        let root = default.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
        Brands { default, root }
    }

    fn clients_dir(&self) -> PathBuf {
        self.root.join(CLIENTS)
    }

    pub fn client_dir(&self, client: &str) -> PathBuf {
        self.clients_dir().join(client)
    }

    /// Clients that have a kit, sorted by folder name.
    pub fn list(&self) -> Vec<Value> {
        let Ok(entries) = std::fs::read_dir(self.clients_dir()) else { return vec![] };
        let mut out: Vec<Value> = entries
            .filter_map(Result::ok)
            .filter(|e| e.path().join(FILE).is_file())
            .map(|e| {
                let client = e.file_name().to_string_lossy().into_owned();
                let file = e.path().join(FILE);
                let name = read(&file).ok().and_then(|v| v.get("name").and_then(Value::as_str).map(str::to_string));
                json!({"client": client, "name": name, "kit": file})
            })
            .collect();
        out.sort_by(|a, b| a["client"].as_str().cmp(&b["client"].as_str()));
        out
    }

    /// The folders from `<root>` (exclusive) down to `dir` (inclusive) when `dir` is under `<root>`.
    fn chain_below_root(&self, dir: &Path) -> Option<Vec<PathBuf>> {
        let root = canonical(&self.root);
        let rel = canonical(dir).strip_prefix(&root).ok()?.to_path_buf();
        let mut cur = root;
        let mut out = vec![];
        for c in rel.components() {
            if let Component::Normal(part) = c {
                cur = cur.join(part);
                out.push(cur.clone());
            }
        }
        Some(out)
    }

    /// The client whose folder holds `project`, if any.
    pub fn client_of(&self, project: &Path) -> Option<String> {
        let chain = self.chain_below_root(project.parent()?)?;
        let first = chain.first()?;
        if first.file_name()? != CLIENTS {
            return None;
        }
        chain.get(1)?.file_name().map(|n| n.to_string_lossy().into_owned())
    }

    /// Merge the cascade for `project` (the open .ecproj, if saved) and `client` (forced, else
    /// the project's).
    pub fn resolve(&self, project: Option<&Path>, client: Option<&str>) -> Result<Resolved, String> {
        let mut files: Vec<PathBuf> = vec![];
        if self.default.is_file() {
            files.push(self.default.clone());
        }
        let detected = project.and_then(|p| self.client_of(p));
        let client = match client {
            Some(c) => Some(check_client(c)?.to_string()),
            None => detected.clone(),
        };
        if let Some(c) = &client {
            let kit = self.client_dir(c).join(FILE);
            if !kit.is_file() {
                return Err(format!("client `{c}` has no brand kit ({}); create it with brand_new", kit.display()));
            }
            files.push(kit);
        }
        if let Some(p) = project {
            // Folder kits between the client folder and the project, then `<project>.brand.json`;
            // only when the project belongs to the client in use.
            if detected == client
                && let Some(dir) = p.parent()
                && let Some(chain) = self.chain_below_root(dir)
            {
                let skip = if client.is_some() { 2 } else { 0 };
                for d in chain.iter().skip(skip) {
                    let f = d.join(FILE);
                    if f.is_file() && !files.iter().any(|x| canonical(x) == canonical(&f)) {
                        files.push(f);
                    }
                }
            }
            let own = p.with_extension("brand.json");
            if own.is_file() {
                files.push(own);
            }
        }
        if files.is_empty() {
            return Err(format!("no brand kit: create {}", self.default.display()));
        }
        let mut kit = json!({});
        for f in &files {
            merge(&mut kit, read(f)?);
        }
        Ok(Resolved { kit, client, sources: files })
    }

    /// Create `<root>/clients/<client>/` with `projets/`, `medias/`, `out/` and a kit holding only
    /// overrides (or a copy of another client's kit).
    pub fn create(&self, client: &str, from: Option<&str>) -> Result<PathBuf, String> {
        let client = check_client(client)?;
        let dir = self.client_dir(client);
        let kit = dir.join(FILE);
        if kit.exists() {
            return Err(format!("client `{client}` already has a brand kit: {}", kit.display()));
        }
        let mut body = match from {
            Some(src) => read(&self.client_dir(check_client(src)?).join(FILE))?,
            None => json!({
                "_comment": "Only what this client changes: anything missing comes from the studio kit (brand_kit.json at the root).",
                "colors": {},
                "fonts": {},
                "logo": {"file": dir.join("medias").join("logo.svg").to_string_lossy().replace('\\', "/")}
            }),
        };
        body["name"] = json!(client);
        for sub in ["projets", "medias", "out"] {
            std::fs::create_dir_all(dir.join(sub)).map_err(|e| format!("cannot create {}: {e}", dir.join(sub).display()))?;
        }
        let text = serde_json::to_string_pretty(&body).map_err(|e| e.to_string())?;
        std::fs::write(&kit, text + "\n").map_err(|e| format!("cannot write {}: {e}", kit.display()))?;
        Ok(kit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mymotion-brand-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::create_dir_all(&d);
        d
    }

    fn write(p: &Path, v: Value) {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(p, v.to_string());
    }

    #[test]
    fn merge_is_deep_and_drops_comments() {
        let mut a = json!({"colors": {"primary": "#111", "text": "#fff"}, "chart": [1, 2], "_comment": "x"});
        merge(&mut a, json!({"colors": {"primary": "#f00"}, "chart": [3]}));
        assert_eq!(a, json!({"colors": {"primary": "#f00", "text": "#fff"}, "chart": [3]}));
    }

    #[test]
    fn client_names() {
        assert!(check_client("Acme Corp").is_ok());
        assert!(check_client("ok_1-2").is_ok());
        for bad in ["", " ", "..", "a/b", "a\\b", "c:", ".hidden", "x?"] {
            assert!(check_client(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn cascade() {
        let root = tmp("cascade");
        let brands = Brands::new(root.join(FILE));
        write(&root.join(FILE), json!({"name": "Studio", "colors": {"primary": "#000", "text": "#fff"}, "fps": 30}));
        // No project, no client: studio only.
        let r = brands.resolve(None, None).unwrap_or_else(|e| unreachable_msg(&e));
        assert_eq!(r.kit["name"], "Studio");
        assert!(r.client.is_none());

        let kit = brands.create("Acme", None).unwrap_or_else(|e| unreachable_msg(&e));
        assert!(kit.is_file() && root.join("clients/Acme/projets").is_dir());
        assert!(brands.create("Acme", None).is_err());
        write(&kit, json!({"name": "Acme", "colors": {"primary": "#f00"}}));
        write(&root.join("clients/Acme/projets/Campagne/brand_kit.json"), json!({"fps": 25}));
        let project = root.join("clients/Acme/projets/Campagne/teaser.ecproj");
        write(&project, json!({}));
        write(&project.with_extension("brand.json"), json!({"colors": {"text": "#ccc"}}));

        assert_eq!(brands.client_of(&project).as_deref(), Some("Acme"));
        let r = brands.resolve(Some(&project), None).unwrap_or_else(|e| unreachable_msg(&e));
        assert_eq!(r.client.as_deref(), Some("Acme"));
        assert_eq!(r.kit["colors"], json!({"primary": "#f00", "text": "#ccc"}));
        assert_eq!(r.kit["fps"], 25);
        assert_eq!(r.sources.len(), 4);

        // Forcing another client ignores this project's folder kits.
        brands.create("Beta", Some("Acme")).unwrap_or_else(|e| unreachable_msg(&e));
        let r = brands.resolve(Some(&project), Some("Beta")).unwrap_or_else(|e| unreachable_msg(&e));
        assert_eq!(r.kit["name"], "Beta");
        assert_eq!(r.kit["fps"], 30);
        assert!(brands.resolve(None, Some("Nobody")).is_err());
        assert_eq!(brands.list().len(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    fn unreachable_msg<T>(e: &str) -> T {
        // Tests may fail loudly.
        #[allow(clippy::panic)]
        {
            panic!("{e}")
        }
    }
}
