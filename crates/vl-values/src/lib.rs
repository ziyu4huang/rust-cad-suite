//! 4-state bit vectors: `{data, mask}` per limb. Bit semantics:
//! `mask=0` → data bit valid (0/1); `mask=1, data=0` → x; `mask=1, data=1` → z.
//! Two representations (ADR-0008): widths ≤ 128 stay in a single `u128` limb
//! (the pre-M8 layout, byte-identical math); wider values spill to
//! little-endian `u64` limb vectors. Results whose width ≤ 128 are always
//! canonicalized back to the small arm, so the interpreter hot loop never
//! sees the heap form. The public shape stays a struct with a `width` field
//! (dozens of call sites read `bits.width` directly).

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Bits {
    pub width: u32,
    repr: Repr,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum Repr {
    /// width ≤ 128 — single-limb, math identical to the pre-M8 layout.
    N { data: u128, mask: u128 },
    /// width > 128 — little-endian u64 limbs, len = ceil(width/64), the high
    /// limb pre-masked to the width (bits ≥ width are always 0).
    W { data: Box<[u64]>, mask: Box<[u64]> },
}

pub const MAX_WIDTH: u32 = 1 << 20;

#[derive(Debug, Clone)]
pub struct ValueError(pub String);

impl std::fmt::Display for ValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub type VResult<T> = Result<T, ValueError>;

impl Bits {
    /// Const small-arm constructor (gate constants; width ≤ 128 callers).
    pub const fn n(width: u32, data: u128, mask: u128) -> Bits {
        Bits {
            width,
            repr: Repr::N { data, mask },
        }
    }

    pub fn is_wide(&self) -> bool {
        matches!(self.repr, Repr::W { .. })
    }

    /// Raw low-128 data bits (NOT mask-applied) — bit-0 probes at call sites.
    pub fn low_data(&self) -> u128 {
        match &self.repr {
            Repr::N { data, .. } => *data,
            Repr::W { data, .. } => {
                data.first().copied().unwrap_or(0) as u128
                    | ((data.get(1).copied().unwrap_or(0) as u128) << 64)
            }
        }
    }

    /// Raw low-128 mask bits.
    pub fn low_mask(&self) -> u128 {
        match &self.repr {
            Repr::N { mask, .. } => *mask,
            Repr::W { mask, .. } => {
                mask.first().copied().unwrap_or(0) as u128
                    | ((mask.get(1).copied().unwrap_or(0) as u128) << 64)
            }
        }
    }
}

fn chk(width: u32) -> VResult<()> {
    if width > MAX_WIDTH {
        Err(ValueError(format!(
            "vectors wider than {MAX_WIDTH} bits are not yet supported (M8 cap)"
        )))
    } else {
        Ok(())
    }
}

/// Declared-range width `[msv:lsv]` with an explicit cap error (i64 → u32
/// casts silently wrapped before M8).
pub fn range_width(msv: i64, lsv: i64) -> VResult<u32> {
    let w = (msv as i128 - lsv as i128).abs() + 1;
    if w > MAX_WIDTH as i128 {
        Err(ValueError(format!(
            "declared range width {w} exceeds the {MAX_WIDTH}-bit limit"
        )))
    } else {
        Ok(w as u32)
    }
}

pub fn low_bits(width: u32) -> u128 {
    if width == 0 {
        0
    } else if width >= 128 {
        !0u128
    } else {
        (1u128 << width) - 1
    }
}

pub fn limbs_for(width: u32) -> usize {
    width.div_ceil(64) as usize
}

fn full_mask_vec(width: u32) -> Vec<u64> {
    let n = limbs_for(width);
    let mut v = vec![u64::MAX; n];
    let rem = width % 64;
    if rem != 0 {
        v[n - 1] = (1u64 << rem) - 1;
    }
    v
}

/// Truncate raw limb accumulators to the canonical `width` form: drop limbs
/// past the width, mask the high limb.
fn trunc_limbs(data: &[u64], mask: &[u64], width: u32) -> (Box<[u64]>, Box<[u64]>) {
    let n = limbs_for(width);
    let mut d = vec![0u64; n];
    let mut m = vec![0u64; n];
    for i in 0..n.min(data.len()) {
        d[i] = data[i];
    }
    for i in 0..n.min(mask.len()) {
        m[i] = mask[i];
    }
    let rem = width % 64;
    if rem != 0 {
        d[n - 1] &= (1u64 << rem) - 1;
        m[n - 1] &= (1u64 << rem) - 1;
    }
    (d.into_boxed_slice(), m.into_boxed_slice())
}

fn limbs_to_u128_raw(data: &[u64]) -> u128 {
    data.first().copied().unwrap_or(0) as u128
        | ((data.get(1).copied().unwrap_or(0) as u128) << 64)
}

fn limbs_to_u128(data: &[u64], mask: &[u64]) -> u128 {
    let d0 = data.first().copied().unwrap_or(0) as u128;
    let d1 = data.get(1).copied().unwrap_or(0) as u128;
    let m0 = mask.first().copied().unwrap_or(0) as u128;
    let m1 = mask.get(1).copied().unwrap_or(0) as u128;
    (d0 & !m0) | ((d1 & !m1) << 64)
}

/// Canonical pack: width ≤ 128 → the small arm; wider → masked limb arm.
fn pack(width: u32, data: &[u64], mask: &[u64]) -> VResult<Bits> {
    chk(width)?;
    if width == 0 {
        return Ok(Bits::n(0, 0, 0));
    }
    if width <= 128 {
        let lb = low_bits(width) as u128;
        let d0 = data.first().copied().unwrap_or(0) as u128;
        let d1 = data.get(1).copied().unwrap_or(0) as u128;
        let m0 = mask.first().copied().unwrap_or(0) as u128;
        let m1 = mask.get(1).copied().unwrap_or(0) as u128;
        Ok(Bits {
            width,
            repr: Repr::N {
                data: (d0 | (d1 << 64)) & lb,
                mask: (m0 | (m1 << 64)) & lb,
            },
        })
    } else {
        let (d, m) = trunc_limbs(data, mask, width);
        Ok(Bits {
            width,
            repr: Repr::W { data: d, mask: m },
        })
    }
}

// ---------- wide limb engine (pub: shared-leaf surface; the old "interp/vpi readmem+VPI parse" note was stale — workspace grep at extraction found zero external users, and riv-kernel is the planned consumer) ----------

pub struct Limbs {
    pub data: Vec<u64>,
    pub mask: Vec<u64>,
}

impl Limbs {
    pub fn zeros(width: u32) -> Limbs {
        let n = limbs_for(width);
        Limbs {
            data: vec![0; n],
            mask: vec![0; n],
        }
    }

    /// Build from a value at its own width (no extension).
    pub fn of(b: &Bits) -> Limbs {
        match &b.repr {
            Repr::N { data, mask } => {
                let n = limbs_for(b.width).max(1);
                let mut d = vec![*data as u64, (*data >> 64) as u64];
                let mut m = vec![*mask as u64, (*mask >> 64) as u64];
                d.truncate(n);
                m.truncate(n);
                Limbs { data: d, mask: m }
            }
            Repr::W { data, mask } => Limbs {
                data: data.to_vec(),
                mask: mask.to_vec(),
            },
        }
    }

    /// Zero-extend / truncate to `width` limbs.
    pub fn padded(b: &Bits, width: u32) -> Limbs {
        let n = limbs_for(width);
        let mut l = Limbs::zeros(width);
        let rem = width % 64;
        match &b.repr {
            Repr::N { data, mask } => {
                let d = [*data as u64, (*data >> 64) as u64];
                let m = [*mask as u64, (*mask >> 64) as u64];
                for i in 0..n.min(2) {
                    l.data[i] = d[i];
                    l.mask[i] = m[i];
                }
            }
            Repr::W { data, mask } => {
                for i in 0..n.min(data.len()) {
                    l.data[i] = data[i];
                }
                for i in 0..n.min(mask.len()) {
                    l.mask[i] = mask[i];
                }
            }
        }
        if rem != 0 {
            l.data[n - 1] &= (1u64 << rem) - 1;
            l.mask[n - 1] &= (1u64 << rem) - 1;
        }
        l
    }

    pub fn pack(&self, width: u32) -> VResult<Bits> {
        pack(width, &self.data, &self.mask)
    }

    pub fn to_u128(&self) -> u128 {
        limbs_to_u128(&self.data, &self.mask)
    }

    /// `dst |= src << sh` (bit-granular insert; src bits above its own width
    /// are zero by invariant).
    pub fn insert(&mut self, src: &Limbs, sh: u32) {
        let (li, off) = ((sh / 64) as usize, sh % 64);
        for (i, &s) in src.data.iter().enumerate() {
            let idx = li + i;
            if idx >= self.data.len() {
                break;
            }
            self.data[idx] |= s << off;
            if off > 0 && idx + 1 < self.data.len() {
                self.data[idx + 1] |= s >> (64 - off);
            }
        }
        for (i, &s) in src.mask.iter().enumerate() {
            let idx = li + i;
            if idx >= self.mask.len() {
                break;
            }
            self.mask[idx] |= s << off;
            if off > 0 && idx + 1 < self.mask.len() {
                self.mask[idx + 1] |= s >> (64 - off);
            }
        }
    }
}

fn limbs_add(a: &[u64], b: &[u64], n: usize) -> Vec<u64> {
    let mut out = vec![0u64; n];
    let mut carry = 0u64;
    for i in 0..n {
        let (r, c1) = a[i].overflowing_add(b[i]);
        let (r2, c2) = r.overflowing_add(carry);
        out[i] = r2;
        carry = c1 as u64 + c2 as u64;
    }
    out
}

fn limbs_sub(a: &[u64], b: &[u64], n: usize) -> Vec<u64> {
    let mut out = vec![0u64; n];
    let mut borrow = 0u64;
    for i in 0..n {
        let (r, b1) = a[i].overflowing_sub(b[i]);
        let (r2, b2) = r.overflowing_sub(borrow);
        out[i] = r2;
        borrow = b1 as u64 + b2 as u64;
    }
    out
}

fn limbs_neg(a: &[u64], n: usize) -> Vec<u64> {
    let zero = vec![0u64; n];
    limbs_sub(&zero, a, n)
}

/// Negate as a `width`-bit two's complement (masks the padding above width).
fn limbs_neg_width(a: &[u64], n: usize, width: u32) -> Vec<u64> {
    let mut r = limbs_neg(a, n);
    mask_width(&mut r, width);
    r
}

/// Schoolbook multiply, truncated to `n` limbs (mod 2^(64n)).
fn limbs_mul(a: &[u64], b: &[u64], n: usize) -> Vec<u64> {
    let mut out = vec![0u64; n];
    for i in 0..n {
        let mut carry = 0u128;
        for j in 0..(n - i) {
            let cur = out[i + j] as u128 + (a[i] as u128) * (b[j] as u128) + carry;
            out[i + j] = cur as u64;
            carry = cur >> 64;
        }
    }
    out
}

/// Modular power `base^exp mod 2^width` via square-and-multiply (exp ≤ 2^20;
/// caller guards the cap and the 0/1 base special cases).
fn pow_mod2w(base: &[u64], exp: u128, width: u32) -> Vec<u64> {
    let n = limbs_for(width);
    let mut acc = vec![0u64; n];
    acc[0] = 1; // the number ONE in little-endian limbs (vec![1; n] would be
                // [1,1,1,1] — the bug that made every pow limb garbage)
    let mut sq = base.to_vec();
    sq.resize(n, 0);
    mask_width(&mut sq, width);
    let mut e = exp;
    while e > 0 {
        if e & 1 == 1 {
            acc = limbs_mul(&acc, &sq, n);
            mask_width(&mut acc, width);
        }
        e >>= 1;
        if e > 0 {
            sq = limbs_mul(&sq, &sq, n);
            mask_width(&mut sq, width);
        }
    }
    acc
}

fn limbs_cmp(a: &[u64], b: &[u64]) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    for i in (0..a.len().min(b.len())).rev() {
        match a[i].cmp(&b[i]) {
            Ordering::Equal => continue,
            o => return o,
        }
    }
    Ordering::Equal
}

