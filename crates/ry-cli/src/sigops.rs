//! SigSpec/SigBit 運算工具層(proc 家族 pass 的共用語義;kernel/rtlil.cc
//! 移植面)。M1-d 的 sigmap.rs 只覆蓋 union-find;本檔補齊 proc 弧需要的
//! 值域操作:replace/remove2/sort(_and_unify)/extend_u0/謂詞族/排序序。

use ry_ir::{Bit, Const, SigBit, SigSpec};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// SigBit:相等/排序/謂詞(kernel/rtlil.h:2641 operator< 逐字)
// ---------------------------------------------------------------------------

/// SigBit operator<(rtlil.h:2641):同名 wire 位比 offset;兩 wire 比 name;
/// const 與 wire 比「是 wire」(const=false=0 < wire=true=1 → const 在前)。
/// data 的排序=State enum 序(S0<S1<Sx<Sz<Sm<Sa,rtlil.h enum 定義序)。
pub fn bit_lt(a: &SigBit, b: &SigBit) -> bool {
    match (a, b) {
        (
            SigBit::Wire { wire: wa, offset: oa },
            SigBit::Wire { wire: wb, offset: ob },
        ) => {
            if wa == wb {
                oa < ob
            } else {
                wa < wb
            }
        }
        (SigBit::Const(x), SigBit::Const(y)) => bit_state_ord(*x) < bit_state_ord(*y),
        (SigBit::Const(_), SigBit::Wire { .. }) => true,
        (SigBit::Wire { .. }, SigBit::Const(_)) => false,
    }
}

fn bit_state_ord(b: Bit) -> u8 {
    match b {
        Bit::S0 => 0,
        Bit::S1 => 1,
        Bit::Sx => 2,
        Bit::Sz => 3,
        Bit::Sm => 4,
        Bit::Sa => 5,
    }
}

// ---------------------------------------------------------------------------
// SigSpec 謂詞族(rtlil.cc is_fully_* 逐字;chunk 視圖=逐位等價)
// ---------------------------------------------------------------------------

pub fn is_fully_const(sig: &SigSpec) -> bool {
    sig.bits.iter().all(|b| matches!(b, SigBit::Const(_)))
}

/// 全定值:無 wire 位且各位皆 S0/S1。
pub fn is_fully_def(sig: &SigSpec) -> bool {
    sig.bits
        .iter()
        .all(|b| matches!(b, SigBit::Const(Bit::S0) | SigBit::Const(Bit::S1)))
}

pub fn is_fully_zero(sig: &SigSpec) -> bool {
    sig.bits.iter().all(|b| *b == SigBit::Const(Bit::S0))
}

/// SigSpec::is_fully_undef(全 x/z 的常數位;has_marked_bits 的 Sm 不算)。
pub fn is_fully_undef(sig: &SigSpec) -> bool {
    sig.bits
        .iter()
        .all(|b| matches!(b, SigBit::Const(Bit::Sx) | SigBit::Const(Bit::Sz)))
}

/// Const::as_bool(位 0=S1;kernel/rtlil.h)。
pub fn const_as_bool(c: &Const) -> bool {
    match c {
        Const::Str(_) => false,
        Const::Bits { bits, .. } => bits.first() == Some(&Bit::S1),
    }
}

// ---------------------------------------------------------------------------
// SigSpec 操作(rtlil.cc 逐字;pattern 域只作用 wire 位)
// ---------------------------------------------------------------------------

/// SigSpec::replace(pattern, with, other)(rtlil.cc:4881):pattern_to_with
/// 只收 wire 位;遍歷 self,命中 wire 位 → other[j] = with[idx]。
pub fn replace_into(pattern: &SigSpec, with: &SigSpec, sig: &SigSpec, other: &mut SigSpec) {
    debug_assert_eq!(sig.bits.len(), other.bits.len());
    debug_assert_eq!(pattern.bits.len(), with.bits.len());
    let mut pattern_to_with: HashMap<(String, i64), usize> = HashMap::new();
    for (i, b) in pattern.bits.iter().enumerate() {
        if let SigBit::Wire { wire, offset } = b {
            pattern_to_with.entry((wire.clone(), *offset)).or_insert(i);
        }
    }
    for (j, b) in sig.bits.iter().enumerate() {
        if let SigBit::Wire { wire, offset } = b {
            if let Some(&idx) = pattern_to_with.get(&(wire.clone(), *offset)) {
                other.bits[j] = with.bits[idx].clone();
            }
        }
    }
}

