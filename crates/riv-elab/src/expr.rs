//! Expression lowering with IEEE 1364-2005 §5.4/§5.5 width+sign rules
//! baked into ops at lowering time (design §5-3), plus the constant
//! evaluator used for params/ranges/labels.
//!
//! Context contract (settled by verilator probes recorded in the design
//! doc's evidence block and this session):
//! - Assignment and continuous-assignment RHS see the LHS width as their
//!   context (probe: `reg [8:0] q; q = a + b` keeps the carry → 300).
//! - Self-determined sub-contexts: shift amounts, comparison operands
//!   (extended to each other, not to the outer context), concat parts,
//!   logical operands, reduction operands, ternary conditions, display
//!   args.
//! - Extension is a SIGN extension iff the extended expression's own
//!   all-operands signedness says so (probe: `q = signed_x` → 511, not
//!   255); the kernel performs the adjust at op time using the baked
//!   `signed` flag, so no context signedness flows top-down — the
//!   demanded width is the only context.
//! - Unsized decimal literals are signed 32-bit (probe: `s > -1` with a
//!   4-bit signed s → 1); unsized based literals are unsigned 32-bit.

use crate::err::{err_at, ElabError, ElabResult};
use crate::fmt;
use riv_ir::{Base, BinKind, BitKind, CmpKind, FmtPiece, Literal, Lval, Op, RedKind, SensItem, ShiftKind, Slot, SysId, VarId, VarKind};
use vl_syntax::ast::{self as ast, Base as AB, BinOp, DelayVal, Expr, Stmt, UnOp};
use vl_values::{from_numlit_parts, Bits, Base as VB};

/// A lowered value: frame slot + baked type info.
#[derive(Clone, Copy, Debug)]
pub struct Val {
    pub slot: Slot,
    pub w: u32,
    pub signed: bool,
    /// `$time`-family values carry unit ticks (used by %t at C5).
    pub is_time: bool,
}

/// Scope entry: a thread-local (block-local) variable living in a frame
/// slot, or a module-level design var.
#[derive(Clone, Copy, Debug)]
pub enum NameKind {
    Local { slot: Slot, w: u32, signed: bool },
    Var(VarId),
}

/// State shared by the lowering of one thread. Reads `Elab` immutably
/// (vars/params are fully registered before any thread is lowered) and
/// owns its op buffer.
pub struct Lower<'e, 'f> {
    pub e: &'e super::Elab<'e>,
    /// Display-format string pool, local to this lowering session; the
    /// walker merges it into the design pool at commit time (with an
    /// index rebase on SysCall fmt fields).
    pub strs: Vec<Vec<FmtPiece>>,
    pub ops: Vec<Op>,
    pub next_slot: Slot,
    pub scopes: Vec<(String, NameKind)>,
    /// Design vars read so far in this thread (for `@*` lowering).
    pub reads: Vec<VarId>,
    pub file: &'f str,
    /// Per-source-file timescale ratio (unit / precision) baked into
    /// ScaleTime ops and literal delay scaling.
    pub scale_num: u64,
    pub scale_den: u64,
}

impl<'e, 'f> Lower<'e, 'f> {
    pub fn slot(&mut self) -> Slot {
        let s = self.next_slot;
        self.next_slot += 1;
        s
    }

    pub fn emit(&mut self, op: Op) -> u32 {
        self.ops.push(op);
        (self.ops.len() - 1) as u32
    }

    pub fn here(&self) -> u32 {
        self.ops.len() as u32
    }

    pub fn err(&self, span: rcs_diag::Span, msg: impl Into<String>) -> ElabError {
        err_at(self.file, self.line_of(span), msg)
    }

    pub fn line_of(&self, span: rcs_diag::Span) -> u32 {
        self.e.sm.line_col(span.file, span.start).0
    }

    fn lookup(&self, name: &str) -> Option<NameKind> {
        if let Some((_, k)) = self.scopes.iter().rev().find(|(n, _)| n == name) {
            return Some(*k);
        }
        // Design vars resolve innermost-first, walking OUT through
        // transparent (generate) scopes to the enclosing module scope;
        // instance scopes are opaque boundaries.
        for sc in self.e.scopes.iter().rev() {
            if let Some(v) = sc.names.get(name) {
                return Some(NameKind::Var(*v));
            }
            if !sc.transparent {
                break;
            }
        }
        None
    }

    /// Resolve a hierarchical reference (`u1.u2.a`, `blk[3].z`) by dotted
    /// path against the var arena, walking scopes outward from the current
    /// prefix (Verilog name resolution: innermost scope first).
    fn resolve_hier(&self, path: &str) -> Option<VarId> {
        let mut pfx = self.e.cur().prefix.clone();
        let found = loop {
            let cand = format!("{pfx}{path}");
            if let Some(i) = self.e.vars.iter().position(|v| v.name == cand) {
                break Some(i as VarId);
            }
            match pfx.rfind('.') {
                Some(i) if i > 0 => pfx.truncate(i + 1),
                _ => break None,
            }
        }
        .or_else(|| self.e.vars.iter().position(|v| v.name == path).map(|i| i as VarId))?;
        // Collapsed-port aliases: the arena still holds the dead child row
        // under its dotted name; follow it to the live storage var.
        let mut v = found;
        while let Some(next) = self.e.aliases.get(&v) {
            v = *next;
        }
        Some(v)
    }

    /// Resolve a hierarchical path to a design var (hard error when
    /// missing — AGENTS §6).
    fn hier_var(&self, path: &str, span: rcs_diag::Span) -> ElabResult<VarId> {
        self.resolve_hier(path).ok_or_else(|| {
            self.err(span, format!("hierarchical name `{path}` does not resolve"))
        })
    }

    /// Flatten a HierDot base into a path component (Ident / indexed
    /// generate scope `blk[3]` with a constant index).
    fn hier_base_path(&self, base: &Expr) -> Option<String> {
        match base {
            Expr::Ident { name, .. } => Some(name.clone()),
            Expr::Index { base, index, .. } => {
                let Expr::Ident { name, .. } = base.as_ref() else { return None };
                let (b, _) = self.const_eval(index).ok()?;
                if b.has_unknown() {
                    return None;
                }
                Some(format!("{name}[{}]", b.to_u128_lossy()))
            }
            Expr::HierDot { base, name, .. } => Some(format!("{}.{}", self.hier_base_path(base)?, name)),
            Expr::HierIdent { parts, .. } => Some(parts.join(".")),
            _ => None,
        }
    }

    pub fn note_read(&mut self, var: VarId) {
        if !self.reads.contains(&var) {
            self.reads.push(var);
        }
    }

    /// Materialize a constant into a fresh slot.
    pub fn const_bits(&mut self, b: &Bits, signed: bool) -> Slot {
        let dst = self.slot();
        let lit = bits_to_literal(b, signed);
        self.emit(Op::Const { dst, lit });
        dst
    }

    pub fn const_u64(&mut self, v: u64, w: u32) -> Slot {
        let dst = self.slot();
        self.emit(Op::Const {
            dst,
            lit: Literal {
                width: w,
                signed: false,
                base: Base::Dec,
                digits: v.to_string(),
            },
        });
        dst
    }

    fn pool_str(&mut self, pieces: Vec<FmtPiece>) -> u32 {
        let idx = self.strs.len() as u32;
        self.strs.push(pieces);
        idx
    }
}

/// Render a Bits as a wire literal (hex for known values, binary with
/// per-bit x/z otherwise) so 4-state values roundtrip through the IR
/// exactly.
pub fn bits_to_literal(b: &Bits, signed: bool) -> Literal {
    if b.has_unknown() {
        // vl-values 4-state layout: x = (data 0, mask 1), z = (data 1,
        // mask 1) — mirrors from_numlit_parts's z-digit encoding
        // `(low_bits, low_bits)` and all_x's `(0, mask)`. Sentinels
        // compare bit-exactly via === (to_u128_lossy masks unknown bits,
        // so a raw data probe would misclassify).
        let x1 = Bits::from_parts(1, 0, 1).unwrap();
        let z1 = Bits::from_parts(1, 1, 1).unwrap();
        let mut digits = String::new();
        for i in (0..b.width).rev() {
            let bit = b.part(i, i).unwrap();
            let c = if bit.case_eq(&x1).unwrap() == Bits::from_u64(1, 1).unwrap() {
                'x'
            } else if bit.case_eq(&z1).unwrap() == Bits::from_u64(1, 1).unwrap() {
                'z'
            } else if bit.to_u128_lossy() & 1 == 1 {
                '1'
            } else {
                '0'
            };
            digits.push(c);
        }
        Literal {
            width: b.width,
            signed,
            base: Base::Bin,
            digits,
        }
    } else {
        Literal {
            width: b.width,
            signed,
            base: Base::Hex,
            digits: b.fmt_hex(),
        }
    }
}

