//! WASM facade over the rverilator simulator core ([`vl_sim`]).
//!
//! Facade pattern (ADR-0003): `vl-sim` stays wasm-agnostic — this crate only
//! translates types across the JS boundary and reuses the CLI's top-module
//! auto-detection. Compiled with `wasm-pack build crates/vl-wasm --target bun`
//! (or `--target web`); consumed from JS/Bun, no toolchain in the browser.

use vl_syntax::parser;
use vl_syntax::pp::{self, PpOptions};
use wasm_bindgen::prelude::*;

fn compile_sources(sources: &[String]) -> Result<(Vec<vl_syntax::ast::Module>, Option<String>), JsValue> {
    let mut sm = rcs_diag::SourceMap::new();
    let mut diags = rcs_diag::DiagSink::new();
    let opts = PpOptions::default();
    let mut modules = Vec::new();
    let mut timescale: Option<String> = None;
    for (i, src) in sources.iter().enumerate() {
        let name = format!("source_{i}.v");
        // preprocess_source always returns output (errors land in diags).
        let out = pp::preprocess_source(&name, src, &mut sm, &mut diags, &opts);
        if timescale.is_none() {
            timescale = out.timescale.clone();
        }
        let parsed = parser::parse_source_file(out.tokens, &mut diags);
        modules.extend(parsed.modules);
    }
    if diags.has_errors() {
        let mut text = String::new();
        rcs_diag::render_diags(&mut text, &sm, &diags.diags);
        return Err(JsValue::from_str(&format!("parse errors:\n{text}")));
    }
    Ok((modules, timescale))
}

fn resolve_top(modules: &[vl_syntax::ast::Module], top: &str) -> Result<String, JsValue> {
    if !top.is_empty() {
        return Ok(top.to_string());
    }
    vl_cli::detect_top(modules).map_err(|cands| {
        JsValue::from_str(&format!(
            "cannot auto-detect top module (candidates: {}); pass --top",
            if cands.is_empty() { "none".to_string() } else { cands.join(", ") }
        ))
    })
}

/// One-shot result: design output plus the VCD trace (if the TB dumps).
#[wasm_bindgen]
pub struct RunResult {
    stdout: String,
    vcd: Option<String>,
}

#[wasm_bindgen]
impl RunResult {
    #[wasm_bindgen(getter)]
    pub fn stdout(&self) -> String {
        self.stdout.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn vcd(&self) -> Option<String> {
        self.vcd.clone()
    }
}

fn run_inner(top: &str, sources: Vec<String>) -> Result<RunResult, JsValue> {
    let (modules, timescale) = compile_sources(&sources)?;
    let top = resolve_top(&modules, top)?;
    let mut sim = vl_sim::Sim::build(&modules, &top, timescale.as_deref())
        .map_err(|e| JsValue::from_str(&format!("elaborate: {}", e.message)))?;
    sim.run()
        .map_err(|e| JsValue::from_str(&format!("run: {}", e.message)))?;
    let stdout = sim.take_output();
    let vcd = sim.take_vcd().map(|(_, text)| text);
    Ok(RunResult { stdout, vcd })
}

/// Compile + simulate in one shot. `top` may be empty for auto-detection.
#[wasm_bindgen]
pub fn run_verilog(top: &str, sources: Vec<String>) -> Result<RunResult, JsValue> {
    run_inner(top, sources)
}

/// stdout-only convenience (the common corpus-check shape).
#[wasm_bindgen]
pub fn run_verilog_stdout(top: &str, sources: Vec<String>) -> Result<String, JsValue> {
    Ok(run_inner(top, sources)?.stdout())
}

/// Stepwise handle: build once, run/inspect from JS (rVPI-style surface
/// lands here in a later slice).
#[wasm_bindgen]
pub struct WasmSim {
    inner: vl_sim::interp::Sim,
}

#[wasm_bindgen]
impl WasmSim {
    #[wasm_bindgen(constructor)]
    pub fn new(top: &str, sources: Vec<String>) -> Result<WasmSim, JsValue> {
        let (modules, timescale) = compile_sources(&sources)?;
        let top = resolve_top(&modules, top)?;
        let inner = vl_sim::Sim::build(&modules, &top, timescale.as_deref())
            .map_err(|e| JsValue::from_str(&format!("elaborate: {}", e.message)))?;
        Ok(WasmSim { inner })
    }

    pub fn run(&mut self) -> Result<(), JsValue> {
        self.inner
            .run()
            .map_err(|e| JsValue::from_str(&format!("run: {}", e.message)))
    }

    pub fn take_output(&mut self) -> String {
        self.inner.take_output()
    }

    pub fn take_vcd(&mut self) -> Option<String> {
        self.inner.take_vcd().map(|(_, text)| text)
    }
}
