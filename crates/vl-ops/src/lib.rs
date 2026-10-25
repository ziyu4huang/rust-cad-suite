//! Lowered-op application semantics — the SINGLE source for what each
//! lowered operator means (IEEE 1364-2005 §5), shared by all three
//! execution surfaces (ADR-0009 U-2):
//!
//! - `riv-kernel` thread stepping and monitor-range re-evaluation (runtime),
//! - `riv-elab::interp` lowered-op const evaluation (params/ranges/labels),
//! - `vl-sim::eval` AST-expression arms (legacy face, mapped onto the same
//!   operator vocabulary).
//!
//! Width/sign coercion at the operand boundary (`adjust`, self-determined
//! shift amounts, comparison max-width pairing) lives here too, so a §5.4/§5.5
//! rule change is one edit, not four. The 4-state x/z truth tables are the
//! IEEE §5 track (AGENTS §2/§5): unit-tested here, independent of any
//! oracle diff.

use riv_ir::{Base, BinKind, BitKind, CaseCmpKind, CmpKind, Literal, RedKind, ShiftKind};
pub use vl_values::{Bits, ValueError, VResult};

/// Width coercion to `w` (zero-extend unsigned / sign-extend signed,
/// truncate on shrink; value-preserving).
pub fn adjust(b: &Bits, w: u32, signed: bool) -> VResult<Bits> {
    if signed {
        b.sadjust(w)
    } else {
        b.uadjust(w)
    }
}

/// Parse a lowered literal (digit string pre-verified by the front end).
pub fn parse_const(lit: &Literal) -> VResult<Bits> {
    vl_values::from_numlit_parts(
        Some(lit.width),
        lit.signed,
        match lit.base {
            Base::Dec => vl_values::Base::Dec,
            Base::Bin => vl_values::Base::Bin,
            Base::Oct => vl_values::Base::Oct,
            Base::Hex => vl_values::Base::Hex,
        },
        &lit.digits,
    )
}

/// `+ - * / % **` at context width `w`; `signed` selects the signed
/// division/remainder/power forms (§5.4.2/§5.4.3).
pub fn apply_bin(kind: BinKind, a: &Bits, b: &Bits, w: u32, signed: bool) -> VResult<Bits> {
    let av = adjust(a, w, signed)?;
    let bv = adjust(b, w, signed)?;
    match kind {
        BinKind::Add => av.add(&bv),
        BinKind::Sub => av.sub(&bv),
        BinKind::Mul => av.mul(&bv),
        BinKind::Div => {
            if signed {
                av.sdiv(&bv)
            } else {
                av.div(&bv)
            }
        }
        BinKind::Rem => {
            if signed {
                av.srem(&bv)
            } else {
                av.rem(&bv)
            }
        }
        BinKind::Pow => {
            if signed {
                av.spow(&bv)
            } else {
                av.pow(&bv)
            }
        }
    }
}

/// `& | ^` at context width `w`. Coercion follows the EXPRESSION type:
/// sign-extend only when the whole expression is signed — dual-oracle
/// evidence (2026-10-24 probe, verilator 5.050 + iverilog 13.0 agree):
/// `$signed(4'b1000) & $signed(4'b1001)` in an 8-bit context = `11111000`
/// (sign-extend both), while the unsigned-expression form = `00001000`.
pub fn apply_bit(kind: BitKind, a: &Bits, b: &Bits, w: u32, signed: bool) -> VResult<Bits> {
    let av = adjust(a, w, signed)?;
    let bv = adjust(b, w, signed)?;
    match kind {
        BitKind::And => av.bit_and(&bv),
        BitKind::Or => av.bit_or(&bv),
        BitKind::Xor => av.bit_xor(&bv),
    }
}

/// `~^`/`^~` — xor then invert (identical semantics). No lowering emitter
/// today (reserved op); unsigned coercion until one exists.
pub fn apply_bit_xnor(a: &Bits, b: &Bits, w: u32) -> VResult<Bits> {
    Ok(apply_bit(BitKind::Xor, a, b, w, false)?.bit_not())
}