fn base_map(b: AB) -> VB {
    match b {
        AB::Dec => VB::Dec,
        AB::Bin => VB::Bin,
        AB::Oct => VB::Oct,
        AB::Hex => VB::Hex,
    }
}

impl<'e, 'f> Lower<'e, 'f> {
    /// Statically decide an expression's signedness (IEEE §5.5.1: an
    /// operation is signed iff all its operands are signed; shift results
    /// follow the LEFT operand only — the amount is self-determined).
    pub fn expr_signed(&self, e: &Expr) -> bool {
        match e {
            Expr::NumLit(n) => n.signed || (n.width.is_none() && n.base == AB::Dec),
            Expr::Ident { name, .. } => match self.lookup(name) {
                Some(NameKind::Local { signed, .. }) => signed,
                Some(NameKind::Var(v)) => self.e.vars[v as usize].signed,
                None => self.scope_param(name).map(|(_, s)| *s).unwrap_or(false),
            },
            Expr::StrLit { .. } | Expr::RealLit { .. } => false,
            Expr::Call { func, .. } => match func.as_str() {
                "$signed" => true,
                "$unsigned" => false,
                _ => false,
            },
            Expr::Unary { op, expr, .. } => match op {
                UnOp::Plus | UnOp::Minus | UnOp::BitNot => self.expr_signed(expr),
                _ => false,
            },
            Expr::Binary { op, lhs, rhs, .. } => match op {
                BinOp::Add
                | BinOp::Sub
                | BinOp::Mul
                | BinOp::Div
                | BinOp::Mod
                | BinOp::Pow
                | BinOp::BitAnd
                | BinOp::BitOr
                | BinOp::BitXor => self.expr_signed(lhs) && self.expr_signed(rhs),
                // Shift result signedness = left operand only.
                BinOp::Shl | BinOp::Shr | BinOp::AShl | BinOp::AShr => self.expr_signed(lhs),
                _ => false,
            },
            Expr::Ternary { then, els, .. } => self.expr_signed(then) && self.expr_signed(els),
            _ => false,
        }
    }

    /// Natural (self-determined) width of an expression, IEEE §5.4.1
    /// Table 33.
    pub fn natural_w(&self, e: &Expr) -> ElabResult<u32> {
        Ok(match e {
            Expr::NumLit(n) => n.width.unwrap_or(32),
            Expr::StrLit { value, .. } => (8 * value.chars().count() as u32).max(8),
            Expr::Ident { name, .. } => match self.lookup(name) {
                Some(NameKind::Local { w, .. }) => w,
                Some(NameKind::Var(v)) => self.e.vars[v as usize].width,
                None => self.param_bits(name)?.0.width,
            },
            Expr::Unary { op, expr, .. } => match op {
                UnOp::Plus | UnOp::Minus | UnOp::BitNot => self.natural_w(expr)?,
                _ => 1,
            },
            Expr::Binary { op, lhs, rhs, .. } => match op {
                BinOp::Add
                | BinOp::Sub
                | BinOp::Mul
                | BinOp::Div
                | BinOp::Mod
                | BinOp::Pow
                | BinOp::BitAnd
                | BinOp::BitOr
                | BinOp::BitXor => self.natural_w(lhs)?.max(self.natural_w(rhs)?),
                BinOp::Shl | BinOp::Shr | BinOp::AShl | BinOp::AShr => self.natural_w(lhs)?,
                BinOp::Eq
                | BinOp::Ne
                | BinOp::EqCase
                | BinOp::NeCase
                | BinOp::Lt
                | BinOp::Le
                | BinOp::Gt
                | BinOp::Ge
                | BinOp::LogAnd
                | BinOp::LogOr => 1,
            },
            Expr::Ternary { then, els, .. } => self.natural_w(then)?.max(self.natural_w(els)?),
            Expr::Concat { parts, .. } => {
                let mut w = 0u32;
                for p in parts {
                    w += self.natural_w(p)?;
                }
                w.max(1)
            }
            Expr::Repeat { value, .. } => self.natural_w(value)?,
            Expr::Index { .. } => 1,
            Expr::PartSelect { msb, lsb, .. } => {
                let (m, l) = (self.const_u32_of(msb)?, self.const_u32_of(lsb)?);
                m.abs_diff(l) + 1
            }
            Expr::IndexedPart { width, .. } => self.const_u32_of(width)?,
            Expr::HierIdent { parts, .. } => {
                let v = self.hier_var(&parts.join("."), e.span())?;
                self.e.vars[v as usize].width
            }
            Expr::HierDot { base, name, span } => {
                let p = self.hier_base_path(base)
                    .ok_or_else(|| self.err(*span, "hierarchical base must be a scope path here"))?;
                let v = self.hier_var(format!("{p}.{name}").as_str(), *span)?;
                self.e.vars[v as usize].width
            }
            Expr::RealLit { span, .. } => {
                return Err(self.err(
                    *span,
                    "real literals are not supported (real types are not in the planned R2-R5 surface; see docs/RIVERILOG-PLAN.md §8)",
                ))
            }
            Expr::Call { span, func, args } => match func.as_str() {
                "$signed" | "$unsigned" => {
                    let [a] = args.as_slice() else {
                        return Err(self.err(*span, format!("{func} takes exactly one argument")));
                    };
                    self.natural_w(a)?
                }
                "$clog2" | "$bits" => 32,
                "$time" | "$realtime" => 64,
                _ => {
                    return Err(self.err(
                        *span,
                        format!("function call `{func}` needs tasks/functions (milestone R3)"),
                    ))
                }
            },
        })
    }

    /// Resolve an identifier to a constant parameter value.
    fn scope_param(&self, name: &str) -> Option<&(Bits, bool)> {
        for sc in self.e.scopes.iter().rev() {
            if let Some(p) = sc.params.get(name) {
                return Some(p);
            }
            if !sc.transparent {
                break;
            }
        }
        None
    }

    /// Functions resolve innermost-first, walking out through transparent
    /// (generate) scopes.
    fn scope_func(&self, name: &str) -> Option<(u32, Vec<(u32, ast::Direction, u32, bool)>)> {
        for sc in self.e.scopes.iter().rev() {
            if let Some(f) = sc.funcs.get(name) {
                return Some(f.clone());
            }
            if !sc.transparent {
                break;
            }
        }
        None
    }

    pub fn param_bits(&self, name: &str) -> ElabResult<(Bits, bool)> {
        match self.scope_param(name) {
            Some((b, s)) => Ok((b.clone(), *s)),
            None => Err(err_at(
                self.file,
                0,
                format!("identifier `{name}` is not declared (AGENTS §6: unresolved names hard-error)"),
            )),
        }
    }

    pub fn const_u32_of(&self, e: &Expr) -> ElabResult<u32> {
        let (b, _) = self.const_eval(e)?;
        if b.has_unknown() {
            return Err(err_at(
                self.file,
                self.line_of(e.span()),
                "constant required here (value contains x/z)",
            ));
        }
        Ok(b.to_u128_lossy() as u32)
    }

