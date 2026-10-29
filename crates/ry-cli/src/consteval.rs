//! RTLIL::const_* 求值器族(kernel/calc.cc 移植面;opt_expr -keepdc 的
//! FOLD 家族與 ConstEval 用)。位序=LSB-first(ry-ir Bit)。大數域=i128
//! (語料常數寬 ≤64;>128 觸發即 diff 紅燈屆時擴)。

use ry_ir::{Bit, Const};

pub fn b2c(bits: Vec<Bit>) -> Const {
    Const::Bits { width: bits.len() as i64, bits, signed: false, is_unsized: false }
}

pub fn c_bits(c: &Const) -> Vec<Bit> {
    match c {
        Const::Str(s) => {
            // decode_string 的位化:每字元 8 位 LSB-first(yosys Const(Str)
            // 位域視圖;opt_expr 不觸字串參數 cell,防禦空)。
            let mut out = Vec::new();
            for ch in s.chars() {
                let v = ch as u32;
                for i in 0..8 {
                    out.push(if (v >> i) & 1 == 1 { Bit::S1 } else { Bit::S0 });
                }
            }
            out
        }
        Const::Bits { bits, .. } => bits.clone(),
    }
}

/// calc.cc extend_u0(Const 版;signed 補最高位)。
fn extend(c: &mut Vec<Bit>, width: usize, is_signed: bool) {
    let padding = if is_signed && !c.is_empty() { *c.last().unwrap() } else { Bit::S0 };
    while c.len() < width {
        c.push(padding);
    }
}

/// const2big(calc.cc:39):undef_bit_pos=首個非 0/1 位(無=-1)。
fn const2big(bits: &[Bit], as_signed: bool) -> (i128, i64) {
    let mut undef_bit_pos: i64 = -1;
    let mut num_bits = bits.len();
    let mut neg = false;
    let inv_sign_bit = if as_signed && num_bits > 0 && bits[num_bits - 1] == Bit::S1 {
        neg = true;
        num_bits -= 1;
        Bit::S0
    } else {
        Bit::S1
    };
    let mut mag: u128 = 0;
    for i in 0..num_bits {
        match bits[i] {
            Bit::S0 | Bit::S1 => {
                if bits[i] == inv_sign_bit {
                    mag |= 1u128 << i;
                }
            }
            _ => {
                if undef_bit_pos < 0 {
                    undef_bit_pos = i as i64;
                }
            }
        }
    }
    let v = if neg { (mag as i128).wrapping_add(1) } else { mag as i128 };
    (v, undef_bit_pos)
}

/// big2const(calc.cc:65)。
fn big2big(val: i128, result_len: usize, undef_bit_pos: i64) -> Const {
    if undef_bit_pos >= 0 {
        return Const::Bits { width: result_len as i64, bits: vec![Bit::Sx; result_len], signed: false, is_unsized: false };
    }
    let mut result = vec![Bit::S0; result_len];
    if val != 0 {
        if val < 0 {
            let mag = (-(val + 1)) as u128; // mag-1 of magnitude
            for i in 0..result_len {
                result[i] = if (mag >> i) & 1 == 1 { Bit::S0 } else { Bit::S1 };
            }
        } else {
            let mag = val as u128;
            for i in 0..result_len {
                result[i] = if (mag >> i) & 1 == 1 { Bit::S1 } else { Bit::S0 };
            }
        }
    }
    b2c(result)
}

fn logic_and(a: Bit, b: Bit) -> Bit {
    if a == Bit::S0 { return Bit::S0; }
    if b == Bit::S0 { return Bit::S0; }
    if a != Bit::S1 { return Bit::Sx; }
    if b != Bit::S1 { return Bit::Sx; }
    Bit::S1
}

fn logic_or(a: Bit, b: Bit) -> Bit {
    if a == Bit::S1 { return Bit::S1; }
    if b == Bit::S1 { return Bit::S1; }
    if a != Bit::S0 { return Bit::Sx; }
    if b != Bit::S0 { return Bit::Sx; }
    Bit::S0
}

fn logic_xor(a: Bit, b: Bit) -> Bit {
    if !matches!(a, Bit::S0 | Bit::S1) { return Bit::Sx; }
    if !matches!(b, Bit::S0 | Bit::S1) { return Bit::Sx; }
    if a != b { Bit::S1 } else { Bit::S0 }
}

