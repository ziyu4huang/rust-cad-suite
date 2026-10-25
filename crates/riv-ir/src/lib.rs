//! riv-ir — the RIV-1 intermediate representation shared by `riv-elab`
//! (writer) and `riv-kernel` (reader/interpreter).
//!
//! Format contract (docs/riv-r2-design.md §3):
//! - Text, line-oriented. First line `riv 1`, last line `riv end`
//!   (the reader asserts both). Section order is fixed:
//!   `meta`, `vars`, `arrays`, `assigns`, `threads`, `funcs`, `strs`.
//! - Literals travel as (width, signed, base, digits) and only become
//!   `vl_values::Bits` at load time (PLAN §9.8-5) — this crate stays
//!   zero-dependency (PLAN §2-4).
//! - Ops are three-address with widths/signs baked at lowering time
//!   (IEEE 1364-2005 §5.4/§5.5); every operand is a frame slot index,
//!   there is no value stack (§9.8-1: threads carry an explicit PC and
//!   survive suspension/serialization).
//! - The writer→reader→writer roundtrip must be byte-identical (tested).
//!
//! Deviation from the design's `Thread.pc0` field: each thread owns its
//! op vector, so `pc0` is always 0 and is not carried; the on-disk op
//! lines keep their `<pc>` column as a format-stability anchor and the
//! reader verifies pcs are sequential.

/// Format version stamped in the header. IR growth (R3/R4/R5 batches)
/// bumps this; R6 freezes byte stability (design §3). v2 = the C5/R3-A
/// behavioral batch (casez/casex wildcards, $time/$monitor family,
/// plusargs). v3 = the C6/R3-B procedural batch: Call with explicit
/// formal-slot wiring (ins/outs), fork/join ops, force/release ops.
/// v4 = the R4 structural batch: array element access (`Lval::Elem` +
/// `LoadElem`), the `gates` section (primitives + delay triples), event
/// vars (`->ev`/`@ev`), `$stop`, and `$readmemh/b`.
pub const FORMAT_VERSION: u32 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Design {
    pub meta: Meta,
    pub vars: Vec<Var>,
    /// Unpacked-dimension rows, keyed by var id. Empty until arrays land
    /// (R4); the section exists so the format does not grow a version for
    /// a purely additive section.
    pub arrays: Vec<ArrayDim>,
    /// Cont-assign thread indices with their LHS var (design §3 `assigns`).
    pub assigns: Vec<(u32, u32)>,
    /// Gate primitives (R4 structural batch): evaluated by the kernel on
    /// input change, output updates scheduled with rise/fall/turn-off
    /// delays (inertial — a newer evaluation replaces the pending one).
    pub gates: Vec<Gate>,
    pub threads: Vec<Thread>,
    pub funcs: Vec<Func>,
    /// Compiled display-format pool (design §3 strs + §5-4: entries are
    /// lowered piece programs, not raw format strings).
    pub strs: Vec<Vec<FmtPiece>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meta {
    pub top: String,
    /// Raw `` `timescale `` string, e.g. `1ns/1ps`.
    pub timescale: String,
    /// Precision half, e.g. `1ps` (rendered in the `$finish` line).
    pub precision: String,
    /// Unit half, e.g. `1ns`.
    pub unit: String,
    /// Source file paths exactly as passed on the command line.
    pub sources: Vec<String>,
}

pub type VarId = u32;
pub type Slot = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarKind {
    Reg,
    Net,
    Array,
    /// Named event (R4-e): no value storage; `->ev` triggers, `@ev` waits
    /// fire on the trigger in the slot the trigger happens.
    Event,
}

/// Gate primitive kinds (IEEE §7 table families; bufif/notif output z when
/// disabled — 4-state tables live in the kernel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateKind {
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Xnor,
    Buf,
    Not,
    Bufif0,
    Bufif1,
    Notif0,
    Notif1,
}

impl GateKind {
    pub fn tag(self) -> &'static str {
        match self {
            GateKind::And => "and",
            GateKind::Nand => "nand",
            GateKind::Or => "or",
            GateKind::Nor => "nor",
            GateKind::Xor => "xor",
            GateKind::Xnor => "xnor",
            GateKind::Buf => "buf",
            GateKind::Not => "not",
            GateKind::Bufif0 => "bufif0",
            GateKind::Bufif1 => "bufif1",
            GateKind::Notif0 => "notif0",
            GateKind::Notif1 => "notif1",
        }
    }
    pub fn from_tag(s: &str) -> Option<GateKind> {
        Some(match s {
            "and" => GateKind::And,
            "nand" => GateKind::Nand,
            "or" => GateKind::Or,
            "nor" => GateKind::Nor,
            "xor" => GateKind::Xor,
            "xnor" => GateKind::Xnor,
            "buf" => GateKind::Buf,
            "not" => GateKind::Not,
            "bufif0" => GateKind::Bufif0,
            "bufif1" => GateKind::Bufif1,
            "notif0" => GateKind::Notif0,
            "notif1" => GateKind::Notif1,
            _ => return None,
        })
    }
}

/// One gate primitive instance (R4): output var + input vars + the delay
/// triple in precision ticks (rise, fall, turn-off; turn-off = transition
/// to z, bufif/notif only). Zero delays behave like a cont-assign.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gate {
    pub kind: GateKind,
    pub out: VarId,
    pub inputs: Vec<VarId>,
    pub rise: u64,
    pub fall: u64,
    pub turnoff: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Var {
    pub id: VarId,
    pub name: String,
    pub width: u32,
    pub signed: bool,
    pub kind: VarKind,
    /// Declared range (msb, lsb) — descending when msb >= lsb. Bit/part
    /// selects address DECLARED indices; the kernel maps them to absolute
    /// positions with these bounds (descending: abs = idx - lsb;
    /// ascending: abs = lsb - idx).
    /// Declaration initializer (regs and net constant drivers alike); the
    /// kernel turns this into a value-change event at t0 so `always @*`
    /// and cont-assign threads see the transition (design §4).
    pub init: Option<Literal>,
    pub msb: u32,
    pub lsb: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrayDim {
    pub var: VarId,
    /// (msb, lsb) per unpacked dimension, elaboration-order.
    pub dims: Vec<(u32, u32)>,
}

/// Based-literal bases (mirrors `vl_syntax::ast::Base`; this crate stays
/// zero-dependency so the AST type cannot be referenced).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    Dec,
    Bin,
    Oct,
    Hex,
}

impl Base {
    pub fn tag(self) -> char {
        match self {
            Base::Dec => 'd',
            Base::Bin => 'b',
            Base::Oct => 'o',
            Base::Hex => 'h',
        }
    }
    pub fn from_tag(c: char) -> Option<Base> {
        Some(match c {
            'd' => Base::Dec,
            'b' => Base::Bin,
            'o' => Base::Oct,
            'h' => Base::Hex,
            _ => return None,
        })
    }
}

/// A literal as carried on the wire: width, signedness, base and digits
/// (x/z/? preserved verbatim, underscores removed by the frontend).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Literal {
    pub width: u32,
    pub signed: bool,
    pub base: Base,
    pub digits: String,
}

