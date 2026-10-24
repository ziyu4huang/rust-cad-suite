//! Shim module — the 4-state value core moved to the zero-dependency
//! `vl-values` crate (extraction per docs/riv-r2-design.md §2,
//! RIVERILOG-PLAN §3.1/§8-R1; same recipe as the ADR-0004 vl-vcd extraction).
//! The whole surface re-exports unchanged; only the vl-syntax-facing literal
//! adapter stays here, so every existing call site (`use crate::value::…`,
//! `vl_sim::value::Bits`) keeps compiling untouched.

pub use vl_values::*;

/// Parse a [`vl_syntax::ast::NumLit`] into Bits (width from literal, sized
/// truncation keeping low bits, x/z/? digits → unknown bits, unsized → ≥32).
pub fn from_numlit(n: &vl_syntax::ast::NumLit) -> VResult<Bits> {
    let base = match n.base {
        vl_syntax::ast::Base::Dec => Base::Dec,
        vl_syntax::ast::Base::Bin => Base::Bin,
        vl_syntax::ast::Base::Oct => Base::Oct,
        vl_syntax::ast::Base::Hex => Base::Hex,
    };
    from_numlit_parts(n.width, n.signed, base, &n.digits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vl_syntax::ast::{Base, NumLit};
    use vl_syntax::parser::expr::parse_based;

    fn lit(text: &str) -> Bits {
        // Direct literal through the parser's based-literal parser.
        let n = parse_based(text, Default::default());
        from_numlit(&n).unwrap()
    }

    fn dec(v: u64, w: u32) -> Bits {
        Bits::from_u64(w, v).unwrap()
    }

    fn x_of() -> Bits {
        Bits::all_x(4).unwrap()
    }

    #[test]
    fn wide_u128_values() {
        // ADR-0006: single u128 limb, widths 65..=128.
        let v = lit("100'hdead_beef_cafe_babe_1");
        assert_eq!(v.width, 100);
        assert_eq!(v.to_u128_lossy() & 0xff, 0xe1);
        // fmt_dec renders the FULL width (u128 Display), not a truncated u64.
        let big = Bits::from_u128(96, (1u128 << 90) + 12345).unwrap();
        assert_eq!(big.fmt_dec(), ((1u128 << 90) + 12345).to_string());
        // Value-fit check at declared width (borderline legal).
        assert!(from_numlit(&parse_based("80'hf", Default::default())).is_ok());
        assert!(from_numlit(&parse_based("80'h1_00000000_00000000_00000", Default::default())).is_err());
        // Shift amounts beyond 64 bits (was a truncation trap pre-widening).
        let amt = Bits::from_u128(96, 1u128 << 70).unwrap();
        assert_eq!(big.shl(&amt).unwrap(), Bits::zeros(96).unwrap());
        // Signed path via i128 (wide signed compare helper sanity).
        let neg = lit("80'sh8000_0000_0000_0000_0000"); // sign bit set
        assert!(neg.to_i128_lossy() < 0);
    }

    #[test]
    fn literals_round_trip() {
        assert_eq!(lit("8'sh3f").to_u64_lossy(), 0x3f);
        assert_eq!(lit("4'b01x?").fmt_bin(), "01xz");
        assert!(from_numlit(&parse_based("12'hDEAD_BEEF", Default::default())).is_err(),
                "oversized sized literal errors (oracle parity)");
        assert_eq!(lit("'h3x").fmt_hex(), "0000003x"); // unsized ≥32 bits (§2.5.1)
        assert_eq!(lit("'sd15").to_i64_lossy(), 15);
        assert_eq!(from_numlit(&NumLit {
            span: Default::default(),
            width: None,
            signed: false,
            base: Base::Dec,
            digits: "42".into()
        })
        .unwrap()
        .to_u64_lossy(), 42);
    }

    #[test]
    fn compare_and_case() {
        let a = lit("4'b1x00");
        assert!(a.logic_eq(&a).unwrap().has_unknown(), "1x00==1x00 → x");
        assert_eq!(a.case_eq(&a).unwrap().to_u64_lossy(), 1, "1x00===1x00 → 1");
        assert_eq!(dec(3, 4).lt(&dec(4, 4)).unwrap().to_u64_lossy(), 1);
    }

    #[test]
    fn shifts_and_ashr() {
        let v = lit("8'sb11100010");
        assert_eq!(v.shr(&dec(2, 8)).unwrap().fmt_bin(), "00111000");
        assert_eq!(v.ashr(&dec(2, 8), true).unwrap().fmt_bin(), "11111000");
        let xs = lit("4'sbx101");
        assert_eq!(xs.ashr(&dec(1, 4), true).unwrap().fmt_bin(), "xx10");
        assert!(dec(1, 4).shl(&x_of()).unwrap().has_unknown());
    }

    #[test]
    fn signed_adjust() {
        let v = lit("4'sb1010"); // -6
        assert_eq!(v.sadjust(8).unwrap().to_i64_lossy(), -6);
        assert_eq!(v.uadjust(8).unwrap().to_u64_lossy(), 0b0000_1010);
    }

    #[test]
    fn wide_literals_and_roundtrip() {
        // 256-bit literal, hex accumulation across 4 limbs.
        let v = lit("256'h00000000000000000000000000000000000000000000000000000000000000ff");
        assert_eq!(v.width, 256);
        assert_eq!(v.to_u64_lossy(), 0xff);
        // 0xf << 160 at width 200: nibble 40 of 50.
        let big = lit("200'hf_0000000000000000000000000000000000000000");
        assert_eq!(big.width, 200);
        assert_eq!(
            big.fmt_hex(),
            format!("{}f{}", "0".repeat(9), "0".repeat(40))
        );
        // Decimal parse beyond 128 bits (upstream accepts 61-digit params).
        let d = from_numlit(&NumLit {
            span: Default::default(),
            width: Some(161),
            signed: false,
            base: Base::Dec,
            digits: "1461501637330902918203684832716283019655932542976".into(), // 2^160
        })
        .unwrap();
        assert_eq!(d.width, 161);
        assert!(d.to_u128_lossy() == 0, "2^160 has zero low 128 bits");
        assert_eq!(d.part(160, 160).unwrap().to_u64_lossy(), 1);
        // fmt_dec of 2^160.
        assert_eq!(d.fmt_dec(), "1461501637330902918203684832716283019655932542976");
        // 129-bit boundary literal is legal.
        let b = lit("129'h1_0000000000000000000000000");
        assert_eq!(b.width, 129);
        assert!(b.is_wide());
        // Too many digits still errors above 128 (D5 law is width-agnostic):
        // 33 f-digits = 132 value bits > declared 130 (zeros never count).
        assert!(from_numlit(
            &parse_based("130'hfffffffffffffffffffffffffffffffff", Default::default())
        )
        .is_err());
        assert!(from_numlit(
            &parse_based("130'h1_00000000000000000000000000000000", Default::default())
        )
        .is_ok(), "2^128 fits declared 130");
    }
}