/// Bitwise restoring long division on equal-length limb vectors.
/// Returns (quotient, remainder); caller guarantees `b != 0`.
fn limbs_divmod(a: &[u64], b: &[u64], width: u32) -> (Vec<u64>, Vec<u64>) {
    let n = limbs_for(width) + 1; // one extra limb so r<<1 cannot overflow
    let mut bp = vec![0u64; n];
    for (i, &v) in b.iter().enumerate().take(n) {
        bp[i] = v;
    }
    let mut q = vec![0u64; n];
    let mut r = vec![0u64; n];
    for i in (0..width).rev() {
        // r = (r << 1) | bit_i(a)
        let mut carry = (a[(i / 64) as usize] >> (i % 64)) & 1;
        for limb in r.iter_mut() {
            let nc = *limb >> 63;
            *limb = (*limb << 1) | carry;
            carry = nc;
        }
        if limbs_cmp(&r, &bp) != std::cmp::Ordering::Less {
            r = limbs_sub(&r, &bp, n);
            q[(i / 64) as usize] |= 1u64 << (i % 64);
        }
    }
    q.truncate(limbs_for(width));
    r.truncate(limbs_for(width));
    (q, r)
}

/// Long division of a limb vector by a single word: (quotient, remainder).
pub fn limbs_divmod_small(a: &[u64], d: u64) -> (Vec<u64>, u64) {
    let mut q = vec![0u64; a.len()];
    let mut rem = 0u128;
    for i in (0..a.len()).rev() {
        let cur = (rem << 64) | a[i] as u128;
        q[i] = (cur / d as u128) as u64;
        rem = cur % d as u128;
    }
    (q, rem as u64)
}

/// Zero the bits above `width` in a limb vector (limb ops run on whole
/// limbs; every signed intermediate must be re-masked or garbage above the
/// width compounds — e.g. negating a 200-bit value as 256 bits).
fn mask_width(v: &mut [u64], width: u32) {
    let rem = width % 64;
    if rem != 0 {
        let n = limbs_for(width);
        if n >= 1 && n <= v.len() {
            v[n - 1] &= (1u64 << rem) - 1;
        }
    }
}

fn limbs_is_zero(v: &[u64]) -> bool {
    v.iter().all(|&x| x == 0)
}

/// Decimal fold: `v = v*10 + digit` on a growing limb vector.
pub fn limbs_mul_add_small(v: &mut Vec<u64>, mul: u64, add: u64) {
    let mut carry = add as u128;
    for limb in v.iter_mut() {
        let cur = (*limb as u128) * (mul as u128) + carry;
        *limb = cur as u64;
        carry = cur >> 64;
    }
    while carry > 0 {
        v.push(carry as u64);
        carry >>= 64;
    }
}

fn limbs_sign_bit(data: &[u64], width: u32) -> bool {
    if width == 0 {
        return false;
    }
    let li = ((width - 1) / 64) as usize;
    let bit = 1u64 << ((width - 1) % 64);
    data.get(li).copied().unwrap_or(0) & bit != 0
}

fn limbs_sign_limb(l: &Limbs, width: u32) -> (bool, bool) {
    if width == 0 {
        return (false, false);
    }
    let li = ((width - 1) / 64) as usize;
    let bit = 1u64 << ((width - 1) % 64);
    (
        l.data.get(li).copied().unwrap_or(0) & bit != 0,
        l.mask.get(li).copied().unwrap_or(0) & bit != 0,
    )
}

fn shl_bits_in_place(v: &mut [u64], s: u32) {
    let mut carry = 0u64;
    for limb in v.iter_mut() {
        let nc = *limb >> (64 - s);
        *limb = (*limb << s) | carry;
        carry = nc;
    }
}

fn known_nonzero_above(data: &[u64], mask: &[u64], bit_index: u32) -> bool {
    let li = (bit_index / 64) as usize;
    let off = bit_index % 64;
    if off > 0 {
        if let (Some(&d), Some(&m)) = (data.get(li), mask.get(li)) {
            if (d & !m) >> off != 0 {
                return true;
            }
        }
    }
    for i in (li + if off > 0 { 1 } else { 0 })..data.len() {
        let d = data.get(i).copied().unwrap_or(0);
        let m = mask.get(i).copied().unwrap_or(0);
        if d & !m != 0 {
            return true;
        }
    }
    false
}

impl Bits {
    pub fn zeros(width: u32) -> VResult<Bits> {
        chk(width)?;
        if width <= 128 {
            Ok(Bits::n(width, 0, 0))
        } else {
            let n = limbs_for(width);
            Ok(Bits {
                width,
                repr: Repr::W {
                    data: vec![0; n].into_boxed_slice(),
                    mask: vec![0; n].into_boxed_slice(),
                },
            })
        }
    }

    pub fn from_u64(width: u32, v: u64) -> VResult<Bits> {
        Bits::from_u128(width, v as u128)
    }

    pub fn from_u128(width: u32, v: u128) -> VResult<Bits> {
        chk(width)?;
        let dropped = if width >= 128 { 0 } else { v >> width };
        if dropped != 0 {
            return Err(ValueError(format!(
                "value {v} does not fit in {width} bits"
            )));
        }
        if width <= 128 {
            Ok(Bits::n(width, v, 0))
        } else {
            let n = limbs_for(width);
            let mut d = vec![0u64; n];
            d[0] = v as u64;
            d[1] = (v >> 64) as u64;
            Ok(Bits {
                width,
                repr: Repr::W {
                    data: d.into_boxed_slice(),
                    mask: vec![0; n].into_boxed_slice(),
                },
            })
        }
    }

    /// Small-arm constructor for width ≤ 128 callers (interp/vpi internals).
    pub fn from_parts(width: u32, data: u128, mask: u128) -> VResult<Bits> {
        chk(width)?;
        if width <= 128 {
            Ok(Bits::n(width, data, mask))
        } else {
            let n = limbs_for(width);
            let mut d = vec![0u64; n];
            let mut m = vec![0u64; n];
            d[0] = data as u64;
            d[1] = (data >> 64) as u64;
            m[0] = mask as u64;
            m[1] = (mask >> 64) as u64;
            let (d, m) = trunc_limbs(&d, &m, width);
            Ok(Bits {
                width,
                repr: Repr::W { data: d, mask: m },
            })
        }
    }

    /// All-x (used by conservative arithmetic X and unknown shifts).
    pub fn all_x(width: u32) -> VResult<Bits> {
        chk(width)?;
        if width <= 128 {
            Ok(Bits::n(width, 0, low_bits(width)))
        } else {
            let m = full_mask_vec(width).into_boxed_slice();
            let n = m.len();
            Ok(Bits {
                width,
                repr: Repr::W {
                    data: vec![0; n].into_boxed_slice(),
                    mask: m,
                },
            })
        }
    }

    pub fn is_known(&self) -> bool {
        match &self.repr {
            Repr::N { mask, .. } => *mask == 0,
            Repr::W { mask, .. } => mask.iter().all(|&m| m == 0),
        }
    }

    pub fn has_unknown(&self) -> bool {
        !self.is_known()
    }

    /// Known-bit value; unknown bits read as 0 (x-initial policy = 0).
    /// Low 64 bits only — use [`Bits::to_u128_lossy`] for full-width values.
    pub fn to_u64_lossy(&self) -> u64 {
        self.to_u128_lossy() as u64
    }

    pub fn to_u128_lossy(&self) -> u128 {
        match &self.repr {
            Repr::N { data, mask } => data & !mask,
            Repr::W { data, mask } => limbs_to_u128(data, mask),
        }
    }

    /// Signed interpretation of the known bits (x MSB reads as 0).
    pub fn to_i64_lossy(&self) -> i64 {
        self.to_i128_lossy() as i64
    }

    /// Full-width signed interpretation; callers doing signed arithmetic or
    /// compares on wide (>128-bit) values must use the limb-aware signed
    /// helpers (`cmp_signed`/`sdiv`/`srem`/`spow`/`fmt_dec_signed`) instead —
    /// this form only sees the low 128 bits.
    pub fn to_i128_lossy(&self) -> i128 {
        let u = self.to_u128_lossy();
        if self.width == 0 {
            return 0;
        }
        if self.width >= 128 {
            return u as i128;
        }
        if u & (1u128 << (self.width - 1)) != 0 {
            (u | !low_bits(self.width)) as i128
        } else {
            u as i128
        }
    }

    pub fn trunc(&self, width: u32) -> VResult<Bits> {
        chk(width)?;
        match &self.repr {
            Repr::N { data, mask } => {
                let lb = low_bits(width);
                Ok(Bits {
                    width,
                    repr: Repr::N {
                        data: data & lb,
                        mask: mask & lb,
                    },
                })
            }
            Repr::W { data, mask } => {
                if width <= 128 {
                    let lb = low_bits(width);
                    let d = limbs_to_u128_raw(data);
                    let m = limbs_to_u128_raw(mask);
                    Ok(Bits {
                        width,
                        repr: Repr::N {
                            data: d & lb,
                            mask: m & lb,
                        },
                    })
                } else {
                    let (d, m) = trunc_limbs(data, mask, width);
                    Ok(Bits {
                        width,
                        repr: Repr::W { data: d, mask: m },
                    })
                }
            }
        }
    }

