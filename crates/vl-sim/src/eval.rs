//! Expression evaluation with IEEE 1364-2005 §5.4/§5.5 width and sign
//! rules: context-determined operands are coerced to the expression width
//! (sign-extended only when the whole expression is signed), self-determined
//! operands (comparison sides, shift amounts, concat parts, conditions) start
//! a fresh context.

use crate::value::{self, Bits, ValueError};
use rcs_diag::Span;
use vl_syntax::ast::{BinOp, Expr, UnOp};

// Operator application (the per-op IEEE §5 semantics: coercion + truth
// tables) is delegated to vl-ops — the single source shared with the
// unified engine's runtime and const interpreter (ADR-0009 U-2). This
// file keeps only AST-level walking: width/sign propagation, name
// resolution, short-circuit evaluation order.
use vl_ops as vops;

#[derive(Clone, Debug)]
pub enum Val {
    Bits { bits: Bits, signed: bool },
    Str(String),
}

impl Val {
    pub fn unsigned(bits: Bits) -> Val {
        Val::Bits {
            bits,
            signed: false,
        }
    }
    pub fn bool1(b: bool) -> Val {
        Val::unsigned(Bits::from_u64(1, b as u64).expect("1-bit"))
    }
    /// Bits view (strings pack as ASCII, MSB = first char; §2.6.1).
    pub fn to_bits(&self, span: Span) -> SResult<(Bits, bool)> {
        match self {
            Val::Bits { bits, signed } => Ok((bits.clone(), *signed)),
            Val::Str(s) => Ok((str_bits(s, span)?, false)),
        }
    }
}

pub fn str_bits(s: &str, span: Span) -> SResult<Bits> {
    if s.len() > 8 {
        return Err(SimError::at(
            "string literals longer than 8 characters are not yet supported in expressions (M7)",
            span,
        ));
    }
    let mut v: u64 = 0;
    for b in s.bytes() {
        v = (v << 8) | b as u64;
    }
    Ok(Bits::from_u64((s.len() as u32 * 8).max(8), v)?)
}

#[derive(Debug, Clone)]
pub struct SimError {
    pub message: String,
    pub span: Option<Span>,
}

impl SimError {
    pub fn new(message: impl Into<String>, span: Option<Span>) -> Self {
        SimError {
            message: message.into(),
            span,
        }
    }
    pub fn at(message: impl Into<String>, span: Span) -> Self {
        SimError::new(message, Some(span))
    }
}

impl From<ValueError> for SimError {
    fn from(e: ValueError) -> Self {
        SimError::new(e.0, None)
    }
}

pub type SResult<T> = Result<T, SimError>;

/// Name resolution + declared ranges + calls for the evaluator.
pub trait Lookup {
    fn lookup(&mut self, name: &str, span: Span) -> SResult<Val>;
    /// Declared `[msb:lsb]` of a vector, if any (scalars → None).
    fn decl_range(&mut self, name: &str) -> Option<(i64, i64)>;
    /// `(width, signed)` of a declared variable/parameter.
    fn var_info(&mut self, name: &str) -> Option<(u32, bool)>;
    /// `(return width, signed)` of a user function, if declared.
    fn function_info(&mut self, name: &str) -> Option<(u32, bool)>;
    fn call_function(&mut self, name: &str, args: &[Val], span: Span) -> SResult<Val>;
    /// Unpacked-array (memory) info if `name` is one:
    /// `(address of word 0, word count, element width, element signed)`.
    fn mem_info(&mut self, name: &str) -> Option<(i64, usize, u32, bool)>;
    /// Read one bounds-checked memory word.
    fn read_mem_word(&mut self, name: &str, word: usize) -> SResult<Bits>;
    /// Current simulation time in precision units, and unit/precision ratio.
    fn now(&self) -> (u64, u64);
}

/// If `e` is a bare (possibly hierarchical) identifier, its canonical name —
/// the only base form that can denote a memory word select.
pub fn mem_base_name(e: &Expr) -> Option<String> {
    match e {
        Expr::Ident { name, .. } => Some(name.clone()),
        Expr::HierIdent { parts, .. } => Some(parts.join(".")),
        _ => None,
    }
}

