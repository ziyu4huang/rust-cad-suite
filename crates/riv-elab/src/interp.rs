//! Constant-time interpreter over lowered ops (the elaborator's const
//! evaluator). It executes the SAME ops the kernel will, through the SAME
//! application layer (`vl-ops`) — one lowering, one semantics, zero
//! duplication (ADR-0009 U-2; AGENTS §5: width/sign rules live in one
//! place).
//!
//! Scheduling/system ops are errors here — constant contexts cannot
//! contain them (the lowering would have hard-errored first anyway).

use riv_ir::{Op, SysId};
use vl_values::Bits;

pub struct NotConst(pub String);

impl std::fmt::Display for NotConst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
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
                let b = vl_ops::parse_const(lit).map_err(|e| NotConst(format!("literal: {}", e.0)))?;
                last = b.clone();
                frame[*dst as usize] = b;
            }
            Op::Mov { dst, src } => {
                last = frame[*src as usize].clone();
                frame[*dst as usize] = last.clone();
            }
            Op::Bin { kind, dst, a, b, w, signed } => {
                let r = vl_ops::apply_bin(*kind, &frame[*a as usize], &frame[*b as usize], *w, *signed)
                    .map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::Bit { kind, dst, a, b, w, signed } => {
                let r = vl_ops::apply_bit(*kind, &frame[*a as usize], &frame[*b as usize], *w, *signed)
                    .map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::BitXnor { dst, a, b, w } => {
                let r = vl_ops::apply_bit_xnor(&frame[*a as usize], &frame[*b as usize], *w)
                    .map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::BitNot { dst, a, w, signed } => {
                let r = vl_ops::apply_bit_not(&frame[*a as usize], *w, *signed)
                    .map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::Reduce { kind, dst, a } => {
                let r = vl_ops::apply_reduce(*kind, &frame[*a as usize])
                    .map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::LogicAnd { dst, a, b } => {
                let r = vl_ops::apply_logic_and(&frame[*a as usize], &frame[*b as usize])
                    .map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::LogicOr { dst, a, b } => {
                let r = vl_ops::apply_logic_or(&frame[*a as usize], &frame[*b as usize])
                    .map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::LogicNot { dst, a } => {
                let r = vl_ops::apply_logic_not(&frame[*a as usize]).map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::Cmp { kind, dst, a, b, signed } => {
                let r = vl_ops::apply_cmp(*kind, &frame[*a as usize], &frame[*b as usize], *signed)
                    .map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::CaseCmp { kind, dst, a, b } => {
                let r = vl_ops::apply_case_cmp(*kind, &frame[*a as usize], &frame[*b as usize])
                    .map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::Shift { kind, dst, a, b, w, signed } => {
                let r = vl_ops::apply_shift(*kind, &frame[*a as usize], &frame[*b as usize], *w, *signed)
                    .map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::Sel { dst, cond, a, b } => {
                let r = vl_ops::apply_sel(&frame[*cond as usize], &frame[*a as usize], &frame[*b as usize])
                    .map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::Concat { dst, parts } => {
                let vals: Vec<Bits> = parts.iter().map(|p| frame[*p as usize].clone()).collect();
                let r = vl_ops::apply_concat(&vals).map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::Replicate { dst, src, times } => {
                let r = vl_ops::apply_replicate(&frame[*src as usize], *times)
                    .map_err(|e| NotConst(e.0))?;
                last = r.clone();
                frame[*dst as usize] = r;
            }
            Op::LoadVar { .. } | Op::LoadBit { .. } | Op::LoadPart { .. } | Op::LoadPartDyn { .. } | Op::LoadElem { .. } | Op::AssignNbaDelay { .. } => {
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
