//! The Phase-1 interpreter: flat elaboration (scope-chain name resolution,
//! union-find port connections, generate unrolling) and the event scheduler
//! (IEEE 1364-2005 ch.9/§11-inspired): processes (initial/always) lowered to
//! jump-code with suspend points, a time wheel, the NBA region, and edge
//! sensitivity wakes.
//!
//! ADR-0001 "segments": every process body lowers to a straight instruction
//! list whose suspend points are exactly `#delay`/`@(...)` — tier B can emit
//! async code per segment chain from the same lowering.

use crate::eval::{self, eval, eval_ctx, Lookup, SimError, SResult, Val};
use crate::value::{low_bits, Bits};
pub use crate::vpi::{CellKind, VpiHandle, VpiState};
use rcs_diag::Span;
use std::collections::{BTreeMap, HashMap, VecDeque};
use vl_syntax::ast::{self, CaseItem, CaseKind, DeclKind, Direction, EventEdge, Expr, Module, Stmt};

// ---------- data model ----------

#[derive(Clone)]
pub struct Cell {
    pub bits: Bits,
    pub signed: bool,
    pub msb: i64,
    pub lsb: i64,
    /// Unpacked (memory) words; `bits` carries the element width/sign layout.
    pub mem: Option<Memory>,
    /// Object class for the rVPI table (frozen at elaboration).
    pub kind: CellKind,
}

#[derive(Clone)]
pub struct Memory {
    /// Logical address of word 0 (normalized decl range bound).
    pub addr_lsb: i64,
    pub words: Vec<Bits>,
}

struct Assign {
    lhs: Expr,
    rhs: Expr,
    lhs_w: u32,
}

#[derive(Clone)]
struct FuncDef {
    instrs: std::rc::Rc<Vec<Instr>>,
    inputs: Vec<(String, u32, bool)>,
    ret: String,
    ret_w: u32,
    ret_s: bool,
}

#[derive(Clone)]
struct TaskDef {
    /// Raw body + task-local scope (lowered inline at each call site).
    body: Vec<Stmt>,
    scope: Scope,
    ports: Vec<(String, Direction, u32, bool)>,
}

/// Name-resolution scope chain, innermost last: `{prefix}{name}` is tried
/// from the innermost scope outward; the first registered cell/def wins.
#[derive(Clone)]
struct Scope {
    chain: Vec<String>,
}

impl Scope {
    fn new(root: String) -> Scope {
        Scope { chain: vec![root] }
    }
    #[allow(dead_code)]
    fn push(&self, prefix: String) -> Scope {
        let mut c = self.chain.clone();
        c.push(prefix);
        Scope { chain: c }
    }
    fn innermost(&self) -> &str {
        self.chain.last().map(|s| s.as_str()).unwrap_or("")
    }
}

// ---------- process VM ----------

#[derive(Clone, Debug)]
enum Instr {
    BAssign(Expr, Expr),
    NbAssign(Expr, Expr),
    JumpIfFalse(Expr, usize),
    Jump(usize),
    Case {
        kind: CaseKind,
        expr: Expr,
        arms: Vec<(Vec<Expr>, usize)>,
        default: usize,
    },
    RepeatInit(Expr),
    RepeatLoop {
        end: usize,
    },
    Delay(ast::DelayVal),
    Wait(Vec<EventEdge>),
    SysCall(String, Vec<Expr>),
}

#[derive(PartialEq, Clone, Copy, Debug)]
enum PState {
    Runnable,
    Waiting,
    Delayed,
    Done,
}

struct Process {
    instrs: std::rc::Rc<Vec<Instr>>,
    pc: usize,
    counters: Vec<u64>,
    state: PState,
    waits: Vec<(EventEdge, Bits)>,
    #[allow(dead_code)]
    label: String,
}

#[derive(PartialEq)]
enum Flow {
    Next,
    Suspend,
    Finish,
    End,
}

pub struct Sim {
    pub(crate) parent: HashMap<String, String>,
    pub(crate) cells: HashMap<String, Cell>,
    assigns: Vec<Assign>,
    modules: HashMap<String, Module>,
    funcs: HashMap<String, FuncDef>,
    tasks: HashMap<String, TaskDef>,
    procs: Vec<Process>,
    runnable: VecDeque<usize>,
    inactive: VecDeque<usize>,
    delayed: BTreeMap<u64, Vec<usize>>,
    nba: Vec<(Expr, Val)>,
    pub(crate) now: u64,
    /// `#d` advances the clock by `d * (unit_fs / prec_fs)` precision ticks.
    pub(crate) unit_fs: u64,
    pub(crate) prec_fs: u64,
    out: String,
    pub(crate) finished: bool,
    genblk_counter: HashMap<String, u32>,
    /// Declared net/reg vars eligible for VCD dump (params/genvars/statics excluded).
    dumpable: std::collections::BTreeSet<String>,
    /// union-find alias groups: root → all member names (VCD records each).
    pub(crate) alias_groups: HashMap<String, Vec<String>>,
    pub(crate) vcd: Option<crate::vcd::VcdWriter>,
    dump_path: Option<String>,
    /// Simulation precision, for the VCD `$timescale` header.
    prec_name: &'static str,
    /// Primitive gate instances (and/or/not/buf/bufif*/notif*): continuous
    /// outputs re-evaluated in the settle loop.
    gates: Vec<GateInst>,
    /// VCD element vars: full `root[i]` name -> (memory cell root, word index).
    pub(crate) mem_elems: HashMap<String, (String, usize)>,
    /// Base dir for `$readmemh/$readmemb` path resolution (default: cwd).
    file_base: std::path::PathBuf,
    /// Injected files for `$readmem*` (wasm/browser hosts): name → contents;
    /// checked before the real filesystem.
    pub(crate) injected_files: HashMap<String, String>,
    /// `for (...) begin : NAME` — the iteration scope already carries
    /// `NAME[i].`; the inner GenBlock with the same name must not push again.
    pending_gen_block: Option<String>,
    /// rVPI state (frozen object table + callbacks), built by finish_elab.
    pub(crate) vpi: VpiState,
    /// Top module name (rVPI path-prefix stripping).
    pub(crate) top_name: String,
}

// ---------- primitive gates ----------

#[derive(Clone, Copy, PartialEq, Debug)]
enum GateKind {
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Xnor,
    Not,
    Buf,
    Bufif0,
    Bufif1,
    Notif0,
    Notif1,
}

fn gate_kind(s: &str) -> Option<GateKind> {
    Some(match s {
        "and" => GateKind::And,
        "nand" => GateKind::Nand,
        "or" => GateKind::Or,
        "nor" => GateKind::Nor,
        "xor" => GateKind::Xor,
        "xnor" => GateKind::Xnor,
        "not" => GateKind::Not,
        "buf" => GateKind::Buf,
        "bufif0" => GateKind::Bufif0,
        "bufif1" => GateKind::Bufif1,
        "notif0" => GateKind::Notif0,
        "notif1" => GateKind::Notif1,
        _ => return None,
    })
}

struct GateInst {
    kind: GateKind,
    outs: Vec<Expr>,
    ins: Vec<Expr>,
}

impl GateInst {
    fn split(kind: GateKind, conns: &[Expr], span: Span) -> SResult<GateInst> {
        use GateKind::*;
        let (outs, ins): (Vec<Expr>, Vec<Expr>) = match kind {
            And | Nand | Or | Nor | Xor | Xnor => {
                if conns.len() < 3 {
                    return Err(SimError::at("gate needs 1 output and >=2 inputs", span));
                }
                (vec![conns[0].clone()], conns[1..].to_vec())
            }
            Not | Buf => {
                if conns.len() < 2 {
                    return Err(SimError::at("not/buf needs >=1 output and 1 input", span));
                }
                (
                    conns[..conns.len() - 1].to_vec(),
                    vec![conns[conns.len() - 1].clone()],
                )
            }
            Bufif0 | Bufif1 | Notif0 | Notif1 => {
                if conns.len() != 3 {
                    return Err(SimError::at(
                        "bufif/notif needs output, data, enable terminals",
                        span,
                    ));
                }
                (vec![conns[0].clone()], vec![conns[1].clone(), conns[2].clone()])
            }
        };
        Ok(GateInst { kind, outs, ins })
    }
}

const GATE_X: Bits = Bits { width: 1, data: 0, mask: 1 };
const GATE_Z: Bits = Bits { width: 1, data: 1, mask: 1 };
const fn gate_b(t: bool) -> Bits {
    Bits { width: 1, data: t as u64, mask: 0 }
}

/// IEEE 1364-2005 §7 primitive tables (4-state; no strengths — a disabled
/// bufif/notif drives z; x control or x/z input yields x).
fn gate_fold(kind: GateKind, ins: &[Bits]) -> Bits {
    use GateKind::*;
    let v = |b: &Bits| -> Option<bool> {
        if b.has_unknown() { None } else { Some(b.to_u64_lossy() != 0) }
    };
    let xs: Vec<Option<bool>> = ins.iter().map(v).collect();
    let any = |p: fn(&Option<bool>) -> bool| xs.iter().any(p);
    match kind {
        And | Nand => {
            let b = if any(|x| *x == Some(false)) {
                Some(false)
            } else if any(|x| x.is_none()) {
                None
            } else {
                Some(true)
            };
            b.map_or(GATE_X, |t| gate_b(if kind == And { t } else { !t }))
        }
        Or | Nor => {
            let b = if any(|x| *x == Some(true)) {
                Some(true)
            } else if any(|x| x.is_none()) {
                None
            } else {
                Some(false)
            };
            b.map_or(GATE_X, |t| gate_b(if kind == Or { t } else { !t }))
        }
        Xor | Xnor => {
            if any(|x| x.is_none()) {
                GATE_X
            } else {
                let odd = xs.iter().filter(|x| **x == Some(true)).count() % 2 == 1;
                gate_b(if kind == Xor { odd } else { !odd })
            }
        }
        Not | Buf => xs[0].map_or(GATE_X, |t| gate_b(if kind == Not { !t } else { t })),
        Bufif1 | Bufif0 | Notif1 | Notif0 => {
            let (data, en) = (xs[0], xs[1]);
            let enabled = match en {
                Some(true) => kind == Bufif1 || kind == Notif1,
                Some(false) => kind == Bufif0 || kind == Notif0,
                None => return GATE_X,
            };
            if !enabled {
                return GATE_Z;
            }
            match data {
                Some(t) => {
                    let inv = kind == Notif1 || kind == Notif0;
                    gate_b(if inv { !t } else { t })
                }
                None => GATE_X,
            }
        }
    }
}

/// Parse one `$readmem*` value token: LSB-aligned into `w` bits, truncating
/// extra high bits; `x/z/?` carry into the unknown mask (x → x, z → z).
fn readmem_val(tok: &[char], bin: bool, w: u32, task: &str) -> SResult<Bits> {
    let mut data: u64 = 0;
    let mut mask: u64 = 0;
    let mut nbits = 0usize;
    for &c in tok {
        if c == '_' {
            continue;
        }
        let (d, m) = if bin {
            match c {
                '0' => (0u64, 0u64),
                '1' => (1, 0),
                'x' | 'X' => (0, 1),
                'z' | 'Z' | '?' => (1, 1),
                _ => {
                    return Err(SimError::new(
                        format!("{task}: bad binary digit `{c}`"),
                        None,
                    ))
                }
            }
        } else {
            match c {
                '0'..='9' | 'a'..='f' | 'A'..='F' => (c.to_digit(16).unwrap() as u64, 0),
                'x' | 'X' => (0, 1),
                'z' | 'Z' | '?' => (1, 1),
                _ => {
                    return Err(SimError::new(
                        format!("{task}: bad hex digit `{c}`"),
                        None,
                    ))
                }
            }
        };
        let sh = if bin { 1 } else { 4 };
        data = (data << sh) | d;
        mask = (mask << sh) | m;
        nbits += sh;
    }
    let width = w as usize;
    let (data, mask) = if nbits > width {
        let drop = nbits - width;
        (data >> drop, mask >> drop)
    } else {
        let pad = width - nbits;
        (data << pad, mask << pad)
    };
    Ok(Bits {
        width: w,
        data,
        mask,
    })
}

// ---------- build / elaboration ----------

