//! rVPI — a Rust-native, structurally-equivalent analog of the C Verilog
//! Procedural Interface (IEEE 1364-2005 §23-26), designed for this
//! simulator's architecture: typed handles over a frozen elaboration table,
//! closure callbacks fired at fixed scheduler points, no unsafe, no C ABI.
//!
//! Structure mapping (C-VPI → rVPI): `vpiHandle` → [`VpiHandle`] (index into
//! a name-sorted frozen table); `vpi_handle_by_name` → [`Sim::vpi_handle_by_name`];
//! `vpi_iterate` → [`Sim::vpi_children`]; `vpi_get_value`/`vpi_put_value` →
//! [`Sim::vpi_get_as`]/[`Sim::vpi_put_value`]; `vpi_register_cb` →
//! [`Sim::register_cb`]; `vpi_get_time` → [`Sim::vpi_time`];
//! `vpi_control(vpiFinish)` → [`Sim::finish`].
//!
//! Divergences from C-VPI are registered in docs/REGISTRY.md (D12-D15).

use crate::eval::SResult;
use crate::interp::Sim;
use crate::value::Bits;
use crate::eval::SimError;
use std::collections::{BTreeMap, HashMap};

/// Runaway guard: callbacks firing per drain point (cascade budget).
const CB_CASCADE_BUDGET: u32 = 10_000;

/// Typed handle into the frozen object table. Validity is structural:
/// the table is built once at the end of elaboration and never mutates
/// (no `cells.remove` exists in the runtime). Handles are per-`Sim`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct VpiHandle(pub u32);

/// Object class, mirroring the vpiObjType distinctions we model.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellKind {
    Module,
    Net,
    Reg,
    Integer,
    Time,
    Param,
    Localparam,
    Genvar,
    Event,
    MemoryWord,
    FuncStatic,
    TaskStatic,
}

impl CellKind {
    pub fn name(&self) -> &'static str {
        match self {
            CellKind::Module => "module",
            CellKind::Net => "net",
            CellKind::Reg => "reg",
            CellKind::Integer => "integer",
            CellKind::Time => "time",
            CellKind::Param => "parameter",
            CellKind::Localparam => "localparam",
            CellKind::Genvar => "genvar",
            CellKind::Event => "event",
            CellKind::MemoryWord => "memory word",
            CellKind::FuncStatic => "function static",
            CellKind::TaskStatic => "task static",
        }
    }
}

/// One row of the frozen object table (index == [`VpiHandle`]).
#[derive(Clone, Debug)]
pub struct ObjMeta {
    /// Canonical dotted path in the cell key space (root module uses the
    /// top module's name; its path prefix for lookups is empty).
    pub name: String,
    pub kind: CellKind,
}

/// One-to-many relation, mirroring `vpi_iterate` relations.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Relation {
    Nets,
    Regs,
    Params,
    Modules,
}

/// Scalar strength values (vpi0/vpi1/vpiX/vpiZ analog), width-1 cells only.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scalar {
    Zero,
    One,
    X,
    Z,
}

/// Requested value format (`vpi_get_value` format constants analog).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    /// 2-state i64; errors when the value carries x/z (deterministic; the C
    /// interface returns garbage here).
    Int,
    /// Width-1 4-state.
    Scalar,
    /// Full 4-state vector at declared width.
    Vector,
    /// Binary string, x/z visible.
    BinStr,
    /// Current simulation time (ignores the cell).
    Time,
}

/// A value crossing the API boundary.
#[derive(Clone, Debug)]
pub enum VpiValue {
    Int(i64),
    Scalar(Scalar),
    Vector(Bits),
    BinStr(String),
    Time(u64),
}

/// Time base for [`Sim::vpi_time`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimeScale {
    /// Precision ticks (the simulator's native `now`).
    Precision,
    /// Simulation units (`$time` semantics: unit/precision ratio applied).
    Sim,
}

/// Callback reason. Slice 1 mirrors cbStartOfSimulation, cbValueChange,
/// cbAfterDelay, cbEndOfSimulation (synch reasons are slice 2).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CbReason {
    StartOfSimulation,
    /// Canonical handle (aliases fold to the same handle at registration).
    ValueChange(VpiHandle),
    /// Fire when `now` reaches `t` (precision ticks), start of slot.
    AfterDelay(u64),
    EndOfSimulation,
}

