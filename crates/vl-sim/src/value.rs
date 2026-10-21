//! 4-state bit vectors: `{data, mask}` per 64-bit limb. Bit semantics:
//! `mask=0` → data bit valid (0/1); `mask=1, data=0` → x; `mask=1, data=1` → z.
//! v0 carries a single limb (width ≤ 64; wider vectors error with an M7
//! pointer — multi-limb layout is its own ADR per docs/adr/0001).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Bits {
    pub width: u32,
    pub data: u64,
    pub mask: u64,
}

pub const MAX_WIDTH: u32 = 64;

#[derive(Debug, Clone)]
pub struct ValueError(pub String);

impl std::fmt::Display for ValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub type VResult<T> = Result<T, ValueError>;

fn chk(width: u32) -> VResult<()> {
    if width > MAX_WIDTH {
        Err(ValueError(format!(
            "vectors wider than {MAX_WIDTH} bits are not yet supported (milestone M7)"
        )))
    } else {
        Ok(())
    }
}

impl Bits {
    pub fn zeros(width: u32) -> VResult<Bits> {
        chk(width)?;
        Ok(Bits {
            width,
            data: 0,
            mask: 0,
        })
    }

    pub fn from_u64(width: u32, v: u64) -> VResult<Bits> {
        chk(width)?;
        let dropped = if width >= 64 { 0 } else { v >> width };
        if dropped != 0 {
            return Err(ValueError(format!(
                "value {v} does not fit in {width} bits"
            )));
        }
        Ok(Bits {
            width,
            data: v,
            mask: 0,
        })
    }

    /// All-x (used by conservative arithmetic X and unknown shifts).
    pub fn all_x(width: u32) -> VResult<Bits> {
        chk(width)?;
        Ok(Bits {
            width,
            data: 0,
            mask: low_bits(width),
        })
    }

    pub fn is_known(&self) -> bool {
        self.mask == 0
    }

    pub fn has_unknown(&self) -> bool {
        self.mask != 0
    }

    /// Known-bit value; unknown bits read as 0 (x-initial policy = 0).
    pub fn to_u64_lossy(&self) -> u64 {
        self.data & !self.mask
    }

    /// Signed interpretation of the known bits (x MSB reads as 0).
    pub fn to_i64_lossy(&self) -> i64 {
        let u = self.to_u64_lossy();
        if self.width == 0 {
            return 0;
        }
        if self.width >= 64 {
            return u as i64;
        }
        if u & (1 << (self.width - 1)) != 0 {
            (u | !low_bits(self.width)) as i64
        } else {
            u as i64
        }
    }

    pub fn trunc(&self, width: u32) -> VResult<Bits> {
        chk(width)?;
        let lb = low_bits(width);
        Ok(Bits {
            width,
            data: self.data & lb,
            mask: self.mask & lb,
        })
    }

    /// Zero-extend / truncate to `width` (unsigned adjust).
    pub fn uadjust(&self, width: u32) -> VResult<Bits> {
        if width > self.width {
            let mut b = self.trunc(self.width)?;
            chk(width)?;
            b.width = width;
            Ok(b)
        } else {
            self.trunc(width)
        }
    }

    /// Sign-extend (or truncate) to `width`.
    pub fn sadjust(&self, width: u32) -> VResult<Bits> {
        if width <= self.width {
            return self.trunc(width);
        }
        chk(width)?;
        let sign_unknown = self.mask & (1 << (self.width - 1)) != 0;
        let sign = self.data & (1 << (self.width - 1)) != 0;
        let mut b = *self;
        b.width = width;
        if sign {
            b.data |= !low_bits(self.width);
        }
        if sign_unknown {
            b.mask |= !low_bits(self.width);
        }
        b.data &= low_bits(width);
        b.mask &= low_bits(width);
        Ok(b)
    }