fn sys_func_info(func: &str) -> Option<(u32, bool)> {
    match func {
        "$clog2" | "$bits" => Some((32, true)),
        "$time" | "$stime" => Some((64, false)),
        "$realtime" => Some((64, false)),
        "$random" | "$urandom" => Some((32, true)),
        _ => None,
    }
}

/// Self-determined width (IEEE 1364-2005 Table 5-22).
pub fn nat_width(e: &Expr, ctx: &mut dyn Lookup) -> SResult<u32> {
    Ok(match e {
        Expr::Ident { span, name } => ctx
            .var_info(name)
            .ok_or_else(|| SimError::at(format!("undeclared identifier `{name}`"), *span))?
            .0,
        Expr::HierIdent { span, parts } => {
            let n = parts.join(".");
            ctx.var_info(&n)
                .ok_or_else(|| SimError::at(format!("undeclared identifier `{n}`"), *span))?
                .0
        }
        Expr::NumLit(n) => value::from_numlit(n).map_err(|e| SimError::at(e.0, n.span))?.width,
        Expr::RealLit { span, .. } => {
            return Err(SimError::at("real literals land at milestone M7", *span))
        }
        Expr::StrLit { value, .. } => (value.len() as u32 * 8).max(8),
        Expr::Unary { op, expr, .. } => match op {
            UnOp::Not | UnOp::And | UnOp::Nand | UnOp::Or | UnOp::Nor | UnOp::Xor | UnOp::Xnor => 1,
            UnOp::BitNot | UnOp::Plus | UnOp::Minus => nat_width(expr, ctx)?,
        },
        Expr::Binary { op, lhs, rhs, .. } => match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod
            | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
                nat_width(lhs, ctx)?.max(nat_width(rhs, ctx)?)
            }
            BinOp::Pow | BinOp::Shl | BinOp::Shr | BinOp::AShl | BinOp::AShr => nat_width(lhs, ctx)?,
            BinOp::Eq | BinOp::Ne | BinOp::EqCase | BinOp::NeCase | BinOp::Lt | BinOp::Le
            | BinOp::Gt | BinOp::Ge | BinOp::LogAnd | BinOp::LogOr => 1,
        },
        Expr::Ternary { then, els, .. } => nat_width(then, ctx)?.max(nat_width(els, ctx)?),
        Expr::Delay3 { span, .. } => {
            return Err(SimError::at(
                "min:typ:max delay literal is only valid in a gate delay",
                *span,
            ))
        }
        Expr::Concat { parts, .. } => {
            let mut w = 0;
            for p in parts {
                w += nat_width(p, ctx)?;
            }
            w
        }
        Expr::Repeat { span, count, value } => {
            let c = const_u64(count, ctx, *span)?;
            let total = nat_width(value, ctx)? as u64 * c;
            if total > value::MAX_WIDTH as u64 {
                return Err(SimError::at(
                    format!("repetition width {total} exceeds the {}-bit limit", value::MAX_WIDTH),
                    *span,
                ));
            }
            total as u32
        }
        Expr::HierDot { span, .. } => {
            let _ = span;
            1
        }
        Expr::Index { span, base, .. } => {
            // Memory word select: self-determined width = element width (§5.2).
            if let Some(mn) = mem_base_name(base) {
                if let Some((_, _, w, _)) = ctx.mem_info(&mn) {
                    return Ok(w);
                }
            }
            let _ = span;
            1
        }
        Expr::PartSelect { span, msb, lsb, .. } => {
            let m = const_i64_expr(msb, ctx, *span)?;
            let l = const_i64_expr(lsb, ctx, *span)?;
            value::range_width(m, l).map_err(|e| SimError::at(e.0, *span))?
        }
        Expr::IndexedPart { span, width, .. } => const_u64(width, ctx, *span)? as u32,
        Expr::Call { span, func, args } => match func.as_str() {
            "$signed" | "$unsigned" => {
                let a = args
                    .first()
                    .ok_or_else(|| SimError::at(format!("{func} needs 1 argument"), *span))?;
                nat_width(a, ctx)?
            }
            f if f.starts_with('$') => {
                sys_func_info(f)
                    .ok_or_else(|| SimError::at(format!("system function `{f}` is not yet supported"), *span))?
                    .0
            }
            f => ctx
                .function_info(f)
                .ok_or_else(|| SimError::at(format!("unknown function `{f}`"), *span))?
                .0,
        },
    })
}