/// Delivered to the closure at each firing.
pub struct CbInfo {
    pub reason: CbReason,
    /// Precision ticks at the firing point.
    pub time: u64,
    /// For ValueChange: the new value at declared width (post-settle).
    pub value: Option<Bits>,
}

pub type CbFn = Box<dyn FnMut(&mut Sim, &CbInfo)>;

/// Opaque callback id (`vpi_remove_cb` token). Ids are never recycled.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CbId(pub u32);

pub(crate) struct CbEntry {
    pub(crate) reason: CbReason,
    pub(crate) f: CbFn,
}

/// All rVPI state hanging off `Sim`.
#[derive(Default)]
pub struct VpiState {
    /// Sorted at build; index == VpiHandle; frozen.
    pub table: Vec<ObjMeta>,
    /// canonical name → table index (alias members included).
    pub index: HashMap<String, u32>,
    /// Write-burst collapse: canonical handle → newest value, drained at the
    /// fixed fire points (same collapse semantics as the VCD pending map).
    pub pending_vc: BTreeMap<u32, Bits>,
    /// Callback registry with tombstones (ids never recycled).
    pub(crate) cbs: Vec<Option<CbEntry>>,
    /// Time-anchored callbacks: fire tick → registry indices.
    pub timers: BTreeMap<u64, Vec<usize>>,
    next_cb: u32,
}

impl VpiState {
    fn add_cb(&mut self, reason: CbReason, f: CbFn) -> CbId {
        let id = CbId(self.next_cb);
        self.next_cb += 1;
        // Time-anchored reasons are indexed for O(log n) dispatch.
        if let CbReason::AfterDelay(t) = reason {
            self.timers.entry(t).or_default().push(self.cbs.len());
        }
        self.cbs.push(Some(CbEntry { reason, f }));
        id
    }

    fn has_reason(&self, pred: impl Fn(&CbReason) -> bool) -> bool {
        self.cbs
            .iter()
            .any(|c| c.as_ref().is_some_and(|e| pred(&e.reason)))
    }

    pub fn has_vc_callbacks(&self) -> bool {
        self.has_reason(|r| matches!(r, CbReason::ValueChange(_)))
    }

    /// Indices (in registration order) of live callbacks matching `want`.
    fn matching(&self, want: &CbReason) -> Vec<usize> {
        self.cbs
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                c.as_ref()
                    .is_some_and(|e| cb_reason_matches(&e.reason, want))
            })
            .map(|(i, _)| i)
            .collect()
    }
}

fn cb_reason_matches(a: &CbReason, fired: &CbReason) -> bool {
    match (a, fired) {
        (CbReason::StartOfSimulation, CbReason::StartOfSimulation) => true,
        (CbReason::EndOfSimulation, CbReason::EndOfSimulation) => true,
        (CbReason::ValueChange(h1), CbReason::ValueChange(h2)) => h1 == h2,
        // AfterDelay dispatches via the timers map, not matching().
        (CbReason::AfterDelay(_), CbReason::AfterDelay(_)) => false,
        _ => false,
    }
}

impl Sim {
    // ---------- table construction (called from build) ----------