/// `~` — context-determined: coerce the operand to the expression width
/// (per its signedness), THEN invert. Dual-oracle evidence (2026-10-24
/// probe, verilator + iverilog byte-identical on all four shapes):
/// unsigned ctx `~4'b1000` in 8 bits = `11110111` (zero-extend, invert),
/// signed ctx `~$signed(4'b1000)` = `00000111` (sign-extend, invert),
/// nested `~(4-bit + 4-bit)` keeps the carry. The 2026-10-23
/// "self-determined per Table 5-22" treatment diverged from BOTH oracles
/// on the unsigned shape and is reverted here (REGISTRY, U-2).
pub fn apply_bit_not(a: &Bits, w: u32, signed: bool) -> VResult<Bits> {
    Ok(adjust(a, w, signed)?.bit_not())
}

/// Reductions `& | ^` and negated forms (§5.2.2).
pub fn apply_reduce(kind: RedKind, a: &Bits) -> VResult<Bits> {
    match kind {
        RedKind::And => a.reduce_and(),
        RedKind::Or => a.reduce_or(),
        RedKind::Xor => a.reduce_xor(),
        RedKind::Nand => Ok(a.reduce_and()?.bit_not()),
        RedKind::Nor => Ok(a.reduce_or()?.bit_not()),
        RedKind::Xnor => Ok(a.reduce_xor()?.bit_not()),
    }
}

/// `&&` (IEEE Table 5-28 x rows): 0 dominates, 1 passes the other side
/// through, unknown-vs-1 is x.
pub fn apply_logic_and(a: &Bits, b: &Bits) -> VResult<Bits> {
    logic_pair(a, b, true)
}

/// `||`: 1 dominates, 0 passes the other side through, unknown-vs-0 is x.
pub fn apply_logic_or(a: &Bits, b: &Bits) -> VResult<Bits> {
    logic_pair(a, b, false)
}

fn logic_pair(a: &Bits, b: &Bits, is_and: bool) -> VResult<Bits> {
    let ta = a.truthy()?;
    if is_and && ta == Some(false) {
        return Bits::from_u64(1, 0);
    }
    if !is_and && ta == Some(true) {
        return Bits::from_u64(1, 1);
    }
    let tb = b.truthy()?;
    match (is_and, ta, tb) {
        (true, Some(true), Some(v)) => Bits::from_u64(1, v as u64),
        (true, _, Some(false)) => Bits::from_u64(1, 0),
        (false, Some(false), Some(v)) => Bits::from_u64(1, v as u64),
        (false, _, Some(true)) => Bits::from_u64(1, 1),
        _ => Bits::all_x(1),
    }
}

/// `!` — x/z invert to x.
pub fn apply_logic_not(a: &Bits) -> VResult<Bits> {
    Ok(match a.truthy()? {
        Some(v) => Bits::from_u64(1, (!v) as u64)?,
        None => Bits::all_x(1)?,
    })
}

/// Relational/equality `< <= > >= == !=` (§5.4.1/§5.4.2): operands pair at
/// max width (sign-extended only when BOTH sides signed); any unknown bit
/// (after ===-free comparison) yields x.
pub fn apply_cmp(kind: CmpKind, a: &Bits, b: &Bits, signed: bool) -> VResult<Bits> {
    let w = a.width.max(b.width);
    let av = adjust(a, w, signed)?;
    let bv = adjust(b, w, signed)?;
    let ord = if signed {
        av.cmp_signed(&bv)
    } else {
        av.cmp_unsigned(&bv)
    };
    Ok(match (kind, ord) {
        (_, None) => Bits::all_x(1)?,
        (CmpKind::Lt, Some(o)) => Bits::from_u64(1, o.is_lt() as u64)?,
        (CmpKind::Le, Some(o)) => Bits::from_u64(1, o.is_le() as u64)?,
        (CmpKind::Gt, Some(o)) => Bits::from_u64(1, o.is_gt() as u64)?,
        (CmpKind::Ge, Some(o)) => Bits::from_u64(1, o.is_ge() as u64)?,
        (CmpKind::Eq, Some(o)) => Bits::from_u64(1, o.is_eq() as u64)?,
        (CmpKind::Ne, Some(o)) => Bits::from_u64(1, o.is_ne() as u64)?,
    })
}