    /// Per-bit bitwise AND with full x/z tables (0 & x = 0, 1 & x = x, z ≡ x).
    pub fn bit_and(&self, other: &Bits) -> VResult<Bits> {
        let w = self.width.max(other.width);
        chk(w)?;
        let a = self.uadjust(w)?;
        let b = other.uadjust(w)?;
        let ones = a.data & !a.mask & b.data & !b.mask;
        let zeros = (!a.data & !a.mask) | (!b.data & !b.mask);
        Ok(Bits {
            width: w,
            data: ones,
            mask: low_bits(w) & !(ones | zeros),
        })
    }

    pub fn bit_or(&self, other: &Bits) -> VResult<Bits> {
        let w = self.width.max(other.width);
        let a = self.uadjust(w)?;
        let b = other.uadjust(w)?;
        let ones = a.data & !a.mask | b.data & !b.mask;
        let zeros = (!a.data & !a.mask) & (!b.data & !b.mask);
        Ok(Bits {
            width: w,
            data: ones,
            mask: low_bits(w) & !(ones | zeros),
        })
    }

    /// XOR: any unknown input bit → unknown output bit (exact elsewhere).
    pub fn bit_xor(&self, other: &Bits) -> VResult<Bits> {
        let w = self.width.max(other.width);
        let a = self.uadjust(w)?;
        let b = other.uadjust(w)?;
        Ok(Bits {
            width: w,
            data: a.data ^ b.data,
            mask: a.mask | b.mask,
        })
    }

    pub fn bit_not(&self) -> Bits {
        // Known bits flip; unknown bits keep their kind (x stays x, z stays z).
        Bits {
            width: self.width,
            data: (!self.data & !self.mask) | (self.data & self.mask),
            mask: self.mask,
        }
    }

    /// Both operands known → arithmetic; any unknown bit → all-x result
    /// (conservative; exact on the X-clean differential corpus; IEEE-§5-exact
    /// bit-serial arithmetic is an M4 item per ADR-0001).
    fn arith(&self, other: &Bits, f: impl Fn(u64, u64) -> u64) -> VResult<Bits> {
        let w = self.width.max(other.width);
        if self.has_unknown() || other.has_unknown() {
            return Bits::all_x(w);
        }
        let a = self.to_u64_lossy();
        let b = other.to_u64_lossy();
        let r = f(a, b);
        if w >= 64 {
            Ok(Bits {
                width: w,
                data: r,
                mask: 0,
            })
        } else {
            Bits::from_u64(w, r & low_bits(w))
        }
    }

    pub fn add(&self, other: &Bits) -> VResult<Bits> {
        self.arith(other, |a, b| a.wrapping_add(b))
    }

    pub fn sub(&self, other: &Bits) -> VResult<Bits> {
        self.arith(other, |a, b| a.wrapping_sub(b))
    }

    pub fn mul(&self, other: &Bits) -> VResult<Bits> {
        self.arith(other, |a, b| a.wrapping_mul(b))
    }

    pub fn div(&self, other: &Bits) -> VResult<Bits> {
        let w = self.width.max(other.width);
        if self.has_unknown() || other.has_unknown() {
            return Bits::all_x(w);
        }
        let b = other.to_u64_lossy();
        if b == 0 {
            // x/0 = x (IEEE 1364-2005 §5.1.5)
            return Bits::all_x(w);
        }
        let a = self.to_u64_lossy();
        Bits::from_u64(w.min(64), a / b)
    }

    pub fn rem(&self, other: &Bits) -> VResult<Bits> {
        let w = self.width.max(other.width);
        if self.has_unknown() || other.has_unknown() {
            return Bits::all_x(w);
        }
        let b = other.to_u64_lossy();
        if b == 0 {
            return Bits::all_x(w);
        }
        let a = self.to_u64_lossy();
        Bits::from_u64(w.min(64), a % b)
    }

    pub fn pow(&self, other: &Bits) -> VResult<Bits> {
        let w = self.width.max(other.width);
        if self.has_unknown() || other.has_unknown() {
            return Bits::all_x(w);
        }
        let a = self.to_u64_lossy();
        let b = other.to_u64_lossy();
        let r = (a as u128)
            .checked_pow(b.min(64) as u32)
            .map(|v| (v & low_bits(w) as u128) as u64);
        match r {
            Some(v) => Bits::from_u64(w.min(64), v),
            None => Bits::all_x(w),
        }
    }

