//! Expression evaluation (IEEE 1364-2005 §5 width/sign rules, 4-state).

use crate::value::{self, Bits, ValueError};
use rcs_diag::Span;
use vl_syntax::ast::{BinOp, Expr, UnOp};

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

/// Name resolution + declared ranges for the evaluator.
pub trait Lookup {
    fn lookup(&mut self, name: &str, span: Span) -> SResult<Val>;
    /// Declared `[msb:lsb]` of a vector, if any (scalars → None).
    fn decl_range(&mut self, name: &str) -> Option<(i64, i64)>;
}

fn bits_of(v: Val, span: Span) -> SResult<(Bits, bool)> {
    match v {
        Val::Bits { bits, signed } => Ok((bits, signed)),
        Val::Str(_) => Err(SimError::at(
            "string literal used where a bit-vector value is required",
            span,
        )),
    }
}

/// Extend both operands to a common width per §5.4 (signed only if both are).
fn align(a: Bits, sa: bool, b: Bits, sb: bool) -> SResult<(Bits, Bits, bool, u32)> {
    let w = a.width.max(b.width);
    let signed = sa && sb;
    let (a2, b2) = if signed {
        (a.sadjust(w)?, b.sadjust(w)?)
    } else {
        (a.uadjust(w)?, b.uadjust(w)?)
    };
    Ok((a2, b2, signed, w))
}