fn logic_xnor(a: Bit, b: Bit) -> Bit {
    if !matches!(a, Bit::S0 | Bit::S1) { return Bit::Sx; }
    if !matches!(b, Bit::S0 | Bit::S1) { return Bit::Sx; }
    if a == b { Bit::S1 } else { Bit::S0 }
}

fn logic_wrapper(f: fn(Bit, Bit) -> Bit, a1: &Const, a2: &Const, s1: bool, s2: bool, result_len: i64) -> Const {
    let mut b1 = c_bits(a1);
    let mut b2 = c_bits(a2);
    let rl = if result_len < 0 { b1.len().max(b2.len()) } else { result_len as usize };
    extend(&mut b1, rl, s1);
    extend(&mut b2, rl, s2);
    let mut out = vec![Bit::Sx; rl];
    for i in 0..rl {
        let a = b1.get(i).copied().unwrap_or(Bit::S0);
        let b = b2.get(i).copied().unwrap_or(Bit::S0);
        out[i] = f(a, b);
    }
    b2c(out)
}

fn logic_reduce_wrapper(initial: Bit, f: fn(Bit, Bit) -> Bit, a1: &Const, result_len: i64) -> Const {
    let bits = c_bits(a1);
    let mut temp = initial;
    for &b in &bits {
        temp = f(temp, b);
    }
    let mut out = vec![temp];
    if (out.len() as i64) < result_len {
        out.resize(result_len as usize, Bit::S0);
    }
    b2c(out)
}

pub fn const_not(a1: &Const, _a2: &Const, s1: bool, _s2: bool, result_len: i64) -> Const {
    let rl = if result_len < 0 { c_bits(a1).len() } else { result_len as usize };
    let mut arg = c_bits(a1);
    extend(&mut arg, rl, s1);
    let mut out = vec![Bit::Sx; rl];
    for i in 0..rl {
        out[i] = match arg.get(i) {
            None => Bit::S0,
            Some(Bit::S0) => Bit::S1,
            Some(Bit::S1) => Bit::S0,
            _ => Bit::Sx,
        };
    }
    b2c(out)
}

pub fn const_and(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    logic_wrapper(logic_and, a, b, s1, s2, rl)
}
pub fn const_or(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    logic_wrapper(logic_or, a, b, s1, s2, rl)
}
pub fn const_xor(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    logic_wrapper(logic_xor, a, b, s1, s2, rl)
}
pub fn const_xnor(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    logic_wrapper(logic_xnor, a, b, s1, s2, rl)
}
pub fn const_reduce_and(a: &Const, _b: &Const, _s1: bool, _s2: bool, rl: i64) -> Const {
    logic_reduce_wrapper(Bit::S1, logic_and, a, rl)
}
pub fn const_reduce_or(a: &Const, _b: &Const, _s1: bool, _s2: bool, rl: i64) -> Const {
    logic_reduce_wrapper(Bit::S0, logic_or, a, rl)
}
pub fn const_reduce_xor(a: &Const, _b: &Const, _s1: bool, _s2: bool, rl: i64) -> Const {
    logic_reduce_wrapper(Bit::S0, logic_xor, a, rl)
}
pub fn const_reduce_xnor(a: &Const, _b: &Const, _s1: bool, _s2: bool, rl: i64) -> Const {
    let mut buffer = logic_reduce_wrapper(Bit::S0, logic_xor, a, rl);
    if let Const::Bits { bits, .. } = &mut buffer {
        if !bits.is_empty() {
            bits[0] = match bits[0] {
                Bit::S0 => Bit::S1,
                Bit::S1 => Bit::S0,
                other => other,
            };
        }
    }
    buffer
}
pub fn const_reduce_bool(a: &Const, _b: &Const, _s1: bool, _s2: bool, rl: i64) -> Const {
    logic_reduce_wrapper(Bit::S0, logic_or, a, rl)
}

/// const2big 的布爾視角(isZero+undef)。
fn const_logic_bit(c: &Const, signed: bool) -> Bit {
    let bits = c_bits(c);
    let (v, undef) = const2big(&bits, signed);
    if v == 0 {
        if undef >= 0 { Bit::Sx } else { Bit::S0 }
    } else {
        Bit::S1
    }
}

