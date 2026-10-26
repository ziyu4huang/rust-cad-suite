//! U-4 (ADR-0009 unified engine): additive rVPI hooks. The callback
//! REGISTRY lives with the embedder (vl-sim's VpiState, ADR-0003); the
//! Kernel owns only the four FIRING POINTS, called with the kernel itself
//! so the embedder can read/write storage. All hooks are Option — the
//! production CLI paths pay zero cost (a None check per var commit).
//!
//! Firing-point contract (mirrors the legacy Sim wiring, ADR-0003 §slice-1):
//! - start_of_simulation: once at run() entry, before any thread step.
//! - value_change: post-settle at the single var commit funnel
//!   (store_var_raw), arg = committed VarId, value already stored.
//! - after_delay: at each time-wheel advance, arg = the new `now`
//!   (precision ticks), before the slots at that time execute.
//! - end_of_simulation: once at the finish path (any of the finished
//!   checkpoints), guarded against double-fire.

use std::rc::Rc;

use crate::Kernel;

/// One hook = closure over the running kernel; `arg` is VarId for
/// value_change and precision ticks for after_delay (0 for start/end).
pub type HookFn = Box<dyn FnMut(&mut Kernel, u64)>;

#[derive(Default)]
pub struct VpiHooks {
    pub start_of_sim: Option<HookFn>,
    pub end_of_sim: Option<HookFn>,
    pub value_change: Option<HookFn>,
    pub after_delay: Option<HookFn>,
}

impl VpiHooks {
    pub fn is_active(&self) -> bool {
        self.start_of_sim.is_some()
            || self.end_of_sim.is_some()
            || self.value_change.is_some()
            || self.after_delay.is_some()
    }
}

/// Storage view handed to embedders so callbacks can read/write design
/// vars without touching kernel internals.
impl Kernel {
    /// Post-commit value of a design var (post-settle, post-2-state
    /// projection when the profile projects).
    pub fn vpi_read_var(&self, var: u32) -> Option<vl_values::Bits> {
        self.vars.get(var as usize).cloned()
    }

    /// Blocking write through the same funnel as design writes: 2-state
    /// projection, no-op when unchanged, waiter wake + gate resched.
    /// The VCD sink is not informed (VCD sees design-driven changes; a
    /// put_value via VPI during a run is a testbench-side force — the
    /// legacy face has the same asymmetry, REGISTRY rivD note pending).
    pub fn vpi_write_var(&mut self, var: u32, v: vl_values::Bits) -> Result<(), String> {
        let mut sink = NoopDump;
        self.store_var_raw(var, v, &mut sink)
            .map_err(|e| e.0)
    }

    /// Width of a design var (declared).
    pub fn vpi_var_width(&self, var: u32) -> Option<u32> {
        self.d.vars.get(var as usize).map(|v| v.width)
    }

    pub fn vpi_var_name(&self, var: u32) -> Option<&str> {
        self.d.vars.get(var as usize).map(|v| v.name.as_str())
    }

    pub fn vpi_now(&self) -> u64 {
        self.now
    }
}

/// Sink that drops everything (VPI-driven writes are not dump events).
pub struct NoopDump;
impl crate::DumpSink for NoopDump {
    fn opened(&mut self, _path: &str) {}
    fn dump_t0(&mut self, _values: &std::collections::BTreeMap<String, String>) {}
    fn change(&mut self, _name: &str, _value: &str) {}
    fn flush_slot(&mut self, _now: u64) {}
}

/// Shared hook cell for embedders that construct the set before the
/// Kernel (Rc<RefCell<VpiHooks>> is installed post-construction).
pub fn shared() -> Rc<std::cell::RefCell<VpiHooks>> {
    Rc::new(std::cell::RefCell::new(VpiHooks::default()))
}
