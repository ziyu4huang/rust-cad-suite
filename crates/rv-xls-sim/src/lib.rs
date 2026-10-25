//! `rv-xls-sim` — the XLS-facing process adapter over the shared core.
//!
//! Google XLS's simulation layer is a pluggable interface
//! (`xls/simulation/verilog_simulator.h`): `Run(text, file_type, macros,
//! includes) -> (stdout, stderr)` plus a syntax-check shape, whose only
//! built-in implementation shells out to `iverilog`/`vvp`. This crate is the
//! same shape for rverilator: it takes Verilog in memory or on disk, drives the
//! SAME unified engine the CLI and wasm faces use (`vl_sim::USim`), and returns
//! the design's stdout separately from diagnostics.
//!
//! Two entry shapes share one request model:
//!
//! ```text
//! rv-xls-sim [--syntax-check] [-D NAME[=VAL]] [-I DIR] [--top-module M] file.v...
//! rv-xls-sim --json        # request on stdin, response on stdout
//! ```
//!
//! Contract notes (normative copy: `docs/XLS-ADAPTER.md`):
//!   - design `$display` output goes to stdout; diagnostics go to stderr (the
//!     CLI itself keeps Verilator's all-on-stdout convention — this face exists
//!     for callers that separate the streams, like XLS does);
//!   - `.sv` is refused explicitly (milestone), never silently downgraded;
//!   - the process exits 0 on a clean run, 1 on a compile/simulation failure,
//!     2 on a malformed invocation;
//!   - stdout never contains the scratch directory; diagnostics may reference
//!     the scratch file (named after the source, e.g. `top.v:1:11`) because
//!     that is the file the compiler actually read — the directory is random
//!     and is removed when the run finishes.

pub mod json;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rcs_diag::{DiagSink, SourceMap};
use vl_sim::USim;
use vl_syntax::parser;
use vl_syntax::pp::{self, PpOptions};

pub const VERSION_LINE: &str =
    "rv-xls-sim 0.1.0 (rust-cad-suite) — XLS VerilogSimulator-shaped adapter over the unified rverilator core";

const USAGE: &str = "\
rv-xls-sim — XLS VerilogSimulator-shaped adapter over the rverilator core

Usage: rv-xls-sim [options] <file.v>...
       rv-xls-sim --json   < request.json   (response on stdout)

Options:
  --syntax-check       Preprocess + parse + elaborate; no simulation
  --top-module <mod>   Top module (default: auto-detect the un-instantiated one)
  -D <NAME[=VAL]>      Predefine a preprocessor macro (default 1)
  -I <dir>             Add an include search directory
  --file <path>        Input file (same as a positional file)
  --json               Read a JSON request on stdin, write a JSON response
  --version            Print the version line
  -h, --help           This help

Streams: design output -> stdout, diagnostics -> stderr.
Exit codes: 0 = ok, 1 = compile/simulation error, 2 = bad invocation.
";

/// One input: a real file (`text: None`) or in-memory text written to scratch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub name: String,
    pub text: Option<String>,
}

impl Source {
    pub fn disk(path: &str) -> Self {
        Self { name: path.to_string(), text: None }
    }

    pub fn memory(name: &str, text: &str) -> Self {
        Self { name: name.to_string(), text: Some(text.to_string()) }
    }
}

/// The adapter's request model, shared by argv and JSON entry points.
#[derive(Debug, Clone)]
pub struct Request {
    pub sources: Vec<Source>,
    /// `".v"` only; `.sv` is a hard error (see `run`).
    pub file_type: String,
    pub top: Option<String>,
    /// `NAME` or `NAME=VALUE`, in order.
    pub defines: Vec<String>,
    pub includes: Vec<PathBuf>,
    pub syntax_only: bool,
}

impl Default for Request {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            file_type: ".v".to_string(),
            top: None,
            defines: Vec::new(),
            includes: Vec::new(),
            syntax_only: false,
        }
    }
}

