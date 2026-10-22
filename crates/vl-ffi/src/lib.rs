//! C-ABI facade over the rverilator simulator core for host embedders
//! (Bun `bun:ffi`, Python ctypes, …). Complements `vl-wasm` (browser).
//!
//! Protocol (no serde, line-based manifest):
//! ```text
//! top <name>|auto
//! file <path>          # source file, relative to the manifest's dir
//! inject <name> <path> # virtual $readmem* file
//! ```
//! `rcs_run` returns the design stdout, or a string starting with
//! `ERROR: ` — callers branch on the prefix. Free returned strings with
//! [`rcs_free`].

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::PathBuf;
use vl_syntax::parser;
use vl_syntax::pp::{self, PpOptions};

#[no_mangle]
pub extern "C" fn rcs_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn rcs_free(s: *mut c_char) {
    if !s.is_null() {
        unsafe { drop(CString::from_raw(s)) };
    }
}

fn run(manifest_path: &str) -> Result<String, String> {
    let manifest = std::fs::read_to_string(manifest_path)
        .map_err(|e| format!("cannot read manifest `{manifest_path}`: {e}"))?;
    let base = PathBuf::from(manifest_path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    let mut top: Option<String> = None;
    let mut files: Vec<String> = Vec::new();
    let mut inject: Vec<(String, String)> = Vec::new();
    for (i, raw) in manifest.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (kw, rest) = line.split_once(' ').ok_or_else(|| {
            format!("manifest line {}: expected `top|file|inject <arg>`", i + 1)
        })?;
        match kw {
            "top" => top = Some(rest.trim().to_string()),
            "file" => files.push(base.join(rest.trim()).to_string_lossy().into()),
            "inject" => {
                let (name, path) = rest.trim().split_once(' ').ok_or_else(|| {
                    format!("manifest line {}: `inject <name> <path>`", i + 1)
                })?;
                let contents = std::fs::read_to_string(base.join(path.trim()))
                    .map_err(|e| format!("cannot read injected file `{path}`: {e}"))?;
                inject.push((name.to_string(), contents));
            }
            other => return Err(format!("manifest line {}: unknown keyword `{other}`", i + 1)),
        }
    }
    let mut sm = rcs_diag::SourceMap::new();
    let mut diags = rcs_diag::DiagSink::new();
    let opts = PpOptions::default();
    let mut modules = Vec::new();
    let mut timescale: Option<String> = None;
    for f in &files {
        let out = pp::preprocess_file(std::path::Path::new(f), &mut sm, &mut diags, &opts)
            .ok_or_else(|| format!("preprocess failed: {f}"))?;
        if timescale.is_none() {
            timescale = out.timescale.clone();
        }
        let parsed = parser::parse_source_file(out.tokens, &mut diags);
        modules.extend(parsed.modules);
    }
    if diags.has_errors() {
        let mut text = String::new();
        rcs_diag::render_diags(&mut text, &sm, &diags.diags);
        return Err(format!("parse errors:\n{text}"));
    }
    let top = match top {
        Some(t) if t != "auto" => t,
        _ => vl_detect_top(&modules)?,
    };
    let mut sim = vl_sim::Sim::build(&modules, &top, timescale.as_deref())
        .map_err(|e| format!("elaborate: {}", e.message))?;
    for (n, c) in inject {
        sim.inject_file(&n, &c);
    }
    sim.run().map_err(|e| format!("run: {}", e.message))?;
    Ok(sim.take_output())
}

// Top detection without pulling in vl-cli (keeps the ABI surface lean).
fn vl_detect_top(modules: &[vl_syntax::ast::Module]) -> Result<String, String> {
    use vl_syntax::ast::ModuleItem;
    let instantiated: std::collections::HashSet<String> = modules
        .iter()
        .flat_map(|m| m.items.iter())
        .filter_map(|it| match it {
            ModuleItem::Instance(i) => Some(i.mod_name.clone()),
            _ => None,
        })
        .collect();
    let cands: Vec<String> = modules
        .iter()
        .map(|m| m.name.clone())
        .filter(|n| !instantiated.contains(n))
        .collect();
    match cands.as_slice() {
        [only] => Ok(only.clone()),
        _ => Err(format!(
            "cannot auto-detect top module (candidates: {}); write `top <name>` in the manifest",
            if cands.is_empty() { "none".into() } else { cands.join(", ") }
        )),
    }
}

#[no_mangle]
pub extern "C" fn rcs_run(manifest: *const c_char) -> *mut c_char {
    let result = unsafe { CStr::from_ptr(manifest) }
        .to_string_lossy()
        .into_owned();
    let out = match std::panic::catch_unwind(|| run(&result)) {
        Ok(Ok(text)) => text,
        Ok(Err(e)) => format!("ERROR: {e}"),
        Err(_) => "ERROR: internal panic".to_string(),
    };
    CString::new(out).unwrap_or_else(|_| CString::new("ERROR: invalid utf-8").unwrap()).into_raw()
}
