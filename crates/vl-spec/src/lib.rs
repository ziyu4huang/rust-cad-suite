//! vl-spec — spec-verification core (ADR-0007).
//!
//! Three-layer spec form, all machine-checkable:
//! 1. interface contract — ports/params/reset values (asserted by a
//!    [`Invariant`] or a plain test against the elaborated sim);
//! 2. executable golden model — a [`GoldenModel`] stepped in LOCKSTEP with
//!    the RTL, projections compared every cycle;
//! 3. per-cycle protocol invariants — [`Invariant`] impls checked every
//!    cycle, violations recorded with time/signal/expected-vs-actual.
//!
//! Tick discipline (pinned): inputs change only while clk is low; the
//! harness applies them via rVPI put_value (which settles combinational
//! logic immediately) and then pairs every put with a `run_until` — put
//! wakes processes, only run_until runs them. Sampling happens after
//! `run_until` returns, i.e. after the slot is fully drained (active +
//! inactive + NBA + settle) — no same-slot display-vs-NBA ambiguity.
//!
//! Wasm-safe: no filesystem or process access at runtime; results are
//! returned as values (the demo page consumes the same API).

pub mod report;

use std::collections::HashMap;
use vl_sim::interp::{RunEnd, Sim};
use vl_sim::vpi::{Format, VpiHandle, VpiValue};

pub use report::{SpecReport, SpecViolation, ViolationKind};

#[derive(Debug, Clone)]
pub struct SpecError(pub String);

impl std::fmt::Display for SpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SpecError {}

impl From<vl_sim::SimError> for SpecError {
    fn from(e: vl_sim::SimError) -> Self {
        SpecError(e.message)
    }
}

/// Compile + elaborate from in-memory sources (the wasm path; the CLI path
/// wraps the same pp+parse).
pub fn build_sim(files: &[(&str, &str)], top: &str) -> Result<Sim, SpecError> {
    let mut sm = rcs_diag::SourceMap::new();
    let mut diags = rcs_diag::DiagSink::new();
    let opts = vl_syntax::pp::PpOptions::default();
    let mut modules = Vec::new();
    let mut timescale: Option<String> = None;
    for (name, src) in files {
        let out = vl_syntax::pp::preprocess_source(name, src, &mut sm, &mut diags, &opts);
        if timescale.is_none() {
            timescale = out.timescale.clone();
        }
        let parsed = vl_syntax::parser::parse_source_file(out.tokens, &mut diags);
        modules.extend(parsed.modules);
    }
    if diags.has_errors() {
        let mut text = String::new();
        rcs_diag::render_diags(&mut text, &sm, &diags.diags);
        return Err(SpecError(format!("parse errors:\n{text}")));
    }
    vl_sim::Sim::build(&modules, top, timescale.as_deref())
        .map_err(|e| SpecError(format!("elaborate: {}", e.message)))
}

/// Handle cache: names resolve once, reads are per-cycle.
#[derive(Default)]
pub struct Signals {
    index: HashMap<String, usize>,
    handles: Vec<VpiHandle>,
    names: Vec<String>,
}

impl Signals {
    pub fn bind(&mut self, sim: &Sim, name: &str) -> Result<SignalId, SpecError> {
        if let Some(&id) = self.index.get(name) {
            return Ok(SignalId(id));
        }
        let h = sim
            .vpi_handle_by_name(name)
            .ok_or_else(|| SpecError(format!("no signal `{name}`")))?;
        let id = SignalId(self.handles.len());
        self.handles.push(h);
        self.names.push(name.to_string());
        self.index.insert(name.to_string(), id.0);
        Ok(id)
    }

    pub fn u64(&self, sim: &Sim, id: SignalId) -> Result<u64, SpecError> {
        match sim.vpi_get_as(self.handles[id.0], Format::Int)? {
            VpiValue::Int(v) => Ok(v as u64),
            VpiValue::Vector(b) => Ok(b.to_u128_lossy() as u64),
            other => Err(SpecError(format!("signal `{}`: {other:?} is not integral", self.names[id.0]))),
        }
    }

    pub fn bits(&self, sim: &Sim, id: SignalId) -> Result<vl_sim::value::Bits, SpecError> {
        match sim.vpi_get_as(self.handles[id.0], Format::Vector)? {
            VpiValue::Vector(b) => Ok(b),
            other => Err(SpecError(format!("signal `{}`: {other:?} is not a vector", self.names[id.0]))),
        }
    }

