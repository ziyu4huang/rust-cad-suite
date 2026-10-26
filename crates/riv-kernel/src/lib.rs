//! riv-kernel — the event-driven interpreter for RIV-1 designs (design
//! §4): stratified per-slot loop (Active FIFO → Inactive #0 → NBA
//! batch-commit → Monitor region skeleton, repeated until all empty),
//! BTreeMap time wheel in global precision units, threads with explicit
//! PC + frame slots (three-address ops, no value stack).
//!
//! Thread order = declaration order (ADR-0001 convention); wakeup into
//! the Active tail preserves it. `$finish` prints the iverilog-shaped
//! `<file>:<line>: $finish called at <t> (<precision>)` line and stops.
//! VCD goes through [`DumpSink`] — the kernel itself never produces VCD
//! text (ADR-0004: rivvp attaches the vl-vcd writer).

use std::collections::{BTreeMap, VecDeque};

use riv_ir::{Arg, Base, Design, EdgeKind, FmtPiece, GateKind, Lval, Op, SensItem, Slot, SysId, VarId, VarKind};
use vl_values::{from_numlit_parts, Bits, Base as VB};

#[derive(Debug)]
pub struct RunError(pub String);

pub struct RunOutcome {
    /// Everything the design (and the finish/VCD-info tool lines) wrote
    /// to stdout, in order.
    pub output: String,
    /// 0 = clean stop ($finish or event exhaustion); 1 = runtime error.
    pub exit: i32,
    /// True when the run reached a finished checkpoint (finish/stop/error/
    /// event exhaustion); false when a stepwise deadline stopped it early
    /// (U-5 slice-2: the caller resumes with another run_until).
    pub finished: bool,
}

pub mod vpi;

/// VCD/dump hook. The kernel notifies; the host (rivvp) owns the actual
/// vl-vcd writer and file IO.
pub trait DumpSink {
    /// `$dumpvars` opened the dump for `path`.
    fn opened(&mut self, path: &str);
    /// Dump-control section (`$dumpoff`/`$dumpon`/`$dumpall`, R5): every
    /// var's value under the current timestamp (x's for dumpoff).
    fn dump_section(&mut self, _now: u64, _name: &str, _values: &BTreeMap<String, String>) {}
    /// t0 initialization dump (current value of every var, by name).
    fn dump_t0(&mut self, values: &BTreeMap<String, String>);
    /// A committed change (name = `top.var`).
    fn change(&mut self, name: &str, value: &str);
    /// The time slot at `now` settled — flush accumulated changes.
    fn flush_slot(&mut self, now: u64);
    /// Simulation ended — persist/close.
    fn finish(&mut self) {}
}

enum Status {
    /// In the active FIFO or a wheel entry.
    Ready,
    /// Suspended on an edge/level wait list. `Thread` borrows the design's
    /// static thread sensitivity (the hot always-@ path: zero allocation);
    /// `Own` covers dynamic in-body WaitEdge lists.
    Wait(WaitList),
    /// Suspended at a fork...join — resumed when the group counter hits 0.
    /// The group id lives in `join_groups` (waiter pairing); the payload is
    /// not needed on the status itself.
    Join,
    Done,
}

#[derive(Clone)]
enum WaitList {
    Thread(u32),
    Own(Vec<SensItem>),
}

impl WaitList {
    fn items<'a>(&'a self, d: &'a Design) -> &'a [SensItem] {
        match self {
            WaitList::Thread(base) => &d.threads[*base as usize].sens,
            WaitList::Own(v) => v,
        }
    }
}

/// Body reference: usize::MAX = the thread's own body; else the func idx.
type BodyRef = usize;
const BODY_OWN: usize = usize::MAX;

/// Saved return context for a task/function call (design §3: per-activation
/// frames for `automatic`, per-instance shared frames for static).
#[derive(Clone)]
struct CallCtx {
    ret_body: BodyRef,
    ret_pc: u32,
    ret_dst: Option<Slot>,
    /// (resolved caller lvalue, callee formal slot) committed on return.
    outs: Vec<(Lval, u32)>,
    prev_shared: Option<usize>,
    pushed_frame: bool,
}

struct ThreadState {
    pc: u32,
    body: BodyRef,
    /// Design-thread index for BODY_OWN body lookup (fork branches share
    /// the spawner's design thread; runtime indices diverge from it).
    base: usize,
    /// Activation stack; base frame = the thread's own locals.
    frames: Vec<Vec<Bits>>,
    /// Current static-func shared frame (None = use frames.last()).
    shared: Option<usize>,
    calls: Vec<CallCtx>,
    /// Join group this branch belongs to (set at fork spawn; JoinDone
    /// decrements it).
    join_group: Option<u32>,
    status: Status,
}

enum Resume {
    /// Suspend into the inactive region (#0).
    Inactive,
    /// Suspend onto the time wheel.
    Wheel(u64),
    /// Keep executing / finished.
    None,
}

pub struct Kernel {
    d: std::rc::Rc<Design>,
    vars: Vec<Bits>,
    threads: Vec<ThreadState>,
    active: VecDeque<usize>,
    inactive: VecDeque<usize>,
    nba: Vec<(riv_ir::Lval, Bits)>,
    wheel: BTreeMap<u64, Vec<usize>>,
    now: u64,
    out: String,
    finished: Option<(u32, u32)>,
    pub dump_path: Option<String>,
    pub dump_open: bool,
    /// U-4: additive rVPI firing points (empty = zero cost).
    pub vpi_hooks: vpi::VpiHooks,
    /// end_of_simulation fired once (finish checkpoints are several).
    vpi_ended: bool,
    started: bool,
    plusargs: Vec<String>,
    /// `$monitor` request (None = never armed). `last: None` = armed, not
    /// yet displayed — the t0 arm displays on the first monitor pass.
    monitor: Option<MonitorReq>,
    monitor_enabled: bool,
    /// `$strobe` requests for the current slot (drained each monitor pass).
    strobes: Vec<StrobeReq>,
    /// Static task/function shared frames (design §3: per-instance shared
    /// storage — concurrent calls share it, pinned by unit test).
    shared_frames: Vec<Vec<Bits>>,
    /// `force` masks per var: 1-bits are forced. The stored value always
    /// carries the forced bits (force commits them); writes only land on
    /// unforced bits (17_force_bits: part-forces leave the rest writable).
    forces: Vec<Option<Bits>>,
    /// fork...join groups: (remaining branch count, waiter thread).
    join_groups: Vec<(u32, usize)>,
    /// Unpacked-array storage, parallel to `d.vars` (empty vec for
    /// non-array vars).
    arrays: Vec<Vec<Bits>>,
    /// Per-array (declared lsb, element count) for index mapping.
    array_dims: Vec<Option<(u64, u64)>>,
    /// Scheduled inertial gate updates: time → gate indices (a newer
    /// evaluation cancels the pending entry first).
    gate_due: BTreeMap<u64, Vec<usize>>,
    /// Pending (time, value) per gate; None = nothing scheduled.
    gate_pending: Vec<Option<(u64, Bits)>>,
    /// Delayed NBA commits (`q <= #10 d`): time → (lvalue, sampled value).
    nba_due: BTreeMap<u64, Vec<(Lval, Bits)>>,
    /// Reverse index (RK5 optimization round): var → threads currently in
    /// Status::Wait with that var in their sensitivity list. Wakes scan
    /// only the candidates instead of every thread.
    waiters: Vec<Vec<usize>>,
    /// var → gates watching it (input or output side).
    gate_watchers: Vec<Vec<usize>>,
    /// Pre-parsed Const literals (RK5 hot loop): per body (threads then
    /// funcs), per pc — a Const executes once per pass and must not
    /// re-parse its digit string every time.
    const_rt: Vec<Vec<Option<Bits>>>,
    /// Recycled wheel entry vectors (the wheel allocates one Vec per wake
    /// time without this pool).
    wheel_pool: Vec<Vec<usize>>,
    /// Open files for the $fopen family: descriptor → handle. Descriptors
    /// are 32-bit: MCD form = single bit (bit 0 = stdout), channel-FD form
    /// = 0x8000_0000 | (11 + n) — iverilog's measured allocation.
    files: Vec<(i32, Option<std::fs::File>)>,
    next_mcd_bit: u32,
    next_fd_n: u32,
    /// Dump enabled (between $dumpoff and $dumpon changes are not recorded).
    dump_on: bool,
    /// $random state (sequence divergence = rivD11).
    rng_seed: u32,
    /// rivvp -n: $stop behaves like $finish (rivD2).
    pub stop_as_finish: bool,
    /// Exit code when that -n/-N stop halts the run (rivvp -N = stop with
    /// exit 1, measured on vvp 13.0; default 0 keeps -n byte-compatible).
    pub stop_exit: i32,
    /// iverilog-shaped finish-banner suppression (ADR-0009 U-1: the
    /// rverilator face's goldens never carried the line — rivD8's
    /// drop-contract expressed at the API instead of text filtering).
    pub emit_finish_line: bool,
    /// `VCD info: dumpfile …` tool line (rivD8 same contract).
    pub emit_vcd_info_line: bool,
    /// $readmem* resolution base (USim drives the case dir; CLI cwd).
    pub file_base: std::path::PathBuf,
    /// wasm/browser injected files: name → contents, checked first.
    pub injected_files: std::collections::HashMap<String, String>,
    /// Verilator-form profile (ADR-0009): 2-state projection at the
    /// storage boundary — init x→0, every committed value x/z→0. The
    /// riverilog face keeps 4-state (false).
    pub two_state: bool,
    /// -n $stop halted the run: like natural exhaustion — NO finish
    /// banner (measured: iverilog -n prints only the stop-called line).
    stopped_silently: bool,
}

/// Unit→precision scale parsed from meta.timescale (unit fs / precision fs).
pub(crate) fn unit_ratio(ts: &str) -> Option<u128> {
    fn half_fems(h: &str) -> Option<u128> {
        let digits: String = h.chars().take_while(|c| c.is_ascii_digit()).collect();
        let rest = &h[digits.len()..];
        let m: u128 = digits.parse().ok()?;
        let mult: u128 = match rest {
            "s" => 1_000_000_000_000_000,
            "ms" => 1_000_000_000_000,
            "us" => 1_000_000_000,
            "ns" => 1_000_000,
            "ps" => 1_000,
            "fs" => 1,
            _ => return None,
        };
        Some(m * mult)
    }
    let (u, p) = ts.split_once('/')?;
    Some(half_fems(u.trim())? / half_fems(p.trim())?)
}

struct MonitorReq {
    thread: usize,
    fmt: u32,
    args: Vec<Arg>,
    /// pc range of the argument-producer ops (re-executed at render time
    /// so `$time` and variable reads are fresh).
    start: u32,
    end: u32,
    last: Option<Vec<Bits>>,
}

struct StrobeReq {
    thread: usize,
    fmt: u32,
    args: Vec<Arg>,
    start: u32,
    end: u32,
}

impl Kernel {
    pub fn new(design: Design) -> Result<Kernel, RunError> {
        let d = std::rc::Rc::new(design);
        let mut vars = Vec::with_capacity(d.vars.len());
        for v in &d.vars {
            let b = match &v.init {
                Some(lit) => from_numlit_parts(
                    Some(lit.width),
                    lit.signed,
                    match lit.base {
                        Base::Dec => VB::Dec,
                        Base::Bin => VB::Bin,
                        Base::Oct => VB::Oct,
                        Base::Hex => VB::Hex,
                    },
                    &lit.digits,
                )
                .map_err(|e| RunError(format!("var `{}` init: {}", v.name, e.0)))?,
                None => Bits::all_x(v.width).map_err(|e| RunError(e.0))?,
            };
            vars.push(b);
        }
        let mut shared_frames = Vec::with_capacity(d.funcs.len());
        for f in &d.funcs {
            let frame = (0..f.frame_len)
                .map(|_| Bits::all_x(1).map_err(|e| RunError(e.0)))
                .collect::<Result<Vec<_>, _>>()?;
            shared_frames.push(frame);
        }
        let forces = (0..d.vars.len()).map(|_| None).collect::<Vec<_>>();
        let mut arrays = Vec::with_capacity(d.vars.len());
        let mut array_dims = Vec::with_capacity(d.vars.len());
        for v in &d.vars {
            let dim = d.arrays.iter().find(|a| a.var == v.id).and_then(|a| a.dims.first().copied());
            match dim {
                Some((msb, lsb)) => {
                    let (lo, hi) = (msb.min(lsb) as u64, msb.max(lsb) as u64);
                    let n = (hi - lo + 1) as usize;
                    arrays.push((0..n).map(|_| Bits::all_x(v.width).unwrap()).collect());
                    array_dims.push(Some((lo, hi)));
                }
                None => {
                    arrays.push(Vec::new());
                    array_dims.push(None);
                }
            }
        }
        let gate_pending = (0..d.gates.len()).map(|_| None).collect::<Vec<_>>();
        let mut threads = Vec::with_capacity(d.threads.len());
        for t in &d.threads {
            threads.push(ThreadState {
                pc: 0,
                body: BODY_OWN,
                base: threads.len(),
                frames: vec![vec![Bits::zeros(1).map_err(|e| RunError(e.0))?; t.frame_len as usize]],
                shared: None,
                calls: Vec::new(),
                join_group: None,
                status: if t.sens.is_empty() {
                    Status::Ready
                } else {
                    // Edge-triggered always: armed before first execution
                    // (static sensitivity — the zero-allocation form).
                    Status::Wait(WaitList::Thread(threads.len() as u32))
                },
            });
        }
        let mut k = Kernel {
            threads,
            shared_frames,
            forces,
            join_groups: Vec::new(),
            arrays,
            array_dims,
            gate_due: BTreeMap::new(),
            gate_pending,
            nba_due: BTreeMap::new(),
            emit_finish_line: true,
            emit_vcd_info_line: true,
            two_state: false,
            file_base: std::path::PathBuf::new(),
            injected_files: std::collections::HashMap::new(),
            waiters: vec![Vec::new(); d.vars.len()],
            const_rt: {
                let n = d.threads.len() + d.funcs.len();
                let mut rt: Vec<Vec<Option<Bits>>> = vec![Vec::new(); n];
                let parse = |lit: &riv_ir::Literal| -> Option<Bits> {
                    from_numlit_parts(
                        Some(lit.width),
                        lit.signed,
                        match lit.base {
                            Base::Dec => VB::Dec,
                            Base::Bin => VB::Bin,
                            Base::Oct => VB::Oct,
                            Base::Hex => VB::Hex,
                        },
                        &lit.digits,
                    )
                    .ok()
                };
                for (i, t) in d.threads.iter().enumerate() {
                    rt[i] = t
                        .body
                        .iter()
                        .map(|op| match op {
                            Op::Const { lit, .. } => parse(lit),
                            _ => None,
                        })
                        .collect();
                }
                let nt = d.threads.len();
                for (i, f) in d.funcs.iter().enumerate() {
                    rt[nt + i] = f
                        .body
                        .iter()
                        .map(|op| match op {
                            Op::Const { lit, .. } => parse(lit),
                            _ => None,
                        })
                        .collect();
                }
                rt
            },
            wheel_pool: Vec::new(),
            files: Vec::new(),
            next_mcd_bit: 1,
            next_fd_n: 0,
            dump_on: true,
            rng_seed: 1,
            gate_watchers: {
                let mut w: Vec<Vec<usize>> = vec![Vec::new(); d.vars.len()];
                for (gi, g) in d.gates.iter().enumerate() {
                    w[g.out as usize].push(gi);
                    for i in &g.inputs {
                        w[*i as usize].push(gi);
                    }
                }
                w
            },
            stop_as_finish: false,
            stop_exit: 0,
            stopped_silently: false,
            started: false,
            vars,
            active: VecDeque::new(),
            inactive: VecDeque::new(),
            nba: Vec::new(),
            wheel: BTreeMap::new(),
            now: 0,
            out: String::new(),
            finished: None,
            dump_path: None,
            dump_open: false,
            vpi_hooks: vpi::VpiHooks::default(),
            vpi_ended: false,
            plusargs: Vec::new(),
            monitor: None,
            monitor_enabled: true,
            strobes: Vec::new(),
            d,
        };
        // Register the edge-triggered always threads' initial waits.
        let d0 = std::rc::Rc::clone(&k.d);
        for ti in 0..k.threads.len() {
            if let Status::Wait(wl) = &k.threads[ti].status {
                for it in wl.items(&d0) {
                    k.waiters[it.var as usize].push(ti);
                }
            }
        }
        Ok(k)
    }

    /// Arm a wait: park the thread and register it in the per-var waiter
    /// lists. A list equal to the thread's static sensitivity parks in the
    /// zero-allocation `Thread` form (the always-@ hot path).
    fn arm_wait(&mut self, ti: usize, items: &[SensItem]) {
        let base = self.threads[ti].base as u32;
        let static_form = self.d.threads[self.threads[ti].base].sens.as_slice() == items;
        let wl = if static_form {
            WaitList::Thread(base)
        } else {
            WaitList::Own(items.to_vec())
        };
        for it in items {
            if let Some(list) = self.waiters.get_mut(it.var as usize) {
                list.push(ti);
            }
        }
        self.threads[ti].status = Status::Wait(wl);
    }

    /// Remove a thread from every waiter list (it is leaving Wait). The
    /// status slot is replaced with Ready by the caller right after — here
    /// it is transiently replaced to release the borrow.
    fn disarm_wait(&mut self, ti: usize) {
        let d = std::rc::Rc::clone(&self.d);
        let wl = std::mem::replace(&mut self.threads[ti].status, Status::Ready);
        if let Status::Wait(wl) = &wl {
            for it in wl.items(&d) {
                if let Some(list) = self.waiters.get_mut(it.var as usize) {
                    list.retain(|&x| x != ti);
                }
            }
        }
        // Restore whatever the caller sets next; callers set Ready anyway.
        let _ = std::mem::replace(&mut self.threads[ti].status, wl);
    }