impl Sim {
    pub fn build(modules: &[Module], top: &str, timescale: Option<&str>) -> SResult<Sim> {
        let (unit_fs, prec_fs) = parse_timescale(timescale);
        let prec_name = fs_name(prec_fs);
        let mut sim = Sim {
            parent: HashMap::new(),
            cells: HashMap::new(),
            assigns: Vec::new(),
            modules: modules.iter().map(|m| (m.name.clone(), m.clone())).collect(),
            funcs: HashMap::new(),
            tasks: HashMap::new(),
            procs: Vec::new(),
            runnable: VecDeque::new(),
            inactive: VecDeque::new(),
            delayed: BTreeMap::new(),
            nba: Vec::new(),
            now: 0,
            unit_fs,
            prec_fs,
            out: String::new(),
            finished: false,
            genblk_counter: HashMap::new(),
            dumpable: std::collections::BTreeSet::new(),
            alias_groups: HashMap::new(),
            vcd: None,
            dump_path: None,
            prec_name,
            gates: Vec::new(),
            mem_elems: HashMap::new(),
            file_base: std::path::PathBuf::new(),
            injected_files: HashMap::new(),
            pending_gen_block: None,
            vpi: VpiState::default(),
            top_name: top.to_string(),
        };
        let m = sim
            .modules
            .get(top)
            .ok_or_else(|| SimError::new(format!("top module `{top}` not found"), None))?
            .clone();
        let scope = Scope::new(String::new());
        sim.elab_module(&m, &scope, &HashMap::new(), &HashMap::new())?;
        sim.finish_elab(top);
        Ok(sim)
    }

    /// Non-mutating union-find chase: `find()` path-compresses (needs
    /// `&mut`); the rVPI read API is `&self`. Chains are frozen after
    /// elaboration — `union` only runs during instance elab.
    pub(crate) fn find_follow(&self, name: &str) -> String {
        let mut cur = name.to_string();
        while let Some(p) = self.parent.get(&cur) {
            if p == &cur {
                break;
            }
            cur = p.clone();
        }
        cur
    }

    pub fn take_output(&mut self) -> String {
        std::mem::take(&mut self.out)
    }

    /// Resolve `$readmem*` relative paths against `dir` (default: process cwd).
    pub fn set_file_base(&mut self, dir: std::path::PathBuf) {
        self.file_base = dir;
    }

    /// Provide a virtual file for `$readmem*` (checked before the real
    /// filesystem — the wasm/browser host has none).
    pub fn inject_file(&mut self, name: &str, contents: &str) {
        self.injected_files.insert(name.to_string(), contents.to_string());
    }

    fn elab_module(
        &mut self,
        m: &Module,
        scope: &Scope,
        param_overrides: &HashMap<String, Bits>,
        param_override_signed: &HashMap<String, bool>,
    ) -> SResult<()> {
        // Pass 1a: header parameters (later params may reference earlier ones).
        for p in &m.params {
            let full = format!("{}{}", scope.innermost(), p.name);
            if let Some(v) = param_overrides.get(&p.name) {
                self.cells.insert(
                    full,
                    Cell {
                        bits: *v,
                        signed: *param_override_signed.get(&p.name).unwrap_or(&false),
                        msb: v.to_i64_lossy(),
                        lsb: 0,
                        mem: None,
                        kind: CellKind::Param,
                    },
                );
                continue;
            }
            let pe = self.prefix_expr(&p.init, scope)?;
            let v = eval(&pe, self)?;
            let (bits, signed) = v.to_bits(p.span)?;
            self.cells.insert(
                full,
                Cell {
                    bits,
                    signed,
                    msb: bits.to_i64_lossy(),
                    lsb: 0,
                    mem: None,
                    kind: CellKind::Param,
                },
            );
        }
        // Pass 1b: item parameters, then functions/tasks (constant
        // functions are legal in port ranges and decl inits), then
        // nets/regs.
        for it in &m.items {
            if let ast::ModuleItem::Decl(d) = it {
                if matches!(d.kind, DeclKind::Parameter | DeclKind::Localparam) {
                    self.elab_param_decl(d, scope)?;
                }
            }
        }
        for it in &m.items {
            if let ast::ModuleItem::Function(f) = it {
                self.register_func(f, scope)?;
            } else if let ast::ModuleItem::Task(t) = it {
                self.register_task(t, scope)?;
            }
        }
        // ANSI header ports are declarations too — register any that no item
        // declared (non-ANSI ports get their cells from dir-decl items).
        for p in &m.ports {
            if p.dir.is_none() {
                continue;
            }
            let name = format!("{}{}", scope.innermost(), p.name);
            if self.cells.contains_key(&name) {
                continue;
            }
            self.dumpable.insert(name.clone());
            let (w, msb, lsb) = match &p.range {
                Some(r) => {
                    let mpe = self.prefix_expr(&r.msb, scope)?;
                    let lpe = self.prefix_expr(&r.lsb, scope)?;
                    let msv = eval(&mpe, self)?.to_bits(r.span)?.0.to_i64_lossy();
                    let lsv = eval(&lpe, self)?.to_bits(r.span)?.0.to_i64_lossy();
                    let w = (msv - lsv).unsigned_abs() as u32 + 1;
                    (w, msv, lsv)
                }
                None => (1, 0, 0),
            };
            self.cells.insert(
                name,
                Cell {
                    bits: Bits::zeros(w)?,
                    signed: p.signed,
                    msb,
                    lsb,
                    mem: None,
                    kind: CellKind::Net,
                },
            );
        }
        for it in &m.items {
            if let ast::ModuleItem::Decl(d) = it {
                if !matches!(d.kind, DeclKind::Parameter | DeclKind::Localparam) {
                    self.elab_var_decl(d, scope)?;
                }
            }
        }
        for it in &m.items {
            match it {
                ast::ModuleItem::Function(f) => self.register_func(f, scope)?,
                ast::ModuleItem::Task(t) => self.register_task(t, scope)?,
                _ => {}
            }
        }
        // Pass 2: behaviors — generate constructs first (they may declare
        // nets that other module-level behaviors reference hierarchically).
        for it in &m.items {
            if matches!(
                it,
                ast::ModuleItem::GenerateRegion(_)
                    | ast::ModuleItem::GenIf { .. }
                    | ast::ModuleItem::GenFor { .. }
                    | ast::ModuleItem::GenCase { .. }
            ) {
                self.elab_item(it, scope)?;
            }
        }
        for it in &m.items {
            if !matches!(
                it,
                ast::ModuleItem::GenerateRegion(_)
                    | ast::ModuleItem::GenIf { .. }
                    | ast::ModuleItem::GenFor { .. }
                    | ast::ModuleItem::GenCase { .. }
                    | ast::ModuleItem::Decl(_)
                    | ast::ModuleItem::Function(_)
                    | ast::ModuleItem::Task(_)
            ) {
                self.elab_item(it, scope)?;
            }
        }
        Ok(())
    }

    fn elab_param_decl(&mut self, d: &ast::Decl, scope: &Scope) -> SResult<()> {
        for e in &d.entries {
            let init = e
                .init
                .as_ref()
                .ok_or_else(|| SimError::at("parameter without initializer", e.span))?;
            let pe = self.prefix_expr(init, scope)?;
            let v = eval(&pe, self)?;
            let (bits, signed) = v.to_bits(e.span)?;
            let name = format!("{}{}", scope.innermost(), e.name);
            self.cells.insert(
                name,
                Cell {
                    bits,
                    signed,
                    msb: bits.to_i64_lossy(),
                    lsb: 0,
                    mem: None,
                    kind: if matches!(d.kind, DeclKind::Localparam) { CellKind::Localparam } else { CellKind::Param },
                },
            );
        }
        Ok(())
    }

