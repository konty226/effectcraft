//! # mymotion-mcp
//!
//! An MCP server for motion design with Claude. It starts `effectcraft-cli mcp <args>` as a child
//! process and proxies the MCP stdio transport (JSON-RPC 2.0, one message per line) to it, so every
//! EffectCraft tool, resource, progress notification and cancellation passes through unchanged.
//! On top it adds the tools in [`tools`] (filmstrip, curve plots, focus context, checkpoints, brand
//! kit), which it implements by calling the child's own tools.
//!
//! Usage: `mymotion-mcp [--cli PATH] [--brand FILE] [effectcraft-cli mcp args…]`, for example
//! `mymotion-mcp --brand D:\MotionProjects\brand_kit.json --bridge 9877`. `--cli` defaults to
//! `effectcraft-cli` next to this executable.
//!
//! Never crash: no `unwrap`/`expect`/`panic`; a failing tool returns `isError: true`.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod b64;
mod draw;
mod tools;

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};

/// How long an internal call to the child may take (a filmstrip renders one frame per call).
const CALL_TIMEOUT: Duration = Duration::from_secs(300);
/// Ids of the requests the proxy sends itself start with this.
const OWN_ID: &str = "mymotion-";

/// What to change in the child's reply to a client request before passing it on.
#[derive(Clone, Copy)]
enum Merge {
    Initialize,
    ToolsList,
    ResourcesList,
}

pub struct Proxy {
    out: Mutex<std::io::Stdout>,
    child_in: Mutex<Option<ChildStdin>>,
    pending: Mutex<HashMap<String, Sender<Value>>>,
    merges: Mutex<HashMap<String, Merge>>,
    next: AtomicU64,
    pub brand: Option<PathBuf>,
    pub checkpoints: Mutex<Vec<tools::Checkpoint>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Proxy {
    /// Write one message to the client.
    fn reply(&self, msg: &str) {
        let mut out = lock(&self.out);
        // A closed stdout means the client is gone; nothing useful can be done about it.
        let _ = writeln!(out, "{msg}");
        let _ = out.flush();
    }

    fn to_child(&self, line: &str) -> Result<(), String> {
        let mut guard = lock(&self.child_in);
        let stdin = guard.as_mut().ok_or("the EffectCraft MCP server has stopped")?;
        writeln!(stdin, "{line}").and_then(|()| stdin.flush()).map_err(|e| format!("cannot reach the EffectCraft MCP server: {e}"))
    }

    /// Call one of the child's tools and wait for its `result` (an `isError` result is an `Err`
    /// carrying its text).
    pub fn call(&self, name: &str, arguments: Value) -> Result<Value, String> {
        let id = format!("{OWN_ID}{}", self.next.fetch_add(1, Ordering::Relaxed));
        let (tx, rx) = channel();
        lock(&self.pending).insert(id.clone(), tx);
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": arguments}});
        if let Err(e) = self.to_child(&msg.to_string()) {
            lock(&self.pending).remove(&id);
            return Err(e);
        }
        let reply = rx.recv_timeout(CALL_TIMEOUT).map_err(|_| {
            lock(&self.pending).remove(&id);
            format!("{name}: no reply from the EffectCraft MCP server")
        })?;
        if let Some(err) = reply.get("error") {
            return Err(format!("{name}: {}", err.get("message").and_then(Value::as_str).unwrap_or("error")));
        }
        let result = reply.get("result").cloned().unwrap_or(Value::Null);
        if result.get("isError").and_then(Value::as_bool) == Some(true) {
            return Err(format!("{name}: {}", tools::text_of(&result)));
        }
        Ok(result)
    }

    /// Route one line from the child: a reply to one of our own calls, a reply to merge, or
    /// anything else (passed through untouched).
    fn on_child_line(&self, line: &str) {
        let Ok(mut msg) = serde_json::from_str::<Value>(line) else {
            self.reply(line);
            return;
        };
        let key = msg.get("id").map(Value::to_string);
        if let Some(id) = msg.get("id").and_then(Value::as_str).filter(|id| id.starts_with(OWN_ID)) {
            if let Some(tx) = lock(&self.pending).remove(id) {
                let _ = tx.send(msg);
            }
            return;
        }
        let merge = key.and_then(|k| lock(&self.merges).remove(&k));
        match (merge, msg.get_mut("result")) {
            (Some(m), Some(result)) => {
                self.merge(m, result);
                self.reply(&msg.to_string());
            }
            _ => self.reply(line),
        }
    }

    fn merge(&self, m: Merge, result: &mut Value) {
        match m {
            Merge::Initialize => {
                let base = result.get("instructions").and_then(Value::as_str).unwrap_or_default().to_string();
                result["instructions"] = json!(format!("{base}\n\n{}", tools::INSTRUCTIONS));
                if let Some(caps) = result.get_mut("capabilities").and_then(Value::as_object_mut) {
                    caps.entry("resources").or_insert_with(|| json!({}));
                }
            }
            Merge::ToolsList => {
                if let Some(list) = result.get_mut("tools").and_then(Value::as_array_mut) {
                    list.extend(tools::definitions());
                }
            }
            Merge::ResourcesList => {
                if let Some(list) = result.get_mut("resources").and_then(Value::as_array_mut) {
                    list.extend(tools::resources(self));
                }
            }
        }
    }

    /// Handle one line from the client.
    fn on_client_line(self: &Arc<Self>, line: &str) {
        let Ok(msg) = serde_json::from_str::<Value>(line) else {
            self.forward(line);
            return;
        };
        let method = msg.get("method").and_then(Value::as_str).unwrap_or_default();
        let id = msg.get("id").cloned();
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        let merge = match method {
            "initialize" => Some(Merge::Initialize),
            "tools/list" => Some(Merge::ToolsList),
            "resources/list" => Some(Merge::ResourcesList),
            _ => None,
        };
        if let (Some(m), Some(id)) = (merge, &id) {
            lock(&self.merges).insert(id.to_string(), m);
        }
        let Some(id) = id else {
            self.forward(line);
            return;
        };
        if method == "resources/read" {
            let uri = params.get("uri").and_then(Value::as_str).unwrap_or_default();
            if let Some(reply) = tools::read_resource(self, uri) {
                self.reply(&rpc_result(&id, reply).to_string());
                return;
            }
        }
        if method == "tools/call" {
            let name = params.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
            if tools::is_ours(&name) {
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                let me = Arc::clone(self);
                // Off the reading thread: our tools call the child, whose replies this loop routes.
                let spawned = std::thread::Builder::new().name(format!("tool-{name}")).spawn(move || {
                    let reply = match tools::check_args(&name, &args) {
                        Err(message) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32602, "message": message}}),
                        Ok(()) => rpc_result(&id, tools::run(&me, &name, &args)),
                    };
                    me.reply(&reply.to_string());
                });
                if let Err(e) = spawned {
                    self.reply(
                        &json!({"jsonrpc": "2.0", "id": msg["id"], "error": {"code": -32603, "message": format!("cannot start tool: {e}")}}).to_string(),
                    );
                }
                return;
            }
        }
        self.forward(line);
    }

    fn forward(&self, line: &str) {
        if let Err(e) = self.to_child(line)
            && let Some(id) = serde_json::from_str::<Value>(line).ok().and_then(|m| m.get("id").cloned())
        {
            self.reply(&json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32603, "message": e}}).to_string());
        }
    }
}

