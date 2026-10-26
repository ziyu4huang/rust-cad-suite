//! Unified-engine backend (ADR-0009 U-1): the riv-elab → riv-kernel
//! pipeline behind the simulation facade the CLI/difftest faces drive.
//! Zero-drift acceptance = the corpus/e2e goldens: the harvest arcs proved
//! this pipeline reproduces them byte-exact (58/59 direct; 53_readmem is
//! the registered rivD10 two-oracle divergence where we match iverilog).
//!
//! Staging: USim carries the faces that don't need rVPI (CLI --binary,
//! difftest). The VPI-bearing facades (wasm/ffi/spec) migrate at U-4;
//! vpi.rs stays on the legacy segments interpreter until then.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use riv_kernel::{DumpSink, Kernel};
use crate::value::Bits;

use crate::vpi;
use crate::SimError;

pub type SResult<T> = Result<T, SimError>;

fn err(m: impl Into<String>) -> SimError {
    SimError::new(m.into(), None)
}

/// VCD capture over the vl-vcd writer (single core, ADR-0004): the scope
/// tree from the design's dotted var paths (rivvp's sink shape, moved
/// here so every facade shares one implementation).
struct VcdSink {
    writer: Option<vl_vcd::VcdWriter>,
    var_list: Vec<(String, Vec<String>, u32)>,
    timescale: String,
    text: String,
    opened: bool,
}

impl VcdSink {
    fn new(d: &riv_ir::Design) -> VcdSink {
        // Raw var paths ("a", "dut.q") — legacy VCD shape, no top prefix.
        let var_list: Vec<(String, Vec<String>, u32)> = d
            .vars
            .iter()
            .filter(|v| v.kind != riv_ir::VarKind::Event && v.kind != riv_ir::VarKind::Array)
            .map(|v| {
                (
                    v.name.clone(),
                    v.name.split('.').map(|c| c.to_string()).collect(),
                    v.width,
                )
            })
            .collect();
        VcdSink {
            writer: None,
            var_list,
            timescale: d.meta.precision.clone(),
            text: String::new(),
            opened: false,
        }
    }
}

impl DumpSink for VcdSink {
    fn opened(&mut self, _path: &str) {
        self.opened = true;
        let vars = self.var_list.clone();
        let mut w = vl_vcd::VcdWriter::new_named(vars, "rverilator (rust-cad-suite)", &self.timescale);
        w.start();
        self.writer = Some(w);
    }

    fn dump_t0(&mut self, values: &BTreeMap<String, String>) {
        if let Some(w) = self.writer.as_mut() {
            w.dump_t0(values);
        }
    }

    fn change(&mut self, name: &str, value: &str) {
        if let Some(w) = self.writer.as_mut() {
            w.record_change(name, value);
        }
    }

    fn flush_slot(&mut self, now: u64) {
        if let Some(w) = self.writer.as_mut() {
            w.flush_at(now);
        }
    }

    fn finish(&mut self) {
        if let Some(w) = self.writer.take() {
            self.text = w.finish();
        }
    }
}

/// Unified-engine simulation. Public methods mirror the legacy `Sim`
/// output surface (run/take_output/take_vcd/now_ticks); `run_until`
/// arrives with the kernel's stepwise extension (ADR-0007 parity).
pub struct USim {
    kernel: Kernel,
    /// Kernel::run drains its internal buffer into RunOutcome.output —
    /// captured here for take_output (legacy take semantics).
    output: String,
    /// Kept for the U-2 求值合流 (const surface) and VCD scope rebuilds.
    #[allow(dead_code)]
    design: std::rc::Rc<riv_ir::Design>,
    sink: VcdSink,
    /// Taken by `take_vcd` (legacy semantics: `None` after the take).
    vcd_taken: bool,
    /// U-4 slice-2: the shared rVPI registry (ADR-0003), one concrete type
    /// for every face. Cell = Rc<RefCell<_>> because the kernel's hook
    /// closures (FnMut(&mut Kernel, u64)) must reach it while the kernel
    /// is mutably borrowed; all mutation happens inside short borrow_mut
    /// windows at the four firing points.
    vpi: Rc<RefCell<vpi::VpiState>>,
    /// Table index → kernel VarId (VpiState::build sorts by name; the
    /// kernel's ids are declaration order — this is the only bridge).
    vpi_vars: Rc<Vec<u32>>,
    /// Kernel VarId → table index (value-change hook arg routing).
    vpi_handle_of: Rc<HashMap<u32, u32>>,
    /// Table index → canonical name (host.name() while the registry cell
    /// is already borrowed by the firing loop).
    vpi_names: Rc<Vec<String>>,
    /// First fire error (cascade budget, host write failure) — surfaced by
    /// run() as SResult, mirroring the legacy face's Err path.
    vpi_error: Rc<RefCell<Option<String>>>,
    hooks_installed: bool,
    /// `finish()` before `run()` (wasm hosts finish pre-run); honored at
    /// run entry via kernel stop_as_finish.
    finish_requested: bool,
}