    fn elab_var_decl(&mut self, d: &ast::Decl, scope: &Scope) -> SResult<()> {
        match d.kind {
            DeclKind::Genvar | DeclKind::Event => {}
            DeclKind::Real | DeclKind::Realtime => {
                return Err(SimError::at(
                    "real variables are not yet supported (milestone M7)",
                    d.span,
                ))
            }
            _ => {
                let (width, signed, msb, lsb) = match &d.range {
                    Some(r) => {
                        let mpe = self.prefix_expr(&r.msb, scope)?;
                        let lpe = self.prefix_expr(&r.lsb, scope)?;
                        let msv = eval(&mpe, self)?.to_bits(r.span)?.0.to_i64_lossy();
                        let lsv = eval(&lpe, self)?.to_bits(r.span)?.0.to_i64_lossy();
                        let w = (msv - lsv).unsigned_abs() as u32 + 1;
                        (w, d.signed, msv, lsv)
                    }
                    None => match d.kind {
                        DeclKind::Integer => (32, true, 31, 0),
                        DeclKind::Time | DeclKind::Realtime => (64, false, 63, 0),
                        _ => (1, d.signed, 0, 0),
                    },
                };
                for e in &d.entries {
                    // Unpacked dims (memories): exactly one range supported.
                    let mem = match e.unpacked.len() {
                        0 => None,
                        1 => {
                            let ur = &e.unpacked[0];
                            let mpe = self.prefix_expr(&ur.msb, scope)?;
                            let lpe = self.prefix_expr(&ur.lsb, scope)?;
                            let mb = eval(&mpe, self)?.to_bits(ur.span)?.0;
                            let lb = eval(&lpe, self)?.to_bits(ur.span)?.0;
                            if mb.has_unknown() || lb.has_unknown() {
                                return Err(SimError::at(
                                    "memory range bounds contain x/z",
                                    ur.span,
                                ));
                            }
                            let (m, l) = (mb.to_i64_lossy(), lb.to_i64_lossy());
                            let (lo, hi) = (m.min(l), m.max(l));
                            Some(Memory {
                                addr_lsb: lo,
                                words: vec![Bits::zeros(width)?; (hi - lo + 1).max(0) as usize],
                            })
                        }
                        _ => {
                            return Err(SimError::at(
                                "multi-dimensional unpacked arrays are not yet supported (M8)",
                                e.span,
                            ))
                        }
                    };
                    // Declarations always land in the innermost scope.
                    let name = format!("{}{}", scope.innermost(), e.name);
                    if let Some(m) = &mem {
                        for i in 0..m.words.len() {
                            let en = format!("{name}[{}]", m.addr_lsb + i as i64);
                            self.dumpable.insert(en.clone());
                            self.mem_elems.insert(en, (name.clone(), i));
                        }
                    } else if !name.contains('[') {
                        self.dumpable.insert(name.clone());
                    }
                    self.cells.insert(
                        name.clone(),
                        Cell {
                            bits: Bits::zeros(width)?,
                            signed,
                            msb,
                            lsb,
                            mem,
                            kind: var_decl_kind(d.kind),
                        },
                    );
                    if let Some(init) = &e.init {
                        let pe = self.prefix_expr(init, scope)?;
                        if matches!(d.kind, DeclKind::Net(_)) {
                            // `wire w = expr` is a continuous assignment with
                            // context-determined width (upstream semantics).
                            self.assigns.push(Assign {
                                lhs: Expr::Ident {
                                    span: e.span,
                                    name: name.clone(),
                                },
                                rhs: pe,
                                lhs_w: width,
                            });
                        } else {
                            let v = eval_ctx(&pe, self, width)?;
                            let (b, s) = v.to_bits(e.span)?;
                            self.set_name(&name, b, s)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn register_func(&mut self, f: &ast::Function, scope: &Scope) -> SResult<()> {
        let fname = f.name.clone();
        let fscope = scope.push(format!("{fname}."));
        let ret_w = match (&f.ret_range, f.ret_type) {
            (Some(r), _) => {
                let mpe = self.prefix_expr(&r.msb, scope)?;
                let lpe = self.prefix_expr(&r.lsb, scope)?;
                let msv = eval(&mpe, self)?.to_bits(r.span)?.0.to_i64_lossy();
                let lsv = eval(&lpe, self)?.to_bits(r.span)?.0.to_i64_lossy();
                (msv - lsv).unsigned_abs() as u32 + 1
            }
            (None, Some(ast::ParamType::Integer)) => 32,
            (None, Some(ast::ParamType::Time | ast::ParamType::Realtime)) => 64,
            (None, _) => 1,
        };
        let ret_s = f.ret_signed
            || matches!(
                f.ret_type,
                Some(ast::ParamType::Integer | ast::ParamType::Realtime)
            );
        // Return variable (static storage, named after the function).
        let ret_flat = format!("{}{}", fscope.innermost(), fname);
        self.cells.insert(
            ret_flat.clone(),
            Cell {
                bits: Bits::zeros(ret_w)?,
                signed: ret_s,
                msb: ret_w as i64 - 1,
                lsb: 0,
                mem: None,
                kind: CellKind::FuncStatic,
            },
        );
        let mut inputs = Vec::new();
        // ANSI ports + old-style `input` declarations.
        let input_decls = f.ports.iter().chain(
            f.decls
                .iter()
                .filter(|d| d.dir == Some(Direction::Input)),
        );
        for p in input_decls {
            let ty_w: Option<(u32, bool)> = match p.ty {
                Some(ast::ParamType::Integer) => Some((32, true)),
                Some(ast::ParamType::Time | ast::ParamType::Realtime) => Some((64, false)),
                _ => None,
            };
            let (w, msb, lsb) = match (&p.range, ty_w) {
                (Some(r), _) => {
                    let mpe = self.prefix_expr(&r.msb, scope)?;
                    let lpe = self.prefix_expr(&r.lsb, scope)?;
                    let msv = eval(&mpe, self)?.to_bits(r.span)?.0.to_i64_lossy();
                    let lsv = eval(&lpe, self)?.to_bits(r.span)?.0.to_i64_lossy();
                    let w = (msv - lsv).unsigned_abs() as u32 + 1;
                    (w, msv, lsv)
                }
                (None, Some((tw, _))) => (tw, tw as i64 - 1, 0),
                (None, None) => (1, 0, 0),
            };
            let psigned = p.signed || matches!(p.ty, Some(ast::ParamType::Integer | ast::ParamType::Realtime));
            for e in &p.entries {
                let flat = format!("{}{}", fscope.innermost(), e.name);
                self.cells.insert(
                    flat.clone(),
                    Cell {
                        bits: Bits::zeros(w)?,
                        signed: psigned,
                        msb,
                        lsb,
                        mem: None,
                        kind: CellKind::FuncStatic,
                    },
                );
                inputs.push((flat, w, psigned));
            }
        }
        // Function-body variable declarations (static locals).
        for d in &f.decls {
            if d.dir.is_none() {
                self.elab_var_decl(d, &fscope)?;
            }
        }
        // Lower the body synchronously (timing → hard error at exec time).
        let mut instrs = Vec::new();
        for st in &f.body {
            self.lower_stmt(&mut instrs, st, &fscope, 0)?;
        }
        let flat = self
            .resolve_name(&fname, scope)
            .unwrap_or_else(|| format!("{}{}", scope.innermost(), fname));
        self.funcs.insert(
            flat,
            FuncDef {
                instrs: std::rc::Rc::new(instrs),
                inputs,
                ret: ret_flat,
                ret_w,
                ret_s,
            },
        );
        Ok(())
    }

    fn register_task(&mut self, t: &ast::Function, scope: &Scope) -> SResult<()> {
        let tscope = scope.push(format!("{}.", t.name));
        let mut ports = Vec::new();
        let port_decls = t.ports.iter().chain(t.decls.iter().filter(|d| d.dir.is_some()));
        for p in port_decls {
            let ty_w: Option<(u32, bool)> = match p.ty {
                Some(ast::ParamType::Integer) => Some((32, true)),
                Some(ast::ParamType::Time | ast::ParamType::Realtime) => Some((64, false)),
                _ => None,
            };
            let (w, msb, lsb) = match (&p.range, ty_w) {
                (Some(r), _) => {
                    let mpe = self.prefix_expr(&r.msb, scope)?;
                    let lpe = self.prefix_expr(&r.lsb, scope)?;
                    let msv = eval(&mpe, self)?.to_bits(r.span)?.0.to_i64_lossy();
                    let lsv = eval(&lpe, self)?.to_bits(r.span)?.0.to_i64_lossy();
                    let w = (msv - lsv).unsigned_abs() as u32 + 1;
                    (w, msv, lsv)
                }
                (None, Some((tw, _))) => (tw, tw as i64 - 1, 0),
                (None, None) => (1, 0, 0),
            };
            let psigned = p.signed || matches!(p.ty, Some(ast::ParamType::Integer | ast::ParamType::Realtime));
            for e in &p.entries {
                let flat = format!("{}{}", tscope.innermost(), e.name);
                self.cells.insert(
                    flat.clone(),
                    Cell {
                        bits: Bits::zeros(w)?,
                        signed: psigned,
                        msb,
                        lsb,
                        mem: None,
                        kind: CellKind::TaskStatic,
                    },
                );
                ports.push((flat, p.dir.unwrap_or(Direction::Input), w, psigned));
            }
        }
        for d in &t.decls {
            if d.dir.is_none() {
                self.elab_var_decl(d, &tscope)?;
            }
        }
        let flat = self
            .resolve_name(&t.name, scope)
            .unwrap_or_else(|| format!("{}{}", scope.innermost(), t.name));
        self.tasks.insert(
            flat,
            TaskDef {
                body: t.body.clone(),
                scope: tscope,
                ports,
            },
        );
        Ok(())
    }

    fn elab_item(&mut self, it: &ast::ModuleItem, scope: &Scope) -> SResult<()> {
        match it {
            ast::ModuleItem::ContAssign(pairs) => {
                for p in pairs {
                    let lhs = self.prefix_expr(&p.lhs, scope)?;
                    let rhs = self.prefix_expr(&p.rhs, scope)?;
                    let lhs_w = self.lvalue_width(&lhs)?;
                    self.assigns.push(Assign { lhs, rhs, lhs_w });
                }
            }
            ast::ModuleItem::Initial { body, .. } => {
                let mut instrs = Vec::new();
                self.lower_stmt(&mut instrs, body, scope, 0)?;
                self.procs.push(Process {
                    instrs: std::rc::Rc::new(instrs),
                    pc: 0,
                    counters: Vec::new(),
                    state: PState::Runnable,
                    waits: Vec::new(),
                    label: "initial".to_string(),
                });
            }
            ast::ModuleItem::Always { span, ctrl, body } => {
                let mut instrs = Vec::new();
                let mut level_edges: Option<Vec<EventEdge>> = None;
                match ctrl {
                    Some(ec) if ec.edges.is_empty() => {
                        // always @* — sensitivity = identifiers read in body.
                        let mut names = Vec::new();
                        self.collect_reads(body, &mut names);
                        names.sort();
                        names.dedup();
                        if names.is_empty() {
                            return Err(SimError::at(
                                "always @* has no readable signals in its body",
                                *span,
                            ));
                        }
                        let edges: Vec<EventEdge> = names
                            .into_iter()
                            .map(|n| EventEdge {
                                kind: ast::EdgeKind::Any,
                                expr: Expr::Ident {
                                    span: *span,
                                    name: n,
                                },
                            })
                            .collect();
                        level_edges = Some(edges);
                    }
                    Some(ec) => {
                        // Pure level-sensitive lists (all `Any`) evaluate once
                        // at t0 before waiting (upstream parity, E2E finding).
                        let level = ec.edges.iter().all(|e| e.kind == ast::EdgeKind::Any);
                        if level {
                            level_edges = Some(self.prefix_edges(&ec.edges, scope)?);
                        } else {
                            instrs.push(Instr::Wait(self.prefix_edges(&ec.edges, scope)?));
                        }
                    }
                    None => {}
                }
                self.lower_stmt(&mut instrs, body, scope, 0)?;
                if let Some(edges) = level_edges {
                    instrs.push(Instr::Wait(edges));
                    instrs.push(Instr::Jump(0));
                } else {
                    instrs.push(Instr::Jump(0));
                }
                self.procs.push(Process {
                    instrs: std::rc::Rc::new(instrs),
                    pc: 0,
                    counters: Vec::new(),
                    state: PState::Runnable,
                    waits: Vec::new(),
                    label: "always".to_string(),
                });
            }
            ast::ModuleItem::Instance(inst) => self.elab_instance(inst, scope)?,
            ast::ModuleItem::GenerateRegion(items) => {
                for it2 in items {
                    self.elab_item(it2, scope)?;
                }
            }
            ast::ModuleItem::GenIf {
                cond,
                then,
                els,
                ..
            } => {
                let pe = self.prefix_expr(cond, scope)?;
                let (b, _) = eval(&pe, self)?.to_bits(cond.span())?;
                // IEEE 1364-2005 §12.4.3: a single NAMED begin-block as the
                // whole branch is the scope itself — no implicit genblk.

                let take_else = b.truthy()? != Some(true);
                let raw: &[ast::ModuleItem] = if !take_else {
                    then
                } else {
                    els.as_deref().unwrap_or(&[])
                };
                // A single named block contributes its INNER items under the
                // block's own scope; anything else runs under an implicit
                // genblk scope.
                let (items, named): (Vec<ast::ModuleItem>, Option<String>) = match raw {
                    [ast::ModuleItem::GenBlock { name: Some(n), items: inner, .. }] => {
                        (inner.clone(), Some(n.clone()))
                    }
                    other => (other.to_vec(), None),
                };
                if !items.is_empty() || !raw.is_empty() {
                    let sub = match named {
                        Some(n) => scope.push(format!("{}{n}.", scope.innermost())),
                        None => {
                            let n = self.next_genblk(scope);
                            scope.push(format!("{}genblk{n}.", scope.innermost()))
                        }
                    };
                    for it2 in &items {
                        self.elab_item(it2, &sub)?;
                    }
                }

            }
            ast::ModuleItem::GenFor {
                var,
                init,
                cond,
                step,
                block,
                items,
                ..
            } => {
                let start = self.eval_gen_const(init, scope)?;
                let mut i = start;
                loop {
                    // Bind the genvar as a scoped constant per iteration.
                    let blk = block.clone().unwrap_or_default();
                    let iterscope =
                        scope.push(format!("{}{blk}[{i}].", scope.innermost()));
                    let gname = format!("{}{}", iterscope.innermost(), var);
                    self.cells.insert(
                        gname,
                        Cell {
                            bits: Bits::from_u64(32, i as u64)?,
                            signed: true,
                            msb: 31,
                            lsb: 0,
                            mem: None,
                            kind: CellKind::Genvar,
                        },
                    );
                    let pe = self.prefix_expr(cond, &iterscope)?;
                    let (b, _) = eval(&pe, self)?.to_bits(cond.span())?;
                    if b.truthy()? != Some(true) {
                        break;
                    }
                    self.pending_gen_block = block.clone();
                    for it2 in items {
                        self.elab_item(it2, &iterscope)?;
                    }
                    self.pending_gen_block = None;
                    let pe_rhs = self.prefix_expr(&step.rhs, &iterscope)?;
                    let (nb, _) = eval(&pe_rhs, self)?.to_bits(step.span)?;
                    let next = nb.to_i64_lossy();
                    if next == i {
                        return Err(SimError::at(
                            "generate-for step does not advance the genvar",
                            step.span,
                        ));
                    }
                    i = next;
                }
            }
            ast::ModuleItem::GenCase {
                expr,
                items,
                default,
                ..
            } => {
                let pe = self.prefix_expr(expr, scope)?;
                let (b, _) = eval(&pe, self)?.to_bits(expr.span())?;
                let mut chosen: Option<&Vec<ast::ModuleItem>> = None;
                'outer: for (labels, its) in items {
                    for l in labels {
                        let ple = self.prefix_expr(l, scope)?;
                        let (lb, _) = eval(&ple, self)?.to_bits(l.span())?;
                        if b.case_eq(&lb)?.to_u64_lossy() == 1 {
                            chosen = Some(its);
                            break 'outer;
                        }
                    }
                }
                let picked = chosen.or(default.as_ref());
                if let Some(its) = picked {
                    // IEEE 1364-2005 §12.4.3: when the branch is exactly one
                    // NAMED begin-block, that block IS the scope — no
                    // implicit genblk (references say `blk.x`, not
                    // `genblkN.blk.x`).
                    if let [ast::ModuleItem::GenBlock {
                        name: Some(n), items: inner, ..
                    }] = its.as_slice()
                    {
                        let sub = scope.push(format!("{}{n}.", scope.innermost()));
                        for it2 in inner {
                            self.elab_item(it2, &sub)?;
                        }
                    } else {
                        let n = self.next_genblk(scope);
                        let sub = scope.push(format!("{}genblk{n}.", scope.innermost()));
                        for it2 in its {
                            self.elab_item(it2, &sub)?;
                        }
                    }
                }
            }
            ast::ModuleItem::GenBlock { name, items, .. } => {
                let sub = if name.is_some() && self.pending_gen_block.take() == *name {
                    // `for ... begin : NAME`: the iteration scope already
                    // carries `NAME[i].` — reuse it verbatim.
                    scope.clone()
                } else {
                    match name {
                        Some(n) => scope.push(format!("{}{n}.", scope.innermost())),
                        None => {
                            let n = self.next_genblk(scope);
                            scope.push(format!("{}genblk{n}.", scope.innermost()))
                        }
                    }
                };
                for it2 in items {
                    self.elab_item(it2, &sub)?;
                }
            }
            ast::ModuleItem::Decl(d) => match d.kind {
                DeclKind::Parameter | DeclKind::Localparam => self.elab_param_decl(d, scope)?,
                _ => self.elab_var_decl(d, scope)?,
            },
            ast::ModuleItem::Function(_) | ast::ModuleItem::Task(_) => {}
            ast::ModuleItem::Defparam(_) => {
                return Err(SimError::at(
                    "defparam is not yet supported (use #(.PARAM()) instance overrides)",
                    it_span(it),
                ))
            }
            ast::ModuleItem::Empty => {}
        }
        Ok(())
    }

    fn next_genblk(&mut self, scope: &Scope) -> u32 {
        let key = scope.innermost().to_string();
        let c = self.genblk_counter.entry(key).or_insert(0);
        *c += 1;
        *c
    }

    fn eval_gen_const(&mut self, e: &Expr, scope: &Scope) -> SResult<i64> {
        let pe = self.prefix_expr(e, scope)?;
        let (b, _) = eval(&pe, self)?.to_bits(e.span())?;
        if b.has_unknown() {
            return Err(SimError::at("generate constant contains x/z", e.span()));
        }
        Ok(b.to_i64_lossy())
    }

    fn elab_instance(&mut self, inst: &ast::Instance, scope: &Scope) -> SResult<()> {
        if let Some(k) = gate_kind(&inst.mod_name) {
            if !inst.params.is_empty() {
                return Err(SimError::at(
                    "gate primitives cannot have parameter overrides",
                    inst.span,
                ));
            }
            for unit in &inst.insts {
                if unit.array.is_some() {
                    return Err(SimError::at("gate instance arrays land at M8", unit.span));
                }
                let mut conns = Vec::with_capacity(unit.conns.len());
                for c in &unit.conns {
                    if c.name.is_some() {
                        return Err(SimError::at("gate connections must be positional", c.span));
                    }
                    match &c.expr {
                        Some(e) => {
                            let pe = self.prefix_expr(e, scope)?;
                            conns.push(pe);
                        }
                        None => return Err(SimError::at("unconnected gate terminal", c.span)),
                    }
                }
                self.gates.push(GateInst::split(k, &conns, inst.span)?);
            }
            return Ok(());
        }
        let child = self
            .modules
            .get(&inst.mod_name)
            .ok_or_else(|| {
                SimError::at(
                    format!(
                        "unknown module `{}` (gate primitives land at M7)",
                        inst.mod_name
                    ),
                    inst.span,
                )
            })?
            .clone();
        for unit in &inst.insts {
            let child_prefix = format!("{}{}.", scope.innermost(), unit.name);
            let child_scope = Scope::new(child_prefix.clone());
            let mut overrides: HashMap<String, Bits> = HashMap::new();
            let mut ovs_signed: HashMap<String, bool> = HashMap::new();
            if !inst.params.is_empty() {
                let named: Vec<&ast::ParamConn> =
                    inst.params.iter().filter(|p| p.name.is_some()).collect();
                if !named.is_empty() {
                    for p in named {
                        if let Some(e) = &p.expr {
                            let pe = self.prefix_expr(e, scope)?;
                            let (bits, signed) = eval(&pe, self)?.to_bits(p.span)?;
                            overrides.insert(p.name.clone().unwrap(), bits);
                            ovs_signed.insert(p.name.clone().unwrap(), signed);
                        }
                    }
                } else {
                    for (i, p) in inst.params.iter().enumerate() {
                        // Upstream tolerates positional overrides beyond the
                        // formal list (and empty ones); match that leniency.
                        let Some(formal) = child.params.get(i) else {
                            continue;
                        };
                        if let Some(e) = &p.expr {
                            let pe = self.prefix_expr(e, scope)?;
                            let (bits, signed) = eval(&pe, self)?.to_bits(p.span)?;
                            overrides.insert(formal.name.clone(), bits);
                            ovs_signed.insert(formal.name.clone(), signed);
                        }
                    }
                }
            }
            self.elab_module(&child, &child_scope, &overrides, &ovs_signed)?;
            for (i, conn) in unit.conns.iter().enumerate() {
                let formal_name = match &conn.name {
                    Some(n) => n.clone(),
                    None => {
                        let Some(p) = child.ports.get(i) else {
                            return Err(SimError::at(
                                "positional port connection out of range",
                                conn.span,
                            ));
                        };
                        p.name.clone()
                    }
                };
                let formal = format!("{child_prefix}{formal_name}");
                let dir = self.port_direction(&child, &formal_name);
                match &conn.expr {
                    None => {}
                    Some(Expr::Ident { name, .. }) => {
                        let actual = self.resolve_name(name, scope).ok_or_else(|| {
                            SimError::at(
                                format!("undeclared identifier `{name}` in port connection"),
                                conn.span,
                            )
                        })?;
                        self.union(&formal, &actual);
                    }
                    Some(e) => match dir {
                        Some(Direction::Output) => {
                            let pe = self.prefix_expr(e, scope)?;
                            let lhs_w = self.lvalue_width(&pe)?;
                            self.assigns.push(Assign {
                                lhs: pe,
                                rhs: Expr::Ident {
                                    span: conn.span,
                                    name: formal,
                                },
                                lhs_w,
                            });
                        }
                        Some(Direction::Inout) => {
                            return Err(SimError::at(
                                "inout ports connected to expressions are not yet supported (M7)",
                                conn.span,
                            ))
                        }
                        _ => {
                            let pe = self.prefix_expr(e, scope)?;
                            let lhs_w = self.cells.get(&formal).map(|c| c.bits.width).unwrap_or(1);
                            self.assigns.push(Assign {
                                lhs: Expr::Ident {
                                    span: conn.span,
                                    name: formal,
                                },
                                rhs: pe,
                                lhs_w,
                            });
                        }
                    },
                }
            }
        }
        Ok(())
    }

    fn port_direction(&self, m: &Module, port: &str) -> Option<Direction> {
        for p in &m.ports {
            if p.name == port {
                return p.dir;
            }
        }
        for it in &m.items {
            if let ast::ModuleItem::Decl(d) = it {
                if let Some(dir) = d.dir {
                    for e in &d.entries {
                        if e.name == port {
                            return Some(dir);
                        }
                    }
                }
            }
        }
        None
    }

    // ---------- naming ----------

    /// `blk[I-1].z`-style hierarchical reference: flatten the base path
    /// (indexes evaluate at elaboration), try the scope chain innermost-out,
    /// and return the canonical cell name.
    fn resolve_hier_dot(
        &mut self,
        base: &Expr,
        name: &str,
        scope: &Scope,
    ) -> SResult<String> {
        let path = self.hier_path(base, scope)?;
        for prefix in scope.chain.iter().rev() {
            let cand = format!("{prefix}{path}.{name}");
            let root = self.find(&cand);
            if self.cells.contains_key(&root) {
                return Ok(root);
            }
            if self.cells.contains_key(&cand) {
                return Ok(cand);
            }
        }
        Err(SimError::at(
            format!("undeclared hierarchical identifier `{path}.{name}`"),
            base.span().merge(base.span()),
        ))
    }

    /// Textual path of a hierarchical base: idents joined by `.`, indexes
    /// evaluated and rendered as `[i]`.
    fn hier_path(&mut self, e: &Expr, scope: &Scope) -> SResult<String> {
        match e {
            Expr::Ident { name, .. } => Ok(name.clone()),
            Expr::HierIdent { parts, .. } => Ok(parts.join(".")),
            Expr::HierDot { base, name, .. } => {
                let b = self.hier_path(base, scope)?;
                Ok(format!("{b}.{name}"))
            }
            Expr::Index { base, index, .. } => {
                let b = self.hier_path(base, scope)?;
                let pe = self.prefix_expr(index, scope)?;
                let v = eval(&pe, self)?;
                let (bits, _) = v.to_bits(index.span())?;
                if bits.has_unknown() {
                    return Err(SimError::at(
                        "hierarchical reference index contains x/z",
                        index.span(),
                    ));
                }
                Ok(format!("{b}[{}]", bits.to_u64_lossy()))
            }
            other => Err(SimError::at(
                "unsupported hierarchical reference base",
                other.span(),
            )),
        }
    }

    fn resolve_name(&self, name: &str, scope: &Scope) -> Option<String> {
        if std::env::var_os("RCS_DEBUG_RESOLVE").is_some() && name.contains(".") {
            let keys: Vec<&String> = self.cells.keys().take(30).collect();
            eprintln!("DBG resolve {name} chain={:?} keys={keys:?}", scope.chain);
        }
        for p in scope.chain.iter().rev() {
            let cand = format!("{p}{name}");
            if self.cells.contains_key(&cand)
                || self.funcs.contains_key(&cand)
                || self.tasks.contains_key(&cand)
            {
                return Some(cand);
            }
        }
        None
    }

    fn prefix_expr(&mut self, e: &Expr, scope: &Scope) -> SResult<Expr> {
        Ok(match e {
            Expr::Ident { span, name } => Expr::Ident {
                span: *span,
                name: self.resolve_name(name, scope).ok_or_else(|| {
                    SimError::at(format!("undeclared identifier `{name}`"), *span)
                })?,
            },
            Expr::HierIdent { span, parts } => {
                let joined = parts.join(".");
                let resolved = self.resolve_name(&joined, scope).or_else(|| {
                    // `<ModuleName>.sig` self-reference (roots are addressable
                    // by their module name): strip the module segment.
                    if parts.len() >= 2 && self.modules.contains_key(&parts[0]) {
                        let rest = parts[1..].join(".");
                        self.resolve_name(&rest, scope)
                    } else {
                        None
                    }
                });
                Expr::Ident {
                    span: *span,
                    name: resolved.ok_or_else(|| {
                        SimError::at(
                            format!("undeclared hierarchical identifier `{joined}`"),
                            *span,
                        )
                    })?,
                }
            }
            Expr::NumLit(_) | Expr::RealLit { .. } | Expr::StrLit { .. } => e.clone(),
            Expr::Unary { span, op, expr } => Expr::Unary {
                span: *span,
                op: *op,
                expr: Box::new(self.prefix_expr(expr, scope)?),
            },
            Expr::Binary { span, op, lhs, rhs } => Expr::Binary {
                span: *span,
                op: *op,
                lhs: Box::new(self.prefix_expr(lhs, scope)?),
                rhs: Box::new(self.prefix_expr(rhs, scope)?),
            },
            Expr::Ternary {
                span,
                cond,
                then,
                els,
            } => Expr::Ternary {
                span: *span,
                cond: Box::new(self.prefix_expr(cond, scope)?),
                then: Box::new(self.prefix_expr(then, scope)?),
                els: Box::new(self.prefix_expr(els, scope)?),
            },
            Expr::Concat { span, parts } => Expr::Concat {
                span: *span,
                parts: parts
                    .iter()
                    .map(|p| self.prefix_expr(p, scope))
                    .collect::<SResult<_>>()?,
            },
            Expr::Repeat { span, count, value } => Expr::Repeat {
                span: *span,
                count: Box::new(self.prefix_expr(count, scope)?),
                value: Box::new(self.prefix_expr(value, scope)?),
            },
            Expr::Index { span, base, index } => Expr::Index {
                span: *span,
                base: Box::new(self.prefix_expr(base, scope)?),
                index: Box::new(self.prefix_expr(index, scope)?),
            },
            Expr::HierDot { span, base, name } => {
                let canon = self.resolve_hier_dot(base, name, scope)?;
                Expr::Ident {
                    span: *span,
                    name: canon,
                }
            }
            Expr::PartSelect {
                span,
                base,
                msb,
                lsb,
            } => Expr::PartSelect {
                span: *span,
                base: Box::new(self.prefix_expr(base, scope)?),
                msb: Box::new(self.prefix_expr(msb, scope)?),
                lsb: Box::new(self.prefix_expr(lsb, scope)?),
            },
            Expr::IndexedPart {
                span,
                base,
                offset,
                width,
                plus,
            } => Expr::IndexedPart {
                span: *span,
                base: Box::new(self.prefix_expr(base, scope)?),
                offset: Box::new(self.prefix_expr(offset, scope)?),
                width: Box::new(self.prefix_expr(width, scope)?),
                plus: *plus,
            },
            Expr::Call { span, func, args } => {
                let func2 = if func.starts_with('$') {
                    func.clone()
                } else {
                    self.resolve_name(func, scope)
                        .ok_or_else(|| SimError::at(format!("unknown function `{func}`"), *span))?
                };
                Expr::Call {
                    span: *span,
                    func: func2,
                    args: args
                        .iter()
                        .map(|a| self.prefix_expr(a, scope))
                        .collect::<SResult<_>>()?,
                }
            }
        })
    }

    fn prefix_edges(&mut self, edges: &[EventEdge], scope: &Scope) -> SResult<Vec<EventEdge>> {
        edges
            .iter()
            .map(|ed| {
                Ok(EventEdge {
                    kind: ed.kind,
                    expr: self.prefix_expr(&ed.expr, scope)?,
                })
            })
            .collect()
    }

    fn collect_reads(&self, s: &Stmt, out: &mut Vec<String>) {
        match s {
            Stmt::Block { body, .. } => {
                for b in body {
                    self.collect_reads(b, out);
                }
            }
            Stmt::BAssign { lhs, rhs, .. } | Stmt::NbAssign { lhs, rhs, .. } => {
                self.collect_expr_reads(rhs, out);
                self.collect_expr_reads(lhs, out);
            }
            Stmt::If {
                cond,
                then,
                els,
                ..
            } => {
                self.collect_expr_reads(cond, out);
                self.collect_reads(then, out);
                if let Some(e) = els {
                    self.collect_reads(e, out);
                }
            }
            Stmt::Case { expr, items, .. } => {
                self.collect_expr_reads(expr, out);
                for it in items {
                    match it {
                        CaseItem::Items(labels, st) => {
                            for l in labels {
                                self.collect_expr_reads(l, out);
                            }
                            self.collect_reads(st, out);
                        }
                        CaseItem::Default(st) => self.collect_reads(st, out),
                    }
                }
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
                ..
            } => {
                for p in init.iter().chain(step.iter()) {
                    self.collect_expr_reads(&p.rhs, out);
                    self.collect_expr_reads(&p.lhs, out);
                }
                self.collect_expr_reads(cond, out);
                self.collect_reads(body, out);
            }
            Stmt::While { cond, body, .. } => {
                self.collect_expr_reads(cond, out);
                self.collect_reads(body, out);
            }
            Stmt::Repeat { count, body, .. } => {
                self.collect_expr_reads(count, out);
                self.collect_reads(body, out);
            }
            Stmt::Forever { body, .. } => self.collect_reads(body, out),
            Stmt::DelayCtrl { body, .. } => {
                if let Some(b) = body {
                    self.collect_reads(b, out);
                }
            }
            Stmt::EventCtrl { ctrl, body, .. } => {
                for e in &ctrl.edges {
                    self.collect_expr_reads(&e.expr, out);
                }
                if let Some(b) = body {
                    self.collect_reads(b, out);
                }
            }
            Stmt::SysCall { args, .. } | Stmt::SubCall { args, .. } => {
                for a in args {
                    self.collect_expr_reads(a, out);
                }
            }
            Stmt::Disable { .. } | Stmt::Null { .. } => {}
        }
    }

    fn collect_expr_reads(&self, e: &Expr, out: &mut Vec<String>) {
        match e {
            Expr::Ident { name, .. } => out.push(name.clone()),
            Expr::HierIdent { parts, .. } => out.push(parts.join(".")),
            Expr::Unary { expr, .. } => self.collect_expr_reads(expr, out),
            Expr::Binary { lhs, rhs, .. } => {
                self.collect_expr_reads(lhs, out);
                self.collect_expr_reads(rhs, out);
            }
            Expr::Ternary {
                cond,
                then,
                els,
                ..
            } => {
                self.collect_expr_reads(cond, out);
                self.collect_expr_reads(then, out);
                self.collect_expr_reads(els, out);
            }
            Expr::Concat { parts, .. } | Expr::Call { args: parts, .. } => {
                for p in parts {
                    self.collect_expr_reads(p, out);
                }
            }
            Expr::Repeat { count, value, .. } => {
                self.collect_expr_reads(count, out);
                self.collect_expr_reads(value, out);
            }
            Expr::Index { base, index, .. } => {
                self.collect_expr_reads(base, out);
                self.collect_expr_reads(index, out);
            }
            Expr::PartSelect {
                base,
                msb,
                lsb,
                ..
            } => {
                self.collect_expr_reads(base, out);
                self.collect_expr_reads(msb, out);
                self.collect_expr_reads(lsb, out);
            }
            Expr::IndexedPart {
                base,
                offset,
                ..
            } => {
                self.collect_expr_reads(base, out);
                self.collect_expr_reads(offset, out);
            }
            _ => {}
        }
    }

    // ---------- lowering ----------

    fn lower_stmt(
        &mut self,
        out: &mut Vec<Instr>,
        s: &Stmt,
        scope: &Scope,
        depth: usize,
    ) -> SResult<()> {
        if depth > 64 {
            return Err(SimError::at("statement nesting too deep", stmt_span(s)));
        }
        match s {
            Stmt::Block {
                name,
                decls,
                body,
                ..
            } => {
                let bscope = match name {
                    Some(n) => scope.push(format!("{n}.")),
                    None => scope.clone(),
                };
                for d in decls {
                    self.elab_var_decl(d, &bscope)?;
                }
                for b in body {
                    self.lower_stmt(out, b, &bscope, depth + 1)?;
                }
            }
            Stmt::BAssign { lhs, rhs, .. } => {
                let pl = self.prefix_expr(lhs, scope)?;
                let pr = self.prefix_expr(rhs, scope)?;
                out.push(Instr::BAssign(pl, pr));
            }
            Stmt::NbAssign { lhs, rhs, .. } => {
                let pl = self.prefix_expr(lhs, scope)?;
                let pr = self.prefix_expr(rhs, scope)?;
                out.push(Instr::NbAssign(pl, pr));
            }
            Stmt::If {
                cond,
                then,
                els,
                ..
            } => {
                let pc_ = self.prefix_expr(cond, scope)?;
                let j = out.len();
                out.push(Instr::JumpIfFalse(pc_.clone(), 0));
                self.lower_stmt(out, then, scope, depth + 1)?;
                if let Some(e) = els {
                    let j2 = out.len();
                    out.push(Instr::Jump(0));
                    out[j] = Instr::JumpIfFalse(pc_, out.len());
                    self.lower_stmt(out, e, scope, depth + 1)?;
                    out[j2] = Instr::Jump(out.len());
                } else {
                    out[j] = Instr::JumpIfFalse(pc_, out.len());
                }
            }
            Stmt::Case {
                kind,
                expr,
                items,
                ..
            } => {
                let pe = self.prefix_expr(expr, scope)?;
                let cidx = out.len();
                out.push(Instr::Case {
                    kind: *kind,
                    expr: pe.clone(),
                    arms: Vec::new(),
                    default: 0,
                });
                let mut arm_targets = Vec::new();
                let mut end_jumps = Vec::new();
                let mut default_target = usize::MAX;
                for it in items {
                    match it {
                        CaseItem::Items(labels, body) => {
                            let mut pls = Vec::new();
                            for l in labels {
                                pls.push(self.prefix_expr(l, scope)?);
                            }
                            arm_targets.push((pls, out.len()));
                            self.lower_stmt(out, body, scope, depth + 1)?;
                            end_jumps.push(out.len());
                            out.push(Instr::Jump(0));
                        }
                        CaseItem::Default(body) => {
                            default_target = out.len();
                            self.lower_stmt(out, body, scope, depth + 1)?;
                            end_jumps.push(out.len());
                            out.push(Instr::Jump(0));
                        }
                    }
                }
                let end = out.len();
                for j in end_jumps {
                    out[j] = Instr::Jump(end);
                }
                if default_target == usize::MAX {
                    default_target = end;
                }
                out[cidx] = Instr::Case {
                    kind: *kind,
                    expr: pe,
                    arms: arm_targets,
                    default: default_target,
                };
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
                ..
            } => {
                for p in init {
                    let pl = self.prefix_expr(&p.lhs, scope)?;
                    let pr = self.prefix_expr(&p.rhs, scope)?;
                    out.push(Instr::BAssign(pl, pr));
                }
                let top = out.len();
                let pc_ = self.prefix_expr(cond, scope)?;
                let j = out.len();
                out.push(Instr::JumpIfFalse(pc_.clone(), 0));
                self.lower_stmt(out, body, scope, depth + 1)?;
                for p in step {
                    let pl = self.prefix_expr(&p.lhs, scope)?;
                    let pr = self.prefix_expr(&p.rhs, scope)?;
                    out.push(Instr::BAssign(pl, pr));
                }
                out.push(Instr::Jump(top));
                out[j] = Instr::JumpIfFalse(pc_, out.len());
            }
            Stmt::While { cond, body, .. } => {
                let top = out.len();
                let pc_ = self.prefix_expr(cond, scope)?;
                let j = out.len();
                out.push(Instr::JumpIfFalse(pc_.clone(), 0));
                self.lower_stmt(out, body, scope, depth + 1)?;
                out.push(Instr::Jump(top));
                out[j] = Instr::JumpIfFalse(pc_, out.len());
            }
            Stmt::Repeat { count, body, .. } => {
                let pc_ = self.prefix_expr(count, scope)?;
                out.push(Instr::RepeatInit(pc_));
                let top = out.len();
                let r = out.len();
                out.push(Instr::RepeatLoop { end: 0 });
                self.lower_stmt(out, body, scope, depth + 1)?;
                out.push(Instr::Jump(top));
                out[r] = Instr::RepeatLoop { end: out.len() };
            }
            Stmt::Forever { body, .. } => {
                let top = out.len();
                self.lower_stmt(out, body, scope, depth + 1)?;
                out.push(Instr::Jump(top));
            }
            Stmt::DelayCtrl { delay, body, .. } => {
                let d = match delay {
                    ast::DelayVal::Lit(t) => ast::DelayVal::Lit(t.clone()),
                    ast::DelayVal::Expr(e) => ast::DelayVal::Expr(self.prefix_expr(e, scope)?),
                };
                out.push(Instr::Delay(d));
                if let Some(b) = body {
                    self.lower_stmt(out, b, scope, depth + 1)?;
                }
            }
            Stmt::EventCtrl { ctrl, body, .. } => {
                out.push(Instr::Wait(self.prefix_edges(&ctrl.edges, scope)?));
                if let Some(b) = body {
                    self.lower_stmt(out, b, scope, depth + 1)?;
                }
            }
            Stmt::SysCall { name, args, .. } => {
                let pargs: Vec<Expr> = if matches!(
                    name.as_str(),
                    "$dumpvars" | "$dumpoff" | "$dumpon" | "$dumpall"
                ) {
                    // Arguments are dump levels / scope names, not expressions.
                    Vec::new()
                } else {
                    args.iter()
                        .map(|a| self.prefix_expr(a, scope))
                        .collect::<SResult<_>>()?
                };
                out.push(Instr::SysCall(name.clone(), pargs));
            }
            Stmt::SubCall { span, name, args } => {
                let flat = self
                    .resolve_name(name, scope)
                    .ok_or_else(|| SimError::at(format!("unknown task `{name}`"), *span))?;
                let def = self.tasks.get(&flat).cloned().ok_or_else(|| {
                    SimError::at(
                        format!("`{name}` is not a task (function calls are expressions)"),
                        *span,
                    )
                })?;
                if depth > 16 {
                    return Err(SimError::at(
                        "task call nesting too deep (recursion?)",
                        *span,
                    ));
                }
                let mut ai = 0usize;
                for (pname, dir, _w, _s) in &def.ports {
                    let Some(arg) = args.get(ai) else {
                        return Err(SimError::at(
                            format!("task `{name}` expects {} arguments", def.ports.len()),
                            *span,
                        ));
                    };
                    ai += 1;
                    if matches!(dir, Direction::Input | Direction::Inout) {
                        let pport = Expr::Ident {
                            span: *span,
                            name: pname.clone(),
                        };
                        out.push(Instr::BAssign(pport, self.prefix_expr(arg, scope)?));
                    }
                }
                for st in &def.body {
                    self.lower_stmt(out, st, &def.scope, depth + 1)?;
                }
                let mut ai = 0usize;
                for (pname, dir, _w, _s) in &def.ports {
                    let arg = args.get(ai).cloned();
                    ai += 1;
                    if matches!(dir, Direction::Output | Direction::Inout) {
                        if let Some(arg) = arg {
                            let pport = Expr::Ident {
                                span: *span,
                                name: pname.clone(),
                            };
                            out.push(Instr::BAssign(self.prefix_expr(&arg, scope)?, pport));
                        }
                    }
                }
            }
            Stmt::Disable { span, name } => {
                return Err(SimError::at(
                    format!("`disable {name}` lands at milestone M7"),
                    *span,
                ))
            }
            Stmt::Null { .. } => {}
        }
        Ok(())
    }

    // ---------- union-find / cells ----------

    fn find(&mut self, name: &str) -> String {
        match self.parent.get(name).cloned() {
            Some(p) => {
                let root = self.find(&p);
                self.parent.insert(name.to_string(), root.clone());
                root
            }
            None => name.to_string(),
        }
    }

    fn union(&mut self, a: &str, b: &str) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra == rb {
            return;
        }
        // Merge alias groups under the new root (VCD records every member).
        let ga = self
            .alias_groups
            .remove(&ra)
            .unwrap_or_else(|| vec![ra.clone()]);
        let gb = self
            .alias_groups
            .entry(rb.clone())
            .or_insert_with(|| vec![rb.clone()]);
        gb.extend(ga);
        self.parent.insert(ra, rb);
    }

    fn read_name(&mut self, name: &str, span: Span) -> SResult<(Bits, bool)> {
        let root = self.find(name);
        match self.cells.get(&root) {
            Some(c) => Ok((c.bits, c.signed)),
            None => Err(SimError::at(
                format!("undeclared identifier `{name}`"),
                span,
            )),
        }
    }

    pub(crate) fn set_name(&mut self, name: &str, bits: Bits, signed: bool) -> SResult<()> {
        let root = self.find(name);
        let stored = match self.cells.get_mut(&root) {
            Some(c) => {
                c.bits = if c.signed && signed {
                    bits.sadjust(c.bits.width)?
                } else {
                    bits.uadjust(c.bits.width)?
                };
                c.bits
            }
            None => {
                return Err(SimError::new(
                    format!("undeclared identifier `{name}`"),
                    None,
                ))
            }
        };
        if let Some(w) = self.vcd.as_mut() {
            if w.started() {
                match self.alias_groups.get(&root) {
                    Some(members) => {
                        for m in members {
                            w.record_change(m, stored);
                        }
                    }
                    None => w.record_change(&root, stored),
                }
            }
        }
        if self.vpi.has_vc_callbacks() {
            if let Some(idx) = self.vpi.index.get(&root).copied() {
                self.vpi.pending_vc.insert(idx, stored);
            }
        }
        Ok(())
    }

    /// Build the VCD writer from every dumpable declared variable.
    fn vcd_begin(&mut self) {
        if self.vcd.is_none() {
            let vars: Vec<(String, Vec<String>, u32)> = self
                .dumpable
                .iter()
                .filter_map(|n| {
                    let w = if let Some((root, _)) = self.mem_elems.get(n) {
                        self.cells.get(root)?.bits.width
                    } else {
                        let root = self.parent.get(n).cloned().unwrap_or_else(|| n.clone());
                        self.cells.get(&root).or_else(|| self.cells.get(n))?.bits.width
                    };
                    Some((n.clone(), n.split('.').map(|s| s.to_string()).collect(), w))
                })
                .collect();
            let mut w = crate::vcd::VcdWriter::new(vars, self.prec_name);
            w.start();
            self.vcd = Some(w);
        }
        if let Some(w) = self.vcd.as_mut() {
            w.start();
        }
        let snapshot = self.vcd_snapshot();
        if let Some(w) = self.vcd.as_mut() {
            w.dump_t0(&snapshot);
        }
    }

    fn vcd_snapshot(&mut self) -> std::collections::BTreeMap<String, Bits> {
        let names: Vec<String> = self.dumpable.iter().cloned().collect();
        let mut m = std::collections::BTreeMap::new();
        for n in names {
            if let Some((root, wi)) = self.mem_elems.get(&n) {
                if let Some(mem) = self.cells.get(root).and_then(|c| c.mem.as_ref()) {
                    m.insert(n, mem.words[*wi].clone());
                }
                continue;
            }
            let root = self.find(&n);
            if let Some(c) = self.cells.get(&root) {
                m.insert(n, c.bits);
            }
        }
        m
    }

    fn vcd_flush(&mut self) {
        let now = self.now;
        if let Some(w) = self.vcd.as_mut() {
            w.flush_at(now);
        }
    }

    /// The VCD text (if `$dumpvars` ran) and the `$dumpfile` path.
    pub fn take_vcd(&mut self) -> Option<(Option<String>, String)> {
        let w = self.vcd.take()?;
        let text = w.finish();
        if text.is_empty() {
            return None;
        }
        Some((self.dump_path.take(), text))
    }

    // ---------- lvalues ----------

    fn assign_lvalue(&mut self, lhs: &Expr, v: Val) -> SResult<()> {
        let (bits, signed) = v.to_bits(lhs.span())?;
        match lhs {
            Expr::Ident { name, .. } => self.set_name(name, bits, signed),
            Expr::HierIdent { parts, .. } => self.set_name(&parts.join("."), bits, signed),
            Expr::Index { base, index, .. } => {
                // Memory word store (whole element).
                if let Some(mn) = eval::mem_base_name(base) {
                    if let Some((lsb, n, w, esigned)) = self.mem_info(&mn) {
                        let root = self.find(&mn);
                        let (i, _) = eval(index, self)?.to_bits(index.span())?;
                        if i.has_unknown() {
                            return Ok(()); // x-indexed write: no effect
                        }
                        let wi = i.to_u64_lossy() as i64 - lsb;
                        if wi < 0 || wi as usize >= n {
                            return Ok(()); // out-of-range word write: ignored
                        }
                        let word = if esigned && signed {
                            bits.sadjust(w)?
                        } else {
                            bits.uadjust(w)?
                        };
                        {
                            let c = self.cells.get_mut(&root).unwrap();
                            c.mem.as_mut().unwrap().words[wi as usize] = word.clone();
                        }
                        if self.vpi.has_vc_callbacks() {
                            let en = format!("{root}[{}]", lsb + wi);
                            if let Some(idx) = self.vpi.index.get(&en).copied() {
                                self.vpi.pending_vc.insert(idx, word);
                            }
                        }
                        if let Some(w) = self.vcd.as_mut() {
                            if w.started() {
                                w.record_change(&format!("{root}[{}]", lsb + wi), word);
                            }
                        }
                        return Ok(());
                    }
                }
                let (cur, cs) = self.read_lvalue_base(base)?;
                let (i, _) = eval(index, self)?.to_bits(index.span())?;
                if i.has_unknown() {
                    return Ok(());
                }
                let pos = eval::bit_position(base, i.to_i64_lossy(), self);
                let cur2 = match pos {
                    Some(p) if p >= 0 && (p as u32) < cur.width => {
                        cur.set_part(p as u32, p as u32, &bits)?
                    }
                    _ => cur,
                };
                self.write_base(base, cur2, cs)
            }
            Expr::PartSelect {
                base,
                msb,
                lsb,
                ..
            } => {
                let (cur, cs) = self.read_lvalue_base(base)?;
                let (m, _) = eval(msb, self)?.to_bits(msb.span())?;
                let (l, _) = eval(lsb, self)?.to_bits(lsb.span())?;
                if m.has_unknown() || l.has_unknown() {
                    return Ok(());
                }
                let hi = eval::bit_position(base, m.to_i64_lossy(), self);
                let lo = eval::bit_position(base, l.to_i64_lossy(), self);
                let cur2 = match (hi, lo) {
                    (Some(h), Some(l2)) if h >= 0 && l2 >= 0 && (h.max(l2) as u32) < cur.width => {
                        cur.set_part(h as u32, l2 as u32, &bits)?
                    }
                    _ => cur,
                };
                self.write_base(base, cur2, cs)
            }
            Expr::Concat { parts, .. } => {
                let mut off = bits.width;
                for p in parts {
                    let w = self.lvalue_width(p)?;
                    if off < w {
                        return Err(SimError::at(
                            "concat lvalue wider than rhs",
                            lhs.span(),
                        ));
                    }
                    off -= w;
                    let slice = bits.part(off + w - 1, off)?;
                    self.assign_lvalue(p, Val::Bits {
                        bits: slice,
                        signed: false,
                    })?;
                }
                Ok(())
            }
            other => Err(SimError::at(
                "expression is not an assignable lvalue",
                other.span(),
            )),
        }
    }

    fn read_lvalue_base(&mut self, e: &Expr) -> SResult<(Bits, bool)> {
        match e {
            Expr::Ident { name, .. } => self.read_name(name, e.span()),
            Expr::HierIdent { parts, .. } => self.read_name(&parts.join("."), e.span()),
            other => Err(SimError::at("unsupported selection base (M7)", other.span())),
        }
    }

    fn write_base(&mut self, e: &Expr, bits: Bits, signed: bool) -> SResult<()> {
        match e {
            Expr::Ident { name, .. } => self.set_name(name, bits, signed),
            Expr::HierIdent { parts, .. } => self.set_name(&parts.join("."), bits, signed),
            other => Err(SimError::at("unsupported selection base (M7)", other.span())),
        }
    }

    pub fn lvalue_width(&mut self, e: &Expr) -> SResult<u32> {
        match e {
            Expr::Ident { name, .. } => {
                let root = self.find(name);
                Ok(self.cells.get(&root).map(|c| c.bits.width).unwrap_or(1))
            }
            Expr::HierIdent { parts, .. } => {
                let key = parts.join(".");
                let root = self.find(&key);
                Ok(self.cells.get(&root).map(|c| c.bits.width).unwrap_or(1))
            }
            Expr::Index { base, .. } => {
                // Memory element lvalue: element width; plain bit-select: 1 bit.
                if let Some(mn) = eval::mem_base_name(base) {
                    if let Some((_, _, w, _)) = self.mem_info(&mn) {
                        return Ok(w);
                    }
                }
                Ok(1)
            }
            Expr::PartSelect { msb, lsb, .. } => {
                let (m, _) = eval(msb, self)?.to_bits(msb.span())?;
                let (l, _) = eval(lsb, self)?.to_bits(lsb.span())?;
                Ok((m.to_i64_lossy() - l.to_i64_lossy()).unsigned_abs() as u32 + 1)
            }
            Expr::Concat { parts, .. } => {
                let mut w = 0;
                for p in parts {
                    w += self.lvalue_width(p)?;
                }
                Ok(w)
            }
            other => Err(SimError::at("not an lvalue", other.span())),
        }
    }

    // ---------- combinational settle + sensitivity wake ----------

    pub(crate) fn settle(&mut self) -> SResult<()> {
        for _ in 0..100 {
            let mut changed = false;
            let assigns: Vec<(Expr, Expr, u32)> = self
                .assigns
                .iter()
                .map(|a| (a.lhs.clone(), a.rhs.clone(), a.lhs_w))
                .collect();
            for (lhs, rhs, w) in &assigns {
                let v = eval_ctx(rhs, self, *w)?;
                let (bits, signed) = v.to_bits(rhs.span())?;
                let cur = self.read_lvalue_base_quiet(lhs)?;
                let target = if cur.1 {
                    bits.sadjust(cur.0.width)?
                } else {
                    bits.uadjust(cur.0.width)?
                };
                if cur.0 != target {
                    self.assign_lvalue(lhs, Val::Bits { bits, signed })?;
                    changed = true;
                }
            }
            let gs: Vec<(GateKind, Vec<Expr>, Vec<Expr>)> = self
                .gates
                .iter()
                .map(|g| (g.kind, g.outs.clone(), g.ins.clone()))
                .collect();
            for (kind, outs, ins) in &gs {
                let mut iv = Vec::with_capacity(ins.len());
                for e in ins {
                    let (b, _) = eval(e, self)?.to_bits(e.span())?;
                    iv.push(b);
                }
                let val = gate_fold(*kind, &iv);
                for o in outs {
                    let cur = self.read_lvalue_base_quiet(o)?;
                    let target = if cur.1 {
                        val.sadjust(cur.0.width)?
                    } else {
                        val.uadjust(cur.0.width)?
                    };
                    if cur.0 != target {
                        self.assign_lvalue(o, Val::Bits { bits: val.clone(), signed: false })?;
                        changed = true;
                    }
                }
            }
            if !changed {
                return Ok(());
            }
        }
        Err(SimError::new(
            "combinational network did not converge after 100 iterations (combinational loop — upstream reports UNOPTFLAT)",
            None,
        ))
    }

    /// Current value of an assignment target (any lvalue form is also a
    /// valid rvalue, so evaluate it): used by settle() for change detection.
    fn read_lvalue_base_quiet(&mut self, e: &Expr) -> SResult<(Bits, bool)> {
        match e {
            Expr::Ident { name, .. } => {
                let root = self.find(name);
                Ok(match self.cells.get(&root) {
                    Some(c) => (c.bits, c.signed),
                    None => (Bits::zeros(1)?, false),
                })
            }
            other => eval(other, self)?.to_bits(other.span()),
        }
    }

    /// Re-sample every waiting process's sensitivity list; woken processes
    /// join the runnable queue in declaration order (determinism, ADR-0001).
    pub(crate) fn wake_check(&mut self) -> SResult<()> {
        let waiting: Vec<usize> = (0..self.procs.len())
            .filter(|&i| self.procs[i].state == PState::Waiting)
            .collect();
        for pid in waiting {
            let mut waits = std::mem::take(&mut self.procs[pid].waits);
            let mut triggered = false;
            for w in waits.iter_mut() {
                let (nb, _) = eval(&w.0.expr, self)?.to_bits(w.0.expr.span())?;
                if edge_fired(w.0.kind, &w.1, &nb) {
                    triggered = true;
                }
                w.1 = nb;
            }
            self.procs[pid].waits = waits;
            if triggered {
                self.procs[pid].state = PState::Runnable;
                self.runnable.push_back(pid);
            }
        }
        Ok(())
    }

    // ---------- the scheduler ----------

    pub fn run(&mut self) -> SResult<()> {
        self.settle()?;
        self.wake_check()?;
        self.fire_start_of_simulation()?;
        self.drain_vc_callbacks()?;
        self.runnable = (0..self.procs.len()).collect();
        let mut budget: u64 = 0;
        loop {
            while let Some(pid) = self.runnable.pop_front() {
                if self.finished {
                    break;
                }
                budget += 1;
                if budget > 50_000_000 {
                    return Err(SimError::new(
                        "scheduler executed >50M process steps (runaway simulation?)",
                        None,
                    ));
                }
                self.step_process(pid)?;
            }
            if self.finished {
                self.vcd_flush();
                self.fire_end_of_simulation()?;
                return Ok(());
            }
            if !self.inactive.is_empty() {
                self.runnable.extend(self.inactive.drain(..));
                continue;
            }
            if !self.nba.is_empty() {
                let items = std::mem::take(&mut self.nba);
                for (lhs, v) in items {
                    self.assign_lvalue(&lhs, v)?;
                }
                self.settle()?;
                self.wake_check()?;
                self.drain_vc_callbacks()?;
                continue;
            }
            // End of this time slot: dump what changed, then advance.
            self.vcd_flush();
            self.drain_vc_callbacks()?;
            match self.delayed.keys().next().copied() {
                Some(t) => {
                    self.now = t;
                    self.fire_due_timers()?;
                    if let Some(v) = self.delayed.remove(&t) {
                        for pid in v {
                            self.procs[pid].state = PState::Runnable;
                            self.runnable.push_back(pid);
                        }
                    }
                }
                None => {
                    self.fire_end_of_simulation()?;
                    return Ok(());
                }
            }
        }
    }

    fn step_process(&mut self, pid: usize) -> SResult<()> {
        let instrs = self.procs[pid].instrs.clone();
        let mut counters = std::mem::take(&mut self.procs[pid].counters);
        let mut pc = self.procs[pid].pc;
        let mut guard: u64 = 0;
        let flow = loop {
            guard += 1;
            if guard > 10_000_000 {
                return Err(SimError::new(
                    "process executed >10M instructions without suspending (zero-delay loop?)",
                    None,
                ));
            }
            if pc >= instrs.len() {
                break Flow::End;
            }
            match self.exec_one(&instrs, &mut pc, &mut counters, Some(pid))? {
                Flow::Next => {}
                f => break f,
            }
        };
        let p = &mut self.procs[pid];
        p.pc = pc;
        p.counters = counters;
        match flow {
            Flow::End | Flow::Finish => p.state = PState::Done,
            Flow::Next => unreachable!(),
            Flow::Suspend => {} // state set by exec_one
        }
        Ok(())
    }

    /// Execute one instruction; suspends on delay/wait (process mode) or
    /// errors on them (sync mode, `pid: None`).
    fn exec_one(
        &mut self,
        instrs: &[Instr],
        pc: &mut usize,
        counters: &mut Vec<u64>,
        pid: Option<usize>,
    ) -> SResult<Flow> {
        let instr = instrs[*pc].clone();
        match &instr {
            Instr::BAssign(lhs, rhs) => {
                let w = self.lvalue_width(lhs)?;
                let v = eval_ctx(rhs, self, w)?;
                self.assign_lvalue(lhs, v)?;
                self.settle()?;
                self.wake_check()?;
                *pc += 1;
                Ok(Flow::Next)
            }
            Instr::NbAssign(lhs, rhs) => {
                let w = self.lvalue_width(lhs)?;
                let v = eval_ctx(rhs, self, w)?;
                self.nba.push((lhs.clone(), v));
                *pc += 1;
                Ok(Flow::Next)
            }
            Instr::JumpIfFalse(cond, target) => {
                let (b, _) = eval(cond, self)?.to_bits(cond.span())?;
                *pc = if b.truthy()? == Some(true) { *pc + 1 } else { *target };
                Ok(Flow::Next)
            }
            Instr::Jump(t) => {
                *pc = *t;
                Ok(Flow::Next)
            }
            Instr::Case {
                kind,
                expr,
                arms,
                default,
            } => {
                let (v, _) = eval(expr, self)?.to_bits(expr.span())?;
                let mut target = *default;
                'outer: for (labels, t) in arms {
                    for l in labels {
                        let (lv, _) = eval(l, self)?.to_bits(l.span())?;
                        if self.case_matches(*kind, &v, &lv)? {
                            target = *t;
                            break 'outer;
                        }
                    }
                }
                *pc = target;
                Ok(Flow::Next)
            }
            Instr::RepeatInit(count) => {
                let (c, _) = eval(count, self)?.to_bits(count.span())?;
                if c.has_unknown() {
                    return Err(SimError::at("unknown repeat count", count.span()));
                }
                counters.push(c.to_u64_lossy());
                *pc += 1;
                Ok(Flow::Next)
            }
            Instr::RepeatLoop { end } => {
                match counters.last_mut() {
                    Some(c) if *c > 0 => {
                        *c -= 1;
                        *pc += 1;
                    }
                    Some(_) => {
                        counters.pop();
                        *pc = *end;
                    }
                    None => *pc = *end,
                }
                Ok(Flow::Next)
            }
            Instr::Delay(d) => {
                let ticks = self.delay_ticks(d)?;
                match pid {
                    None => Err(SimError::new(
                        "timing control inside a function is illegal (IEEE 1364-2005 §10.3)",
                        None,
                    )),
                    Some(pid) => {
                        let target = self.now.saturating_add(ticks);
                        if ticks == 0 {
                            self.inactive.push_back(pid);
                        } else {
                            self.delayed.entry(target).or_default().push(pid);
                        }
                        self.procs[pid].state = PState::Delayed;
                        *pc += 1;
                        Ok(Flow::Suspend)
                    }
                }
            }
            Instr::Wait(edges) => {
                let mut waits = Vec::with_capacity(edges.len());
                for e in edges {
                    let (b, _) = eval(&e.expr, self)?.to_bits(e.expr.span())?;
                    waits.push((e.clone(), b));
                }
                match pid {
                    None => Err(SimError::new(
                        "event control inside a function is illegal (IEEE 1364-2005 §10.3)",
                        None,
                    )),
                    Some(pid) => {
                        self.procs[pid].waits = waits;
                        self.procs[pid].state = PState::Waiting;
                        *pc += 1;
                        Ok(Flow::Suspend)
                    }
                }
            }
            Instr::SysCall(name, args) => {
                let f = self.syscall(name, args)?;
                *pc += 1;
                Ok(f)
            }
        }
    }

    fn delay_ticks(&mut self, d: &ast::DelayVal) -> SResult<u64> {
        let ratio = self.unit_fs / self.prec_fs.max(1);
        match d {
            ast::DelayVal::Lit(text) => {
                let f: f64 = text
                    .parse()
                    .map_err(|_| SimError::new(format!("bad delay literal `{text}`"), None))?;
                Ok((f * ratio as f64).round() as u64)
            }
            ast::DelayVal::Expr(e) => {
                let (b, _) = eval(e, self)?.to_bits(e.span())?;
                if b.has_unknown() {
                    return Err(SimError::at("unknown delay value", e.span()));
                }
                let v = b.to_u64_lossy();
                v.checked_mul(ratio)
                    .ok_or_else(|| SimError::at("delay overflow", e.span()))
            }
        }
    }

    fn case_matches(&self, kind: CaseKind, v: &Bits, label: &Bits) -> SResult<bool> {
        let w = v.width.max(label.width);
        let a = v.uadjust(w)?;
        let b = label.uadjust(w)?;
        match kind {
            CaseKind::Case => Ok(a.case_eq(&b)?.to_u64_lossy() == 1),
            CaseKind::Casez => {
                // x AND z in the case item are don't-care (§9.5.2).
                let care = !b.mask & low_bits(w);
                Ok(((a.data ^ b.data) & care) == 0 && ((a.mask ^ b.mask) & care) == 0)
            }
            CaseKind::Casex => {
                let care = !(a.mask | b.mask) & low_bits(w);
                Ok(((a.data ^ b.data) & care) == 0)
            }
        }
    }

    fn syscall(&mut self, name: &str, args: &[Expr]) -> SResult<Flow> {
        match name {
            "$display" | "$write" | "$error" | "$warning" | "$info" | "$fatal" | "$strobe" => {
                let text = self.format_display(args)?;
                let prefix = match name {
                    "$error" | "$fatal" => "%Error: ",
                    "$warning" => "%Warning: ",
                    "$info" => "%Info: ",
                    _ => "",
                };
                self.out.push_str(prefix);
                self.out.push_str(&text);
                if name != "$write" {
                    self.out.push('\n');
                }
                if name == "$fatal" {
                    self.finished = true;
                    return Ok(Flow::Finish);
                }
                Ok(Flow::Next)
            }
            "$finish" | "$stop" => {
                self.finished = true;
                Ok(Flow::Finish)
            }
            "$dumpfile" => {
                if let Some(a) = args.first() {
                    if let Val::Str(p) = eval(a, self)? {
                        self.dump_path = Some(p);
                    }
                }
                Ok(Flow::Next)
            }
            "$dumpvars" => {
                self.vcd_begin();
                Ok(Flow::Next)
            }
            "$dumpoff" | "$dumpon" | "$dumpall" => Ok(Flow::Next),
            "$readmemh" | "$readmemb" => self.sys_readmem(name, args),
            _ => Err(SimError::new(
                format!("system task `{name}` is not yet supported"),
                None,
            )),
        }
    }

    /// `$readmemh`/`$readmemb` (§17.2.4): whitespace-separated values into
    /// memory words; `@addr` jumps to a logical address; start/finish args
    /// bound the fill window; values wider than a word truncate to the LSBs.
    fn sys_readmem(&mut self, name: &str, args: &[Expr]) -> SResult<Flow> {
        if args.len() < 2 || args.len() > 4 {
            return Err(SimError::new(format!("{name} takes 2..4 arguments"), None));
        }
        let path = match eval(&args[0], self)? {
            Val::Str(s) => s,
            _ => {
                return Err(SimError::new(
                    format!("{name}: file name must be a string literal"),
                    None,
                ))
            }
        };
        let mn = eval::mem_base_name(&args[1]).ok_or_else(|| {
            SimError::new(format!("{name}: second argument must be a memory"), None)
        })?;
        if self.mem_info(&mn).is_none() {
            return Err(SimError::new(
                format!("{name}: `{mn}` is not a memory"),
                None,
            ));
        }
        let (lsb, n, w, _esigned) = self.mem_info(&mn).unwrap();
        let root = self.find(&mn);
        let mut opt_addr = |e: Option<&Expr>| -> SResult<Option<i64>> {
            match e {
                Some(e) => {
                    let (b, _) = eval(e, self)?.to_bits(e.span())?;
                    if b.has_unknown() {
                        return Ok(None);
                    }
                    Ok(Some(b.to_u64_lossy() as i64))
                }
                None => Ok(None),
            }
        };
        let start = opt_addr(args.get(2))?;
        let finish = opt_addr(args.get(3))?;
        let fp = if std::path::Path::new(&path).is_absolute() {
            std::path::PathBuf::from(&path)
        } else {
            self.file_base.join(&path)
        };
        let text = match self.injected_files.get(&path) {
            Some(t) => t.clone(),
            None => std::fs::read_to_string(&fp).map_err(|e| {
                SimError::new(format!("{name}: cannot open `{path}`: {e}"), None)
            })?,
        };
        let bin = name == "$readmemb";
        let chars: Vec<char> = text.chars().collect();
        let mut words = {
            let c = self.cells.get(&root).unwrap();
            c.mem.as_ref().unwrap().words.clone()
        };
        let mut touched: Vec<usize> = Vec::new();
        let mut i = 0usize;
        let mut wi: i64 = match start {
            Some(s) => s - lsb,
            None => 0,
        };
        while i < chars.len() {
            let c = chars[i];
            if c.is_whitespace() {
                i += 1;
                continue;
            }
            if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                i = (i + 2).min(chars.len());
                continue;
            }
            if c == '@' {
                i += 1;
                let mut addr: u64 = 0;
                let mut any = false;
                while i < chars.len() && (chars[i].is_ascii_hexdigit() || chars[i] == '_') {
                    if let Some(d) = chars[i].to_digit(16) {
                        addr = addr * 16 + d as u64;
                        any = true;
                    }
                    i += 1;
                }
                if !any {
                    return Err(SimError::new(format!("{name}: bad @address"), None));
                }
                wi = addr as i64 - lsb;
                continue;
            }
            // Value token: collect until whitespace/comment.
            let mut tok: Vec<char> = Vec::new();
            while i < chars.len()
                && !chars[i].is_whitespace()
                && !(chars[i] == '/' && i + 1 < chars.len() && (chars[i + 1] == '/' || chars[i + 1] == '*'))
            {
                tok.push(chars[i]);
                i += 1;
            }
            if let Some(f) = finish {
                if wi + lsb > f {
                    break;
                }
            }
            if wi >= 0 && (wi as usize) < n {
                let val = readmem_val(&tok, bin, w, name)?;
                words[wi as usize] = val;
                touched.push(wi as usize);
            }
            wi += 1;
        }
        {
            let c = self.cells.get_mut(&root).unwrap();
            c.mem.as_mut().unwrap().words = words;
        }
        if let Some(w) = self.vcd.as_mut() {
            if w.started() {
                for wi in touched {
                    let val = {
                        let c = self.cells.get(&root).unwrap();
                        c.mem.as_ref().unwrap().words[wi].clone()
                    };
                    w.record_change(&format!("{root}[{}]", lsb + wi as i64), val);
                }
            }
        }
        Ok(Flow::Next)
    }

    /// `$display` formatting per IEEE 1364-2005 §17.1.1.
    fn format_display(&mut self, args: &[Expr]) -> SResult<String> {
        let mut vals: Vec<Val> = Vec::with_capacity(args.len());
        for a in args {
            vals.push(eval(a, self)?);
        }
        let mut out = String::new();
        let fmt = match vals.first() {
            Some(Val::Str(s)) => Some(s.clone()),
            _ => None,
        };
        let Some(fmt) = fmt else {
            for v in &vals {
                match v {
                    Val::Bits { bits, .. } => out.push_str(&bits.fmt_dec()),
                    Val::Str(s) => out.push_str(s),
                }
            }
            return Ok(out);
        };
        let mut ai = 1usize;
        let chars: Vec<char> = fmt.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c != '%' {
                out.push(c);
                i += 1;
                continue;
            }
            i += 1;
            if i >= chars.len() {
                return Err(SimError::new("format string ends with `%`", None));
            }
            if chars[i] == '%' {
                out.push('%');
                i += 1;
                continue;
            }
            let mut zero_pad = false;
            let mut width: Option<usize> = None;
            while i < chars.len() {
                if chars[i] == '0' && width.is_none() && !zero_pad {
                    zero_pad = true;
                    i += 1;
                } else if let Some(d) = chars[i].to_digit(10) {
                    width = Some(width.unwrap_or(0) * 10 + d as usize);
                    i += 1;
                } else {
                    break;
                }
            }
            let Some(conv) = chars.get(i).copied() else {
                return Err(SimError::new("format string ends with `%`", None));
            };
            i += 1;
            // `%t` without an argument prints the current time (§17.1.1).
            if matches!(conv, 't' | 'T') && ai >= vals.len() {
                let (now, _ratio) = self.now();
                let mut s = now.to_string();
                let pad = if zero_pad { s.len() } else { width.unwrap_or(20).max(s.len()) };
                while s.len() < pad {
                    s.insert(0, ' ');
                }
                out.push_str(&s);
                continue;
            }
            let Some(arg) = vals.get(ai) else {
                return Err(SimError::new("not enough $display arguments", None));
            };
            ai += 1;
            let piece = match (conv, arg) {
                ('s', Val::Str(s)) => s.clone(),
                ('c', Val::Bits { bits, .. }) => char::from_u32(bits.to_u64_lossy() as u32)
                    .unwrap_or('?')
                    .to_string(),
                ('d' | 'D', Val::Bits { bits, signed }) => {
                    let natural = if *signed {
                        bits.to_i64_lossy().abs().to_string().len()
                            + (bits.to_i64_lossy() < 0) as usize
                    } else {
                        let maxv = if bits.width >= 64 {
                            u64::MAX
                        } else {
                            low_bits(bits.width)
                        };
                        maxv.to_string().len()
                    };
                    let mut s = if *signed {
                        if bits.has_unknown() { "x".to_string() } else { bits.to_i64_lossy().to_string() }
                    } else {
                        bits.fmt_dec()
                    };
                    let pad = match (zero_pad, width) {
                        (true, None) => s.chars().count(),
                        (true, Some(w)) => w.max(s.chars().count()),
                        (false, w) => w.unwrap_or(natural).max(s.chars().count()),
                    };
                    let fill = if zero_pad && width.is_some() { '0' } else { ' ' };
                    while s.chars().count() < pad {
                        s.insert(0, fill);
                    }
                    s
                }
                ('b' | 'B', Val::Bits { bits, .. }) => {
                    let mut s = bits.fmt_bin();
                    let pad = width.unwrap_or(bits.width as usize).max(s.len());
                    while s.len() < pad {
                        s.insert(0, if zero_pad { '0' } else { ' ' });
                    }
                    s
                }
                ('h' | 'H' | 'x' | 'X', Val::Bits { bits, .. }) => {
                    let nib = bits.width.div_ceil(4) as usize;
                    let mut s = bits.fmt_hex();
                    let pad = width.unwrap_or(nib).max(s.len());
                    while s.len() < pad {
                        s.insert(0, if zero_pad { '0' } else { ' ' });
                    }
                    s
                }
                ('o' | 'O', Val::Bits { bits, .. }) => {
                    if bits.has_unknown() {
                        "x".to_string()
                    } else {
                        format!("{:o}", bits.to_u64_lossy())
                    }
                }
                ('t' | 'T', Val::Bits { bits, .. }) => {
                    // %t prints in time-precision units; the argument ($time)
                    // is in time-unit units (§17.1.1/§17.3.3).
                    let (_now, ratio) = self.now();
                    let v = bits.to_u64_lossy() * ratio;
                    let mut s = v.to_string();
                    let pad = if zero_pad { s.len() } else { width.unwrap_or(20).max(s.len()) };
                    while s.len() < pad {
                        s.insert(0, ' ');
                    }
                    s
                }
                ('s', Val::Bits { .. }) => {
                    return Err(SimError::new("%s needs a string argument", None))
                }
                (_, Val::Str(_)) => {
                    return Err(SimError::new(
                        format!("format `%{conv}` needs a value argument"),
                        None,
                    ))
                }
                (other, _) => {
                    return Err(SimError::new(
                        format!("unsupported format specifier `%{other}`"),
                        None,
                    ))
                }
            };
            out.push_str(&piece);
        }
        Ok(out)
    }
}

/// IEEE 1364-2005 §9.7.2 edge transition tables (x-aware).
fn edge_fired(kind: ast::EdgeKind, old: &Bits, new: &Bits) -> bool {
    let bit = |b: &Bits| -> u8 {
        let l = b.width.max(1);
        match b.part(l - 1, l - 1) {
            Ok(x) if !x.has_unknown() => x.to_u64_lossy() as u8, // 0 or 1
            Ok(_) => 2,                                          // x/z
            Err(_) => 2,
        }
    };
    let (o, n) = (bit(old), bit(new));
    match kind {
        ast::EdgeKind::Posedge => (o == 0 && n != 0) || (o == 2 && n == 1),
        ast::EdgeKind::Negedge => (o == 1 && n != 1) || (o == 2 && n == 0),
        ast::EdgeKind::Edge | ast::EdgeKind::Any => old != new,
    }
}

fn parse_timescale(ts: Option<&str>) -> (u64, u64) {
    let fs = |mag: u64, unit: &str| -> u64 {
        let base: u64 = match unit {
            "s" => 1_000_000_000_000_000,
            "ms" => 1_000_000_000_000,
            "us" => 1_000_000_000,
            "ns" => 1_000_000,
            "ps" => 1_000,
            "fs" => 1,
            _ => 1_000_000,
        };
        base * mag
    };
    let parse_one = |s: &str| -> Option<u64> {
        let idx = s.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(s.len());
        let (num, unit) = s.split_at(idx);
        let mag: u64 = num.parse().ok()?;
        Some(fs(mag, unit))
    };
    if let Some(t) = ts {
        if let Some((u, p)) = t.split_once('/') {
            if let (Some(u), Some(p)) = (parse_one(u.trim()), parse_one(p.trim())) {
                return (u, p);
            }
        }
    }
    // No `timescale anywhere → upstream default 1ps/1ps.
    (1_000, 1_000)
}

/// Human name for a femtosecond magnitude (VCD `$timescale` header).
fn var_decl_kind(k: DeclKind) -> CellKind {
    match k {
        DeclKind::Reg => CellKind::Reg,
        DeclKind::Integer => CellKind::Integer,
        DeclKind::Time => CellKind::Time,
        DeclKind::Genvar => CellKind::Genvar,
        DeclKind::Event => CellKind::Event,
        _ => CellKind::Net,
    }
}

fn fs_name(fs: u64) -> &'static str {
    match fs {
        1_000_000_000_000_000 => "1s",
        1_000_000_000_000 => "1ms",
        1_000_000_000 => "1us",
        1_000_000 => "1ns",
        1_000 => "1ps",
        1 => "1fs",
        _ => "1ps",
    }
}

