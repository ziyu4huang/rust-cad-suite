//! Constant-time interpreter over lowered ops (the elaborator's const
//! evaluator). It executes the SAME ops the kernel will, so parameter and
//! range values share the runtime width semantics — one lowering, no
//! duplicated arithmetic (design §5-2: semantic reimplementation, and
//! AGENTS §5: width/sign rules live in one place).
//!
//! Scheduling/system ops are errors here — constant contexts cannot
//! contain them (the lowering would have hard-errored first anyway).

use riv_ir::{BinKind, BitKind, CmpKind, Op, RedKind, ShiftKind, SysId};
use vl_values::Bits;

pub struct NotConst(pub String);

impl std::fmt::Display for NotConst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn adjust(b: &Bits, w: u32, signed: bool) -> Bits {
    if signed {
        b.sadjust(w).unwrap_or_else(|_| Bits::zeros(w).unwrap())
    } else {
        b.uadjust(w).unwrap_or_else(|_| Bits::zeros(w).unwrap())
    }
}

pub fn eval_ops(ops: &[Op], frame: &mut Vec<Bits>) -> Result<Bits, NotConst> {
    let mut pc = 0usize;
    let mut last = Bits::zeros(1).unwrap();
    while pc < ops.len() {
        let op = &ops[pc];
        pc += 1;
        match op {
            Op::Const { dst, lit } => {
                let b = vl_values::from_numlit_parts(
                    Some(lit.width),
                    lit.signed,
                    match lit.base {
                        riv_ir::Base::Dec => vl_values::Base::Dec,
                        riv_ir::Base::Bin => vl_values::Base::Bin,
                        riv_ir::Base::Oct => vl_values::Base::Oct,
                        riv_ir::Base::Hex => vl_values::Base::Hex,
                    },
                    &lit.digits,
                )
                .map_err(|e| NotConst(format!("literal: {}", e.0)))?;
                last = b.clone();
                frame[*dst as usize] = b;
            }
            Op::Mov { dst, src } => {
                last = frame[*src as usize].clone();
                frame[*dst as usize] = last.clone();
            }
            Op::Bin { kind, dst, a, b, w, signed } => {
                let av = adjust(&frame[*a as usize], *w, *signed);
                let bv = adjust(&frame[*b as usize], *w, *signed);
                let r = match kind {
                    BinKind::Add => av.add(&bv),
                    BinKind::Sub => av.sub(&bv),
                    BinKind::Mul => av.mul(&bv),
                    BinKind::Div => {
                        if *signed {
                            av.sdiv(&bv)
                        } else {
                            av.div(&bv)
                        }
                    }
                    BinKind::Rem => {
                        if *signed {
                            av.srem(&bv)
                        } else {
                            av.rem(&bv)
                        }
                    }
                    BinKind::Pow => {
                        if *signed {
                            av.spow(&bv)
                        } else {
                            av.pow(&bv)
                        }
                    }
                }
                .map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::Bit { kind, dst, a, b, w } => {
                let av = adjust(&frame[*a as usize], *w, false);
                let bv = adjust(&frame[*b as usize], *w, false);
                let r = match kind {
                    BitKind::And => av.bit_and(&bv),
                    BitKind::Or => av.bit_or(&bv),
                    BitKind::Xor => av.bit_xor(&bv),
                }
                .map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::BitXnor { dst, a, b, w } => {
                let av = adjust(&frame[*a as usize], *w, false);
                let bv = adjust(&frame[*b as usize], *w, false);
                let r = av.bit_xor(&bv).map_err(|e| NotConst(e.0))?;
                let r = r.bit_not();
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::BitNot { dst, a, w } => {
                let r = adjust(&frame[*a as usize], *w, false).bit_not();
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::Reduce { kind, dst, a } => {
                let av = &frame[*a as usize];
                let r = match kind {
                    RedKind::And => av.reduce_and(),
                    RedKind::Or => av.reduce_or(),
                    RedKind::Xor => av.reduce_xor(),
                    RedKind::Nand => Ok(av.reduce_and().map_err(|e| NotConst(e.0))?.bit_not()),
                    RedKind::Nor => Ok(av.reduce_or().map_err(|e| NotConst(e.0))?.bit_not()),
                    RedKind::Xnor => Ok(av.reduce_xor().map_err(|e| NotConst(e.0))?.bit_not()),
                }
                .map_err(|e: vl_values::ValueError| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::LogicAnd { dst, a, b } => {
                let av = truthy(&frame[*a as usize])?;
                let bv = truthy(&frame[*b as usize])?;
                let r = Bits::from_u64(1, (av && bv) as u64).unwrap();
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::LogicOr { dst, a, b } => {
                let av = truthy(&frame[*a as usize])?;
                let bv = truthy(&frame[*b as usize])?;
                let r = Bits::from_u64(1, (av || bv) as u64).unwrap();
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::LogicNot { dst, a } => {
                let av = truthy(&frame[*a as usize])?;
                let r = Bits::from_u64(1, !av as u64).unwrap();
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::Cmp { kind, dst, a, b, signed } => {
                let (av, bv) = cmp_operands(&frame[*a as usize], &frame[*b as usize], *signed);
                let ord = if *signed {
                    av.cmp_signed(&bv)
                } else {
                    av.cmp_unsigned(&bv)
                };
                let r = match (kind, ord) {
                    (_, None) => Bits::all_x(1).unwrap(),
                    (CmpKind::Lt, Some(o)) => Bits::from_u64(1, o.is_lt() as u64).unwrap(),
                    (CmpKind::Le, Some(o)) => Bits::from_u64(1, o.is_le() as u64).unwrap(),
                    (CmpKind::Gt, Some(o)) => Bits::from_u64(1, o.is_gt() as u64).unwrap(),
                    (CmpKind::Ge, Some(o)) => Bits::from_u64(1, o.is_ge() as u64).unwrap(),
                    (CmpKind::Eq, Some(o)) => Bits::from_u64(1, o.is_eq() as u64).unwrap(),
                    (CmpKind::Ne, Some(o)) => Bits::from_u64(1, o.is_ne() as u64).unwrap(),
                };
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::CaseCmp { kind, dst, a, b } => {
                let av = &frame[*a as usize];
                let bv = &frame[*b as usize];
                let r = match kind {
                    riv_ir::CaseCmpKind::ExactEq => av.case_eq(bv),
                    riv_ir::CaseCmpKind::ExactNe => av.case_ne(bv),
                    // Wildcard matching never yields x — const contexts in
                    // the corpus only use exact ===; wild forms map to 1/0.
                    riv_ir::CaseCmpKind::WildZ => {
                        Ok(Bits::from_u64(1, av.casez_match(bv) as u64).unwrap())
                    }
                    riv_ir::CaseCmpKind::WildX => {
                        Ok(Bits::from_u64(1, av.casex_match(bv) as u64).unwrap())
                    }
                }
                .map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::Shift { kind, dst, a, b, w, signed } => {
                let av = adjust(&frame[*a as usize], *w, *signed);
                let bv = &frame[*b as usize];
                let r = match kind {
                    ShiftKind::Shl => av.shl(bv),
                    ShiftKind::Shr => av.shr(bv),
                    ShiftKind::Ashr => av.ashr(bv, *signed),
                }
                .map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::Sel { dst, cond, a, b } => {
                let cv = &frame[*cond as usize];
                let r = match cv.truthy().map_err(|e| NotConst(e.0))? {
                    Some(true) => frame[*a as usize].clone(),
                    Some(false) => frame[*b as usize].clone(),
                    None => frame[*a as usize]
                        .merge(&frame[*b as usize])
                        .map_err(|e| NotConst(e.0))?,
                };
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::Concat { dst, parts } => {
                let vals: Vec<Bits> = parts.iter().map(|p| frame[*p as usize].clone()).collect();
                let r = Bits::concat(&vals).map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::Replicate { dst, src, times } => {
                let base = &frame[*src as usize];
                let vals: Vec<Bits> = (0..*times).map(|_| base.clone()).collect();
                let r = Bits::concat(&vals).map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::LoadVar { .. } | Op::LoadBit { .. } | Op::LoadPart { .. } | Op::LoadPartDyn { .. } => {
                return Err(NotConst(
                    "reads a non-constant variable".into(),
                ))
            }
            Op::AssignBlk { .. }
            | Op::AssignNba { .. }
            | Op::Branch { .. }
            | Op::Jump { .. }
            | Op::Delay { .. }
            | Op::ScaleTime { .. }
            | Op::WaitEdge { .. }
            | Op::Call { .. }
            | Op::Ret { .. }
            | Op::AssignForce { .. }
            | Op::Release { .. }
            | Op::Fork { .. }
            | Op::JoinDone
            | Op::JoinPoint
            | Op::EndThread => return Err(NotConst("control flow is not constant".into())),
            Op::SysCall { id, args, dst, .. } => {
                if *id == SysId::Clog2 {
                    let Some(dst) = dst else {
                        return Err(NotConst("clog2 without destination".into()));
                    };
                    let Some(arg) = args.first() else {
                        return Err(NotConst("clog2 without argument".into()));
                    };
                    let v = &frame[arg.slot as usize];
                    let c = v.clog2().map_err(|e| NotConst(e.0))?;
                    let r = Bits::from_u64(32, c).unwrap();
                    last = r.clone();
                    frame[*dst as usize] = r;
                } else {
                    return Err(NotConst("system calls are not constant".into()));
                }
            }
        }
    }
    Ok(last)
}

fn truthy(b: &Bits) -> Result<bool, NotConst> {
    b.truthy()
        .map_err(|e| NotConst(e.0))?
        .ok_or_else(|| NotConst("unknown condition".into()))
}

fn cmp_operands(a: &Bits, b: &Bits, signed: bool) -> (Bits, Bits) {
    let w = a.width.max(b.width);
    (adjust(a, w, signed), adjust(b, w, signed))
}
