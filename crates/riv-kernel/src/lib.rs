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
}

/// VCD/dump hook. The kernel notifies; the host (rivvp) owns the actual
/// vl-vcd writer and file IO.
pub trait DumpSink {
    /// `$dumpvars` opened the dump for `path`.
    fn opened(&mut self, path: &str);
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
    /// Suspended on an edge/level wait list.
    Wait(Vec<SensItem>),
    /// Suspended at a fork...join — resumed when the group counter hits 0.
    /// The group id lives in `join_groups` (waiter pairing); the payload is
    /// not needed on the status itself.
    Join,
    Done,
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
    d: Design,
    vars: Vec<Bits>,
    threads: Vec<ThreadState>,
    active: VecDeque<usize>,
    inactive: VecDeque<usize>,
    nba: Vec<(riv_ir::Lval, Bits)>,
    wheel: BTreeMap<u64, Vec<usize>>,
    now: u64,
    out: String,
    finished: Option<(u32, u32)>,
    dump_path: Option<String>,
    dump_open: bool,
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
    /// `force` overlays per var: reads see it, writes are ignored.
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
    /// rivvp -n: $stop behaves like $finish (rivD2).
    pub stop_as_finish: bool,
}

/// Unit→precision scale parsed from meta.timescale (unit fs / precision fs).
fn unit_ratio(ts: &str) -> Option<u128> {
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
    pub fn new(d: Design) -> Result<Kernel, RunError> {
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
                    // Edge-triggered always: armed before first execution.
                    Status::Wait(t.sens.clone())
                },
            });
        }
        Ok(Kernel {
            threads,
            shared_frames,
            forces,
            join_groups: Vec::new(),
            arrays,
            array_dims,
            gate_due: BTreeMap::new(),
            gate_pending,
            stop_as_finish: false,
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
            plusargs: Vec::new(),
            monitor: None,
            monitor_enabled: true,
            strobes: Vec::new(),
            d,
        })
    }

    /// Plusargs for `$test$plusargs`/`$value$plusargs` (+N=42 form).
    pub fn set_plusargs(&mut self, args: Vec<String>) {
        self.plusargs = args;
    }

    /// Run to completion; ready threads start in declaration order.
    pub fn run(&mut self, sink: &mut dyn DumpSink) -> RunOutcome {
        for ti in 0..self.threads.len() {
            if matches!(self.threads[ti].status, Status::Ready) {
                self.active.push_back(ti);
            }
        }
        // Gates evaluate once at t0 (iverilog semantics): output deltas
        // schedule through the inertial wheel like any input change.
        for gi in 0..self.d.gates.len() {
            self.eval_gate(gi);
        }
        // Main slot loop (design §4).
        loop {
            loop {
                // 1. Active region: FIFO drain; commits wake into the tail.
                while let Some(ti) = self.active.pop_front() {
                    match self.step_thread(ti, sink) {
                        Ok(Resume::Inactive) => self.inactive.push_back(ti),
                        Ok(Resume::Wheel(at)) => self.wheel.entry(at).or_default().push(ti),
                        Ok(Resume::None) => {}
                        Err(e) => return self.finish_out(sink, Some(e)),
                    }
                    if self.finished.is_some() {
                        return self.finish_out(sink, None);
                    }
                }
                // 2. Inactive region (#0 resumes).
                while let Some(ti) = self.inactive.pop_front() {
                    self.threads[ti].status = Status::Ready;
                    match self.step_thread(ti, sink) {
                        Ok(Resume::Inactive) => self.inactive.push_back(ti),
                        Ok(Resume::Wheel(at)) => self.wheel.entry(at).or_default().push(ti),
                        Ok(Resume::None) => {}
                        Err(e) => return self.finish_out(sink, Some(e)),
                    }
                    if self.finished.is_some() {
                        return self.finish_out(sink, None);
                    }
                }
                // 3. NBA batch commit.
                let batch = std::mem::take(&mut self.nba);
                for (lv, val) in batch {
                    if let Err(e) = self.commit(&lv, val, sink) {
                        return self.finish_out(sink, Some(e));
                    }
                    if self.finished.is_some() {
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
            // Advance the wheel: threads and pending gate updates both.
            let next_thread = self.wheel.keys().next().copied();
            let next_gate = self.gate_due.keys().next().copied();
            let at = match (next_thread, next_gate) {
                (Some(a), Some(b)) => a.min(b),
                (Some(a), None) => a,
                (None, Some(b)) => b,
                (None, None) => break, // no more events: natural stop (exit 0)
            };
            self.now = at;
            if next_thread == Some(at) {
                let (_, wake) = self.wheel.pop_first().unwrap();
                for ti in wake {
                    self.threads[ti].status = Status::Ready;
                    self.active.push_back(ti);
                }
            }
        }
        self.finish_out(sink, None)
    }

    fn finish_out(&mut self, sink: &mut dyn DumpSink, err: Option<RunError>) -> RunOutcome {
        sink.finish();
        let exit = match err {
            None => 0,
            Some(e) => {
                self.out.push_str(&format!("rivvp: runtime error: {}\n", e.0));
                1
            }
        };
        if let Some((file, line)) = self.finished {
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
        }
    }

    // ------------------------------------------------------------------
    // Thread execution
    // ------------------------------------------------------------------

    fn step_thread(&mut self, ti: usize, sink: &mut dyn DumpSink) -> Result<Resume, RunError> {
        loop {
            // Re-read per iteration: Call/Ret switch the body ref.
            let body_len = self.body_len(ti);
            let pc = self.threads[ti].pc;
            if pc >= body_len {
                self.finish_pass(ti);
                return Ok(Resume::None);
            }
            let op = self.body_op(ti, pc);
            self.threads[ti].pc = pc + 1;
            match op {
                Op::Const { dst, lit } => {
                    let b = from_numlit_parts(
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
                    .map_err(|e| RunError(format!("const: {}", e.0)))?;
                    self.set_slot(ti, dst, b);
                }
                Op::Mov { dst, src } => {
                    let v = self.slot(ti, src)?.clone();
                    self.set_slot(ti, dst, v);
                }
                Op::Bin { kind, dst, a, b, w, signed } => {
                    let av = self.adjusted(ti, a, w, signed)?;
                    let bv = self.adjusted(ti, b, w, signed)?;
                    let r = match kind {
                        riv_ir::BinKind::Add => av.add(&bv),
                        riv_ir::BinKind::Sub => av.sub(&bv),
                        riv_ir::BinKind::Mul => av.mul(&bv),
                        riv_ir::BinKind::Div => {
                            if signed {
                                av.sdiv(&bv)
                            } else {
                                av.div(&bv)
                            }
                        }
                        riv_ir::BinKind::Rem => {
                            if signed {
                                av.srem(&bv)
                            } else {
                                av.rem(&bv)
                            }
                        }
                        riv_ir::BinKind::Pow => {
                            if signed {
                                av.spow(&bv)
                            } else {
                                av.pow(&bv)
                            }
                        }
                    }
                    .map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::Bit { kind, dst, a, b, w } => {
                    let av = self.adjusted(ti, a, w, false)?;
                    let bv = self.adjusted(ti, b, w, false)?;
                    let r = match kind {
                        riv_ir::BitKind::And => av.bit_and(&bv),
                        riv_ir::BitKind::Or => av.bit_or(&bv),
                        riv_ir::BitKind::Xor => av.bit_xor(&bv),
                    }
                    .map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::BitXnor { dst, a, b, w } => {
                    let av = self.adjusted(ti, a, w, false)?;
                    let bv = self.adjusted(ti, b, w, false)?;
                    let r = av.bit_xor(&bv).map_err(|e| RunError(e.0))?.bit_not();
                    self.set_slot(ti, dst, r);
                }
                Op::BitNot { dst, a, w } => {
                    let av = self.adjusted(ti, a, w, false)?;
                    self.set_slot(ti, dst, av.bit_not());
                }
                Op::Reduce { kind, dst, a } => {
                    let av = self.slot(ti, a)?;
                    let r = match kind {
                        riv_ir::RedKind::And => av.reduce_and(),
                        riv_ir::RedKind::Or => av.reduce_or(),
                        riv_ir::RedKind::Xor => av.reduce_xor(),
                        riv_ir::RedKind::Nand => {
                            Ok(av.reduce_and().map_err(|e: vl_values::ValueError| RunError(e.0))?.bit_not())
                        }
                        riv_ir::RedKind::Nor => {
                            Ok(av.reduce_or().map_err(|e: vl_values::ValueError| RunError(e.0))?.bit_not())
                        }
                        riv_ir::RedKind::Xnor => {
                            Ok(av.reduce_xor().map_err(|e: vl_values::ValueError| RunError(e.0))?.bit_not())
                        }
                    }
                    .map_err(|e: vl_values::ValueError| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::LogicAnd { dst, a, b } => {
                    let av = truthy(self.slot(ti, a)?)?;
                    let bv = truthy(self.slot(ti, b)?)?;
                    self.set_slot(ti, dst, Bits::from_u64(1, (av && bv) as u64).unwrap());
                }
                Op::LogicOr { dst, a, b } => {
                    let av = truthy(self.slot(ti, a)?)?;
                    let bv = truthy(self.slot(ti, b)?)?;
                    self.set_slot(ti, dst, Bits::from_u64(1, (av || bv) as u64).unwrap());
                }
                Op::LogicNot { dst, a } => {
                    let av = truthy(self.slot(ti, a)?)?;
                    self.set_slot(ti, dst, Bits::from_u64(1, !av as u64).unwrap());
                }
                Op::Cmp { kind, dst, a, b, signed } => {
                    let (av, bv) = self.cmp_pair(ti, a, b, signed)?;
                    let ord = if signed {
                        av.cmp_signed(&bv)
                    } else {
                        av.cmp_unsigned(&bv)
                    };
                    let r = match (kind, ord) {
                        (_, None) => Bits::all_x(1).unwrap(),
                        (riv_ir::CmpKind::Lt, Some(o)) => Bits::from_u64(1, o.is_lt() as u64).unwrap(),
                        (riv_ir::CmpKind::Le, Some(o)) => Bits::from_u64(1, o.is_le() as u64).unwrap(),
                        (riv_ir::CmpKind::Gt, Some(o)) => Bits::from_u64(1, o.is_gt() as u64).unwrap(),
                        (riv_ir::CmpKind::Ge, Some(o)) => Bits::from_u64(1, o.is_ge() as u64).unwrap(),
                        (riv_ir::CmpKind::Eq, Some(o)) => Bits::from_u64(1, o.is_eq() as u64).unwrap(),
                        (riv_ir::CmpKind::Ne, Some(o)) => Bits::from_u64(1, o.is_ne() as u64).unwrap(),
                    };
                    self.set_slot(ti, dst, r);
                }
                Op::CaseCmp { kind, dst, a, b } => {
                    let av = self.slot(ti, a)?.clone();
                    let bv = self.slot(ti, b)?.clone();
                    let r = match kind {
                        riv_ir::CaseCmpKind::ExactEq => av.case_eq(&bv),
                        riv_ir::CaseCmpKind::ExactNe => av.case_ne(&bv),
                        // Case-statement item matching: non-match is 0,
                        // never x (IEEE 9.5 — the item simply does not fire).
                        riv_ir::CaseCmpKind::WildZ => Ok(Bits::from_u64(1, av.casez_match(&bv) as u64).unwrap()),
                        riv_ir::CaseCmpKind::WildX => Ok(Bits::from_u64(1, av.casex_match(&bv) as u64).unwrap()),
                    }
                    .map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::Shift { kind, dst, a, b, w, signed } => {
                    let av = self.adjusted(ti, a, w, signed)?;
                    let bv = self.slot(ti, b)?.clone();
                    let r = match kind {
                        riv_ir::ShiftKind::Shl => av.shl(&bv),
                        riv_ir::ShiftKind::Shr => av.shr(&bv),
                        riv_ir::ShiftKind::Ashr => av.ashr(&bv, signed),
                    }
                    .map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::Sel { dst, cond, a, b } => {
                    let cv = self.slot(ti, cond)?.clone();
                    let r = match cv.truthy().map_err(|e| RunError(e.0))? {
                        Some(true) => self.slot(ti, a)?.clone(),
                        Some(false) => self.slot(ti, b)?.clone(),
                        None => self
                            .slot(ti, a)?
                            .merge(self.slot(ti, b)?)
                            .map_err(|e| RunError(e.0))?,
                    };
                    self.set_slot(ti, dst, r);
                }
                Op::Concat { dst, parts } => {
                    let vals: Vec<Bits> = parts
                        .iter()
                        .map(|p| self.slot(ti, *p).cloned())
                        .collect::<Result<_, _>>()?;
                    let r = Bits::concat(&vals).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, dst, r);
                }
                Op::Replicate { dst, src, times } => {
                    let base = self.slot(ti, src)?.clone();
                    let vals: Vec<Bits> = (0..times).map(|_| base.clone()).collect();
                    let r = Bits::concat(&vals).map_err(|e| RunError(e.0))?;
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
                Op::AssignBlk { lv, src } => {
                    let val = self.slot(ti, src)?.clone();
                    let resolved = self.resolve_lvalue(ti, &lv)?;
                    self.commit(&resolved, val, sink)?;
                }
                Op::AssignNba { lv, src } => {
                    let val = self.slot(ti, src)?.clone();
                    let resolved = self.resolve_lvalue(ti, &lv)?;
                    self.nba.push((resolved, val));
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
                Op::WaitEdge { items } => {
                    self.threads[ti].status = Status::Wait(items);
                    return Ok(Resume::None);
                }
                Op::SysCall { id, fmt, args, dst, file, line, start } => match id {
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
                            args,
                            start,
                            end: pc,
                            last: None,
                        });
                        self.monitor_enabled = true;
                    }
                    SysId::Stop => {
                        // rivD2: default = iverilog's banner + CONTINUE
                        // (EOF semantics; we never read stdin); -n makes
                        // $stop equivalent to $finish.
                        if self.stop_as_finish {
                            self.finished = Some((file, line));
                            self.threads[ti].status = Status::Done;
                            return Ok(Resume::None);
                        }
                        let num = unit_ratio(&self.d.meta.timescale).unwrap_or(1).max(1) as u64;
                        let _ = num;
                        self.out.push_str(&format!(
                            "** VVP Stop(0) **\n** Flushing output streams.\n** Current simulation time is {} ticks.\n** Resuming simulation forever.\n",
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
                        self.run_readmem(id == SysId::ReadmemH, &path, var, bounds, sink)?;
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
                            args,
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
                Op::Call { func, ins, outs, dst } => {
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
                Op::AssignForce { lv, src } => {
                    let val = self.slot(ti, src)?.clone();
                    let var = match lv {
                        Lval::Var { var }
                        | Lval::Bit { var, .. }
                        | Lval::Part { var, .. }
                        | Lval::PartDyn { var, .. } => var,
                        Lval::Elem { .. } => {
                            return Err(RunError("force on array elements is not supported".into()))
                        }
                        Lval::Concat { .. } => {
                            return Err(RunError("force on a concatenation is not supported".into()))
                        }
                    };
                    let vi = self.d.vars[var as usize].clone();
                    let newv = if val.width == vi.width {
                        val
                    } else if val.width > vi.width {
                        val.trunc(vi.width).map_err(|e| RunError(e.0))?
                    } else {
                        val.uadjust(vi.width).map_err(|e| RunError(e.0))?
                    };
                    // Store first (overlay unset → normal change propagation:
                    // w = r cont-assign sees the forced value), then pin.
                    self.store_var(var, newv.clone(), sink)?;
                    self.forces[var as usize] = Some(newv);
                }
                Op::Release { lv } => {
                    let var = match lv {
                        Lval::Var { var }
                        | Lval::Bit { var, .. }
                        | Lval::Part { var, .. }
                        | Lval::PartDyn { var, .. } => var,
                        Lval::Elem { .. } => {
                            return Err(RunError("release on array elements is not supported".into()))
                        }
                        Lval::Concat { .. } => {
                            return Err(RunError("release on a concatenation is not supported".into()))
                        }
                    };
                    // The reg keeps the forced value (04_force golden); the
                    // overlay clears so normal writes work again.
                    self.forces[var as usize] = None;
                }
                Op::Fork { branches, join } => {
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
        let sens = self.d.threads[self.threads[ti].base].sens.clone();
        if sens.is_empty() {
            self.threads[ti].status = Status::Done;
        } else {
            // Re-arm from the top of the body: the next event re-executes
            // the whole always statement.
            self.threads[ti].pc = 0;
            self.threads[ti].status = Status::Wait(sens);
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

    fn body_len(&self, ti: usize) -> u32 {
        match self.threads[ti].body {
            BODY_OWN => self.d.threads[self.threads[ti].base].body.len() as u32,
            f => self.d.funcs[f].body.len() as u32,
        }
    }

    fn body_op(&self, ti: usize, pc: u32) -> Op {
        match self.threads[ti].body {
            BODY_OWN => self.d.threads[self.threads[ti].base].body[pc as usize].clone(),
            f => self.d.funcs[f].body[pc as usize].clone(),
        }
    }

    fn var(&self, v: VarId) -> Result<&Bits, RunError> {
        self.vars
            .get(v as usize)
            .ok_or_else(|| RunError(format!("var id {v} out of range")))
    }

    /// Var read with the force overlay applied (design §4: `force` makes
    /// reads see the forced value; `w = r` cont-assign threads follow).
    fn read_var(&self, v: VarId) -> Result<&Bits, RunError> {
        if let Some(Some(f)) = self.forces.get(v as usize) {
            return Ok(f);
        }
        self.var(v)
    }

    fn adjusted(&self, ti: usize, s: u32, w: u32, signed: bool) -> Result<Bits, RunError> {
        let v = self.slot(ti, s)?;
        if signed {
            v.sadjust(w)
        } else {
            v.uadjust(w)
        }
        .map_err(|e| RunError(e.0))
    }

    fn cmp_pair(&self, ti: usize, a: u32, b: u32, signed: bool) -> Result<(Bits, Bits), RunError> {
        let av = self.slot(ti, a)?;
        let bv = self.slot(ti, b)?;
        let w = av.width.max(bv.width);
        let adj = |v: &Bits| -> Result<Bits, RunError> {
            if signed {
                v.sadjust(w)
            } else {
                v.uadjust(w)
            }
            .map_err(|e| RunError(e.0))
        };
        Ok((adj(av)?, adj(bv)?))
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
        if self.forces.get(var as usize).map_or(false, |f| f.is_some()) {
            // IEEE: while a reg is forced, procedural/NBA writes have no
            // effect until `release`.
            return Ok(());
        }
        let old = self.vars[var as usize].clone();
        if old == newv {
            return Ok(());
        }
        self.vars[var as usize] = newv.clone();
        if self.dump_open {
            sink.change(&self.var_flat_name(var), &newv.fmt_bin());
        }
        self.wake(var, &old, &newv);
        Ok(())
    }

    fn var_flat_name(&self, var: VarId) -> String {
        format!("{}.{}", self.d.meta.top, self.d.vars[var as usize].name)
    }

    /// Wake suspended threads whose wait matches this change (thread-index
    /// order = declaration order, ADR-0001), then re-evaluate gates that
    /// watch this var (inertial scheduling).
    fn wake(&mut self, var: VarId, old: &Bits, new: &Bits) {
        for ti in 0..self.threads.len() {
            let items = match &self.threads[ti].status {
                Status::Wait(items) => items.clone(),
                _ => continue,
            };
            for item in &items {
                if item.var != var {
                    continue;
                }
                let fire = match item.edge {
                    EdgeKind::Any => true,
                    EdgeKind::Pos => edge_fire(old, new, true),
                    EdgeKind::Neg => edge_fire(old, new, false),
                };
                if fire {
                    self.threads[ti].status = Status::Ready;
                    self.active.push_back(ti);
                    break;
                }
            }
        }
        for gi in 0..self.d.gates.len() {
            let g = &self.d.gates[gi];
            if g.out == var || g.inputs.iter().any(|i| *i == var) {
                self.eval_gate(gi);
            }
        }
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
        for ti in 0..self.threads.len() {
            let items = match &self.threads[ti].status {
                Status::Wait(items) => items.clone(),
                _ => continue,
            };
            if items.iter().any(|i| i.var == ev && i.edge == EdgeKind::Any) {
                self.threads[ti].status = Status::Ready;
                self.active.push_back(ti);
            }
        }
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
    ) -> Result<(), RunError> {
        if self.d.vars.get(var as usize).map(|v| v.kind) != Some(VarKind::Array) {
            return Err(RunError("readmem target is not a memory".into()));
        }
        let text = std::fs::read_to_string(path).map_err(|e| RunError(format!("readmem `{path}`: {e}")))?;
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
                Op::Const { dst, lit } => {
                    let b = from_numlit_parts(
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
                    .map_err(|e| RunError(format!("const: {}", e.0)))?;
                    self.set_slot(ti, *dst, b);
                }
                Op::Mov { dst, src } => {
                    let v = self.slot(ti, *src)?.clone();
                    self.set_slot(ti, *dst, v);
                }
                Op::Bin { kind, dst, a, b, w, signed } => {
                    let av = self.adjusted(ti, *a, *w, *signed)?;
                    let bv = self.adjusted(ti, *b, *w, *signed)?;
                    let r = match kind {
                        riv_ir::BinKind::Add => av.add(&bv),
                        riv_ir::BinKind::Sub => av.sub(&bv),
                        riv_ir::BinKind::Mul => av.mul(&bv),
                        riv_ir::BinKind::Div => {
                            if *signed { av.sdiv(&bv) } else { av.div(&bv) }
                        }
                        riv_ir::BinKind::Rem => {
                            if *signed { av.srem(&bv) } else { av.rem(&bv) }
                        }
                        riv_ir::BinKind::Pow => {
                            if *signed { av.spow(&bv) } else { av.pow(&bv) }
                        }
                    }
                    .map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::Bit { kind, dst, a, b, w } => {
                    let av = self.adjusted(ti, *a, *w, false)?;
                    let bv = self.adjusted(ti, *b, *w, false)?;
                    let r = match kind {
                        riv_ir::BitKind::And => av.bit_and(&bv),
                        riv_ir::BitKind::Or => av.bit_or(&bv),
                        riv_ir::BitKind::Xor => av.bit_xor(&bv),
                    }
                    .map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::BitXnor { dst, a, b, w } => {
                    let av = self.adjusted(ti, *a, *w, false)?;
                    let bv = self.adjusted(ti, *b, *w, false)?;
                    let r = av.bit_xor(&bv).map_err(|e| RunError(e.0))?.bit_not();
                    self.set_slot(ti, *dst, r);
                }
                Op::BitNot { dst, a, w } => {
                    let av = self.adjusted(ti, *a, *w, false)?;
                    self.set_slot(ti, *dst, av.bit_not());
                }
                Op::Reduce { kind, dst, a } => {
                    let av = self.slot(ti, *a)?;
                    let r = match kind {
                        riv_ir::RedKind::And => av.reduce_and(),
                        riv_ir::RedKind::Or => av.reduce_or(),
                        riv_ir::RedKind::Xor => av.reduce_xor(),
                        riv_ir::RedKind::Nand => Ok(av
                            .reduce_and()
                            .map_err(|e: vl_values::ValueError| RunError(e.0))?
                            .bit_not()),
                        riv_ir::RedKind::Nor => Ok(av
                            .reduce_or()
                            .map_err(|e: vl_values::ValueError| RunError(e.0))?
                            .bit_not()),
                        riv_ir::RedKind::Xnor => Ok(av
                            .reduce_xor()
                            .map_err(|e: vl_values::ValueError| RunError(e.0))?
                            .bit_not()),
                    }
                    .map_err(|e: vl_values::ValueError| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::LogicAnd { dst, a, b } => {
                    let av = truthy(self.slot(ti, *a)?)?;
                    let bv = truthy(self.slot(ti, *b)?)?;
                    self.set_slot(ti, *dst, Bits::from_u64(1, (av && bv) as u64).unwrap());
                }
                Op::LogicOr { dst, a, b } => {
                    let av = truthy(self.slot(ti, *a)?)?;
                    let bv = truthy(self.slot(ti, *b)?)?;
                    self.set_slot(ti, *dst, Bits::from_u64(1, (av || bv) as u64).unwrap());
                }
                Op::LogicNot { dst, a } => {
                    let av = truthy(self.slot(ti, *a)?)?;
                    self.set_slot(ti, *dst, Bits::from_u64(1, !av as u64).unwrap());
                }
                Op::Cmp { kind, dst, a, b, signed } => {
                    let (av, bv) = self.cmp_pair(ti, *a, *b, *signed)?;
                    let ord = if *signed {
                        av.cmp_signed(&bv)
                    } else {
                        av.cmp_unsigned(&bv)
                    };
                    let r = match (kind, ord) {
                        (_, None) => Bits::all_x(1).unwrap(),
                        (riv_ir::CmpKind::Lt, Some(o)) => Bits::from_u64(1, o.is_lt() as u64).unwrap(),
                        (riv_ir::CmpKind::Le, Some(o)) => Bits::from_u64(1, o.is_le() as u64).unwrap(),
                        (riv_ir::CmpKind::Gt, Some(o)) => Bits::from_u64(1, o.is_gt() as u64).unwrap(),
                        (riv_ir::CmpKind::Ge, Some(o)) => Bits::from_u64(1, o.is_ge() as u64).unwrap(),
                        (riv_ir::CmpKind::Eq, Some(o)) => Bits::from_u64(1, o.is_eq() as u64).unwrap(),
                        (riv_ir::CmpKind::Ne, Some(o)) => Bits::from_u64(1, o.is_ne() as u64).unwrap(),
                    };
                    self.set_slot(ti, *dst, r);
                }
                Op::CaseCmp { kind, dst, a, b } => {
                    let av = self.slot(ti, *a)?.clone();
                    let bv = self.slot(ti, *b)?.clone();
                    let r = match kind {
                        riv_ir::CaseCmpKind::ExactEq => av.case_eq(&bv),
                        riv_ir::CaseCmpKind::ExactNe => av.case_ne(&bv),
                        riv_ir::CaseCmpKind::WildZ => Ok(Bits::from_u64(1, av.casez_match(&bv) as u64).unwrap()),
                        riv_ir::CaseCmpKind::WildX => Ok(Bits::from_u64(1, av.casex_match(&bv) as u64).unwrap()),
                    }
                    .map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::Shift { kind, dst, a, b, w, signed } => {
                    let av = self.adjusted(ti, *a, *w, *signed)?;
                    let bv = self.slot(ti, *b)?.clone();
                    let r = match kind {
                        riv_ir::ShiftKind::Shl => av.shl(&bv),
                        riv_ir::ShiftKind::Shr => av.shr(&bv),
                        riv_ir::ShiftKind::Ashr => av.ashr(&bv, *signed),
                    }
                    .map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::Sel { dst, cond, a, b } => {
                    let cv = self.slot(ti, *cond)?.clone();
                    let r = match cv.truthy().map_err(|e| RunError(e.0))? {
                        Some(true) => self.slot(ti, *a)?.clone(),
                        Some(false) => self.slot(ti, *b)?.clone(),
                        None => self
                            .slot(ti, *a)?
                            .merge(self.slot(ti, *b)?)
                            .map_err(|e| RunError(e.0))?,
                    };
                    self.set_slot(ti, *dst, r);
                }
                Op::Concat { dst, parts } => {
                    let vals: Vec<Bits> = parts
                        .iter()
                        .map(|p| self.slot(ti, *p).cloned())
                        .collect::<Result<_, _>>()?;
                    let r = Bits::concat(&vals).map_err(|e| RunError(e.0))?;
                    self.set_slot(ti, *dst, r);
                }
                Op::Replicate { dst, src, times } => {
                    let base = self.slot(ti, *src)?.clone();
                    let vals: Vec<Bits> = (0..*times).map(|_| base.clone()).collect();
                    let r = Bits::concat(&vals).map_err(|e| RunError(e.0))?;
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
                let display = match &req.last {
                    None => true, // t0 arm: first pass displays unconditionally
                    Some(last) => last != &vals,
                };
                if display {
                    let text = self.render(Some(req.fmt), &req.args, req.thread)?;
                    self.out.push_str(&text);
                    self.out.push('\n');
                    req.last = Some(vals);
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

    fn open_dump(&mut self, sink: &mut dyn DumpSink) -> Result<(), RunError> {
        let path = self
            .dump_path
            .clone()
            .unwrap_or_else(|| "dump.vcd".to_string());
        sink.opened(&path);
        self.out
            .push_str(&format!("VCD info: dumpfile {path} opened for output.\n"));
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

fn truthy(b: &Bits) -> Result<bool, RunError> {
    b.truthy()
        .map_err(|e| RunError(e.0))?
        .ok_or_else(|| RunError("condition is unknown".into()))
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

/// Render one format spec (IEEE §F.4 + the 11/45-display goldens).
/// `%t` scales is_time args (unit ticks) into precision units by
/// `scale = unit/precision`; the default field is 20 (AGENTS §5), %0t
/// minimal. Non-time args render raw.
fn fmt_one(ch: char, width: u32, zero_pad: bool, v: &Bits, signed: bool, is_time: bool, scale: u64) -> String {
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
                Op::BitNot { dst: 1, a: 0, w: 1 },
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
        let f = file_pool(&mut d);
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