    /// Equality: any known-bit mismatch → 0; else any unknown → x; else 1.
    /// `===` uses [`Bits::case_eq`] instead (IEEE 1364-2005 §5.1.6/§5.1.7).
    pub fn logic_eq(&self, other: &Bits) -> VResult<Bits> {
        let w = self.width.max(other.width);
        let a = self.uadjust(w)?;
        let b = other.uadjust(w)?;
        let known_diff = (a.data ^ b.data) & !(a.mask | b.mask);
        if known_diff != 0 {
            return Bits::from_u64(1, 0);
        }
        if a.mask != 0 || b.mask != 0 {
            return Bits::all_x(1);
        }
        Bits::from_u64(1, 1)
    }

    pub fn logic_ne(&self, other: &Bits) -> VResult<Bits> {
        Ok(self.logic_eq(other)?.bit_not())
    }

    /// `===`: bit-exact including x/z identity.
    pub fn case_eq(&self, other: &Bits) -> VResult<Bits> {
        Bits::from_u64(1, (self.data == other.data && self.mask == other.mask) as u64)
    }

    pub fn case_ne(&self, other: &Bits) -> VResult<Bits> {
        Ok(self.case_eq(other)?.bit_not())
    }

    /// Unsigned less-than (both must be known; caller handles signedness).
    pub fn lt(&self, other: &Bits) -> VResult<Bits> {
        if self.has_unknown() || other.has_unknown() {
            return Bits::all_x(1);
        }
        Bits::from_u64(1, (self.to_u64_lossy() < other.to_u64_lossy()) as u64)
    }

    pub fn shl(&self, amount: &Bits) -> VResult<Bits> {
        if amount.has_unknown() {
            return Bits::all_x(self.width);
        }
        let s = amount.to_u64_lossy();
        if s >= self.width as u64 {
            return Bits::zeros(self.width);
        }
        let s = s as u32;
        Ok(Bits {
            width: self.width,
            data: (self.data << s) & low_bits(self.width),
            mask: (self.mask << s) & low_bits(self.width),
        })
    }

    pub fn shr(&self, amount: &Bits) -> VResult<Bits> {
        if amount.has_unknown() {
            return Bits::all_x(self.width);
        }
        let s = amount.to_u64_lossy();
        if s >= 64 {
            return Bits::zeros(self.width);
        }
        let s = s as u32;
        Ok(Bits {
            width: self.width,
            data: self.data >> s,
            mask: self.mask >> s,
        })
    }

    /// Arithmetic right shift: signed → shift in the sign bit (x sign → x);
    /// unsigned → same as `>>` (IEEE 1364-2005 §5.1.10).
    pub fn ashr(&self, amount: &Bits, signed: bool) -> VResult<Bits> {
        if !signed {
            return self.shr(amount);
        }
        if amount.has_unknown() {
            return Bits::all_x(self.width);
        }
        let s = amount.to_u64_lossy();
        if s >= self.width as u64 {
            // Entirely sign bits.
            let sign_unknown = self.mask & (1 << (self.width - 1)) != 0;
            return if sign_unknown {
                Bits::all_x(self.width)
            } else if self.data & (1 << (self.width - 1)) != 0 {
                Ok(Bits {
                    width: self.width,
                    data: low_bits(self.width),
                    mask: 0,
                })
            } else {
                Bits::zeros(self.width)
            };
        }
        let s = s as u32;
        let w = self.width;
        let sign_unknown = self.mask & (1 << (w - 1)) != 0;
        let sign = self.data & (1 << (w - 1)) != 0;
        // Sign-extension happens at the value's own width (128-bit scratch).
        // Unknown sign fills with x (data 0, mask 1); known 1 fills with ones.
        let fill: u128 = if !sign_unknown && sign { !0u128 } else { 0 };
        let fillm: u128 = if sign_unknown { !0u128 } else { 0 };
        let ext: u128 = ((self.data as u128) | (fill << w)) >> s;
        let extm: u128 = ((self.mask as u128) | (fillm << w)) >> s;
        Ok(Bits {
            width: w,
            data: (ext as u64) & low_bits(w),
            mask: (extm as u64) & low_bits(w),
        })
    }