impl Request {
    /// Parse the JSON request shape. Unknown keys are ignored so a new XLS
    /// field does not break the adapter; wrong types are errors.
    pub fn from_json(value: &json::Json) -> Result<Self, String> {
        if !matches!(value, json::Json::Object(_)) {
            return Err("request must be a JSON object".to_string());
        }
        let text = match value.get("text") {
            Some(json::Json::String(text)) => text.clone(),
            Some(_) => return Err("`text` must be a string".to_string()),
            None => return Err("`text` is required".to_string()),
        };
        let mut request = Request {
            sources: vec![Source::memory("top.v", &text)],
            ..Request::default()
        };
        if let Some(field) = value.get("file_type") {
            match field.as_str() {
                Some(text) => request.file_type = text.to_string(),
                None => return Err("`file_type` must be a string".to_string()),
            }
        }
        if let Some(field) = value.get("top") {
            match field {
                json::Json::Null => {}
                other => match other.as_str() {
                    Some(top) => request.top = Some(top.to_string()),
                    None => return Err("`top` must be a string".to_string()),
                },
            }
        }
        if let Some(field) = value.get("defines") {
            match field {
                json::Json::Object(pairs) => {
                    for (name, value) in pairs {
                        match value {
                            json::Json::String(text) if text.is_empty() => {
                                request.defines.push(name.clone());
                            }
                            json::Json::String(text) => {
                                request.defines.push(format!("{name}={text}"));
                            }
                            _ => {
                                return Err(format!("define `{name}` must be a string"));
                            }
                        }
                    }
                }
                _ => return Err("`defines` must be an object of strings".to_string()),
            }
        }
        if let Some(field) = value.get("includes") {
            match field {
                json::Json::Array(items) => {
                    for item in items {
                        match item.as_str() {
                            Some(dir) => request.includes.push(PathBuf::from(dir)),
                            None => return Err("`includes` must be an array of strings".to_string()),
                        }
                    }
                }
                _ => return Err("`includes` must be an array of strings".to_string()),
            }
        }
        if let Some(field) = value.get("syntax_only") {
            match field.as_bool() {
                Some(flag) => request.syntax_only = flag,
                None => return Err("`syntax_only` must be a boolean".to_string()),
            }
        }
        Ok(request)
    }
}

/// One simulation outcome: the two streams plus a process-style exit code.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit: i32,
}

impl Response {
    pub fn success(stdout: String, stderr: String) -> Self {
        Self { ok: true, stdout, stderr, exit: 0 }
    }

    pub fn failure(stdout: String, stderr: String) -> Self {
        Self { ok: false, stdout, stderr, exit: 1 }
    }

    pub fn to_json(&self) -> String {
        format!(
            "{{\"status\":{},\"stdout\":{},\"stderr\":{},\"exit\":{}}}",
            json::quote(if self.ok { "ok" } else { "error" }),
            json::quote(&self.stdout),
            json::quote(&self.stderr),
            self.exit
        )
    }
}

/// Process entry point shared by `main` and the integration tests.
pub fn entry(args: &[String]) -> i32 {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print!("{USAGE}");
        return 0;
    }
    if args.iter().any(|arg| arg == "--version") {
        println!("{VERSION_LINE}");
        return 0;
    }
    if args.iter().any(|arg| arg == "--json") {
        return entry_json();
    }
    let request = match parse_argv(args) {
        Ok(request) => request,
        Err(message) => {
            eprintln!("rv-xls-sim: {message}");
            eprintln!("try `rv-xls-sim --help`");
            return 2;
        }
    };
    let response = run(&request);
    print!("{}", response.stdout);
    eprint!("{}", response.stderr);
    response.exit
}

fn entry_json() -> i32 {
    let mut input = String::new();
    if let Err(error) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("rv-xls-sim: cannot read stdin: {error}");
        return 2;
    }
    let request = match json::parse(&input).and_then(|value| Request::from_json(&value)) {
        Ok(request) => request,
        Err(message) => {
            let response = Response::failure(String::new(), format!("bad request: {message}\n"));
            println!("{}", response.to_json());
            return 2;
        }
    };
    let response = run(&request);
    println!("{}", response.to_json());
    response.exit
}