pub fn eval(e: &Expr, ctx: &mut dyn Lookup) -> SResult<Val> {
    match e {
        Expr::Ident { span, name } => ctx.lookup(name, *span),
        Expr::HierIdent { span, parts } => ctx.lookup(&parts.join("."), *span),
        Expr::NumLit(n) => Ok(Val::Bits {
            bits: value::from_numlit(n).map_err(|e| SimError::at(e.0, n.span))?,
            signed: n.signed,
        }),
        Expr::RealLit { span, .. } => Err(SimError::at(
            "real-number literals are not yet supported (milestone M7)",
            *span,
        )),
        Expr::StrLit { value, .. } => Ok(Val::Str(value.clone())),
        Expr::Unary { span, op, expr } => {
            let (b, signed) = bits_of(eval(expr, ctx)?, *span)?;
            let r = match op {
                UnOp::Not => match b.truthy()? {
                    Some(v) => Val::bool1(!v),
                    None => Val::unsigned(Bits::all_x(1)?),
                },
                UnOp::BitNot => Val::Bits {
                    bits: b.bit_not(),
                    signed,
                },
                UnOp::Plus => Val::Bits { bits: b, signed },
                UnOp::Minus => Val::Bits {
                    bits: Bits::zeros(b.width)?.sub(&b)?,
                    signed,
                },
                UnOp::And => Val::unsigned(b.reduce_and()?),
                UnOp::Nand => Val::unsigned(b.reduce_and()?.bit_not()),
                UnOp::Or => Val::unsigned(b.reduce_or()?),
                UnOp::Nor => Val::unsigned(b.reduce_or()?.bit_not()),
                UnOp::Xor => Val::unsigned(b.reduce_xor()?),
                UnOp::Xnor => Val::unsigned(b.reduce_xor()?.bit_not()),
            };
            Ok(r)
        }
        Expr::Binary { span, op, lhs, rhs } => {
            // Logical ops short-circuit on known operands (§5.1.9).
            if matches!(op, BinOp::LogAnd | BinOp::LogOr) {
                let (a, _) = bits_of(eval(lhs, ctx)?, *span)?;
                let ta = a.truthy()?;
                if *op == BinOp::LogAnd && ta == Some(false) {
                    return Ok(Val::bool1(false));
                }
                if *op == BinOp::LogOr && ta == Some(true) {
                    return Ok(Val::bool1(true));
                }
                let (b, _) = bits_of(eval(rhs, ctx)?, *span)?;
                let tb = b.truthy()?;
                return Ok(match (op, ta, tb) {
                    (BinOp::LogAnd, Some(true), Some(v)) => Val::bool1(v),
                    (BinOp::LogAnd, _, Some(false)) => Val::bool1(false),
                    (BinOp::LogOr, Some(false), Some(v)) => Val::bool1(v),
                    (BinOp::LogOr, _, Some(true)) => Val::bool1(true),
                    _ => Val::unsigned(Bits::all_x(1)?),
                });
            }
            let (a, sa) = bits_of(eval(lhs, ctx)?, *span)?;
            let (b, sb) = bits_of(eval(rhs, ctx)?, *span)?;
            let r = match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod | BinOp::Pow => {
                    let (a2, b2, signed, _) = align(a, sa, b, sb)?;
                    let bits = match op {
                        BinOp::Add => a2.add(&b2)?,
                        BinOp::Sub => a2.sub(&b2)?,
                        BinOp::Mul => a2.mul(&b2)?,
                        BinOp::Div => {
                            if signed && a2.is_known() && b2.is_known() && b2.to_u64_lossy() != 0 {
                                let q = a2.to_i64_lossy().wrapping_div(b2.to_i64_lossy());
                                Bits::from_u64(a2.width, (q as u64) & value::low_bits(a2.width))?
                            } else {
                                a2.div(&b2)?
                            }
                        }
                        BinOp::Mod => {
                            if signed && a2.is_known() && b2.is_known() && b2.to_u64_lossy() != 0 {
                                let q = a2.to_i64_lossy().wrapping_rem(b2.to_i64_lossy());
                                Bits::from_u64(a2.width, (q as u64) & value::low_bits(a2.width))?
                            } else {
                                a2.rem(&b2)?
                            }
                        }
                        _ => a2.pow(&b2)?,
                    };
                    Val::Bits { bits, signed }
                }
                BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
                    let (a2, b2, signed, _) = align(a, sa, b, sb)?;
                    let bits = match op {
                        BinOp::BitAnd => a2.bit_and(&b2)?,
                        BinOp::BitOr => a2.bit_or(&b2)?,
                        _ => a2.bit_xor(&b2)?,
                    };
                    Val::Bits { bits, signed }
                }
                BinOp::Eq | BinOp::Ne => {
                    let (a2, b2, _, _) = align(a, sa, b, sb)?;
                    let r = a2.logic_eq(&b2)?;
                    Val::unsigned(if *op == BinOp::Ne { r.bit_not() } else { r })
                }
                BinOp::EqCase | BinOp::NeCase => {
                    let (a2, b2, _, _) = align(a, sa, b, sb)?;
                    let r = a2.case_eq(&b2)?;
                    Val::unsigned(if *op == BinOp::NeCase { r.bit_not() } else { r })
                }
                BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                    let (a2, b2, signed, _) = align(a, sa, b, sb)?;
                    if a2.has_unknown() || b2.has_unknown() {
                        Val::unsigned(Bits::all_x(1)?)
                    } else {
                        let res = if signed {
                            let (x, y) = (a2.to_i64_lossy(), b2.to_i64_lossy());
                            match op {
                                BinOp::Lt => x < y,
                                BinOp::Le => x <= y,
                                BinOp::Gt => x > y,
                                _ => x >= y,
                            }
                        } else {
                            let (x, y) = (a2.to_u64_lossy(), b2.to_u64_lossy());
                            match op {
                                BinOp::Lt => x < y,
                                BinOp::Le => x <= y,
                                BinOp::Gt => x > y,
                                _ => x >= y,
                            }
                        };
                        Val::bool1(res)
                    }
                }
                BinOp::Shl | BinOp::AShl => Val::Bits {
                    bits: a.shl(&b)?,
                    signed: sa,
                },
                BinOp::Shr => Val::Bits {
                    bits: a.shr(&b)?,
                    signed: sa,
                },
                BinOp::AShr => Val::Bits {
                    bits: a.ashr(&b, sa)?,
                    signed: sa,
                },
                BinOp::LogAnd | BinOp::LogOr => unreachable!(),
            };
            Ok(r)
        }
        Expr::Ternary {
            span,
            cond,
            then,
            els,
        } => {
            let (c, _) = bits_of(eval(cond, ctx)?, *span)?;
            match c.truthy()? {
                Some(true) => eval(then, ctx),
                Some(false) => eval(els, ctx),
                None => {
                    let (t, st) = bits_of(eval(then, ctx)?, *span)?;
                    let (e, se) = bits_of(eval(els, ctx)?, *span)?;
                    let (t2, e2, signed, _) = align(t, st, e, se)?;
                    Ok(Val::Bits {
                        bits: t2.merge(&e2)?,
                        signed,
                    })
                }
            }
        }
        Expr::Concat { span, parts } => {
            let mut bs = Vec::with_capacity(parts.len());
            for p in parts {
                bs.push(bits_of(eval(p, ctx)?, *span)?.0);
            }
            Ok(Val::unsigned(Bits::concat(&bs)?))
        }
        Expr::Repeat { span, count, value } => {
            let (c, _) = bits_of(eval(count, ctx)?, *span)?;
            if c.has_unknown() {
                return Err(SimError::at("replication count is unknown (x/z)", *span));
            }
            let (v, _) = bits_of(eval(value, ctx)?, *span)?;
            Ok(Val::unsigned(v.replicate(c.to_u64_lossy())?))
        }
        Expr::Index { span, base, index } => {
            let (b, _) = bits_of(eval(base, ctx)?, *span)?;
            let (i, _) = bits_of(eval(index, ctx)?, *span)?;
            if i.has_unknown() {
                return Ok(Val::unsigned(Bits::all_x(1)?));
            }
            let pos = bit_position(base, i.to_i64_lossy(), ctx);
            match pos {
                Some(p) if p >= 0 && (p as u32) < b.width => {
                    Ok(Val::unsigned(b.part(p as u32, p as u32)?))
                }
                _ => Ok(Val::unsigned(Bits::all_x(1)?)), // out of range → x (§5.2.1)
            }
        }
        Expr::PartSelect {
            span,
            base,
            msb,
            lsb,
        } => {
            let (b, _) = bits_of(eval(base, ctx)?, *span)?;
            let (m, _) = bits_of(eval(msb, ctx)?, *span)?;
            let (l, _) = bits_of(eval(lsb, ctx)?, *span)?;
            if m.has_unknown() || l.has_unknown() {
                return Err(SimError::at("part-select bounds must be constant", *span));
            }
            let hi = bit_position(base, m.to_i64_lossy(), ctx);
            let lo = bit_position(base, l.to_i64_lossy(), ctx);
            match (hi, lo) {
                (Some(h), Some(l)) if h >= 0 && l >= 0 && (h.max(l) as u32) < b.width => {
                    Ok(Val::unsigned(b.part(h as u32, l as u32)?))
                }
                _ => {
                    let w = (m.to_i64_lossy() - l.to_i64_lossy()).unsigned_abs() as u32 + 1;
                    Ok(Val::unsigned(Bits::all_x(w)?))
                }
            }
        }
        Expr::IndexedPart { span, .. } => Err(SimError::at(
            "indexed part-selects `+:`/`-:` are not yet supported (milestone M7)",
            *span,
        )),
        Expr::Call { span, func, args } => match func.as_str() {
            "$clog2" => {
                let a = args.first().ok_or_else(|| SimError::at("$clog2 needs 1 argument", *span))?;
                let (b, _) = bits_of(eval(a, ctx)?, *span)?;
                if b.has_unknown() {
                    return Ok(Val::unsigned(Bits::all_x(32)?));
                }
                let v = b.to_u64_lossy();
                let r = if v <= 1 { 0 } else { 64 - (v - 1).leading_zeros() as u64 };
                Ok(Val::unsigned(Bits::from_u64(32, r)?))
            }
            "$time" | "$stime" | "$realtime" => {
                // v0 slice has no time model; M5 wires the scheduler clock.
                Ok(Val::unsigned(Bits::from_u64(64, 0)?))
            }
            "$signed" | "$unsigned" => {
                let a = args.first().ok_or_else(|| SimError::at(format!("{func} needs 1 argument"), *span))?;
                let (b, _) = bits_of(eval(a, ctx)?, *span)?;
                Ok(Val::Bits {
                    bits: b,
                    signed: func == "$signed",
                })
            }
            _ if func.starts_with('$') => Err(SimError::at(
                format!("system function `{func}` is not yet supported"),
                *span,
            )),
            _ => Err(SimError::at(
                format!("user function `{func}` calls are not yet supported (milestone M4)"),
                *span,
            )),
        },
    }
}

/// Map a Verilog index onto a bit position using the declared `[msb:lsb]`
/// of the base identifier (`[7:0]` → index 3 is bit 3; `[0:7]` → index 3 is
/// bit 4). Non-identifier bases are 0-based ascending.
fn bit_position(base: &Expr, index: i64, ctx: &mut dyn Lookup) -> Option<i64> {
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