    /// Reduction AND.
    pub fn reduce_and(&self) -> VResult<Bits> {
        let zeros = !self.data & !self.mask;
        if zeros & low_bits(self.width) != 0 {
            Bits::from_u64(1, 0)
        } else if self.mask & low_bits(self.width) != 0 {
            Bits::all_x(1)
        } else {
            Bits::from_u64(1, 1)
        }
    }

    pub fn reduce_or(&self) -> VResult<Bits> {
        let ones = self.data & !self.mask;
        if ones & low_bits(self.width) != 0 {
            Bits::from_u64(1, 1)
        } else if self.mask & low_bits(self.width) != 0 {
            Bits::all_x(1)
        } else {
            Bits::from_u64(1, 0)
        }
    }

    pub fn reduce_xor(&self) -> VResult<Bits> {
        if self.has_unknown() {
            return Bits::all_x(1);
        }
        let v = (self.data.count_ones()) & 1;
        Bits::from_u64(1, v as u64)
    }

    /// Truthiness for `if`/`?:`/logical ops: known → 0/1; any unknown bit that
    /// could matter → x (conservative: any unknown → x).
    pub fn truthy(&self) -> VResult<Option<bool>> {
        if self.has_unknown() {
            return Ok(None);
        }
        Ok(Some(self.to_u64_lossy() != 0))
    }

    /// Ternary merge for `c ? t : e` with unknown cond (IEEE 1364-2005
    /// Table 5-21-ish semantics): bits where both sides agree keep that value,
    /// differing or unknown bits become x.
    pub fn merge(&self, other: &Bits) -> VResult<Bits> {
        let w = self.width.max(other.width);
        let a = self.uadjust(w)?;
        let b = other.uadjust(w)?;
        let ones = a.data & !a.mask & b.data & !b.mask;
        let zeros = !a.data & !a.mask & (!b.data & !b.mask);
        Ok(Bits {
            width: w,
            data: ones,
            mask: low_bits(w) & !(ones | zeros),
        })
    }

    /// Extract `[hi:lo]` (hi ≥ lo; both inclusive, direction-agnostic width).
    pub fn part(&self, hi: u32, lo: u32) -> VResult<Bits> {
        let (hi, lo) = if hi >= lo { (hi, lo) } else { (lo, hi) };
        let w = hi - lo + 1;
        chk(w)?;
        let shifted = self.data >> lo;
        let shiftedm = self.mask >> lo;
        Ok(Bits {
            width: w,
            data: shifted & low_bits(w),
            mask: shiftedm & low_bits(w),
        })
    }

    /// Replace `[hi:lo]` with `v` (v truncated/extended to the field width).
    pub fn set_part(&self, hi: u32, lo: u32, v: &Bits) -> VResult<Bits> {
        let (hi, lo) = if hi >= lo { (hi, lo) } else { (lo, hi) };
        let w = hi - lo + 1;
        let field = v.uadjust(w)?;
        let clear = !(low_bits(w) << lo);
        Ok(Bits {
            width: self.width,
            data: (self.data & clear) | (field.data << lo),
            mask: (self.mask & clear) | (field.mask << lo),
        })
    }

    /// Concatenate: `parts` are given MSB-first (Verilog order).
    pub fn concat(parts: &[Bits]) -> VResult<Bits> {
        let total: u32 = parts.iter().map(|p| p.width).sum();
        chk(total)?;
        let mut data = 0u64;
        let mut mask = 0u64;
        for p in parts {
            data = (data << p.width) | (p.data & low_bits(p.width));
            mask = (mask << p.width) | (p.mask & low_bits(p.width));
        }
        Ok(Bits {
            width: total,
            data,
            mask,
        })
    }