    /// Build the frozen object table: every cell (canonicalized through the
    /// union-find parent map), every memory word, and one synthetic entry per
    /// module scope (a dotted prefix that has children but is not a cell).
    /// Sorted by name — handle ids are deterministic (VCD id precedent).
    pub(crate) fn finish_elab(&mut self, top_name: &str) {
        let mut rows: Vec<(String, CellKind)> = Vec::new();
        let mut cell_names: Vec<String> = self.cells.keys().cloned().collect();
        cell_names.sort();
        for n in &cell_names {
            let canon = self.find_follow(n);
            let kind = self
                .cells
                .get(&canon)
                .or_else(|| self.cells.get(n))
                .map(|c| c.kind)
                .unwrap_or(CellKind::Net);
            rows.push((canon, kind));
        }
        for e in self.mem_elems.keys() {
            rows.push((e.clone(), CellKind::MemoryWord));
        }
        // Module scopes: dotted prefixes of any key that are not cells
        // themselves. The root (empty prefix) is the top module.
        let mut prefixes: Vec<String> = Vec::new();
        for n in cell_names.iter().chain(self.mem_elems.keys()) {
            let mut s = n.as_str();
            while let Some(i) = s.rfind('.') {
                s = &s[..i];
                if s.is_empty() {
                    break;
                }
                if !self.cells.contains_key(s) && !prefixes.contains(&s.to_string()) {
                    prefixes.push(s.to_string());
                }
            }
        }
        for p in prefixes {
            rows.push((p, CellKind::Module));
        }
        rows.push((top_name.to_string(), CellKind::Module)); // root (path "" )
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        rows.dedup_by(|a, b| a.0 == b.0);

        let mut st = VpiState::default();
        for (i, (name, kind)) in rows.into_iter().enumerate() {
            st.index.insert(name.clone(), i as u32);
            st.table.push(ObjMeta { name, kind });
        }
        // Alias members resolve to the canonical handle.
        let aliases: Vec<(String, String)> = self
            .alias_groups
            .iter()
            .flat_map(|(root, members)| {
                members
                    .iter()
                    .map(move |m| (m.clone(), root.clone()))
            })
            .collect();
        for (member, root) in aliases {
            if let Some(idx) = st.index.get(&root).copied() {
                st.index.entry(member).or_insert(idx);
            }
        }
        self.vpi = st;
    }

    fn canon_idx(&self, name: &str) -> Option<u32> {
        if let Some(i) = self.vpi.index.get(name) {
            return Some(*i);
        }
        let canon = self.find_follow(name);
        self.vpi.index.get(&canon).copied()
    }

    // ---------- handles & properties ----------

    /// `vpi_handle_by_name`: accepts bare cell paths ("dut.q", "mem[2]"),
    /// optionally prefixed with the top module name ("tb.dut.q").
    pub fn vpi_handle_by_name(&self, path: &str) -> Option<VpiHandle> {
        let p = path.strip_prefix(&format!("{}.", self.top_name)).unwrap_or(path);
        self.canon_idx(p).map(VpiHandle)
    }

    /// Handle of the root module.
    pub fn vpi_top(&self) -> VpiHandle {
        VpiHandle(self.vpi.index[&self.top_name])
    }

    pub fn vpi_name(&self, h: VpiHandle) -> &str {
        &self.vpi.table[h.0 as usize].name
    }

    pub fn vpi_kind(&self, h: VpiHandle) -> CellKind {
        self.vpi.table[h.0 as usize].kind
    }

    /// Declared width in bits (modules → 0).
    pub fn vpi_size(&self, h: VpiHandle) -> u32 {
        let name = &self.vpi.table[h.0 as usize].name;
        if let Some((root, wi)) = self.mem_elems.get(name) {
            return self
                .cells
                .get(&self.find_follow(root))
                .and_then(|c| c.mem.as_ref())
                .and_then(|m| m.words.get(*wi))
                .map(|w| w.width)
                .unwrap_or(0);
        }
        self.cells
            .get(&self.find_follow(name))
            .map(|c| c.bits.width)
            .unwrap_or(0)
    }

    pub fn vpi_signed(&self, h: VpiHandle) -> Option<bool> {
        let name = &self.vpi.table[h.0 as usize].name;
        self.cells.get(&self.find_follow(name)).map(|c| c.signed)
    }

    /// Declared `[msb, lsb]`; scalars report (0, 0).
    pub fn vpi_range(&self, h: VpiHandle) -> Option<(i64, i64)> {
        let name = &self.vpi.table[h.0 as usize].name;
        self.cells
            .get(&self.find_follow(name))
            .map(|c| (c.msb, c.lsb))
    }