    /// Full expression lowering. Returns the value's slot with its baked
    /// width/signedness; `ctx` is the demanded width (0 = self-determined).
    pub fn lower(&mut self, e: &Expr, ctx: u32) -> ElabResult<Val> {
        match e {
            Expr::Ident { name, .. } => match self.lookup(name) {
                Some(NameKind::Local { slot, w, signed }) => {
                    Ok(Val { slot, w, signed, is_time: false })
                }
                Some(NameKind::Var(v)) => {
                    self.note_read(v);
                    let dst = self.slot();
                    self.emit(Op::LoadVar { dst, var: v });
                    let vi = self.e.vars[v as usize].clone();
                    Ok(Val { slot: dst, w: vi.width, signed: vi.signed, is_time: false })
                }
                None => {
                    let (b, s) = self.param_bits(name)?;
                    let slot = self.const_bits(&b, s);
                    Ok(Val { slot, w: b.width, signed: s, is_time: false })
                }
            },
            Expr::NumLit(n) => {
                // Unsized literals pass width=None so vl-values applies its
                // unsized rules (≥32 bits; `'hx` → the whole value x).
                let signed = n.signed || (n.width.is_none() && n.base == AB::Dec);
                let b = from_numlit_parts(n.width, signed, base_map(n.base), &n.digits)
                    .map_err(|er| self.err(n.span, format!("bad numeric literal: {}", er.0)))?;
                let slot = self.const_bits(&b, signed);
                Ok(Val { slot, w: b.width, signed, is_time: false })
            }
            Expr::StrLit { value, .. } => {
                // IEEE: a string literal is an unsigned vector of 8-bit
                // chars, leftmost char = MSB (design §5-4 StrLit→Bits).
                let w = (8 * value.chars().count() as u32).max(8);
                let mut digits = String::new();
                for c in value.chars() {
                    digits.push_str(&format!("{:02x}", c as u32 as u8));
                }
                if value.is_empty() {
                    digits.push_str("00");
                }
                let dst = self.slot();
                self.emit(Op::Const {
                    dst,
                    lit: Literal { width: w, signed: false, base: Base::Hex, digits },
                });
                Ok(Val { slot: dst, w, signed: false, is_time: false })
            }
            Expr::RealLit { span, .. } => Err(self.err(
                *span,
                "real literals are not supported (real types are not in the planned R2-R5 surface; see docs/RIVERILOG-PLAN.md §8)",
            )),
            Expr::Unary { span, op, expr } => self.lower_unary(*span, *op, expr, ctx),
            Expr::Binary { span, op, lhs, rhs } => self.lower_binary(*span, *op, lhs, rhs, ctx),
            Expr::Ternary { cond, then, els, .. } => {
                let c = self.lower(cond, 0)?;
                let w = ctx.max(self.natural_w(then)?).max(self.natural_w(els)?);
                let signed = self.expr_signed(then) && self.expr_signed(els);
                let t = self.lower(then, w)?;
                let e2 = self.lower(els, w)?;
                let dst = self.slot();
                self.emit(Op::Sel { dst, cond: c.slot, a: t.slot, b: e2.slot });
                Ok(Val { slot: dst, w, signed, is_time: false })
            }
            Expr::Concat { parts, .. } => {
                let mut slots = Vec::new();
                let mut w = 0u32;
                for p in parts {
                    let v = self.lower(p, 0)?;
                    w += v.w;
                    slots.push(v.slot);
                }
                let dst = self.slot();
                self.emit(Op::Concat { dst, parts: slots });
                Ok(Val { slot: dst, w: w.max(1), signed: false, is_time: false })
            }
            Expr::Repeat { span, count, value } => {
                let (cb, _) = self.const_eval(count)?;
                if cb.has_unknown() || cb.to_u128_lossy() == 0 {
                    return Err(self.err(*span, "replication count must be a non-zero constant"));
                }
                let times = cb.to_u128_lossy();
                let v = self.lower(value, 0)?;
                let dst = self.slot();
                self.emit(Op::Replicate { dst, src: v.slot, times: times as u64 });
                Ok(Val { slot: dst, w: v.w * times as u32, signed: false, is_time: false })
            }
            Expr::Index { base, index, .. } => {
                let (var, vw, _vs, _desc) = self.base_var(base)?;
                let i = self.lower(index, 0)?;
                let dst = self.slot();
                if self.e.vars[var as usize].kind == riv_ir::VarKind::Array {
                    self.emit(Op::LoadElem { dst, var, idx: i.slot });
                    Ok(Val { slot: dst, w: vw, signed: false, is_time: false })
                } else {
                    self.emit(Op::LoadBit { dst, var, idx: i.slot });
                    Ok(Val { slot: dst, w: 1, signed: false, is_time: false })
                }
            }
            Expr::PartSelect { base, msb, lsb, span } => {
                let (var, _, _, desc) = self.base_var(base)?;
                let m = self.const_u32_of(msb);
                let l = self.const_u32_of(lsb);
                match (m, l) {
                    (Ok(m), Ok(l)) => {
                        let w = m.abs_diff(l) + 1;
                        let (hi, lo) = if desc { (m, l) } else { (l, m) };
                        let (ahi, alo) = self.abs_part(var, hi, lo, desc, *span)?;
                        let dst = self.slot();
                        self.emit(Op::LoadPart { dst, var, hi: ahi, lo: alo });
                        Ok(Val { slot: dst, w, signed: false, is_time: false })
                    }
                    _ => Err(self.err(
                        *span,
                        "variable part-select bounds are illegal Verilog-2001 (use bit-select or `+:`/`-:`)",
                    )),
                }
            }
            Expr::IndexedPart { span, .. } => Err(self.err(
                *span,
                "indexed part-selects (`+:`/`-:`) are not supported yet (see docs/RIVERILOG-PLAN.md §8)",
            )),
            Expr::HierIdent { parts, span } => {
                let v = self.hier_var(parts.join(".").as_str(), *span)?;
                self.note_read(v);
                let dst = self.slot();
                self.emit(Op::LoadVar { dst, var: v });
                let vi = self.e.vars[v as usize].clone();
                Ok(Val { slot: dst, w: vi.width, signed: vi.signed, is_time: false })
            }
            Expr::HierDot { base, name, span } => {
                let p = self.hier_base_path(base)
                    .ok_or_else(|| self.err(*span, "hierarchical base must be a scope path here"))?;
                let v = self.hier_var(format!("{p}.{name}").as_str(), *span)?;
                self.note_read(v);
                let dst = self.slot();
                self.emit(Op::LoadVar { dst, var: v });
                let vi = self.e.vars[v as usize].clone();
                Ok(Val { slot: dst, w: vi.width, signed: vi.signed, is_time: false })
            }
            Expr::Call { span, func, args } => self.lower_call(*span, func, args, ctx),
        }
    }

    fn lower_unary(&mut self, span: rcs_diag::Span, op: UnOp, expr: &Expr, ctx: u32) -> ElabResult<Val> {
        Ok(match op {
            UnOp::Plus => self.lower(expr, ctx)?,
            UnOp::Minus => {
                let a = self.lower(expr, ctx)?;
                let w = ctx.max(a.w);
                let zero = self.const_u64(0, w);
                let dst = self.slot();
                self.emit(Op::Bin { kind: BinKind::Sub, dst, a: zero, b: a.slot, w, signed: a.signed });
                Val { slot: dst, w, signed: a.signed, is_time: false }
            }
            UnOp::BitNot => {
                let a = self.lower(expr, ctx)?;
                let w = ctx.max(a.w);
                let dst = self.slot();
                self.emit(Op::BitNot { dst, a: a.slot, w });
                Val { slot: dst, w, signed: a.signed, is_time: false }
            }
            UnOp::Not => {
                let a = self.lower(expr, 0)?;
                let dst = self.slot();
                self.emit(Op::LogicNot { dst, a: a.slot });
                Val { slot: dst, w: 1, signed: false, is_time: false }
            }
            UnOp::And | UnOp::Nand | UnOp::Or | UnOp::Nor | UnOp::Xor | UnOp::Xnor => {
                let a = self.lower(expr, 0)?;
                let kind = match op {
                    UnOp::And => RedKind::And,
                    UnOp::Nand => RedKind::Nand,
                    UnOp::Or => RedKind::Or,
                    UnOp::Nor => RedKind::Nor,
                    UnOp::Xor => RedKind::Xor,
                    _ => RedKind::Xnor,
                };
                let dst = self.slot();
                self.emit(Op::Reduce { kind, dst, a: a.slot });
                let _ = span;
                Val { slot: dst, w: 1, signed: false, is_time: false }
            }
        })
    }