fn stmt_span(s: &Stmt) -> Span {
    match s {
        Stmt::Block { span, .. }
        | Stmt::BAssign { span, .. }
        | Stmt::NbAssign { span, .. }
        | Stmt::If { span, .. }
        | Stmt::Case { span, .. }
        | Stmt::For { span, .. }
        | Stmt::While { span, .. }
        | Stmt::Repeat { span, .. }
        | Stmt::Forever { span, .. }
        | Stmt::DelayCtrl { span, .. }
        | Stmt::EventCtrl { span, .. }
        | Stmt::SysCall { span, .. }
        | Stmt::SubCall { span, .. }
        | Stmt::Disable { span, .. }
        | Stmt::Null { span } => *span,
    }
}

fn it_span(it: &ast::ModuleItem) -> Span {
    match it {
        ast::ModuleItem::Decl(d) => d.span,
        ast::ModuleItem::ContAssign(p) | ast::ModuleItem::Defparam(p) => {
            p.first().map(|x| x.span).unwrap_or_default()
        }
        ast::ModuleItem::Always { span, .. } | ast::ModuleItem::Initial { span, .. } => *span,
        ast::ModuleItem::Instance(i) => i.span,
        ast::ModuleItem::GenerateRegion(items) => items.first().map(it_span).unwrap_or_default(),
        ast::ModuleItem::GenIf { span, .. }
        | ast::ModuleItem::GenFor { span, .. }
        | ast::ModuleItem::GenCase { span, .. }
        | ast::ModuleItem::GenBlock { span, .. } => *span,
        ast::ModuleItem::Function(f) | ast::ModuleItem::Task(f) => f.span,
        ast::ModuleItem::Empty => Span::default(),
    }
}