    /// `vpi_iterate`: children of a module handle under `rel`, in table
    /// (name-sorted) order.
    pub fn vpi_children(&self, h: VpiHandle, rel: Relation) -> Vec<VpiHandle> {
        let meta = &self.vpi.table[h.0 as usize];
        if meta.kind != CellKind::Module {
            return Vec::new();
        }
        // The root's path prefix in the cell key space is empty.
        let prefix = if h == self.vpi_top() {
            String::new()
        } else {
            format!("{}.", meta.name)
        };
        let mut out = Vec::new();
        for (i, row) in self.vpi.table.iter().enumerate() {
            if VpiHandle(i as u32) == h {
                continue; // the module row itself
            }
            let Some(rest) = row.name.strip_prefix(&prefix) else {
                continue;
            };
            if rest.is_empty() {
                continue;
            }
            // Immediate child only: no further '.' in the remainder
            // (memory words "mem[2]" count under their memory's module).
            if rest.contains('.') {
                continue;
            }
            // Memory arrays surface only through their word handles in
            // child relations (C-VPI vpiMemory vs vpiMemoryWord split).
            let is_mem_array = row.kind == CellKind::Reg && self
                .cells
                .get(&self.find_follow(&row.name))
                .and_then(|c| c.mem.as_ref())
                .is_some_and(|m| !m.words.is_empty());
            let want = match rel {
                Relation::Nets => matches!(row.kind, CellKind::Net),
                Relation::Regs => !is_mem_array
                    && matches!(
                        row.kind,
                        CellKind::Reg | CellKind::Integer | CellKind::Time | CellKind::MemoryWord
                    ),
                Relation::Params => {
                    matches!(row.kind, CellKind::Param | CellKind::Localparam | CellKind::Genvar)
                }
                Relation::Modules => row.kind == CellKind::Module,
            };
            if want {
                out.push(VpiHandle(i as u32));
            }
        }
        out
    }

    // ---------- values ----------

    /// Raw 4-state bits of a handle (cells or one memory word).
    pub fn vpi_bits(&self, h: VpiHandle) -> Result<Bits, SimError> {
        let name = &self.vpi.table[h.0 as usize].name;
        if let Some((root, wi)) = self.mem_elems.get(name) {
            let canon = self.find_follow(root);
            return match self.cells.get(&canon).and_then(|c| c.mem.as_ref()) {
                Some(m) if *wi < m.words.len() => Ok(m.words[*wi].clone()),
                _ => Err(SimError::new(format!("bad memory word `{name}`"), None)),
            };
        }
        let canon = self.find_follow(name);
        match self.cells.get(&canon) {
            Some(c) if c.mem.is_none() => Ok(c.bits.clone()),
            Some(_) => Err(SimError::new(
                format!("cannot read whole memory `{name}` (use word handles)"),
                None,
            )),
            None => Err(SimError::new(format!("no object `{name}`"), None)),
        }
    }

    /// `vpi_get_value` in an explicit format.
    pub fn vpi_get_as(&self, h: VpiHandle, f: Format) -> Result<VpiValue, SimError> {
        match f {
            Format::Time => Ok(VpiValue::Time(self.now)),
            Format::BinStr => Ok(VpiValue::BinStr(self.vpi_bits(h)?.fmt_bin())),
            Format::Vector => Ok(VpiValue::Vector(self.vpi_bits(h)?)),
            Format::Scalar => {
                let b = self.vpi_bits(h)?;
                if b.width != 1 {
                    return Err(SimError::new("scalar format needs a 1-bit object", None));
                }
                Ok(VpiValue::Scalar(scalar_of(&b)))
            }
            Format::Int => {
                let b = self.vpi_bits(h)?;
                if b.has_unknown() {
                    return Err(SimError::new(
                        "int format on a value carrying x/z (use Vector/BinStr)",
                        None,
                    ));
                }
                let s = self.vpi_signed(h).unwrap_or(false);
                let raw = b.to_u64_lossy();
                let v = if s && b.width > 0 && b.width < 64 && raw & (1 << (b.width - 1)) != 0 {
                    (raw | !((1u64 << b.width) - 1)) as i64
                } else {
                    raw as i64
                };
                Ok(VpiValue::Int(v))
            }
        }
    }