/// Expression signedness (§5.5.1): any unsigned operand → unsigned.
pub fn is_signed(e: &Expr, ctx: &mut dyn Lookup) -> SResult<bool> {
    Ok(match e {
        Expr::Ident { name, .. } => ctx.var_info(name).map(|v| v.1).unwrap_or(false),
        Expr::HierIdent { parts, .. } => ctx.var_info(&parts.join(".")).map(|v| v.1).unwrap_or(false),
        Expr::NumLit(n) => n.signed,
        Expr::RealLit { .. } => true,
        Expr::StrLit { .. } => false,
        Expr::Unary { op, expr, .. } => match op {
            UnOp::BitNot | UnOp::Plus | UnOp::Minus => is_signed(expr, ctx)?,
            _ => false,
        },
        Expr::Binary { op, lhs, rhs, .. } => match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod | BinOp::Pow
            | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
                is_signed(lhs, ctx)? && is_signed(rhs, ctx)?
            }
            BinOp::Shl | BinOp::Shr | BinOp::AShl | BinOp::AShr => is_signed(lhs, ctx)?,
            _ => false,
        },
        Expr::Ternary { then, els, .. } => is_signed(then, ctx)? && is_signed(els, ctx)?,
        Expr::HierDot { .. } => false,
        Expr::Index { base, .. } => {
            // Memory word select keeps the element's declared signedness;
            // plain bit-select results are unsigned (§5.5.1).
            match mem_base_name(base) {
                Some(mn) => ctx.mem_info(&mn).map(|(_, _, _, s)| s).unwrap_or(false),
                None => false,
            }
        }
        Expr::Delay3 { .. } => false,
        Expr::Concat { .. } | Expr::Repeat { .. } | Expr::PartSelect { .. }
        | Expr::IndexedPart { .. } => false,
        Expr::Call { func, .. } => match func.as_str() {
            "$signed" => true,
            "$unsigned" => false,
            f if f.starts_with('$') => sys_func_info(f).map(|v| v.1).unwrap_or(false),
            f => ctx.function_info(f).map(|v| v.1).unwrap_or(false),
        },
    })
}

/// Self-determined evaluation.
pub fn eval(e: &Expr, ctx: &mut dyn Lookup) -> SResult<Val> {
    if let Expr::StrLit { value, .. } = e {
        return Ok(Val::Str(value.clone()));
    }
    let w = nat_width(e, ctx)?;
    let s = is_signed(e, ctx)?;
    Ok(Val::Bits {
        bits: eval_w(e, ctx, w, s)?,
        signed: s,
    })
}

/// Context-determined evaluation for an assignment to a `lhs_w`-bit target:
/// the whole expression is evaluated at `max(lhs_w, natural width)` and the
/// result adjusted to `lhs_w`.
pub fn eval_ctx(e: &Expr, ctx: &mut dyn Lookup, lhs_w: u32) -> SResult<Val> {
    if let Expr::StrLit { value, .. } = e {
        return Ok(Val::Str(value.clone()));
    }
    let nw = nat_width(e, ctx)?;
    let s = is_signed(e, ctx)?;
    let w = lhs_w.max(nw);
    let bits = eval_w(e, ctx, w, s)?;
    let bits = if s { bits.sadjust(lhs_w)? } else { bits.uadjust(lhs_w)? };
    Ok(Val::Bits { bits, signed: s })
}

