//! Verilog AST (v0: Verilog-2005 subset, see PLAN §4).

use rcs_diag::Span;

#[derive(Clone, Debug, PartialEq)]
pub struct SourceFile {
    pub modules: Vec<Module>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub span: Span,
    pub name: String,
    /// Header parameter list `#(...)`.
    pub params: Vec<ParamEntry>,
    pub ports: Vec<Port>,
    /// True when the header used ANSI-style port declarations.
    pub ansi: bool,
    pub items: Vec<ModuleItem>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
    Inout,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Input => "input",
            Direction::Output => "output",
            Direction::Inout => "inout",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetKind {
    Wire,
    Uwire,
    Tri,
    Tri0,
    Tri1,
    Triand,
    Trior,
    Trireg,
    Wand,
    Wor,
    Supply0,
    Supply1,
}

impl NetKind {
    pub fn from_kw(k: crate::token::Kw) -> Option<NetKind> {
        use crate::token::Kw;
        Some(match k {
            Kw::Wire => NetKind::Wire,
            Kw::Uwire => NetKind::Uwire,
            Kw::Tri => NetKind::Tri,
            Kw::Tri0 => NetKind::Tri0,
            Kw::Tri1 => NetKind::Tri1,
            Kw::Triand => NetKind::Triand,
            Kw::Trior => NetKind::Trior,
            Kw::Trireg => NetKind::Trireg,
            Kw::Wand => NetKind::Wand,
            Kw::Wor => NetKind::Wor,
            Kw::Supply0 => NetKind::Supply0,
            Kw::Supply1 => NetKind::Supply1,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            NetKind::Wire => "wire",
            NetKind::Uwire => "uwire",
            NetKind::Tri => "tri",
            NetKind::Tri0 => "tri0",
            NetKind::Tri1 => "tri1",
            NetKind::Triand => "triand",
            NetKind::Trior => "trior",
            NetKind::Trireg => "trireg",
            NetKind::Wand => "wand",
            NetKind::Wor => "wor",
            NetKind::Supply0 => "supply0",
            NetKind::Supply1 => "supply1",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Port {
    pub span: Span,
    pub name: String,
    /// ANSI header info (None entries inherit the previous port's).
    pub dir: Option<Direction>,
    pub net: Option<NetKind>,
    pub is_reg: bool,
    pub signed: bool,
    pub range: Option<Range>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Range {
    pub msb: Expr,
    pub lsb: Expr,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParamEntry {
    pub span: Span,
    pub name: String,
    /// `parameter integer P = ...` style type keyword, if given.
    pub ty: Option<ParamType>,
    pub signed: bool,
    pub range: Option<Range>,
    pub init: Expr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamType {
    Integer,
    Real,
    Time,
    Realtime,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ModuleItem {
    Decl(Decl),
    ContAssign(Vec<AssignPair>),
    Always { span: Span, ctrl: Option<EventCtrl>, body: Stmt },
    Initial { span: Span, body: Stmt },
    Instance(Instance),
    Defparam(Vec<AssignPair>),
    GenerateRegion(Vec<ModuleItem>),
    GenIf {
        span: Span,
        cond: Expr,
        then: Vec<ModuleItem>,
        els: Option<Vec<ModuleItem>>,
    },
    GenFor {
        span: Span,
        var: String,
        init: Expr,
        cond: Expr,
        step: AssignPair,
        block: Option<String>,
        items: Vec<ModuleItem>,
    },
    GenCase {
        span: Span,
        expr: Expr,
        items: Vec<(Vec<Expr>, Vec<ModuleItem>)>,
        default: Option<Vec<ModuleItem>>,
    },
    /// Named (or bare) `begin : name ... end` block inside a generate scope.
    GenBlock {
        span: Span,
        name: Option<String>,
        items: Vec<ModuleItem>,
    },
    Function(Box<Function>),
    Task(Box<Function>),
    Empty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclKind {
    Net(NetKind),
    Reg,
    Integer,
    Real,
    Realtime,
    Time,
    Genvar,
    Event,
    Parameter,
    Localparam,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Decl {
    pub span: Span,
    pub kind: DeclKind,
    /// `parameter integer P = ...` style type keyword.
    pub ty: Option<ParamType>,
    pub signed: bool,
    pub range: Option<Range>,
    /// Direction when this is a non-ANSI `input/output/inout` declaration.
    pub dir: Option<Direction>,
    pub entries: Vec<DeclEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeclEntry {
    pub span: Span,
    pub name: String,
    /// Unpacked dimensions (memories): `reg [7:0] mem [0:255]`.
    pub unpacked: Vec<Range>,
    pub init: Option<Expr>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssignPair {
    pub span: Span,
    pub lhs: Expr,
    pub rhs: Expr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    pub span: Span,
    /// Module or gate-primitive name.
    pub mod_name: String,
    /// `#(...)` parameter overrides: named (`.P(v)`) or positional.
    pub params: Vec<ParamConn>,
    /// Instances: name (empty for unnamed gates), array range, port conns.
    pub insts: Vec<InstUnit>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParamConn {
    pub span: Span,
    /// Some for `.P(v)`; None for positional.
    pub name: Option<String>,
    /// None for empty connections `.P()` / positional `, ,`.
    pub expr: Option<Expr>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InstUnit {
    pub span: Span,
    pub name: String,
    pub array: Option<Range>,
    pub conns: Vec<PortConn>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PortConn {
    pub span: Span,
    /// Some for `.port(expr)`; None for positional.
    pub name: Option<String>,
    pub expr: Option<Expr>,
}

/// Used for both `function` and `task` (Task sets `is_task`).
#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub span: Span,
    pub is_task: bool,
    /// `function automatic ...` / `task automatic ...` — reentrant
    /// (per-activation frames); default is static (shared frame).
    pub automatic: bool,
    pub name: String,
    pub ret_signed: bool,
    pub ret_range: Option<Range>,
    pub ret_type: Option<ParamType>,
    /// ANSI header port declarations.
    pub ports: Vec<Decl>,
    /// Old-style / body declarations.
    pub decls: Vec<Decl>,
    pub body: Vec<Stmt>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    Block {
        span: Span,
        name: Option<String>,
        /// Block-local declarations (`begin : b integer i; ... end`).
        decls: Vec<Decl>,
        body: Vec<Stmt>,
    },
    BAssign {
        span: Span,
        lhs: Expr,
        rhs: Expr,
    },
    NbAssign {
        span: Span,
        lhs: Expr,
        rhs: Expr,
    },
    /// Intra-assignment delay (`a = #5 v;` / `q <= #10 d;`) — the RHS is
    /// sampled at scheduling, the commit lands after the delay.
    IntraAssign {
        span: Span,
        lhs: Expr,
        rhs: Expr,
        delay: DelayVal,
        nba: bool,
    },
    If {
        span: Span,
        cond: Expr,
        then: Box<Stmt>,
        els: Option<Box<Stmt>>,
    },
    Case {
        span: Span,
        kind: CaseKind,
        expr: Expr,
        items: Vec<CaseItem>,
    },
    For {
        span: Span,
        init: Vec<AssignPair>,
        cond: Expr,
        step: Vec<AssignPair>,
        body: Box<Stmt>,
    },
    While {
        span: Span,
        cond: Expr,
        body: Box<Stmt>,
    },
    Repeat {
        span: Span,
        count: Expr,
        body: Box<Stmt>,
    },
    Forever {
        span: Span,
        body: Box<Stmt>,
    },
    DelayCtrl {
        span: Span,
        delay: DelayVal,
        body: Option<Box<Stmt>>,
    },
    EventCtrl {
        span: Span,
        ctrl: EventCtrl,
        body: Option<Box<Stmt>>,
    },
    /// `$display(...)` and friends.
    SysCall {
        span: Span,
        name: String,
        args: Vec<Expr>,
    },
    /// Task call (with or without parentheses).
    SubCall {
        span: Span,
        name: String,
        args: Vec<Expr>,
    },
    /// `wait (cond) stmt_or_null` — level wait until cond is true.
    Wait {
        span: Span,
        cond: Expr,
        body: Option<Box<Stmt>>,
    },
    /// `fork ... join` — concurrent branches; the process waits at the
    /// join for all of them. (join_any/join_none are SV, milestone R8.)
    Fork {
        span: Span,
        name: Option<String>,
        branches: Vec<Stmt>,
    },
    /// `force lhs = rhs;` — procedural continuous override.
    Force {
        span: Span,
        lhs: Expr,
        rhs: Expr,
    },
    /// `release lhs;` — the reg keeps the forced value (04_force golden).
    Release {
        span: Span,
        lhs: Expr,
    },
    /// `deassign lhs;`
    Deassign {
        span: Span,
        lhs: Expr,
    },
    /// `assign lhs = rhs;` inside procedural scope — procedural
    /// continuous assignment (cancelled by `deassign`).
    ProcContAssign {
        span: Span,
        lhs: Expr,
        rhs: Expr,
    },
    /// `-> ev` — named event trigger (R4-e).
    Trigger {
        span: Span,
        name: String,
    },
    Disable {
        span: Span,
        name: String,
    },
    Null {
        span: Span,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseKind {
    Case,
    Casex,
    Casez,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CaseItem {
    Items(Vec<Expr>, Stmt),
    Default(Stmt),
}

#[derive(Clone, Debug, PartialEq)]
pub struct EventCtrl {
    pub span: Span,
    pub edges: Vec<EventEdge>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EventEdge {
    pub kind: EdgeKind,
    pub expr: Expr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    Posedge,
    Negedge,
    Edge,
    Any,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DelayVal {
    /// `#5`, `#1.5`, `#4'b10` — raw literal text.
    Lit(String),
    /// `#d`, `#(a + b)`.
    Expr(Expr),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Ident {
        span: Span,
        name: String,
    },
    HierIdent {
        span: Span,
        parts: Vec<String>,
    },
    /// Generate-scope / instance hierarchical reference whose base is
    /// indexed (`blk[I-1].z`, `netgen[4].in`) — indexes evaluate at
    /// elaboration time, so this collapses to a canonical Ident.
    HierDot {
        span: Span,
        base: Box<Expr>,
        name: String,
    },
    NumLit(NumLit),
    RealLit {
        span: Span,
        text: String,
    },
    StrLit {
        span: Span,
        value: String,
    },
    Unary {
        span: Span,
        op: UnOp,
        expr: Box<Expr>,
    },
    Binary {
        span: Span,
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Ternary {
        span: Span,
        cond: Box<Expr>,
        then: Box<Expr>,
        els: Box<Expr>,
    },
    Concat {
        span: Span,
        parts: Vec<Expr>,
    },
    /// `a:b:c` min:typ:max delay triple (U-3b, measured on iverilog 13.0:
    /// `-T` selects one value; the default is typ with a one-line
    /// "Choosing typ expression." warning, capped at the first ten sites).
    /// Only meaningful in gate delay position; the elaborator decodes it —
    /// other positions treat it as a hard error.
    Delay3 {
        span: Span,
        min: Box<Expr>,
        typ: Box<Expr>,
        max: Box<Expr>,
    },
    Repeat {
        span: Span,
        count: Box<Expr>,
        value: Box<Expr>,
    },
    Index {
        span: Span,
        base: Box<Expr>,
        index: Box<Expr>,
    },
    PartSelect {
        span: Span,
        base: Box<Expr>,
        msb: Box<Expr>,
        lsb: Box<Expr>,
    },
    IndexedPart {
        span: Span,
        base: Box<Expr>,
        offset: Box<Expr>,
        width: Box<Expr>,
        plus: bool,
    },
    Call {
        span: Span,
        func: String,
        args: Vec<Expr>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NumLit {
    pub span: Span,
    pub width: Option<u32>,
    pub signed: bool,
    pub base: Base,
    /// Digits with underscores removed; x/z/? preserved verbatim.
    pub digits: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    Dec,
    Bin,
    Oct,
    Hex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Not,
    BitNot,
    Plus,
    Minus,
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Xnor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Eq,
    Ne,
    EqCase,
    NeCase,
    Lt,
    Le,
    Gt,
    Ge,
    LogAnd,
    LogOr,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    AShl,
    AShr,
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Ident { span, .. }
            | Expr::HierIdent { span, .. }
            | Expr::HierDot { span, .. }
            | Expr::RealLit { span, .. }
            | Expr::StrLit { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Ternary { span, .. }
            | Expr::Concat { span, .. }
            | Expr::Delay3 { span, .. }
            | Expr::Repeat { span, .. }
            | Expr::Index { span, .. }
            | Expr::PartSelect { span, .. }
            | Expr::IndexedPart { span, .. }
            | Expr::Call { span, .. } => *span,
            Expr::NumLit(n) => n.span,
        }
    }

    /// Syntactically valid assignment target (LRM §A.4.1 lvalue).
    pub fn is_lvalue(&self) -> bool {
        matches!(
            self,
            Expr::Ident { .. }
                | Expr::HierIdent { .. }
                | Expr::HierDot { .. }
                | Expr::Index { .. }
                | Expr::PartSelect { .. }
                | Expr::IndexedPart { .. }
                | Expr::Concat { .. }
        )
    }
}
