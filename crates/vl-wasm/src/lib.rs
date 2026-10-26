//! WASM facade over the rverilator simulator core ([`vl_sim`]).
//!
//! Facade pattern (ADR-0003): `vl-sim` stays wasm-agnostic — this crate only
//! translates types across the JS boundary and reuses the CLI's top-module
//! auto-detection. Compiled with `wasm-pack build crates/vl-wasm --target bun`
//! (or `--target web`); consumed from JS/Bun, no toolchain in the browser.

#[cfg(target_arch = "wasm32")]
use vl_sim::vpi::CbReason;
use vl_sim::vpi::{Format, VpiValue};
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

fn sim_from_parts(
    top: &str,
    sources: Vec<String>,
    file_names: Vec<String>,
    file_contents: Vec<String>,
) -> Result<vl_sim::interp::Sim, JsValue> {
    let (modules, timescale) = compile_sources(&sources)?;
    let top = resolve_top(&modules, top)?;
    let mut sim = vl_sim::Sim::build(&modules, &top, timescale.as_deref())
        .map_err(|e| JsValue::from_str(&format!("elaborate: {}", e.message)))?;
    for (n, c) in file_names.into_iter().zip(file_contents) {
        sim.inject_file(&n, &c);
    }
    Ok(sim)
}

/// `run_verilog` with `$readmem*` files injected virtually (browser hosts
/// have no filesystem): `file_names[i]` -> `file_contents[i]`.
#[wasm_bindgen]
pub fn run_verilog_with_files(
    top: &str,
    sources: Vec<String>,
    file_names: Vec<String>,
    file_contents: Vec<String>,
) -> Result<RunResult, JsValue> {
    if file_names.len() != file_contents.len() {
        return Err(JsValue::from_str("file_names/file_contents length mismatch"));
    }
    let mut sim = sim_from_parts(top, sources, file_names, file_contents)?;
    sim.run()
        .map_err(|e| JsValue::from_str(&format!("run: {}", e.message)))?;
    let stdout = sim.take_output();
    let vcd = sim.take_vcd().map(|(_, text)| text);
    Ok(RunResult { stdout, vcd })
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

#[cfg(target_arch = "wasm32")]
fn cb_closure(f: &js_sys::Function) -> vl_sim::vpi::CbFn {
    let f = f.clone();
    Box::new(move |sim, info| {
        // Deliver {time, value, valueHex} as a plain JS object; a throwing
        // JS callback is logged, never aborts the run (registered contract).
        let obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&obj, &"time".into(), &info.time.into());
        let _ = js_sys::Reflect::set(
            &obj,
            &"value".into(),
            &match &info.value {
                Some(b) => b.fmt_bin().into(),
                None => JsValue::NULL,
            },
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &"valueHex".into(),
            &match &info.value {
                Some(b) => b.fmt_hex().into(),
                None => JsValue::NULL,
            },
        );
        if let Err(e) = f.call1(&JsValue::NULL, &obj) {
            if let Some(err_fn) = js_sys::Reflect::get(&js_sys::global(), &"console".into())
                .ok()
                .and_then(|c| js_sys::Reflect::get(&c, &"error".into()).ok())
                .and_then(|v| v.dyn_into::<js_sys::Function>().ok())
            {
                let _ = err_fn.call1(&JsValue::NULL, &e);
            }
        }
        let _ = sim;
    })
}

#[wasm_bindgen]
impl WasmSim {
    #[wasm_bindgen(constructor)]
    pub fn new(top: &str, sources: Vec<String>) -> Result<WasmSim, JsValue> {
        WasmSim::with_files(top, sources, Vec::new(), Vec::new())
    }

    /// Constructor with virtual `$readmem*` files (browser: no filesystem).
    pub fn with_files(
        top: &str,
        sources: Vec<String>,
        file_names: Vec<String>,
        file_contents: Vec<String>,
    ) -> Result<WasmSim, JsValue> {
        let inner = sim_from_parts(top, sources, file_names, file_contents)?;
        Ok(WasmSim { inner })
    }

    /// Inject a virtual file for `$readmem*` (must precede `run`).
    pub fn inject_file(&mut self, name: &str, contents: &str) {
        self.inner.inject_file(name, contents);
    }