    pub fn name(&self, id: SignalId) -> &str {
        &self.names[id.0]
    }

    /// Put an integral value (inputs are driven through this; the put
    /// settles combinational logic immediately but never runs processes —
    /// pair it with a harness cycle/run_until).
    pub fn put_int(&self, sim: &mut Sim, id: SignalId, v: i64) -> Result<(), SpecError> {
        sim.vpi_put_value(self.handles[id.0], VpiValue::Int(v))
            .map_err(|e| SpecError(format!("put {}: {}", self.names[id.0], e.message)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SignalId(usize);

/// Per-cycle context handed to invariants.
pub struct CycleCtx<'a> {
    pub sim: &'a Sim,
    pub sigs: &'a Signals,
    /// Lockstep cycle index (0-based, one clk period per cycle).
    pub cycle: u64,
    /// Precision-tick time at the sampling point.
    pub time: u64,
}

/// A per-cycle protocol invariant (spec layer 3). Stateful by nature — edge
/// memory lives in the impl.
pub trait Invariant {
    fn name(&self) -> &'static str;
    fn check(&mut self, ctx: &CycleCtx, out: &mut Vec<SpecViolation>);
}

/// An executable golden model (spec layer 2). `step` advances one clk cycle
/// on the input the RTL just clocked; `observe` returns the projection the
/// harness compares against the RTL every cycle.
pub trait GoldenModel {
    type Input;
    type Obs: PartialEq + std::fmt::Debug;
    fn step(&mut self, input: &Self::Input);
    fn observe(&self) -> Self::Obs;
}

/// Lockstep harness: drives the DUT programmatically (no Verilog
/// testbench), steps a golden model on the same inputs, compares
/// projections, and runs invariants — every cycle.
pub struct LockstepSpec<M: GoldenModel> {
    sim: Sim,
    sigs: Signals,
    model: M,
    clk: SignalId,
    half: u64,
    driver: Box<dyn Fn(&mut Sim, &Signals, &M::Input) -> Result<(), SpecError>>,
    probe: Box<dyn Fn(&Signals, &Sim) -> Result<M::Obs, SpecError>>,
    invariants: Vec<Box<dyn Invariant>>,
    violations: Vec<SpecViolation>,
    cycle: u64,
    /// Optional post-half hook: runs after both clock halves have drained,
    /// before invariants — bus models refresh levels here so invariants see
    /// the bus as of the real edges.
    post: Option<Box<dyn Fn(&mut Sim, &Signals)>>,
}

impl<M: GoldenModel> LockstepSpec<M> {
    pub fn new(
        sim: Sim,
        clk: &str,
        half_period_ticks: u64,
        model: M,
    ) -> Result<Self, SpecError> {
        let mut sigs = Signals::default();
        let clk = sigs.bind(&sim, clk)?;
        Ok(LockstepSpec {
            sim,
            sigs,
            model,
            clk,
            half: half_period_ticks,
            driver: Box::new(|_, _, _| Ok(())),
            probe: Box::new(|_, _| Err(SpecError("no probe registered".into()))),
            invariants: Vec::new(),
            violations: Vec::new(),
            cycle: 0,
            post: None,
        })
    }

    /// Register the post-half refresh hook (see the field doc).
    pub fn post(mut self, f: impl Fn(&mut Sim, &Signals) + 'static) -> Self {
        self.post = Some(Box::new(f));
        self
    }

    pub fn signals(&mut self) -> &mut Signals {
        &mut self.sigs
    }

    pub fn sim(&mut self) -> &mut Sim {
        &mut self.sim
    }

    /// Resolve a signal name once (handle cached for the spec's lifetime).
    pub fn bind_signal(&mut self, name: &str) -> Result<SignalId, SpecError> {
        self.sigs.bind(&self.sim, name)
    }

    /// Put an integral value outside the driver (init sequences, forced
    /// errors).
    pub fn put_int(&mut self, id: SignalId, v: i64) -> Result<(), SpecError> {
        self.sigs.put_int(&mut self.sim, id, v)
    }

    /// Read an integral signal (field-disjoint accessor: no borrow split
    /// between signals/sim at call sites).
    pub fn read_u64(&self, id: SignalId) -> u64 {
        self.sigs.u64(&self.sim, id).unwrap_or(u64::MAX)
    }

    /// Write an integral signal.
    pub fn write_int(&mut self, id: SignalId, v: i64) {
        let _ = self.sigs.put_int(&mut self.sim, id, v);
    }

    /// Advance to an absolute time (init sequences between reset and the
    /// first lockstep cycle).
    pub fn run_until(&mut self, t: u64) -> Result<RunEnd, SpecError> {
        self.sim
            .run_until(t)
            .map_err(|e| SpecError(format!("run_until: {}", e.message)))
    }

    pub fn time(&self) -> u64 {
        self.sim.now_ticks()
    }

    /// Register an input-driver: applies `input` to DUT ports (call sites
    /// must only put while clk is low — the harness calls this before the
    /// rising edge).
    pub fn driver(
        mut self,
        f: impl Fn(&mut Sim, &Signals, &M::Input) -> Result<(), SpecError> + 'static,
    ) -> Self {
        self.driver = Box::new(f);
        self
    }

    /// Register the RTL-side projection: reads DUT outputs into `M::Obs`
    /// (called after the clock edge slot is drained).
    pub fn probe(
        mut self,
        f: impl Fn(&Signals, &Sim) -> Result<M::Obs, SpecError> + 'static,
    ) -> Self {
        self.probe = Box::new(f);
        self
    }

    pub fn invariant(mut self, inv: impl Invariant + 'static) -> Self {
        self.invariants.push(Box::new(inv));
        self
    }

    fn put_clk(&mut self, v: i64) -> Result<(), SpecError> {
        let h = self.sigs.handles[self.clk.0];
        self.sim
            .vpi_put_value(h, VpiValue::Int(v))
            .map_err(|e| SpecError(format!("clk put: {}", e.message)))?;
        Ok(())
    }

    /// One lockstep cycle: drive at clk-low → posedge half → negedge half →
    /// step the model on the same input → compare projections → run
    /// invariants.
    pub fn cycle(&mut self, input: &M::Input) -> Result<RunEnd, SpecError> {
        let t = self.sim.now_ticks();
        let (t_hi, t_lo_end) = (t + self.half, t + 2 * self.half);
        (self.driver)(&mut self.sim, &self.sigs, input)?;
        self.put_clk(1)?;
        let end = self
            .sim
            .run_until(t_hi)
            .map_err(|e| SpecError(format!("posedge half: {}", e.message)))?;
        self.put_clk(0)?;
        let end = match end {
            RunEnd::Finished => RunEnd::Finished,
            RunEnd::Running => self
                .sim
                .run_until(t_lo_end)
                .map_err(|e| SpecError(format!("negedge half: {}", e.message)))?,
        };
        self.model.step(input);

        if let Some(post) = &self.post {
            let sim = &mut self.sim;
            let sigs = &self.sigs;
            post(sim, sigs);
        }

        // Projection comparison (the lockstep leg).
        let expected = self.model.observe();
        match (self.probe)(&self.sigs, &self.sim) {
            Ok(actual) => {
                if expected != actual {
                    self.violations.push(SpecViolation {
                        time: self.sim.now_ticks(),
                        cycle: self.cycle,
                        kind: ViolationKind::Lockstep("projection"),
                        expected: format!("{expected:?}"),
                        actual: format!("{actual:?}"),
                        message: "RTL projection != golden model".into(),
                    });
                }
            }
            Err(e) => return Err(e),
        }

        // Per-cycle invariants.
        let ctx = CycleCtx {
            sim: &self.sim,
            sigs: &self.sigs,
            cycle: self.cycle,
            time: self.sim.now_ticks(),
        };
        let mut out = std::mem::take(&mut self.violations);
        for inv in &mut self.invariants {
            inv.check(&ctx, &mut out);
        }
        self.violations = out;
        self.cycle += 1;
        Ok(end)
    }

    pub fn finish(self) -> SpecReport {
        SpecReport {
            violations: self.violations,
            cycles_run: self.cycle,
        }
    }
}

/// Thin time accessor so harnesses/invariants can read the sim clock
/// without importing vl-sim internals.
pub fn sim_time(sim: &Sim) -> u64 {
    sim.now_ticks()
}