impl Lookup for Sim {
    fn lookup(&mut self, name: &str, span: Span) -> SResult<Val> {
        let root = self.find(name);
        if let Some(c) = self.cells.get(&root) {
            if c.mem.is_some() {
                return Err(SimError::at(
                    format!("cannot read whole memory `{name}` (must be indexed)"),
                    span,
                ));
            }
        }
        let (bits, signed) = self.read_name(name, span)?;
        Ok(Val::Bits { bits, signed })
    }

    fn mem_info(&mut self, name: &str) -> Option<(i64, usize, u32, bool)> {
        let root = self.find(name);
        let c = self.cells.get(&root)?;
        let m = c.mem.as_ref()?;
        Some((m.addr_lsb, m.words.len(), c.bits.width, c.signed))
    }

    fn read_mem_word(&mut self, name: &str, word: usize) -> SResult<Bits> {
        let root = self.find(name);
        match self.cells.get(&root).and_then(|c| c.mem.as_ref()) {
            Some(m) if word < m.words.len() => Ok(m.words[word].clone()),
            _ => Err(SimError::new(format!("bad memory word read `{name}`"), None)),
        }
    }

    fn decl_range(&mut self, name: &str) -> Option<(i64, i64)> {
        let root = self.find(name);
        self.cells.get(&root).map(|c| (c.msb, c.lsb))
    }