impl Literal {
    pub fn encode(&self) -> String {
        format!("{}{}", self.base.tag(), self.digits)
    }
    pub fn decode(w: u32, signed: bool, s: &str) -> Option<Literal> {
        let mut it = s.chars();
        let base = Base::from_tag(it.next()?)?;
        let digits = it.as_str().to_string();
        if digits.is_empty() {
            return None;
        }
        Some(Literal {
            width: w,
            signed,
            base,
            digits,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadKind {
    Initial,
    Always,
    ContAssign,
}

impl ThreadKind {
    pub fn tag(self) -> &'static str {
        match self {
            ThreadKind::Initial => "initial",
            ThreadKind::Always => "always",
            ThreadKind::ContAssign => "cont-assign",
        }
    }
    pub fn from_tag(s: &str) -> Option<ThreadKind> {
        Some(match s {
            "initial" => ThreadKind::Initial,
            "always" => ThreadKind::Always,
            "cont-assign" => ThreadKind::ContAssign,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    Pos,
    Neg,
    /// Any change — the level-sensitive arm (`@*`, `@(a or b)`); the
    /// kernel treats a pure-`Any` wait as a level wait (design §4).
    Any,
}

impl EdgeKind {
    pub fn tag(self) -> &'static str {
        match self {
            EdgeKind::Pos => "pos",
            EdgeKind::Neg => "neg",
            EdgeKind::Any => "any",
        }
    }
    pub fn from_tag(s: &str) -> Option<EdgeKind> {
        Some(match s {
            "pos" => EdgeKind::Pos,
            "neg" => EdgeKind::Neg,
            "any" => EdgeKind::Any,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SensItem {
    pub var: VarId,
    pub edge: EdgeKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thread {
    pub kind: ThreadKind,
    /// Sensitivity for edge-triggered `always` threads (the kernel arms
    /// these before first execution). Pure-level threads carry `[]` and
    /// start ready — their wait is an in-body `WaitEdge` op so the
    /// combinational t0 first evaluation is expressed in the IR itself.
    pub sens: Vec<SensItem>,
    pub frame_len: u32,
    pub body: Vec<Op>,
}

/// Task/function subroutine (design §3: the frame model is frozen at C2 —
/// per-activation frames (`automatic`) plus per-instance shared frames
/// for `static` callees, both expressed by `automatic` + `frame_len`;
/// the kernel owns the two frame disciplines).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Func {
    pub name: String,
    pub is_task: bool,
    pub automatic: bool,
    /// Slots for args (in call order, starting at 0) + locals + temps.
    pub frame_len: u32,
    pub ret: Option<(u32, bool)>,
    pub body: Vec<Op>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinKind {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
}

impl BinKind {
    pub fn tag(self) -> &'static str {
        match self {
            BinKind::Add => "add",
            BinKind::Sub => "sub",
            BinKind::Mul => "mul",
            BinKind::Div => "div",
            BinKind::Rem => "rem",
            BinKind::Pow => "pow",
        }
    }
    pub fn from_tag(s: &str) -> Option<BinKind> {
        Some(match s {
            "add" => BinKind::Add,
            "sub" => BinKind::Sub,
            "mul" => BinKind::Mul,
            "div" => BinKind::Div,
            "rem" => BinKind::Rem,
            "pow" => BinKind::Pow,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitKind {
    And,
    Or,
    Xor,
}

impl BitKind {
    pub fn tag(self) -> &'static str {
        match self {
            BitKind::And => "and",
            BitKind::Or => "or",
            BitKind::Xor => "xor",
        }
    }
    pub fn from_tag(s: &str) -> Option<BitKind> {
        Some(match s {
            "and" => BitKind::And,
            "or" => BitKind::Or,
            "xor" => BitKind::Xor,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedKind {
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Xnor,
}

impl RedKind {
    pub fn tag(self) -> &'static str {
        match self {
            RedKind::And => "and",
            RedKind::Nand => "nand",
            RedKind::Or => "or",
            RedKind::Nor => "nor",
            RedKind::Xor => "xor",
            RedKind::Xnor => "xnor",
        }
    }
    pub fn from_tag(s: &str) -> Option<RedKind> {
        Some(match s {
            "and" => RedKind::And,
            "nand" => RedKind::Nand,
            "or" => RedKind::Or,
            "nor" => RedKind::Nor,
            "xor" => RedKind::Xor,
            "xnor" => RedKind::Xnor,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmpKind {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

impl CmpKind {
    pub fn tag(self) -> &'static str {
        match self {
            CmpKind::Lt => "lt",
            CmpKind::Le => "le",
            CmpKind::Gt => "gt",
            CmpKind::Ge => "ge",
            CmpKind::Eq => "eq",
            CmpKind::Ne => "ne",
        }
    }
    pub fn from_tag(s: &str) -> Option<CmpKind> {
        Some(match s {
            "lt" => CmpKind::Lt,
            "le" => CmpKind::Le,
            "gt" => CmpKind::Gt,
            "ge" => CmpKind::Ge,
            "eq" => CmpKind::Eq,
            "ne" => CmpKind::Ne,
            _ => return None,
        })
    }
}

/// Case comparison family (IEEE 1364-2005 §9.5): `Exact*` is `===`/`!==`
/// (bit identity); `WildZ` is casez — z/? wildcarding in BOTH the case
/// expression and the item; `WildX` is casex — x and z wildcard in both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseCmpKind {
    ExactEq,
    ExactNe,
    WildZ,
    WildX,
}

impl CaseCmpKind {
    pub fn tag(self) -> &'static str {
        match self {
            CaseCmpKind::ExactEq => "exact",
            CaseCmpKind::ExactNe => "ne",
            CaseCmpKind::WildZ => "wildz",
            CaseCmpKind::WildX => "wildx",
        }
    }
    pub fn from_tag(s: &str) -> Option<CaseCmpKind> {
        Some(match s {
            "exact" => CaseCmpKind::ExactEq,
            "ne" => CaseCmpKind::ExactNe,
            "wildz" => CaseCmpKind::WildZ,
            "wildx" => CaseCmpKind::WildX,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShiftKind {
    Shl,
    Shr,
    /// Arithmetic right shift; fill governed by `signed` (AGENTS 10.8).
    Ashr,
}

impl ShiftKind {
    pub fn tag(self) -> &'static str {
        match self {
            ShiftKind::Shl => "shl",
            ShiftKind::Shr => "shr",
            ShiftKind::Ashr => "ashr",
        }
    }
    pub fn from_tag(s: &str) -> Option<ShiftKind> {
        Some(match s {
            "shl" => ShiftKind::Shl,
            "shr" => ShiftKind::Shr,
            "ashr" => ShiftKind::Ashr,
            _ => return None,
        })
    }
}

/// System calls carried by RIV-1. The enum only lists implemented ids;
/// the elaborator hard-errors unimplemented ones with their milestone
/// pointer before IR exists, and the reader rejects unknown ids (an IR
/// growth bumps FORMAT_VERSION).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SysId {
    /// `$display` — `newline` is folded into the id.
    Display,
    /// `$write` — same family, no trailing newline (11_display golden).
    Write,
    Finish,
    Dumpfile,
    Dumpvars,
    /// `$clog2` as an expression (value-returning).
    Clog2,
    /// `$time` — value-returning, unit ticks (round(now / scale)).
    Time,
    /// `$strobe` — rendered in the monitor region (end-of-slot values).
    Strobe,
    /// `$monitor` — arms/re-arms the slot monitor (t0 arm: the first
    /// monitor pass after arming displays unconditionally).
    Monitor,
    MonitorOff,
    MonitorOn,
    /// `$test$plusargs("pat")` — dst = 1 iff a plusarg matches the pattern.
    TestPlusargs,
    /// `$value$plusargs("N=%d", out)` — parsed value written into
    /// args[0].slot, dst = 1 iff matched.
    ValuePlusargs,
    /// `$stop` — rivD2: without -n the banner prints and simulation
    /// CONTINUES (EOF semantics); with -n rivvp treats it as $finish.
    Stop,
    /// `$readmemh("file", mem[, start[, finish]])` — file name in `fmt`,
    /// optional bounds in args[0]/args[1] slots (width 0 = absent).
    ReadmemH,
    /// `$readmemb` — same shape as ReadmemH, binary digits.
    ReadmemB,
    /// `-> ev` — trigger the named event var.
    Trigger,
    /// `$fopen("file"[, "w"])` — dst receives the descriptor. Mode rides in
    /// args[0].width (0 = one-arg MCD form, 1 = "w" channel-FD form).
    Fopen,
    /// `$fclose(fd)` — fd in args[0].slot.
    Fclose,
    /// `$fwrite(fd, fmt, args...)` — fd in args[0].slot, format in `fmt`,
    /// remaining args render through it.
    Fwrite,
    /// `$fdisplay` — same as Fwrite plus a newline.
    Fdisplay,
    /// `$dumpall` / `$dumpoff` / `$dumpon` — dump-control sections.
    Dumpall,
    Dumpoff,
    Dumpon,
    /// `$random[(seed)]` — 32-bit LCG; seed form updates args[0].slot.
    /// Sequence values are implementation-specific (rivD11).
    Random,
}

impl SysId {
    pub fn tag(self) -> &'static str {
        match self {
            SysId::Display => "display",
            SysId::Write => "write",
            SysId::Finish => "finish",
            SysId::Dumpfile => "dumpfile",
            SysId::Dumpvars => "dumpvars",
            SysId::Clog2 => "clog2",
            SysId::Time => "time",
            SysId::Strobe => "strobe",
            SysId::Monitor => "monitor",
            SysId::MonitorOff => "monitoroff",
            SysId::MonitorOn => "monitoron",
            SysId::TestPlusargs => "testplusargs",
            SysId::ValuePlusargs => "valueplusargs",
            SysId::Stop => "stop",
            SysId::ReadmemH => "readmemh",
            SysId::ReadmemB => "readmemb",
            SysId::Trigger => "trigger",
            SysId::Fopen => "fopen",
            SysId::Fclose => "fclose",
            SysId::Fwrite => "fwrite",
            SysId::Fdisplay => "fdisplay",
            SysId::Dumpall => "dumpall",
            SysId::Dumpoff => "dumpoff",
            SysId::Dumpon => "dumpon",
            SysId::Random => "random",
        }
    }
    pub fn from_tag(s: &str) -> Option<SysId> {
        Some(match s {
            "display" => SysId::Display,
            "write" => SysId::Write,
            "finish" => SysId::Finish,
            "dumpfile" => SysId::Dumpfile,
            "dumpvars" => SysId::Dumpvars,
            "clog2" => SysId::Clog2,
            "time" => SysId::Time,
            "strobe" => SysId::Strobe,
            "monitor" => SysId::Monitor,
            "monitoroff" => SysId::MonitorOff,
            "monitoron" => SysId::MonitorOn,
            "testplusargs" => SysId::TestPlusargs,
            "valueplusargs" => SysId::ValuePlusargs,
            "stop" => SysId::Stop,
            "readmemh" => SysId::ReadmemH,
            "readmemb" => SysId::ReadmemB,
            "trigger" => SysId::Trigger,
            "fopen" => SysId::Fopen,
            "fclose" => SysId::Fclose,
            "fwrite" => SysId::Fwrite,
            "fdisplay" => SysId::Fdisplay,
            "dumpall" => SysId::Dumpall,
            "dumpoff" => SysId::Dumpoff,
            "dumpon" => SysId::Dumpon,
            "random" => SysId::Random,
            _ => return None,
        })
    }
}

/// One display argument: slot plus the baked type info the formatter
/// needs (IEEE §5.4.1 self-determined width + all-operands signedness).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arg {
    pub slot: Slot,
    pub width: u32,
    pub signed: bool,
    /// `$time`-family value (unit ticks) — `%t` rescales by meta scale.
    pub is_time: bool,
}

/// Assignment target, flattened at lowering (concat lvalues become a
/// width-partitioned list; see `Lval::Concat`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lval {
    Var { var: VarId },
    Bit { var: VarId, idx: Slot },
    /// Constant-bounds part select, absolute bit positions.
    Part { var: VarId, hi: u32, lo: u32 },
    /// Variable-bounds part select, absolute positions in slots.
    PartDyn { var: VarId, hi: Slot, lo: Slot },
    /// Unpacked-array element: `mem[idx]` (idx a slot, declared index
    /// space — the kernel maps it through the array dims).
    Elem { var: VarId, idx: Slot },
    /// Concat lvalue, flattened: parts in bit order, each with its width
    /// (the RHS is sliced left-to-right by these widths).
    Concat { parts: Vec<(Lval, u32)> },
}

/// Compiled display format piece (design §5-4: formats are compiled at
/// lowering; the kernel renders pieces, it does not parse format strings).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FmtPiece {
    Text(String),
    Spec { ch: char, width: u32, zero_pad: bool, arg: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    Const { dst: Slot, lit: Literal },
    Mov { dst: Slot, src: Slot },
    Bin { kind: BinKind, dst: Slot, a: Slot, b: Slot, w: u32, signed: bool },
    Bit { kind: BitKind, dst: Slot, a: Slot, b: Slot, w: u32 },
    /// Binary xnor (`a ~^ b`), kept separate from Bit/Xor so the kernel
    /// does not need a negate fixup.
    BitXnor { dst: Slot, a: Slot, b: Slot, w: u32 },
    BitNot { dst: Slot, a: Slot, w: u32 },
    Reduce { kind: RedKind, dst: Slot, a: Slot },
    LogicAnd { dst: Slot, a: Slot, b: Slot },
    LogicOr { dst: Slot, a: Slot, b: Slot },
    LogicNot { dst: Slot, a: Slot },
    Cmp { kind: CmpKind, dst: Slot, a: Slot, b: Slot, signed: bool },
    CaseCmp { kind: CaseCmpKind, dst: Slot, a: Slot, b: Slot },
    Shift { kind: ShiftKind, dst: Slot, a: Slot, b: Slot, w: u32, signed: bool },
    /// Ternary select; unknown cond → bitwise merge of both arms
    /// (vl_values::Bits::merge).
    Sel { dst: Slot, cond: Slot, a: Slot, b: Slot },
    Concat { dst: Slot, parts: Vec<Slot> },
    Replicate { dst: Slot, src: Slot, times: u64 },
    LoadVar { dst: Slot, var: VarId },
    LoadBit { dst: Slot, var: VarId, idx: Slot },
    LoadPart { dst: Slot, var: VarId, hi: u32, lo: u32 },
    LoadPartDyn { dst: Slot, var: VarId, hi: Slot, lo: Slot },
    /// Unpacked-array element read: `mem[idx]` → element value (width =
    /// the array's packed width); unknown/out-of-range index → x.
    LoadElem { dst: Slot, var: VarId, idx: Slot },
    /// `force lhs = rhs;` — procedural override; reads see the forced
    /// value, writes are ignored until `Release` (04_force golden: the
    /// reg keeps the forced value after release).
    AssignForce { lv: Lval, src: Slot },
    Release { lv: Lval },
    /// `fork` — spawn one thread per branch (cloned context), suspend the
    /// current thread on the join group; `JoinDone` in each branch
    /// decrements; the last one resumes the parent at `join`.
    Fork { branches: Vec<u32>, join: u32 },
    JoinDone,
    /// Join resume point — a no-op when executed (the suspension is via
    /// the join status, not this op).
    JoinPoint,
    /// Task/function call. Formal wiring is explicit: `ins` pairs
    /// (caller slot, callee formal slot) are copied in; `outs` pairs
    /// (caller lvalue, callee formal slot) are committed back on return.
    /// dst receives the callee's ret slot (slot 0) for functions.
    Call {
        func: u32,
        ins: Vec<(Slot, u32)>,
        outs: Vec<(Lval, u32)>,
        dst: Option<Slot>,
    },
    Ret { src: Slot },
    /// Blocking assignment — immediate commit, change events wake
    /// waiters (design §4 active region).
    AssignBlk { lv: Lval, src: Slot },
    /// Nonblocking assignment — value sampled now, committed in the NBA
    /// region of the same slot.
    AssignNba { lv: Lval, src: Slot },
    /// NBA with intra-assignment delay (`q <= #10 d`): value sampled now,
    /// committed in the NBA region of the slot `now + amount` — the
    /// process does NOT suspend.
    AssignNbaDelay { lv: Lval, src: Slot, amount: Slot },
    Branch { cond: Slot, then: u32, els: u32 },
    Jump { to: u32 },
    /// Suspend until `now + value` (value in precision units); 0 → the
    /// inactive region of the current slot.
    Delay { amount: Slot },
    /// Scale a unit-tick time value into precision units by `num/den`
    /// (baked from the source file's timescale; also serves $time/%t).
    ScaleTime { dst: Slot, src: Slot, num: u64, den: u64 },
    WaitEdge { items: Vec<SensItem> },
    /// `start` = pc where this syscall's argument-producer ops begin
    /// (Monitor/Strobe re-execute `body[start..self_pc]` at render time so
    /// `$time` and variable reads are fresh; 0 for the rest).
    SysCall { id: SysId, fmt: Option<u32>, args: Vec<Arg>, dst: Option<Slot>, file: u32, line: u32, start: u32 },
    EndThread,
}

/// Escape a field that may contain spaces/tabs (paths, var names, text).
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ' ' => out.push_str("\\ "),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

pub fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('\\') => out.push('\\'),
            Some(' ') => out.push(' '),
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Split a line into fields on unescaped spaces (`\ ` stays in-field).
/// Backslash escapes survive inside the returned fields; callers run
/// `unescape` on the value fields.
fn fields(l: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut esc = false;
    for (i, c) in l.char_indices() {
        if esc {
            esc = false;
            continue;
        }
        match c {
            '\\' => esc = true,
            ' ' => {
                out.push(&l[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&l[start..]);
    out
}

fn fmt_pieces_encode(pieces: &[FmtPiece]) -> Vec<String> {
    let mut lines = Vec::new();
    for p in pieces {
        match p {
            FmtPiece::Text(t) => lines.push(format!("str text {}", escape(t))),
            FmtPiece::Spec { ch, width, zero_pad, arg } => {
                lines.push(format!("str spec {ch} {width} {} {arg}", if *zero_pad { 1 } else { 0 }))
            }
        }
    }
    lines.push("str end".to_string());
    lines
}

impl Design {
    pub fn write(&self) -> String {
        let mut o = String::new();
        o.push_str(&format!("riv {}\n", FORMAT_VERSION));
        o.push_str("meta\n");
        o.push_str(&format!("top {}\n", escape(&self.meta.top)));
        o.push_str(&format!("timescale {}\n", escape(&self.meta.timescale)));
        o.push_str(&format!("unit {}\n", escape(&self.meta.unit)));
        o.push_str(&format!("precision {}\n", escape(&self.meta.precision)));
        o.push_str(&format!("sources {}\n", self.meta.sources.len()));
        for s in &self.meta.sources {
            o.push_str(&format!("src {}\n", escape(s)));
        }
        o.push_str("end\n");
        o.push_str("vars\n");
        for v in &self.vars {
            let init = match &v.init {
                Some(l) => l.encode(),
                None => "-".to_string(),
            };
            o.push_str(&format!(
                "var {} {} {} {} {} {} {} {}\n",
                v.id,
                match v.kind {
                    VarKind::Reg => "reg",
                    VarKind::Net => "net",
                    VarKind::Array => "array",
                    VarKind::Event => "event",
                },
                v.width,
                if v.signed { 1 } else { 0 },
                escape(&v.name),
                init,
                v.msb,
                v.lsb
            ));
        }
        o.push_str("end\n");
        o.push_str("arrays\n");
        for a in &self.arrays {
            o.push_str(&format!("arr {} {}", a.var, a.dims.len()));
            for (m, l) in &a.dims {
                o.push_str(&format!(" {m} {l}"));
            }
            o.push('\n');
        }
        o.push_str("end\n");
        o.push_str("assigns\n");
        for (t, v) in &self.assigns {
            o.push_str(&format!("assign {t} {v}\n"));
        }
        o.push_str("end\n");
        o.push_str("gates\n");
        for g in &self.gates {
            let mut line = format!(
                "gate {} {} {} {} {} {}",
                g.kind.tag(),
                g.out,
                g.inputs.len(),
                g.rise,
                g.fall,
                g.turnoff
            );
            for i in &g.inputs {
                line.push_str(&format!(" {i}"));
            }
            line.push('\n');
            o.push_str(&line);
        }
        o.push_str("end\n");
        o.push_str("threads\n");
        for t in &self.threads {
            o.push_str(&format!(
                "thread {} {} {}\n",
                t.kind.tag(),
                t.frame_len,
                t.sens.len()
            ));
            for s in &t.sens {
                o.push_str(&format!("sens {} {}\n", s.var, s.edge.tag()));
            }
            for (pc, op) in t.body.iter().enumerate() {
                o.push_str(&format!("{pc} {}\n", op_encode(op)));
            }
            o.push_str("endthread\n");
        }
        o.push_str("end\n");
        o.push_str("funcs\n");
        for f in &self.funcs {
            let ret = match f.ret {
                Some((w, s)) => format!("{w} {}", if s { 1 } else { 0 }),
                None => "-".to_string(),
            };
            o.push_str(&format!(
                "func {} {} {} {} {}\n",
                escape(&f.name),
                if f.is_task { "task" } else { "function" },
                if f.automatic { 1 } else { 0 },
                f.frame_len,
                ret
            ));
            for (pc, op) in f.body.iter().enumerate() {
                o.push_str(&format!("{pc} {}\n", op_encode(op)));
            }
            o.push_str("endfunc\n");
        }
        o.push_str("end\n");
        o.push_str("strs\n");
        for (i, pieces) in self.strs.iter().enumerate() {
            o.push_str(&format!("strs {i} {}\n", pieces.len()));
            o.push_str(&fmt_pieces_encode(pieces).join("\n"));
            o.push('\n');
        }
        o.push_str("end\n");
        o.push_str("riv end\n");
        o
    }
}

fn op_encode(op: &Op) -> String {
    let flag = |v: bool| if v { 1 } else { 0 };
    match op {
        Op::Const { dst, lit } => format!("const {dst} {} {} {}", lit.width, flag(lit.signed), lit.encode()),
        Op::Mov { dst, src } => format!("mov {dst} {src}"),
        Op::Bin { kind, dst, a, b: bb, w, signed } => {
            format!("bin {} {dst} {a} {bb} {w} {}", kind.tag(), flag(*signed))
        }
        Op::Bit { kind, dst, a, b, w } => format!("bit {} {dst} {a} {b} {w}", kind.tag()),
        Op::BitXnor { dst, a, b, w } => format!("bitxnor {dst} {a} {b} {w}"),
        Op::BitNot { dst, a, w } => format!("bitnot {dst} {a} {w}"),
        Op::Reduce { kind, dst, a } => format!("reduce {} {dst} {a}", kind.tag()),
        Op::LogicAnd { dst, a, b } => format!("logicand {dst} {a} {b}"),
        Op::LogicOr { dst, a, b } => format!("logicor {dst} {a} {b}"),
        Op::LogicNot { dst, a } => format!("logicnot {dst} {a}"),
        Op::Cmp { kind, dst, a, b, signed } => format!("cmp {} {dst} {a} {b} {}", kind.tag(), flag(*signed)),
        Op::CaseCmp { kind, dst, a, b } => format!("casecmp {} {dst} {a} {b}", kind.tag()),
        Op::Shift { kind, dst, a, b, w, signed } => {
            format!("shift {} {dst} {a} {b} {w} {}", kind.tag(), flag(*signed))
        }
        Op::Sel { dst, cond, a, b } => format!("sel {dst} {cond} {a} {b}"),
        Op::Concat { dst, parts } => {
            let mut s = format!("concat {dst} {}", parts.len());
            for p in parts {
                s.push_str(&format!(" {p}"));
            }
            s
        }
        Op::Replicate { dst, src, times } => format!("replicate {dst} {src} {times}"),
        Op::LoadVar { dst, var } => format!("loadvar {dst} {var}"),
        Op::LoadBit { dst, var, idx } => format!("loadbit {dst} {var} {idx}"),
        Op::LoadPart { dst, var, hi, lo } => format!("loadpart {dst} {var} {hi} {lo}"),
        Op::LoadPartDyn { dst, var, hi, lo } => format!("loadpartdyn {dst} {var} {hi} {lo}"),
        Op::LoadElem { dst, var, idx } => format!("loadelem {dst} {var} {idx}"),
        Op::AssignBlk { lv, src } => format!("assignblk {} {src}", lval_encode(lv)),
        Op::AssignNba { lv, src } => format!("assignnba {} {src}", lval_encode(lv)),
        Op::AssignNbaDelay { lv, src, amount } => format!("assignnbadelay {} {src} {amount}", lval_encode(lv)),
        Op::Branch { cond, then, els } => format!("branch {cond} {then} {els}"),
        Op::Jump { to } => format!("jump {to}"),
        Op::Delay { amount } => format!("delay {amount}"),
        Op::ScaleTime { dst, src, num, den } => format!("scaletime {dst} {src} {num} {den}"),
        Op::WaitEdge { items } => {
            let mut s = format!("waitedge {}", items.len());
            for it in items {
                s.push_str(&format!(" {} {}", it.var, it.edge.tag()));
            }
            s
        }
        Op::SysCall { id, fmt, args, dst, file, line, start } => {
            let mut s = format!("syscall {} {} {}", id.tag(), fmt.map(|f| f.to_string()).unwrap_or_else(|| "-".into()), args.len());
            for a in args {
                s.push_str(&format!(" {} {} {} {}", a.slot, a.width, flag(a.signed), flag(a.is_time)));
            }
            s.push_str(&format!(
                " {} {file} {line} {start}",
                dst.map(|d| d.to_string()).unwrap_or_else(|| "-".into())
            ));
            s
        }


        Op::AssignForce { lv, src } => format!("assignforce {} {src}", lval_encode(lv)),
        Op::Release { lv } => format!("release {}", lval_encode(lv)),
        Op::Fork { branches, join } => {
            let mut s2 = format!("fork {} {join}", branches.len());
            for b in branches {
                s2.push_str(&format!(" {b}"));
            }
            s2
        }
        Op::JoinDone => "joindone".to_string(),
        Op::JoinPoint => "joinpoint".to_string(),
        Op::Call { func, ins, outs, dst } => {
            let mut s2 = format!("call {func} {}", ins.len());
            for (a, fs) in ins {
                s2.push_str(&format!(" {a} {fs}"));
            }
            s2.push_str(&format!(" {}", outs.len()));
            for (lv, fs) in outs {
                s2.push_str(&format!(" {} {fs}", lval_encode(lv)));
            }
            s2.push_str(&format!(
                " {}",
                dst.map(|d| d.to_string()).unwrap_or_else(|| "-".into())
            ));
            s2
        }
        Op::Ret { src } => format!("ret {src}"),
        Op::EndThread => "endthread".to_string(),
    }
}

fn lval_encode(lv: &Lval) -> String {
    match lv {
        Lval::Var { var } => format!("var {var}"),
        Lval::Bit { var, idx } => format!("bit {var} {idx}"),
        Lval::Part { var, hi, lo } => format!("part {var} {hi} {lo}"),
        Lval::PartDyn { var, hi, lo } => format!("partdyn {var} {hi} {lo}"),
        Lval::Elem { var, idx } => format!("elem {var} {idx}"),
        Lval::Concat { parts } => {
            let mut s = format!("concat {}", parts.len());
            for (p, w) in parts {
                s.push_str(&format!(" {} {w}", lval_encode(p)));
            }
            s
        }
    }
}

// ---------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
pub struct ReadError(pub String);

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "riv-ir: {}", self.0)
    }
}

struct Lines<'a> {
    raw: std::str::Lines<'a>,
    n: usize,
}

impl<'a> Lines<'a> {
    fn next(&mut self) -> Option<&'a str> {
        self.n += 1;
        self.raw.next()
    }
}

pub fn read(text: &str) -> Result<Design, ReadError> {
    let mut it = Lines { raw: text.lines(), n: 0 };
    let bad = |what: &str, n: usize| ReadError(format!("line {n}: {what}"));
    match it.next() {
        Some(h) if h.trim() == format!("riv {FORMAT_VERSION}") => {}
        Some(h) => {
            return Err(ReadError(format!(
                "bad header `{h}` (expected `riv {FORMAT_VERSION}`)"
            )))
        }
        None => return Err(ReadError("empty design".into())),
    }
    let mut d = Design {
        meta: Meta { top: String::new(), timescale: String::new(), precision: String::new(), unit: String::new(), sources: Vec::new() },
        vars: Vec::new(),
        arrays: Vec::new(),
        assigns: Vec::new(),
        gates: Vec::new(),
        threads: Vec::new(),
        funcs: Vec::new(),
        strs: Vec::new(),
    };
    // meta
    expect(&mut it, "meta", &mut |l, n| bad(l, n))?;
    let mut sources = 0usize;
    loop {
        let Some(l) = it.next() else { return Err(bad("unterminated meta", it.n)) };
        let t: Vec<&str> = fields(l);
        match t[0] {
            "top" => d.meta.top = unescape(t.get(1).copied().unwrap_or("")),
            "timescale" => d.meta.timescale = unescape(t.get(1).copied().unwrap_or("")),
            "unit" => d.meta.unit = unescape(t.get(1).copied().unwrap_or("")),
            "precision" => d.meta.precision = unescape(t.get(1).copied().unwrap_or("")),
            "sources" => sources = t.get(1).and_then(|s| s.parse().ok()).unwrap_or(0),
            "src" => d.meta.sources.push(unescape(t.get(1).copied().unwrap_or(""))),
            "end" => break,
            other => return Err(bad(&format!("unknown meta key `{other}`"), it.n)),
        }
    }
    if d.meta.sources.len() != sources {
        return Err(bad("source count mismatch", it.n));
    }
    // vars
    expect(&mut it, "vars", &mut |l, n| bad(l, n))?;
    loop {
        let Some(l) = it.next() else { return Err(bad("unterminated vars", it.n)) };
        if l == "end" {
            break;
        }
        let t: Vec<&str> = fields(l);
        if t[0] != "var" {
            return Err(bad(&format!("expected var line, got `{l}`"), it.n));
        }
        if t.len() != 9 {
            return Err(bad("var line needs 9 fields", it.n));
        }
        let kind = match t[2] {
            "reg" => VarKind::Reg,
            "net" => VarKind::Net,
            "array" => VarKind::Array,
            "event" => VarKind::Event,
            other => return Err(bad(&format!("unknown var kind `{other}`"), it.n)),
        };
        let init = if t[6] == "-" {
            None
        } else {
            Some(Literal::decode(
                t[3].parse().map_err(|_| bad("bad width", it.n))?,
                t[4] == "1",
                t[6],
            )
            .ok_or_else(|| bad(&format!("bad literal `{}`", t[6]), it.n))?)
        };
        d.vars.push(Var {
            id: t[1].parse().map_err(|_| bad("bad var id", it.n))?,
            name: unescape(t[5]),
            width: t[3].parse().map_err(|_| bad("bad width", it.n))?,
            signed: t[4] == "1",
            kind,
            init,
            msb: t[7].parse().map_err(|_| bad("bad msb", it.n))?,
            lsb: t[8].parse().map_err(|_| bad("bad lsb", it.n))?,
        });
    }
    // arrays
    expect(&mut it, "arrays", &mut |l, n| bad(l, n))?;
    loop {
        let Some(l) = it.next() else { return Err(bad("unterminated arrays", it.n)) };
        if l == "end" {
            break;
        }
        let t: Vec<&str> = fields(l);
        if t[0] != "arr" {
            return Err(bad("expected arr line", it.n));
        }
        let ndims: usize = t.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        if t.len() != 3 + ndims * 2 {
            return Err(bad("arr dimension count mismatch", it.n));
        }
        let mut dims = Vec::new();
        for i in 0..ndims {
            let m: u32 = t[3 + i * 2].parse().map_err(|_| bad("bad msb", it.n))?;
            let lo: u32 = t[4 + i * 2].parse().map_err(|_| bad("bad lsb", it.n))?;
            dims.push((m, lo));
        }
        d.arrays.push(ArrayDim {
            var: t[1].parse().map_err(|_| bad("bad arr var", it.n))?,
            dims,
        });
    }
    // assigns
    expect(&mut it, "assigns", &mut |l, n| bad(l, n))?;
    loop {
        let Some(l) = it.next() else { return Err(bad("unterminated assigns", it.n)) };
        if l == "end" {
            break;
        }
        let t: Vec<&str> = fields(l);
        if t[0] != "assign" || t.len() != 3 {
            return Err(bad("expected assign line", it.n));
        }
        let th: u32 = t[1].parse().map_err(|_| bad("bad thread idx", it.n))?;
        let v: u32 = t[2].parse().map_err(|_| bad("bad assign var", it.n))?;
        d.assigns.push((th, v));
    }
    // gates
    expect(&mut it, "gates", &mut |l, n| bad(l, n))?;
    loop {
        let Some(l) = it.next() else { return Err(bad("unterminated gates", it.n)) };
        if l == "end" {
            break;
        }
        let t: Vec<&str> = fields(l);
        // `gate <kind> <out> <nin> <rise> <fall> <turnoff> <in...>`
        if t[0] != "gate" || t.len() < 8 {
            return Err(bad("expected gate line", it.n));
        }
        let kind = GateKind::from_tag(t[1]).ok_or_else(|| bad(&format!("unknown gate kind `{}`", t[1]), it.n))?;
        let nin: usize = t[3].parse().map_err(|_| bad("bad gate arity", it.n))?;
        if t.len() != 7 + nin {
            return Err(bad("gate arity mismatch", it.n));
        }
        d.gates.push(Gate {
            kind,
            out: t[2].parse().map_err(|_| bad("bad gate out", it.n))?,
            inputs: (0..nin)
                .map(|i| t[7 + i].parse::<u32>().map_err(|_| bad("bad gate input", it.n)))
                .collect::<Result<_, _>>()?,
            rise: t[4].parse().map_err(|_| bad("bad gate rise", it.n))?,
            fall: t[5].parse().map_err(|_| bad("bad gate fall", it.n))?,
            turnoff: t[6].parse().map_err(|_| bad("bad gate turnoff", it.n))?,
        });
    }
    // threads
    expect(&mut it, "threads", &mut |l, n| bad(l, n))?;
    loop {
        let Some(l) = it.next() else { return Err(bad("unterminated threads", it.n)) };
        if l == "end" {
            break;
        }
        let t: Vec<&str> = fields(l);
        if t[0] != "thread" || t.len() != 4 {
            return Err(bad("expected thread line", it.n));
        }
        let kind = ThreadKind::from_tag(t[1]).ok_or_else(|| bad(&format!("unknown thread kind `{}`", t[1]), it.n))?;
        let frame_len: u32 = t[2].parse().map_err(|_| bad("bad frame_len", it.n))?;
        let nsens: usize = t[3].parse().map_err(|_| bad("bad sens count", it.n))?;
        let mut sens = Vec::new();
        for _ in 0..nsens {
            let Some(sl) = it.next() else { return Err(bad("unterminated sens", it.n)) };
            let st: Vec<&str> = fields(sl);
            if st[0] != "sens" || st.len() != 3 {
                return Err(bad("expected sens line", it.n));
            }
            sens.push(SensItem {
                var: st[1].parse().map_err(|_| bad("bad sens var", it.n))?,
                edge: EdgeKind::from_tag(st[2]).ok_or_else(|| bad(&format!("unknown edge `{}`", st[2]), it.n))?,
            });
        }
        let body = read_ops(&mut it, "endthread")?;
        d.threads.push(Thread { kind, sens, frame_len, body });
    }
    // funcs
    expect(&mut it, "funcs", &mut |l, n| bad(l, n))?;
    loop {
        let Some(l) = it.next() else { return Err(bad("unterminated funcs", it.n)) };
        if l == "end" {
            break;
        }
        let t: Vec<&str> = fields(l);
        if t[0] != "func" || !(t.len() == 6 || t.len() == 7) {
            return Err(bad("expected func line", it.n));
        }
        let is_task = match t[2] {
            "task" => true,
            "function" => false,
            other => return Err(bad(&format!("bad func kind `{other}`"), it.n)),
        };
        let ret = if t[5] == "-" {
            if t.len() != 6 {
                return Err(bad("ret width with stray signed flag", it.n));
            }
            None
        } else {
            if t.len() != 7 {
                return Err(bad("ret missing signed flag", it.n));
            }
            Some((
                t[5].parse().map_err(|_| bad("bad ret width", it.n))?,
                t[6] == "1",
            ))
        };
        let body = read_ops(&mut it, "endfunc")?;
        d.funcs.push(Func {
            name: unescape(t[1]),
            is_task,
            automatic: t[3] == "1",
            frame_len: t[4].parse().map_err(|_| bad("bad func frame_len", it.n))?,
            ret,
            body,
        });
    }
    // strs: per-entry header (`strs <idx> <count>`), count piece lines,
    // `str end` terminator; entries until the section `end`.
    expect(&mut it, "strs", &mut |l, n| bad(l, n))?;
    loop {
        let Some(l) = it.next() else { return Err(bad("unterminated strs", it.n)) };
        if l == "end" {
            break;
        }
        let t: Vec<&str> = fields(l);
        if t[0] != "strs" || t.len() != 3 {
            return Err(bad("expected strs header", it.n));
        }
        let idx: usize = t[1].parse().map_err(|_| bad("bad strs idx", it.n))?;
        let npieces: usize = t[2].parse().map_err(|_| bad("bad strs count", it.n))?;
        let mut pieces = Vec::new();
        for _ in 0..npieces {
            let Some(l) = it.next() else { return Err(bad("unterminated str pieces", it.n)) };
            let t: Vec<&str> = fields(l);
            match t.get(0..2) {
                Some(["str", "text"]) => {
                    pieces.push(FmtPiece::Text(unescape(t.get(2).copied().unwrap_or(""))));
                }
                Some(["str", "spec"]) => {
                    if t.len() != 6 {
                        return Err(bad("spec piece needs 6 fields", it.n));
                    }
                    let ch = t[2].chars().next().ok_or_else(|| bad("bad spec char", it.n))?;
                    pieces.push(FmtPiece::Spec {
                        ch,
                        width: t[3].parse().map_err(|_| bad("bad spec width", it.n))?,
                        zero_pad: t[4] == "1",
                        arg: t[5].parse().map_err(|_| bad("bad spec arg", it.n))?,
                    });
                }
                _ => return Err(bad(&format!("expected str piece, got `{l}`"), it.n)),
            }
        }
        match it.next() {
            Some("str end") => {}
            Some(l) => return Err(bad(&format!("expected `str end`, got `{l}`"), it.n)),
            None => return Err(bad("missing `str end` terminator", it.n)),
        }
        if d.strs.len() != idx {
            return Err(bad(&format!("strs index gap (got {}, expected {idx})", d.strs.len()), it.n));
        }
        d.strs.push(pieces);
    }
    match it.next() {
        Some("riv end") => {}
        Some(l) => return Err(bad(&format!("expected `riv end`, got `{l}`"), it.n)),
        None => return Err(bad("missing `riv end` trailer", it.n)),
    }
    Ok(d)
}

fn expect<'a>(
    it: &mut Lines<'a>,
    want: &str,
    err: &mut dyn FnMut(&str, usize) -> ReadError,
) -> Result<(), ReadError> {
    match it.next() {
        Some(l) if l == want => Ok(()),
        Some(l) => Err(err(&format!("expected `{want}`, got `{l}`"), it.n)),
        None => Err(err(&format!("expected `{want}`, found EOF"), it.n)),
    }
}

fn read_ops<'a>(it: &mut Lines<'a>, terminator: &str) -> Result<Vec<Op>, ReadError> {
    let mut ops = Vec::new();
    loop {
        let Some(l) = it.next() else {
            return Err(ReadError(format!("unterminated thread body (expected {terminator})")));
        };
        if l == terminator {
            return Ok(ops);
        }
        let toks: Vec<&str> = fields(l);
        let pc: u32 = toks[0]
            .parse()
            .map_err(|_| ReadError(format!("line {}: bad pc `{}`", it.n, toks[0])))?;
        if pc != ops.len() as u32 {
            return Err(ReadError(format!(
                "line {}: pc {pc} out of order (expected {})",
                it.n,
                ops.len()
            )));
        }
        ops.push(op_decode(&toks[1..], it.n)?);
    }
}

fn p<'a>(t: &'a [&'a str], i: usize, n: usize) -> Result<&'a str, ReadError> {
    t.get(i)
        .copied()
        .ok_or_else(|| ReadError(format!("line {n}: operand {i} missing")))
}

fn pu(t: &[&str], i: usize, n: usize) -> Result<u32, ReadError> {
    let tok = p(t, i, n)?;
    tok.parse().map_err(|_| ReadError(format!("line {n}: bad number `{tok}`")))
}

fn op_decode(t: &[&str], n: usize) -> Result<Op, ReadError> {
    let e = |m: &str| ReadError(format!("line {n}: {m}"));
    let b = |s: &str| s == "1";
    match p(t, 0, n)? {
        "const" => {
            let lit = Literal::decode(pu(t, 2, n)?, b(p(t, 3, n)?), p(t, 4, n)?)
                .ok_or_else(|| e("bad const literal"))?;
            Ok(Op::Const { dst: pu(t, 1, n)?, lit })
        }
        "mov" => Ok(Op::Mov { dst: pu(t, 1, n)?, src: pu(t, 2, n)? }),
        "bin" => Ok(Op::Bin {
            kind: BinKind::from_tag(p(t, 1, n)?).ok_or_else(|| e("bad bin kind"))?,
            dst: pu(t, 2, n)?,
            a: pu(t, 3, n)?,
            b: pu(t, 4, n)?,
            w: pu(t, 5, n)?,
            signed: b(p(t, 6, n)?),
        }),
        "bit" => Ok(Op::Bit {
            kind: BitKind::from_tag(p(t, 1, n)?).ok_or_else(|| e("bad bit kind"))?,
            dst: pu(t, 2, n)?,
            a: pu(t, 3, n)?,
            b: pu(t, 4, n)?,
            w: pu(t, 5, n)?,
        }),
        "bitxnor" => Ok(Op::BitXnor { dst: pu(t, 1, n)?, a: pu(t, 2, n)?, b: pu(t, 3, n)?, w: pu(t, 4, n)? }),
        "bitnot" => Ok(Op::BitNot { dst: pu(t, 1, n)?, a: pu(t, 2, n)?, w: pu(t, 3, n)? }),
        "reduce" => Ok(Op::Reduce {
            kind: RedKind::from_tag(p(t, 1, n)?).ok_or_else(|| e("bad reduce kind"))?,
            dst: pu(t, 2, n)?,
            a: pu(t, 3, n)?,
        }),
        "logicand" => Ok(Op::LogicAnd { dst: pu(t, 1, n)?, a: pu(t, 2, n)?, b: pu(t, 3, n)? }),
        "logicor" => Ok(Op::LogicOr { dst: pu(t, 1, n)?, a: pu(t, 2, n)?, b: pu(t, 3, n)? }),
        "logicnot" => Ok(Op::LogicNot { dst: pu(t, 1, n)?, a: pu(t, 2, n)? }),
        "cmp" => Ok(Op::Cmp {
            kind: CmpKind::from_tag(p(t, 1, n)?).ok_or_else(|| e("bad cmp kind"))?,
            dst: pu(t, 2, n)?,
            a: pu(t, 3, n)?,
            b: pu(t, 4, n)?,
            signed: b(p(t, 5, n)?),
        }),
        "casecmp" => Ok(Op::CaseCmp {
            kind: CaseCmpKind::from_tag(p(t, 1, n)?).ok_or_else(|| e("bad casecmp kind"))?,
            dst: pu(t, 2, n)?,
            a: pu(t, 3, n)?,
            b: pu(t, 4, n)?,
        }),
        "shift" => Ok(Op::Shift {
            kind: ShiftKind::from_tag(p(t, 1, n)?).ok_or_else(|| e("bad shift kind"))?,
            dst: pu(t, 2, n)?,
            a: pu(t, 3, n)?,
            b: pu(t, 4, n)?,
            w: pu(t, 5, n)?,
            signed: b(p(t, 6, n)?),
        }),
        "sel" => Ok(Op::Sel { dst: pu(t, 1, n)?, cond: pu(t, 2, n)?, a: pu(t, 3, n)?, b: pu(t, 4, n)? }),
        "concat" => {
            let nn = pu(t, 2, n)? as usize;
            if t.len() != 3 + nn {
                return Err(e("concat arity mismatch"));
            }
            Ok(Op::Concat {
                dst: pu(t, 1, n)?,
                parts: (0..nn).map(|i| pu(t, 3 + i, n)).collect::<Result<_, _>>()?,
            })
        }
        "replicate" => Ok(Op::Replicate { dst: pu(t, 1, n)?, src: pu(t, 2, n)?, times: pu(t, 3, n)? as u64 }),
        "loadvar" => Ok(Op::LoadVar { dst: pu(t, 1, n)?, var: pu(t, 2, n)? }),
        "loadbit" => Ok(Op::LoadBit { dst: pu(t, 1, n)?, var: pu(t, 2, n)?, idx: pu(t, 3, n)? }),
        "loadpart" => Ok(Op::LoadPart { dst: pu(t, 1, n)?, var: pu(t, 2, n)?, hi: pu(t, 3, n)?, lo: pu(t, 4, n)? }),
        "loadpartdyn" => Ok(Op::LoadPartDyn { dst: pu(t, 1, n)?, var: pu(t, 2, n)?, hi: pu(t, 3, n)?, lo: pu(t, 4, n)? }),
        "loadelem" => Ok(Op::LoadElem { dst: pu(t, 1, n)?, var: pu(t, 2, n)?, idx: pu(t, 3, n)? }),
        "assignblk" => Ok(Op::AssignBlk { lv: lval_decode(&t[1..t.len() - 1], n)?, src: pu(t, t.len() - 1, n)? }),
        "assignnba" => Ok(Op::AssignNba { lv: lval_decode(&t[1..t.len() - 1], n)?, src: pu(t, t.len() - 1, n)? }),
        "assignnbadelay" => Ok(Op::AssignNbaDelay {
            lv: lval_decode(&t[1..t.len() - 2], n)?,
            src: pu(t, t.len() - 2, n)?,
            amount: pu(t, t.len() - 1, n)?,
        }),
        "branch" => Ok(Op::Branch { cond: pu(t, 1, n)?, then: pu(t, 2, n)?, els: pu(t, 3, n)? }),
        "jump" => Ok(Op::Jump { to: pu(t, 1, n)? }),
        "delay" => Ok(Op::Delay { amount: pu(t, 1, n)? }),
        "scaletime" => Ok(Op::ScaleTime {
            dst: pu(t, 1, n)?,
            src: pu(t, 2, n)?,
            num: pu(t, 3, n)? as u64,
            den: pu(t, 4, n)? as u64,
        }),
        "waitedge" => {
            let nn = pu(t, 1, n)? as usize;
            if t.len() != 2 + nn * 2 {
                return Err(e("waitedge arity mismatch"));
            }
            let mut items = Vec::new();
            for i in 0..nn {
                items.push(SensItem {
                    var: pu(t, 2 + i * 2, n)?,
                    edge: EdgeKind::from_tag(p(t, 3 + i * 2, n)?).ok_or_else(|| e("bad edge kind"))?,
                });
            }
            Ok(Op::WaitEdge { items })
        }
        "syscall" => {
            let tag0 = p(t, 1, n)?;
            let id = SysId::from_tag(tag0).ok_or_else(|| e(&format!("unknown syscall `{tag0}`")))?;
            let fmt = match p(t, 2, n)? {
                "-" => None,
                f => Some(f.parse().map_err(|_| e("bad fmt idx"))?),
            };
            let na = pu(t, 3, n)? as usize;
            let mut args = Vec::new();
            let mut i = 4usize;
            for _ in 0..na {
                args.push(Arg {
                    slot: pu(t, i, n)?,
                    width: pu(t, i + 1, n)?,
                    signed: b(p(t, i + 2, n)?),
                    is_time: b(p(t, i + 3, n)?),
                });
                i += 4;
            }
            let dst = match p(t, i, n)? {
                "-" => None,
                dd => Some(dd.parse().map_err(|_| e("bad syscall dst"))?),
            };
            Ok(Op::SysCall {
                id,
                fmt,
                args,
                dst,
                file: pu(t, i + 1, n)?,
                line: pu(t, i + 2, n)?,
                start: pu(t, i + 3, n)?,
            })
        }
        "assignforce" => Ok(Op::AssignForce {
            lv: lval_decode(&t[1..t.len() - 1], n)?,
            src: pu(t, t.len() - 1, n)?,
        }),
        "release" => Ok(Op::Release { lv: lval_decode(&t[1..], n)? }),
        "fork" => {
            // encoded `fork <nb> <join> <branches...>`
            let nb = pu(t, 1, n)? as usize;
            if t.len() != 3 + nb {
                return Err(e("fork arity mismatch"));
            }
            Ok(Op::Fork {
                join: pu(t, 2, n)?,
                branches: (0..nb).map(|i| pu(t, 3 + i, n)).collect::<Result<_, _>>()?,
            })
        }
        "joindone" => Ok(Op::JoinDone),
        "joinpoint" => Ok(Op::JoinPoint),
        "call" => {
            let func = pu(t, 1, n)?;
            let nin = pu(t, 2, n)? as usize;
            let mut i = 3usize;
            let mut ins = Vec::new();
            for _ in 0..nin {
                ins.push((pu(t, i, n)?, pu(t, i + 1, n)?));
                i += 2;
            }
            let nout = pu(t, i, n)? as usize;
            i += 1;
            let mut outs = Vec::new();
            for _ in 0..nout {
                let sub_len = match p(t, i, n)? {
                    "var" => 2,
                    "bit" | "elem" => 3,
                    "part" | "partdyn" => 4,
                    other => return Err(e(&format!("bad call out lvalue `{other}`"))),
                };
                outs.push((
                    lval_decode(&t[i..i + sub_len], n)?,
                    pu(t, i + sub_len, n)?,
                ));
                i += sub_len + 1;
            }
            let dst = match p(t, i, n)? {
                "-" => None,
                dd => Some(dd.parse().map_err(|_| e("bad call dst"))?),
            };
            Ok(Op::Call { func, ins, outs, dst })
        }
        "ret" => Ok(Op::Ret { src: pu(t, 1, n)? }),
        "endthread" => Ok(Op::EndThread),
        other => Err(e(&format!("unknown opcode `{other}`"))),
    }
}

fn lval_decode(t: &[&str], n: usize) -> Result<Lval, ReadError> {
    let e = |m: &str| ReadError(format!("line {n}: lvalue: {m}"));
    if t.is_empty() {
        return Err(e("empty"));
    }
    match t[0] {
        "var" => Ok(Lval::Var { var: pu(t, 1, n)? }),
        "bit" => Ok(Lval::Bit { var: pu(t, 1, n)?, idx: pu(t, 2, n)? }),
        "part" => Ok(Lval::Part { var: pu(t, 1, n)?, hi: pu(t, 2, n)?, lo: pu(t, 3, n)? }),
        "partdyn" => Ok(Lval::PartDyn { var: pu(t, 1, n)?, hi: pu(t, 2, n)?, lo: pu(t, 3, n)? }),
        "elem" => Ok(Lval::Elem { var: pu(t, 1, n)?, idx: pu(t, 2, n)? }),
        "concat" => {
            let nn: usize = pu(t, 1, n)? as usize;
            let mut rest = &t[2..];
            let mut parts = Vec::new();
            for _ in 0..nn {
                if rest.is_empty() {
                    return Err(e("concat truncated"));
                }
                let sub_len = match rest[0] {
                    "var" => 2,
                    "bit" | "elem" => 3,
                    "part" => 4,
                    "partdyn" => 4,
                    "concat" => return Err(e("nested concat must be flattened at lowering")),
                    other => return Err(e(&format!("bad lvalue form `{other}`"))),
                };
                if rest.len() < sub_len + 1 {
                    return Err(e("concat part truncated"));
                }
                parts.push((lval_decode(&rest[..sub_len], n)?, pu(rest, sub_len, n)?));
                rest = &rest[sub_len + 1..];
            }
            if !rest.is_empty() {
                return Err(e("concat trailing tokens"));
            }
            Ok(Lval::Concat { parts })
        }
        other => Err(e(&format!("bad lvalue form `{other}`"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample() -> Design {
        Design {
            meta: Meta {
                top: "tb".into(),
                timescale: "1ns/1ps".into(),
                unit: "1ns".into(),
                precision: "1ps".into(),
                sources: vec!["dir with space/tb.v".into(), "tb2.v".into()],
            },
            vars: vec![
                Var { id: 0, name: "a b".into(), width: 8, signed: false, kind: VarKind::Reg, init: Some(Literal { width: 8, signed: false, base: Base::Hex, digits: "1z".into() }), msb: 7, lsb: 0 },
                Var { id: 1, name: "sum9".into(), width: 9, signed: false, kind: VarKind::Net, init: None, msb: 8, lsb: 0 },
                Var { id: 2, name: "ev".into(), width: 1, signed: false, kind: VarKind::Event, init: None, msb: 0, lsb: 0 },
            ],
            arrays: vec![ArrayDim { var: 0, dims: vec![(255, 0), (3, 0)] }],
            assigns: vec![(3, 1)],
            gates: vec![
                Gate { kind: GateKind::Nand, out: 1, inputs: vec![0, 0], rise: 6000, fall: 6000, turnoff: 0 },
                Gate { kind: GateKind::Bufif1, out: 1, inputs: vec![0, 1], rise: 0, fall: 1, turnoff: 2 },
            ],
            threads: vec![
                Thread {
                    kind: ThreadKind::Initial,
                    sens: vec![],
                    frame_len: 4,
                    body: vec![
                        Op::Const { dst: 0, lit: Literal { width: 32, signed: true, base: Base::Dec, digits: "300".into() } },
                        Op::Mov { dst: 1, src: 0 },
                        Op::Bin { kind: BinKind::Add, dst: 2, a: 0, b: 1, w: 9, signed: false },
                        Op::Bin { kind: BinKind::Pow, dst: 2, a: 0, b: 1, w: 9, signed: true },
                        Op::Bit { kind: BitKind::Xor, dst: 2, a: 0, b: 1, w: 9 },
                        Op::BitXnor { dst: 2, a: 0, b: 1, w: 9 },
                        Op::BitNot { dst: 2, a: 0, w: 9 },
                        Op::Reduce { kind: RedKind::Xnor, dst: 2, a: 0 },
                        Op::Reduce { kind: RedKind::Nand, dst: 2, a: 0 },
                        Op::LogicAnd { dst: 2, a: 0, b: 1 },
                        Op::LogicOr { dst: 2, a: 0, b: 1 },
                        Op::LogicNot { dst: 2, a: 0 },
                        Op::Cmp { kind: CmpKind::Le, dst: 2, a: 0, b: 1, signed: true },
                        Op::CaseCmp { kind: CaseCmpKind::ExactNe, dst: 2, a: 0, b: 1 },
                        Op::CaseCmp { kind: CaseCmpKind::WildZ, dst: 2, a: 0, b: 1 },
                        Op::CaseCmp { kind: CaseCmpKind::WildX, dst: 2, a: 0, b: 1 },
                        Op::Shift { kind: ShiftKind::Ashr, dst: 2, a: 0, b: 1, w: 9, signed: true },
                        Op::Shift { kind: ShiftKind::Shl, dst: 2, a: 0, b: 1, w: 9, signed: false },
                        Op::Sel { dst: 2, cond: 0, a: 0, b: 1 },
                        Op::Concat { dst: 2, parts: vec![0, 1, 0] },
                        Op::Replicate { dst: 2, src: 0, times: 5 },
                        Op::LoadVar { dst: 3, var: 0 },
                        Op::LoadBit { dst: 3, var: 0, idx: 0 },
                        Op::LoadPart { dst: 3, var: 0, hi: 7, lo: 4 },
                        Op::LoadPartDyn { dst: 3, var: 0, hi: 1, lo: 0 },
                        Op::LoadElem { dst: 3, var: 0, idx: 2 },
                        Op::AssignBlk { lv: Lval::Var { var: 0 }, src: 3 },
                        Op::AssignNba { lv: Lval::Bit { var: 0, idx: 1 }, src: 3 },
                        Op::AssignNbaDelay { lv: Lval::Var { var: 0 }, src: 3, amount: 2 },
                        Op::AssignNba { lv: Lval::Elem { var: 0, idx: 2 }, src: 3 },
                        Op::SysCall { id: SysId::Trigger, fmt: None, args: vec![], dst: None, file: 0, line: 7, start: 0 },
                        Op::SysCall { id: SysId::Stop, fmt: None, args: vec![], dst: None, file: 0, line: 8, start: 0 },
                        Op::SysCall { id: SysId::ReadmemH, fmt: Some(1), args: vec![Arg { slot: 0, width: 32, signed: false, is_time: false }, Arg { slot: 1, width: 0, signed: false, is_time: false }], dst: None, file: 0, line: 9, start: 0 },
                        Op::SysCall { id: SysId::Fopen, fmt: Some(1), args: vec![Arg { slot: 0, width: 1, signed: false, is_time: false }], dst: Some(1), file: 0, line: 10, start: 0 },
                        Op::SysCall { id: SysId::Fwrite, fmt: Some(0), args: vec![Arg { slot: 1, width: 32, signed: true, is_time: false }], dst: None, file: 0, line: 11, start: 0 },
                        Op::SysCall { id: SysId::Fclose, fmt: None, args: vec![Arg { slot: 1, width: 32, signed: true, is_time: false }], dst: None, file: 0, line: 12, start: 0 },
                        Op::SysCall { id: SysId::Dumpall, fmt: None, args: vec![], dst: None, file: 0, line: 13, start: 0 },
                        Op::SysCall { id: SysId::Random, fmt: None, args: vec![Arg { slot: 2, width: 32, signed: true, is_time: false }], dst: Some(3), file: 0, line: 14, start: 0 },
                        Op::AssignBlk { lv: Lval::Part { var: 0, hi: 7, lo: 4 }, src: 3 },
                        Op::AssignNba { lv: Lval::PartDyn { var: 0, hi: 1, lo: 0 }, src: 3 },
                        Op::AssignBlk {
                            lv: Lval::Concat { parts: vec![(Lval::Var { var: 0 }, 1), (Lval::Part { var: 1, hi: 8, lo: 5 }, 4)] },
                            src: 3,
                        },
                        Op::Branch { cond: 0, then: 21, els: 22 },
                        Op::Jump { to: 23 },
                        Op::Delay { amount: 3 },
                        Op::ScaleTime { dst: 3, src: 0, num: 1000, den: 1 },
                        Op::WaitEdge { items: vec![SensItem { var: 0, edge: EdgeKind::Pos }, SensItem { var: 1, edge: EdgeKind::Any }] },
                        Op::SysCall {
                            id: SysId::Display,
                            fmt: Some(0),
                            args: vec![Arg { slot: 0, width: 8, signed: true, is_time: false }, Arg { slot: 1, width: 9, signed: false, is_time: true }],
                            dst: None,
                            file: 0,
                            line: 42,
                            start: 0,
                        },
                        Op::Call { func: 0, ins: vec![(0, 1), (1, 2)], outs: vec![(Lval::Bit { var: 0, idx: 1 }, 3)], dst: Some(2) },
                        Op::Ret { src: 2 },
                        Op::Fork { branches: vec![5, 9], join: 12 },
                        Op::JoinDone,
                        Op::JoinPoint,
                        Op::AssignForce { lv: Lval::Part { var: 1, hi: 8, lo: 5 }, src: 3 },
                        Op::Release { lv: Lval::Var { var: 0 } },
                        Op::EndThread,
                    ],
                },
                Thread {
                    kind: ThreadKind::Always,
                    sens: vec![SensItem { var: 0, edge: EdgeKind::Neg }],
                    frame_len: 0,
                    body: vec![Op::EndThread],
                },
                Thread {
                    kind: ThreadKind::ContAssign,
                    sens: vec![],
                    frame_len: 1,
                    body: vec![Op::EndThread],
                },
            ],
            funcs: vec![Func {
                name: "f n".into(),
                is_task: false,
                automatic: true,
                frame_len: 4,
                ret: Some((8, true)),
                body: vec![Op::Ret { src: 0 }],
            }],
            strs: vec![
                vec![
                    FmtPiece::Text("a=\t\\ ".into()),
                    FmtPiece::Spec { ch: 'd', width: 5, zero_pad: true, arg: 0 },
                    FmtPiece::Spec { ch: 'h', width: 0, zero_pad: false, arg: 1 },
                ],
                vec![],
            ],
        }
    }

    #[test]
    fn roundtrip_is_byte_identical() {
        let d = sample();
        let text1 = d.write();
        let back = read(&text1).unwrap_or_else(|e| panic!("read: {e}"));
        assert_eq!(back, d, "model equality after roundtrip");
        let text2 = back.write();
        assert_eq!(text1, text2, "writer->reader->writer must be byte-identical");
    }

    #[test]
    fn reader_rejects_bad_header_and_truncation() {
        let d = sample();
        let full = d.write();
        assert!(read("riv 2\n").is_err(), "future version must be rejected");
        assert!(read("").is_err());
        assert!(read(&full[..full.len() - 9]).is_err(), "truncated trailer");
        // Missing the final newline variant still parses (lines-based).
        assert!(read(full.trim_end()).is_ok());
    }

    #[test]
    fn reader_rejects_unknown_opcode_and_syscall() {
        let d = sample();
        let text = d.write().replace(" endthread\n", " frobnicate\n");
        assert!(read(&text).is_err());
        let text = d.write().replace("syscall display", "syscall frob");
        assert!(read(&text).is_err());
    }

    #[test]
    fn reader_rejects_pc_gap() {
        let d = sample();
        let text = d.write();
        let text = text.replace("2 bin", "7 bin");
        assert!(read(&text).is_err());
    }

    #[test]
    fn literal_encoding_roundtrip() {
        for (w, s, base, digits) in [
            (8u32, false, Base::Hex, "ff".to_string()),
            (4, false, Base::Bin, "1z?x".to_string()),
            (32, true, Base::Dec, "42".to_string()),
            (6, false, Base::Oct, "52".to_string()),
        ] {
            let l = Literal { width: w, signed: s, base, digits: digits.clone() };
            let dec = Literal::decode(w, s, &l.encode()).unwrap();
            assert_eq!(dec, l);
        }
        assert!(Literal::decode(8, false, "qff").is_none());
        assert!(Literal::decode(8, false, "h").is_none());
    }

    #[test]
    fn escape_roundtrip_and_format_stability() {
        assert_eq!(unescape(&escape("a b\\c\td\ne\r")), "a b\\c\td\ne\r");
        // The header + first section of a design is pinned byte-exact so
        // accidental format drift is caught by this shape.
        let d = sample();
        let text = d.write();
        assert!(text.starts_with("riv 4\nmeta\ntop tb\ntimescale 1ns/1ps\nunit 1ns\nprecision 1ps\nsources 2\nsrc dir\\ with\\ space/tb.v\nsrc tb2.v\nend\n"));
        assert!(text.ends_with("riv end\n"));
    }
}