impl USim {
    /// Build through riv-elab (one lowering — the crate law: the const
    /// evaluator and the kernel share the same lowered ops).
    pub fn build(
        modules: &[vl_syntax::ast::Module],
        top: &str,
        timescale: Option<&str>,
        sm: &rcs_diag::SourceMap,
        files: &[(String, Option<String>)],
    ) -> SResult<USim> {
        // riv-elab derives per-file sticky timescales from `files`; the
        // legacy `timescale` argument stays for signature compatibility
        // with the legacy build (the corpus pins: per-file wins).
        let _ = timescale;
        let file_ts: Vec<riv_elab::FileTs> = files
            .iter()
            .map(|(p, ts)| riv_elab::FileTs { path: p.clone(), timescale: ts.clone() })
            .collect();
        let design = riv_elab::elaborate(riv_elab::ElabInput::strict(
            modules,
            sm,
            Some(top),
            &file_ts,
        ))
        .map_err(|e| err(e.message))?;
        let rc = std::rc::Rc::new(design);
        let mut kernel = Kernel::new((*rc).clone()).map_err(|e| err(e.0))?;
        // The rverilator face never carried the iverilog finish banner nor
        // the VCD-info tool line (rivD8's drop-contract at the API level).
        kernel.emit_finish_line = false;
        kernel.emit_vcd_info_line = false;
        // Verilator-form profile: 2-state projection at the storage
        // boundary (init x→0 included) — the face's spec is match
        // Verilator; 53_readmem returns to the golden this way.
        kernel.project_two_state().map_err(|e| err(e.0))?;
        let sink = VcdSink::new(&rc);
        // U-4 slice-2: build the shared rVPI registry from the same rows
        // the VCD sink lists (value-bearing vars; Event/Array are the
        // registered exclusions — array words are the R7 face's slice).
        // Kernel VarId = declaration order of the full var list; the VCD
        // filter is order-preserving, so indices map back 1:1.
        let mut rows: Vec<(String, vpi::CellKind)> = Vec::new();
        let mut vpi_vars: Vec<u32> = Vec::new();
        let mut handle_of: HashMap<u32, u32> = HashMap::new();
        for (i, v) in rc.vars.iter().enumerate() {
            if v.kind == riv_ir::VarKind::Event || v.kind == riv_ir::VarKind::Array {
                continue;
            }
            let kind = match v.kind {
                riv_ir::VarKind::Reg => vpi::CellKind::Reg,
                _ => vpi::CellKind::Net,
            };
            rows.push((v.name.clone(), kind));
            handle_of.insert(i as u32, vpi_vars.len() as u32);
            vpi_vars.push(i as u32);
        }
        let st = vpi::VpiState::build(rows);
        let vpi_names: Vec<String> = st.table.iter().map(|m| m.name.clone()).collect();
        Ok(USim {
            kernel,
            design: rc,
            sink,
            vcd_taken: false,
            output: String::new(),
            vpi: Rc::new(RefCell::new(st)),
            vpi_vars: Rc::new(vpi_vars),
            vpi_handle_of: Rc::new(handle_of),
            vpi_names: Rc::new(vpi_names),
            vpi_error: Rc::new(RefCell::new(None)),
            hooks_installed: false,
            finish_requested: false,
        })
    }