    /// Zero-extend / truncate to `width` (unsigned adjust).
    pub fn uadjust(&self, width: u32) -> VResult<Bits> {
        if width > self.width {
            if width > 128 {
                return Limbs::padded(self, width).pack(width);
            }
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
        if width > 128 {
            // Limb path: sign-fill the remainder bits of the top source limb
            // first (the limb arm of the N arm's `!low_bits(sw)` fill), then
            // extend whole limbs with the sign kind. Skipping the remainder
            // fill corrupts every source width that is not a multiple of 64
            // (the N-vs-W property test caught this on its first run).
            let (sign, sign_unknown) = self.sign_bit();
            let n = limbs_for(width);
            let fill: u64 = if sign { u64::MAX } else { 0 };
            let fillm: u64 = if sign_unknown { u64::MAX } else { 0 };
            let mut l = Limbs::of(self);
            let rem = self.width % 64;
            if rem != 0 {
                let li = limbs_for(self.width) - 1;
                let hi = !((1u64 << rem) - 1);
                if sign {
                    l.data[li] |= hi;
                }
                if sign_unknown {
                    l.mask[li] |= hi;
                }
            }
            l.data.resize(n, fill);
            l.mask.resize(n, fillm);
            let (d, m) = trunc_limbs(&l.data, &l.mask, width);
            return Ok(Bits {
                width,
                repr: Repr::W { data: d, mask: m },
            });
        }
        let sw = self.width;
        let (data, mask) = match &self.repr {
            Repr::N { data, mask } => (*data, *mask),
            _ => unreachable!("wide source with narrow target truncates above"),
        };
        let sign_unknown = mask & (1u128 << (sw - 1)) != 0;
        let sign = data & (1u128 << (sw - 1)) != 0;
        let mut b = Bits::n(sw, data, mask);
        b.width = width;
        if let Repr::N { data: bd, mask: bm } = &mut b.repr {
            if sign {
                *bd |= !low_bits(sw);
            }
            if sign_unknown {
                *bm |= !low_bits(sw);
            }
            *bd &= low_bits(width);
            *bm &= low_bits(width);
        }
        Ok(b)
    }

    /// Per-bit bitwise AND with full x/z tables (0 & x = 0, 1 & x = x, z ≡ x).
    pub fn bit_and(&self, other: &Bits) -> VResult<Bits> {
        let w = self.width.max(other.width);
        chk(w)?;
        let a = self.uadjust(w)?;
        let b = other.uadjust(w)?;
        if let (&Repr::N { data: ad, mask: am }, &Repr::N { data: bd, mask: bm }) = (&a.repr, &b.repr) {
            let ones = ad & !am & bd & !bm;
            let zeros = (!ad & !am) | (!bd & !bm);
            return Ok(Bits {
                width: w,
                repr: Repr::N {
                    data: ones,
                    mask: low_bits(w) & !(ones | zeros),
                },
            });
        }
        let la = Limbs::of(&a);
        let lb = Limbs::of(&b);
        let n = limbs_for(w);
        let full = full_mask_vec(w);
        let mut ones = vec![0u64; n];
        let mut mask = vec![0u64; n];
        for i in 0..n {
            ones[i] = !la.mask[i] & la.data[i] & !lb.mask[i] & lb.data[i];
            mask[i] = full[i]
                & !(ones[i]
                    | ((!la.data[i] & !la.mask[i]) | (!lb.data[i] & !lb.mask[i])));
        }
        pack(w, &ones, &mask)
    }

    pub fn bit_or(&self, other: &Bits) -> VResult<Bits> {
        let w = self.width.max(other.width);
        chk(w)?;
        let a = self.uadjust(w)?;
        let b = other.uadjust(w)?;
        if let (&Repr::N { data: ad, mask: am }, &Repr::N { data: bd, mask: bm }) = (&a.repr, &b.repr) {
            let ones = ad & !am | bd & !bm;
            let zeros = (!ad & !am) & (!bd & !bm);
            return Ok(Bits {
                width: w,
                repr: Repr::N {
                    data: ones,
                    mask: low_bits(w) & !(ones | zeros),
                },
            });
        }
        let la = Limbs::of(&a);
        let lb = Limbs::of(&b);
        let n = limbs_for(w);
        let full = full_mask_vec(w);
        let mut ones = vec![0u64; n];
        let mut mask = vec![0u64; n];
        for i in 0..n {
            ones[i] = (!la.mask[i] & la.data[i]) | (!lb.mask[i] & lb.data[i]);
            mask[i] = full[i]
                & !(ones[i]
                    | ((!la.data[i] & !la.mask[i]) & (!lb.data[i] & !lb.mask[i])));
        }
        pack(w, &ones, &mask)
    }

    /// XOR: any unknown input bit → unknown output bit (exact elsewhere).
    pub fn bit_xor(&self, other: &Bits) -> VResult<Bits> {
        let w = self.width.max(other.width);
        chk(w)?;
        let a = self.uadjust(w)?;
        let b = other.uadjust(w)?;
        if let (&Repr::N { data: ad, mask: am }, &Repr::N { data: bd, mask: bm }) = (&a.repr, &b.repr) {
            return Ok(Bits {
                width: w,
                repr: Repr::N {
                    data: ad ^ bd,
                    mask: am | bm,
                },
            });
        }
        let la = Limbs::of(&a);
        let lb = Limbs::of(&b);
        let n = limbs_for(w);
        let mut d = vec![0u64; n];
        let mut m = vec![0u64; n];
        for i in 0..n {
            d[i] = la.data[i] ^ lb.data[i];
            m[i] = la.mask[i] | lb.mask[i];
        }
        pack(w, &d, &m)
    }

    pub fn bit_not(&self) -> Bits {
        // Known bits flip; unknown bits keep their kind (x stays x, z stays z).
        // Both arms must mask to the declared width: `!data` flips all 128
        // bits (or the full top limb), and unmasked padding garbage above
        // the width breaks the canonical-form invariant — it leaks through
        // raw-data consumers (`truthy`, `case_eq`) while per-bit renderers
        // (fmt_bin) hide it (the 42_nba_handshake tick=5 kernel bug).
        let keep = low_bits(self.width);
        match &self.repr {
            Repr::N { data, mask } => Bits {
                width: self.width,
                repr: Repr::N {
                    data: ((!data & !mask) | (data & mask)) & keep,
                    mask: *mask & keep,
                },
            },
            Repr::W { data, mask } => {
                let n = data.len();
                let rem = self.width % 64;
                let top_keep = if rem == 0 { u64::MAX } else { (1u64 << rem) - 1 };
                let mut d = vec![0u64; n];
                for i in 0..n {
                    let keep_limb = if i == n - 1 { top_keep } else { u64::MAX };
                    d[i] = ((!data[i] & !mask[i]) | (data[i] & mask[i])) & keep_limb;
                }
                Bits {
                    width: self.width,
                    repr: Repr::W {
                        data: d.into_boxed_slice(),
                        mask: mask.clone(),
                    },
                }
            }
        }
    }

    /// Both operands known → arithmetic; any unknown bit → all-x result
    /// (conservative; exact on the X-clean differential corpus; IEEE-§5-exact
    /// bit-serial arithmetic is an M4 item per ADR-0001).
    fn arith(&self, other: &Bits, f: impl Fn(u128, u128) -> u128) -> VResult<Bits> {
        let w = self.width.max(other.width);
        if self.has_unknown() || other.has_unknown() {
            return Bits::all_x(w);
        }
        let a = self.to_u128_lossy();
        let b = other.to_u128_lossy();
        let r = f(a, b);
        if w >= 128 {
            Ok(Bits {
                width: w,
                repr: Repr::N { data: r, mask: 0 },
            })
        } else {
            Bits::from_u128(w, r & low_bits(w) as u128)
        }
    }

    /// True when either operand is in the wide (limb) representation.
    fn either_wide(a: &Bits, b: &Bits) -> bool {
        matches!((&a.repr, &b.repr), (Repr::W { .. }, _) | (_, Repr::W { .. }))
    }

    fn wide_pair(&self, other: &Bits) -> (u32, Limbs, Limbs) {
        let w = self.width.max(other.width);
        (w, Limbs::padded(self, w), Limbs::padded(other, w))
    }

    pub fn add(&self, other: &Bits) -> VResult<Bits> {
        if !self.has_unknown() && !other.has_unknown() && Self::either_wide(self, other) {
            let (w, a, b) = self.wide_pair(other);
            let n = limbs_for(w);
            let d = limbs_add(&a.data, &b.data, n);
            return pack(w, &d, &vec![0; n]);
        }
        self.arith(other, |a, b| a.wrapping_add(b))
    }

    pub fn sub(&self, other: &Bits) -> VResult<Bits> {
        if !self.has_unknown() && !other.has_unknown() && Self::either_wide(self, other) {
            let (w, a, b) = self.wide_pair(other);
            let n = limbs_for(w);
            let d = limbs_sub(&a.data, &b.data, n);
            return pack(w, &d, &vec![0; n]);
        }
        self.arith(other, |a, b| a.wrapping_sub(b))
    }

    pub fn mul(&self, other: &Bits) -> VResult<Bits> {
        if !self.has_unknown() && !other.has_unknown() && Self::either_wide(self, other) {
            let (w, a, b) = self.wide_pair(other);
            let n = limbs_for(w);
            let d = limbs_mul(&a.data, &b.data, n);
            return pack(w, &d, &vec![0; n]);
        }
        self.arith(other, |a, b| a.wrapping_mul(b))
    }

    pub fn div(&self, other: &Bits) -> VResult<Bits> {
        let w = self.width.max(other.width);
        if self.has_unknown() || other.has_unknown() {
            return Bits::all_x(w);
        }
        if Self::either_wide(self, other) {
            let (_, a, b) = self.wide_pair(other);
            if limbs_is_zero(&b.data) {
                return Bits::all_x(w);
            }
            let (q, _) = limbs_divmod(&a.data, &b.data, w);
            return pack(w, &q, &vec![0; limbs_for(w)]);
        }
        let b = other.to_u128_lossy();
        if b == 0 {
            // x/0 = x (IEEE 1364-2005 §5.1.5)
            return Bits::all_x(w);
        }
        let a = self.to_u128_lossy();
        Bits::from_u128(w, a / b)
    }

    pub fn rem(&self, other: &Bits) -> VResult<Bits> {
        let w = self.width.max(other.width);
        if self.has_unknown() || other.has_unknown() {
            return Bits::all_x(w);
        }
        if Self::either_wide(self, other) {
            let (_, a, b) = self.wide_pair(other);
            if limbs_is_zero(&b.data) {
                return Bits::all_x(w);
            }
            let (_, r) = limbs_divmod(&a.data, &b.data, w);
            return pack(w, &r, &vec![0; limbs_for(w)]);
        }
        let b = other.to_u128_lossy();
        if b == 0 {
            return Bits::all_x(w);
        }
        let a = self.to_u128_lossy();
        Bits::from_u128(w, a % b)
    }

    pub fn pow(&self, other: &Bits) -> VResult<Bits> {
        let w = self.width.max(other.width);
        if self.has_unknown() || other.has_unknown() {
            return Bits::all_x(w);
        }
        if Self::either_wide(self, other) {
            let (_, a, b) = self.wide_pair(other);
            let n = limbs_for(w);
            let base = a.to_u128();
            // Exponent must itself fit 128 bits to be meaningful here.
            if !b.data.iter().skip(2).all(|&x| x == 0) {
                return Bits::all_x(w);
            }
            let exp = b.to_u128();
            // New-ground semantic: exact modular arithmetic (mod 2^w) via
            // square-and-multiply; absurd exponents stay conservative all-x
            // (mirrors the small arm's checked_pow overflow policy).
            let r: Vec<u64> = match base {
                0 => {
                    if exp == 0 {
                        vec![1, 0]
                    } else {
                        vec![0; n]
                    }
                }
                1 => {
                    let mut one = vec![0u64; n];
                    one[0] = 1;
                    one
                }
                _ => {
                    if exp > (1 << 20) {
                        return Bits::all_x(w);
                    }
                    pow_mod2w(&a.data, exp, w)
                }
            };
            return pack(w, &r, &vec![0; n]);
        }
        let a = self.to_u128_lossy();
        let b = other.to_u128_lossy();
        let r = a
            .checked_pow(b.min(128) as u32)
            .map(|v| v & low_bits(w) as u128);
        match r {
            Some(v) => Bits::from_u128(w, v),
            None => Bits::all_x(w),
        }
    }

    /// Signed division (truncation toward zero); x/z or zero divisor → all-x.
    pub fn sdiv(&self, other: &Bits) -> VResult<Bits> {
        let w = self.width.max(other.width);
        if self.has_unknown() || other.has_unknown() {
            return Bits::all_x(w);
        }
        let (_, a, b) = self.wide_pair(other);
        if limbs_is_zero(&b.data) {
            return Bits::all_x(w);
        }
        let n = limbs_for(w);
        let sa = limbs_sign_bit(&a.data, w);
        let sb = limbs_sign_bit(&b.data, w);
        let mag_a = if sa { limbs_neg_width(&a.data, n, w) } else { a.data.clone() };
        let mag_b = if sb { limbs_neg_width(&b.data, n, w) } else { b.data.clone() };
        let (q, _) = limbs_divmod(&mag_a, &mag_b, w);
        let q = if sa != sb { limbs_neg_width(&q, n, w) } else { q };
        pack(w, &q, &vec![0; n])
    }

    /// Signed remainder (sign follows the dividend).
    pub fn srem(&self, other: &Bits) -> VResult<Bits> {
        let w = self.width.max(other.width);
        if self.has_unknown() || other.has_unknown() {
            return Bits::all_x(w);
        }
        let (_, a, b) = self.wide_pair(other);
        if limbs_is_zero(&b.data) {
            return Bits::all_x(w);
        }
        let n = limbs_for(w);
        let sa = limbs_sign_bit(&a.data, w);
        let sb = limbs_sign_bit(&b.data, w);
        let mag_a = if sa { limbs_neg_width(&a.data, n, w) } else { a.data.clone() };
        let mag_b = if sb { limbs_neg_width(&b.data, n, w) } else { b.data.clone() };
        let (_, r) = limbs_divmod(&mag_a, &mag_b, w);
        let r = if sa { limbs_neg_width(&r, n, w) } else { r };
        pack(w, &r, &vec![0; n])
    }

    /// Signed power (IEEE 1364-2005 §5.1.5 sign table, then exact modular
    /// magnitude for non-trivial exponents).
    pub fn spow(&self, other: &Bits) -> VResult<Bits> {
        let w = self.width.max(other.width);
        if self.has_unknown() || other.has_unknown() {
            return Bits::all_x(w);
        }
        let (_, a, b) = self.wide_pair(other);
        let n = limbs_for(w);
        let sa = limbs_sign_bit(&a.data, w);
        let sb = limbs_sign_bit(&b.data, w);
        let mag_a = if sa { limbs_neg_width(&a.data, n, w) } else { a.data.clone() };
        let mag_b = if sb { limbs_neg_width(&b.data, n, w) } else { b.data.clone() };
        // Exponent magnitude must fit 128 bits.
        if !mag_b.iter().skip(2).all(|&x| x == 0) {
            return Bits::all_x(w);
        }
        let exp = limbs_to_u128(&mag_b, &vec![0; n]);
        let base_is_one = mag_a.first() == Some(&1) && mag_a.iter().skip(1).all(|&x| x == 0);
        let base_is_zero = limbs_is_zero(&mag_a);
        if sb {
            // Negative exponent: only |base| ∈ {0,1} are defined (§5.1.5).
            return if base_is_one && !sa {
                Bits::from_u128(w, 1)
            } else if base_is_one && sa {
                // (-1)^negative = ±1 by parity.
                if exp % 2 == 0 {
                    Bits::from_u128(w, 1)
                } else {
                    Bits::zeros(w)?.sub(&Bits::from_u128(w.min(128), 1)?)
                }
            } else if base_is_zero {
                Bits::all_x(w)
            } else {
                Bits::zeros(w)
            };
        }
        if exp > (1 << 20) && !base_is_one && !base_is_zero {
            return Bits::all_x(w);
        }
        let mut acc = pow_mod2w(&mag_a, exp, w);
        if sa && exp % 2 == 1 {
            acc = limbs_neg_width(&acc, n, w);
        }
        pack(w, &acc, &vec![0; n])
    }

    fn sign_bit(&self) -> (bool, bool) {
        let w = self.width;
        if w == 0 {
            return (false, false);
        }
        match &self.repr {
            Repr::N { data, mask } => {
                let bit = 1u128 << (w - 1);
                (data & bit != 0, mask & bit != 0)
            }
            Repr::W { data, mask } => {
                let li = ((w - 1) / 64) as usize;
                let bit = 1u64 << ((w - 1) % 64);
                (
                    data.get(li).copied().unwrap_or(0) & bit != 0,
                    mask.get(li).copied().unwrap_or(0) & bit != 0,
                )
            }
        }
    }

    /// Unsigned comparison on known values (None if either side has unknown
    /// bits). Operands are compared at their common max width.
    pub fn cmp_unsigned(&self, other: &Bits) -> Option<std::cmp::Ordering> {
        if self.has_unknown() || other.has_unknown() {
            return None;
        }
        let w = self.width.max(other.width);
        let a = Limbs::padded(self, w);
        let b = Limbs::padded(other, w);
        Some(limbs_cmp(&a.data, &b.data))
    }

    /// Signed comparison on known values (None if either side has unknown
    /// bits). Fixes the pre-M8 wrap where 65..=128-bit compares truncated
    /// through i64.
    pub fn cmp_signed(&self, other: &Bits) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering;
        if self.has_unknown() || other.has_unknown() {
            return None;
        }
        let w = self.width.max(other.width);
        let a = Limbs::padded(self, w);
        let b = Limbs::padded(other, w);
        let (sa, sma) = limbs_sign_limb(&a, w);
        let (sb, smb) = limbs_sign_limb(&b, w);
        if sma || smb {
            return None; // unknown sign → unknown order
        }
        match (sa, sb) {
            (true, false) => Some(Ordering::Less),
            (false, true) => Some(Ordering::Greater),
            // Same sign: the two's-complement bit patterns order exactly as
            // the values do (e2e 12_signed_ops caught a wrong both-negative
            // reversal here: -6 < -1 holds pattern-wise too).
            _ => Some(limbs_cmp(&a.data, &b.data)),
        }
    }