    fn var_info(&mut self, name: &str) -> Option<(u32, bool)> {
        let root = self.find(name);
        self.cells.get(&root).map(|c| (c.bits.width, c.signed))
    }

    fn function_info(&mut self, name: &str) -> Option<(u32, bool)> {
        self.funcs.get(name).map(|f| (f.ret_w, f.ret_s))
    }

    fn call_function(&mut self, name: &str, args: &[Val], span: Span) -> SResult<Val> {
        let def = self
            .funcs
            .get(name)
            .cloned()
            .ok_or_else(|| SimError::at(format!("unknown function `{name}`"), span))?;
        if args.len() != def.inputs.len() {
            return Err(SimError::at(
                format!(
                    "function `{name}` expects {} argument(s), got {}",
                    def.inputs.len(),
                    args.len()
                ),
                span,
            ));
        }
        for (i, (pname, w, ps)) in def.inputs.iter().enumerate() {
            let (b, _asg) = args[i].to_bits(span)?;
            let adj = if *ps { b.sadjust(*w)? } else { b.uadjust(*w)? };
            let root = self.find(pname);
            if let Some(c) = self.cells.get_mut(&root) {
                c.bits = adj;
            }
        }
        // Zero the return cell (static storage; upstream policy = 0-init).
        {
            let root = self.find(&def.ret);
            if let Some(c) = self.cells.get_mut(&root) {
                c.bits = Bits::zeros(c.bits.width)?;
            }
        }
        let instrs = def.instrs.clone();
        let mut pc = 0usize;
        let mut counters = Vec::new();
        loop {
            if pc >= instrs.len() {
                break;
            }
            match self.exec_one(&instrs, &mut pc, &mut counters, None)? {
                Flow::Next => {}
                Flow::End | Flow::Finish => break,
                Flow::Suspend => unreachable!("sync mode errors on suspend"),
            }
        }
        let root = self.find(&def.ret);
        let (bits, signed) = match self.cells.get(&root) {
            Some(c) => (c.bits, c.signed),
            None => (Bits::zeros(def.ret_w)?, def.ret_s),
        };
        Ok(Val::Bits { bits, signed })
    }

    fn now(&self) -> (u64, u64) {
        let ratio = if self.prec_fs == 0 {
            1
        } else {
            self.unit_fs / self.prec_fs
        };
        (self.now, ratio)
    }
}