pub fn const_logic_not(a: &Const, _b: &Const, s1: bool, _s2: bool, rl: i64) -> Const {
    let bit = match const_logic_bit(a, s1) {
        Bit::S0 => Bit::S1,
        Bit::Sx => Bit::Sx,
        _ => Bit::S0,
    };
    let mut out = vec![bit];
    if (out.len() as i64) < rl {
        out.resize(rl as usize, Bit::S0);
    }
    b2c(out)
}

pub fn const_logic_and(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    let bit = logic_and(const_logic_bit(a, s1), const_logic_bit(b, s2));
    let mut out = vec![bit];
    if (out.len() as i64) < rl {
        out.resize(rl as usize, Bit::S0);
    }
    b2c(out)
}

pub fn const_logic_or(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    let bit = logic_or(const_logic_bit(a, s1), const_logic_bit(b, s2));
    let mut out = vec![bit];
    if (out.len() as i64) < rl {
        out.resize(rl as usize, Bit::S0);
    }
    b2c(out)
}

fn const_shift_worker(a1: &Const, a2: &Const, sign_ext: bool, signed2: bool, direction: i64, result_len: i64, vacant: Bit) -> Const {
    let a1bits = c_bits(a1);
    let a2bits = c_bits(a2);
    let (off_raw, undef) = const2big(&a2bits, signed2);
    let offset = off_raw * direction as i128;
    let rl = if result_len < 0 { a1bits.len() } else { result_len as usize };
    let mut out = vec![Bit::Sx; rl];
    if undef >= 0 {
        return b2c(out);
    }
    for i in 0..rl as i128 {
        let pos = i + offset as i128;
        if pos < 0 {
            out[i as usize] = vacant;
        } else if pos >= a1bits.len() as i128 {
            out[i as usize] = if sign_ext && !a1bits.is_empty() { *a1bits.last().unwrap() } else { vacant };
        } else {
            out[i as usize] = a1bits[pos as usize];
        }
    }
    b2c(out)
}

pub fn const_shl(a: &Const, b: &Const, s1: bool, _s2: bool, rl: i64) -> Const {
    let mut arg = c_bits(a);
    extend(&mut arg, rl.max(0) as usize, s1);
    const_shift_worker(&b2c(arg), b, false, false, -1, rl, Bit::S0)
}
pub fn const_shr(a: &Const, b: &Const, s1: bool, _s2: bool, rl: i64) -> Const {
    let mut arg = c_bits(a);
    extend(&mut arg, rl.max(c_bits(a).len() as i64).max(0) as usize, s1);
    const_shift_worker(&b2c(arg), b, false, false, 1, rl, Bit::S0)
}
pub fn const_sshl(a: &Const, b: &Const, s1: bool, _s2: bool, rl: i64) -> Const {
    const_shift_worker(a, b, s1, false, -1, rl, Bit::S0)
}
pub fn const_sshr(a: &Const, b: &Const, s1: bool, _s2: bool, rl: i64) -> Const {
    const_shift_worker(a, b, s1, false, 1, rl, Bit::S0)
}
pub fn const_shift(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    let mut arg = c_bits(a);
    extend(&mut arg, rl.max(c_bits(a).len() as i64).max(0) as usize, s1);
    const_shift_worker(&b2c(arg), b, false, s2, 1, rl, Bit::S0)
}
pub fn const_shiftx(a: &Const, b: &Const, _s1: bool, s2: bool, rl: i64) -> Const {
    const_shift_worker(a, b, false, s2, 1, rl, Bit::Sx)
}

fn cmp_result(y: bool, undef: i64, rl: i64) -> Const {
    let mut out = vec![if undef >= 0 { Bit::Sx } else if y { Bit::S1 } else { Bit::S0 }];
    if (out.len() as i64) < rl {
        out.resize(rl as usize, Bit::S0);
    }
    b2c(out)
}

pub fn const_lt(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    let (x, u1) = const2big(&c_bits(a), s1);
    let (y, u2) = const2big(&c_bits(b), s2);
    cmp_result(x < y, u1.max(u2), rl)
}
pub fn const_le(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    let (x, u1) = const2big(&c_bits(a), s1);
    let (y, u2) = const2big(&c_bits(b), s2);
    cmp_result(x <= y, u1.max(u2), rl)
}
pub fn const_ge(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    let (x, u1) = const2big(&c_bits(a), s1);
    let (y, u2) = const2big(&c_bits(b), s2);
    cmp_result(x >= y, u1.max(u2), rl)
}
pub fn const_gt(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    let (x, u1) = const2big(&c_bits(a), s1);
    let (y, u2) = const2big(&c_bits(b), s2);
    cmp_result(x > y, u1.max(u2), rl)
}