/// `=== !==` plus case-item wildcard matching. `ExactEq/ExactNe` compare
/// x/z literally; wildcard forms never yield x (IEEE 9.5 — a non-match is
/// simply 0, the case item does not fire).
pub fn apply_case_cmp(kind: CaseCmpKind, a: &Bits, b: &Bits) -> VResult<Bits> {
    match kind {
        CaseCmpKind::ExactEq => a.case_eq(b),
        CaseCmpKind::ExactNe => a.case_ne(b),
        CaseCmpKind::WildZ => Ok(Bits::from_u64(1, a.casez_match(b) as u64)?),
        CaseCmpKind::WildX => Ok(Bits::from_u64(1, a.casex_match(b) as u64)?),
    }
}

/// `<< >> <<< >>>` — the amount is self-determined (never coerced);
/// `>>>` fills with the sign bit only when the (propagated) type is signed.
pub fn apply_shift(kind: ShiftKind, a: &Bits, amt: &Bits, w: u32, signed: bool) -> VResult<Bits> {
    let av = adjust(a, w, signed)?;
    match kind {
        ShiftKind::Shl => av.shl(amt),
        ShiftKind::Shr => av.shr(amt),
        ShiftKind::Ashr => av.ashr(amt, signed),
    }
}

/// `?:` — known condition selects; unknown condition merges branch-wide
/// (differing bits → x, agreeing bits keep their value).
pub fn apply_sel(cond: &Bits, a: &Bits, b: &Bits) -> VResult<Bits> {
    match cond.truthy()? {
        Some(true) => Ok(a.clone()),
        Some(false) => Ok(b.clone()),
        None => a.merge(b),
    }
}

/// Concatenation `{…}` (MSB-first part order, caller's responsibility).
pub fn apply_concat(parts: &[Bits]) -> VResult<Bits> {
    Bits::concat(parts)
}