    fn lower_binary(&mut self, span: rcs_diag::Span, op: BinOp, lhs: &Expr, rhs: &Expr, ctx: u32) -> ElabResult<Val> {
        Ok(match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod | BinOp::Pow => {
                let w = ctx.max(self.natural_w(lhs)?).max(self.natural_w(rhs)?);
                let signed = self.expr_signed(lhs) && self.expr_signed(rhs);
                let a = self.lower(lhs, w)?;
                let b = self.lower(rhs, w)?;
                let kind = match op {
                    BinOp::Add => BinKind::Add,
                    BinOp::Sub => BinKind::Sub,
                    BinOp::Mul => BinKind::Mul,
                    BinOp::Div => BinKind::Div,
                    BinOp::Mod => BinKind::Rem,
                    _ => BinKind::Pow,
                };
                let dst = self.slot();
                self.emit(Op::Bin { kind, dst, a: a.slot, b: b.slot, w, signed });
                let _ = span;
                Val { slot: dst, w, signed, is_time: false }
            }
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
                let w = ctx.max(self.natural_w(lhs)?).max(self.natural_w(rhs)?);
                let a = self.lower(lhs, w)?;
                let b = self.lower(rhs, w)?;
                let kind = match op {
                    BinOp::BitAnd => BitKind::And,
                    BinOp::BitOr => BitKind::Or,
                    _ => BitKind::Xor,
                };
                let dst = self.slot();
                self.emit(Op::Bit { kind, dst, a: a.slot, b: b.slot, w });
                Val { slot: dst, w, signed: a.signed && b.signed, is_time: false }
            }
            BinOp::Shl | BinOp::AShl | BinOp::Shr | BinOp::AShr => {
                let w = ctx.max(self.natural_w(lhs)?);
                let a = self.lower(lhs, w)?;
                let b = self.lower(rhs, 0)?;
                let kind = match op {
                    BinOp::Shl | BinOp::AShl => ShiftKind::Shl,
                    BinOp::Shr => ShiftKind::Shr,
                    _ => ShiftKind::Ashr,
                };
                let dst = self.slot();
                self.emit(Op::Shift { kind, dst, a: a.slot, b: b.slot, w, signed: a.signed });
                Val { slot: dst, w, signed: a.signed, is_time: false }
            }
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne => {
                let w = self.natural_w(lhs)?.max(self.natural_w(rhs)?);
                let signed = self.expr_signed(lhs) && self.expr_signed(rhs);
                let a = self.lower(lhs, w)?;
                let b = self.lower(rhs, w)?;
                let kind = match op {
                    BinOp::Lt => CmpKind::Lt,
                    BinOp::Le => CmpKind::Le,
                    BinOp::Gt => CmpKind::Gt,
                    BinOp::Ge => CmpKind::Ge,
                    BinOp::Eq => CmpKind::Eq,
                    _ => CmpKind::Ne,
                };
                let dst = self.slot();
                self.emit(Op::Cmp { kind, dst, a: a.slot, b: b.slot, signed });
                Val { slot: dst, w: 1, signed: false, is_time: false }
            }
            BinOp::EqCase | BinOp::NeCase => {
                let w = self.natural_w(lhs)?.max(self.natural_w(rhs)?);
                let a = self.lower(lhs, w)?;
                let b = self.lower(rhs, w)?;
                let dst = self.slot();
                self.emit(Op::CaseCmp {
                    kind: if op == BinOp::NeCase { riv_ir::CaseCmpKind::ExactNe } else { riv_ir::CaseCmpKind::ExactEq },
                    dst,
                    a: a.slot,
                    b: b.slot,
                });
                Val { slot: dst, w: 1, signed: false, is_time: false }
            }
            BinOp::LogAnd | BinOp::LogOr => {
                let a = self.lower(lhs, 0)?;
                let b = self.lower(rhs, 0)?;
                let dst = self.slot();
                if op == BinOp::LogAnd {
                    self.emit(Op::LogicAnd { dst, a: a.slot, b: b.slot });
                } else {
                    self.emit(Op::LogicOr { dst, a: a.slot, b: b.slot });
                }
                Val { slot: dst, w: 1, signed: false, is_time: false }
            }
        })
    }

    /// Task/function call wiring (design §3): inputs copied into callee
    /// formal slots, output/inout lvalues committed back on return, dst
    /// receives the ret slot (slot 0). Arg lowering runs at the formal's
    /// width so the kernel's copies are exact.
    fn lower_call_site(
        &mut self,
        span: rcs_diag::Span,
        func: u32,
        name: &str,
        formals: Vec<(u32, ast::Direction, u32, bool)>,
        args: &[Expr],
        dst: Option<Slot>,
    ) -> ElabResult<Slot> {
        if args.len() != formals.len() {
            return Err(self.err(
                span,
                format!(
                    "`{name}` takes {} argument(s), got {}",
                    formals.len(),
                    args.len()
                ),
            ));
        }
        let mut ins: Vec<(Slot, u32)> = Vec::new();
        let mut outs: Vec<(Lval, u32)> = Vec::new();
        for (arg, (fs, dir, w, _signed)) in args.iter().zip(formals.iter()) {
            match dir {
                ast::Direction::Input => {
                    let v = self.lower(arg, *w)?;
                    ins.push((v.slot, *fs));
                }
                ast::Direction::Output => {
                    let lv = self.lower_lvalue(arg)?;
                    outs.push((lv, *fs));
                }
                ast::Direction::Inout => {
                    let v = self.lower(arg, *w)?;
                    ins.push((v.slot, *fs));
                    let lv = self.lower_lvalue(arg)?;
                    outs.push((lv, *fs));
                }
            }
        }
        let call = self.emit(Op::Call { func, ins, outs, dst });
        let _ = call;
        Ok(dst.unwrap_or(0))
    }

    fn lower_call(&mut self, span: rcs_diag::Span, func: &str, args: &[Expr], ctx: u32) -> ElabResult<Val> {
        let _ = ctx;
        match func {
            "$signed" | "$unsigned" => {
                let [a] = args else {
                    return Err(self.err(span, format!("{func} takes exactly one argument")));
                };
                let mut v = self.lower(a, ctx)?;
                v.signed = func == "$signed";
                Ok(v)
            }
            "$bits" => {
                let [a] = args else {
                    return Err(self.err(span, "$bits takes exactly one argument"));
                };
                let w = self.natural_w(a)?;
                let slot = self.const_u64(w as u64, 32);
                Ok(Val { slot, w: 32, signed: false, is_time: false })
            }
            "$clog2" => {
                let [a] = args else {
                    return Err(self.err(span, "$clog2 takes exactly one argument"));
                };
                let v = self.lower(a, 0)?;
                let dst = self.slot();
                self.emit(Op::SysCall {
                    id: SysId::Clog2,
                    fmt: None,
                    args: vec![riv_ir::Arg { slot: v.slot, width: v.w, signed: v.signed, is_time: false }],
                    dst: Some(dst),
                    file: 0,
                    line: 0,
                    start: 0,
                });
                Ok(Val { slot: dst, w: 32, signed: false, is_time: false })
            }
            "$time" => {
                let dst = self.slot();
                self.emit(Op::SysCall {
                    id: SysId::Time,
                    fmt: None,
                    args: vec![],
                    dst: Some(dst),
                    file: 0,
                    line: 0,
                    start: 0,
                });
                Ok(Val { slot: dst, w: 64, signed: false, is_time: true })
            }
            "$realtime" => Err(self.err(
                span,
                format!("`{func}` lands with the $time batch (milestone R3)"),
            )),
            "$test$plusargs" => {
                let [a] = args else {
                    return Err(self.err(span, "$test$plusargs takes exactly one argument"));
                };
                let Expr::StrLit { value, .. } = a else {
                    return Err(self.err(span, "plusarg pattern must be a string literal"));
                };
                let dst = self.slot();
                let fidx = self.pool_str(vec![FmtPiece::Text(value.clone())]);
                self.emit(Op::SysCall {
                    id: SysId::TestPlusargs,
                    fmt: Some(fidx),
                    args: vec![],
                    dst: Some(dst),
                    file: 0,
                    line: 0,
                    start: 0,
                });
                Ok(Val { slot: dst, w: 1, signed: false, is_time: false })
            }
            "$value$plusargs" => {
                let [a, out] = args else {
                    return Err(self.err(span, "$value$plusargs takes exactly two arguments"));
                };
                let Expr::StrLit { value, .. } = a else {
                    return Err(self.err(span, "plusarg pattern must be a string literal"));
                };
                let fidx = self.pool_str(vec![FmtPiece::Text(value.clone())]);
                // The parsed value lands in a fresh slot; the variable write
                // is conditional on the match (IEEE 17.6: only on match).
                let out_slot = self.slot();
                let ret = self.slot();
                let (w, signed) = match out {
                    Expr::Ident { name, span: _ } => match self.lookup(name) {
                        Some(NameKind::Var(v)) => {
                            let vi = self.e.vars[v as usize].clone();
                            (vi.width, vi.signed)
                        }
                        Some(NameKind::Local { w, signed, .. }) => (w, signed),
                        None => return Err(self.err(out.span(), format!("identifier `{name}` is not declared"))),
                    },
                    _ => return Err(self.err(out.span(), "$value$plusargs output must be a variable")),
                };
                self.emit(Op::SysCall {
                    id: SysId::ValuePlusargs,
                    fmt: Some(fidx),
                    args: vec![riv_ir::Arg { slot: out_slot, width: w, signed, is_time: false }],
                    dst: Some(ret),
                    file: 0,
                    line: 0,
                    start: 0,
                });
                let br = self.emit(Op::Branch { cond: ret, then: u32::MAX, els: u32::MAX });
                let then_pc = self.here();
                match out {
                    Expr::Ident { name, .. } => match self.lookup(name) {
                        Some(NameKind::Var(v)) => {
                            self.emit(Op::AssignBlk { lv: Lval::Var { var: v }, src: out_slot });
                        }
                        Some(NameKind::Local { slot, .. }) => {
                            self.emit(Op::Mov { dst: slot, src: out_slot });
                        }
                        None => unreachable!(),
                    },
                    _ => unreachable!(),
                }
                let j = self.emit(Op::Jump { to: u32::MAX });
                let els_pc = self.here();
                self.ops[br as usize] = Op::Branch { cond: ret, then: then_pc, els: els_pc };
                self.ops[j as usize] = Op::Jump { to: els_pc };
                Ok(Val { slot: ret, w: 1, signed: false, is_time: false })
            }
            "$random" | "$dist_uniform" | "$dist_poisson" | "$dist_normal" | "$dist_exponential"
            | "$dist_chi_square" | "$dist_t" | "$dist_erlang" => Err(self.err(
                span,
                format!("`{func}` lands with the IO batch (milestone R5)"),
            )),
            _ => {
                // User-defined function (IEEE §10.3): evaluate args, call,
                // the ret slot is the value.
                let Some((fidx, formals)) = self.scope_func(func) else {
                    return Err(self.err(
                        span,
                        format!("system function `{func}` is not supported (see docs/RIVERILOG-PLAN.md §8)"),
                    ));
                };
                let Some((ret_w, ret_signed)) = self.e.ret_types.get(&fidx).copied() else {
                    return Err(self.err(span, format!("`{func}` is a task (tasks return nothing)")));
                };
                let dst = self.slot();
                self.lower_call_site(span, fidx, func, formals, args, Some(dst))?;
                Ok(Val { slot: dst, w: ret_w, signed: ret_signed, is_time: false })
            }
        }
    }

    /// Resolve the base of an index/part-select to a design var + range.
    fn base_var(&mut self, e: &Expr) -> ElabResult<(VarId, u32, bool, bool)> {
        let Expr::Ident { name, .. } = e else {
            return Err(self.err(
                e.span(),
                "bit/part-select base must be a plain identifier (hierarchical selects land with the strength batch)",
            ));
        };
        match self.lookup(name) {
            Some(NameKind::Var(v)) => {
                self.note_read(v);
                let vi = self.e.vars[v as usize].clone();
                Ok((v, vi.width, vi.signed, vi.desc))
            }
            Some(NameKind::Local { .. }) => Err(self.err(
                e.span(),
                "bit/part-select of a block-local variable is not supported yet (see docs/RIVERILOG-PLAN.md §8)",
            )),
            None => Err(self.err(e.span(), format!("identifier `{name}` is not declared"))),
        }
    }

    /// Map declared-range positions to absolute bit positions (bit 0 =
    /// LSB of the stored vector). Descending `[hi:lo]`: abs = idx - lo;
    /// ascending `[lo:hi]`: abs = lo - idx.
    fn abs_part(&self, var: VarId, hi: u32, lo: u32, desc: bool, span: rcs_diag::Span) -> ElabResult<(u32, u32)> {
        let v = &self.e.vars[var as usize];
        let map = |idx: u64| -> i64 {
            if desc {
                idx as i64 - v.lsb as i64
            } else {
                v.lsb as i64 - idx as i64
            }
        };
        let (a, b) = (map(hi as u64), map(lo as u64));
        if a < 0 || b < 0 || a >= v.width as i64 || b >= v.width as i64 {
            return Err(self.err(
                span,
                format!(
                    "part-select [{hi}:{lo}] leaves the declared range [{}:{}] of `{}`",
                    v.msb, v.lsb, v.name
                ),
            ));
        }
        Ok((a.max(b) as u32, a.min(b) as u32))
    }

    // ------------------------------------------------------------------
    // Constant evaluation: lower into a throwaway thread, then interpret
    // the ops with the same width semantics the kernel will use — one
    // lowering, no semantic duplication (params, ranges, case labels,
    // replication counts).
    // ------------------------------------------------------------------

    pub fn const_eval(&self, e: &Expr) -> ElabResult<(Bits, bool)> {
        let mut low = Lower {
            e: self.e,
            strs: Vec::new(),
            ops: Vec::new(),
            next_slot: 0,
            scopes: self.scopes.clone(),
            reads: Vec::new(),
            file: self.file,
            scale_num: self.scale_num,
            scale_den: self.scale_den,
        };
        let span = e.span();
        let v = low.lower(e, 0).map_err(|mut er| {
            er.message = format!("constant expression: {}", er.message);
            er
        })?;
        let ops = std::mem::take(&mut low.ops);
        let mut frame: Vec<Bits> = vec![Bits::zeros(1).unwrap(); low.next_slot as usize];
        let val = crate::interp::eval_ops(&ops, &mut frame)
            .map_err(|m| err_at(self.file, self.line_of(span), format!("constant expression: {m}")))?;
        Ok((val, v.signed))
    }

    /// Delay → a slot holding precision-unit ticks (design §4: verilator
    /// rounds delays to the timescale UNIT, iverilog to the PRECISION —
    /// ours follows iverilog, ledgered as rivD7 against the
    /// verilator-recorded goldens).
    pub fn delay_units(&mut self, d: &DelayVal, span: rcs_diag::Span) -> ElabResult<Slot> {
        match d {
            ast::DelayVal::Lit(text) => {
                let units = parse_delay_rational(text)
                    .ok_or_else(|| self.err(span, format!("bad delay literal `{text}`")))?;
                let ticks = scale_rational(units.0, units.1, self.scale_num, self.scale_den);
                let dst = self.slot();
                self.emit(Op::Const {
                    dst,
                    lit: Literal {
                        width: 64,
                        signed: false,
                        base: Base::Dec,
                        digits: ticks.to_string(),
                    },
                });
                Ok(dst)
            }
            ast::DelayVal::Expr(e) => {
                let v = self.lower(e, 0)?;
                let dst = self.slot();
                self.emit(Op::ScaleTime { dst, src: v.slot, num: self.scale_num, den: self.scale_den });
                Ok(dst)
            }
        }
    }

    /// Sensitivity items for an event control (`@(posedge a or negedge b)`).
    /// Only plain identifiers are edge expressions in flat designs.
    pub fn sens_items(&mut self, ctrl: &ast::EventCtrl) -> ElabResult<Vec<SensItem>> {
        let mut items = Vec::new();
        for edge in &ctrl.edges {
            let Expr::Ident { span, name } = &edge.expr else {
                return Err(self.err(
                    edge.expr.span(),
                    "event expressions must be plain identifiers (expression sensitivity needs the strength batch)",
                ));
            };
            let var = match self.lookup(name) {
                Some(NameKind::Var(v)) => v,
                Some(NameKind::Local { .. }) => {
                    return Err(self.err(*span, "cannot wait on a block-local variable"))
                }
                None => return Err(self.err(*span, format!("identifier `{name}` is not declared"))),
            };
            self.note_read(var);
            items.push(SensItem {
                var,
                edge: match edge.kind {
                    ast::EdgeKind::Posedge => riv_ir::EdgeKind::Pos,
                    ast::EdgeKind::Negedge => riv_ir::EdgeKind::Neg,
                    ast::EdgeKind::Edge | ast::EdgeKind::Any => riv_ir::EdgeKind::Any,
                },
            });
        }
        Ok(items)
    }
}