    /// Equality: any known-bit mismatch → 0; else any unknown → x; else 1.
    /// `===` uses [`Bits::case_eq`] instead (IEEE 1364-2005 §5.1.6/§5.1.7).
    pub fn logic_eq(&self, other: &Bits) -> VResult<Bits> {
        let w = self.width.max(other.width);
        let a = self.uadjust(w)?;
        let b = other.uadjust(w)?;
        if let (&Repr::N { data: ad, mask: am }, &Repr::N { data: bd, mask: bm }) = (&a.repr, &b.repr) {
            let known_diff = (ad ^ bd) & !(am | bm);
            if known_diff != 0 {
                return Bits::from_u64(1, 0);
            }
            if am != 0 || bm != 0 {
                return Bits::all_x(1);
            }
            return Bits::from_u64(1, 1);
        }
        let la = Limbs::of(&a);
        let lb = Limbs::of(&b);
        for i in 0..la.data.len().max(lb.data.len()) {
            let x = la.data.get(i).copied().unwrap_or(0);
            let xm = la.mask.get(i).copied().unwrap_or(0);
            let y = lb.data.get(i).copied().unwrap_or(0);
            let ym = lb.mask.get(i).copied().unwrap_or(0);
            if (x ^ y) & !(xm | ym) != 0 {
                return Bits::from_u64(1, 0);
            }
        }
        if la.mask.iter().any(|&m| m != 0) || lb.mask.iter().any(|&m| m != 0) {
            return Bits::all_x(1);
        }
        Bits::from_u64(1, 1)
    }

    pub fn logic_ne(&self, other: &Bits) -> VResult<Bits> {
        Ok(self.logic_eq(other)?.bit_not())
    }

    /// `===`: bit-exact including x/z identity.
    pub fn case_eq(&self, other: &Bits) -> VResult<Bits> {
        if let (&Repr::N { data: ad, mask: am }, &Repr::N { data: bd, mask: bm }) = (&self.repr, &other.repr) {
            return Bits::from_u64(1, (ad == bd && am == bm) as u64);
        }
        let w = self.width.max(other.width);
        let a = Limbs::padded(self, w);
        let b = Limbs::padded(other, w);
        Bits::from_u64(1, (a.data == b.data && a.mask == b.mask) as u64)
    }

    pub fn case_ne(&self, other: &Bits) -> VResult<Bits> {
        Ok(self.case_eq(other)?.bit_not())
    }