fn rpc_result(id: &Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

struct Args {
    cli: PathBuf,
    brand: Option<PathBuf>,
    child: Vec<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut cli = None;
    let mut brand = None;
    let mut child = Vec::new();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--cli" => cli = Some(PathBuf::from(it.next().ok_or("--cli needs a path")?)),
            "--brand" => brand = Some(PathBuf::from(it.next().ok_or("--brand needs a file")?)),
            "-h" | "--help" => {
                return Err("usage: mymotion-mcp [--cli PATH] [--brand FILE] [effectcraft-cli mcp args, e.g. --bridge 9877]".into());
            }
            _ => child.push(a),
        }
    }
    let cli = match cli {
        Some(c) => c,
        None => {
            let exe = std::env::current_exe().map_err(|e| format!("cannot locate this executable: {e}"))?;
            let name = if cfg!(windows) { "effectcraft-cli.exe" } else { "effectcraft-cli" };
            exe.with_file_name(name)
        }
    };
    Ok(Args { cli, brand, child })
}

fn main() -> std::process::ExitCode {
    match serve() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("mymotion-mcp: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn serve() -> Result<(), String> {
    let args = parse_args()?;
    let mut child = Command::new(&args.cli)
        .arg("mcp")
        .args(&args.child)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("cannot start {}: {e}", args.cli.display()))?;
    let child_out = child.stdout.take().ok_or("no stdout from effectcraft-cli")?;
    let proxy = Arc::new(Proxy {
        out: Mutex::new(std::io::stdout()),
        child_in: Mutex::new(child.stdin.take()),
        pending: Mutex::new(HashMap::new()),
        merges: Mutex::new(HashMap::new()),
        next: AtomicU64::new(1),
        brand: args.brand,
        checkpoints: Mutex::new(Vec::new()),
    });

    let reader = {
        let proxy = Arc::clone(&proxy);
        std::thread::Builder::new()
            .name("child-out".into())
            .spawn(move || {
                for line in BufReader::new(child_out).lines() {
                    let Ok(line) = line else { break };
                    if !line.trim().is_empty() {
                        proxy.on_child_line(&line);
                    }
                }
                // The child is gone: fail our own pending calls at once.
                lock(&proxy.pending).clear();
            })
            .map_err(|e| format!("cannot start the reader thread: {e}"))?
    };

    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if !line.trim().is_empty() {
            proxy.on_client_line(&line);
        }
    }
    // Client closed stdin: close the child's so it finishes and exits.
    lock(&proxy.child_in).take();
    let _ = reader.join();
    let _ = child.wait();
    Ok(())
}