pub fn const_eq(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    let mut a1 = c_bits(a);
    let mut b1 = c_bits(b);
    let width = a1.len().max(b1.len());
    extend(&mut a1, width, s1 && s2);
    extend(&mut b1, width, s1 && s2);
    let mut out = vec![Bit::S0; rl.max(0) as usize];
    let mut matched = Bit::S1;
    for i in 0..width {
        let x = a1[i];
        let y = b1[i];
        if x == Bit::S0 && y == Bit::S1 {
            return b2c(out);
        }
        if x == Bit::S1 && y == Bit::S0 {
            return b2c(out);
        }
        if !matches!(x, Bit::S0 | Bit::S1) || !matches!(y, Bit::S0 | Bit::S1) {
            matched = Bit::Sx;
        }
    }
    if !out.is_empty() {
        out[0] = matched;
    }
    b2c(out)
}

pub fn const_ne(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    let mut out = match const_eq(a, b, s1, s2, rl) {
        Const::Bits { bits, width, .. } => (bits, width),
        _ => unreachable!(),
    };
    if let Some(f) = out.0.first_mut() {
        let cur = *f;
        *f = match cur {
            Bit::S0 => Bit::S1,
            Bit::S1 => Bit::S0,
            other => other,
        };
    }
    Const::Bits { width: out.1, bits: out.0, signed: false, is_unsized: false }
}

pub fn const_eqx(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    let mut a1 = c_bits(a);
    let mut b1 = c_bits(b);
    let width = a1.len().max(b1.len());
    extend(&mut a1, width, s1 && s2);
    extend(&mut b1, width, s1 && s2);
    let mut out = vec![Bit::S0; rl.max(0) as usize];
    for i in 0..width {
        if a1[i] != b1[i] {
            return b2c(out);
        }
    }
    if !out.is_empty() {
        out[0] = Bit::S1;
    }
    b2c(out)
}

pub fn const_nex(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    let mut out = match const_eqx(a, b, s1, s2, rl) {
        Const::Bits { bits, width, .. } => (bits, width),
        _ => unreachable!(),
    };
    if let Some(f) = out.0.first_mut() {
        let cur = *f;
        *f = match cur {
            Bit::S0 => Bit::S1,
            Bit::S1 => Bit::S0,
            other => other,
        };
    }
    Const::Bits { width: out.1, bits: out.0, signed: false, is_unsized: false }
}

fn arith(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64, f: fn(i128, i128) -> i128, min_undef0: bool) -> Const {
    let (x, u1) = const2big(&c_bits(a), s1);
    let (y, u2) = const2big(&c_bits(b), s2);
    let undef = if min_undef0 { u1.max(u2).max(0).min(0).max(u1.max(u2).min(0)) } else { u1.max(u2) };
    let _ = undef;
    let u = if min_undef0 { u1.min(0) } else { u1.max(u2) };
    let r = rl.max(0) as usize;
    big2big(f(x, y), if rl < 0 { c_bits(a).len().max(c_bits(b).len()) } else { r }, u)
}