    /// `casez` equality: x AND z in the label are don't-care (§9.5.2).
    pub fn casez_eq(&self, label: &Bits) -> VResult<bool> {
        let w = self.width.max(label.width);
        let a = self.uadjust(w)?;
        let b = label.uadjust(w)?;
        if let (&Repr::N { data: ad, mask: am }, &Repr::N { data: bd, mask: bm }) = (&a.repr, &b.repr) {
            let care = !bm & low_bits(w);
            return Ok(((ad ^ bd) & care) == 0 && ((am ^ bm) & care) == 0);
        }
        let la = Limbs::of(&a);
        let lb = Limbs::of(&b);
        for i in 0..la.data.len().max(lb.data.len()) {
            let x = la.data.get(i).copied().unwrap_or(0);
            let xm = la.mask.get(i).copied().unwrap_or(0);
            let y = lb.data.get(i).copied().unwrap_or(0);
            let ym = lb.mask.get(i).copied().unwrap_or(0);
            let care = !ym;
            if ((x ^ y) & care != 0) || ((xm ^ ym) & care != 0) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Case-statement wildcard matching, wildcarding BOTH operands
    /// (IEEE 1364-2005 §9.5.2/§9.5.3). casez: z in either side is
    /// don't-care, x stays literal (matches only x/z on the other side);
    /// casex: x AND z in either side are don't-care. Bit-exact match →
    /// true; a non-match is false (case items simply do not fire — this
    /// never yields x). Complements [`Bits::casez_eq`], which wildcards
    /// the label side only.
    pub fn casez_match(&self, other: &Bits) -> bool {
        Self::case_match_impl(self, other, false)
    }

    pub fn casex_match(&self, other: &Bits) -> bool {
        Self::case_match_impl(self, other, true)
    }

    fn case_match_impl(a: &Bits, b: &Bits, wildx: bool) -> bool {
        let w = a.width.max(b.width);
        let aa = a.uadjust(w).unwrap_or_else(|_| Bits::all_x(w).unwrap());
        let bb = b.uadjust(w).unwrap_or_else(|_| Bits::all_x(w).unwrap());
        // 4-state layout: x = (data 0, mask 1), z = (data 1, mask 1).
        let one = |d: bool, m: bool| -> bool {
            match wildx {
                // casex: any masked bit (x or z) on either side is a wildcard.
                true => m,
                // casez: only z (data under mask) is a wildcard.
                false => m && d,
            }
        };
        match (&aa.repr, &bb.repr) {
            (
                Repr::N { data: ad, mask: am },
                Repr::N { data: bd, mask: bm },
            ) => {
                let all = low_bits(w);
                let mut matched = 0u128;
                for i in 0..w {
                    let bit = 1u128 << i;
                    let zi = (*am & bit) != 0;
                    let zj = (*bm & bit) != 0;
                    let di = (*ad & bit) != 0;
                    let dj = (*bd & bit) != 0;
                    if one(zi && di, zi) || one(zj && dj, zj) {
                        matched |= bit; // wildcard bit
                    } else if di == dj && zi == zj {
                        matched |= bit; // literal equality (incl. x==x, z==z)
                    }
                }
                matched == all
            }
            _ => {
                let la = Limbs::padded(&aa, w);
                let lb = Limbs::padded(&bb, w);
                let rem = w % 64;
                let top_keep = if rem == 0 { u64::MAX } else { (1u64 << rem) - 1 };
                for i in 0..limbs_for(w) {
                    let keep = if i == limbs_for(w) - 1 { top_keep } else { u64::MAX };
                    let mut matched = 0u64;
                    for bit in 0..64u32 {
                        let bmask = 1u64 << bit;
                        if bmask & keep == 0 {
                            break;
                        }
                        let zi = la.mask[i] & bmask != 0;
                        let zj = lb.mask[i] & bmask != 0;
                        let di = la.data[i] & bmask != 0;
                        let dj = lb.data[i] & bmask != 0;
                        if one(zi && di, zi) || one(zj && dj, zj)
                            || (di == dj && zi == zj)
                        {
                            matched |= bmask;
                        }
                    }
                    if matched & keep != keep {
                        return false;
                    }
                }
                true
            }
        }
    }

    /// `casex` equality: x AND z on either side are don't-care (§9.5.3).
    pub fn casex_eq(&self, label: &Bits) -> VResult<bool> {
        let w = self.width.max(label.width);
        let a = self.uadjust(w)?;
        let b = label.uadjust(w)?;
        if let (&Repr::N { data: ad, mask: am }, &Repr::N { data: bd, mask: bm }) = (&a.repr, &b.repr) {
            let care = !(am | bm) & low_bits(w);
            return Ok(((ad ^ bd) & care) == 0);
        }
        let la = Limbs::of(&a);
        let lb = Limbs::of(&b);
        for i in 0..la.data.len().max(lb.data.len()) {
            let x = la.data.get(i).copied().unwrap_or(0);
            let xm = la.mask.get(i).copied().unwrap_or(0);
            let y = lb.data.get(i).copied().unwrap_or(0);
            let ym = lb.mask.get(i).copied().unwrap_or(0);
            let care = !(xm | ym);
            if (x ^ y) & care != 0 {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Unsigned less-than (both must be known; caller handles signedness).
    pub fn lt(&self, other: &Bits) -> VResult<Bits> {
        match self.cmp_unsigned(other) {
            None => Bits::all_x(1),
            Some(o) => Bits::from_u64(1, (o == std::cmp::Ordering::Less) as u64),
        }
    }

    pub fn shl(&self, amount: &Bits) -> VResult<Bits> {
        if amount.has_unknown() {
            return Bits::all_x(self.width);
        }
        let s = amount.to_u128_lossy();
        if s >= self.width as u128 {
            return Bits::zeros(self.width);
        }
        let s = s as u32;
        match &self.repr {
            Repr::N { data, mask } => Ok(Bits {
                width: self.width,
                repr: Repr::N {
                    data: (data << s) & low_bits(self.width),
                    mask: (mask << s) & low_bits(self.width),
                },
            }),
            Repr::W { data, mask } => {
                let n = limbs_for(self.width);
                let (li, off) = ((s / 64) as usize, s % 64);
                let mut d = vec![0u64; n];
                let mut m = vec![0u64; n];
                for k in li..n {
                    let src = k - li;
                    let mut v = data[src] << off;
                    let mut vm = mask[src] << off;
                    if off > 0 && src >= 1 {
                        v |= data[src - 1] >> (64 - off);
                        vm |= mask[src - 1] >> (64 - off);
                    }
                    d[k] = v;
                    m[k] = vm;
                }
                let rem = self.width % 64;
                if rem != 0 {
                    d[n - 1] &= (1u64 << rem) - 1;
                    m[n - 1] &= (1u64 << rem) - 1;
                }
                Ok(Bits {
                    width: self.width,
                    repr: Repr::W {
                        data: d.into_boxed_slice(),
                        mask: m.into_boxed_slice(),
                    },
                })
            }
        }
    }

    pub fn shr(&self, amount: &Bits) -> VResult<Bits> {
        if amount.has_unknown() {
            return Bits::all_x(self.width);
        }
        let s = amount.to_u128_lossy();
        if s >= self.width as u128 {
            return Bits::zeros(self.width);
        }
        let s = s as u32;
        match &self.repr {
            Repr::N { data, mask } => Ok(Bits {
                width: self.width,
                repr: Repr::N {
                    data: data >> s,
                    mask: mask >> s,
                },
            }),
            Repr::W { data, mask } => {
                let n = limbs_for(self.width);
                let (li, off) = ((s / 64) as usize, s % 64);
                let mut d = vec![0u64; n];
                let mut m = vec![0u64; n];
                for k in 0..n {
                    let src = k + li;
                    let mut v = data.get(src).copied().unwrap_or(0) >> off;
                    let mut vm = mask.get(src).copied().unwrap_or(0) >> off;
                    if off > 0 {
                        if let Some(&hi) = data.get(src + 1) {
                            v |= hi << (64 - off);
                        }
                        if let Some(&him) = mask.get(src + 1) {
                            vm |= him << (64 - off);
                        }
                    }
                    d[k] = v;
                    m[k] = vm;
                }
                Ok(Bits {
                    width: self.width,
                    repr: Repr::W {
                        data: d.into_boxed_slice(),
                        mask: m.into_boxed_slice(),
                    },
                })
            }
        }
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
        let s = amount.to_u128_lossy();
        if s >= self.width as u128 {
            // Entirely sign bits.
            let (sign, sign_unknown) = self.sign_bit();
            return if sign_unknown {
                Bits::all_x(self.width)
            } else if sign {
                self.all_ones()
            } else {
                Bits::zeros(self.width)
            };
        }
        let s = s as u32;
        let w = self.width;
        match &self.repr {
            Repr::N { data, mask } => {
                // The fill<<w trick below needs bits [w, w+s) representable in
                // u128; beyond that the fill silently drops (pre-M8 inherited
                // bug, caught by the N-vs-W property: 127-bit -1 >>2 lost the
                // top fill bit). Route through the limb path, which has room.
                if w + s > 128 {
                    let ext = self.sadjust(w + s)?;
                    return ext.ashr(amount, true)?.trunc(w);
                }
                let (sign, sign_unknown) = self.sign_bit();
                let fill: u128 = if !sign_unknown && sign { !0u128 } else { 0 };
                let fillm: u128 = if sign_unknown { !0u128 } else { 0 };
                let (ext, extm) = if w >= 128 {
                    (data >> s, mask >> s)
                } else {
                    ((data | (fill << w)) >> s, (mask | (fillm << w)) >> s)
                };
                Ok(Bits {
                    width: w,
                    repr: Repr::N {
                        data: ext & low_bits(w),
                        mask: extm & low_bits(w),
                    },
                })
            }
            Repr::W { data, mask } => {
                let (sign, sign_unknown) = self.sign_bit();
                let fill: u64 = if !sign_unknown && sign { u64::MAX } else { 0 };
                let fillm: u64 = if sign_unknown { u64::MAX } else { 0 };
                let start = w;
                let end = w + s;
                let en = limbs_for(end);
                let mut d = vec![0u64; en];
                let mut m = vec![0u64; en];
                for (i, &v) in data.iter().enumerate().take(en) {
                    d[i] = v;
                }
                for (i, &v) in mask.iter().enumerate().take(en) {
                    m[i] = v;
                }
                let base = (start / 64) as usize;
                let off = start % 64;
                if off > 0 && base < en {
                    d[base] |= fill << off;
                    m[base] |= fillm << off;
                }
                for j in (base + 1)..en {
                    d[j] = fill;
                    m[j] = fillm;
                }
                // shr by s on the extended limbs, then truncate to width.
                let (li, off2) = ((s / 64) as usize, s % 64);
                let n = limbs_for(w);
                let mut od = vec![0u64; n];
                let mut om = vec![0u64; n];
                for k in 0..n {
                    let src = k + li;
                    let mut v = d.get(src).copied().unwrap_or(0) >> off2;
                    let mut vm = m.get(src).copied().unwrap_or(0) >> off2;
                    if off2 > 0 {
                        if let Some(&hi) = d.get(src + 1) {
                            v |= hi << (64 - off2);
                        }
                        if let Some(&him) = m.get(src + 1) {
                            vm |= him << (64 - off2);
                        }
                    }
                    od[k] = v;
                    om[k] = vm;
                }
                let rem = w % 64;
                if rem != 0 {
                    od[n - 1] &= (1u64 << rem) - 1;
                    om[n - 1] &= (1u64 << rem) - 1;
                }
                Ok(Bits {
                    width: w,
                    repr: Repr::W {
                        data: od.into_boxed_slice(),
                        mask: om.into_boxed_slice(),
                    },
                })
            }
        }
    }

    /// All-ones known value at this value's width.
    fn all_ones(&self) -> VResult<Bits> {
        let w = self.width;
        if w <= 128 {
            Bits::from_u128(w, low_bits(w) as u128)
        } else {
            let n = limbs_for(w);
            let full = full_mask_vec(w);
            pack(w, &full, &vec![0; n])
        }
    }

    /// Reduction AND.
    pub fn reduce_and(&self) -> VResult<Bits> {
        let w = self.width;
        match &self.repr {
            Repr::N { data, mask } => {
                let zeros = !data & !mask;
                if zeros & low_bits(w) != 0 {
                    Bits::from_u64(1, 0)
                } else if mask & low_bits(w) != 0 {
                    Bits::all_x(1)
                } else {
                    Bits::from_u64(1, 1)
                }
            }
            Repr::W { data, mask } => {
                // Bits above the width are zero padding, not real zeros —
                // the CARE mask must gate the complement expressions too
                // (masking the inputs is undone by `!d`).
                let care = full_mask_vec(self.width);
                let mut any_zero = false;
                let mut any_unknown = false;
                for i in 0..data.len() {
                    if (!data[i] & !mask[i]) & care[i] != 0 {
                        any_zero = true;
                    }
                    if mask[i] & care[i] != 0 {
                        any_unknown = true;
                    }
                }
                if any_zero {
                    Bits::from_u64(1, 0)
                } else if any_unknown {
                    Bits::all_x(1)
                } else {
                    Bits::from_u64(1, 1)
                }
            }
        }
    }

    pub fn reduce_or(&self) -> VResult<Bits> {
        let w = self.width;
        match &self.repr {
            Repr::N { data, mask } => {
                let ones = data & !mask;
                if ones & low_bits(w) != 0 {
                    Bits::from_u64(1, 1)
                } else if mask & low_bits(w) != 0 {
                    Bits::all_x(1)
                } else {
                    Bits::from_u64(1, 0)
                }
            }
            Repr::W { data, mask } => {
                let care = full_mask_vec(self.width);
                let mut any_one = false;
                let mut any_unknown = false;
                for i in 0..data.len() {
                    if (data[i] & !mask[i]) & care[i] != 0 {
                        any_one = true;
                    }
                    if mask[i] & care[i] != 0 {
                        any_unknown = true;
                    }
                }
                if any_one {
                    Bits::from_u64(1, 1)
                } else if any_unknown {
                    Bits::all_x(1)
                } else {
                    Bits::from_u64(1, 0)
                }
            }
        }
    }

    pub fn reduce_xor(&self) -> VResult<Bits> {
        if self.has_unknown() {
            return Bits::all_x(1);
        }
        let v = match &self.repr {
            Repr::N { data, .. } => data.count_ones() & 1,
            Repr::W { data, .. } => data.iter().map(|x| x.count_ones()).sum::<u32>() & 1,
        };
        Bits::from_u64(1, v as u64)
    }

    /// Truthiness for `if`/`?:`/logical ops: known → 0/1; any unknown bit that
    /// could matter → x (conservative: any unknown → x).
    pub fn truthy(&self) -> VResult<Option<bool>> {
        if self.has_unknown() {
            return Ok(None);
        }
        Ok(Some(match &self.repr {
            Repr::N { data, .. } => *data != 0,
            Repr::W { data, .. } => data.iter().any(|&x| x != 0),
        }))
    }

    /// Ternary merge for `c ? t : e` with unknown cond (IEEE 1364-2005
    /// Table 5-21-ish semantics): bits where both sides agree keep that value,
    /// differing or unknown bits become x.
    pub fn merge(&self, other: &Bits) -> VResult<Bits> {
        let w = self.width.max(other.width);
        chk(w)?;
        let a = self.uadjust(w)?;
        let b = other.uadjust(w)?;
        if let (&Repr::N { data: ad, mask: am }, &Repr::N { data: bd, mask: bm }) = (&a.repr, &b.repr) {
            let ones = ad & !am & bd & !bm;
            let zeros = !ad & !am & (!bd & !bm);
            return Ok(Bits {
                width: w,
                repr: Repr::N {
                    data: ones,
                    mask: low_bits(w) & !(ones | zeros),
                },
            });
        }
        let la = Limbs::of(&a);
        let lb = Limbs::of(&b);
        let n = limbs_for(w);
        let full = full_mask_vec(w);
        let mut ones = vec![0u64; n];
        let mut mask = vec![0u64; n];
        for i in 0..n {
            ones[i] = !la.mask[i] & la.data[i] & !lb.mask[i] & lb.data[i];
            mask[i] = full[i]
                & !(ones[i] | (!la.data[i] & !la.mask[i] & (!lb.data[i] & !lb.mask[i])));
        }
        pack(w, &ones, &mask)
    }

    /// Extract `[hi:lo]` (hi ≥ lo; both inclusive, direction-agnostic width).
    pub fn part(&self, hi: u32, lo: u32) -> VResult<Bits> {
        let (hi, lo) = if hi >= lo { (hi, lo) } else { (lo, hi) };
        let w = hi - lo + 1;
        chk(w)?;
        match &self.repr {
            Repr::N { data, mask } => Ok(Bits {
                width: w,
                repr: Repr::N {
                    data: data >> lo & low_bits(w),
                    mask: mask >> lo & low_bits(w),
                },
            }),
            Repr::W { data, mask } => {
                if w <= 128 {
                    // Boundary-safe extraction into a u128.
                    let mut d = 0u128;
                    let mut m = 0u128;
                    for k in (0..w).rev() {
                        let idx = (lo + k) as usize;
                        let li = idx / 64;
                        let bit = 1u64 << (idx % 64);
                        let db = data.get(li).copied().unwrap_or(0) & bit != 0;
                        let mb = mask.get(li).copied().unwrap_or(0) & bit != 0;
                        d = (d << 1) | db as u128;
                        m = (m << 1) | mb as u128;
                    }
                    Ok(Bits {
                        width: w,
                        repr: Repr::N { data: d, mask: m },
                    })
                } else {
                    let n = limbs_for(w);
                    let mut od = vec![0u64; n];
                    let mut om = vec![0u64; n];
                    for k in 0..n {
                        let src_bit = lo + (k as u32) * 64;
                        let li = (src_bit / 64) as usize;
                        let off = src_bit % 64;
                        let mut v = data.get(li).copied().unwrap_or(0) >> off;
                        let mut vm = mask.get(li).copied().unwrap_or(0) >> off;
                        if off > 0 {
                            if let Some(&hiw) = data.get(li + 1) {
                                v |= hiw << (64 - off);
                            }
                            if let Some(&hiwm) = mask.get(li + 1) {
                                vm |= hiwm << (64 - off);
                            }
                        }
                        od[k] = v;
                        om[k] = vm;
                    }
                    let rem = w % 64;
                    if rem != 0 {
                        od[n - 1] &= (1u64 << rem) - 1;
                        om[n - 1] &= (1u64 << rem) - 1;
                    }
                    Ok(Bits {
                        width: w,
                        repr: Repr::W {
                            data: od.into_boxed_slice(),
                            mask: om.into_boxed_slice(),
                        },
                    })
                }
            }
        }
    }

    /// Replace `[hi:lo]` with `v` (v truncated/extended to the field width).
    pub fn set_part(&self, hi: u32, lo: u32, v: &Bits) -> VResult<Bits> {
        let (hi, lo) = if hi >= lo { (hi, lo) } else { (lo, hi) };
        let w = hi - lo + 1;
        if let Repr::N { data, mask } = &self.repr {
            if self.width <= 128 {
                let field = v.uadjust(w)?;
                let (fd, fm) = match &field.repr {
                    Repr::N { data: fd, mask: fm } => (*fd, *fm),
                    _ => unreachable!("narrow field stays small"),
                };
                let clear = !(low_bits(w) << lo);
                return Ok(Bits {
                    width: self.width,
                    repr: Repr::N {
                        data: (data & clear) | (fd << lo),
                        mask: (mask & clear) | (fm << lo),
                    },
                });
            }
        }
        let sw = self.width;
        let mut a = Limbs::of(self);
        a.data.resize(limbs_for(sw), 0);
        a.mask.resize(limbs_for(sw), 0);
        let field = v.uadjust(w)?;
        let f = Limbs::of(&field);
        // Clear the field region then insert.
        for k in 0..w {
            let idx = (lo + k) as usize;
            let li = idx / 64;
            let bit = 1u64 << (idx % 64);
            if li < a.data.len() {
                a.data[li] &= !bit;
                a.mask[li] &= !bit;
            }
        }
        a.insert(&f, lo);
        let (d, m) = trunc_limbs(&a.data, &a.mask, sw);
        Ok(Bits {
            width: sw,
            repr: Repr::W { data: d, mask: m },
        })
    }

    /// Concatenate: `parts` are given MSB-first (Verilog order).
    pub fn concat(parts: &[Bits]) -> VResult<Bits> {
        let mut total: u64 = 0;
        for p in parts {
            total += p.width as u64;
            if total > MAX_WIDTH as u64 {
                return Err(ValueError(format!(
                    "concatenation width {total} exceeds the {MAX_WIDTH}-bit limit"
                )));
            }
        }
        let total = total as u32;
        if parts.iter().all(|p| matches!(p.repr, Repr::N { .. })) && total <= 128 {
            let mut data = 0u128;
            let mut mask = 0u128;
            for p in parts {
                if let Repr::N { data: pd, mask: pm } = &p.repr {
                    data = (data << p.width) | (pd & low_bits(p.width));
                    mask = (mask << p.width) | (pm & low_bits(p.width));
                }
            }
            return Ok(Bits {
                width: total,
                repr: Repr::N { data, mask },
            });
        }
        chk(total)?;
        let mut acc = Limbs::zeros(total);
        let mut sh: u32 = total; // parts are MSB-first: fill from the top
        for p in parts {
            sh -= p.width;
            let l = Limbs::of(p);
            acc.insert(&l, sh);
        }
        acc.pack(total)
    }

    pub fn replicate(&self, times: u64) -> VResult<Bits> {
        let total = self
            .width
            .checked_mul(times.min(u64::from(u32::MAX)) as u32)
            .ok_or_else(|| ValueError("replication width overflow".into()))?;
        chk(total)?;
        if times == 0 {
            return Bits::zeros(0);
        }
        if matches!(self.repr, Repr::N { .. }) && total <= 128 {
            let (w, data, mask) = match &self.repr {
                Repr::N { data, mask } => (self.width, *data, *mask),
                _ => unreachable!(),
            };
            let mut d = 0u128;
            let mut m = 0u128;
            for _ in 0..times {
                d = (d << w) | (data & low_bits(w));
                m = (m << w) | (mask & low_bits(w));
            }
            return Ok(Bits {
                width: total,
                repr: Repr::N { data: d, mask: m },
            });
        }
        let mut acc = Limbs::zeros(total);
        let mut sh: u32 = 0;
        let l = Limbs::of(self);
        for _ in 0..times {
            acc.insert(&l, sh);
            sh += self.width;
        }
        acc.pack(total)
    }

    fn bit_pair(&self, i: u32) -> (bool, bool) {
        match &self.repr {
            Repr::N { data, mask } => {
                let bit = 1u128 << i;
                (data & bit != 0, mask & bit != 0)
            }
            Repr::W { data, mask } => {
                let li = (i / 64) as usize;
                let bit = 1u64 << (i % 64);
                (
                    data.get(li).copied().unwrap_or(0) & bit != 0,
                    mask.get(li).copied().unwrap_or(0) & bit != 0,
                )
            }
        }
    }

    /// `%b` rendering: `0`/`1`/`x`/`z` per bit, full width.
    pub fn fmt_bin(&self) -> String {
        let mut s = String::with_capacity(self.width as usize);
        for i in (0..self.width).rev() {
            let (d, m) = self.bit_pair(i);
            if m {
                s.push(if d { 'z' } else { 'x' });
            } else {
                s.push(if d { '1' } else { '0' });
            }
        }
        s
    }

    /// `%h`/`%x` rendering (full width, one char per 4 bits).
    pub fn fmt_hex(&self) -> String {
        let nibbles = self.width.div_ceil(4);
        let mut s = String::with_capacity(nibbles as usize);
        for i in (0..nibbles).rev() {
            let sh = i * 4;
            let (mut d, mut m) = (0u8, 0u8);
            for k in 0..4 {
                if sh + k >= self.width {
                    continue;
                }
                let (db, mb) = self.bit_pair(sh + k);
                d |= (db as u8) << k;
                m |= (mb as u8) << k;
            }
            s.push(match (m, d) {
                (0, v) => char::from_digit(v as u32, 16).unwrap_or('?'),
                (_, 0) => 'x',
                _ => 'z',
            });
        }
        s
    }

    /// `%o` rendering (full width, one char per 3 bits; unknown → "x").
    pub fn fmt_oct(&self) -> String {
        if self.has_unknown() {
            return "x".to_string();
        }
        let groups = self.width.div_ceil(3);
        let mut s = String::with_capacity(groups as usize);
        for i in (0..groups).rev() {
            let sh = i * 3;
            let mut d = 0u32;
            for k in 0..3 {
                if sh + k >= self.width {
                    continue;
                }
                let (db, _) = self.bit_pair(sh + k);
                d |= (db as u32) << k;
            }
            s.push(char::from_digit(d, 8).unwrap_or('?'));
        }
        s
    }

    /// Decimal digit string of the known-bit unsigned magnitude (any width).
    pub fn fmt_dec(&self) -> String {
        if self.has_unknown() {
            // IEEE: if any bit x/z → x (shortest form prints "x")
            return "x".to_string();
        }
        match &self.repr {
            Repr::N { .. } => self.to_u128_lossy().to_string(),
            Repr::W { data, .. } => {
                let mut v = data.to_vec();
                while v.last() == Some(&0) {
                    v.pop();
                }
                if v.is_empty() {
                    return "0".to_string();
                }
                const CHUNK: u64 = 10_000_000_000_000_000_000; // 10^19
                let mut groups: Vec<u64> = Vec::new();
                while !limbs_is_zero(&v) {
                    let (q, r) = limbs_divmod_small(&v, CHUNK);
                    groups.push(r);
                    v = q;
                    while v.last() == Some(&0) {
                        v.pop();
                    }
                }
                let mut s = String::new();
                for (i, g) in groups.iter().enumerate().rev() {
                    if i == groups.len() - 1 {
                        s.push_str(&g.to_string());
                    } else {
                        s.push_str(&format!("{g:019}"));
                    }
                }
                s
            }
        }
    }

    /// Signed decimal rendering of the known bits (`%d` with a signed
    /// expression); wide values must use this — `to_i128_lossy` only sees the
    /// low 128 bits.
    pub fn fmt_dec_signed(&self) -> String {
        if self.has_unknown() {
            return "x".to_string();
        }
        if self.width <= 128 {
            return self.to_i128_lossy().to_string();
        }
        let (sign, _) = self.sign_bit();
        if sign {
            let one = Bits::from_u128(128, 1).unwrap();
            let neg = self.bit_not().add(&one).unwrap();
            format!("-{}", neg.fmt_dec())
        } else {
            self.fmt_dec()
        }
    }

    /// Digits of the maximum representable value at this width (unsigned:
    /// 2^w−1; signed: ±(2^(w−1))) plus the sign slot — the `%d` natural pad.
    pub fn dec_natural_len(&self, signed: bool) -> usize {
        if signed {
            if self.width <= 128 {
                let maxv = 1u128 << (self.width.min(127) - 1);
                maxv.to_string().len() + 1
            } else {
                // 2^(w−1) decimal digit count, computed exactly via a
                // power-of-two bignum (cheap: shift-only).
                digits_of_pow2(self.width - 1) + 1
            }
        } else if self.width <= 128 {
            low_bits(self.width).to_string().len()
        } else {
            // 2^w − 1 has the same digit count as 2^w.
            digits_of_pow2(self.width)
        }
    }

    /// `$clog2` on known values (limb-aware; `v ≤ 1 → 0`). Fixes the
    /// pre-M8 u64 truncation for 65..=128-bit arguments.
    pub fn clog2(&self) -> VResult<u64> {
        if self.has_unknown() {
            return Err(ValueError("clog2 of unknown value".into()));
        }
        if self.width <= 128 {
            let v = self.to_u128_lossy();
            return Ok(if v <= 1 {
                0
            } else {
                128 - (v - 1).leading_zeros() as u64
            });
        }
        // Wide: clog2 = (highest set bit of v−1) + 1; v == 0 → 0.
        let data = match &self.repr {
            Repr::W { data, .. } => data,
            _ => unreachable!(),
        };
        if data.iter().all(|&x| x == 0) {
            return Ok(0);
        }
        let mut dec = data.to_vec();
        let mut borrow = 1u64;
        for limb in dec.iter_mut() {
            let (r, b) = limb.overflowing_sub(borrow);
            *limb = r;
            borrow = b as u64;
        }
        let mut high = None;
        for (i, &x) in dec.iter().enumerate().rev() {
            if x != 0 {
                high = Some(i as u64 * 64 + (63 - x.leading_zeros() as u64));
                break;
            }
        }
        Ok(match high {
            None => 0,
            Some(h) => h + 1,
        })
    }
}

/// Exact decimal digit count of 2^exp for exp > 64 (cheap bignum shift).
fn digits_of_pow2(exp: u32) -> usize {
    let mut v: Vec<u64> = vec![1]; // 2^0
    let mut e = exp;
    while e > 0 {
        let step = e.min(63);
        let mut carry = 0u64;
        for limb in v.iter_mut() {
            let nc = *limb >> (64 - step);
            *limb = (*limb << step) | carry;
            carry = nc;
        }
        if carry > 0 {
            v.push(carry);
        }
        e -= step;
    }
    let b = Bits {
        width: exp + 1,
        repr: Repr::W {
            data: v.into_boxed_slice(),
            mask: vec![0; limbs_for(exp + 1)].into_boxed_slice(),
        },
    };
    b.fmt_dec().len()
}

/// Based-literal number bases (mirror of `vl_syntax::ast::Base`; this crate
/// stays zero-dependency, so the AST type cannot be referenced here).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    Dec,
    Bin,
    Oct,
    Hex,
}

/// Parse based-literal parts into Bits (width from literal, sized
/// truncation keeping low bits, x/z/? digits → unknown bits, unsized →
/// ≥32). Decoupled from `vl_syntax::ast::NumLit` (it embeds a
/// `rcs_diag::Span`): the vl-sim shim adapts `NumLit` onto this entry
/// point. `signed` is carried for call-site fidelity — literal signedness
/// is a type attribute and does not change the stored bits (the
/// pre-extraction `from_numlit` never read it).
pub fn from_numlit_parts(
    width: Option<u32>,
    signed: bool,
    base: Base,
    digits: &str,
) -> VResult<Bits> {
    let _ = signed; // intentionally unused — see the doc comment above
    let raw = digits;
    // Lexer-normalized text has no underscores, but accept them anyway so
    // direct parses (tests, LSP-style tooling) behave identically.
    let digits: String = digits.chars().filter(|&c| c != '_').collect();
    let digits = digits.as_str();
    // Decimal: x/z allowed only as the whole value (IEEE 1364-2005 §2.5.1).
    if base == Base::Dec {
        let w = width.unwrap_or(32);
        chk(w)?;
        if digits.chars().all(|c| matches!(c, 'x' | 'X' | 'z' | 'Z' | '?')) {
            let mut b = Bits::all_x(w)?;
            if matches!(digits.chars().next(), Some('z' | 'Z' | '?')) {
                if let Repr::W { data, mask } = &mut b.repr {
                    *data = mask.clone(); // z
                }
            }
            return Ok(b);
        }
        if digits.chars().any(|c| matches!(c, 'x' | 'X' | 'z' | 'Z' | '?')) {
            return Err(ValueError(
                "x/z digits are not allowed inside a decimal literal".into(),
            ));
        }
        // Limb-folded decimal parse (61+ digit literals are legal upstream).
        let mut v: Vec<u64> = vec![0; digits.len() / 19 + 2];
        for c in digits.chars() {
            let d = c.to_digit(10).ok_or_else(|| {
                ValueError(format!("decimal literal `{digits}` has a bad digit"))
            })? as u64;
            limbs_mul_add_small(&mut v, 10, d);
        }
        return pack(w, &v, &vec![0; v.len()]);
    }
    let per: u32 = match base {
        Base::Bin => 1,
        Base::Oct => 3,
        Base::Hex => 4,
        Base::Dec => unreachable!(),
    };
    let nat_width = digits.len() as u32 * per;
    let full = width.unwrap_or_else(|| nat_width.max(32));
    chk(full)?;
    // Unsized literal made only of x/z/? digits: the unknown fills the whole
    // (≥32-bit) value (`'hx` → 32 x's), per §2.5.1 and the oracle.
    if width.is_none()
        && digits.chars().all(|c| matches!(c, 'x' | 'X' | 'z' | 'Z' | '?'))
    {
        let w = full.min(MAX_WIDTH);
        let mut b = Bits::all_x(w)?;
        if matches!(digits.chars().next(), Some('z' | 'Z' | '?')) {
            match &mut b.repr {
                Repr::N { data, mask } => *data = *mask,
                Repr::W { data, mask } => *data = mask.clone(),
            }
        }
        return Ok(b);
    }
    // Accumulate every digit into limb vectors (no early drop — the declared
    // overflow check needs the full value).
    let cap = limbs_for(full).max(digits.len() as usize * per as usize / 64 + 2);
    let mut data = vec![0u64; cap];
    let mut mask = vec![0u64; cap];
    for c in digits.chars() {
        let nib: (u64, u64) = match c.to_digit(16) {
            Some(v) if (v as u32) < (1 << per) => (v as u64, 0),
            _ => match c {
                'x' | 'X' => (0, low_bits(per) as u64),
                'z' | 'Z' | '?' => (low_bits(per) as u64, low_bits(per) as u64),
                _ => {
                    return Err(ValueError(format!(
                        "digit `{c}` invalid for base of literal"
                    )))
                }
            },
        };
        shl_bits_in_place(&mut data, per);
        shl_bits_in_place(&mut mask, per);
        data[0] |= nib.0;
        mask[0] |= nib.1;
    }
    let w = full.min(MAX_WIDTH);
    // Sized-literal "too many digits": any KNOWN nonzero bit above the
    // declared width is an error (x/z digits above declared are don't-care).
    if let Some(declared) = width {
        if known_nonzero_above(&data, &mask, declared) {
            return Err(ValueError(format!(
                "too many digits for {declared}-bit number: `{}`",
                raw
            )));
        }
    }
    let (d, m) = trunc_limbs(&data, &mask, w);
    pack(w, &d, &m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(v: u64, w: u32) -> Bits {
        Bits::from_u64(w, v).unwrap()
    }

    // ---------- M8 multi-limb (ADR-0008) ----------
    fn wide(width: u32, limbs: &[u64]) -> Bits {
        assert!(width > 128);
        pack(width, limbs, &vec![0; limbs_for(width)]).unwrap()
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
    fn wide_arithmetic() {
        // (2^192 − 1) + 1 at width 193 vs 192 (carry across 3 limbs).
        let m192 = wide(192, &[u64::MAX, u64::MAX, u64::MAX]);
        let one = Bits::from_u128(193, 1).unwrap();
        let sum = m192.add(&one).unwrap();
        assert_eq!(sum.width, 193);
        assert_eq!(sum.part(192, 192).unwrap().to_u64_lossy(), 1);
        let wrapped = m192.add(&Bits::from_u128(192, 1).unwrap()).unwrap();
        assert_eq!(wrapped.to_u128_lossy(), 0);
        assert!(wrapped.is_wide());
        // mul result width = max operand width (context-determined contract,
        // same as the small arm): (2^128+1)^2 mod 2^129 = 1.
        let a = wide(129, &[1, 0, 1]); // 2^128 + 1
        let sq = a.mul(&a).unwrap();
        assert_eq!(sq.width, 129);
        assert_eq!(sq.to_u64_lossy(), 1);
        // At a wider context the full square materializes:
        let aw = wide(258, &[1, 0, 1]);
        let sqw = aw.mul(&aw).unwrap();
        assert_eq!(sqw.width, 258);
        assert_eq!(sqw.to_u64_lossy(), 1);
        assert_eq!(sqw.part(129, 129).unwrap().to_u64_lossy(), 1);
        assert_eq!(sqw.part(256, 256).unwrap().to_u64_lossy(), 1);
        // Division: 2^200 / 2^100 = 2^100.
        let two201 = wide(201, &[0, 0, 0, 1 << 8]); // 2^200 (bit 200: limb 3, off 8)
        let two150 = wide(150, &[0, 1 << 36, 0]); // 2^100 (limb 1, off 36)
        let q = two201.div(&two150).unwrap();
        assert_eq!(q.part(100, 100).unwrap().to_u64_lossy(), 1);
        assert_eq!(q.to_u128_lossy(), 1u128 << 100, "quotient = 2^100");
        assert_eq!(q.part(99, 0).unwrap().to_u64_lossy(), 0);
        // mod 2^100: (2^200 + 5) mod 2^100 = 5.
        let five = wide(201, &[5, 0, 0, 1 << 8]);
        let r = five.rem(&two150).unwrap();
        assert_eq!(r.to_u64_lossy(), 5);
        // x/0 = x at wide too.
        assert!(m192.div(&Bits::zeros(192).unwrap()).unwrap().has_unknown());
        // pow: exact modular semantics — 3^200 truncated to the 300-bit
        // result width (Python cross-checked constant).
        let three = Bits::from_u128(300, 3).unwrap();
        let e200 = Bits::from_u128(300, 200).unwrap();
        let p = three.pow(&e200).unwrap();
        assert_eq!(p.width, 300);
        assert_eq!(
            p.fmt_dec(),
            "793849668459578066151832703989653410385866505736381627336266876004462561183201719148392609"
        );
        // At a 320-bit context the full 3^200 materializes.
        let three320 = Bits::from_u128(320, 3).unwrap();
        let e320 = Bits::from_u128(320, 200).unwrap();
        let pfull = three320.pow(&e320).unwrap();
        assert_eq!(
            pfull.fmt_dec(),
            "265613988875874769338781322035779626829233452653394495974574961739092490901302182994384699044001"
        );
    }

    #[test]
    fn wide_signed() {
        // -5 at 200 bits = ~5 + 1.
        let five = Bits::from_u128(200, 5).unwrap();
        let neg5 = five
            .bit_not()
            .add(&Bits::from_u128(200, 1).unwrap())
            .unwrap();
        let three = Bits::from_u128(200, 3).unwrap();
        let q = neg5.sdiv(&three).unwrap(); // -5/3 = -1 (trunc toward zero)
        assert_eq!(q.fmt_dec_signed(), "-1");
        let r = neg5.srem(&three).unwrap(); // -5 % 3 = -2 (sign of dividend)
        assert_eq!(r.fmt_dec_signed(), "-2");
        // Signed compares (the live-bug fix: pre-M8 these wrapped via i64).
        assert_eq!(neg5.cmp_signed(&three), Some(std::cmp::Ordering::Less));
        assert_eq!(three.cmp_signed(&neg5), Some(std::cmp::Ordering::Greater));
        let minus_one = Bits::zeros(200)
            .unwrap()
            .sub(&Bits::from_u128(200, 1).unwrap())
            .unwrap();
        assert_eq!(minus_one.cmp_signed(&three), Some(std::cmp::Ordering::Less));
        // spow: (-2)^3 = -8 at wide.
        let neg2 = Bits::zeros(200)
            .unwrap()
            .sub(&Bits::from_u128(200, 2).unwrap())
            .unwrap();
        let cube = neg2.spow(&three).unwrap();
        assert_eq!(cube.fmt_dec_signed(), "-8");
        // spow negative exponent table: (-1)^-3 = -1, 2^-3 = 0.
        let minus_three = Bits::zeros(200).unwrap().sub(&three).unwrap();
        assert_eq!(minus_one.spow(&minus_three).unwrap().fmt_dec_signed(), "-1");
        // Both-negative compare: -1 > -3 (same sign = pattern order).
        assert_eq!(
            minus_one.cmp_signed(&minus_three),
            Some(std::cmp::Ordering::Greater)
        );
        assert_eq!(
            minus_three.cmp_signed(&minus_one),
            Some(std::cmp::Ordering::Less)
        );
        let two = Bits::from_u128(200, 2).unwrap();
        assert_eq!(two.spow(&minus_three).unwrap().fmt_dec_signed(), "0");
        // 2^199 with the sign bit set is NEGATIVE at width 200.
        let two199 = wide(200, &[0, 0, 0, 1 << 7]);
        assert_eq!(two199.cmp_signed(&three), Some(std::cmp::Ordering::Less));
        // fmt_dec_signed(-2^199) == "-" + fmt_dec(2^199).
        let neg_big = minus_one.mul(&two199).unwrap();
        assert_eq!(neg_big.fmt_dec_signed(), format!("-{}", two199.fmt_dec()));
        // Live-bug repro pin (adversarial /tmp/widecmp3.v, 96-bit):
        // 2^64 vs 2^64−1 must compare unsigned-greater AND signed-greater
        // (pre-M8 both read through i64 and diverged from the oracle).
        let a96 = Bits::from_u128(96, 1u128 << 64).unwrap();
        let b96 = Bits::from_u128(96, (1u128 << 64) - 1).unwrap();
        assert_eq!(a96.cmp_unsigned(&b96), Some(std::cmp::Ordering::Greater));
        assert_eq!(a96.cmp_signed(&b96), Some(std::cmp::Ordering::Greater));
    }

    #[test]
    fn wide_shifts_and_parts() {
        let v = wide(256, &[0xdead_beef_cafe_babe, 0x1234_5678_9abc_def0, 0, 0]);
        // shl 64: limb 0 → limb 1.
        let sh = v.shl(&Bits::from_u128(300, 64).unwrap()).unwrap();
        assert_eq!(sh.part(127, 64).unwrap().to_u64_lossy(), 0xdead_beef_cafe_babe);
        // shr 64 back.
        let back = sh.shr(&Bits::from_u128(300, 64).unwrap()).unwrap();
        assert_eq!(back.to_u64_lossy(), 0xdead_beef_cafe_babe);
        // ashr of a negative wide value: sign fill.
        let minus = Bits::zeros(256)
            .unwrap()
            .sub(&Bits::from_u128(256, 0x10).unwrap())
            .unwrap();
        let sh1 = minus.ashr(&Bits::from_u128(256, 1).unwrap(), true).unwrap();
        assert_eq!(sh1.to_u64_lossy(), 0xFFFF_FFFF_FFFF_FFF8, "-16 >>1 = -8");
        // part across limb boundary [195:130].
        let base = wide(256, &[u64::MAX, u64::MAX, u64::MAX, u64::MAX]);
        let p = base.part(195, 130).unwrap();
        assert_eq!(p.width, 66);
        assert_eq!(p.to_u128_lossy(), (1u128 << 66) - 1);
        // set_part across the boundary.
        let s = base.set_part(195, 130, &Bits::zeros(66).unwrap()).unwrap();
        assert_eq!(s.part(129, 128).unwrap().to_u64_lossy(), 3);
        assert_eq!(s.part(196, 196).unwrap().to_u64_lossy(), 1);
        // concat of 3×100-bit (small-arm parts) into 300.
        let c = Bits::concat(&[
            Bits::from_u128(100, 1).unwrap(),
            Bits::from_u128(100, 2).unwrap(),
            Bits::from_u128(100, 3).unwrap(),
        ])
        .unwrap();
        assert_eq!(c.width, 300);
        assert_eq!(c.to_u64_lossy(), 3);
        assert_eq!(c.part(199, 100).unwrap().to_u64_lossy(), 2);
        assert_eq!(c.part(299, 200).unwrap().to_u64_lossy(), 1);
        // replicate 129-bit ×3 = 387.
        let unit = wide(129, &[7, 0, 1]);
        let rep = unit.replicate(3).unwrap();
        assert_eq!(rep.width, 387);
        assert_eq!(rep.to_u64_lossy(), 7);
    }

    #[test]
    fn wide_x_tables_and_reduce() {
        let x = Bits::all_x(200).unwrap();
        assert!(x.is_wide());
        let one = Bits::from_u128(200, 1).unwrap();
        assert_eq!(one.bit_and(&x).unwrap().fmt_bin().chars().last().unwrap(), 'x');
        assert_eq!(one.bit_or(&x).unwrap().fmt_bin().chars().next_back().unwrap(), '1');
        assert_eq!(x.bit_not().fmt_bin().chars().next().unwrap(), 'x');
        // reduce over an all-ones wide.
        let ones = Bits::zeros(200).unwrap().sub(&one).unwrap();
        assert_eq!(ones.reduce_and().unwrap().to_u64_lossy(), 1);
        assert_eq!(ones.reduce_or().unwrap().to_u64_lossy(), 1);
        // even number of ones → xor 0.
        let two = Bits::from_u128(200, 3).unwrap();
        assert_eq!(two.reduce_xor().unwrap().to_u64_lossy(), 0);
        // merge with unknown bits.
        let m = one.merge(&x).unwrap();
        assert!(m.has_unknown());
    }

    #[test]
    fn wide_eq_case_and_clog2() {
        let a = wide(200, &[0xaaaa, 0, 0, 0]);
        assert_eq!(a.case_eq(&a).unwrap().to_u64_lossy(), 1);
        assert_eq!(a.logic_eq(&a).unwrap().to_u64_lossy(), 1);
        let b = wide(200, &[0xaaab, 0, 0, 0]);
        assert_eq!(a.logic_eq(&b).unwrap().to_u64_lossy(), 0);
        // casez with z don't-care in the label (wide): low 4 bits z, rest 0.
        let label = Bits::zeros(200)
            .unwrap()
            .set_part(3, 0, &Bits::all_x(4).unwrap())
            .unwrap();
        let lo5 = wide(200, &[0x5, 0, 0, 0]); // care bits (4+) all zero
        assert!(lo5.casez_eq(&label).unwrap());
        let hi = wide(200, &[0x50, 0, 0, 0]); // bit 4 set vs label 0 → mismatch
        assert!(!hi.casez_eq(&label).unwrap());
        // clog2 of 2^200 → 200.
        let p = wide(300, &[0, 0, 0, 1 << 8, 0]);
        assert_eq!(p.clog2().unwrap(), 200);
        // clog2 of 0/1 → 0.
        assert_eq!(Bits::zeros(300).unwrap().clog2().unwrap(), 0);
        assert_eq!(Bits::from_u128(300, 1).unwrap().clog2().unwrap(), 0);
        // clog2 of a 96-bit value (pre-M8 u64-truncation zone).
        let big = Bits::from_u128(96, 1u128 << 90).unwrap();
        assert_eq!(big.clog2().unwrap(), 90);
    }

    #[test]
    fn mixed_n_w_crosscheck() {
        // Mixed small/wide ops must equal promote-then-op (ADR-0008 law).
        let small = Bits::from_u128(64, 0xdeadbeef).unwrap();
        let big = wide(192, &[0xdeadbeef, 0, 0]);
        assert_eq!(small.bit_and(&big).unwrap(), big.bit_and(&small).unwrap());
        assert_eq!(small.add(&big).unwrap(), big.add(&small).unwrap());
        assert_eq!(small.mul(&big).unwrap(), big.mul(&small).unwrap());
        assert_eq!(small.logic_eq(&big).unwrap().to_u64_lossy(), 1);
        let q1 = big.div(&small).unwrap();
        let small_promoted = small.uadjust(192).unwrap();
        let q2 = big.div(&small_promoted).unwrap();
        assert_eq!(q1, q2);
        assert_eq!(q1.part(64, 0).unwrap().to_u64_lossy(), 1);
        // Results whose width ≤ 128 canonicalize back to the small arm.
        let p = big.part(63, 0).unwrap();
        assert!(!p.is_wide());
        let cmp = big.logic_eq(&big).unwrap();
        assert!(!cmp.is_wide());
    }

    /// Mechanical check for the AGENTS 10.8 limb laws: for every
    /// context-determined op, computing at the value's own width (N arm) must
    /// equal computing sign-padded at a wide width (W arm) truncated back.
    /// Self-determined ops (reductions, concat, replicate, truthy) are
    /// excluded by definition (IEEE 1364-2005 §5.4 — padding is not part of
    /// their operand). This would have caught the pack two-limb bug, the
    /// reduce care-mask bug and the cmp reversal mechanically.
    #[test]
    fn n_w_padding_property() {
        let widths = [1u32, 4, 7, 63, 64, 65, 80, 100, 127, 128];
        let tw = 192u32; // any width > 128 forces the W arm
        let mag = |w: u32, k: usize| -> u128 {
            match k {
                0 => 0,
                1 => 1,
                2 => low_bits(w),                       // all ones
                3 => 1u128 << (w - 1),                  // sign bit
                _ => 0xA5A5_5A5A_0000_00FF & low_bits(w), // pattern
            }
        };
        for &w in &widths {
            for k1 in 0..5 {
                for k2 in 0..5 {
                    let a = Bits::from_u128(w, mag(w, k1)).unwrap();
                    let b = Bits::from_u128(w, mag(w, k2)).unwrap();
                    let aw = a.uadjust(tw).unwrap();
                    let bw = b.uadjust(tw).unwrap();
                    let asg = a.sadjust(tw).unwrap();
                    let bsg = b.sadjust(tw).unwrap();
                    let cmp1 = |op: &str, r: &Bits, rw: &Bits| {
                        assert_eq!(r.fmt_bin(), rw.fmt_bin(),
                            "op {op} mismatch at width {w} ({k1},{k2})");
                    };
                    let cmp1b = |op: &str, r: &Bits, rw: &Bits| {
                        assert_eq!(r.to_u64_lossy(), rw.to_u64_lossy(),
                            "op {op} 1-bit mismatch at width {w} ({k1},{k2})");
                    };
                    cmp1("add", &a.add(&b).unwrap(), &aw.add(&bw).unwrap().trunc(w).unwrap());
                    cmp1("sub", &a.sub(&b).unwrap(), &aw.sub(&bw).unwrap().trunc(w).unwrap());
                    cmp1("mul", &a.mul(&b).unwrap(), &aw.mul(&bw).unwrap().trunc(w).unwrap());
                    cmp1("div", &a.div(&b).unwrap(), &aw.div(&bw).unwrap().trunc(w).unwrap());
                    cmp1("rem", &a.rem(&b).unwrap(), &aw.rem(&bw).unwrap().trunc(w).unwrap());
                    cmp1("and", &a.bit_and(&b).unwrap(), &aw.bit_and(&bw).unwrap().trunc(w).unwrap());
                    cmp1("or", &a.bit_or(&b).unwrap(), &aw.bit_or(&bw).unwrap().trunc(w).unwrap());
                    cmp1("xor", &a.bit_xor(&b).unwrap(), &aw.bit_xor(&bw).unwrap().trunc(w).unwrap());
                    cmp1("merge", &a.merge(&b).unwrap(), &aw.merge(&bw).unwrap().trunc(w).unwrap());
                    cmp1b("eq", &a.logic_eq(&b).unwrap(), &aw.logic_eq(&bw).unwrap());
                    cmp1b("case_eq", &a.case_eq(&b).unwrap(), &aw.case_eq(&bw).unwrap());
                    cmp1b("lt", &a.lt(&b).unwrap(), &aw.lt(&bw).unwrap());
                    let sh = (k1 as u128) % (w as u128).max(1); // keep below w
                    let amt = Bits::from_u128(w, sh).unwrap();
                    let amtw = Bits::from_u128(tw, sh).unwrap();
                    cmp1("shl", &a.shl(&amt).unwrap(), &aw.shl(&amtw).unwrap().trunc(w).unwrap());
                    cmp1("shr", &a.shr(&amt).unwrap(), &aw.shr(&amtw).unwrap().trunc(w).unwrap());
                    cmp1("ashr", &a.ashr(&amt, true).unwrap(), &asg.ashr(&amtw, true).unwrap().trunc(w).unwrap());
                    cmp1("part", &a.part(w / 2, 0).unwrap(), &aw.part(w / 2, 0).unwrap());
                    if a.is_known() && b.is_known() {
                        cmp1("sdiv", &a.sdiv(&b).unwrap(), &asg.sdiv(&bsg).unwrap().trunc(w).unwrap());
                        cmp1("srem", &a.srem(&b).unwrap(), &asg.srem(&bsg).unwrap().trunc(w).unwrap());
                        // Same-sign and cross-sign compare order must be
                        // padding-invariant (the both-negative reversal bug
                        // class — AGENTS 10.8 ③).
                        assert_eq!(a.cmp_signed(&b), asg.cmp_signed(&bsg));
                    }
                }
            }
        }
        // Limb round-trip: of/padded preserve the value exactly.
        for &w in &widths {
            for k in 0..5 {
                let a = Bits::from_u128(w, mag(w, k)).unwrap();
                let rt = Limbs::of(&a).pack(w).unwrap();
                assert_eq!(a, rt, "limb round-trip at width {w} ({k})");
                let pt = Limbs::padded(&a, 200).pack(200).unwrap();
                assert_eq!(pt.trunc(w).unwrap(), a, "padded round-trip at width {w} ({k})");
            }
        }
    }

    #[test]
    fn cap_errors() {
        assert!(Bits::zeros(MAX_WIDTH + 1).is_err());
        assert!(Bits::all_x(MAX_WIDTH + 1).is_err());
        let unit = Bits::from_u128(8, 0xff).unwrap();
        assert!(unit.replicate((1 << 20) as u64 / 8 + 1).is_err());
        assert!(range_width(2_000_000, 0).is_err());
        assert_eq!(range_width(255, 0).unwrap(), 256);
        assert_eq!(range_width(0, 255).unwrap(), 256);
    }

    #[test]
    fn case_wildcard_match_truth_table() {
        // Per-bit table over (a, b): (casez, casex). z/? wildcard both
        // sides for casez; x AND z for casex; x stays literal for casez.
        let b = |s: &str| from_numlit_parts(Some(s.len() as u32), false, Base::Bin, s).unwrap();
        let check = |a: &str, c: &str, cz: bool, cx: bool| {
            let (x, y) = (b(a), b(c));
            assert_eq!(x.casez_match(&y), cz, "casez {a} vs {c}");
            assert_eq!(x.casex_match(&y), cx, "casex {a} vs {c}");
        };
        check("0", "0", true, true);
        check("1", "0", false, false);
        check("z", "0", true, true); // z wildcard either side
        check("0", "z", true, true);
        check("x", "0", false, true); // casez: x literal
        check("0", "x", false, true);
        check("x", "x", true, true); // literal equality
        check("z", "z", true, true);
        check("z", "x", true, true); // z in either side wildcards casez too
        check("x", "z", true, true);
        check("1", "x", false, true);
        // Vector forms (13/26 wildcards).
        check("10x1", "1011", false, true);
        check("10?1", "1011", true, true);
        check("00", "01", false, false);
        // Width mismatch zero-extends (b=01 vs a=0101 → bit-2 mismatch).
        check("0101", "01", false, false);
        check("0001", "01", true, true);
    }

    #[test]
    fn bit_not_is_width_canonical() {
        // The 42_nba_handshake bug: ~1 at width 1 left !1 (=2^128-2) in the
        // raw data; fmt_bin masked it, truthy()/case_eq() did not. The
        // canonical invariant is data == 0 above the width whenever the
        // value is fully known.
        for w in 1..=128u32 {
            let ones = Bits::from_u128(w, low_bits(w)).unwrap();
            let n = ones.bit_not();
            assert_eq!(n.width, w);
            if let Repr::N { data, mask } = &n.repr {
                assert_eq!(
                    *data & !low_bits(w),
                    0,
                    "width {w}: garbage above width in bit_not data"
                );
                assert_eq!(*mask & !low_bits(w), 0, "width {w}: garbage above width in mask");
                assert_eq!(*data, low_bits(w) & !low_bits(w), "width {w}: ~all-ones must be 0");
            }
            assert!(!n.truthy().unwrap().unwrap(), "width {w}: ~all-ones is falsy");
        }
        // W-arm: >128-bit widths mask the top-limb remainder.
        for w in [129u32, 130, 191, 200] {
            let ones = Bits::from_u128(w, u128::MAX).unwrap(); // low 128 ones; top bits 0? no — build all-ones via wide
            let _ = ones;
        }
        let all_ones_wide = {
            let n = limbs_for(200);
            pack(200, &vec![u64::MAX; n], &vec![0u64; n]).unwrap()
        };
        let n = all_ones_wide.bit_not();
        if let Repr::W { data, mask } = &n.repr {
            let rem = 200 % 64;
            let top = data.last().unwrap();
            assert_eq!(
                top & !((1u64 << rem) - 1),
                0,
                "top-limb garbage above width in bit_not (W arm)"
            );
            assert_eq!(*top, 0, "~all-ones at width 200: top limb (bits 192..199) must be 0");
            for m in mask.iter() {
                assert_eq!(*m, 0);
            }
        } else {
            panic!("width 200 must stay in W repr");
        }
    }
}