/// Parse the argv shape. Value-taking flags accept both `-D X` and `-DX`.
pub fn parse_argv(args: &[String]) -> Result<Request, String> {
    let mut request = Request::default();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--syntax-check" | "--lint-only" => request.syntax_only = true,
            "--top-module" | "--top" => {
                request.top = Some(take_value(args, &mut index, arg)?);
            }
            "--file" => {
                let file = take_value(args, &mut index, arg)?;
                request.sources.push(Source::disk(&file));
            }
            "-I" => {
                let dir = take_value(args, &mut index, arg)?;
                request.includes.push(PathBuf::from(dir));
            }
            "-D" => {
                let define = take_value(args, &mut index, arg)?;
                request.defines.push(define);
            }
            "--" => {
                for file in &args[index + 1..] {
                    request.sources.push(Source::disk(file));
                }
                break;
            }
            other if other.starts_with("--top-module=") => {
                request.top = Some(other["--top-module=".len()..].to_string());
            }
            other if other.starts_with("--top=") => {
                request.top = Some(other["--top=".len()..].to_string());
            }
            other if other.starts_with("-I") && other.len() > 2 => {
                request.includes.push(PathBuf::from(&other[2..]));
            }
            other if other.starts_with("-D") && other.len() > 2 => {
                request.defines.push(other[2..].to_string());
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option `{other}`"));
            }
            file => request.sources.push(Source::disk(file)),
        }
        index += 1;
    }
    if request.sources.is_empty() {
        return Err("no input files".to_string());
    }
    Ok(request)
}

fn take_value(args: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("`{flag}` requires a value"))
}

/// Simulate (or syntax-check) one request.
pub fn run(request: &Request) -> Response {
    match run_inner(request) {
        Ok(response) => response,
        Err(message) => Response::failure(String::new(), message),
    }
}

fn run_inner(request: &Request) -> Result<Response, String> {
    let file_type = request.file_type.trim();
    if file_type.is_empty() || file_type.eq_ignore_ascii_case(".v") || file_type.eq_ignore_ascii_case("v") {
        // supported
    } else if file_type.eq_ignore_ascii_case(".sv") || file_type.eq_ignore_ascii_case("sv") {
        return Ok(Response::failure(
            String::new(),
            ".sv (SystemVerilog) is out of scope: rverilator targets Verilog-2005; \
             this is a hard error, not a silent downgrade (milestone)\n"
                .to_string(),
        ));
    } else {
        return Ok(Response::failure(
            String::new(),
            format!("unsupported file_type `{file_type}` (expected .v)\n"),
        ));
    }
    if request.sources.is_empty() {
        return Ok(Response::failure(String::new(), "no input files\n".to_string()));
    }

    let prepared = prepare(request)?;
    let compiled = compile(request, &prepared.paths);
    if request.syntax_only {
        return Ok(syntax_check(request, &compiled));
    }
    Ok(simulate(request, &prepared, &compiled))
}

struct Prepared {
    paths: Vec<String>,
    /// `$readmem*` resolution base (the scratch dir for in-memory text).
    file_base: PathBuf,
    /// Kept alive until the run finishes; dropping removes the directory.
    _scratch: Option<Scratch>,
}

fn prepare(request: &Request) -> Result<Prepared, String> {
    let needs_text = request.sources.iter().any(|source| source.text.is_some());
    let scratch = if needs_text { Some(Scratch::new()?) } else { None };
    let mut paths = Vec::new();
    for (index, source) in request.sources.iter().enumerate() {
        match &source.text {
            Some(text) => {
                let dir = scratch
                    .as_ref()
                    .expect("scratch directory exists when a source has text")
                    .path();
                let name = safe_name(&source.name, index);
                let path = dir.join(&name);
                fs::write(&path, text)
                    .map_err(|error| format!("cannot write {}: {error}\n", path.display()))?;
                paths.push(path.display().to_string());
            }
            None => paths.push(source.name.clone()),
        }
    }
    let file_base = match &scratch {
        Some(scratch) => scratch.path().to_path_buf(),
        None => std::env::current_dir().unwrap_or_default(),
    };
    Ok(Prepared { paths, file_base, _scratch: scratch })
}