    /// `vpi_put_value` with `vpiNoDelay` semantics: writes through the same
    /// path as a blocking assignment (immediate + settle + wake). Delay modes
    /// are slice 2.
    pub fn vpi_put_value(&mut self, h: VpiHandle, v: VpiValue) -> Result<(), SimError> {
        let bits = match v {
            VpiValue::Int(i) => Bits::from_u64(64, i as u64)?,
            VpiValue::Vector(b) => b,
            VpiValue::BinStr(s) => {
                let mut data = 0u64;
                let mut mask = 0u64;
                for c in s.chars().filter(|c| !c.is_whitespace() && *c != '_') {
                    let (d, m) = match c {
                        '0' => (0u64, 0u64),
                        '1' => (1, 0),
                        'x' | 'X' => (0, 1),
                        'z' | 'Z' => (1, 1),
                        _ => return Err(SimError::new(format!("bad binary digit `{c}`"), None)),
                    };
                    data = (data << 1) | d;
                    mask = (mask << 1) | m;
                }
                let width = 64 - (data | mask).leading_zeros().max(1);
                let _ = width;
                Bits::n(64, data as u128, mask as u128)
            }
            VpiValue::Scalar(sc) => Bits::n(
                1,
                match sc {
                    Scalar::One | Scalar::Z => 1,
                    _ => 0,
                },
                match sc {
                    Scalar::X | Scalar::Z => 1,
                    _ => 0,
                },
            ),
            VpiValue::Time(_) => {
                return Err(SimError::new("cannot put a Time value", None));
            }
        };
        self.vpi_write_bits(h, &bits)
    }

    pub(crate) fn vpi_write_bits(&mut self, h: VpiHandle, bits: &Bits) -> SResult<()> {
        // A put before the first run_until still wakes edge-waiters — but
        // without init_once the always-block prologues have never run, so
        // no process is registered on its sensitivity list yet and the
        // edge is silently lost (caught by the spec-i2c read-transaction
        // lockstep: the DUT's serial shift dropped every bit but the
        // last). Seed the scheduler BEFORE the write so the edge is seen
        // against a fully registered sensitivity state.
        self.init_once()?;
        let name = self.vpi.table[h.0 as usize].name.clone();
        if let Some((root, wi)) = self.mem_elems.get(&name).cloned() {
            let canon = self.find_follow(&root);
            let (w, esigned) = match self.cells.get(&canon) {
                Some(c) => (c.bits.width, c.signed),
                None => return Err(SimError::new(format!("no memory `{root}`"), None)),
            };
            let word = if esigned {
                bits.sadjust(w)?
            } else {
                bits.uadjust(w)?
            };
            {
                let c = self.cells.get_mut(&canon).unwrap();
                c.mem.as_mut().unwrap().words[wi] = word.clone();
            }
            if let Some(w) = self.vcd.as_mut() {
                if w.started() {
                    w.record_change(&format!("{canon}[{}]", wi), &word.fmt_bin());
                }
            }
            let idx = self.vpi.index.get(&name).copied();
            if let Some(idx) = idx {
                self.vpi.pending_vc.insert(idx, word);
            }
            self.settle()?;
            self.wake_check()?;
            self.drain_vc_callbacks()?;
            return Ok(());
        }
        let canon = self.find_follow(&name);
        let (esigned, is_mem) = {
            let c = self
                .cells
                .get(&canon)
                .ok_or_else(|| SimError::new(format!("no object `{name}`"), None))?;
            (c.signed, c.mem.is_some())
        };
        if is_mem {
            return Err(SimError::new(
                format!("cannot write whole memory `{name}` (use word handles)"),
                None,
            ));
        }
        self.set_name(&canon, bits.clone(), esigned)?;
        self.settle()?;
        self.wake_check()?;
        self.drain_vc_callbacks()
    }

    // ---------- time & control ----------

    pub fn vpi_time(&self, s: TimeScale) -> u64 {
        match s {
            TimeScale::Precision => self.now,
            TimeScale::Sim => {
                if self.unit_fs == 0 {
                    return self.now;
                }
                // Precision ticks → simulation units (inverse of delay_ticks).
                self.now * self.prec_fs / self.unit_fs
            }
        }
    }

    /// `vpi_control(vpiFinish)` (Stop conflates; matches `$stop` behavior).
    pub fn finish(&mut self) {
        self.finished = true;
    }

    // ---------- callbacks ----------

    pub fn register_cb(&mut self, reason: CbReason, f: CbFn) -> CbId {
        let reason = match reason {
            // Canonicalize so aliases and the write-site records agree.
            CbReason::ValueChange(h) => {
                let name = self.vpi.table[h.0 as usize].name.clone();
                match self.canon_idx(&name) {
                    Some(i) => CbReason::ValueChange(VpiHandle(i)),
                    None => CbReason::ValueChange(h),
                }
            }
            r => r,
        };
        if let CbReason::ValueChange(h) = &reason {
            debug_assert!((*h).0 < self.vpi.table.len() as u32);
        }
        self.vpi.add_cb(reason, f)
    }