/// Parse a delay literal (`5`, `1.5`, `0.25`) into an exact rational
/// (numerator, denominator).
pub fn parse_delay_rational(text: &str) -> Option<(u128, u128)> {
    let (int_part, frac_part) = match text.split_once('.') {
        Some((i, f)) => (i, f),
        None => (text, ""),
    };
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    if !int_part.chars().all(|c| c.is_ascii_digit()) || !frac_part.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let den = 10u128.checked_pow(frac_part.len() as u32)?;
    let mut num: u128 = if int_part.is_empty() { 0 } else { int_part.parse().ok()? };
    if !frac_part.is_empty() {
        num = num.checked_mul(den)?.checked_add(frac_part.parse().ok()?)?;
    }
    Some((num, den))
}

/// `(n/d) * (sn/sd)`, rounded half-up, saturating.
pub fn scale_rational(n: u128, d: u128, sn: u64, sd: u64) -> u64 {
    let num = n.saturating_mul(sn as u128);
    let den = d.saturating_mul(sd as u128);
    ((num + den / 2) / den).min(u64::MAX as u128) as u64
}



impl<'e, 'f> Lower<'e, 'f> {
    /// Statement lowering (design §4 thread bodies). Branch/Jump targets
    /// are absolute op indices within this thread's body.
    pub fn lower_stmt(&mut self, s: &Stmt) -> ElabResult<()> {
        match s {
            Stmt::Block { decls, body, name, .. } => {
                if name.is_some() {
                    // Named blocks are hierarchical scopes; plain execution
                    // works, but name references (disable/hier) are R3/R4.
                }
                let mut pushed = 0usize;
                for d in decls {
                    match d.kind {
                        ast::DeclKind::Parameter | ast::DeclKind::Localparam => {
                            return Err(self.err(
                                d.span,
                                "block-local parameters are not supported (see docs/RIVERILOG-PLAN.md §8)",
                            ))
                        }
                        ast::DeclKind::Real | ast::DeclKind::Realtime => {
                            return Err(self.err(
                                d.span,
                                "`real` variables are not supported (real types are not in the planned R2-R5 surface; see docs/RIVERILOG-PLAN.md §8)",
                            ))
                        }
                        ast::DeclKind::Event => {
                            return Err(self.err(
                                d.span,
                                "block-local named events are not supported yet (declare events at module scope)",
                            ))
                        }
                        ast::DeclKind::Genvar => {
                            return Err(self.err(
                                d.span,
                                "genvars must be declared at module scope (generate constructs bind them at elaboration)",
                            ))
                        }
                        _ => {}
                    }
                    if !d.entries.is_empty() && !d.entries.iter().all(|e| e.unpacked.is_empty()) {
                        return Err(self.err(
                            d.span,
                            "block-local unpacked arrays are not supported yet (declare memories at module scope; see docs/RIVERILOG-PLAN.md §8)",
                        ));
                    }
                    let (w_default, signed_default) = match d.kind {
                        ast::DeclKind::Integer => (32u32, true),
                        ast::DeclKind::Time => (64u32, false),
                        _ => (1u32, d.signed),
                    };
                    let (w, hi, lo) = match &d.range {
                        Some(r) => {
                            let (hi, lo) = (self.const_u32_of(&r.msb)?, self.const_u32_of(&r.lsb)?);
                            (hi.abs_diff(lo) + 1, hi, lo)
                        }
                        None => (w_default, w_default.saturating_sub(1), 0),
                    };
                    let _ = (hi, lo);
                    for entry in &d.entries {
                        let slot = self.slot();
                        self.scopes.push((
                            entry.name.clone(),
                            NameKind::Local { slot, w, signed: signed_default },
                        ));
                        pushed += 1;
                        if let Some(init) = &entry.init {
                            let v = self.lower(init, w)?;
                            self.emit(Op::Mov { dst: slot, src: v.slot });
                        }
                    }
                }
                for st in body {
                    self.lower_stmt(st)?;
                }
                for _ in 0..pushed {
                    self.scopes.pop();
                }
                Ok(())
            }
            Stmt::BAssign { lhs, rhs, .. } => self.lower_assign(lhs, rhs, false),
            Stmt::NbAssign { lhs, rhs, .. } => self.lower_assign(lhs, rhs, true),
            Stmt::If { cond, then, els, .. } => {
                let c = self.lower(cond, 0)?;
                let br = self.emit(Op::Branch { cond: c.slot, then: u32::MAX, els: u32::MAX });
                let then_pc = self.here();
                self.lower_stmt(then)?;
                match els {
                    Some(e) => {
                        let j = self.emit(Op::Jump { to: u32::MAX });
                        let els_pc = self.here();
                        self.lower_stmt(e)?;
                        let end = self.here();
                        self.ops[br as usize] = Op::Branch { cond: c.slot, then: then_pc, els: els_pc };
                        self.ops[j as usize] = Op::Jump { to: end };
                    }
                    None => {
                        let end = self.here();
                        self.ops[br as usize] = Op::Branch { cond: c.slot, then: then_pc, els: end };
                    }
                }
                Ok(())
            }
            Stmt::Case { kind, expr, items, .. } => {
                let cmp_kind = match kind {
                    ast::CaseKind::Case => riv_ir::CaseCmpKind::ExactEq,
                    ast::CaseKind::Casez => riv_ir::CaseCmpKind::WildZ,
                    ast::CaseKind::Casex => riv_ir::CaseCmpKind::WildX,
                };
                let v = self.lower(expr, 0)?;
                let mut jumps: Vec<u32> = Vec::new();
                let mut default_stmt: Option<&Stmt> = None;
                for item in items {
                    match item {
                        ast::CaseItem::Items(labels, stmt) => {
                            for label in labels {
                                let l = self.lower(label, 0)?;
                                let eq = self.slot();
                                self.emit(Op::CaseCmp { kind: cmp_kind, dst: eq, a: v.slot, b: l.slot });
                                let br = self.emit(Op::Branch { cond: eq, then: u32::MAX, els: u32::MAX });
                                let body_pc = self.here();
                                self.lower_stmt(stmt)?;
                                let j = self.emit(Op::Jump { to: u32::MAX });
                                jumps.push(j);
                                let els_pc = self.here();
                                self.ops[br as usize] =
                                    Op::Branch { cond: eq, then: body_pc, els: els_pc };
                            }
                        }
                        ast::CaseItem::Default(stmt) => default_stmt = Some(stmt),
                    }
                }
                if let Some(d) = default_stmt {
                    // IEEE 9.5: default matches only when no item did,
                    // regardless of its source position (default-first).
                    self.lower_stmt(d)?;
                }
                let end = self.here();
                for j in jumps {
                    if let Op::Jump { to } = &mut self.ops[j as usize] {
                        *to = end;
                    }
                }
                Ok(())
            }
            Stmt::For { init, cond, step, body, .. } => {
                for a in init {
                    self.lower_assign(&a.lhs, &a.rhs, false)?;
                }
                let top = self.here();
                let c = self.lower(cond, 0)?;
                let br = self.emit(Op::Branch { cond: c.slot, then: u32::MAX, els: u32::MAX });
                let body_pc = self.here();
                self.lower_stmt(body)?;
                for a in step {
                    self.lower_assign(&a.lhs, &a.rhs, false)?;
                }
                self.emit(Op::Jump { to: top });
                let end = self.here();
                self.ops[br as usize] = Op::Branch { cond: c.slot, then: body_pc, els: end };
                Ok(())
            }
            Stmt::While { cond, body, .. } => {
                let top = self.here();
                let c = self.lower(cond, 0)?;
                let br = self.emit(Op::Branch { cond: c.slot, then: u32::MAX, els: u32::MAX });
                let body_pc = self.here();
                self.lower_stmt(body)?;
                self.emit(Op::Jump { to: top });
                let end = self.here();
                self.ops[br as usize] = Op::Branch { cond: c.slot, then: body_pc, els: end };
                Ok(())
            }
            Stmt::Forever { body, .. } => {
                // `forever` terminates only via $finish/disable — the loop
                // wrap is the delay-driven-always shape.
                let top = self.here();
                self.lower_stmt(body)?;
                self.emit(Op::Jump { to: top });
                Ok(())
            }
            Stmt::Repeat { count, body, .. } => {
                // `repeat (n) stmt` with a private counter (the
                // `repeat (n) @(posedge clk)` idiom of 44/27/42 lowers
                // through here; the count is evaluated once at entry).
                let c = self.lower(count, 0)?;
                let ctr = self.slot();
                self.emit(Op::Mov { dst: ctr, src: c.slot });
                let top = self.here();
                let br = self.emit(Op::Branch { cond: ctr, then: u32::MAX, els: u32::MAX });
                let body_pc = self.here();
                self.lower_stmt(body)?;
                let one = self.const_u64(1, c.w);
                self.emit(Op::Bin { kind: BinKind::Sub, dst: ctr, a: ctr, b: one, w: c.w, signed: false });
                self.emit(Op::Jump { to: top });
                let end = self.here();
                self.ops[br as usize] = Op::Branch { cond: ctr, then: body_pc, els: end };
                Ok(())
            }
            Stmt::DelayCtrl { delay, body, span } => {
                let amt = self.delay_units(delay, *span)?;
                self.emit(Op::Delay { amount: amt });
                if let Some(b) = body {
                    self.lower_stmt(b)?;
                }
                Ok(())
            }
            Stmt::EventCtrl { ctrl, body, .. } => {
                let items = self.sens_items(ctrl)?;
                if items.is_empty() {
                    // Statement-level `@*`: wait-first on the body's read
                    // set. Lower the body into a temp buffer to learn the
                    // set, then splice the wait in front (rebasing the
                    // body's branch targets).
                    let mut tmp = Vec::new();
                    let saved = std::mem::replace(&mut self.ops, std::mem::take(&mut tmp));
                    let reads_before = self.reads.len();
                    if let Some(b) = body {
                        self.lower_stmt(b)?;
                    } else {
                        return Err(self.err(ctrl.span, "`@*` without a statement"));
                    }
                    let body_ops = std::mem::replace(&mut self.ops, saved);
                    let new_reads: Vec<VarId> = self.reads[reads_before..].to_vec();
                    if new_reads.is_empty() {
                        return Err(self.err(ctrl.span, "`@*` has an empty sensitivity list"));
                    }
                    self.emit(Op::WaitEdge {
                        items: new_reads
                            .into_iter()
                            .map(|var| SensItem { var, edge: riv_ir::EdgeKind::Any })
                            .collect(),
                    });
                    let off = self.here();
                    let mut body_ops = body_ops;
                    rebase(&mut body_ops, off);
                    self.ops.extend(body_ops);
                    Ok(())
                } else {
                    self.emit(Op::WaitEdge { items });
                    if let Some(b) = body {
                        self.lower_stmt(b)?;
                    }
                    Ok(())
                }
            }
            Stmt::Wait { cond, body, .. } => {
                // `wait (cond) stmt` = suspend until cond is true: re-eval
                // loop over the condition's read set (level wait).
                let top = self.here();
                let c = self.lower(cond, 0)?;
                let br = self.emit(Op::Branch { cond: c.slot, then: u32::MAX, els: u32::MAX });
                let wait_pc = self.here();
                let reads = self.reads.clone();
                if reads.is_empty() {
                    return Err(self.err(cond.span(), "`wait` condition reads no variables"));
                }
                self.emit(Op::WaitEdge {
                    items: reads
                        .into_iter()
                        .map(|var| SensItem { var, edge: riv_ir::EdgeKind::Any })
                        .collect(),
                });
                self.emit(Op::Jump { to: top });
                let done = self.here();
                self.ops[br as usize] = Op::Branch { cond: c.slot, then: done, els: wait_pc };
                if let Some(b) = body {
                    self.lower_stmt(b)?;
                }
                Ok(())
            }
            Stmt::Fork { span, branches, .. } => {
                if branches.is_empty() {
                    return Err(self.err(*span, "`fork` with no branches"));
                }
                // Each branch body ends with JoinDone; the parent suspends
                // on the join group and resumes at JoinPoint.
                // Reserve the Fork op first so branch 0 starts at fork+1
                // (starting at the fork's own pc would re-fork forever).
                let fork_pc = self.emit(Op::Fork { branches: vec![], join: 0 });
                let mut pcs = Vec::new();
                for b in branches {
                    pcs.push(self.here());
                    self.lower_stmt(b)?;
                    self.emit(Op::JoinDone);
                }
                let join_pc = self.here();
                self.emit(Op::JoinPoint);
                self.ops[fork_pc as usize] = Op::Fork {
                    branches: pcs,
                    join: join_pc,
                };
                Ok(())
            }
            Stmt::Force { lhs, rhs, .. } => {
                let lv = self.lower_lvalue(lhs)?;
                if matches!(lv, Lval::Concat { .. }) {
                    return Err(self.err(
                        lhs.span(),
                        "`force` on a concatenation is not supported (see docs/RIVERILOG-PLAN.md §8)",
                    ));
                }
                let lw = match &lv {
                    Lval::Var { var } => self.e.vars[*var as usize].width,
                    Lval::Bit { .. } => 1,
                    Lval::Part { hi, lo, .. } => hi.abs_diff(*lo) + 1,
                    Lval::PartDyn { .. } => 1,
                    Lval::Elem { var, .. } => self.e.vars[*var as usize].width,
                    Lval::Concat { .. } => unreachable!(),
                };
                let v = self.lower(rhs, lw)?;
                self.emit(Op::AssignForce { lv, src: v.slot });
                Ok(())
            }
            Stmt::Release { lhs, .. } => {
                let lv = self.lower_lvalue(lhs)?;
                self.emit(Op::Release { lv });
                Ok(())
            }
            Stmt::Deassign { span, .. } => Err(self.err(
                *span,
                "`deassign` is not yet supported (milestone R3)",
            )),
            Stmt::ProcContAssign { span, .. } => Err(self.err(
                *span,
                "procedural continuous assignment (`assign`) is not yet supported (milestone R3)",
            )),
            Stmt::SysCall { span, name, args } => self.lower_syscall(*span, name, args),
            Stmt::SubCall { span, name, args } => {
                let Some((func, formals)) = self.scope_func(name) else {
                    return Err(self.err(
                        *span,
                        format!("task call `{name}` is not declared"),
                    ));
                };
                self.lower_call_site(*span, func, name, formals, args, None)?;
                Ok(())
            }
            Stmt::Disable { span, name } => Err(self.err(
                *span,
                format!("`disable {name}` lands at the behavioral batch (milestone R3)"),
            )),
            Stmt::Null { .. } => Ok(()),
        }
    }