    /// Plusargs for `$test$plusargs`/`$value$plusargs` (+N=42 form).
    pub fn set_plusargs(&mut self, args: Vec<String>) {
        self.plusargs = args;
    }

    /// Verilator-form profile (ADR-0009): project all storage (vars,
    /// array elements) to 2-state — init x→0 included. Call once after
    /// construction, before run.
    pub fn project_two_state(&mut self) -> Result<(), RunError> {
        self.two_state = true;
        for v in self.vars.iter_mut() {
            *v = v.to_2state().map_err(|e| RunError(e.0))?;
        }
        for arr in self.arrays.iter_mut() {
            for el in arr.iter_mut() {
                *el = el.to_2state().map_err(|e| RunError(e.0))?;
            }
        }
        Ok(())
    }

    /// Take the accumulated output (backend seam; rivvp prints the same
    /// buffer via RunOutcome — do not use both paths for one run).
    pub fn take_output_buf(&mut self) -> String {
        std::mem::take(&mut self.out)
    }

    /// Simulation time in precision ticks.
    pub fn now_ticks(&self) -> u64 {
        self.now
    }

    /// U-5 slice-1: open the VCD dump without a TB `$dumpvars` — the
    /// injected-`$dumpvars`-at-t0 equivalent (legacy Sim::force_dump
    /// parity). Facades that always want waveforms call this before run.
    pub fn force_dump(&mut self, sink: &mut dyn DumpSink) {
        let _ = self.open_dump(sink);
    }

    /// `$readmem*` path resolution: injected files (wasm) by exact name
    /// first, then `file_base`-joined, then bare (cwd).
    pub fn resolve_readmem(&self, path: &str) -> Option<std::path::PathBuf> {
        if self.injected_files.contains_key(path) {
            return Some(std::path::PathBuf::from(path));
        }
        let p = std::path::Path::new(path);
        if p.is_absolute() {
            return Some(p.to_path_buf());
        }
        if !self.file_base.as_os_str().is_empty() {
            return Some(self.file_base.join(path));
        }
        Some(p.to_path_buf())
    }

    /// Run to completion; ready threads start in declaration order.
    pub fn run(&mut self, sink: &mut dyn DumpSink) -> RunOutcome {
        self.run_steps(sink, None)
    }

    /// U-5 slice-2 stepwise run (ADR-0007 parity with the legacy face):
    /// advance to absolute precision tick `t` — the slot AT `t` drains
    /// fully (legacy run_until contract) and the state stays resumable
    /// (deadline stop skips finish_out: no end hook, no VCD finish).
    /// Re-entry composes: the next run_until continues from `now`.
    pub fn run_until(
        &mut self,
        sink: &mut dyn DumpSink,
        t: u64,
    ) -> Result<RunOutcome, RunError> {
        if t <= self.now {
            return Err(RunError(format!(
                "run_until deadline {t} <= current time {}",
                self.now
            )));
        }
        Ok(self.run_steps(sink, Some(t)))
    }

    fn run_steps(&mut self, sink: &mut dyn DumpSink, deadline: Option<u64>) -> RunOutcome {
        // U-5 slice-2 stepwise: a put between run calls wakes threads to
        // Ready without a queue to land in — seed Ready threads on EVERY
        // entry (the slot-settled invariant means the queue was empty at
        // the previous return, so this can't double-queue a thread).
        for ti in 0..self.threads.len() {
            if matches!(self.threads[ti].status, Status::Ready) {
                self.active.push_back(ti);
            }
        }
        // The start hook and the t0 gate sweep run exactly once across
        // resume boundaries (stepwise or not).
        if !self.started {
            self.started = true;
            if let Some(mut f) = self.vpi_hooks.start_of_sim.take() {
                f(self, 0);
                self.vpi_hooks.start_of_sim = Some(f);
            }
            // Gates evaluate once at t0 (iverilog semantics): output
            // deltas schedule through the inertial wheel like any input
            // change.
            for gi in 0..self.d.gates.len() {
                self.eval_gate(gi);
            }
        }
        // Main slot loop (design §4).
        loop {
            loop {
                // 1. Active region: FIFO drain; commits wake into the tail.
                while let Some(ti) = self.active.pop_front() {
                    match self.step_thread(ti, sink) {
                        Ok(Resume::Inactive) => self.inactive.push_back(ti),
                        Ok(Resume::Wheel(at)) => {
                            let v = self
                                .wheel
                                .entry(at)
                                .or_insert_with(|| self.wheel_pool.pop().unwrap_or_default());
                            v.push(ti);
                        }
                        Ok(Resume::None) => {}
                        Err(e) => return self.finish_out(sink, Some(e)),
                    }
                    if self.finished.is_some() || self.stopped_silently {
                        return self.finish_out(sink, None);
                    }
                }
                // 2. Inactive region (#0 resumes).
                while let Some(ti) = self.inactive.pop_front() {
                    self.threads[ti].status = Status::Ready;
                    match self.step_thread(ti, sink) {
                        Ok(Resume::Inactive) => self.inactive.push_back(ti),
                        Ok(Resume::Wheel(at)) => {
                            let v = self
                                .wheel
                                .entry(at)
                                .or_insert_with(|| self.wheel_pool.pop().unwrap_or_default());
                            v.push(ti);
                        }
                        Ok(Resume::None) => {}
                        Err(e) => return self.finish_out(sink, Some(e)),
                    }
                    if self.finished.is_some() || self.stopped_silently {
                        return self.finish_out(sink, None);
                    }
                }
                // 3. NBA batch commit.
                let batch = std::mem::take(&mut self.nba);
                for (lv, val) in batch {
                    if let Err(e) = self.commit(&lv, val, sink) {
                        return self.finish_out(sink, Some(e));
                    }
                    if self.finished.is_some() || self.stopped_silently {
                        return self.finish_out(sink, None);
                    }
                }
                // 3.5 Gate updates due NOW (inertial commits). Commits can
                // wake threads (next inner iteration) and cascade into more
                // zero-delay gate updates — drain until stable.
                let mut cascades = 0usize;
                while let Some(gates) = self.gate_due.remove(&self.now) {
                    for gi in gates {
                        if let Err(e) = self.commit_gate(gi, sink) {
                            return self.finish_out(sink, Some(e));
                        }
                    }
                    cascades += 1;
                    if cascades > 100_000 {
                        return self.finish_out(sink, Some(RunError("gate cascade exceeds 100000 updates in one slot".into())));
                    }
                }
                // 4. Monitor region: $strobe (source order) then $monitor
                // (on-change, at most once per slot, t0 arm) — design §4.
                if let Err(e) = self.run_monitor_region(sink) {
                    return self.finish_out(sink, Some(e));
                }
                if self.active.is_empty()
                    && self.inactive.is_empty()
                    && self.nba.is_empty()
                    && !self.gate_due.contains_key(&self.now)
                {
                    break;
                }
            }
            // Slot settled: flush dump changes under this time.
            sink.flush_slot(self.now);
            // Advance the wheel: threads, gate updates and delayed NBAs
            // all participate. Updates due AT the new time land BEFORE the
            // processes woken there (iverilog event order: the update was
            // scheduled earlier, so it runs first — 20_gate_delays pins
            // a display at the exact settle time).
            let next_thread = self.wheel.keys().next().copied();
            let next_gate = self.gate_due.keys().next().copied();
            let next_nba = self.nba_due.keys().next().copied();
            let Some(at) = [next_thread, next_gate, next_nba]
                .into_iter()
                .flatten()
                .min()
            else {
                if let Some(t) = deadline {
                    if t > self.now {
                        // Stepwise deadline with an externally-driven clock
                        // (no future events scheduled): the deadline IS the
                        // schedule point — advance `now` to it and hand the
                        // state back to the caller (spec layer's lockstep
                        // halves; legacy sched_loop did the same).
                        self.now = t;
                        return RunOutcome {
                            output: std::mem::take(&mut self.out),
                            exit: 0,
                            finished: false,
                        };
                    }
                }
                break; // no more events: natural stop (exit 0)
            };
            if deadline.is_some_and(|t| at > t) {
                // U-5 slice-2: the slot at the deadline drained; stop BEFORE
                // advancing past it. No finish_out — the stepwise caller
                // resumes (finished=false, exit=0, output handed over).
                return RunOutcome {
                    output: std::mem::take(&mut self.out),
                    exit: 0,
                    finished: false,
                };
            }
            self.now = at;
            // U-4: time-wheel advance fires the embedder's AfterDelay
            // timers (before the slots at this time execute).
            if let Some(mut f) = self.vpi_hooks.after_delay.take() {
                f(self, at);
                self.vpi_hooks.after_delay = Some(f);
                // U-4 slice-2: a timer callback's finish request
                // (vpi_control(vpiFinish) analog, UsimHost::finish) lands
                // between time steps — honor it before the slots run, so
                // the stop short-circuits this time step entirely (the
                // legacy face's fire_due_timers → finish does the same).
                if self.stop_as_finish {
                    self.stopped_silently = true;
                    return self.finish_out(sink, None);
                }
            }
            if let Some(mut gates) = self.gate_due.remove(&at) {
                while let Some(gi) = gates.pop() {
                    if let Err(e) = self.commit_gate(gi, sink) {
                        return self.finish_out(sink, Some(e));
                    }
                }
            }
            if let Some(updates) = self.nba_due.remove(&at) {
                for (lv, val) in updates {
                    if let Err(e) = self.commit(&lv, val, sink) {
                        return self.finish_out(sink, Some(e));
                    }
                }
            }
            if next_thread == Some(at) {
                let (_, mut wake) = self.wheel.pop_first().unwrap();
                for ti in wake.drain(..) {
                    self.threads[ti].status = Status::Ready;
                    self.active.push_back(ti);
                }
                if self.wheel_pool.len() < 64 {
                    self.wheel_pool.push(wake);
                }
            }
        }
        self.finish_out(sink, None)
    }

    fn finish_out(&mut self, sink: &mut dyn DumpSink, err: Option<RunError>) -> RunOutcome {
        sink.finish();
        if !self.vpi_ended {
            self.vpi_ended = true;
            if let Some(mut f) = self.vpi_hooks.end_of_sim.take() {
                f(self, 0);
                self.vpi_hooks.end_of_sim = Some(f);
            }
        }
        let exit = match err {
            // -n/-N stop: the host picks the exit (rivvp -N exits 1).
            None if self.stopped_silently => self.stop_exit,
            None => 0,
            Some(e) => {
                self.out.push_str(&format!("rivvp: runtime error: {}\n", e.0));
                1
            }
        };
        if self.stopped_silently {
            self.finished = None;
        }
        if let Some((file, line)) = self.finished {
            if !self.emit_finish_line {
                self.finished = None;
                return RunOutcome {
                    output: std::mem::take(&mut self.out),
                    exit,
                    finished: true,
                };
            }
            let file = self
                .d
                .strs
                .get(file as usize)
                .and_then(|p| p.first())
                .map(|p| match p {
                    FmtPiece::Text(t) => t.clone(),
                    _ => String::new(),
                })
                .unwrap_or_default();
            self.out.push_str(&format!(
                "{file}:{line}: $finish called at {} ({})\n",
                self.now, self.d.meta.precision
            ));
        }
        RunOutcome {
            output: std::mem::take(&mut self.out),
            exit,
            finished: true,
        }
    }

    // ------------------------------------------------------------------
    // Thread execution
    // ------------------------------------------------------------------