/// Only the basename is used: a request cannot write outside the scratch dir.
fn safe_name(name: &str, index: usize) -> String {
    let base = Path::new(name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if base.is_empty() {
        format!("source_{index}.v")
    } else {
        base.to_string()
    }
}

struct Scratch {
    dir: PathBuf,
}

/// Process-local sequence: two threads can observe the same clock tick, and a
/// shared scratch directory would let one test's `top.v` overwrite another's
/// (found by the flaky `parse_error_goes_to_stderr` contract test).
static SCRATCH_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl Scratch {
    fn new() -> Result<Self, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let seq = SCRATCH_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "rv-xls-sim-{}-{stamp}-{seq}",
            std::process::id()
        ));
        fs::create_dir(&dir)
            .map_err(|error| format!("cannot create scratch dir {}: {error}\n", dir.display()))?;
        Ok(Self { dir })
    }

    fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

struct Compiled {
    modules: Vec<vl_syntax::ast::Module>,
    timescale: Option<String>,
    /// `(path, per-file timescale)` — riv-elab derives sticky timescales from it.
    files: Vec<(String, Option<String>)>,
    diagnostics: String,
    parse_failed: bool,
    errors: usize,
    sm: SourceMap,
}

fn compile(request: &Request, paths: &[String]) -> Compiled {
    let options = PpOptions {
        include_dirs: request.includes.clone(),
        predefines: request.defines.clone(),
    };
    let mut sm = SourceMap::new();
    let mut diags = DiagSink::new();
    let mut modules = Vec::new();
    let mut timescale = None;
    let mut files = Vec::new();
    for path_text in paths {
        let path = Path::new(path_text);
        // preprocess_file always reports failures through `diags`.
        let Some(output) = pp::preprocess_file(path, &mut sm, &mut diags, &options) else {
            continue;
        };
        files.push((path_text.clone(), output.timescale.clone()));
        if timescale.is_none() {
            timescale = output.timescale.clone();
        }
        modules.extend(parser::parse_source_file(output.tokens, &mut diags).modules);
    }
    let mut diagnostics = String::new();
    rcs_diag::render_diags(&mut diagnostics, &sm, &diags.diags);
    Compiled {
        modules,
        timescale,
        files,
        diagnostics,
        parse_failed: diags.has_errors(),
        errors: diags.error_count(),
        sm,
    }
}

fn simulate(request: &Request, prepared: &Prepared, compiled: &Compiled) -> Response {
    if compiled.parse_failed {
        return Response::failure(String::new(), parse_failure(compiled));
    }
    let top = match explicit_top(request) {
        Some(top) => top,
        None => match vl_cli::detect_top(&compiled.modules) {
            Ok(top) => top,
            Err(candidates) => return Response::failure(String::new(), no_top(&candidates)),
        },
    };
    let mut sim = match USim::build(
        &compiled.modules,
        &top,
        compiled.timescale.as_deref(),
        &compiled.sm,
        &compiled.files,
    ) {
        Ok(sim) => sim,
        Err(error) => {
            return Response::failure(
                String::new(),
                format!("{}%Error: {}\n", compiled.diagnostics, error.message),
            )
        }
    };
    sim.set_file_base(prepared.file_base.clone());
    let result = sim.run();
    let stdout = sim.take_output();
    match result {
        Ok(()) => Response::success(stdout, compiled.diagnostics.clone()),
        Err(error) => Response::failure(
            stdout,
            format!("{}%Error: {}\n", compiled.diagnostics, error.message),
        ),
    }
}