    /// Run to completion (kernel's stratified slot loop).
    pub fn run(&mut self) -> SResult<()> {
        self.kernel.stop_as_finish = self.finish_requested;
        self.install_hooks();
        let outcome = self.kernel.run(&mut self.sink);
        self.sink.finish();
        self.output.push_str(&outcome.output);
        if let Some(e) = self.vpi_error.borrow_mut().take() {
            return Err(err(e));
        }
        if outcome.exit != 0 {
            // rivvp surfaces `rivvp: runtime error: …`; the rverilator
            // face uses `%Error:` — strip the engine prefix, keep the text.
            let msg = outcome
                .output
                .lines()
                .find(|l| l.starts_with("rivvp: runtime error: "))
                .map(|l| l.trim_start_matches("rivvp: runtime error: ").to_string())
                .unwrap_or_else(|| "simulation failed".to_string());
            return Err(err(msg));
        }
        Ok(())
    }

    /// Design stdout (without the iverilog-shaped finish banner — the
    /// kernel's rivD8 line is suppressed for this face; the goldens never
    /// carried it).
    pub fn take_output(&mut self) -> String {
        std::mem::take(&mut self.output)
    }

    /// `(dump_path, vcd_text)` when `$dumpvars` opened a dump; `None` when
    /// not or already taken (legacy take semantics).
    pub fn take_vcd(&mut self) -> Option<(Option<String>, String)> {
        if self.vcd_taken || !self.sink.opened || self.sink.text.is_empty() {
            return None;
        }
        self.vcd_taken = true;
        let path = self.kernel.dump_path.clone();
        Some((path, std::mem::take(&mut self.sink.text)))
    }

    pub fn now_ticks(&self) -> u64 {
        self.kernel.now_ticks()
    }

    /// `$readmem*` resolution base (difftest drives the case dir; the CLI
    /// runs from cwd like the legacy engine).
    pub fn set_file_base(&mut self, dir: PathBuf) {
        self.kernel.file_base = dir;
    }

    /// wasm/browser hosts: name → contents, checked before the filesystem.
    pub fn inject_file(&mut self, name: &str, contents: &str) {
        self.kernel.injected_files.insert(name.to_string(), contents.to_string());
    }

    /// Force the VCD dump without a TB `$dumpvars` (legacy Sim::force_dump
    /// parity; wasm demo / FFI vcd keyword). Call before `run()`.
    pub fn force_dump(&mut self) {
        self.kernel.force_dump(&mut self.sink);
    }

    /// `vpi_control(vpiFinish)` (Stop conflates; matches `$stop`). Honored
    /// at run entry or at the next after_delay checkpoint.
    pub fn finish(&mut self) {
        self.finish_requested = true;
    }

    fn var_of_handle(&self, h: vpi::VpiHandle) -> SResult<u32> {
        self.vpi_vars
            .get(h.0 as usize)
            .copied()
            .ok_or_else(|| err(format!("rVPI: handle {} out of range", h.0)))
    }

    /// Raw 4-state bits of a table object (value-bearing vars only — the
    /// USim table excludes Event/Array rows, registered U-4 slice-2 scope).
    pub fn vpi_bits(&self, h: vpi::VpiHandle) -> SResult<Bits> {
        let var = self.var_of_handle(h)?;
        self.kernel
            .vpi_read_var(var)
            .ok_or_else(|| err("rVPI: var storage missing"))
    }

    /// Declared width (legacy vpi_size parity).
    pub fn vpi_size(&self, h: vpi::VpiHandle) -> u32 {
        self.var_of_handle(h)
            .ok()
            .and_then(|var| self.kernel.vpi_var_width(var))
            .unwrap_or(0)
    }

    fn var_signed(&self, var: u32) -> bool {
        self.design.vars.get(var as usize).is_some_and(|v| v.signed)
    }