    fn step_thread(&mut self, ti: usize, sink: &mut dyn DumpSink) -> Result<Resume, RunError> {
        // The design is immutable at run time; a local Rc handle lets the
        // op be borrowed (no per-step clone — the RK5 hot loop) while the
        // thread state mutates.
        let d = std::rc::Rc::clone(&self.d);
        loop {
            // Re-read per iteration: Call/Ret switch the body ref.
            let body_len = match self.threads[ti].body {
                BODY_OWN => d.threads[self.threads[ti].base].body.len() as u32,
                f => d.funcs[f].body.len() as u32,
            };
            let pc = self.threads[ti].pc;
            if pc >= body_len {
                self.finish_pass(ti);
                return Ok(Resume::None);
            }
            let op: &Op = match self.threads[ti].body {
                BODY_OWN => &d.threads[self.threads[ti].base].body[pc as usize],
                f => &d.funcs[f].body[pc as usize],
            };
            self.threads[ti].pc = pc + 1;
            match *op {
                Op::Const { dst, ref lit } => {
                    // Pre-parsed at load (the digit string never changes).
                    let bk = match self.threads[ti].body {
                        BODY_OWN => self.threads[ti].base,
                        f => d.threads.len() + f,
                    };
                    let b = match &self.const_rt[bk][pc as usize] {
                        Some(b) => b.clone(),
                        None => vl_ops::parse_const(lit)
                            .map_err(|e| RunError(format!("const: {}", e.0)))?,
                    };
                    self.set_slot(ti, dst, b);
                }
                Op::Mov { dst, src } => {
                    let v = self.slot(ti, src)?.clone();
                    self.set_slot(ti, dst, v);
                }
                Op::Bin { kind, dst, a, b, w, signed } => {
                    let av = self.slot(ti, a)?.clone();
                    let bv = self.slot(ti, b)?.clone();
                    let r = vl_ops::apply_bin(kind, &av, &bv, w, signed)
                        .map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::Bit { kind, dst, a, b, w, signed } => {
                    let av = self.slot(ti, a)?.clone();
                    let bv = self.slot(ti, b)?.clone();
                    let r = vl_ops::apply_bit(kind, &av, &bv, w, signed)
                        .map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::BitXnor { dst, a, b, w } => {
                    let av = self.slot(ti, a)?.clone();
                    let bv = self.slot(ti, b)?.clone();
                    let r = vl_ops::apply_bit_xnor(&av, &bv, w).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::BitNot { dst, a, w, signed } => {
                    let av = self.slot(ti, a)?.clone();
                    let r = vl_ops::apply_bit_not(&av, w, signed).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::Reduce { kind, dst, a } => {
                    let av = self.slot(ti, a)?.clone();
                    let r = vl_ops::apply_reduce(kind, &av).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::LogicAnd { dst, a, b } => {
                    let av = self.slot(ti, a)?.clone();
                    let bv = self.slot(ti, b)?.clone();
                    let r = vl_ops::apply_logic_and(&av, &bv).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::LogicOr { dst, a, b } => {
                    let av = self.slot(ti, a)?.clone();
                    let bv = self.slot(ti, b)?.clone();
                    let r = vl_ops::apply_logic_or(&av, &bv).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::LogicNot { dst, a } => {
                    let av = self.slot(ti, a)?.clone();
                    let r = vl_ops::apply_logic_not(&av).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::Cmp { kind, dst, a, b, signed } => {
                    let av = self.slot(ti, a)?.clone();
                    let bv = self.slot(ti, b)?.clone();
                    let r = vl_ops::apply_cmp(kind, &av, &bv, signed).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::CaseCmp { kind, dst, a, b } => {
                    let av = self.slot(ti, a)?.clone();
                    let bv = self.slot(ti, b)?.clone();
                    let r = vl_ops::apply_case_cmp(kind, &av, &bv).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::Shift { kind, dst, a, b, w, signed } => {
                    let av = self.slot(ti, a)?.clone();
                    let bv = self.slot(ti, b)?.clone();
                    let r = vl_ops::apply_shift(kind, &av, &bv, w, signed)
                        .map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::Sel { dst, cond, a, b } => {
                    let cv = self.slot(ti, cond)?.clone();
                    let av = self.slot(ti, a)?.clone();
                    let bv = self.slot(ti, b)?.clone();
                    let r = vl_ops::apply_sel(&cv, &av, &bv).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::Concat { dst, ref parts } => {
                    let vals: Vec<Bits> = parts
                        .iter()
                        .map(|p| self.slot(ti, *p).cloned())
                        .collect::<Result<_, _>>()?;
                    let r = vl_ops::apply_concat(&vals).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::Replicate { dst, src, times } => {
                    let base = self.slot(ti, src)?.clone();
                    let r = vl_ops::apply_replicate(&base, times).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::LoadVar { dst, var } => {
                    let v = self.read_var(var)?.clone();
                    self.set_slot(ti, dst, v);
                }
                Op::LoadBit { dst, var, idx } => {
                    let iv = self.slot(ti, idx)?.clone();
                    let r = self.load_bit(var, &iv)?;
                    self.set_slot(ti, dst, r);
                }
                Op::LoadPart { dst, var, hi, lo } => {
                    let vv = self.read_var(var)?.clone();
                    let r = Bits::part(&vv, hi, lo).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::LoadPartDyn { dst, var, hi, lo } => {
                    let hv = self.slot(ti, hi)?.clone();
                    let lv = self.slot(ti, lo)?.clone();
                    let vv = self.read_var(var)?.clone();
                    let r = dyn_part(&vv, &hv, &lv)?;
                    self.set_slot(ti, dst, r);
                }
                Op::LoadElem { dst, var, idx } => {
                    let iv = self.slot(ti, idx)?.clone();
                    let r = self.load_elem(var, &iv)?;
                    self.set_slot(ti, dst, r);
                }
                Op::AssignBlk { ref lv, src } => {
                    let val = self.slot(ti, src)?.clone();
                    let resolved = self.resolve_lvalue(ti, lv)?;
                    self.commit(&resolved, val, sink)?;
                }
                Op::AssignNba { ref lv, src } => {
                    let val = self.slot(ti, src)?.clone();
                    let resolved = self.resolve_lvalue(ti, lv)?;
                    self.nba.push((resolved, val));
                }
                Op::AssignNbaDelay { ref lv, src, amount } => {
                    let val = self.slot(ti, src)?.clone();
                    let resolved = self.resolve_lvalue(ti, lv)?;
                    let av = self.slot(ti, amount)?.clone();
                    if av.has_unknown() {
                        return Err(RunError("delayed NBA amount is unknown (x)".into()));
                    }
                    let at = self.now.saturating_add(av.to_u128_lossy().min(u64::MAX as u128) as u64);
                    self.nba_due.entry(at).or_default().push((resolved, val));
                }
                Op::Branch { cond, then, els } => {
                    let cv = self.slot(ti, cond)?.clone();
                    let t = cv.truthy().map_err(|e| RunError(e.0))?;
                    self.threads[ti].pc = match t {
                        Some(true) => then,
                        Some(false) | None => els,
                    };
                }
                Op::Jump { to } => {
                    self.threads[ti].pc = to;
                }
                Op::Delay { amount } => {
                    let av = self.slot(ti, amount)?.clone();
                    if av.has_unknown() {
                        return Err(RunError(format!(
                            "delay amount is unknown (x) at time {}",
                            self.now
                        )));
                    }
                    let ticks = av.to_u128_lossy();
                    if ticks == 0 {
                        return Ok(Resume::Inactive);
                    }
                    let at = self.now.saturating_add(ticks.min(u64::MAX as u128) as u64);
                    return Ok(Resume::Wheel(at));
                }
                Op::ScaleTime { dst, src, num, den } => {
                    let sv = self.slot(ti, src)?.clone();
                    if sv.has_unknown() {
                        return Err(RunError("time expression is unknown (x)".into()));
                    }
                    let v = sv.to_u128_lossy();
                    let scaled = (v * num as u128 + (den as u128) / 2) / den as u128;
                    let r =
                        Bits::from_u128(64, scaled.min(u128::from(u64::MAX))).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::WaitEdge { ref items } => {
                    self.arm_wait(ti, items);
                    return Ok(Resume::None);
                }
                Op::SysCall { id, fmt, ref args, dst, file, line, start } => match id {
                    SysId::Display | SysId::Write => {
                        let text = self.render(fmt, &args, ti)?;
                        self.out.push_str(&text);
                        if id == SysId::Display {
                            self.out.push('\n');
                        }
                    }
                    SysId::Finish => {
                        self.finished = Some((file, line));
                        self.threads[ti].status = Status::Done;
                        return Ok(Resume::None);
                    }
                    SysId::Dumpfile => {
                        let path = fmt
                            .and_then(|f| self.d.strs.get(f as usize))
                            .and_then(|p| p.first())
                            .map(|p| match p {
                                FmtPiece::Text(t) => t.clone(),
                                _ => String::new(),
                            })
                            .unwrap_or_else(|| "dump.vcd".to_string());
                        self.dump_path = Some(path);
                    }
                    SysId::Dumpvars => {
                        self.open_dump(sink)?;
                    }
                    SysId::Clog2 => {
                        let Some(d) = dst else {
                            return Err(RunError("clog2 without destination".into()));
                        };
                        let Some(arg) = args.first() else {
                            return Err(RunError("clog2 without argument".into()));
                        };
                        let v = self.slot(ti, arg.slot)?.clone();
                        let c = v.clog2().map_err(|e| RunError(e.0))?;
                        self.set_slot(ti, d, Bits::from_u64(32, c).unwrap());
                    }
                    SysId::Time => {
                        // $time = simulated time scaled to the module's time
                        // unit, rounded (IEEE §17.7.1); the golden pins
                        // %0d of $time = unit ticks (24: 23) while %t
                        // re-scales to precision ticks (23000).
                        let num = unit_ratio(&self.d.meta.timescale).ok_or_else(|| {
                            RunError(format!("bad timescale `{}`", self.d.meta.timescale))
                        })? as u64;
                        let unit_ticks = (self.now + num / 2) / num.max(1);
                        let Some(d) = dst else {
                            return Err(RunError("$time without destination".into()));
                        };
                        self.set_slot(ti, d, Bits::from_u64(64, unit_ticks).unwrap());
                    }
                    SysId::Monitor => {
                        self.monitor = Some(MonitorReq {
                            thread: ti,
                            fmt: fmt.ok_or_else(|| RunError("monitor without format".into()))?,
                            args: args.clone(),
                            start,
                            end: pc,
                            last: None,
                        });
                        self.monitor_enabled = true;
                    }
                    SysId::Stop => {
                        // rivD2, bytes measured on iverilog 13.0 (vvp,
                        // stdin=/dev/null): the stop-called line, then the
                        // four banner lines, then simulation CONTINUES
                        // (EOF = continue). -n makes $stop ≡ $finish (the
                        // stop-called line only, then halt).
                        let file_name = self
                            .d
                            .strs
                            .get(file as usize)
                            .and_then(|p| p.first())
                            .map(|p| match p {
                                FmtPiece::Text(t) => t.clone(),
                                _ => String::new(),
                            })
                            .unwrap_or_default();
                        self.out.push_str(&format!(
                            "{file_name}:{line}: $stop called at {} ({})\n",
                            self.now, self.d.meta.precision
                        ));
                        if self.stop_as_finish {
                            self.stopped_silently = true;
                            self.threads[ti].status = Status::Done;
                            return Ok(Resume::None);
                        }
                        self.out.push_str(&format!(
                            "** VVP Stop(0) **\n** Flushing output streams.\n** Current simulation time is {} ticks.\n** Continue **\n",
                            self.now
                        ));
                    }
                    SysId::ReadmemH | SysId::ReadmemB => {
                        let path = fmt
                            .and_then(|f| self.d.strs.get(f as usize))
                            .and_then(|p| p.first())
                            .map(|p| match p {
                                FmtPiece::Text(t) => t.clone(),
                                _ => String::new(),
                            })
                            .unwrap_or_default();
                        let var = args.first().map(|a| a.slot as VarId).ok_or_else(|| {
                            RunError("readmem without a memory argument".into())
                        })?;
                        let mut bounds: Vec<Option<u64>> = Vec::new();
                        for a in args[1..3.min(args.len())].iter() {
                            if a.width == 0 {
                                bounds.push(None);
                            } else {
                                let v = self.slot(ti, a.slot)?.clone();
                                if v.has_unknown() {
                                    return Err(RunError("readmem bound is unknown".into()));
                                }
                                bounds.push(Some(v.to_u128_lossy() as u64));
                            }
                        }
                        let hex = id == SysId::ReadmemH;
                        let task = if hex { "$readmemh" } else { "$readmemb" };
                        self.run_readmem(hex, &path, var, bounds, sink, task, file, line)?;
                    }
                    SysId::Dumpall => {
                        if self.dump_open {
                            let values = self.dump_values(true);
                            sink.dump_section(self.now, "dumpall", &values);
                        }
                    }
                    SysId::Dumpoff => {
                        self.dump_on = false;
                        if self.dump_open {
                            let values = self.dump_values_x();
                            sink.dump_section(self.now, "dumpoff", &values);
                        }
                    }
                    SysId::Dumpon => {
                        self.dump_on = true;
                        if self.dump_open {
                            let values = self.dump_values(true);
                            sink.dump_section(self.now, "dumpon", &values);
                        }
                    }
                    SysId::Fopen => {
                        let Some(d) = dst else {
                            return Err(RunError("$fopen without destination".into()));
                        };
                        let path = fmt
                            .and_then(|f| self.d.strs.get(f as usize))
                            .and_then(|p| p.first())
                            .map(|p| match p {
                                FmtPiece::Text(t) => t.clone(),
                                _ => String::new(),
                            })
                            .unwrap_or_default();
                        let mode_w = args.first().map(|a| a.width == 1).unwrap_or(false);
                        let key: i32 = if mode_w {
                            // iverilog channel-FD form (measured): first
                            // "w" file = 0x8000_000B, incrementing.
                            let k = (0x8000_0000u32 | (11 + self.next_fd_n)) as i32;
                            self.next_fd_n += 1;
                            k
                        } else {
                            let k = 1i32 << self.next_mcd_bit;
                            self.next_mcd_bit += 1;
                            if self.next_mcd_bit >= 31 {
                                return Err(RunError("too many open files (MCD bits exhausted)".into()));
                            }
                            k
                        };
                        let f = std::fs::File::create(&path)
                            .map_err(|e| RunError(format!("$fopen `{path}`: {e}")))?;
                        self.files.push((key, Some(f)));
                        let v = Bits::from_u64(32, key as u32 as u64).unwrap();
                        self.set_slot(ti, d, v);
                    }
                    SysId::Fclose => {
                        let fd = self
                            .slot(ti, args.first().map(|a| a.slot).unwrap_or(0))?
                            .clone();
                        if fd.has_unknown() {
                            return Err(RunError("$fclose descriptor is unknown".into()));
                        }
                        let key = fd.to_u128_lossy() as u32 as i32;
                        for (k, f) in self.files.iter_mut() {
                            if *k == key {
                                *f = None; // dropped here
                            }
                        }
                        self.files.retain(|(k, f)| *k != key && f.is_some());
                    }
                    SysId::Fwrite | SysId::Fdisplay => {
                        let text = self.render(
                            fmt,
                            &args[1.min(args.len())..],
                            ti,
                        )?;
                        let fd = self.slot(ti, args.first().map(|a| a.slot).unwrap_or(0))?.clone();
                        if fd.has_unknown() {
                            return Err(RunError("file descriptor is unknown".into()));
                        }
                        let key = fd.to_u128_lossy() as u32;
                        // Channel-FD form (high bit set, iverilog measured):
                        // writes ONLY the file behind the FD — the stdout
                        // bit does not apply (`fd | 1` stays file-only).
                        // MCD form: every set bit writes, bit 0 = stdout.
                        let fd_form = key & 0x8000_0000 != 0;
                        let write_file = |f: &mut Option<std::fs::File>, text: &str| -> Result<(), RunError> {
                            use std::io::Write;
                            if let Some(f) = f.as_mut() {
                                let mut buf = text.to_string();
                                if id == SysId::Fdisplay {
                                    buf.push('\n');
                                }
                                f.write_all(buf.as_bytes())
                                    .map_err(|e| RunError(format!("$fwrite: {e}")))?;
                            }
                            Ok(())
                        };
                        if fd_form {
                            for (k, f) in self.files.iter_mut() {
                                if *k as u32 == key {
                                    write_file(f, &text)?;
                                }
                            }
                        } else {
                            if key & 1 == 1 {
                                self.out.push_str(&text);
                                if id == SysId::Fdisplay {
                                    self.out.push('\n');
                                }
                            }
                            for (k, f) in self.files.iter_mut() {
                                if *k != 1 && key & *k as u32 != 0 {
                                    write_file(f, &text)?;
                                }
                            }
                        }
                    }
                    SysId::Random => {
                        let Some(d) = dst else {
                            return Err(RunError("$random without destination".into()));
                        };
                        let mut x = self.rng_seed;
                        if let Some(a) = args.first() {
                            if a.width > 0 {
                                let sv = self.slot(ti, a.slot)?.clone();
                                x = if sv.has_unknown() {
                                    1
                                } else {
                                    sv.to_u128_lossy() as u32
                                };
                            }
                        }
                        x = x.wrapping_mul(1103515245).wrapping_add(12345);
                        self.rng_seed = x;
                        if let Some(a) = args.first() {
                            if a.width > 0 {
                                self.set_slot(ti, a.slot, Bits::from_u64(32, x as u64).unwrap());
                            }
                        }
                        let v = Bits::from_u64(32, x as u64).unwrap();
                        // 32-bit signed rendering ($random returns signed).
                        self.set_slot(ti, d, v);
                    }
                    SysId::Trigger => {
                        let ev = args.first().map(|a| a.slot as VarId).ok_or_else(|| {
                            RunError("-> trigger without an event".into())
                        })?;
                        self.fire_event(ev)?;
                    }
                    SysId::MonitorOff => self.monitor_enabled = false,
                    SysId::MonitorOn => self.monitor_enabled = true,
                    SysId::Strobe => {
                        self.strobes.push(StrobeReq {
                            thread: ti,
                            fmt: fmt.ok_or_else(|| RunError("strobe without format".into()))?,
                            args: args.clone(),
                            start,
                            end: pc,
                        });
                    }
                    SysId::TestPlusargs => {
                        let pat = fmt
                            .and_then(|f| self.d.strs.get(f as usize))
                            .and_then(|p| p.first())
                            .map(|p| match p {
                                FmtPiece::Text(t) => t.clone(),
                                _ => String::new(),
                            })
                            .unwrap_or_default();
                        let hit = self
                            .plusargs
                            .iter()
                            .any(|pa| pa.trim_start_matches('+').starts_with(&pat));
                        let Some(d) = dst else {
                            return Err(RunError("$test$plusargs without destination".into()));
                        };
                        self.set_slot(ti, d, Bits::from_u64(1, hit as u64).unwrap());
                    }
                    SysId::ValuePlusargs => {
                        let pat = fmt
                            .and_then(|f| self.d.strs.get(f as usize))
                            .and_then(|p| p.first())
                            .map(|p| match p {
                                FmtPiece::Text(t) => t.clone(),
                                _ => String::new(),
                            })
                            .unwrap_or_default();
                        let Some(d) = dst else {
                            return Err(RunError("$value$plusargs without destination".into()));
                        };
                        let parsed = self.parse_plusarg(&pat);
                        match parsed {
                            Some((value, out_w)) => {
                                let Some(a) = args.first() else {
                                    return Err(RunError(
                                        "$value$plusargs without an output variable".into(),
                                    ));
                                };
                                let v = Bits::from_u128(out_w.max(1), value)
                                    .unwrap_or_else(|_| Bits::zeros(out_w.max(1)).unwrap());
                                self.set_slot(ti, a.slot, v);
                                self.set_slot(ti, d, Bits::from_u64(1, 1).unwrap());
                            }
                            None => {
                                self.set_slot(ti, d, Bits::from_u64(1, 0).unwrap());
                            }
                        }
                    }
                },
                Op::Call { func, ref ins, ref outs, dst } => {
                    let fi = func as usize;
                    let flen = self.d.funcs[fi].frame_len;
                    let automatic = self.d.funcs[fi].automatic;
                    // Resolve caller lvalues now (dynamic indices are read
                    // in the CALLER's frame), copy args into the callee's
                    // formal slots, switch context.
                    let mut resolved_outs = Vec::with_capacity(outs.len());
                    for (lv, fs) in outs.iter() {
                        let r = self.resolve_lvalue(ti, lv)?;
                        resolved_outs.push((r, *fs));
                    }
                    // Copy args BEFORE switching frames: the caller slots
                    // live in the caller's current frame (nested static
                    // calls share different frames — reading them after the
                    // switch hit the callee's frame and produced x/garbage).
                    let mut in_vals = Vec::with_capacity(ins.len());
                    for (a, fs) in ins.iter() {
                        let v = self.slot(ti, *a)?.clone();
                        in_vals.push((v, *fs));
                    }
                    let prev_shared = self.threads[ti].shared;
                    let pushed_frame;
                    if automatic {
                        self.threads[ti].frames.push(
                            vec![Bits::zeros(1).unwrap(); flen as usize],
                        );
                        self.threads[ti].shared = None;
                        pushed_frame = true;
                    } else {
                        self.threads[ti].shared = Some(fi);
                        pushed_frame = false;
                    }
                    for (v, fs) in in_vals {
                        self.set_slot(ti, fs, v);
                    }
                    let ctx = CallCtx {
                        ret_body: self.threads[ti].body,
                        ret_pc: pc + 1, // the op AFTER the call in the caller
                        ret_dst: dst,
                        outs: resolved_outs,
                        prev_shared,
                        pushed_frame,
                    };
                    self.threads[ti].calls.push(ctx);
                    self.threads[ti].body = fi;
                    self.threads[ti].pc = 0;
                }
                Op::Ret { src } => {
                    let ret_val = self.slot(ti, src)?.clone();
                    let ctx = self.threads[ti].calls.pop().ok_or_else(|| {
                        RunError("ret outside a task/function".into())
                    })?;
                    let outs = ctx.outs.clone();
                    let out_vals: Vec<(Lval, Bits)> = outs
                        .iter()
                        .map(|(lv, fs)| Ok((lv.clone(), self.slot(ti, *fs)?.clone())))
                        .collect::<Result<_, RunError>>()?;
                    // Restore caller context.
                    if ctx.pushed_frame {
                        self.threads[ti].frames.pop();
                    }
                    self.threads[ti].shared = ctx.prev_shared;
                    self.threads[ti].body = ctx.ret_body;
                    self.threads[ti].pc = ctx.ret_pc;
                    if let Some(d) = ctx.ret_dst {
                        self.set_slot(ti, d, ret_val);
                    }
                    for (lv, val) in out_vals {
                        self.commit(&lv, val, sink)?;
                    }
                }
                Op::AssignForce { ref lv, src } => {
                    let val = self.slot(ti, src)?.clone();
                    let var = match lv {
                        Lval::Var { var }
                        | Lval::Bit { var, .. }
                        | Lval::Part { var, .. }
                        | Lval::PartDyn { var, .. } => *var,
                        Lval::Elem { .. } => {
                            return Err(RunError("force on array elements is not supported".into()))
                        }
                        Lval::Concat { .. } => {
                            return Err(RunError("force on a concatenation is not supported".into()))
                        }
                    };
                    let vi = d.vars[var as usize].clone();
                    let forced = if val.width == vi.width {
                        val
                    } else if val.width > vi.width {
                        val.trunc(vi.width).map_err(|e| RunError(e.0))?
                    } else {
                        val.uadjust(vi.width).map_err(|e| RunError(e.0))?
                    };
                    // Mask = the bits this lvalue selects (whole var, part,
                    // or bit); storage receives the forced bits (change
                    // events propagate to followers).
                    let (mask, patched) = match lv {
                        Lval::Var { .. } => {
                            let m = all_ones(vi.width)?;
                            (m.clone(), forced)
                        }
                        Lval::Part { hi, lo, .. } => {
                            let (hi, lo) = (*hi, *lo);
                            let w = hi.abs_diff(lo) + 1;
                            let cur = self.var(var)?.clone();
                            let m = mask_of(vi.width, hi, lo)?;
                            let patched = Bits::set_part(&cur, hi, lo, &forced.trunc(w).map_err(|e| RunError(e.0))?)
                                .map_err(|e| RunError(e.0))?;
                            (m, patched)
                        }
                        Lval::Bit { idx, .. } => {
                            let iv = self.slot(ti, *idx)?.clone();
                            match self.abs_index(var, &iv)? {
                                Some(abs) => {
                                    let cur = self.var(var)?.clone();
                                    let m = mask_of(vi.width, abs, abs)?;
                                    let patched = Bits::set_part(&cur, abs, abs, &forced.trunc(1).map_err(|e| RunError(e.0))?)
                                        .map_err(|e| RunError(e.0))?;
                                    (m, patched)
                                }
                                None => {
                                    let m = Bits::zeros(vi.width).map_err(|e| RunError(e.0))?;
                                    (m, self.var(var)?.clone())
                                }
                            }
                        }
                        Lval::PartDyn { hi, lo, .. } => {
                            let hv = self.slot(ti, *hi)?.clone();
                            let lv2 = self.slot(ti, *lo)?.clone();
                            if hv.has_unknown() || lv2.has_unknown() {
                                let m = Bits::zeros(vi.width).map_err(|e| RunError(e.0))?;
                                (m, self.var(var)?.clone())
                            } else {
                                let (h, l) = (hv.to_u128_lossy() as u32, lv2.to_u128_lossy() as u32);
                                let (hi2, lo2) = (h.max(l), h.min(l));
                                let w = hi2.abs_diff(lo2) + 1;
                                let cur = self.var(var)?.clone();
                                let m = mask_of(vi.width, hi2, lo2)?;
                                let patched = Bits::set_part(&cur, hi2, lo2, &forced.trunc(w).map_err(|e| RunError(e.0))?)
                                    .map_err(|e| RunError(e.0))?;
                                (m, patched)
                            }
                        }
                        _ => unreachable!(" Elem/Concat rejected above"),
                    };
                    self.store_var_raw(var, patched, sink)?;
                    self.forces[var as usize] = Some(mask);
                }
                Op::Release { ref lv } => {
                    let var = match lv {
                        Lval::Var { var }
                        | Lval::Bit { var, .. }
                        | Lval::Part { var, .. }
                        | Lval::PartDyn { var, .. } => *var,
                        Lval::Elem { .. } => {
                            return Err(RunError("release on array elements is not supported".into()))
                        }
                        Lval::Concat { .. } => {
                            return Err(RunError("release on a concatenation is not supported".into()))
                        }
                    };
                    let is_net = d.vars[var as usize].kind == VarKind::Net;
                    self.forces[var as usize] = None;
                    if is_net {
                        // Release on a net: the driver re-takes the wire —
                        // poke its cont-assign threads awake to re-evaluate
                        // (16_force_net: released y follows ~a).
                        let threads: Vec<usize> = self
                            .d
                            .assigns
                            .iter()
                            .filter(|(_, v)| *v == var)
                            .map(|(t, _)| *t as usize)
                            .collect();
                        for t in threads {
                            if matches!(self.threads[t].status, Status::Wait(_)) {
                                self.disarm_wait(t);
                                self.threads[t].status = Status::Ready;
                                self.threads[t].pc = 0; // re-run from the top
                                self.active.push_back(t);
                            }
                        }
                    }
                    // Regs keep the forced value (04_force golden).
                }
                Op::Fork { ref branches, join } => {
                    if branches.is_empty() {
                        self.threads[ti].pc = join;
                    } else {
                        let group = self.join_groups.len() as u32;
                        self.join_groups.push((branches.len() as u32, ti));
                        // Snapshot the parent context to clone per branch.
                        let parent_frames = self.threads[ti].frames.clone();
                        let parent_shared = self.threads[ti].shared;
                        let parent_calls = self.threads[ti].calls.clone();
                        let parent_body = self.threads[ti].body;
                        let mut spawn: Vec<ThreadState> = Vec::with_capacity(branches.len());
                        for bpc in branches.iter() {
                            spawn.push(ThreadState {
                                pc: *bpc,
                                body: parent_body,
                                base: self.threads[ti].base,
                                frames: parent_frames.clone(),
                                shared: parent_shared,
                                calls: parent_calls.clone(),
                                join_group: Some(group),
                                status: Status::Ready,
                            });
                        }
                        for st in spawn {
                            self.threads.push(st);
                            self.active.push_back(self.threads.len() - 1);
                        }
                        self.threads[ti].pc = join;
                        let _ = group;
                        self.threads[ti].status = Status::Join;
                        return Ok(Resume::None);
                    }
                }
                Op::JoinDone => {
                    let Some(group) = self.threads[ti].join_group else {
                        return Err(RunError("joindone outside a fork branch".into()));
                    };
                    let (count, waiter) = self
                        .join_groups
                        .get_mut(group as usize)
                        .ok_or_else(|| RunError("joindone without a join group".into()))?;
                    *count -= 1;
                    if *count == 0 {
                        let waiter = *waiter;
                        self.threads[waiter].status = Status::Ready;
                        self.active.push_back(waiter);
                    }
                    self.threads[ti].status = Status::Done;
                    return Ok(Resume::None);
                }
                Op::JoinPoint => {
                    // Executed only if the fork had no branches or on a
                    // spurious resume — a no-op.
                }
                Op::EndThread => {
                    self.finish_pass(ti);
                    return Ok(Resume::None);
                }
            }
        }
    }

    /// A thread reached its body end: `initial`/cont-assign threads are
    /// done; edge-triggered `always` threads (thread-level sens) re-arm on
    /// their sensitivity list and wait for the next event.
    fn finish_pass(&mut self, ti: usize) {
        let sens_empty = self.d.threads[self.threads[ti].base].sens.is_empty();
        if sens_empty {
            self.threads[ti].status = Status::Done;
        } else {
            // Re-arm from the top of the body: the next event re-executes
            // the whole always statement (static sensitivity → zero-alloc
            // Thread form).
            self.threads[ti].pc = 0;
            let base = self.threads[ti].base as u32;
            for it in &self.d.threads[self.threads[ti].base].sens {
                if let Some(list) = self.waiters.get_mut(it.var as usize) {
                    list.push(ti);
                }
            }
            self.threads[ti].status = Status::Wait(WaitList::Thread(base));
        }
    }

    // ------------------------------------------------------------------
    // Storage + change propagation
    // ------------------------------------------------------------------

    fn slot(&self, ti: usize, s: u32) -> Result<&Bits, RunError> {
        self.cur_frame(ti)
            .get(s as usize)
            .ok_or_else(|| RunError(format!("frame slot {s} out of range (thread {ti})")))
    }

    /// The active frame: a static func's shared frame, else the top
    /// activation frame.
    fn cur_frame(&self, ti: usize) -> &Vec<Bits> {
        match self.threads[ti].shared {
            Some(f) => &self.shared_frames[f],
            None => self.threads[ti].frames.last().unwrap(),
        }
    }

    fn set_slot(&mut self, ti: usize, s: u32, v: Bits) {
        if let Some(f) = self.threads[ti].shared {
            let sf = &mut self.shared_frames[f];
            if s as usize >= sf.len() {
                sf.resize(s as usize + 1, Bits::zeros(1).unwrap());
            }
            sf[s as usize] = v;
        } else {
            let f = self.threads[ti].frames.last_mut().unwrap();
            if s as usize >= f.len() {
                f.resize(s as usize + 1, Bits::zeros(1).unwrap());
            }
            f[s as usize] = v;
        }
    }

    fn var(&self, v: VarId) -> Result<&Bits, RunError> {
        self.vars
            .get(v as usize)
            .ok_or_else(|| RunError(format!("var id {v} out of range")))
    }

    /// Var read — the stored value already carries forced bits (the force
    /// commits them into storage; design §4: reads and `w = r` followers
    /// see the forced value).
    fn read_var(&self, v: VarId) -> Result<&Bits, RunError> {
        self.var(v)
    }

    fn load_bit(&self, var: VarId, idx: &Bits) -> Result<Bits, RunError> {
        let v = self.read_var(var)?;
        let Some(abs) = self.abs_index(var, idx)? else {
            return Bits::all_x(1).map_err(|e| RunError(e.0));
        };
        Bits::part(v, abs, abs).map_err(|e| RunError(e.0))
    }

    /// Declared index → absolute bit position; None for unknown or
    /// out-of-range indices (IEEE: reads are x, writes no-op).
    fn abs_index(&self, var: VarId, idx: &Bits) -> Result<Option<u32>, RunError> {
        if idx.has_unknown() {
            return Ok(None);
        }
        let v = &self.d.vars[var as usize];
        let i = idx.to_u128_lossy();
        let abs = if v.msb >= v.lsb {
            i.checked_sub(v.lsb as u128)
        } else {
            (v.lsb as u128).checked_sub(i)
        };
        Ok(match abs {
            Some(a) if a < v.width as u128 => Some(a as u32),
            _ => None,
        })
    }

    /// Resolve an lvalue's dynamic parts into constants (NBA sampling).
    fn resolve_lvalue(&self, ti: usize, lv: &Lval) -> Result<Lval, RunError> {
        Ok(match lv {
            Lval::Var { var } => Lval::Var { var: *var },
            Lval::Bit { var, idx } => {
                let iv = self.slot(ti, *idx)?.clone();
                match self.abs_index(*var, &iv)? {
                    Some(abs) => Lval::Part { var: *var, hi: abs, lo: abs },
                    None => Lval::Bit { var: *var, idx: *idx }, // no-op at commit
                }
            }
            Lval::Part { .. } => lv.clone(),
            // Array element with a known index folds into the Part
            // encoding below; unknown indices become the no-op Bit form.
            Lval::Elem { var, idx } => {
                let iv = self.slot(ti, *idx)?.clone();
                match self.elem_index(*var, &iv)? {
                    Some(abs) => Lval::Part { var: *var, hi: abs as u32, lo: abs as u32 },
                    None => Lval::Bit { var: *var, idx: *idx },
                }
            }
            Lval::PartDyn { var, hi, lo } => {
                let hv = self.slot(ti, *hi)?.clone();
                let lv2 = self.slot(ti, *lo)?.clone();
                if hv.has_unknown() || lv2.has_unknown() {
                    lv.clone() // no-op at commit
                } else {
                    Lval::Part {
                        var: *var,
                        hi: hv.to_u128_lossy() as u32,
                        lo: lv2.to_u128_lossy() as u32,
                    }
                }
            }
            Lval::Concat { parts } => Lval::Concat {
                parts: parts
                    .iter()
                    .map(|(p, w)| Ok((self.resolve_lvalue(ti, p)?, *w)))
                    .collect::<Result<Vec<_>, RunError>>()?,
            },
        })
    }

    /// Commit a value to an lvalue; on change, wake waiters and notify the
    /// dump sink. This is THE write path (blocking assigns, NBA commits,
    /// cont-assign stores all funnel here).
    fn commit(&mut self, lv: &Lval, val: Bits, sink: &mut dyn DumpSink) -> Result<(), RunError> {
        match lv {
            Lval::Var { var } => {
                let vi = self.d.vars[*var as usize].clone();
                let newv = if val.width == vi.width {
                    val
                } else if val.width > vi.width {
                    val.trunc(vi.width).map_err(|e| RunError(e.0))?
                } else {
                    val.uadjust(vi.width).map_err(|e| RunError(e.0))?
                };
                self.store_var(*var, newv, sink)
            }
            Lval::Bit { .. } => {
                // Unresolved dynamic index (unknown) → IEEE: no effect.
                Ok(())
            }
            // PartDyn never reaches commit: resolve_lvalue constant-folds
            // dynamic bounds before NBA scheduling, and elaboration only
            // emits dynamic loads for `+:`/`-:` (unsupported, hard-errored).
            Lval::PartDyn { .. } => unreachable!("unresolved dynamic part in commit"),
            Lval::Elem { .. } => unreachable!("unresolved array element in commit"),
            Lval::Part { var, hi, lo } => {
                let vi = self.d.vars[*var as usize].clone();
                // Resolved array element (resolve_lvalue encodes the
                // absolute element index in hi==lo; presence in the arrays
                // section is the criterion).
                if self.array_dims.get(*var as usize).map_or(false, |d| d.is_some()) {
                    let w = vi.width;
                    let part_val = val.trunc(w).map_err(|e| RunError(e.0))?;
                    return self.store_elem(*var, *hi as u64, part_val, sink);
                }
                let w = hi.abs_diff(*lo) + 1;
                if *hi >= vi.width {
                    return Ok(()); // out-of-range write: no effect
                }
                let part_val = val.trunc(w).map_err(|e| RunError(e.0))?;
                let old = self.var(*var)?.clone();
                let newv = Bits::set_part(&old, *hi, *lo, &part_val).map_err(|e| RunError(e.0))?;
                self.store_var(*var, newv, sink)
            }
            Lval::Concat { parts } => {
                // Slice the RHS MSB-first across the parts' widths.
                let mut off = val.width;
                for (p, w) in parts {
                    off = off.saturating_sub(*w);
                    let slice = Bits::part(&val, off + w - 1, off).map_err(|e| RunError(e.0))?;
                    self.commit(p, slice, sink)?;
                }
                Ok(())
            }
        }
    }

    fn store_var(&mut self, var: VarId, newv: Bits, sink: &mut dyn DumpSink) -> Result<(), RunError> {
        // Forced bits absorb writes (IEEE: writes have no effect on a
        // forced lvalue until release — bit-granular for part forces).
        if let Some(Some(mask)) = self.forces.get(var as usize) {
            let cur = self.vars[var as usize].clone();
            let mask = mask.clone();
            let ones = all_ones(newv.width)?;
            let inv = ones.bit_xor(&mask).map_err(|e| RunError(e.0))?;
            let effective = newv
                .bit_and(&inv)
                .map_err(|e| RunError(e.0))?
                .bit_or(&cur.bit_and(&mask).map_err(|e| RunError(e.0))?)
                .map_err(|e| RunError(e.0))?;
            return self.store_var_raw(var, effective, sink);
        }
        self.store_var_raw(var, newv, sink)
    }

    /// Unconditional storage write + change propagation.
    fn store_var_raw(&mut self, var: VarId, newv: Bits, sink: &mut dyn DumpSink) -> Result<(), RunError> {
        let newv = if self.two_state {
            newv.to_2state().map_err(|e| RunError(e.0))?
        } else {
            newv
        };
        let old = self.vars[var as usize].clone();
        if old == newv {
            return Ok(());
        }
        self.vars[var as usize] = newv.clone();
        if self.dump_open && self.dump_on {
            sink.change(&self.var_flat_name(var), &newv.fmt_bin());
        }
        self.wake(var, &old, &newv);
        // U-4: post-settle value-change firing point (ADR-0003 write-point
        // parity; single funnel = all writes, VPI puts included).
        if let Some(mut f) = self.vpi_hooks.value_change.take() {
            f(self, var as u64);
            self.vpi_hooks.value_change = Some(f);
        }
        Ok(())
    }

    /// Flat dump name = the design var's raw path ("a", "dut.q") — the
    /// legacy VCD shape (top module carries no prefix; ADR-0009 U-1
    /// byte-compat with corpus expect.vcd).
    fn var_flat_name(&self, var: VarId) -> String {
        self.d.vars[var as usize].name.clone()
    }

    /// Wake suspended threads whose wait matches this change (waiter-list
    /// order follows registration = declaration order, ADR-0001), then
    /// re-evaluate gates that watch this var (inertial scheduling).
    fn wake(&mut self, var: VarId, old: &Bits, new: &Bits) {
        // Take the waiter list (capacity is kept — the hot path stays
        // allocation-free after warmup).
        let mut candidates = std::mem::take(&mut self.waiters[var as usize]);
        let d = std::rc::Rc::clone(&self.d);
        for ti in candidates.iter().copied() {
            let fire = match &self.threads[ti].status {
                Status::Wait(wl) => wl.items(&d).iter().any(|item| item.var == var && match item.edge {
                    EdgeKind::Any => true,
                    EdgeKind::Pos => edge_fire(old, new, true),
                    EdgeKind::Neg => edge_fire(old, new, false),
                }),
                _ => false, // stale entry — dropped on restore below
            };
            if fire {
                self.disarm_wait(ti);
                self.threads[ti].status = Status::Ready;
                self.active.push_back(ti);
            }
        }
        candidates.retain(|&ti| matches!(self.threads[ti].status, Status::Wait(_)));
        self.waiters[var as usize] = candidates;
        let watchers = std::mem::take(&mut self.gate_watchers[var as usize]);
        for gi in watchers.iter().copied() {
            self.eval_gate(gi);
        }
        self.gate_watchers[var as usize] = watchers;
    }

    // ------------------------------------------------------------------
    // Named events (R4-e)
    // ------------------------------------------------------------------

    /// `-> ev`: wake threads currently waiting on the event var (IEEE:
    /// a trigger with no waiter is lost). Event vars carry no value.
    fn fire_event(&mut self, ev: VarId) -> Result<(), RunError> {
        if self.d.vars.get(ev as usize).map(|v| v.kind) != Some(VarKind::Event) {
            return Err(RunError(format!("`->` target var {} is not an event", ev)));
        }
        let mut candidates = std::mem::take(&mut self.waiters[ev as usize]);
        let d = std::rc::Rc::clone(&self.d);
        for ti in candidates.iter().copied() {
            let hit = match &self.threads[ti].status {
                Status::Wait(wl) => wl.items(&d).iter().any(|i| i.var == ev && i.edge == EdgeKind::Any),
                _ => false,
            };
            if hit {
                self.disarm_wait(ti);
                self.threads[ti].status = Status::Ready;
                self.active.push_back(ti);
            }
        }
        candidates.retain(|&ti| matches!(self.threads[ti].status, Status::Wait(_)));
        self.waiters[ev as usize] = candidates;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Unpacked arrays (R4-c)
    // ------------------------------------------------------------------

    /// Declared element index → storage position; None for unknown/OOB.
    fn elem_index(&self, var: VarId, idx: &Bits) -> Result<Option<u64>, RunError> {
        if idx.has_unknown() {
            return Ok(None);
        }
        let Some(Some((lo, hi))) = self.array_dims.get(var as usize) else {
            return Err(RunError(format!("var {} is not an array", var)));
        };
        let i = idx.to_u128_lossy() as u64;
        Ok(if i >= *lo && i <= *hi { Some(i - lo) } else { None })
    }

    fn load_elem(&self, var: VarId, idx: &Bits) -> Result<Bits, RunError> {
        let w = self.d.vars[var as usize].width;
        match self.elem_index(var, idx)? {
            Some(abs) => Ok(self.arrays[var as usize][abs as usize].clone()),
            None => Bits::all_x(w).map_err(|e| RunError(e.0)),
        }
    }

    /// Element write: value change wakes waiters on the ARRAY var (the
    /// @*/cont-assign read set uses the var id; per-element granularity is
    /// coarser than IEEE but conservative-correct for wake purposes).
    fn store_elem(&mut self, var: VarId, abs: u64, val: Bits, sink: &mut dyn DumpSink) -> Result<(), RunError> {
        let slot = self
            .arrays
            .get_mut(var as usize)
            .and_then(|a| a.get_mut(abs as usize))
            .ok_or_else(|| RunError(format!("element {} out of range for var {}", abs, var)))?;
        let val = if self.two_state {
            val.to_2state().map_err(|e| RunError(e.0))?
        } else {
            val
        };
        if *slot == val {
            return Ok(());
        }
        let old = slot.clone();
        *slot = val.clone();
        let _ = old;
        // Per-element VCD dump is R5 (riv_vcd gate); flat dumps skip arrays.
        let _ = sink;
        self.wake(var, &old, &val);
        Ok(())
    }

    /// $readmemh/$readmemb (R4-c, pulled forward from R5): whitespace-
    /// separated tokens, @addr jumps, // and /* */ comments, x/z digits;
    /// optional start/finish bounds (absent args carry width 0).
    fn run_readmem(
        &mut self,
        hex: bool,
        path: &str,
        var: VarId,
        bounds: Vec<Option<u64>>,
        sink: &mut dyn DumpSink,
        task: &str,
        file: u32,
        line: u32,
    ) -> Result<(), RunError> {
        if self.d.vars.get(var as usize).map(|v| v.kind) != Some(VarKind::Array) {
            return Err(RunError("readmem target is not a memory".into()));
        }
        let text = match self.injected_files.get(path) {
            Some(c) => c.clone(),
            None => {
                let resolved = self.resolve_readmem(path).unwrap_or_else(|| std::path::PathBuf::from(path));
                std::fs::read_to_string(&resolved)
                    .map_err(|e| RunError(format!("readmem `{path}`: {e}")))?
            }
        };
        let Some(Some((lo, hi))) = self.array_dims.get(var as usize).copied() else {
            return Err(RunError("readmem target is not a memory".into()));
        };
        let start = bounds.first().copied().flatten().unwrap_or(lo);
        let finish = bounds.get(1).copied().flatten().unwrap_or(hi);
        let w = self.d.vars[var as usize].width;
        // Strip comments, then tokenize.
        let mut clean = String::with_capacity(text.len());
        let bytes: Vec<char> = text.chars().collect();
        let mut i = 0usize;
        while i < bytes.len() {
            let c = bytes[i];
            if c == '/' && bytes.get(i + 1) == Some(&'/') {
                while i < bytes.len() && bytes[i] != '\n' {
                    i += 1;
                }
            } else if c == '/' && bytes.get(i + 1) == Some(&'*') {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == '*' && bytes[i + 1] == '/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
            } else {
                clean.push(c);
                i += 1;
            }
        }
        let mut addr: u64 = start;
        for tok in clean.split_whitespace() {
            if let Some(rest) = tok.strip_prefix('@') {
                addr = rest.parse::<u64>().map_err(|_| RunError(format!("readmem bad address `{rest}`")))?;
                continue;
            }
            if addr > finish {
                // iverilog byte shape (stdout): over-range words warn once
                // and the rest is ignored.
                let file_name = self
                    .d
                    .strs
                    .get(file as usize)
                    .and_then(|p| p.first())
                    .map(|p| match p {
                        FmtPiece::Text(t) => t.clone(),
                        _ => String::new(),
                    })
                    .unwrap_or_default();
                self.out.push_str(&format!(
                    "WARNING: {file_name}:{line}: {task}({path}): Too many words in the file for the requested range [{start}:{finish}].\n"
                ));
                break;
            }
            let bits = from_numlit_parts(Some(w), false, if hex { VB::Hex } else { VB::Bin }, &tok.to_ascii_lowercase())
                .map_err(|e| RunError(format!("readmem token `{tok}`: {}", e.0)))?;
            let abs = addr.checked_sub(lo).ok_or_else(|| RunError("readmem address below the array range".to_string()))?;
            self.store_elem(var, abs, bits, sink)?;
            addr += 1;
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Gate primitives (R4-d): 4-state tables + inertial scheduling
    // ------------------------------------------------------------------

    /// Effective gate output: the pending value if one is scheduled, else
    /// the stored var.
    fn gate_current(&self, gi: usize) -> Bits {
        if let Some(Some((_, v))) = self.gate_pending.get(gi) {
            return v.clone();
        }
        let out = self.d.gates[gi].out;
        self.vars[out as usize].clone()
    }

    /// Evaluate gate gi's table with current inputs; a delta vs the
    /// effective output reschedules the pending update (pure inertial: the
    /// previous pending entry is cancelled).
    fn eval_gate(&mut self, gi: usize) {
        let g = self.d.gates[gi].clone();
        let inputs: Vec<Bits> = g.inputs.iter().map(|v| self.vars[*v as usize].clone()).collect();
        let out = gate_table(g.kind, &inputs);
        let cur = self.gate_current(gi);
        if out == cur {
            return;
        }
        // Transition-selected delay (IEEE §7.7): to 1 = rise, to 0 = fall,
        // to z = turn-off, to x = min of the delays used.
        let delay = gate_delay(g.rise, g.fall, g.turnoff, &out);
        // Cancel any pending entry for this gate.
        if let Some(Some((at, _))) = self.gate_pending.get(gi).cloned() {
            if let Some(v) = self.gate_due.get_mut(&at) {
                v.retain(|x| *x != gi);
                if v.is_empty() {
                    self.gate_due.remove(&at);
                }
            }
        }
        let at = self.now.saturating_add(delay);
        self.gate_pending[gi] = Some((at, out));
        self.gate_due.entry(at).or_default().push(gi);
    }

    /// Commit a due gate update (the slot loop drains `gate_due` at `now`).
    fn commit_gate(&mut self, gi: usize, sink: &mut dyn DumpSink) -> Result<(), RunError> {
        let Some(Some((at, val))) = self.gate_pending.get(gi).cloned() else {
            return Ok(());
        };
        if at != self.now {
            // Re-scheduled for later by a newer evaluation.
            self.gate_due.entry(at).or_default().push(gi);
            return Ok(());
        }
        self.gate_pending[gi] = None;
        let out = self.d.gates[gi].out;
        self.store_var(out, val, sink)
    }

    // ------------------------------------------------------------------
    // Monitor region + lazy argument re-evaluation
    // ------------------------------------------------------------------

    /// Re-execute an argument-producer op range in the thread's frame. The
    /// elaborator only lowers expression ops here; anything else is a loud
    /// error at the point of divergence (not silent drift).
    fn eval_range(&mut self, ti: usize, start: u32, end: u32) -> Result<(), RunError> {
        let body = self.d.threads[ti].body.clone();
        for pc in start as usize..end as usize {
            let Some(op) = body.get(pc) else {
                return Err(RunError(format!("monitor arg range pc {pc} out of body")));
            };
            match op {
                Op::Const { dst, ref lit } => {
                    let b = vl_ops::parse_const(lit)
                        .map_err(|e| RunError(format!("const: {}", e.0)))?;
                    self.set_slot(ti, *dst, b);
                }
                Op::Mov { dst, src } => {
                    let v = self.slot(ti, *src)?.clone();
                    self.set_slot(ti, *dst, v);
                }
                Op::Bin { kind, dst, a, b, w, signed } => {
                    let av = self.slot(ti, *a)?.clone();
                    let bv = self.slot(ti, *b)?.clone();
                    let r = vl_ops::apply_bin(*kind, &av, &bv, *w, *signed)
                        .map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::Bit { kind, dst, a, b, w, signed } => {
                    let av = self.slot(ti, *a)?.clone();
                    let bv = self.slot(ti, *b)?.clone();
                    let r = vl_ops::apply_bit(*kind, &av, &bv, *w, *signed)
                        .map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::BitXnor { dst, a, b, w } => {
                    let av = self.slot(ti, *a)?.clone();
                    let bv = self.slot(ti, *b)?.clone();
                    let r = vl_ops::apply_bit_xnor(&av, &bv, *w).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::BitNot { dst, a, w, signed } => {
                    let av = self.slot(ti, *a)?.clone();
                    let r = vl_ops::apply_bit_not(&av, *w, *signed).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::Reduce { kind, dst, a } => {
                    let av = self.slot(ti, *a)?.clone();
                    let r = vl_ops::apply_reduce(*kind, &av).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::LogicAnd { dst, a, b } => {
                    let av = self.slot(ti, *a)?.clone();
                    let bv = self.slot(ti, *b)?.clone();
                    let r = vl_ops::apply_logic_and(&av, &bv).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::LogicOr { dst, a, b } => {
                    let av = self.slot(ti, *a)?.clone();
                    let bv = self.slot(ti, *b)?.clone();
                    let r = vl_ops::apply_logic_or(&av, &bv).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::LogicNot { dst, a } => {
                    let av = self.slot(ti, *a)?.clone();
                    let r = vl_ops::apply_logic_not(&av).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::Cmp { kind, dst, a, b, signed } => {
                    let av = self.slot(ti, *a)?.clone();
                    let bv = self.slot(ti, *b)?.clone();
                    let r = vl_ops::apply_cmp(*kind, &av, &bv, *signed).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::CaseCmp { kind, dst, a, b } => {
                    let av = self.slot(ti, *a)?.clone();
                    let bv = self.slot(ti, *b)?.clone();
                    let r = vl_ops::apply_case_cmp(*kind, &av, &bv).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::Shift { kind, dst, a, b, w, signed } => {
                    let av = self.slot(ti, *a)?.clone();
                    let bv = self.slot(ti, *b)?.clone();
                    let r = vl_ops::apply_shift(*kind, &av, &bv, *w, *signed)
                        .map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::Sel { dst, cond, a, b } => {
                    let cv = self.slot(ti, *cond)?.clone();
                    let av = self.slot(ti, *a)?.clone();
                    let bv = self.slot(ti, *b)?.clone();
                    let r = vl_ops::apply_sel(&cv, &av, &bv).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::Concat { dst, ref parts } => {
                    let vals: Vec<Bits> = parts
                        .iter()
                        .map(|p| self.slot(ti, *p).cloned())
                        .collect::<Result<_, _>>()?;
                    let r = vl_ops::apply_concat(&vals).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::Replicate { dst, src, times } => {
                    let base = self.slot(ti, *src)?.clone();
                    let r = vl_ops::apply_replicate(&base, *times).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::LoadVar { dst, var } => {
                    let v = self.read_var(*var)?.clone();
                    self.set_slot(ti, *dst, v);
                }
                Op::LoadBit { dst, var, idx } => {
                    let iv = self.slot(ti, *idx)?.clone();
                    let r = self.load_bit(*var, &iv)?;
                    self.set_slot(ti, *dst, r);
                }
                Op::LoadPart { dst, var, hi, lo } => {
                    let vv = self.read_var(*var)?.clone();
                    let r = Bits::part(&vv, *hi, *lo).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::LoadPartDyn { dst, var, hi, lo } => {
                    let hv = self.slot(ti, *hi)?.clone();
                    let lv = self.slot(ti, *lo)?.clone();
                    let vv = self.read_var(*var)?.clone();
                    let r = dyn_part(&vv, &hv, &lv)?;
                    self.set_slot(ti, *dst, r);
                }
                Op::LoadElem { dst, var, idx } => {
                    let iv = self.slot(ti, *idx)?.clone();
                    let r = self.load_elem(*var, &iv)?;
                    self.set_slot(ti, *dst, r);
                }
                Op::ScaleTime { dst, src, num, den } => {
                    let sv = self.slot(ti, *src)?.clone();
                    if sv.has_unknown() {
                        return Err(RunError("time expression is unknown (x)".into()));
                    }
                    let v = sv.to_u128_lossy();
                    let scaled = (v * *num as u128 + (*den as u128) / 2) / *den as u128;
                    let r = Bits::from_u128(64, scaled.min(u128::from(u64::MAX)))
                        .map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::SysCall { id: SysId::Time, dst, .. } => {
                    let num = unit_ratio(&self.d.meta.timescale).ok_or_else(|| {
                        RunError(format!("bad timescale `{}`", self.d.meta.timescale))
                    })? as u64;
                    let unit_ticks = (self.now + num / 2) / num.max(1);
                    let Some(d) = dst else {
                        return Err(RunError("$time without destination".into()));
                    };
                    self.set_slot(ti, *d, Bits::from_u64(64, unit_ticks).unwrap());
                }
                Op::Call { .. } | Op::Ret { .. } | Op::AssignForce { .. } | Op::Release { .. } => {
                    return Err(RunError(format!(
                        "op `{}` cannot appear in a $monitor/$strobe argument expression",
                        op_tag_other()
                    )))
                }
                Op::Fork { .. } | Op::JoinDone | Op::JoinPoint => {
                    return Err(RunError(format!(
                        "op `{}` cannot appear in a $monitor/$strobe argument expression",
                        op_tag_other()
                    )))
                }
                other => {
                    return Err(RunError(format!(
                        "op `{}` cannot appear in a $monitor/$strobe argument expression",
                        op_tag(other)
                    )))
                }
            }
        }
        Ok(())
    }

    fn run_monitor_region(&mut self, sink: &mut dyn DumpSink) -> Result<(), RunError> {
        // $strobe first (execution order), then $monitor on-change.
        let strobes = std::mem::take(&mut self.strobes);
        for req in strobes {
            self.eval_range(req.thread, req.start, req.end)?;
            let text = self.render(Some(req.fmt), &req.args, req.thread)?;
            self.out.push_str(&text);
            self.out.push('\n');
        }
        if self.monitor_enabled {
            if let Some(mut req) = self.monitor.take() {
                self.eval_range(req.thread, req.start, req.end)?;
                let vals: Vec<Bits> = req
                    .args
                    .iter()
                    .map(|a| self.slot(req.thread, a.slot).cloned())
                    .collect::<Result<_, _>>()?;
                // Change detection EXCLUDES time arguments — oracle-measured
                // (iverilog 13.0, U-3b probe): `$monitor("%0t o=%b", $time, o)`
                // prints only at t0 and when `o` itself changes; $time moving
                // alone (or unrelated var events) does not re-fire.
                let watched: Vec<Bits> = vals
                    .iter()
                    .zip(req.args.iter())
                    .map(|(v, a)| if a.is_time { Bits::from_u64(64, 0).unwrap() } else { v.clone() })
                    .collect();
                let display = match &req.last {
                    None => true, // t0 arm: first pass displays unconditionally
                    Some(last) => last != &watched,
                };
                if display {
                    let text = self.render(Some(req.fmt), &req.args, req.thread)?;
                    self.out.push_str(&text);
                    self.out.push('\n');
                    req.last = Some(watched);
                }
                self.monitor = Some(req);
            }
        }
        let _ = sink;
        Ok(())
    }

    /// `N=%d` plusarg pattern → (parsed value, output width in bits).
    /// Radix from the pattern spec: %d decimal, %h hex, %o octal, %b
    /// binary (08_plusargs golden pins %d).
    fn parse_plusarg(&self, pat: &str) -> Option<(u128, u32)> {
        let (prefix, spec) = pat.split_once('%')?;
        let (radix, _chars) = match spec.chars().next()? {
            'd' | 'D' => (10u32, 0usize),
            'h' | 'H' => (16, 0),
            'o' | 'O' => (8, 0),
            'b' | 'B' => (2, 0),
            _ => return None,
        };
        let hit = self
            .plusargs
            .iter()
            .find(|pa| pa.trim_start_matches('+').starts_with(prefix))?;
        let val_str = hit.trim_start_matches('+')[prefix.len()..].trim();
        if val_str.is_empty() || !val_str.chars().all(|c| c.is_digit(radix)) {
            return None;
        }
        let v = u128::from_str_radix(val_str, radix).ok()?;
        Some((v, 64))
    }

    // ------------------------------------------------------------------
    // Display + dump
    // ------------------------------------------------------------------

    fn render(&self, fmt: Option<u32>, args: &[Arg], ti: usize) -> Result<String, RunError> {
        // `$display;` (no format) is legal — renders empty.
        let Some(fmt) = fmt else { return Ok(String::new()) };
        let pieces = self
            .d
            .strs
            .get(fmt as usize)
            .ok_or_else(|| RunError(format!("format index {fmt} out of range")))?;
        let scale = unit_ratio(&self.d.meta.timescale).unwrap_or(1).max(1) as u64;
        let mut out = String::new();
        for piece in pieces {
            match piece {
                FmtPiece::Text(t) => out.push_str(t),
                FmtPiece::Spec { ch, width, zero_pad, arg } => {
                    let Some(a) = args.get(*arg as usize) else {
                        return Err(RunError("display argument missing".into()));
                    };
                    let v = self.slot(ti, a.slot)?;
                    out.push_str(&fmt_one(*ch, *width, *zero_pad, v, a.signed, a.is_time, scale));
                }
            }
        }
        Ok(out)
    }

    /// All dumpable vars' current values (arrays/events excluded — per-
    /// element dump is R5+).
    fn dump_values(&self, _current: bool) -> BTreeMap<String, String> {
        let mut values = BTreeMap::new();
        for (i, v) in self.d.vars.iter().enumerate() {
            if v.kind == VarKind::Event || v.kind == VarKind::Array {
                continue;
            }
            values.insert(self.var_flat_name(i as VarId), self.vars[i].fmt_bin());
        }
        values
    }

    fn dump_values_x(&self) -> BTreeMap<String, String> {
        let mut values = BTreeMap::new();
        for (i, v) in self.d.vars.iter().enumerate() {
            if v.kind == VarKind::Event || v.kind == VarKind::Array {
                continue;
            }
            let x = "x".repeat(v.width as usize);
            values.insert(self.var_flat_name(i as VarId), x);
        }
        values
    }

    fn open_dump(&mut self, sink: &mut dyn DumpSink) -> Result<(), RunError> {
        let path = self
            .dump_path
            .clone()
            .unwrap_or_else(|| "dump.vcd".to_string());
        sink.opened(&path);
        if self.emit_vcd_info_line {
            self.out
                .push_str(&format!("VCD info: dumpfile {path} opened for output.\n"));
        }
        let mut values = BTreeMap::new();
        for (i, _v) in self.d.vars.iter().enumerate() {
            values.insert(self.var_flat_name(i as VarId), self.vars[i].fmt_bin());
        }
        sink.dump_t0(&values);
        self.dump_open = true;
        Ok(())
    }
}

fn op_tag_other() -> &'static str {
    "procedural"
}

fn op_tag(op: &Op) -> &'static str {
    match op {
        Op::Const { .. } => "const",
        Op::Mov { .. } => "mov",
        Op::Bin { .. } => "bin",
        Op::Bit { .. } => "bit",
        Op::BitXnor { .. } => "bitxnor",
        Op::BitNot { .. } => "bitnot",
        Op::Reduce { .. } => "reduce",
        Op::LogicAnd { .. } => "logicand",
        Op::LogicOr { .. } => "logicor",
        Op::LogicNot { .. } => "logicnot",
        Op::Cmp { .. } => "cmp",
        Op::CaseCmp { .. } => "casecmp",
        Op::Shift { .. } => "shift",
        Op::Sel { .. } => "sel",
        Op::Concat { .. } => "concat",
        Op::Replicate { .. } => "replicate",
        Op::LoadVar { .. } => "loadvar",
        Op::LoadBit { .. } => "loadbit",
        Op::LoadPart { .. } => "loadpart",
        Op::LoadPartDyn { .. } => "loadpartdyn",
        Op::LoadElem { .. } => "loadelem",
        Op::AssignBlk { .. } => "assignblk",
        Op::AssignNba { .. } => "assignnba",
        Op::AssignNbaDelay { .. } => "assignnbadelay",
        Op::Branch { .. } => "branch",
        Op::Jump { .. } => "jump",
        Op::Delay { .. } => "delay",
        Op::ScaleTime { .. } => "scaletime",
        Op::WaitEdge { .. } => "waitedge",
        Op::SysCall { .. } => "syscall",
        Op::Call { .. } => "call",
        Op::Ret { .. } => "ret",
        Op::AssignForce { .. } => "assignforce",
        Op::Release { .. } => "release",
        Op::Fork { .. } => "fork",
        Op::JoinDone => "joindone",
        Op::JoinPoint => "joinpoint",
        Op::EndThread => "endthread",
    }
}

/// IEEE 1364-2005 §9.7.2 edge tables on the LSB.
fn edge_fire(old: &Bits, new: &Bits, pos: bool) -> bool {
    let bit = |b: &Bits| -> (bool, bool) {
        if b.width == 0 {
            return (false, false);
        }
        let lsb = b.part(0, 0).unwrap();
        let data = lsb.to_u128_lossy() & 1 == 1;
        (data, lsb.has_unknown())
    };
    let (od, ou) = bit(old);
    let (nd, nu) = bit(new);
    if pos {
        // posedge: 0→1, 0→x, 0→z, x→1, z→1
        matches!(
            (od, ou, nd, nu),
            (false, false, true, false)
                | (false, false, true, true)
                | (false, false, false, true)
                | (false, true, true, false)
                | (false, true, true, true)
        )
    } else {
        // negedge: 1→0, 1→x, 1→z, x→0, z→0
        matches!(
            (od, ou, nd, nu),
            (true, false, false, false)
                | (true, false, false, true)
                | (true, false, true, true)
                | (false, true, false, false)
                | (true, true, false, false)
        )
    }
}

/// One-bit 4-state value classification.
#[derive(Clone, Copy, PartialEq)]
enum B4 {
    Zero,
    One,
    X,
    Z,
}

fn b4(v: &Bits) -> B4 {
    if v.width == 0 {
        return B4::X;
    }
    let lsb = v.part(0, 0).unwrap();
    if lsb.has_unknown() {
        // vl-values layout: z = (data 1, mask 1), x = (data 0, mask 1).
        let data = lsb.to_u128_lossy() & 1 == 1;
        if data {
            B4::Z
        } else {
            B4::X
        }
    } else if lsb.to_u128_lossy() & 1 == 1 {
        B4::One
    } else {
        B4::Zero
    }
}

fn b4_bits(v: B4) -> Bits {
    match v {
        B4::Zero => vl_values::from_numlit_parts(Some(1), false, VB::Bin, "0").unwrap(),
        B4::One => vl_values::from_numlit_parts(Some(1), false, VB::Bin, "1").unwrap(),
        B4::X => Bits::all_x(1).unwrap(),
        B4::Z => vl_values::from_numlit_parts(Some(1), false, VB::Bin, "z").unwrap(),
    }
}

/// IEEE §7 gate tables (1-bit): and/nand/or/nor/xor/xnor two-or-more
/// inputs, buf/not one input, bufif/notif with z when disabled.
fn gate_table(kind: GateKind, inputs: &[Bits]) -> Bits {
    let vals: Vec<B4> = inputs.iter().map(b4).collect();
    let out = match kind {
        GateKind::And | GateKind::Nand => {
            let mut acc = B4::One;
            for v in &vals {
                acc = match (acc, v) {
                    (B4::Zero, _) | (_, B4::Zero) => B4::Zero,
                    (B4::One, B4::One) => B4::One,
                    _ => B4::X,
                };
            }
            if kind == GateKind::Nand {
                not4(acc)
            } else {
                acc
            }
        }
        GateKind::Or | GateKind::Nor => {
            let mut acc = B4::Zero;
            for v in &vals {
                acc = match (acc, v) {
                    (B4::One, _) | (_, B4::One) => B4::One,
                    (B4::Zero, B4::Zero) => B4::Zero,
                    _ => B4::X,
                };
            }
            if kind == GateKind::Nor {
                not4(acc)
            } else {
                acc
            }
        }
        GateKind::Xor | GateKind::Xnor => {
            if vals.iter().any(|v| matches!(v, B4::X | B4::Z)) {
                B4::X
            } else {
                let par = vals.iter().filter(|v| **v == B4::One).count() % 2 == 1;
                let r = if par { B4::One } else { B4::Zero };
                if kind == GateKind::Xnor {
                    not4(r)
                } else {
                    r
                }
            }
        }
        GateKind::Buf => vals.first().copied().unwrap_or(B4::X),
        GateKind::Not => not4(vals.first().copied().unwrap_or(B4::X)),
        GateKind::Bufif0 | GateKind::Notif0 | GateKind::Bufif1 | GateKind::Notif1 => {
            let data = vals.first().copied().unwrap_or(B4::X);
            let en = vals.get(1).copied().unwrap_or(B4::X);
            let active = matches!(kind, GateKind::Bufif0 | GateKind::Notif0)
                == (en == B4::Zero);
            match en {
                B4::Zero | B4::One => {
                    if active {
                        let d = if matches!(kind, GateKind::Notif0 | GateKind::Notif1) {
                            not4(data)
                        } else {
                            data
                        };
                        // buf gating is by the ENABLE level: bufif0 passes
                        // data when en==0, bufif1 when en==1.
                        if (en == B4::Zero) == matches!(kind, GateKind::Bufif0 | GateKind::Notif0) {
                            d
                        } else {
                            B4::Z
                        }
                    } else {
                        B4::Z
                    }
                }
                _ => {
                    // Unknown enable: x unless the output would be z
                    // anyway (data z keeps z).
                    if data == B4::Z {
                        B4::Z
                    } else {
                        B4::X
                    }
                }
            }
        }
    };
    b4_bits(out)
}

/// All-ones of width `w` (u64::MAX only fits w <= 64).
fn all_ones(w: u32) -> Result<Bits, RunError> {
    vl_values::from_numlit_parts(Some(w), false, VB::Bin, &"1".repeat(w as usize))
        .map_err(|e| RunError(e.0))
}

/// Width-`w` value with ones ONLY in [hi:lo] (the force mask for a
/// selection — zeros elsewhere, so unforced bits stay writable).
fn mask_of(w: u32, hi: u32, lo: u32) -> Result<Bits, RunError> {
    let n = hi.abs_diff(lo) + 1;
    let zeros = Bits::zeros(w).map_err(|e| RunError(e.0))?;
    Bits::set_part(&zeros, hi, lo, &all_ones(n)?).map_err(|e| RunError(e.0))
}

fn not4(v: B4) -> B4 {
    match v {
        B4::Zero => B4::One,
        B4::One => B4::Zero,
        B4::X | B4::Z => B4::X,
    }
}

/// Transition-selected delay (IEEE §7.7.3): to 1 = rise, to 0 = fall,
/// to z = turn-off, to x = min of the applicable delays.
fn gate_delay(rise: u64, fall: u64, turnoff: u64, new: &Bits) -> u64 {
    match b4(new) {
        B4::One => rise,
        B4::Zero => fall,
        B4::Z => turnoff,
        B4::X => * [rise, fall, turnoff].iter().filter(|d| **d > 0).min().unwrap_or(&0),
    }
}

fn dyn_part(v: &Bits, hv: &Bits, lv: &Bits) -> Result<Bits, RunError> {
    if hv.has_unknown() || lv.has_unknown() {
        let w = hv.width.max(lv.width);
        return Bits::all_x(w).map_err(|e| RunError(e.0));
    }
    let (h, l) = (hv.to_u128_lossy() as u32, lv.to_u128_lossy() as u32);
    let (hi, lo) = if h >= l { (h, l) } else { (l, h) };
    let w = hi - lo + 1;
    if lo >= v.width {
        return Bits::all_x(w).map_err(|e| RunError(e.0));
    }
    let hi_c = hi.min(v.width - 1);
    let known = Bits::part(v, hi_c, lo).map_err(|e| RunError(e.0))?;
    if hi_c == hi {
        Ok(known)
    } else {
        // Top out of range: x-fill above the known bits.
        let fill = Bits::all_x(hi - hi_c).map_err(|e| RunError(e.0))?;
        Bits::concat(&[fill, known]).map_err(|e| RunError(e.0))
    }
}

/// M9 tier-B display face (ADR-0002): THE one $display/%-formatting
/// path. `args` is indexed by FmtPiece::Spec.arg — one
/// (value, signed, is_time) triple per display argument. The interpreter
/// (`Self::render`) and generated models (`--gen-rust`) both land here;
/// a second formatting implementation would be a contract fork.
pub fn render_pieces(
    pieces: &[riv_ir::FmtPiece],
    args: &[(Bits, bool, bool)],
    scale: u64,
) -> String {
    let mut out = String::new();
    for piece in pieces {
        match piece {
            riv_ir::FmtPiece::Text(t) => out.push_str(t),
            riv_ir::FmtPiece::Spec { ch, width, zero_pad, arg } => {
                let Some((v, signed, is_time)) = args.get(*arg as usize) else {
                    continue;
                };
                out.push_str(&fmt_one(*ch, *width, *zero_pad, v, *signed, *is_time, scale));
            }
        }
    }
    out
}

/// Unit→precision scale parsed from a `timescale` string (unit fs /
/// precision fs) — public for the generated-model runtime (ADR-0002).
pub fn unit_ratio_pub(ts: &str) -> Option<u128> {
    unit_ratio(ts)
}

/// Render one format spec (IEEE §F.4 + the 11/45-display goldens).
/// `%t` scales is_time args (unit ticks) into precision units by
/// `scale = unit/precision`; the default field is 20 (AGENTS §5), %0t
/// minimal. Non-time args render raw.
pub fn fmt_one(ch: char, width: u32, zero_pad: bool, v: &Bits, signed: bool, is_time: bool, scale: u64) -> String {
    fn pad(s: String, width: u32, zero_pad: bool) -> String {
        let w = width as usize;
        if w == 0 || s.len() >= w {
            return s;
        }
        let fill = w - s.len();
        if zero_pad {
            if let Some(stripped) = s.strip_prefix('-') {
                format!("-{}{}", "0".repeat(fill), stripped)
            } else {
                format!("{}{}", "0".repeat(fill), s)
            }
        } else {
            format!("{}{}", " ".repeat(fill), s)
        }
    }
    match ch {
        'd' => {
            if v.has_unknown() {
                // "x" — minimal; x-padding beyond this is unpinned in corpus.
                return "x".to_string();
            }
            let s = if signed { v.fmt_dec_signed() } else { v.fmt_dec() };
            if width == 0 {
                if zero_pad {
                    // `%0d` = minimal (10_literals golden: d=300).
                    s
                } else {
                    // Default field = type-natural decimal width (probe:
                    // 8'd42 → " 42", -8'sd5 → "  -5", 1'd1 → "1").
                    let nat = v.dec_natural_len(signed);
                    if s.len() >= nat {
                        s
                    } else {
                        format!("{}{}", " ".repeat(nat - s.len()), s)
                    }
                }
            } else {
                pad(s, width, zero_pad)
            }
        }
        'h' => {
            let s = v.fmt_hex();
            if width == 0 {
                s
            } else {
                pad(s, width, zero_pad)
            }
        }
        'b' => {
            let s = v.fmt_bin();
            if width == 0 {
                s
            } else {
                pad(s, width, zero_pad)
            }
        }
        'o' => {
            let s = v.fmt_oct();
            if width == 0 {
                s
            } else {
                pad(s, width, zero_pad)
            }
        }
        'c' => {
            let low = v.part(7, 0).unwrap_or_else(|_| Bits::zeros(8).unwrap());
            if low.has_unknown() {
                "x".to_string()
            } else {
                (low.to_u128_lossy() as u8 as char).to_string()
            }
        }
        's' => {
            let nbytes = (v.width / 8).max(1);
            let mut s = String::new();
            for i in 0..nbytes {
                let lo = (nbytes - 1 - i) * 8;
                let hi = (lo + 7).min(v.width - 1);
                let byte = v.part(hi, lo).unwrap_or_else(|_| Bits::zeros(1).unwrap());
                if byte.has_unknown() {
                    s.push('x');
                } else {
                    s.push(byte.to_u128_lossy() as u8 as char);
                }
            }
            if width == 0 {
                s
            } else {
                pad(s, width, zero_pad)
            }
        }
        't' => {
            if v.has_unknown() || v.width > 64 {
                return "x".to_string();
            }
            let raw = v.to_u128_lossy() as u64;
            let ticks = if is_time { raw.saturating_mul(scale) } else { raw };
            let s = ticks.to_string();
            if width == 0 {
                if zero_pad {
                    s // %0t minimal (02/24 goldens)
                } else {
                    pad(s, 20, false) // default field 20 (AGENTS §5)
                }
            } else {
                pad(s, width, zero_pad)
            }
        }
        other => format!("%{other}"),
    }
}

/// A sink that ignores dumps (designs without $dumpvars).
pub struct NoDump;

impl DumpSink for NoDump {
    fn opened(&mut self, _path: &str) {}
    fn dump_t0(&mut self, _values: &BTreeMap<String, String>) {}
    fn change(&mut self, _name: &str, _value: &str) {}
    fn flush_slot(&mut self, _now: u64) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use riv_ir::{Arg, FmtPiece, Literal, Op, Thread, ThreadKind, Var, VarKind};

    fn lit_dec(w: u32, v: u64) -> Literal {
        Literal { width: w, signed: false, base: Base::Dec, digits: v.to_string() }
    }

    fn var(id: u32, name: &str, w: u32) -> Var {
        Var { id, name: name.into(), width: w, signed: false, kind: VarKind::Reg, init: None, msb: w - 1, lsb: 0 }
    }

    fn net(id: u32, name: &str, w: u32) -> Var {
        Var { id, name: name.into(), width: w, signed: false, kind: VarKind::Net, init: None, msb: w - 1, lsb: 0 }
    }

    fn meta(top: &str) -> riv_ir::Meta {
        riv_ir::Meta {
            top: top.into(),
            timescale: "1ns/1ps".into(),
            unit: "1ns".into(),
            precision: "1ps".into(),
            sources: vec!["tb.v".into()],
        }
    }

    fn file_pool(d: &mut Design) -> u32 {
        d.strs.push(vec![FmtPiece::Text("tb.v".into())]);
        (d.strs.len() - 1) as u32
    }

    fn finish_op(file: u32, line: u32) -> Op {
        Op::SysCall { id: SysId::Finish, fmt: None, args: vec![], dst: None, file, line, start: 0 }
    }

    /// The 03_nba region-order contract (design §8 pinned unit test):
    /// at t0 the active region reads the OLD value across an NBA, the #0
    /// (inactive) region still reads it, the NBA commits after inactive,
    /// and #1 sees the new value.
    #[test]
    fn region_order_active_inactive_nba() {
        let mut d = Design {
            meta: meta("tb"),
            vars: vec![var(0, "a", 1)],
            arrays: vec![],
            assigns: vec![],
            gates: vec![],
            threads: vec![],
            funcs: vec![],
            strs: vec![],
        };
        let f = file_pool(&mut d);
        // initial a <= 1  (NBA)
        d.threads.push(Thread {
            kind: ThreadKind::Initial,
            sens: vec![],
            frame_len: 1,
            body: vec![
                Op::Const { dst: 0, lit: lit_dec(1, 1) },
                Op::AssignNba { lv: Lval::Var { var: 0 }, src: 0 },
                Op::EndThread,
            ],
        });
        // initial $display; #0 $display; #1 $display; $finish
        // display fmt pieces: text "a=%b" + spec
        let fmt = {
            d.strs.push(vec![
                FmtPiece::Text("a=".into()),
                FmtPiece::Spec { ch: 'b', width: 0, zero_pad: true, arg: 0 },
            ]);
            (d.strs.len() - 1) as u32
        };
        let disp_a = |pc_line: u32| Op::SysCall {
            id: SysId::Display,
            fmt: Some(fmt),
            args: vec![Arg { slot: 1, width: 1, signed: false, is_time: false }],
            dst: None,
            file: f,
            line: pc_line,
            start: 0,
        };
        d.threads.push(Thread {
            kind: ThreadKind::Initial,
            sens: vec![],
            frame_len: 2,
            body: vec![
                Op::LoadVar { dst: 1, var: 0 },
                disp_a(3),
                Op::Const { dst: 0, lit: lit_dec(64, 0) }, // #0
                Op::Delay { amount: 0 },
                Op::LoadVar { dst: 1, var: 0 },
                disp_a(4),
                Op::Const { dst: 0, lit: lit_dec(64, 1000) }, // #1
                Op::Delay { amount: 0 },
                Op::LoadVar { dst: 1, var: 0 },
                disp_a(5),
                finish_op(f, 7),
                Op::EndThread,
            ],
        });
        // Give a a declaration init 0 as its own thread (decl order: first).
        d.threads.insert(
            0,
            Thread {
                kind: ThreadKind::Initial,
                sens: vec![],
                frame_len: 1,
                body: vec![
                    Op::Const { dst: 0, lit: lit_dec(1, 0) },
                    Op::AssignBlk { lv: Lval::Var { var: 0 }, src: 0 },
                    Op::EndThread,
                ],
            },
        );
        let mut k = Kernel::new(d).unwrap();
        let out = k.run(&mut NoDump);
        assert_eq!(
            out.output,
            "a=0\na=0\na=1\ntb.v:7: $finish called at 1000 (1ps)\n"
        );
        assert_eq!(out.exit, 0);
    }

    /// NBA sampling: the value AND the assigned value are sampled at
    /// scheduling time (44_lfsr's shift register depends on it).
    #[test]
    fn nba_samples_old_value() {
        let mut d = Design {
            meta: meta("tb"),
            vars: vec![var(0, "q", 4)],
            arrays: vec![],
            assigns: vec![],
            gates: vec![],
            threads: vec![],
            funcs: vec![],
            strs: vec![],
        };
        let f = file_pool(&mut d);
        // display fmt: strs[1]
        d.strs.push(vec![
            FmtPiece::Text("q=".into()),
            FmtPiece::Spec { ch: 'b', width: 0, zero_pad: true, arg: 0 },
        ]);
        let fmt = 1u32;
        let disp = Op::SysCall {
            id: SysId::Display,
            fmt: Some(fmt),
            args: vec![Arg { slot: 1, width: 4, signed: false, is_time: false }],
            dst: None,
            file: f,
            line: 5,
            start: 0,
        };
        d.threads.push(Thread {
            kind: ThreadKind::Initial,
            sens: vec![],
            frame_len: 3,
            body: vec![
                // q <= 4'hF (sampled now; commits in the NBA region)
                Op::Const { dst: 0, lit: Literal { width: 4, signed: false, base: Base::Hex, digits: "f".into() } },
                Op::AssignNba { lv: Lval::Var { var: 0 }, src: 0 },
                // active region: the blocking read still sees x
                Op::LoadVar { dst: 1, var: 0 },
                disp.clone(),
                // #0 → inactive region: STILL the old value (the NBA
                // commits after inactive — the 03_nba contract)
                Op::Const { dst: 2, lit: lit_dec(64, 0) },
                Op::Delay { amount: 2 },
                Op::LoadVar { dst: 1, var: 0 },
                disp.clone(),
                // #1 → next slot; the NBA has committed → q = F
                Op::Const { dst: 2, lit: lit_dec(64, 1000) },
                Op::Delay { amount: 2 },
                Op::LoadVar { dst: 1, var: 0 },
                disp.clone(),
                // q = q + 1 (blocking, F+1 wraps to 0)
                Op::Const { dst: 2, lit: lit_dec(4, 1) },
                Op::Bin { kind: riv_ir::BinKind::Add, dst: 1, a: 1, b: 2, w: 4, signed: false },
                Op::AssignBlk { lv: Lval::Var { var: 0 }, src: 1 },
                Op::LoadVar { dst: 1, var: 0 },
                disp,
                finish_op(f, 6),
                Op::EndThread,
            ],
        });
        let mut k = Kernel::new(d).unwrap();
        let out = k.run(&mut NoDump);
        assert_eq!(
            out.output,
            "q=xxxx\nq=xxxx\nq=1111\nq=0000\ntb.v:6: $finish called at 1000 (1ps)\n"
        );
    }

    /// `repeat (n) @(posedge clk)` idiom: edge counting across slots.
    #[test]
    fn repeat_waitedge_idiom() {
        let mut d = Design {
            meta: meta("tb"),
            vars: vec![var(0, "clk", 1), var(1, "hits", 8)],
            arrays: vec![],
            assigns: vec![],
            gates: vec![],
            threads: vec![],
            funcs: vec![],
            strs: vec![],
        };
        let f = file_pool(&mut d);
        // reg clk = 0 (declaration initializer → t0 change event, as the
        // corpus's `reg clk = 0` lowers; without it ~x = x forever).
        d.threads.push(Thread {
            kind: ThreadKind::Initial,
            sens: vec![],
            frame_len: 1,
            body: vec![
                Op::Const { dst: 0, lit: lit_dec(1, 0) },
                Op::AssignBlk { lv: Lval::Var { var: 0 }, src: 0 },
                Op::EndThread,
            ],
        });
        // always #5 clk = ~clk
        d.threads.push(Thread {
            kind: ThreadKind::Always,
            sens: vec![],
            frame_len: 2,
            body: vec![
                Op::Const { dst: 0, lit: lit_dec(64, 5000) },
                Op::Delay { amount: 0 },
                Op::LoadVar { dst: 0, var: 0 },
                Op::BitNot { dst: 1, a: 0, w: 1, signed: false },
                Op::AssignBlk { lv: Lval::Var { var: 0 }, src: 1 },
                Op::Jump { to: 0 },
            ],
        });
        // initial: repeat (3) @(posedge clk); $display time; $finish
        d.threads.push(Thread {
            kind: ThreadKind::Initial,
            sens: vec![],
            frame_len: 2,
            body: vec![
                Op::Const { dst: 0, lit: lit_dec(32, 3) },
                Op::Branch { cond: 0, then: 2, els: 6 },
                Op::WaitEdge { items: vec![riv_ir::SensItem { var: 0, edge: riv_ir::EdgeKind::Pos }] },
                Op::Const { dst: 1, lit: lit_dec(32, 1) },
                Op::Bin { kind: riv_ir::BinKind::Sub, dst: 0, a: 0, b: 1, w: 32, signed: false },
                Op::Jump { to: 1 },
                finish_op(f, 9),
                Op::EndThread,
            ],
        });
        let mut k = Kernel::new(d).unwrap();
        let out = k.run(&mut NoDump);
        // 3rd posedge at 25000 (clk toggles every 5ns from t0).
        assert_eq!(out.output, "tb.v:9: $finish called at 25000 (1ps)\n");
    }

    /// %d default field = type-natural width; %0d minimal; %5d/%05d pad
    /// (probed verilator rule + 10/11 golden pins).
    #[test]
    fn display_field_widths() {
        let mut d = Design {
            meta: meta("tb"),
            vars: vec![],
            arrays: vec![],
            assigns: vec![],
            gates: vec![],
            threads: vec![],
            funcs: vec![],
            strs: vec![],
        };
        let f = file_pool(&mut d);
        // %d of 8'd42 → " 42"; %0d → "42"; %5d → "   42"; %05d → "00042".
        d.strs.push(vec![
            FmtPiece::Spec { ch: 'd', width: 0, zero_pad: false, arg: 0 },
            FmtPiece::Spec { ch: 'd', width: 0, zero_pad: true, arg: 0 },
            FmtPiece::Spec { ch: 'd', width: 5, zero_pad: false, arg: 0 },
            FmtPiece::Spec { ch: 'd', width: 5, zero_pad: true, arg: 0 },
        ]);
        let fmt = (d.strs.len() - 1) as u32;
        d.threads.push(Thread {
            kind: ThreadKind::Initial,
            sens: vec![],
            frame_len: 1,
            body: vec![
                Op::Const { dst: 0, lit: lit_dec(8, 42) },
                Op::SysCall {
                    id: SysId::Write,
                    fmt: Some(fmt),
                    args: vec![Arg { slot: 0, width: 8, signed: false, is_time: false }],
                    dst: None,
                    file: f,
                    line: 3,
                    start: 0,
                },
                Op::SysCall { id: SysId::Display, fmt: None, args: vec![], dst: None, file: f, line: 3 , start: 0 },
                finish_op(f, 4),
                Op::EndThread,
            ],
        });
        let mut k = Kernel::new(d).unwrap();
        let out = k.run(&mut NoDump);
        assert_eq!(out.output, " 4242   4200042\ntb.v:4: $finish called at 0 (1ps)\n");
    }

    /// $monitor semantics (02_monitor golden): t0 arm displays on the
    /// FIRST monitor pass after arming; later passes only on change; the
    /// $time argument re-evaluates at render time (fresh t per display).
    #[test]
    fn monitor_t0_arm_and_fresh_time() {
        let mut d = Design {
            meta: meta("tb"),
            vars: vec![var(0, "c", 8)],
            arrays: vec![],
            assigns: vec![],
            gates: vec![],
            threads: vec![],
            funcs: vec![],
            strs: vec![],
        };
        let f = file_pool(&mut d);
        // decl init c = 0 (own thread, declaration order)
        d.threads.push(Thread {
            kind: ThreadKind::Initial,
            sens: vec![],
            frame_len: 1,
            body: vec![
                Op::Const { dst: 0, lit: lit_dec(8, 0) },
                Op::AssignBlk { lv: Lval::Var { var: 0 }, src: 0 },
                Op::EndThread,
            ],
        });
        // fmt: "m: t=%0t c=%0d" (strs[1]); args: $time (strs[2] not needed —
        // the Time syscall needs no fmt), c load
        d.strs.push(vec![
            FmtPiece::Text("m: t=".into()),
            FmtPiece::Spec { ch: 't', width: 0, zero_pad: true, arg: 0 },
            FmtPiece::Text(" c=".into()),
            FmtPiece::Spec { ch: 'd', width: 0, zero_pad: true, arg: 1 },
        ]);
        let fmt = 1u32;
        // initial: $monitor(...); #5 c = 10; #5 c = 20; #5 $finish
        // arg ops start at pc 0: [SysCall Time → slot0, LoadVar c → slot1]
        d.threads.push(Thread {
            kind: ThreadKind::Initial,
            sens: vec![],
            frame_len: 2,
            body: vec![
                Op::SysCall { id: SysId::Time, fmt: None, args: vec![], dst: Some(0), file: f, line: 5, start: 0 },
                Op::LoadVar { dst: 1, var: 0 },
                Op::SysCall {
                    id: SysId::Monitor,
                    fmt: Some(fmt),
                    args: vec![
                        Arg { slot: 0, width: 64, signed: false, is_time: true },
                        Arg { slot: 1, width: 8, signed: false, is_time: false },
                    ],
                    dst: None,
                    file: f,
                    line: 5,
                    start: 0,
                },
                Op::Const { dst: 1, lit: lit_dec(64, 5000) },
                Op::Delay { amount: 1 },
                Op::Const { dst: 1, lit: lit_dec(8, 10) },
                Op::AssignBlk { lv: Lval::Var { var: 0 }, src: 1 },
                Op::Const { dst: 1, lit: lit_dec(64, 5000) },
                Op::Delay { amount: 1 },
                Op::Const { dst: 1, lit: lit_dec(8, 20) },
                Op::AssignBlk { lv: Lval::Var { var: 0 }, src: 1 },
                Op::Const { dst: 1, lit: lit_dec(64, 5000) },
                Op::Delay { amount: 1 },
                finish_op(f, 9),
                Op::EndThread,
            ],
        });
        let mut k = Kernel::new(d).unwrap();
        let out = k.run(&mut NoDump);
        // t0 arm display + one per change, $time fresh each render:
        // m: t=0 c=0 / m: t=5000 c=10 / m: t=10000 c=20 (then finish at
        // 15000 without a monitor line — no change).
        assert_eq!(
            out.output,
            "m: t=0 c=0\nm: t=5000 c=10\nm: t=10000 c=20\ntb.v:9: $finish called at 15000 (1ps)\n"
        );
    }

    /// $time = unit ticks (24_clock_time: %0d of $time = 23 at t=23000
    /// with 1ns/1ps); %t re-scales to precision ticks (23000).
    #[test]
    fn time_scales_unit_and_precision() {
        assert_eq!(unit_ratio("1ns/1ps"), Some(1000));
        assert_eq!(unit_ratio("1s/1ms"), Some(1_000));
        assert_eq!(unit_ratio("10ns/1ns"), Some(10));
        let mut d = Design {
            meta: meta("tb"),
            vars: vec![],
            arrays: vec![],
            assigns: vec![],
            gates: vec![],
            threads: vec![],
            funcs: vec![],
            strs: vec![],
        };
        let f = file_pool(&mut d);
        // initial: #23; $display("%0d", $time); $display("%0t", $time)
        d.strs.push(vec![
            FmtPiece::Spec { ch: 'd', width: 0, zero_pad: true, arg: 0 },
        ]);
        d.strs.push(vec![
            FmtPiece::Spec { ch: 't', width: 0, zero_pad: true, arg: 0 },
        ]);
        d.threads.push(Thread {
            kind: ThreadKind::Initial,
            sens: vec![],
            frame_len: 1,
            body: vec![
                Op::Const { dst: 0, lit: lit_dec(64, 23_000) },
                Op::Delay { amount: 0 },
                Op::SysCall { id: SysId::Time, fmt: None, args: vec![], dst: Some(0), file: f, line: 4, start: 0 },
                Op::SysCall {
                    id: SysId::Display,
                    fmt: Some(1),
                    args: vec![Arg { slot: 0, width: 64, signed: false, is_time: true }],
                    dst: None,
                    file: f,
                    line: 4,
                    start: 0,
                },
                Op::SysCall {
                    id: SysId::Display,
                    fmt: Some(2),
                    args: vec![Arg { slot: 0, width: 64, signed: false, is_time: true }],
                    dst: None,
                    file: f,
                    line: 4,
                    start: 0,
                },
                finish_op(f, 5),
                Op::EndThread,
            ],
        });
        let mut k = Kernel::new(d).unwrap();
        let out = k.run(&mut NoDump);
        assert_eq!(out.output, "23\n23000\ntb.v:5: $finish called at 23000 (1ps)\n");
    }

    /// casez item matching through the CaseCmp WildZ op (26_fsm shape).
    #[test]
    fn casez_dispatch_matches_wildcards() {
        let mut d = Design {
            meta: meta("tb"),
            vars: vec![var(0, "s", 2), var(1, "r", 8)],
            arrays: vec![],
            assigns: vec![],
            gates: vec![],
            threads: vec![],
            funcs: vec![],
            strs: vec![],
        };
        let f = file_pool(&mut d);
        // s = 2'b01; casez (s) 2'b0?: r = 7; default: r = 9; display r
        let zlit = |digits: &str| Literal {
            width: 2,
            signed: false,
            base: Base::Bin,
            digits: digits.into(),
        };
        d.threads.push(Thread {
            kind: ThreadKind::Initial,
            sens: vec![],
            frame_len: 3,
            body: vec![
                Op::Const { dst: 0, lit: zlit("01") },
                Op::AssignBlk { lv: Lval::Var { var: 0 }, src: 0 },
                Op::LoadVar { dst: 0, var: 0 },
                Op::Const { dst: 1, lit: zlit("0?") },
                Op::CaseCmp { kind: riv_ir::CaseCmpKind::WildZ, dst: 2, a: 0, b: 1 },
                Op::Branch { cond: 2, then: 6, els: 7 },
                Op::Const { dst: 2, lit: lit_dec(8, 7) },
                Op::AssignBlk { lv: Lval::Var { var: 1 }, src: 2 },
                Op::Jump { to: 11 },
                Op::Const { dst: 2, lit: lit_dec(8, 9) },
                Op::AssignBlk { lv: Lval::Var { var: 1 }, src: 2 },
                Op::LoadVar { dst: 2, var: 1 },
                Op::SysCall {
                    id: SysId::Display,
                    fmt: Some(1),
                    args: vec![Arg { slot: 2, width: 8, signed: false, is_time: false }],
                    dst: None,
                    file: f,
                    line: 8,
                    start: 0,
                },
                finish_op(f, 9),
                Op::EndThread,
            ],
        });
        d.strs.push(vec![
            FmtPiece::Text("r=".into()),
            FmtPiece::Spec { ch: 'd', width: 0, zero_pad: true, arg: 0 },
        ]);
        let mut k = Kernel::new(d).unwrap();
        let out = k.run(&mut NoDump);
        assert_eq!(out.output, "r=7\ntb.v:9: $finish called at 0 (1ps)\n");
    }

    /// The design §3-mandated test the corpus cannot catch: TWO threads
    /// concurrently calling the SAME static task share one frame — the
    /// second call's input overwrites the first's formal slot. Automatic
    /// tasks would give each thread its own activation (checked below).
    #[test]
    fn concurrent_static_task_shares_frame() {
        let mut d = Design {
            meta: meta("tb"),
            vars: vec![var(0, "o1", 8), var(1, "o2", 8)],
            arrays: vec![],
            assigns: vec![],
            gates: vec![],
            threads: vec![],
            funcs: vec![],
            strs: vec![],
        };
        // static task t(input [7:0] v, output [7:0] o): o = v (frame: ret 0,
        // formal v=1, formal o=2, local tmp=3).
        d.funcs.push(riv_ir::Func {
            name: "t".into(),
            is_task: true,
            automatic: false,
            frame_len: 4,
            ret: None,
            body: vec![
                Op::Mov { dst: 2, src: 1 },
                Op::Ret { src: 0 },
            ],
        });
        // initial: t(11, o1) — the arg copy lands in the SHARED frame.
        d.threads.push(Thread {
            kind: ThreadKind::Initial,
            sens: vec![],
            frame_len: 1,
            body: vec![
                Op::Const { dst: 0, lit: lit_dec(8, 11) },
                Op::Call { func: 0, ins: vec![(0, 1)], outs: vec![(Lval::Var { var: 0 }, 2)], dst: None },
                Op::EndThread,
            ],
        });
        // initial: t(22, o2) — runs after thread 1 in declaration order.
        d.threads.push(Thread {
            kind: ThreadKind::Initial,
            sens: vec![],
            frame_len: 1,
            body: vec![
                Op::Const { dst: 0, lit: lit_dec(8, 22) },
                Op::Call { func: 0, ins: vec![(0, 1)], outs: vec![(Lval::Var { var: 1 }, 2)], dst: None },
                Op::EndThread,
            ],
        });
        let mut k = Kernel::new(d).unwrap();
        let out = k.run(&mut NoDump);
        // No $finish in this design — output is empty; the claims are the
        // shared-frame assertions below.
        assert_eq!(out.output, "");
        // The shared-frame claim is checked via the internal state: the
        // static frame's formal slot holds the LAST call's arg.
        assert_eq!(
            k.shared_frames[0][1].to_u128_lossy(),
            22,
            "static task formal slot = shared storage (last call wins)"
        );
        assert_eq!(
            k.shared_frames[0][2].to_u128_lossy(),
            22,
            "output formal also shared"
        );
    }

    /// fork...join with deterministic delays (06_fork golden shape): all
    /// branches run concurrently, the parent waits at the join, output
    /// ordered by time.
    #[test]
    fn fork_join_branches_and_waiter() {
        let mut d = Design {
            meta: meta("tb"),
            vars: vec![],
            arrays: vec![],
            assigns: vec![],
            gates: vec![],
            threads: vec![],
            funcs: vec![],
            strs: vec![],
        };
        let f = file_pool(&mut d);
        d.strs.push(vec![FmtPiece::Text("A".into())]);
        d.strs.push(vec![FmtPiece::Text("B".into())]);
        d.strs.push(vec![FmtPiece::Text("C".into())]);
        d.strs.push(vec![FmtPiece::Text("joined".into())]);
        // fmt pool: strs[0..3] used as single-piece formats.
        // fmt indices start at 1: file_pool owns strs[0].
        let disp = |fi: u32| Op::SysCall {
            id: SysId::Display,
            fmt: Some(fi + 1),
            args: vec![],
            dst: None,
            file: f,
            line: 3,
            start: 0,
        };
        d.threads.push(Thread {
            kind: ThreadKind::Initial,
            sens: vec![],
            frame_len: 1,
            body: vec![
                Op::Fork { branches: vec![1, 5, 9], join: 11 },
                // branch A: #5 display A
                Op::Const { dst: 0, lit: lit_dec(64, 5000) },
                Op::Delay { amount: 0 },
                disp(0),
                Op::JoinDone,
                // branch B: #2 display B
                Op::Const { dst: 0, lit: lit_dec(64, 2000) },
                Op::Delay { amount: 0 },
                disp(1),
                Op::JoinDone,
                // branch C: display C immediately
                disp(2),
                Op::JoinDone,
                // join point
                Op::JoinPoint,
                disp(3),
                finish_op(f, 6),
                Op::EndThread,
            ],
        });
        let mut k = Kernel::new(d).unwrap();
        let out = k.run(&mut NoDump);
        assert_eq!(
            out.output,
            "C\nB\nA\njoined\ntb.v:6: $finish called at 5000 (1ps)\n"
        );
    }

    /// force/release (04_force golden shape): the forced value is visible
    /// to reads and cont-assign followers; release keeps the value; writes
    /// between force and release are ignored.
    #[test]
    fn force_release_overlay() {
        let mut d = Design {
            meta: meta("tb"),
            vars: vec![var(0, "r", 1), var(1, "w", 1)],
            arrays: vec![],
            assigns: vec![],
            gates: vec![],
            threads: vec![],
            funcs: vec![],
            strs: vec![],
        };
        let f = file_pool(&mut d);
        d.strs.push(vec![
            FmtPiece::Text("r=".into()),
            FmtPiece::Spec { ch: 'b', width: 0, zero_pad: true, arg: 0 },
            FmtPiece::Text(" w=".into()),
            FmtPiece::Spec { ch: 'b', width: 0, zero_pad: true, arg: 1 },
        ]);
        let fmt = 1u32;
        let disp = Op::SysCall {
            id: SysId::Display,
            fmt: Some(fmt),
            args: vec![
                Arg { slot: 0, width: 1, signed: false, is_time: false },
                Arg { slot: 1, width: 1, signed: false, is_time: false },
            ],
            dst: None,
            file: f,
            line: 6,
            start: 0,
        };
        // wire w = r (cont-assign thread)
        d.threads.push(Thread {
            kind: ThreadKind::ContAssign,
            sens: vec![],
            frame_len: 1,
            body: vec![
                Op::LoadVar { dst: 0, var: 0 },
                Op::AssignBlk { lv: Lval::Var { var: 1 }, src: 0 },
                Op::WaitEdge { items: vec![riv_ir::SensItem { var: 0, edge: riv_ir::EdgeKind::Any }] },
                Op::Jump { to: 0 },
                Op::EndThread,
            ],
        });
        // initial: r=1 decl-init thread is inserted below; then
        // force r=1; display; r=0 (ignored!); display; release r; display.
        d.threads.insert(
            0,
            Thread {
                kind: ThreadKind::Initial,
                sens: vec![],
                frame_len: 1,
                body: vec![
                    Op::Const { dst: 0, lit: lit_dec(1, 0) },
                    Op::AssignBlk { lv: Lval::Var { var: 0 }, src: 0 },
                    Op::EndThread,
                ],
            },
        );
        // Time-separated like the 04 golden: the display happens one slot
        // after each mutation so the cont-assign follower has settled.
        d.threads.push(Thread {
            kind: ThreadKind::Initial,
            sens: vec![],
            frame_len: 1,
            body: vec![
                Op::Const { dst: 0, lit: lit_dec(64, 1000) },
                Op::Delay { amount: 0 },
                Op::Const { dst: 0, lit: lit_dec(1, 1) },
                Op::AssignForce { lv: Lval::Var { var: 0 }, src: 0 },
                Op::Const { dst: 0, lit: lit_dec(64, 1000) },
                Op::Delay { amount: 0 },
                Op::LoadVar { dst: 0, var: 0 },
                Op::LoadVar { dst: 1, var: 1 },
                disp.clone(),
                // blocking write while forced — ignored
                Op::Const { dst: 0, lit: lit_dec(1, 0) },
                Op::AssignBlk { lv: Lval::Var { var: 0 }, src: 0 },
                Op::LoadVar { dst: 0, var: 0 },
                Op::LoadVar { dst: 1, var: 1 },
                disp.clone(),
                Op::Release { lv: Lval::Var { var: 0 } },
                Op::LoadVar { dst: 0, var: 0 },
                Op::LoadVar { dst: 1, var: 1 },
                disp,
                finish_op(f, 8),
                Op::EndThread,
            ],
        });
        let mut k = Kernel::new(d).unwrap();
        let out = k.run(&mut NoDump);
        assert_eq!(
            out.output,
            "r=1 w=1\nr=1 w=1\nr=1 w=1\ntb.v:8: $finish called at 2000 (1ps)\n"
        );
    }

    /// R4-a probe: blocking assignments to bit selects and array elements
    /// (the latent no-op commit path — blocking bit writes were silently
    /// dropped before resolve_lvalue routing; found while wiring arrays).
    #[test]
    fn blocking_bit_and_elem_assigns_commit() {
        let mut d = Design {
            meta: meta("tb"),
            vars: vec![var(0, "q", 4), var(1, "mem", 8)],
            arrays: vec![riv_ir::ArrayDim { var: 1, dims: vec![(2, 0)] }],
            assigns: vec![],
            gates: vec![],
            threads: vec![],
            funcs: vec![],
            strs: vec![],
        };
        let _ = &mut d;
        let mut d2 = Design {
            meta: meta("tb"),
            vars: vec![var(0, "q", 4), var(1, "mem", 8)],
            arrays: vec![riv_ir::ArrayDim { var: 1, dims: vec![(2, 0)] }],
            assigns: vec![],
            gates: vec![],
            threads: vec![],
            funcs: vec![],
            strs: vec![],
        };
        d2.threads.push(Thread {
            kind: ThreadKind::Initial,
            sens: vec![],
            frame_len: 2,
            body: vec![
                // q[1] = 1'b1 (dynamic index in a slot)
                Op::Const { dst: 0, lit: lit_dec(4, 1) },
                Op::Const { dst: 1, lit: lit_dec(1, 1) },
                Op::AssignBlk { lv: Lval::Bit { var: 0, idx: 0 }, src: 1 },
                // mem[2] = 8'hA5
                Op::Const { dst: 0, lit: lit_dec(4, 2) },
                Op::Const { dst: 1, lit: Literal { width: 8, signed: false, base: Base::Hex, digits: "a5".into() } },
                Op::AssignBlk { lv: Lval::Elem { var: 1, idx: 0 }, src: 1 },
                Op::EndThread,
            ],
        });
        let mut k = Kernel::new(d2).unwrap();
        let out = k.run(&mut NoDump);
        assert_eq!(out.exit, 0);
        assert_eq!(k.vars[0].fmt_bin(), "xx1x", "q[1] blocking write committed (unwritten bits stay x)");
        assert_eq!(k.arrays[1][2].fmt_hex(), "a5", "mem[2] blocking write committed");
    }

    /// R4-d probe: the inertial cancel window. `nand #6 g(y, x, x)` with a
    /// 4-tick pulse (x 0→1 at t3, back at t7) must NOT produce the
    /// transport glitch — the pending rise at t13 is the only update.
    #[test]
    fn gate_delay_is_inertial_not_transport() {
        let mut d = Design {
            meta: meta("tb"),
            vars: vec![var(0, "x", 1), net(1, "y", 1)],
            arrays: vec![],
            assigns: vec![],
            gates: vec![riv_ir::Gate {
                kind: riv_ir::GateKind::Nand,
                out: 1,
                inputs: vec![0, 0],
                rise: 6000,
                fall: 6000,
                turnoff: 0,
            }],
            threads: vec![],
            funcs: vec![],
            strs: vec![],
        };
        // x: =0 at t0, =1 at t3, =0 at t7 (pulse shorter than the delay).
        d.threads.push(Thread {
            kind: ThreadKind::Initial,
            sens: vec![],
            frame_len: 1,
            body: vec![
                Op::Const { dst: 0, lit: lit_dec(1, 0) },
                Op::AssignBlk { lv: Lval::Var { var: 0 }, src: 0 },
                Op::Const { dst: 0, lit: lit_dec(64, 3000) },
                Op::Delay { amount: 0 },
                Op::Const { dst: 0, lit: lit_dec(1, 1) },
                Op::AssignBlk { lv: Lval::Var { var: 0 }, src: 0 },
                Op::Const { dst: 0, lit: lit_dec(64, 4000) },
                Op::Delay { amount: 0 },
                Op::Const { dst: 0, lit: lit_dec(1, 0) },
                Op::AssignBlk { lv: Lval::Var { var: 0 }, src: 0 },
                Op::Const { dst: 0, lit: lit_dec(64, 10_000) },
                Op::Delay { amount: 0 },
                Op::EndThread,
            ],
        });
        let mut k = Kernel::new(d).unwrap();
        let out = k.run(&mut NoDump);
        assert_eq!(out.exit, 0);
        // Inertial: the t9 fall is cancelled by the t7 re-rise → y goes
        // straight to 1 at t13 (x's t7 pulse never reaches the output).
        assert_eq!(k.vars[1].fmt_bin(), "1", "y settles to 1 at t13");
    }

    /// R4-d probe: bufif1 z-drive on disable (54_gates never observes it —
    /// en=0 arrives after the last display).
    #[test]
    fn bufif_disable_drives_z() {
        let mut d = Design {
            meta: meta("tb"),
            vars: vec![var(0, "a", 1), var(1, "en", 1), net(2, "w", 1)],
            arrays: vec![],
            assigns: vec![],
            gates: vec![riv_ir::Gate {
                kind: riv_ir::GateKind::Bufif1,
                out: 2,
                inputs: vec![0, 1],
                rise: 0,
                fall: 0,
                turnoff: 0,
            }],
            threads: vec![],
            funcs: vec![],
            strs: vec![],
        };
        d.threads.push(Thread {
            kind: ThreadKind::Initial,
            sens: vec![],
            frame_len: 1,
            body: vec![
                Op::Const { dst: 0, lit: lit_dec(1, 1) }, // a = 1
                Op::AssignBlk { lv: Lval::Var { var: 0 }, src: 0 },
                Op::Const { dst: 0, lit: lit_dec(1, 1) }, // en = 1
                Op::AssignBlk { lv: Lval::Var { var: 1 }, src: 0 },
                Op::Const { dst: 0, lit: lit_dec(64, 1000) },
                Op::Delay { amount: 0 },
                Op::Const { dst: 0, lit: lit_dec(1, 0) }, // en = 0 → z
                Op::AssignBlk { lv: Lval::Var { var: 1 }, src: 0 },
                Op::Const { dst: 0, lit: lit_dec(64, 1000) },
                Op::Delay { amount: 0 },
                Op::EndThread,
            ],
        });
        let mut k = Kernel::new(d).unwrap();
        let out = k.run(&mut NoDump);
        assert_eq!(out.exit, 0);
        // a=1, en=1 → w=1; en→0 → w=z (zero turnoff delay).
        assert_eq!(k.vars[2].fmt_bin(), "z", "bufif1 disabled drives z");
    }

    /// IEEE edge tables: x→1 is a posedge, 1→x is a negedge (10_gates
    /// family needs this at R4; the table is pinned here).
    #[test]
    fn edge_tables_match_ieee() {
        let b = |s: &str| {
            vl_values::from_numlit_parts(Some(1), false, VB::Bin, s).unwrap()
        };
        assert!(edge_fire(&b("x"), &b("1"), true));
        assert!(edge_fire(&b("0"), &b("1"), true));
        assert!(edge_fire(&b("0"), &b("x"), true));
        assert!(!edge_fire(&b("1"), &b("x"), true));
        assert!(!edge_fire(&b("1"), &b("0"), true));
        assert!(edge_fire(&b("1"), &b("x"), false));
        assert!(edge_fire(&b("1"), &b("0"), false));
        assert!(edge_fire(&b("x"), &b("0"), false));
        assert!(!edge_fire(&b("0"), &b("1"), false));
    }
}