fn ext(b: Bits, signed: bool, w: u32) -> SResult<Bits> {
    if signed {
        b.sadjust(w)
    } else {
        b.uadjust(w)
    }
    .map_err(Into::into)
}

/// Evaluate `e` as a context-determined operand of width `w`; `signed` is the
/// propagated expression type (§5.5.1 step 2).
fn eval_w(e: &Expr, ctx: &mut dyn Lookup, w: u32, signed: bool) -> SResult<Bits> {
    match e {
        Expr::Ident { span, name } => {
            let (b, _) = ctx.lookup(name, *span)?.to_bits(*span)?;
            // A leaf is extended using its OWN signedness only if the whole
            // expression is signed (otherwise zero-extended).
            ext(b, signed, w)
        }
        Expr::HierIdent { span, parts } => {
            let (b, _) = ctx.lookup(&parts.join("."), *span)?.to_bits(*span)?;
            ext(b, signed, w)
        }
        Expr::NumLit(n) => {
            let b = value::from_numlit(n).map_err(|e| SimError::at(e.0, n.span))?;
            ext(b, signed, w)
        }
        Expr::RealLit { span, .. } => Err(SimError::at("real literals land at milestone M7", *span)),
        Expr::StrLit { span, value } => ext(str_bits(value, *span)?, false, w),
        Expr::Unary { span, op, expr } => match op {
            UnOp::Not => {
                let (b, _) = eval(expr, ctx)?.to_bits(*span)?;
                ext(vops::apply_logic_not(&b)?, false, w)
            }
            UnOp::BitNot => {
                // Context-determined (dual-oracle evidence, 2026-10-24
                // probe — see REGISTRY incident register 2026-10-24): the operand is
                // EVALUATED
                // at the expression width (nested ops see the context, e.g.
                // ~(4-bit + 4-bit) keeps the carry), coerced per the
                // expression signedness, THEN inverted. The 2026-10-23
                // "self-determined per Table 5-22" fix diverged from BOTH
                // oracles (unsigned shape) and is reverted here.
                let b = eval_w(expr, ctx, w, signed)?;
                Ok(vops::apply_bit_not(&b, w, signed)?)
            }
            UnOp::Plus => eval_w(expr, ctx, w, signed),
            UnOp::Minus => {
                let b = eval_w(expr, ctx, w, signed)?;
                let zero = Bits::zeros(w)?;
                Ok(vops::apply_bin(riv_ir::BinKind::Sub, &zero, &b, w, signed)?)
            }
            UnOp::And | UnOp::Nand | UnOp::Or | UnOp::Nor | UnOp::Xor | UnOp::Xnor => {
                let (b, _) = eval(expr, ctx)?.to_bits(*span)?;
                let kind = match op {
                    UnOp::And => riv_ir::RedKind::And,
                    UnOp::Nand => riv_ir::RedKind::Nand,
                    UnOp::Or => riv_ir::RedKind::Or,
                    UnOp::Nor => riv_ir::RedKind::Nor,
                    UnOp::Xor => riv_ir::RedKind::Xor,
                    _ => riv_ir::RedKind::Xnor,
                };
                ext(vops::apply_reduce(kind, &b)?, false, w)
            }
        },
        Expr::Binary { span, op, lhs, rhs } => match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod | BinOp::Pow => {
                let kind = match op {
                    BinOp::Add => riv_ir::BinKind::Add,
                    BinOp::Sub => riv_ir::BinKind::Sub,
                    BinOp::Mul => riv_ir::BinKind::Mul,
                    BinOp::Div => riv_ir::BinKind::Div,
                    BinOp::Mod => riv_ir::BinKind::Rem,
                    _ => riv_ir::BinKind::Pow,
                };
                if *op == BinOp::Pow {
                    // Exponent is self-determined (Table 5-22); apply_bin's
                    // value-preserving coercion to `w` cannot change it.
                    let (b, _) = eval(rhs, ctx)?.to_bits(*span)?;
                    let a = eval_w(lhs, ctx, w, signed)?;
                    Ok(vops::apply_bin(kind, &a, &b, w, signed)?)
                } else {
                    let a = eval_w(lhs, ctx, w, signed)?;
                    let b = eval_w(rhs, ctx, w, signed)?;
                    Ok(vops::apply_bin(kind, &a, &b, w, signed)?)
                }
            }
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
                let kind = match op {
                    BinOp::BitAnd => riv_ir::BitKind::And,
                    BinOp::BitOr => riv_ir::BitKind::Or,
                    _ => riv_ir::BitKind::Xor,
                };
                let a = eval_w(lhs, ctx, w, signed)?;
                let b = eval_w(rhs, ctx, w, signed)?;
                Ok(vops::apply_bit(kind, &a, &b, w, signed)?)
            }
            BinOp::Shl | BinOp::AShl | BinOp::Shr | BinOp::AShr => {
                let kind = match op {
                    BinOp::Shl | BinOp::AShl => riv_ir::ShiftKind::Shl,
                    BinOp::Shr => riv_ir::ShiftKind::Shr,
                    _ => riv_ir::ShiftKind::Ashr,
                };
                let a = eval_w(lhs, ctx, w, signed)?;
                let (amt, _) = eval(rhs, ctx)?.to_bits(*span)?;
                Ok(vops::apply_shift(kind, &a, &amt, w, signed)?)
            }
            BinOp::Eq | BinOp::Ne | BinOp::EqCase | BinOp::NeCase | BinOp::Lt | BinOp::Le
            | BinOp::Gt | BinOp::Ge => {
                // Comparison operands form their own context (§5.4.1).
                let cw = nat_width(lhs, ctx)?.max(nat_width(rhs, ctx)?);
                let cs = is_signed(lhs, ctx)? && is_signed(rhs, ctx)?;
                let a = eval_w(lhs, ctx, cw, cs)?;
                let b = eval_w(rhs, ctx, cw, cs)?;
                let r = match op {
                    BinOp::Eq => vops::apply_cmp(riv_ir::CmpKind::Eq, &a, &b, cs)?,
                    BinOp::Ne => vops::apply_cmp(riv_ir::CmpKind::Ne, &a, &b, cs)?,
                    BinOp::EqCase => vops::apply_case_cmp(riv_ir::CaseCmpKind::ExactEq, &a, &b)?,
                    BinOp::NeCase => vops::apply_case_cmp(riv_ir::CaseCmpKind::ExactNe, &a, &b)?,
                    _ => {
                        // Limb-aware compare — the pre-M8 paths truncated
                        // through i64 and diverged from the oracle at
                        // 65..=128 bits (live bug, pinned in value.rs tests).
                        let kind = match op {
                            BinOp::Lt => riv_ir::CmpKind::Lt,
                            BinOp::Le => riv_ir::CmpKind::Le,
                            BinOp::Gt => riv_ir::CmpKind::Gt,
                            _ => riv_ir::CmpKind::Ge,
                        };
                        vops::apply_cmp(kind, &a, &b, cs)?
                    }
                };
                ext(r, false, w)
            }
            BinOp::LogAnd | BinOp::LogOr => {
                // Short-circuit EVALUATION stays here (the rhs is not even
                // evaluated when the lhs decides); the value table —
                // including the x rows — is vl-ops'.
                let (a, _) = eval(lhs, ctx)?.to_bits(*span)?;
                let ta = a.truthy()?;
                if *op == BinOp::LogAnd && ta == Some(false) {
                    return ext(Bits::from_u64(1, 0)?, false, w);
                }
                if *op == BinOp::LogOr && ta == Some(true) {
                    return ext(Bits::from_u64(1, 1)?, false, w);
                }
                let (b, _) = eval(rhs, ctx)?.to_bits(*span)?;
                let r = if *op == BinOp::LogAnd {
                    vops::apply_logic_and(&a, &b)?
                } else {
                    vops::apply_logic_or(&a, &b)?
                };
                ext(r, false, w)
            }
        },
        Expr::Ternary {
            span,
            cond,
            then,
            els,
        } => {
            let (c, _) = eval(cond, ctx)?.to_bits(*span)?;
            match c.truthy()? {
                Some(true) => eval_w(then, ctx, w, signed),
                Some(false) => eval_w(els, ctx, w, signed),
                None => {
                    let t = eval_w(then, ctx, w, signed)?;
                    let e2 = eval_w(els, ctx, w, signed)?;
                    Ok(vops::apply_sel(&c, &t, &e2)?)
                }
            }
        }
        Expr::Delay3 { span, .. } => Err(SimError::at(
            "min:typ:max delay literal is only valid in a gate delay",
            *span,
        )),
        Expr::Concat { span, parts } => {
            let mut bs = Vec::with_capacity(parts.len());
            for p in parts {
                bs.push(eval(p, ctx)?.to_bits(*span)?.0);
            }
            ext(vops::apply_concat(&bs)?, false, w)
        }
        Expr::Repeat { span, count, value: v } => {
            let c = const_u64(count, ctx, *span)?;
            let (b, _) = eval(v, ctx)?.to_bits(*span)?;
            ext(vops::apply_replicate(&b, c)?, false, w)
        }
        Expr::HierDot { span, .. } => Err(SimError::at(
            "hierarchical reference reached evaluation unresolved",
            *span,
        )),
        Expr::Index { span, base, index } => {
            // Memory word select (§5.2): resolve against the memory table
            // before falling back to bit-select.
            if let Some(mn) = mem_base_name(base) {
                if let Some((lsb, words, ew, _esigned)) = ctx.mem_info(&mn) {
                    let (i, _) = eval(index, ctx)?.to_bits(index.span())?;
                    let r = if i.has_unknown() {
                        Bits::all_x(ew)?
                    } else {
                        // Word addresses are unsigned (§5.2): 2'b10 → word 2.
                        let wi = i.to_u64_lossy() as i64 - lsb;
                        if wi >= 0 && (wi as usize) < words {
                            ctx.read_mem_word(&mn, wi as usize)?
                        } else {
                            Bits::all_x(ew)? // out-of-range word → x
                        }
                    };
                    return Ok(ext(r, signed, w)?);
                }
            }
            let (b, _) = eval(base, ctx)?.to_bits(*span)?;
            let (i, _) = eval(index, ctx)?.to_bits(*span)?;
            let r = if i.has_unknown() {
                Bits::all_x(1)?
            } else {
                match bit_position(base, i.to_i64_lossy(), ctx) {
                    Some(p) if p >= 0 && (p as u32) < b.width => b.part(p as u32, p as u32)?,
                    _ => Bits::all_x(1)?, // out of range → x (§5.2.1)
                }
            };
            ext(r, false, w)
        }
        Expr::PartSelect {
            span,
            base,
            msb,
            lsb,
        } => {
            let (b, _) = eval(base, ctx)?.to_bits(*span)?;
            let m = const_i64_expr(msb, ctx, *span)?;
            let l = const_i64_expr(lsb, ctx, *span)?;
            let hi = bit_position(base, m, ctx);
            let lo = bit_position(base, l, ctx);
            let r = match (hi, lo) {
                (Some(h), Some(l2)) if h >= 0 && l2 >= 0 && (h.max(l2) as u32) < b.width => {
                    b.part(h as u32, l2 as u32)?
                }
                _ => Bits::all_x(value::range_width(m, l)?)?,
            };
            ext(r, false, w)
        }
        Expr::IndexedPart {
            span,
            base,
            offset,
            width,
            plus,
        } => {
            let (b, _) = eval(base, ctx)?.to_bits(*span)?;
            let (o, _) = eval(offset, ctx)?.to_bits(*span)?;
            let fw = const_u64(width, ctx, *span)? as u32;
            let r = if o.has_unknown() {
                Bits::all_x(fw)?
            } else {
                let o = o.to_i64_lossy();
                let (lo_idx, hi_idx) = if *plus { (o, o + fw as i64 - 1) } else { (o - fw as i64 + 1, o) };
                let hi = bit_position(base, hi_idx, ctx);
                let lo = bit_position(base, lo_idx, ctx);
                match (hi, lo) {
                    (Some(h), Some(l2)) if h >= 0 && l2 >= 0 && (h.max(l2) as u32) < b.width => {
                        b.part(h as u32, l2 as u32)?
                    }
                    _ => Bits::all_x(fw)?,
                }
            };
            ext(r, false, w)
        }
        Expr::Call { span, func, args } => {
            let r = match func.as_str() {
                "$bits" => {
                    let a = args.first().ok_or_else(|| SimError::at("$bits needs 1 argument", *span))?;
                    let w = nat_width(a, ctx)?;
                    Bits::from_u64(32, w as u64)?
                }
                "$clog2" => {
                    let a = args.first().ok_or_else(|| SimError::at("$clog2 needs 1 argument", *span))?;
                    let (b, _) = eval(a, ctx)?.to_bits(*span)?;
                    if b.has_unknown() {
                        Bits::all_x(32)?
                    } else {
                        Bits::from_u64(32, b.clog2().map_err(|e| SimError::at(e.0, *span))?)?
                    }
                }
                "$time" | "$stime" => {
                    let (now, ratio) = ctx.now();
                    let t = if ratio == 0 { now } else { (now + ratio / 2) / ratio };
                    Bits::from_u64(64, t)?
                }
                "$realtime" => {
                    let (now, ratio) = ctx.now();
                    Bits::from_u64(64, if ratio == 0 { now } else { now / ratio })?
                }
                "$signed" | "$unsigned" => {
                    let a = args.first().ok_or_else(|| SimError::at(format!("{func} needs 1 argument"), *span))?;
                    let (b, _) = eval(a, ctx)?.to_bits(*span)?;
                    // Re-typed operand: extend per the propagated expression type.
                    return ext(b, signed, w);
                }
                f if f.starts_with('$') => {
                    return Err(SimError::at(
                        format!("system function `{f}` is not yet supported"),
                        *span,
                    ))
                }
                f => {
                    let mut vals = Vec::with_capacity(args.len());
                    for a in args {
                        vals.push(eval(a, ctx)?);
                    }
                    let v = ctx.call_function(f, &vals, *span)?;
                    v.to_bits(*span)?.0
                }
            };
            ext(r, signed, w)
        }
    }
}