    /// `vpi_get_value` int semantics: x/z refuses; signed vars sign-extend.
    pub fn vpi_get_int(&self, h: vpi::VpiHandle) -> SResult<i64> {
        let b = self.vpi_bits(h)?;
        if b.has_unknown() {
            return Err(err("int format on a value carrying x/z (use Vector/BinStr)"));
        }
        let raw = b.to_u64_lossy();
        if self.var_signed(self.var_of_handle(h)?)
            && b.width > 0
            && b.width < 64
            && raw & (1 << (b.width - 1)) != 0
        {
            Ok((raw | !((1u64 << b.width) - 1)) as i64)
        } else {
            Ok(raw as i64)
        }
    }

    /// NoDelay write through the kernel's single write funnel.
    pub fn vpi_put_bits(&mut self, h: vpi::VpiHandle, bits: &Bits) -> SResult<()> {
        let var = self.var_of_handle(h)?;
        self.kernel.vpi_write_var(var, bits.clone()).map_err(|e| err(e))
    }

    // ---------- rVPI registration face (U-4 slice-2, ADR-0003) ----------

    /// `vpi_register_cb` on the shared registry. Same signature/semantics
    /// as the legacy Sim face (portable callback code across faces).
    pub fn register_cb(&mut self, reason: vpi::CbReason, f: vpi::CbFn) -> vpi::CbId {
        self.vpi.borrow_mut().register(reason, f)
    }

    /// `vpi_remove_cb` (tombstone; ids never recycled).
    pub fn remove_cb(&mut self, id: vpi::CbId) {
        let mut v = self.vpi.borrow_mut();
        if (id.0 as usize) < v.cbs.len() {
            v.cbs[id.0 as usize] = None;
        }
    }

    /// `vpi_handle_by_name` (bare cell paths, same canon rule as legacy).
    pub fn vpi_handle_by_name(&self, path: &str) -> Option<vpi::VpiHandle> {
        let p = path
            .strip_prefix(&format!("{}.", self.design.meta.top))
            .unwrap_or(path);
        self.vpi.borrow().index.get(p).copied().map(vpi::VpiHandle)
    }

    /// Install the kernel hook closures that route the four unified-engine
    /// firing points into the shared registry. Only when callbacks exist —
    /// the CLI paths stay zero-cost (slice-1 Option gate).
    fn install_hooks(&mut self) {
        if self.hooks_installed || !self.vpi.borrow().has_callbacks() {
            return;
        }
        self.hooks_installed = true;
        let vpi_c = self.vpi.clone();
        let vars = self.vpi_vars.clone();
        let names = self.vpi_names.clone();
        let verr = self.vpi_error.clone();
        self.kernel.vpi_hooks.start_of_sim = Some(Box::new(move |k, _| {
            fire_in(&vpi_c, &vars, &names, &verr, k, |v, h| v.fire_start(h));
        }));
        let vpi_c = self.vpi.clone();
        let handle_of = self.vpi_handle_of.clone();
        let vars = self.vpi_vars.clone();
        let names = self.vpi_names.clone();
        let verr = self.vpi_error.clone();
        self.kernel.vpi_hooks.value_change = Some(Box::new(move |k, arg| {
            let var = arg as u32;
            let Some(hi) = handle_of.get(&var).copied() else {
                return;
            };
            // Collapse into the pending map, then drain immediately: the
            // kernel funnel fires post-settle per commit, so the callback
            // time is the committing tick (the legacy face drains at its
            // statement-chunk points; the cadence difference is the
            // registered U-4 slice-2 divergence, ADR-0003 amendment).
            if let Some(b) = k.vpi_read_var(var) {
                let mut v = vpi_c.borrow_mut();
                if v.has_vc_callbacks() {
                    v.pending_vc.insert(hi, b);
                }
            }
            fire_in(&vpi_c, &vars, &names, &verr, k, |v, h| v.fire_vc(h));
        }));
        let vpi_c = self.vpi.clone();
        let vars = self.vpi_vars.clone();
        let names = self.vpi_names.clone();
        let verr = self.vpi_error.clone();
        self.kernel.vpi_hooks.after_delay = Some(Box::new(move |k, _| {
            fire_in(&vpi_c, &vars, &names, &verr, k, |v, h| v.fire_due_timers(h));
        }));
        let vpi_c = self.vpi.clone();
        let vars = self.vpi_vars.clone();
        let names = self.vpi_names.clone();
        let verr = self.vpi_error.clone();
        self.kernel.vpi_hooks.end_of_sim = Some(Box::new(move |k, _| {
            fire_in(&vpi_c, &vars, &names, &verr, k, |v, h| v.fire_vc(h));
            fire_in(&vpi_c, &vars, &names, &verr, k, |v, h| v.fire_end(h));
        }));
    }
}