    fn lower_assign(&mut self, lhs: &Expr, rhs: &Expr, nba: bool) -> ElabResult<()> {
        // Block-local targets never reach Lval (they are frame slots).
        if let Expr::Ident { name, .. } = lhs {
            if let Some(NameKind::Local { slot, w, signed }) = self.lookup(name) {
                if nba {
                    return Err(self.err(
                        lhs.span(),
                        "nonblocking assignment to a block-local variable is not supported yet (see docs/RIVERILOG-PLAN.md §8)",
                    ));
                }
                let v = self.lower(rhs, w)?;
                self.emit(Op::Mov { dst: slot, src: v.slot });
                let _ = signed;
                return Ok(());
            }
        }
        let lv = self.lower_lvalue(lhs)?;
        let lw = match &lv {
            Lval::Var { var } => self.e.vars[*var as usize].width,
            Lval::Bit { .. } => 1,
            Lval::Part { hi, lo, .. } => hi.abs_diff(*lo) + 1,
            Lval::PartDyn { .. } => 1,
            Lval::Elem { var, .. } => self.e.vars[*var as usize].width,
            Lval::Concat { parts } => parts.iter().map(|(_, w)| *w).sum(),
        };
        // Procedural assignment targets regs (nets get cont-assign drivers).
        if let Some(var) = lval_base_var(&lv) {
            let vi = &self.e.vars[var as usize];
            let ok = vi.kind == VarKind::Reg || (vi.kind == VarKind::Array && vi.array_of_reg);
            if !ok {
                return Err(self.err(
                    lhs.span(),
                    format!(
                        "procedural assignment to net `{}` (nets are driven continuously)",
                        self.e.vars[var as usize].name
                    ),
                ));
            }
        }
        // LHS-width context for the RHS (probe-verified carry rule).
        let v = self.lower(rhs, lw)?;
        let op = if nba {
            Op::AssignNba { lv, src: v.slot }
        } else {
            Op::AssignBlk { lv, src: v.slot }
        };
        self.emit(op);
        Ok(())
    }