pub fn const_add(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    arith(a, b, s1, s2, rl, |x, y| x.wrapping_add(y), false)
}
pub fn const_sub(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    arith(a, b, s1, s2, rl, |x, y| x.wrapping_sub(y), false)
}
pub fn const_mul(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    // calc.cc:min(undef_bit_pos, 0) —— 非 0 即 -1 效果(任何 undef→全 x)。
    let (x, u1) = const2big(&c_bits(a), s1);
    let (y, u2) = const2big(&c_bits(b), s2);
    let u = u1.max(u2).min(0);
    let r = if rl < 0 { c_bits(a).len().max(c_bits(b).len()) } else { rl.max(0) as usize };
    big2big(x.wrapping_mul(y), r, u)
}
pub fn const_div(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    let bits_a = c_bits(a);
    let bits_b = c_bits(b);
    let (x, u1) = const2big(&bits_a, s1);
    let (y, u2) = const2big(&bits_b, s2);
    if y == 0 {
        return Const::Bits { width: rl, bits: vec![Bit::Sx; rl.max(0) as usize], signed: false, is_unsized: false };
    }
    let r = if rl < 0 { bits_a.len().max(bits_b.len()) } else { rl.max(0) as usize };
    big2big(x.wrapping_div(y), r, u1.max(u2))
}
pub fn const_mod(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    let bits_a = c_bits(a);
    let bits_b = c_bits(b);
    let (x, u1) = const2big(&bits_a, s1);
    let (y, u2) = const2big(&bits_b, s2);
    if y == 0 {
        return Const::Bits { width: rl, bits: vec![Bit::Sx; rl.max(0) as usize], signed: false, is_unsized: false };
    }
    let r = if rl < 0 { bits_a.len().max(bits_b.len()) } else { rl.max(0) as usize };
    big2big(x.wrapping_rem(y), r, u1.max(u2))
}
pub fn const_divfloor(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    let bits_a = c_bits(a);
    let bits_b = c_bits(b);
    let (x, u1) = const2big(&bits_a, s1);
    let (y, u2) = const2big(&bits_b, s2);
    if y == 0 {
        return Const::Bits { width: rl, bits: vec![Bit::Sx; rl.max(0) as usize], signed: false, is_unsized: false };
    }
    let result_pos = (x < 0) == (y < 0);
    let ax = x.abs();
    let by = y.abs();
    let r = if result_pos || ax == 0 { ax / by } else { -((ax + by - 1) / by) };
    let rr = if rl < 0 { bits_a.len().max(bits_b.len()) } else { rl.max(0) as usize };
    big2big(r, rr, u1.max(u2))
}
pub fn const_modfloor(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    let bits_a = c_bits(a);
    let bits_b = c_bits(b);
    let (x, u1) = const2big(&bits_a, s1);
    let (y, u2) = const2big(&bits_b, s2);
    if y == 0 {
        return Const::Bits { width: rl, bits: vec![Bit::Sx; rl.max(0) as usize], signed: false, is_unsized: false };
    }
    let a_sign = x < 0;
    let b_sign = y < 0;
    let ax = x.abs();
    let by = y.abs();
    let truncated = if a_sign { -(ax % by) } else { ax % by };
    let modulo = if truncated == 0 || a_sign == b_sign {
        truncated
    } else if b_sign {
        truncated - y
    } else {
        truncated + y
    };
    let rr = if rl < 0 { bits_a.len().max(bits_b.len()) } else { rl.max(0) as usize };
    big2big(modulo, rr, u1.max(u2))
}

pub fn const_pow(a: &Const, b: &Const, s1: bool, s2: bool, rl: i64) -> Const {
    let bits_a = c_bits(a);
    let bits_b = c_bits(b);
    let (x, u1) = const2big(&bits_a, s1);
    let (y, u2) = const2big(&bits_b, s2);
    let r = if rl < 0 { bits_a.len().max(bits_b.len()) } else { rl.max(0) as usize };
    if x == 0 && y < 0 {
        return Const::Bits { width: rl, bits: vec![Bit::Sx; r], signed: false, is_unsized: false };
    }
    if x == 0 && y > 0 {
        return Const::Bits { width: rl, bits: vec![Bit::S0; r], signed: false, is_unsized: false };
    }
    let mut result: i128 = 1;
    if y < 0 {
        if x < -1 || x > 1 {
            result = 0;
        }
        if x == -1 {
            result = if (-y) % 2 == 0 { 1 } else { -1 };
        }
    }
    if y > 0 {
        let modulus: i128 = 1i128 << (if rl >= 0 { rl } else { 1024 }.min(126));
        let mut base = x;
        let mut e = y;
        let mut flip = false;
        if base < 0 {
            base *= -1;
            if e % 2 == 1 {
                flip = true;
            }
        }
        let mut acc: i128 = 1;
        while e > 0 {
            if e % 2 == 1 {
                acc = (acc * base) % modulus;
            }
            e /= 2;
            base = (base * base) % modulus;
        }
        result = acc;
        if flip {
            result *= -1;
        }
    }
    big2big(result, r, u1.max(u2).min(0))
}

pub fn const_pos(a: &Const, _b: &Const, s1: bool, _s2: bool, rl: i64) -> Const {
    let mut arg = c_bits(a);
    extend(&mut arg, rl.max(0) as usize, s1);
    b2c(arg)
}