/// Run one firing function with the kernel bridged as the [`vpi::VpiHost`].
/// First error wins (surfaced by USim::run, mirroring the legacy Err path).
fn fire_in(
    vpi: &Rc<RefCell<vpi::VpiState>>,
    vars: &Rc<Vec<u32>>,
    names: &Rc<Vec<String>>,
    verr: &Rc<RefCell<Option<String>>>,
    k: &mut Kernel,
    f: impl FnOnce(&mut vpi::VpiState, &mut dyn vpi::VpiHost) -> SResult<()>,
) {
    let mut v = vpi.borrow_mut();
    let mut host = UsimHost {
        k,
        vars: Rc::clone(vars),
        names: Rc::clone(names),
    };
    if let Err(e) = f(&mut v, &mut host) {
        drop(v);
        let mut slot = verr.borrow_mut();
        if slot.is_none() {
            *slot = Some(e.message.clone());
        }
    }
}

/// The unified-engine embedder view: kernel storage through the slice-1
/// vpi_* face, table-index → VarId via `vars`.
struct UsimHost<'a> {
    k: &'a mut Kernel,
    vars: Rc<Vec<u32>>,
    names: Rc<Vec<String>>,
}

impl vpi::VpiHost for UsimHost<'_> {
    fn bits(&self, h: vpi::VpiHandle) -> SResult<Bits> {
        let var = *self
            .vars
            .get(h.0 as usize)
            .ok_or_else(|| err(format!("rVPI: handle {} out of range", h.0)))?;
        self.k
            .vpi_read_var(var)
            .ok_or_else(|| err("rVPI: var storage missing"))
    }
    fn put_bits(&mut self, h: vpi::VpiHandle, bits: &Bits) -> SResult<()> {
        let var = *self
            .vars
            .get(h.0 as usize)
            .ok_or_else(|| err(format!("rVPI: handle {} out of range", h.0)))?;
        self.k.vpi_write_var(var, bits.clone()).map_err(|e| err(e))
    }
    fn name(&self, h: vpi::VpiHandle) -> &str {
        self.names.get(h.0 as usize).map(|s| s.as_str()).unwrap_or("")
    }
    fn time(&self, _ts: vpi::TimeScale) -> u64 {
        self.k.vpi_now()
    }
    fn finish(&mut self) {
        self.k.stop_as_finish = true;
    }
}

/// Small additive surface on Kernel used by the backend (implemented in
/// riv-kernel): output buffer access, ticks, dump path, file resolution.
pub trait KernelBackend {
    fn take_output_buf(&mut self) -> String;
    fn now_ticks(&self) -> u64;
    fn resolve_readmem(&self, path: &str) -> Option<PathBuf>;
}

impl KernelBackend for Kernel {
    fn take_output_buf(&mut self) -> String {
        Kernel::take_output_buf(self)
    }
    fn now_ticks(&self) -> u64 {
        Kernel::now_ticks(self)
    }
    fn resolve_readmem(&self, path: &str) -> Option<PathBuf> {
        Kernel::resolve_readmem(self, path)
    }
}

/// Unused placeholder import guard (kernel DumpSink re-export stability).
#[allow(dead_code)]
fn _sink_is_object_safe(_: &dyn DumpSink) {}

/// Path helpers kept module-level for tests.
pub fn join_base(base: &Path, path: &str) -> PathBuf {
    if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        base.join(path)
    }
}