    /// Lvalue lowering (flattened concat, dynamic bit indices as slots).
    pub fn lower_lvalue(&mut self, e: &Expr) -> ElabResult<Lval> {
        match e {
            Expr::Ident { name, span } => match self.lookup(name) {
                Some(NameKind::Var(v)) => Ok(Lval::Var { var: v }),
                Some(NameKind::Local { .. }) => Err(self.err(
                    *span,
                    "assignment target must be a module variable here",
                )),
                None => Err(self.err(*span, format!("identifier `{name}` is not declared"))),
            },
            Expr::Index { base, index, .. } => {
                let (var, _vw, _vs, _desc) = self.base_var(base)?;
                let i = self.lower(index, 0)?;
                if self.e.vars[var as usize].kind == riv_ir::VarKind::Array {
                    Ok(Lval::Elem { var, idx: i.slot })
                } else {
                    Ok(Lval::Bit { var, idx: i.slot })
                }
            }
            Expr::PartSelect { base, msb, lsb, span } => {
                let (var, _, _, desc) = self.base_var(base)?;
                let m = self.const_u32_of(msb);
                let l = self.const_u32_of(lsb);
                match (m, l) {
                    (Ok(m), Ok(l)) => {
                        let (hi, lo) = if desc { (m, l) } else { (l, m) };
                        let (ahi, alo) = self.abs_part(var, hi, lo, desc, *span)?;
                        Ok(Lval::Part { var, hi: ahi, lo: alo })
                    }
                    _ => Err(self.err(
                        *span,
                        "variable part-select bounds are illegal Verilog-2001 (use bit-select or `+:`/`-:`)",
                    )),
                }
            }
            Expr::Concat { parts, .. } => {
                let mut flat: Vec<(Lval, u32)> = Vec::new();
                for p in parts {
                    let sub = self.lower_lvalue(p)?;
                    let w = lval_width_of(&sub, self.e);
                    match sub {
                        Lval::Concat { parts: inner } => flat.extend(inner),
                        other => flat.push((other, w)),
                    }
                }
                Ok(Lval::Concat { parts: flat })
            }
            Expr::IndexedPart { span, .. } => Err(self.err(
                *span,
                "indexed part-selects (`+:`/`-:`) are not supported yet (see docs/RIVERILOG-PLAN.md §8)",
            )),
            Expr::HierIdent { parts, span } => {
                let v = self.hier_var(parts.join(".").as_str(), *span)?;
                Ok(Lval::Var { var: v })
            }
            Expr::HierDot { base, name, span } => {
                let p = self.hier_base_path(base)
                    .ok_or_else(|| self.err(*span, "hierarchical base must be a scope path here"))?;
                let v = self.hier_var(format!("{p}.{name}").as_str(), *span)?;
                Ok(Lval::Var { var: v })
            }
            other => Err(self.err(
                other.span(),
                "expression is not a valid assignment target",
            )),
        }
    }