/// SigSpec::remove2(pattern, other)(rtlil.cc:4976):把 self 中的 pattern
/// wire 位刪除(保持序),同步刪 other 對應位。常數位永不刪。
pub fn remove2(pattern: &SigSpec, sig: &mut SigSpec, other: &mut SigSpec) {
    let mut keep: Vec<usize> = Vec::new();
    for (i, b) in sig.bits.iter().enumerate() {
        match b {
            SigBit::Wire { wire, offset } => {
                let hit = pattern.bits.iter().any(|p| match p {
                    SigBit::Wire { wire: pw, offset: po } => pw == wire && po == offset,
                    _ => false,
                });
                if !hit {
                    keep.push(i);
                }
            }
            SigBit::Const(_) => keep.push(i),
        }
    }
    if keep.len() != sig.bits.len() {
        let mut ns = SigSpec::default();
        let mut no = SigSpec::default();
        for &i in &keep {
            ns.bits.push(sig.bits[i].clone());
            no.bits.push(other.bits[i].clone());
        }
        *sig = ns;
        *other = no;
    }
}

/// SigSpec::sort_and_unify(rtlil.cc:4860):排序(bit_lt 全序)去重。
pub fn sort_and_unify(sig: &mut SigSpec) {
    sig.bits.sort_by(|a, b| bit_cmp(a, b));
    sig.bits.dedup();
}

fn bit_cmp(a: &SigBit, b: &SigBit) -> std::cmp::Ordering {
    if bit_lt(a, b) {
        std::cmp::Ordering::Less
    } else if bit_lt(b, a) {
        std::cmp::Ordering::Greater
    } else {
        std::cmp::Ordering::Equal
    }
}

/// extend_u0(calc.cc:29):signed 補最高位、否則補 S0,至 width。
pub fn extend_u0(sig: &mut SigSpec, width: i64, is_signed: bool) {
    let padding = if is_signed {
        sig.bits.last().cloned().unwrap_or(SigBit::Const(Bit::S0))
    } else {
        SigBit::Const(Bit::S0)
    };
    while (sig.bits.len() as i64) < width {
        sig.bits.push(padding.clone());
    }
}

/// SigSpec::extract_end(offset):取 offset 起到尾。
pub fn extract_end(sig: &SigSpec, offset: i64) -> SigSpec {
    SigSpec { bits: sig.bits[offset.max(0) as usize..].to_vec() }
}

/// Const(State, width) 對應的 SigSpec。
pub fn sig_const(b: Bit, width: i64) -> SigSpec {
    SigSpec { bits: vec![SigBit::Const(b); width.max(0) as usize] }
}

/// 從 Const 構 SigSpec(sigspec 域丟 signed/unsized flags,R7)。
pub fn sig_from_const(c: &Const) -> SigSpec {
    match c {
        Const::Str(_) => SigSpec::default(),
        Const::Bits { bits, .. } => SigSpec { bits: bits.iter().map(|&b| SigBit::Const(b)).collect() },
    }
}

/// SigSpec::has_marked_bits(含 Sm 常數位;proc_arst 的 error 判定)。
pub fn has_marked_bits(sig: &SigSpec) -> bool {
    sig.bits
        .iter()
        .any(|b| *b == SigBit::Const(Bit::Sm))
}

/// IdString::unescape(rtlil.h:280 精確規則):剝 `\` 僅當 str[1] 非
/// `$`/`\`/數字;`\top`→`top`、`\$proc`→`\$proc`(保留)。
pub fn id_unescape(name: &str) -> String {
    let b = name.as_bytes();
    if b.len() >= 2 && b[0] == b'\\' && b[1] != b'$' && b[1] != b'\\' && !b[1].is_ascii_digit() {
        return name[1..].to_string();
    }
    name.to_string()
}

// ---------------------------------------------------------------------------
// log_signal(kernel/log.cc:580 = dump_sigspec autoint 形;printer::log_signal
// 需 wire 寬表——此處包一層以 Module 構表)
// ---------------------------------------------------------------------------

pub fn log_signal(sig: &SigSpec, widths: &HashMap<String, i64>) -> String {
    let view: HashMap<&str, i64> = widths.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    ry_ir::printer::log_signal(sig, &view)
}

/// log_id 形(IdString %s=原始形;已由呼叫端以 `.name` 直拼,此處僅文檔錨)。

// ---------------------------------------------------------------------------
// 常數值域(Const<->i64;proc_rom 的地址運算/proc_memwr PORTID 等)
// ---------------------------------------------------------------------------

/// Const::as_int(saturating to i32 界;語料內皆小值)。u32 位序 LSB-first。
pub fn const_as_int(c: &Const) -> i64 {
    match c {
        Const::Str(_) => 0,
        Const::Bits { bits, .. } => {
            let mut v: i64 = 0;
            for (i, b) in bits.iter().enumerate() {
                if *b == Bit::S1 {
                    v |= 1 << i;
                }
            }
            v
        }
    }
}