    pub fn remove_cb(&mut self, id: CbId) {
        if (id.0 as usize) < self.vpi.cbs.len() {
            self.vpi.cbs[id.0 as usize] = None;
        }
    }

    /// Drain collapsed value-change callbacks (take-drain; a callback that
    /// writes appends to the freshly-taken map's successor — fires at the
    /// next drain point, never re-entrantly).
    pub fn drain_vc_callbacks(&mut self) -> SResult<()> {
        if !self.vpi.has_vc_callbacks() || self.vpi.pending_vc.is_empty() {
            self.vpi.pending_vc.clear();
            return Ok(());
        }
        let due: Vec<(u32, Bits)> = std::mem::take(&mut self.vpi.pending_vc)
            .into_iter()
            .collect();
        let now = self.now;
        let mut invocations = 0u32;
        for (hidx, bits) in due {
            let ids = self.vpi.matching(&CbReason::ValueChange(VpiHandle(hidx)));
            for i in ids {
                let Some(mut entry) = self.vpi.cbs[i].take() else {
                    continue;
                };
                invocations += 1;
                if invocations > CB_CASCADE_BUDGET {
                    self.vpi.cbs[i] = Some(entry);
                    return Err(SimError::new(
                        "rVPI callback cascade exceeded budget (runaway value-change loop?)",
                        None,
                    ));
                }
                let info = CbInfo {
                    reason: CbReason::ValueChange(VpiHandle(hidx)),
                    time: now,
                    value: Some(bits.clone()),
                };
                (entry.f)(self, &info);
                self.vpi.cbs[i] = Some(entry);
            }
        }
        Ok(())
    }

    /// Fire `cbAfterDelay(t)` timers due at the newly-advanced `now`.
    pub fn fire_due_timers(&mut self) -> SResult<()> {
        loop {
            let Some((&t, _)) = self.vpi.timers.iter().next() else {
                break;
            };
            if t > self.now {
                break;
            }
            let ids = self.vpi.timers.remove(&t).unwrap_or_default();
            let now = self.now;
            let mut invocations = 0u32;
            for i in ids {
                let Some(mut entry) = self.vpi.cbs[i].take() else {
                    continue;
                };
                invocations += 1;
                if invocations > CB_CASCADE_BUDGET {
                    self.vpi.cbs[i] = Some(entry);
                    return Err(SimError::new(
                        "rVPI callback cascade exceeded budget (timer loop?)",
                        None,
                    ));
                }
                let info = CbInfo {
                    reason: CbReason::AfterDelay(t),
                    time: now,
                    value: None,
                };
                (entry.f)(self, &info);
                self.vpi.cbs[i] = Some(entry);
            }
        }
        Ok(())
    }

    /// `cbStartOfSimulation`.
    pub fn fire_start_of_simulation(&mut self) -> SResult<()> {
        let ids = self.vpi.matching(&CbReason::StartOfSimulation);
        let now = self.now;
        for i in ids {
            let Some(mut entry) = self.vpi.cbs[i].take() else {
                continue;
            };
            let info = CbInfo {
                reason: CbReason::StartOfSimulation,
                time: now,
                value: None,
            };
            (entry.f)(self, &info);
            self.vpi.cbs[i] = Some(entry);
        }
        Ok(())
    }

    /// `cbEndOfSimulation` (both natural-end and `$finish` exits; not on
    /// simulation errors — registered divergence).
    pub fn fire_end_of_simulation(&mut self) -> SResult<()> {
        let ids = self.vpi.matching(&CbReason::EndOfSimulation);
        let now = self.now;
        for i in ids {
            let Some(mut entry) = self.vpi.cbs[i].take() else {
                continue;
            };
            let info = CbInfo {
                reason: CbReason::EndOfSimulation,
                time: now,
                value: None,
            };
            (entry.f)(self, &info);
            self.vpi.cbs[i] = Some(entry);
        }
        Ok(())
    }
}

fn scalar_of(b: &Bits) -> Scalar {
    let unknown = b.low_mask() & 1 != 0;
    let bit = b.low_data() & 1 != 0;
    match (unknown, bit) {
        (false, false) => Scalar::Zero,
        (false, true) => Scalar::One,
        (true, false) => Scalar::X,
        (true, true) => Scalar::Z,
    }
}