    pub fn replicate(&self, times: u64) -> VResult<Bits> {
        let total = self
            .width
            .checked_mul(times as u32)
            .ok_or_else(|| ValueError("replication width overflow".into()))?;
        chk(total)?;
        let mut data = 0u64;
        let mut mask = 0u64;
        for _ in 0..times {
            data = (data << self.width) | (self.data & low_bits(self.width));
            mask = (mask << self.width) | (self.mask & low_bits(self.width));
        }
        Ok(Bits {
            width: total,
            data,
            mask,
        })
    }

    /// `%b` rendering: `0`/`1`/`x`/`z` per bit, full width.
    pub fn fmt_bin(&self) -> String {
        let mut s = String::new();
        for i in (0..self.width).rev() {
            let bit = 1u64 << i;
            if self.mask & bit != 0 {
                s.push(if self.data & bit != 0 { 'z' } else { 'x' });
            } else {
                s.push(if self.data & bit != 0 { '1' } else { '0' });
            }
        }
        s
    }

    /// `%h`/`%x` rendering (full width, one char per 4 bits).
    pub fn fmt_hex(&self) -> String {
        let nibbles = self.width.div_ceil(4);
        let mut s = String::new();
        for i in (0..nibbles).rev() {
            let sh = i * 4;
            let d = (self.data >> sh) & 0xF;
            let m = (self.mask >> sh) & 0xF;
            s.push(match (m, d) {
                (0, v) => char::from_digit(v as u32, 16).unwrap_or('?'),
                (_, 0) => 'x',
                _ => 'z',
            });
        }
        s
    }

    /// `%0d` rendering (x → "x").
    pub fn fmt_dec(&self) -> String {
        if self.has_unknown() {
            // IEEE: if any bit x/z → x (shortest form prints "x")
            return "x".to_string();
        }
        self.to_u64_lossy().to_string()
    }
}

pub fn low_bits(width: u32) -> u64 {
    if width == 0 {
        0
    } else if width >= 64 {
        !0u64
    } else {
        (1u64 << width) - 1
    }
}

