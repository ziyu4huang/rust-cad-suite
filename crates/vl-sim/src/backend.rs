//! Unified-engine backend (ADR-0009 U-1): the riv-elab → riv-kernel
//! pipeline behind the simulation facade the CLI/difftest faces drive.
//! Zero-drift acceptance = the corpus/e2e goldens: the harvest arcs proved
//! this pipeline reproduces them byte-exact (58/59 direct; 53_readmem is
//! the registered rivD10 two-oracle divergence where we match iverilog).
//!
//! Staging: USim carries the faces that don't need rVPI (CLI --binary,
//! difftest). The VPI-bearing facades (wasm/ffi/spec) migrate at U-4;
//! vpi.rs stays on the legacy segments interpreter until then.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use riv_kernel::{DumpSink, Kernel};

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
        let design = riv_elab::elaborate(riv_elab::ElabInput {
            modules,
            sm,
            top: Some(top),
            files: &file_ts,
        })
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
        Ok(USim {
            kernel,
            design: rc,
            sink,
            vcd_taken: false,
            output: String::new(),
        })
    }

    /// Run to completion (kernel's stratified slot loop).
    pub fn run(&mut self) -> SResult<()> {
        self.kernel.stop_as_finish = false;
        let outcome = self.kernel.run(&mut self.sink);
        self.sink.finish();
        self.output.push_str(&outcome.output);
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