/// Replication `{N{…}}` (`times` is u64 to match the lowered IR field).
pub fn apply_replicate(src: &Bits, times: u64) -> VResult<Bits> {
    Bits::concat(&vec![src.clone(); times as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(s: &str) -> Bits {
        vl_values::from_numlit_parts(Some(s.len() as u32), false, vl_values::Base::Bin, s).unwrap()
    }
    fn bit(v: bool) -> Bits {
        Bits::from_u64(1, v as u64).unwrap()
    }
    fn known1(b: &Bits) -> bool {
        !b.has_unknown() && b.width == 1 && b.to_u64_lossy() == 1
    }
    fn known0(b: &Bits) -> bool {
        !b.has_unknown() && b.width == 1 && b.to_u64_lossy() == 0
    }
    fn is_x(b: &Bits) -> bool {
        b.has_unknown()
    }

    // IEEE Table 5-28 rows — the x/z logic-operator track (AGENTS §5:
    // tables first; independent from any oracle diff).
    #[test]
    fn logic_and_x_table() {
        let (z, o, x) = (lit("0"), lit("1"), lit("x"));
        assert!(known0(&apply_logic_and(&z, &x).unwrap()), "0&&x=0");
        assert!(is_x(&apply_logic_and(&o, &x).unwrap()), "1&&x=x");
        assert!(is_x(&apply_logic_and(&x, &o).unwrap()), "x&&1=x");
        assert!(known0(&apply_logic_and(&x, &z).unwrap()), "x&&0=0");
        assert!(is_x(&apply_logic_and(&x, &x).unwrap()), "x&&x=x");
        assert!(known1(&apply_logic_and(&o, &o).unwrap()), "1&&1=1");
        assert!(known0(&apply_logic_and(&o, &z).unwrap()), "1&&0=0");
        // Multi-bit operands: truthy of 4'b0010 is 1.
        assert!(is_x(&apply_logic_and(&lit("0010"), &x).unwrap()), "nonzero&&x=x");
        assert!(known0(&apply_logic_and(&lit("0000"), &x).unwrap()), "zero&&x=0");
    }

    #[test]
    fn logic_or_and_not_x_table() {
        let (z, o, x) = (lit("0"), lit("1"), lit("x"));
        assert!(known1(&apply_logic_or(&o, &x).unwrap()), "1||x=1");
        assert!(is_x(&apply_logic_or(&z, &x).unwrap()), "0||x=x");
        assert!(known1(&apply_logic_or(&x, &o).unwrap()), "x||1=1");
        assert!(is_x(&apply_logic_or(&x, &z).unwrap()), "x||0=x");
        assert!(is_x(&apply_logic_or(&x, &x).unwrap()), "x||x=x");
        assert!(is_x(&apply_logic_not(&x).unwrap()), "!x=x");
        assert!(known1(&apply_logic_not(&z).unwrap()), "!0=1");
        assert!(known0(&apply_logic_not(&o).unwrap()), "!1=0");
    }

    #[test]
    fn sel_known_and_unknown_cond() {
        let a = lit("0011");
        let b = lit("0101");
        assert_eq!(apply_sel(&bit(true), &a, &b).unwrap(), a, "cond 1 picks a");
        assert_eq!(apply_sel(&bit(false), &a, &b).unwrap(), b, "cond 0 picks b");
        // Unknown condition: agreeing bits keep values, differing → x.
        let m = apply_sel(&lit("x"), &a, &b).unwrap();
        assert_eq!(m.fmt_bin(), "0xx1");
        assert!(is_x(&m));
    }

    #[test]
    fn bin_signed_div_rem_and_width() {
        // -4 / 2 signed at width 4 = -2 (sign-extended adjust is part of
        // the semantics, not the caller's job).
        let m4 = lit("1100"); // unsigned 12, signed -4
        let two = lit("0010");
        let r = apply_bin(BinKind::Div, &m4, &two, 4, true).unwrap();
        assert_eq!(r.fmt_bin(), "1110", "signed -4/2=-2");
        let ru = apply_bin(BinKind::Div, &m4, &two, 4, false).unwrap();
        assert_eq!(ru.fmt_bin(), "0110", "unsigned 12/2=6");
        // Carry preservation at context width (AGENTS §5 acceptance):
        let (a, b) = (lit("10000000"), lit("10000000"));
        let s = apply_bin(BinKind::Add, &a, &b, 9, false).unwrap();
        assert_eq!(s.width, 9);
        assert_eq!(s.fmt_bin(), "100000000", "carry kept at w=9");
    }

    #[test]
    fn shift_amount_self_determined() {
        // >>> fills sign bits only when signed; the amount is never coerced.
        let v = lit("11100010");
        let amt = Bits::from_u64(96, 2).unwrap(); // amount width irrelevant
        assert_eq!(
            apply_shift(ShiftKind::Ashr, &v, &amt, 8, true).unwrap().fmt_bin(),
            "11111000"
        );
        assert_eq!(
            apply_shift(ShiftKind::Ashr, &v, &amt, 8, false).unwrap().fmt_bin(),
            "00111000",
            "unsigned >>> zero-fills"
        );
    }

    #[test]
    fn cmp_pairing_and_x() {
        // Max-width pairing with unsigned dominance: 4'b1110 vs 2'b11 —
        // unsigned compare 14>3; signed would be -2>3=false.
        let a = lit("1110");
        let b = lit("11");
        assert!(known1(&apply_cmp(CmpKind::Gt, &a, &b, false).unwrap()));
        assert!(known0(&apply_cmp(CmpKind::Gt, &a, &b, true).unwrap()), "signed -2>3 false");
        assert!(is_x(&apply_cmp(CmpKind::Lt, &lit("1x00"), &lit("0001"), false).unwrap()), "unknown bit → x");
        // === compares x/z literally.
        let r = apply_case_cmp(CaseCmpKind::ExactEq, &lit("1x00"), &lit("1x00")).unwrap();
        assert!(known1(&r));
    }

    #[test]
    fn reduce_wiring_and_case_wildcards() {
        // Nand/Nor/Xnor are reduce-then-invert, single source here.
        assert_eq!(apply_reduce(RedKind::And, &lit("1111")).unwrap().to_u64_lossy(), 1);
        assert_eq!(apply_reduce(RedKind::Nand, &lit("1111")).unwrap().to_u64_lossy(), 0);
        assert_eq!(apply_reduce(RedKind::Nor, &lit("0000")).unwrap().to_u64_lossy(), 1);
        // Casez wildcard: z in the ITEM (lhs) is a don't-care; non-match 0.
        assert!(known1(&apply_case_cmp(CaseCmpKind::WildZ, &lit("1zz1"), &lit("1001")).unwrap()));
        assert!(known0(&apply_case_cmp(CaseCmpKind::WildZ, &lit("1zz1"), &lit("0001")).unwrap()));
    }

    #[test]
    fn concat_replicate_xnor_bitnot() {
        let parts = vec![lit("01"), lit("x1")];
        assert_eq!(apply_concat(&parts).unwrap().fmt_bin(), "01x1");
        assert_eq!(apply_replicate(&lit("01"), 3).unwrap().fmt_bin(), "010101");
        assert_eq!(
            apply_bit_xnor(&lit("0011"), &lit("0101"), 4).unwrap().fmt_bin(),
            "1001",
            "xnor(^~) = invert(xor): 0110 → 1001"
        );
        // ~ is context-determined (dual-oracle probe, 2026-10-24): coerce
        // to expression width/sign FIRST, then invert. Unsigned shape:
        let a4 = lit("1000");
        assert_eq!(apply_bit_not(&a4, 8, false).unwrap().fmt_bin(), "11110111");
        // Signed shape (sign-extend 1000 → 11111000, invert):
        assert_eq!(apply_bit_not(&a4, 8, true).unwrap().fmt_bin(), "00000111");
        // 1-bit operand in 8-bit unsigned context (probe case y):
        assert_eq!(
            apply_bit_not(&lit("1"), 8, false).unwrap().fmt_bin(),
            "11111110"
        );
    }

    #[test]
    fn bitwise_signed_coercion() {
        // Dual-oracle probe (2026-10-24): bitwise operands coerce per the
        // EXPRESSION type. Both-signed: sign-extend → 1111_1000 & 1111_1001.
        let (p, q) = (lit("1000"), lit("1001"));
        assert_eq!(
            apply_bit(BitKind::And, &p, &q, 8, true).unwrap().fmt_bin(),
            "11111000"
        );
        // Unsigned expression: zero-extend → 0000_1000 & 0000_1001.
        assert_eq!(
            apply_bit(BitKind::And, &p, &q, 8, false).unwrap().fmt_bin(),
            "00001000"
        );
        // At natural width (no extension) both forms agree:
        assert_eq!(
            apply_bit(BitKind::And, &p, &q, 4, true).unwrap().fmt_bin(),
            "1000"
        );
    }

    #[test]
    fn adjust_and_parse_const() {
        let v = lit("1010");
        assert_eq!(adjust(&v, 8, false).unwrap().fmt_bin(), "00001010");
        assert_eq!(adjust(&v, 8, true).unwrap().fmt_bin(), "11111010", "signed 4'sb1010=-6");
        assert_eq!(adjust(&v, 2, false).unwrap().fmt_bin(), "10", "shrink truncates");
        let c = parse_const(&Literal {
            width: 6,
            signed: false,
            base: Base::Bin,
            digits: "01x1z1".into(),
        })
        .unwrap();
        assert_eq!(c.fmt_bin(), "01x1z1");
        assert!(parse_const(&Literal {
            width: 4,
            signed: false,
            base: Base::Hex,
            digits: "zzzz".into()
        })
        .is_ok());
    }
}