pub fn const_neg(a: &Const, _b: &Const, s1: bool, _s2: bool, rl: i64) -> Const {
    let zero = Const::Bits { width: 1, bits: vec![Bit::S0], signed: false, is_unsized: false };
    const_sub(&zero, a, true, s1, rl)
}

pub fn const_mux(a: &Const, b: &Const, s: &Const) -> Const {
    let ab = c_bits(a);
    let bb = c_bits(b);
    let sb = c_bits(s);
    if sb.first() == Some(&Bit::S0) {
        return b2c(ab);
    }
    if sb.first() == Some(&Bit::S1) {
        return b2c(bb);
    }
    let mut ret = ab.clone();
    for i in 0..ret.len() {
        if ret[i] != bb[i] {
            ret[i] = Bit::Sx;
        }
    }
    b2c(ret)
}

pub fn const_pmux(a: &Const, b: &Const, s: &Const) -> Const {
    let ab = c_bits(a);
    let bb = c_bits(b);
    let sb = c_bits(s);
    if sb.iter().all(|&x| x == Bit::S0) {
        return b2c(ab);
    }
    let ones = sb.iter().filter(|&&x| x == Bit::S1).count();
    if ones != 1 || sb.iter().any(|&x| !matches!(x, Bit::S0 | Bit::S1)) {
        return Const::Bits { width: ab.len() as i64, bits: vec![Bit::Sx; ab.len()], signed: false, is_unsized: false };
    }
    for i in 0..sb.len() {
        if sb[i] == Bit::S1 {
            return b2c(bb[i * ab.len()..(i + 1) * ab.len()].to_vec());
        }
    }
    b2c(ab)
}

/// Const::is_onehot(恰一位 S1,其餘 S0;返回位號)。
pub fn const_is_onehot(c: &Const) -> Option<usize> {
    let bits = c_bits(c);
    let mut pos: Option<usize> = None;
    for (i, b) in bits.iter().enumerate() {
        match b {
            Bit::S1 => {
                if pos.is_some() {
                    return None;
                }
                pos = Some(i);
            }
            Bit::S0 => {}
            _ => return None,
        }
    }
    pos
}

/// Const::is_fully_zero / as_int(saturating int)。
pub fn const_as_int_sat(c: &Const, signed_ext: bool) -> i64 {
    let bits = c_bits(c);
    let (v, u) = const2big(&bits, signed_ext);
    if u >= 0 {
        return 0;
    }
    v as i64
}

pub fn const_bmux(a: &Const, s: &Const) -> Const {
    let mut t = c_bits(a);
    let sb = c_bits(s);
    for i in (0..sb.len()).rev() {
        let sel = sb[i];
        let half = t.len() / 2;
        let mut new_t: Vec<Bit> = Vec::new();
        if sel == Bit::S0 {
            new_t = t[..half].to_vec();
        } else if sel == Bit::S1 {
            new_t = t[half..].to_vec();
        } else {
            for j in 0..half {
                new_t.push(if t[j] == t[j + half] { t[j] } else { Bit::Sx });
            }
        }
        t = new_t;
    }
    b2c(t)
}

pub fn const_demux(a: &Const, s: &Const) -> Const {
    let width = c_bits(a).len();
    let sw = c_bits(s).len();
    let mut res: Vec<Bit> = Vec::new();
    let sb = c_bits(s);
    for i in 0..(1usize << sw) {
        let mut ne = false;
        for j in 0..sw {
            let bit = i & (1 << j) != 0;
            let expected = if bit { Bit::S0 } else { Bit::S1 };
            if sb[j] == expected {
                ne = true;
                break;
            }
        }
        if ne {
            res.extend(vec![Bit::S0; width]);
        } else {
            res.extend(c_bits(a));
        }
    }
    b2c(res)
}

pub fn const_bweqx(a: &Const, b: &Const) -> Const {
    let ab = c_bits(a);
    let bb = c_bits(b);
    logic_wrapper(logic_xnor, &b2c(ab), &b2c(bb), false, false, -1)
}

pub fn const_bwmux(a: &Const, b: &Const, s: &Const) -> Const {
    let ab = c_bits(a);
    let bb = c_bits(b);
    let sb = c_bits(s);
    let mut out = ab.clone();
    for i in 0..ab.len() {
        out[i] = if sb[i] == Bit::S1 { bb[i] } else if sb[i] == Bit::S0 { ab[i] } else if ab[i] == bb[i] { ab[i] } else { Bit::Sx };
    }
    b2c(out)
}