    /// Force the VCD dump without a TB `$dumpvars` (call before `run`).
    pub fn force_dump(&mut self) {
        self.inner.force_dump();
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

    // ---------- rVPI surface ----------

    /// Current value: fmt = "int" | "bin" | "hex" | "dec".
    pub fn vpi_get(&mut self, path: &str, fmt: &str) -> Result<String, JsValue> {
        let h = self
            .inner
            .vpi_handle_by_name(path)
            .ok_or_else(|| JsValue::from_str(&format!("no object `{path}`")))?;
        let f = match fmt {
            "int" => Format::Int,
            "bin" => Format::BinStr,
            "hex" => {
                let b = self.inner.vpi_bits(h).map_err(js_err)?;
                return Ok(b.fmt_hex());
            }
            "dec" => {
                let b = self.inner.vpi_bits(h).map_err(js_err)?;
                return Ok(b.fmt_dec());
            }
            _ => return Err(JsValue::from_str("fmt: int|bin|hex|dec")),
        };
        match self.inner.vpi_get_as(h, f) {
            Ok(v) => Ok(match v {
                VpiValue::Int(i) => i.to_string(),
                VpiValue::Scalar(s) => format!("{s:?}"),
                VpiValue::Vector(b) => b.fmt_bin(),
                VpiValue::BinStr(s) => s,
                VpiValue::Time(t) => t.to_string(),
            }),
            Err(e) => Err(JsValue::from_str(&e.message)),
        }
    }

    /// Blocking write (put_value NoDelay semantics).
    pub fn vpi_put_int(&mut self, path: &str, value: i64) -> Result<(), JsValue> {
        let h = self
            .inner
            .vpi_handle_by_name(path)
            .ok_or_else(|| JsValue::from_str(&format!("no object `{path}`")))?;
        self.inner
            .vpi_put_value(h, VpiValue::Int(value))
            .map_err(|e| JsValue::from_str(&e.message))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn on_value_change(&mut self, path: &str, f: js_sys::Function) -> Result<(), JsValue> {
        let h = self
            .inner
            .vpi_handle_by_name(path)
            .ok_or_else(|| JsValue::from_str(&format!("no object `{path}`")))?;
        self.inner.register_cb(CbReason::ValueChange(h), cb_closure(&f));
        Ok(())
    }

    /// `ticks` are precision ticks (the TB timescale's precision unit).
    #[cfg(target_arch = "wasm32")]
    pub fn on_after_delay(&mut self, ticks: u64, f: js_sys::Function) {
        self.inner
            .register_cb(CbReason::AfterDelay(ticks), cb_closure(&f));
    }

    #[cfg(target_arch = "wasm32")]
    pub fn on_start_of_simulation(&mut self, f: js_sys::Function) {
        self.inner
            .register_cb(CbReason::StartOfSimulation, cb_closure(&f));
    }

    #[cfg(target_arch = "wasm32")]
    pub fn on_end_of_simulation(&mut self, f: js_sys::Function) {
        self.inner
            .register_cb(CbReason::EndOfSimulation, cb_closure(&f));
    }

    /// `vpi_control(vpiFinish)`.
    pub fn finish(&mut self) {
        self.inner.finish();
    }

    // The js-sys callback surface exists only under wasm32; the host build
    // gets compiling stubs (this crate is built for wasm exclusively).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn on_value_change(&mut self, _path: &str, _f: js_sys::Function) -> Result<(), JsValue> {
        Err(JsValue::from_str("rVPI callbacks are wasm-only"))
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn on_after_delay(&mut self, _ticks: u64, _f: js_sys::Function) {}
    #[cfg(not(target_arch = "wasm32"))]
    pub fn on_start_of_simulation(&mut self, _f: js_sys::Function) {}
    #[cfg(not(target_arch = "wasm32"))]
    pub fn on_end_of_simulation(&mut self, _f: js_sys::Function) {}
}

/// Parse a VCD with the SAME reader the CLI and Bun-FFI facades use (the
/// shared `vl-vcd` core) — the browser demo renders from this, so all three
/// tiers share one waveform parser. Errors surface as
/// `VL-E1xx: message @ byte N`; tolerated conditions are counted in
/// `warning_count` (details are not yet exposed across the boundary).
/// One-shot convenience: parse + render with default options (toggle-heavy
/// hosts should hold a `VcdJs` and call its `to_svg` instead).
#[wasm_bindgen]
pub fn vcd_to_svg(text: &str) -> Result<String, JsValue> {
    vcd_parse(text)?.to_svg(Vec::new(), 0, 0, 0, 0, 0)
}

#[wasm_bindgen]
pub fn vcd_parse(text: &str) -> Result<VcdJs, JsValue> {
    let f = vl_vcd::parse(text).map_err(|d| {
        JsValue::from_str(&format!("{}: {} @ byte {}", d.code, d.message, d.offset))
    })?;
    Ok(VcdJs { inner: f })
}

/// JS view of a parsed VCD. Times are precision ticks (`times()` returns a
/// BigInt64Array); values are raw 4-state/real/string text.
#[wasm_bindgen]
pub struct VcdJs {
    inner: vl_vcd::VcdFile,
}

/// `to_svg` flags: bit0 = t0 set, bit1 = t1 set, bit2 = height set.
pub const SVG_F_T0: u32 = 1;
pub const SVG_F_T1: u32 = 2;
pub const SVG_F_HEIGHT: u32 = 4;

#[wasm_bindgen]
impl VcdJs {
    #[wasm_bindgen(getter)]
    pub fn timescale(&self) -> Option<String> {
        self.inner.timescale.clone()
    }

    #[wasm_bindgen(getter, js_name = varCount)]
    pub fn var_count(&self) -> usize {
        self.inner.vars.len()
    }

    #[wasm_bindgen(getter, js_name = tMax)]
    pub fn t_max(&self) -> u64 {
        self.inner.t_max()
    }

    #[wasm_bindgen(getter, js_name = warningCount)]
    pub fn warning_count(&self) -> usize {
        self.inner.warnings.len()
    }

    pub fn var_id(&self, i: usize) -> String {
        self.inner.vars[i].id.clone()
    }

    pub fn var_name(&self, i: usize) -> String {
        self.inner.vars[i].name.clone()
    }

    pub fn var_width(&self, i: usize) -> u32 {
        self.inner.vars[i].width
    }

    pub fn var_type(&self, i: usize) -> String {
        self.inner.vars[i].var_type.clone()
    }

    /// Change times for var `i`, ascending (BigInt64Array across the
    /// boundary).
    pub fn times(&self, i: usize) -> Vec<u64> {
        self.inner
            .changes
            .iter()
            .filter(|c| c.var == i)
            .map(|c| c.time)
            .collect()
    }

    /// Raw value texts for var `i`, index-aligned with `times(i)`.
    pub fn values(&self, i: usize) -> Vec<String> {
        self.inner
            .changes
            .iter()
            .filter(|c| c.var == i)
            .map(|c| c.value.to_raw())
            .collect()
    }

    /// Render the ALREADY-PARSED dump to a deterministic SVG (no re-parse on
    /// toggle). `signals` = row subset in render order (empty = all, file
    /// order); `t0`/`t1` are u64 (BigInt at the boundary — ADR-0005) and only
    /// read when the matching flag bit is set; `height` likewise with
    /// SVG_F_HEIGHT.
    pub fn to_svg(
        &self,
        signals: Vec<String>,
        width: u32,
        height: u32,
        t0: u64,
        t1: u64,
        flags: u32,
    ) -> Result<String, JsValue> {
        let opts = vl_vcd::RenderOptions {
            width: if width == 0 { 960 } else { width },
            height: if flags & SVG_F_HEIGHT != 0 && height > 0 {
                Some(height)
            } else {
                None
            },
            t0: if flags & SVG_F_T0 != 0 { Some(t0) } else { None },
            t1: if flags & SVG_F_T1 != 0 { Some(t1) } else { None },
            signals: if signals.is_empty() { None } else { Some(signals) },
            ..Default::default()
        };
        vl_vcd::render(&self.inner, &opts)
            .map_err(|d| JsValue::from_str(&format!("{}: {} @ byte {}", d.code, d.message, d.offset)))
    }
}

fn js_err(e: vl_sim::SimError) -> JsValue {
    JsValue::from_str(&e.message)
}

// ---------- spec-verification layer (ADR-0007 browser closed loop) ----------
//
// One-shot: the scripted I2C spec check (clean-room master + golden model +
// four protocol invariants, canonical write script) runs wholly inside wasm;
// JS receives the structured report plus the run's VCD for SVG rendering.
// The scenario composition lives in spec-i2c (core-first, AGENTS 10.7);
// this surface only translates types. u64 times cross as BigInt.

/// Variant selector for [`i2c_spec_check`]: 0 = correct RTL, 1 = LSB-first
/// (bit order surgically broken), 2 = no-ack-release (the slave-side ack
/// release surgically removed). Unknown values are a hard error.
#[wasm_bindgen]
pub fn i2c_spec_check(variant: u32) -> Result<SpecCheckJs, JsValue> {
    let v = match variant {
        0 => spec_i2c::scenario::Variant::Correct,
        1 => spec_i2c::scenario::Variant::LsbFirst,
        2 => spec_i2c::scenario::Variant::NoAckRelease,
        other => return Err(JsValue::from_str(&format!("unknown i2c spec variant {other} (0=correct, 1=lsb-first, 2=no-ack-release)"))),
    };
    let inner = spec_i2c::scenario::run(v).map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(SpecCheckJs { inner })
}

/// The (possibly surgically broken) RTL source the check runs, for
/// display via textContent — broken variants show the flipped lines.
#[wasm_bindgen]
pub fn i2c_rtl(variant: u32) -> Result<String, JsValue> {
    let v = match variant {
        0 => spec_i2c::scenario::Variant::Correct,
        1 => spec_i2c::scenario::Variant::LsbFirst,
        2 => spec_i2c::scenario::Variant::NoAckRelease,
        other => return Err(JsValue::from_str(&format!("unknown i2c spec variant {other}"))),
    };
    spec_i2c::scenario::rtl_for(v).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Structured result of one [`i2c_spec_check`] run.
#[wasm_bindgen]
pub struct SpecCheckJs {
    inner: spec_i2c::scenario::ScenarioResult,
}

#[wasm_bindgen]
impl SpecCheckJs {
    /// Lockstep cycles the script ran (BigInt across the boundary).
    #[wasm_bindgen(getter, js_name = cyclesRun)]
    pub fn cycles_run(&self) -> u64 {
        self.inner.report.cycles_run
    }

    #[wasm_bindgen(getter, js_name = violationCount)]
    pub fn violation_count(&self) -> usize {
        self.inner.report.violations.len()
    }

    /// Precision-tick time of violation `i` (BigInt).
    pub fn violation_time(&self, i: usize) -> Result<u64, JsValue> {
        self.inner
            .report
            .violations
            .get(i)
            .map(|v| v.time)
            .ok_or_else(|| JsValue::from_str("violation index out of range"))
    }

    /// Lockstep cycle index of violation `i` (BigInt).
    pub fn violation_cycle(&self, i: usize) -> Result<u64, JsValue> {
        self.inner
            .report
            .violations
            .get(i)
            .map(|v| v.cycle)
            .ok_or_else(|| JsValue::from_str("violation index out of range"))
    }

    /// Kind of violation `i`: "invariant `<name>`" or "lockstep[<name>]".
    pub fn violation_kind(&self, i: usize) -> Result<String, JsValue> {
        self.inner
            .report
            .violations
            .get(i)
            .map(|v| match &v.kind {
                vl_spec::ViolationKind::Invariant(n) => format!("invariant `{n}`"),
                vl_spec::ViolationKind::Lockstep(n) => format!("lockstep[{n}]"),
            })
            .ok_or_else(|| JsValue::from_str("violation index out of range"))
    }

    pub fn violation_expected(&self, i: usize) -> Result<String, JsValue> {
        self.inner
            .report
            .violations
            .get(i)
            .map(|v| v.expected.clone())
            .ok_or_else(|| JsValue::from_str("violation index out of range"))
    }

    pub fn violation_actual(&self, i: usize) -> Result<String, JsValue> {
        self.inner
            .report
            .violations
            .get(i)
            .map(|v| v.actual.clone())
            .ok_or_else(|| JsValue::from_str("violation index out of range"))
    }

    pub fn violation_message(&self, i: usize) -> Result<String, JsValue> {
        self.inner
            .report
            .violations
            .get(i)
            .map(|v| v.message.clone())
            .ok_or_else(|| JsValue::from_str("violation index out of range"))
    }

    /// Canonical multi-line text form (cargo-test facing).
    pub fn render_text(&self) -> String {
        self.inner.report.render_text()
    }

    /// The run's VCD text (force_dump at assembly; taken before finish) —
    /// feed to `vcd_parse` + the SVG renderer, never canvas.
    #[wasm_bindgen(getter)]
    pub fn vcd(&self) -> Option<String> {
        self.inner.vcd.clone()
    }
}

// ---------------------------------------------------------------------------
// riverilog/rivvp face (ADR-0009: one engine, two CLI faces — the browser
// gets both, exactly like the native CLIs).
// ---------------------------------------------------------------------------

/// riverilog face, compile step: sources -> RIV-1 text. The same pipeline
/// as the `riverilog` CLI minus the filesystem surface (shared pp+parse ->
/// flat elaboration -> design write; no -y/-E in the browser). `top` may be
/// empty for auto-detection.
#[wasm_bindgen]
pub fn riv_build(top: &str, sources: Vec<String>) -> Result<String, JsValue> {
    let mut sm = rcs_diag::SourceMap::new();
    let mut diags = rcs_diag::DiagSink::new();
    let opts = PpOptions::default();
    let mut modules = Vec::new();
    let mut files = Vec::new();
    for (i, src) in sources.iter().enumerate() {
        let name = format!("source_{i}.v");
        let out = pp::preprocess_source(&name, src, &mut sm, &mut diags, &opts);
        files.push(riv_elab::FileTs {
            path: name,
            timescale: out.timescale.clone(),
        });
        let parsed = parser::parse_source_file(out.tokens, &mut diags);
        modules.extend(parsed.modules);
    }
    if diags.has_errors() {
        let mut text = String::new();
        rcs_diag::render_diags(&mut text, &sm, &diags.diags);
        return Err(JsValue::from_str(&format!("errors:\n{text}")));
    }
    let top = resolve_top(&modules, top)?;
    let design = riv_elab::elaborate(riv_elab::ElabInput::strict(
        &modules,
        &sm,
        Some(&top),
        &files,
    ))
    .map_err(|e| JsValue::from_str(&e.message))?;
    Ok(design.write())
}

/// rivvp face, run step: a RIV-1 design text plus plusargs -> stdout + VCD.
/// The same kernel the `rivvp` CLI drives, with its defaults ($stop pauses
/// and the run returns the output produced so far; the finish banner is the
/// iverilog-face form).
#[wasm_bindgen]
pub fn riv_run(riv: String, plusargs: Vec<String>) -> Result<RunResult, JsValue> {
    let d = riv_ir::read(&riv)
        .map_err(|e| JsValue::from_str(&format!("not a riv design ({e})")))?;
    let mut kernel = riv_kernel::Kernel::new(d.clone())
        .map_err(|e| JsValue::from_str(&format!("design load: {}", e.0)))?;
    kernel.set_plusargs(plusargs);
    let mut sink = RivVcdSink::new(&d);
    let outcome = kernel.run(&mut sink);
    Ok(RunResult {
        stdout: outcome.output,
        vcd: sink.text,
    })
}

/// DumpSink over the vl-vcd writer capturing the VCD text in memory (the
/// browser has no filesystem for rivvp's file write). Var registry mirrors
/// the rivvp sink: non-event/non-array vars under raw dotted paths split
/// into real VCD scopes (ADR-0009 U-1 byte-compat shape).
struct RivVcdSink {
    timescale: String,
    path: Option<String>,
    writer: Option<vl_vcd::VcdWriter>,
    var_list: Vec<(String, Vec<String>, u32)>,
    text: Option<String>,
}

impl RivVcdSink {
    fn new(d: &riv_ir::Design) -> RivVcdSink {
        let var_list: Vec<(String, Vec<String>, u32)> = d
            .vars
            .iter()
            .filter(|v| {
                v.kind != riv_ir::VarKind::Event && v.kind != riv_ir::VarKind::Array
            })
            .map(|v| {
                (
                    v.name.clone(),
                    v.name.split('.').map(|c| c.to_string()).collect(),
                    v.width,
                )
            })
            .collect();
        RivVcdSink {
            timescale: d.meta.precision.clone(),
            path: None,
            writer: None,
            var_list,
            text: None,
        }
    }
}

impl riv_kernel::DumpSink for RivVcdSink {
    fn opened(&mut self, path: &str) {
        self.path = Some(path.to_string());
    }

    fn dump_section(&mut self, now: u64, name: &str, values: &std::collections::BTreeMap<String, String>) {
        if let Some(w) = self.writer.as_mut() {
            w.dump_section(now, name, values);
        }
    }

    fn dump_t0(&mut self, values: &std::collections::BTreeMap<String, String>) {
        let vars = self.var_list.clone();
        let mut w = vl_vcd::VcdWriter::new_named(
            vars,
            "rivvp (riverilog runtime, rust-cad-suite wasm)",
            &self.timescale,
        );
        w.start();
        w.dump_t0(values);
        self.writer = Some(w);
    }

    fn change(&mut self, name: &str, value: &str) {
        if let Some(w) = &mut self.writer {
            w.record_change(name, value);
        }
    }

    fn flush_slot(&mut self, now: u64) {
        if let Some(w) = &mut self.writer {
            w.flush_at(now);
        }
    }

    fn finish(&mut self) {
        if let Some(w) = self.writer.take() {
            self.text = Some(w.finish());
        }
    }
}