fn syntax_check(request: &Request, compiled: &Compiled) -> Response {
    if compiled.parse_failed {
        return Response::failure(String::new(), parse_failure(compiled));
    }
    match explicit_top(request) {
        Some(top) => elaborate(compiled, &top),
        None => match vl_cli::detect_top(&compiled.modules) {
            Ok(top) => elaborate(compiled, &top),
            Err(candidates) if candidates.len() > 1 => {
                // `--lint-only` lints every root; the adapter keeps that shape
                // so a multi-module text does not need an explicit top.
                for top in &candidates {
                    let response = elaborate(compiled, top);
                    if !response.ok {
                        return response;
                    }
                }
                Response::success(String::new(), compiled.diagnostics.clone())
            }
            Err(candidates) => Response::failure(String::new(), no_top(&candidates)),
        },
    }
}

fn elaborate(compiled: &Compiled, top: &str) -> Response {
    match USim::build(
        &compiled.modules,
        top,
        compiled.timescale.as_deref(),
        &compiled.sm,
        &compiled.files,
    ) {
        Ok(_) => Response::success(String::new(), compiled.diagnostics.clone()),
        Err(error) => Response::failure(
            String::new(),
            format!("{}%Error: {}\n", compiled.diagnostics, error.message),
        ),
    }
}

fn explicit_top(request: &Request) -> Option<String> {
    request
        .top
        .as_deref()
        .map(str::trim)
        .filter(|top| !top.is_empty())
        .map(str::to_string)
}

fn parse_failure(compiled: &Compiled) -> String {
    format!(
        "{}%Error: {} error(s) during parsing\n",
        compiled.diagnostics, compiled.errors
    )
}

fn no_top(candidates: &[String]) -> String {
    let listed = if candidates.is_empty() {
        "none".to_string()
    } else {
        candidates.join(", ")
    };
    format!("%Error: cannot auto-detect top module (candidates: {listed}); pass top\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_accepts_attached_and_separate_values() {
        let request = parse_argv(&[
            "--syntax-check".to_string(),
            "-DWIDTH=8".to_string(),
            "-I".to_string(),
            "inc".to_string(),
            "--top-module=tb".to_string(),
            "top.v".to_string(),
        ])
        .expect("valid argv");
        assert!(request.syntax_only);
        assert_eq!(request.defines, vec!["WIDTH=8".to_string()]);
        assert_eq!(request.includes, vec![PathBuf::from("inc")]);
        assert_eq!(request.top.as_deref(), Some("tb"));
        assert_eq!(request.sources, vec![Source::disk("top.v")]);
    }

    #[test]
    fn argv_rejects_unknown_flags_and_missing_files() {
        assert!(parse_argv(&["--nope".to_string()]).is_err());
        assert!(parse_argv(&[]).is_err());
        assert!(parse_argv(&["-D".to_string()]).is_err());
    }

    #[test]
    fn json_request_reads_the_documented_fields() {
        let value = json::parse(
            r#"{"text": "module tb; endmodule", "file_type": ".v",
                "top": "tb", "defines": {"A": "1", "B": ""},
                "includes": ["inc"], "syntax_only": true,
                "future_field": 3}"#,
        )
        .expect("valid json");
        let request = Request::from_json(&value).expect("valid request");
        assert_eq!(request.file_type, ".v");
        assert_eq!(request.top.as_deref(), Some("tb"));
        assert_eq!(request.defines, vec!["A=1".to_string(), "B".to_string()]);
        assert_eq!(request.includes, vec![PathBuf::from("inc")]);
        assert!(request.syntax_only);
        assert_eq!(request.sources[0].name, "top.v");
    }

    #[test]
    fn json_request_rejects_wrong_shapes() {
        for text in [
            r#"{"text": 3}"#,
            r#"{"top": "tb"}"#,
            r#"{"text": "x", "syntax_only": "yes"}"#,
            r#"{"text": "x", "includes": "inc"}"#,
            r#"{"text": "x", "defines": {"A": 1}}"#,
        ] {
            let value = json::parse(text).expect("json parses");
            assert!(Request::from_json(&value).is_err(), "{text} should be rejected");
        }
    }

    #[test]
    fn scratch_directories_are_unique() {
        let first = Scratch::new().expect("scratch");
        let second = Scratch::new().expect("scratch");
        assert_ne!(first.path(), second.path());
    }
}