    fn lower_syscall(&mut self, span: rcs_diag::Span, name: &str, args: &[Expr]) -> ElabResult<()> {
        let line = self.line_of(span);
        let file_idx = self.pool_file_idx();
        match name {
            "$display" | "$write" => {
                let (pieces, arg_exprs): (Vec<FmtPiece>, &[Expr]) = match args.first() {
                    None => (Vec::new(), &[]),
                    Some(Expr::StrLit { value, .. }) => (
                        fmt::compile(value, self.file, line)?,
                        &args[1..],
                    ),
                    Some(_) => {
                        return Err(self.err(
                            span,
                            "display format must be a string literal (dynamic formats not supported)",
                        ))
                    }
                };
                let nspec = pieces
                    .iter()
                    .filter(|p| matches!(p, FmtPiece::Spec { .. }))
                    .count();
                if arg_exprs.len() < nspec {
                    return Err(self.err(
                        span,
                        format!("{name}: format needs {nspec} arguments, got {}", arg_exprs.len()),
                    ));
                }
                if arg_exprs.len() > nspec {
                    return Err(self.err(
                        span,
                        format!("{name}: {} extra arguments beyond the format", arg_exprs.len() - nspec),
                    ));
                }
                let mut ir_args = Vec::new();
                for a in arg_exprs {
                    let v = self.lower(a, 0)?;
                    ir_args.push(riv_ir::Arg {
                        slot: v.slot,
                        width: v.w,
                        signed: v.signed,
                        is_time: v.is_time,
                    });
                }
                let fmt_idx = self.pool_str(pieces);
                let id = if name == "$display" { SysId::Display } else { SysId::Write };
                self.emit(Op::SysCall {
                    id,
                    fmt: Some(fmt_idx),
                    args: ir_args,
                    dst: None,
                    file: file_idx,
                    line,
                    start: 0,
                });
                Ok(())
            }
            "$finish" => {
                if !args.is_empty() {
                    return Err(self.err(
                        span,
                        "$finish with a verbosity argument is not supported (see docs/RIVERILOG-PLAN.md §8)",
                    ));
                }
                self.emit(Op::SysCall { id: SysId::Finish, fmt: None, args: Vec::new(), dst: None, file: file_idx, line, start: 0 });
                Ok(())
            }
            "$dumpfile" => {
                let [arg] = args else {
                    return Err(self.err(span, "$dumpfile takes exactly one argument"));
                };
                let Expr::StrLit { value, .. } = arg else {
                    return Err(self.err(span, "$dumpfile argument must be a string literal"));
                };
                let idx = self.pool_str(vec![FmtPiece::Text(value.clone())]);
                self.emit(Op::SysCall { id: SysId::Dumpfile, fmt: Some(idx), args: Vec::new(), dst: None, file: file_idx, line, start: 0 });
                Ok(())
            }
            "$dumpvars" => {
                // Minimal dump: level/scope arguments are accepted and
                // ignored (flat designs dump everything).
                self.emit(Op::SysCall { id: SysId::Dumpvars, fmt: None, args: Vec::new(), dst: None, file: file_idx, line, start: 0 });
                Ok(())
            }
            "$monitor" | "$strobe" | "$monitoron" | "$monitoroff" => {
                // Monitor-family args are RE-EVALUATED in the monitor region
                // (design §4: $time and variable reads must be fresh at
                // render time), so the syscall records where its
                // argument-producer ops begin (`start`); the kernel
                // re-executes body[start..syscall_pc] in the thread's frame.
                let ops_start = self.here();
                match name {
                    "$monitoron" | "$monitoroff" => {
                        if !args.is_empty() {
                            return Err(self.err(span, format!("{name} takes no arguments")));
                        }
                        let id = if name == "$monitoron" { SysId::MonitorOn } else { SysId::MonitorOff };
                        self.emit(Op::SysCall { id, fmt: None, args: vec![], dst: None, file: file_idx, line, start: 0 });
                        return Ok(());
                    }
                    "$monitor" | "$strobe" => {}
                    _ => unreachable!(),
                }
                let (pieces, arg_exprs): (Vec<FmtPiece>, &[Expr]) = match args.first() {
                    None => (Vec::new(), &[]),
                    Some(Expr::StrLit { value, .. }) => (
                        fmt::compile(value, self.file, line)?,
                        &args[1..],
                    ),
                    Some(_) => {
                        return Err(self.err(
                            span,
                            "display format must be a string literal (dynamic formats not supported)",
                        ))
                    }
                };
                let nspec = pieces
                    .iter()
                    .filter(|p| matches!(p, FmtPiece::Spec { .. }))
                    .count();
                if arg_exprs.len() != nspec {
                    return Err(self.err(
                        span,
                        format!("{name}: format needs {nspec} arguments, got {}", arg_exprs.len()),
                    ));
                }
                let mut ir_args = Vec::new();
                for a in arg_exprs {
                    let v = self.lower(a, 0)?;
                    ir_args.push(riv_ir::Arg {
                        slot: v.slot,
                        width: v.w,
                        signed: v.signed,
                        is_time: v.is_time,
                    });
                }
                let fmt_idx = self.pool_str(pieces);
                let id = if name == "$monitor" { SysId::Monitor } else { SysId::Strobe };
                self.emit(Op::SysCall {
                    id,
                    fmt: Some(fmt_idx),
                    args: ir_args,
                    dst: None,
                    file: file_idx,
                    line,
                    start: ops_start,
                });
                Ok(())
            }
            "$stop" => {
                // rivD2: default = banner + CONTINUE (EOF semantics); -n in
                // rivvp makes $stop equivalent to $finish.
                self.emit(Op::SysCall { id: SysId::Stop, fmt: None, args: Vec::new(), dst: None, file: file_idx, line, start: 0 });
                Ok(())
            }
            "$readmemh" | "$readmemb" => {
                // $readmemh("file", mem[, start[, finish]]) — file name in
                // the fmt pool; start/finish lowered into arg slots with
                // width 0 marking absence.
                let id = if name == "$readmemh" { SysId::ReadmemH } else { SysId::ReadmemB };
                let Some(Expr::StrLit { value, .. }) = args.first() else {
                    return Err(self.err(span, format!("{name} first argument must be a file name string")));
                };
                let Some(Expr::Ident { name: mem, span: mspan }) = args.get(1) else {
                    return Err(self.err(span, format!("{name} second argument must be a memory")));
                };
                let Some(NameKind::Var(mv)) = self.lookup(mem) else {
                    return Err(self.err(*mspan, format!("identifier `{mem}` is not declared")));
                };
                if self.e.vars[mv as usize].kind != riv_ir::VarKind::Array {
                    return Err(self.err(*mspan, format!("`{mem}` is not a memory")));
                }
                let fidx = self.pool_str(vec![FmtPiece::Text(value.clone())]);
                let mut ir_args = vec![riv_ir::Arg { slot: mv as u32, width: 0, signed: false, is_time: false }];
                for bound in args.get(2..4).unwrap_or(&[]) {
                    match bound {
                        Expr::NumLit(_) | Expr::Ident { .. } => {
                            let v = self.lower(bound, 0)?;
                            ir_args.push(riv_ir::Arg { slot: v.slot, width: 32, signed: true, is_time: false });
                        }
                        _ => return Err(self.err(span, "readmem bounds must be constants")),
                    }
                }
                while ir_args.len() < 3 {
                    let dst = self.slot();
                    self.emit(Op::Const { dst, lit: riv_ir::Literal { width: 32, signed: true, base: riv_ir::Base::Dec, digits: "0".into() } });
                    ir_args.push(riv_ir::Arg { slot: dst, width: 0, signed: true, is_time: false });
                }
                self.emit(Op::SysCall { id, fmt: Some(fidx), args: ir_args, dst: None, file: file_idx, line, start: 0 });
                Ok(())
            }
            "$dumpall" | "$dumpoff" | "$dumpon" | "$dumpflush" | "$dumplimit" => Err(self.err(
                span,
                format!("`{name}` lands with the dump batch (milestone R5)"),
            )),
            "$fopen" | "$fwrite" | "$fdisplay" | "$fclose" | "$fmonitor" | "$sdf_annotate" => {
                Err(self.err(
                    span,
                    format!("`{name}` lands with the IO batch (milestone R5)"),
                ))
            }
            "$time" | "$realtime" => Err(self.err(
                span,
                format!("`{name}` lands with the $time batch (milestone R3)"),
            )),
            other => Err(self.err(
                span,
                format!("system task `{other}` is not supported (see docs/RIVERILOG-PLAN.md §8)"),
            )),
        }
    }

    fn pool_file_idx(&mut self) -> u32 {
        // One pool entry per file (dedup by scan; thread counts are small).
        for (i, pieces) in self.strs.iter().enumerate() {
            if pieces.len() == 1 {
                if let FmtPiece::Text(t) = &pieces[0] {
                    if t == self.file {
                        return i as u32;
                    }
                }
            }
        }
        self.pool_str(vec![FmtPiece::Text(self.file.to_string())])
    }
}

fn lval_base_var(lv: &Lval) -> Option<VarId> {
    match lv {
        Lval::Var { var }
        | Lval::Bit { var, .. }
        | Lval::Part { var, .. }
        | Lval::PartDyn { var, .. }
        | Lval::Elem { var, .. } => Some(*var),
        Lval::Concat { parts } => parts.first().map(|(p, _)| lval_base_var(p)).flatten(),
    }
}

fn lval_width_of(lv: &Lval, e: &super::Elab) -> u32 {
    match lv {
        Lval::Var { var } => e.vars[*var as usize].width,
        Lval::Bit { .. } => 1,
        Lval::Part { hi, lo, .. } => hi.abs_diff(*lo) + 1,
        Lval::PartDyn { .. } => 1,
        Lval::Elem { var, .. } => e.vars[*var as usize].width,
        Lval::Concat { parts } => parts.iter().map(|(_, w)| *w).sum(),
    }
}

/// Shift branch targets by `off` (used when splicing lowered bodies).
fn rebase(ops: &mut [Op], off: u32) {
    for op in ops.iter_mut() {
        match op {
            Op::Branch { then, els, .. } => {
                *then += off;
                *els += off;
            }
            Op::Jump { to } => *to += off,
            _ => {}
        }
    }
}
