//! C-ABI facade over the rverilator simulator core for host embedders
//! (Bun `bun:ffi`, Python ctypes, …). Complements `vl-wasm` (browser).
//!
//! Protocol (no serde, line-based manifest):
//! ```text
//! top <name>|auto
//! file <path>          # source file, relative to the manifest's dir
//! inject <name> <path> # virtual $readmem* file
//! vcd <path>           # force the dump (no TB $dumpvars needed) and
//!                      # write the VCD text to <path> after the run
//! ```
//! `rcs_run` returns the design stdout ONLY (adding the VCD here would
//! break embedders that compare stdout byte-exactly — write it to a file
//! with `vcd <path>` instead), or a string starting with `ERROR: ` —
//! callers branch on the prefix. Free returned strings with [`rcs_free`].
//!
//! [`rcs_vcd_svg`] renders VCD text to a deterministic SVG with the SAME
//! renderer behind the `vcd2svg` CLI and the wasm facade (default options;
//! knobs = new entry points later — ABI v1 freeze note, ADR-0005).
//! [`rcs_vcd_parse`] feeds VCD text to the SAME shared reader the CLI and
//! wasm facades use (vl-vcd) and returns a line protocol:
//! ```text
//! ok timescale=<ts|-> vars=<n> tmax=<t> warns=<k>
//! var <idx> <width> <type> <id> <hier-name>
//! c <idx> <time> <value>
//! warn <code> <offset> <message>
//! ```
//! or `ERROR: VL-E1xx ... @ byte N`.

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
    let mut vcd_out: Option<PathBuf> = None;
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
            "vcd" => vcd_out = Some(base.join(rest.trim())),
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
    if vcd_out.is_some() {
        sim.force_dump();
    }
    sim.run().map_err(|e| format!("run: {}", e.message))?;
    if let Some(path) = vcd_out {
        let (_, text) = sim
            .take_vcd()
            .ok_or_else(|| "internal: force_dump produced no VCD".to_string())?;
        std::fs::write(&path, text)
            .map_err(|e| format!("cannot write vcd `{}`: {e}", path.display()))?;
    }
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

/// Parse VCD text with the shared vl-vcd reader. Returns the line protocol
/// (see the module docs) or `ERROR: ...`; free with [`rcs_free`].
#[no_mangle]
pub extern "C" fn rcs_vcd_parse(text: *const c_char) -> *mut c_char {
    let text = unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned();
    let out = match std::panic::catch_unwind(|| vcd_line_protocol(&text)) {
        Ok(Ok(lines)) => lines,
        Ok(Err(e)) => format!("ERROR: {e}"),
        Err(_) => "ERROR: internal panic".to_string(),
    };
    CString::new(out)
        .unwrap_or_else(|_| CString::new("ERROR: invalid utf-8").unwrap())
        .into_raw()
}

/// Render VCD text to SVG (default options). Returns the SVG document or
/// `ERROR: ...`; free with [`rcs_free`].
#[no_mangle]
pub extern "C" fn rcs_vcd_svg(text: *const c_char) -> *mut c_char {
    let text = unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned();
    let out = match std::panic::catch_unwind(|| {
        vl_vcd::parse(&text)
            .and_then(|f| vl_vcd::render(&f, &vl_vcd::RenderOptions::default()))
    }) {
        Ok(Ok(svg)) => svg,
        Ok(Err(d)) => format!("ERROR: {}: {} @ byte {}", d.code, d.message, d.offset),
        Err(_) => "ERROR: internal panic".to_string(),
    };
    CString::new(out)
        .unwrap_or_else(|_| CString::new("ERROR: invalid utf-8").unwrap())
        .into_raw()
}

fn vcd_line_protocol(text: &str) -> Result<String, String> {
    let f = vl_vcd::parse(text).map_err(|d| format!("{}: {} @ byte {}", d.code, d.message, d.offset))?;
    let mut out = String::new();
    out.push_str(&format!(
        "ok timescale={} vars={} tmax={} warns={}\n",
        f.timescale.as_deref().unwrap_or("-"),
        f.vars.len(),
        f.t_max(),
        f.warnings.len()
    ));
    for (i, v) in f.vars.iter().enumerate() {
        out.push_str(&format!(
            "var {i} {} {} {} {}\n",
            v.width, v.var_type, v.id, v.name
        ));
    }
    for c in &f.changes {
        out.push_str(&format!("c {} {} {}\n", c.var, c.time, c.value.to_raw()));
    }
    for w in &f.warnings {
        out.push_str(&format!("warn {} {} {}\n", w.code, w.offset, w.message));
    }
    Ok(out)
}