fn const_u64(e: &Expr, ctx: &mut dyn Lookup, span: Span) -> SResult<u64> {
    let (b, _) = eval(e, ctx)?.to_bits(span)?;
    if b.has_unknown() {
        return Err(SimError::at("constant expression contains x/z", span));
    }
    Ok(b.to_u64_lossy())
}

fn const_i64_expr(e: &Expr, ctx: &mut dyn Lookup, span: Span) -> SResult<i64> {
    let v = eval(e, ctx)?;
    let (b, signed) = v.to_bits(span)?;
    if b.has_unknown() {
        return Err(SimError::at("constant expression contains x/z", span));
    }
    Ok(if signed { b.to_i64_lossy() } else { b.to_u64_lossy() as i64 })
}

/// Map a Verilog index onto a bit position using the declared `[msb:lsb]`
/// of the base identifier (`[7:0]` → index 3 is bit 3; `[0:7]` → index 3 is
/// bit 4). Non-identifier bases are 0-based ascending.
pub fn bit_position(base: &Expr, index: i64, ctx: &mut dyn Lookup) -> Option<i64> {
    let name = match base {
        Expr::Ident { name, .. } => Some(name.clone()),
        Expr::HierIdent { parts, .. } => Some(parts.join(".")),
        _ => None,
    };
    match name.and_then(|n| ctx.decl_range(&n)) {
        Some((msb, lsb)) => {
            if msb >= lsb {
                Some(index - lsb)
            } else {
                Some(lsb - index)
            }
        }
        None => Some(index),
    }
}