/// Parse a [`vl_syntax::ast::NumLit`] into Bits (width from literal, sized
/// truncation keeping low bits, x/z/? digits → unknown bits, unsized → ≥32).
pub fn from_numlit(n: &vl_syntax::ast::NumLit) -> VResult<Bits> {
    use vl_syntax::ast::Base;
    // Lexer-normalized text has no underscores, but accept them anyway so
    // direct parses (tests, LSP-style tooling) behave identically.
    let digits: String = n.digits.chars().filter(|&c| c != '_').collect();
    let digits = digits.as_str();
    // Decimal: x/z allowed only as the whole value (IEEE 1364-2005 §2.5.1).
    if n.base == Base::Dec {
        let w = n.width.unwrap_or(32);
        if digits.chars().all(|c| matches!(c, 'x' | 'X' | 'z' | 'Z' | '?')) {
            let mut b = Bits::all_x(w)?;
            if matches!(digits.chars().next(), Some('z' | 'Z' | '?')) {
                b.data = b.mask; // z
            }
            return Ok(b);
        }
        if digits.chars().any(|c| matches!(c, 'x' | 'X' | 'z' | 'Z' | '?')) {
            return Err(ValueError(
                "x/z digits are not allowed inside a decimal literal".into(),
            ));
        }
        let v: u64 = digits
            .parse()
            .map_err(|_| ValueError(format!("decimal literal `{}` overflows", digits)))?;
        let sized = Bits::from_u64(w.min(MAX_WIDTH), v & low_bits(w.min(MAX_WIDTH)))?;
        return Ok(sized);
    }
    let per: u32 = match n.base {
        Base::Bin => 1,
        Base::Oct => 3,
        Base::Hex => 4,
        Base::Dec => unreachable!(),
    };
    let nat_width = digits.len() as u32 * per;
    let full = n.width.unwrap_or_else(|| nat_width.max(32));
    chk(full)?;
    // Unsized literal made only of x/z/? digits: the unknown fills the whole
    // (≥32-bit) value (`'hx` → 32 x's), per §2.5.1 and the oracle.
    if n.width.is_none()
        && digits.chars().all(|c| matches!(c, 'x' | 'X' | 'z' | 'Z' | '?'))
    {
        let w = full.min(MAX_WIDTH);
        let mut b = Bits::all_x(w)?;
        if matches!(digits.chars().next(), Some('z' | 'Z' | '?')) {
            b.data = b.mask;
        }
        return Ok(b);
    }
    // Upstream (Verilator) errors on more digit-bits than the declared size
    // rather than truncating — match it (found by the E2E differential run).
    if let Some(declared) = n.width {
        if nat_width > declared {
            return Err(ValueError(format!(
                "too many digits for {declared}-bit number: `{}`",
                n.digits
            )));
        }
    }
    let mut data = 0u64;
    let mut mask = 0u64;
    for c in digits.chars() {
        let nib = match c.to_digit(16) {
            Some(v) if (v as u32) < (1 << per) => (v as u64, 0u64),
            _ => match c {
                'x' | 'X' => (0, low_bits(per)),
                'z' | 'Z' | '?' => (low_bits(per), low_bits(per)),
                _ => {
                    return Err(ValueError(format!(
                        "digit `{c}` invalid for base of literal"
                    )))
                }
            },
        };
        data = (data << per) | nib.0;
        mask = (mask << per) | nib.1;
    }
    let w = full.min(MAX_WIDTH);
    Ok(Bits {
        width: w,
        data: data & low_bits(w),
        mask: mask & low_bits(w),
    })
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
    fn bitwise_x_tables() {
        let x = Bits::all_x(4).unwrap();
        let one = dec(0b0001, 4);
        let zero = dec(0, 4);
        // 1 & x = x ; 0 & x = 0
        assert_eq!(one.bit_and(&x).unwrap().fmt_bin(), "000x");
        assert_eq!(zero.bit_and(&x).unwrap().fmt_bin(), "0000");
        // per-bit: (1|x)=1, (0|x)=x → 0001|xxxx = xxx1, 0000|xxxx = xxxx
        assert_eq!(one.bit_or(&x).unwrap().fmt_bin(), "xxx1");
        assert_eq!(zero.bit_or(&x).unwrap().fmt_bin(), "xxxx");
        // ~x = x
        assert_eq!(x.bit_not().fmt_bin(), "xxxx");
    }

    #[test]
    fn arithmetic_conservative_x() {
        let x = Bits::all_x(4).unwrap();
        let one = dec(1, 4);
        assert!(one.add(&x).unwrap().has_unknown());
        assert_eq!(dec(9, 4).add(&dec(8, 4)).unwrap().to_u64_lossy(), 1); // mod 16
        assert!(dec(5, 4).div(&dec(0, 4)).unwrap().has_unknown()); // x/0 = x
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

    fn x_of() -> Bits {
        Bits::all_x(4).unwrap()
    }

    #[test]
    fn part_select_and_concat() {
        let v = dec(0b1011_0110, 8);
        assert_eq!(v.part(7, 4).unwrap().to_u64_lossy(), 0b1011);
        assert_eq!(v.set_part(3, 0, &dec(0b0001, 4)).unwrap().to_u64_lossy(), 0b1011_0001);
        let c = Bits::concat(&[dec(0b10, 2), dec(0b01, 2)]).unwrap();
        assert_eq!(c.to_u64_lossy(), 0b1001);
        assert_eq!(dec(0b01, 2).replicate(3).unwrap().fmt_bin(), "010101");
    }

    #[test]
    fn ternary_merge_matches_lrm() {
        // 1'bx ? 4'b1010 : 4'b0110 → bits equal (bit1=0,bit2=1) kept, others x
        let t = dec(0b1010, 4);
        let e = dec(0b0110, 4);
        let m = t.merge(&e).unwrap();
        assert_eq!(m.fmt_bin(), "xx10"); // equal bits kept, differing → x
    }

    #[test]
    fn signed_adjust() {
        let v = lit("4'sb1010"); // -6
        assert_eq!(v.sadjust(8).unwrap().to_i64_lossy(), -6);
        assert_eq!(v.uadjust(8).unwrap().to_u64_lossy(), 0b0000_1010);
    }
}
