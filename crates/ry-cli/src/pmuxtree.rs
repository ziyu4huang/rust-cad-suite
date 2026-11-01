//! pmuxtree(passes/techmap/pmuxtree.cc 同構移植;RYOSYS-MVP §11.3-3)。
//!
//! * $pmux→$mux 樹;or_generator 四臂(:28-38:0→S0/1→原樣/2→Or/
//!   default→ReduceOr);recursive_mux_generator 遞歸(:40-66)+fully_undef
//!   分支(:99-102 execute 內)。
//! * NEW_ID 錨=clone v0.70 與二進制 **零偏移**(fsm-pbo 錨 golden 親證:
//!   Mux=:65 recursive_mux_generator、ReduceOr=:100/Not=:101 execute、
//!   Or=:35/ReduceOr=:37 or_generator;helper 內部 wire=二進制觀測錨
//!   Or=rtlil.cc:3250/ReduceOr=3205/Not=3201/Mux=3337)。
//! * 遍歷=selected_modules×selected_cells 值拷貝(rev);connect+remove
//!   序按 :104-105;無旗標,參數面=cmd_error(selection 裸字=D-ys11,
//!   oracle 經 extra_args 接受 rc=0=行為面歧)。

use ry_ir::{Bit, Cell, Design, InsertMap, Module, SigBit, SigSpec, Wire};

use crate::help_desc::HELP_PMUXTREE;
use crate::opt::{cmd_error_pass, module_is_boxed};
use crate::opt_dff::{int_param, new_id, widths_of};
use crate::sigops::is_fully_undef;

fn wire_sig(name: &str, width: i64) -> SigSpec {
    SigSpec {
        bits: (0..width).map(|o| SigBit::Wire { wire: name.to_string(), offset: o }).collect(),
    }
}

/// or_generator(pmuxtree.cc:26-39)。
fn or_generator(m: &mut Module, autoidx: &mut i64, sig: &SigSpec) -> SigSpec {
    match sig.bits.len() {
        0 => SigSpec { bits: vec![SigBit::Const(Bit::S0)] },
        1 => sig.clone(),
        2 => {
            let cid = new_id(autoidx, "pmuxtree.cc", 35, "or_generator");
            let wid = new_id(autoidx, "rtlil.cc", 3250, "Or");
            m.wires.push(Wire { name: wid.clone(), width: 1, ..Default::default() });
            let y = wire_sig(&wid, 1);
            m.cells.push(Cell {
                ctype: "$or".to_string(),
                name: cid,
                attributes: InsertMap::new(),
                parameters: {
                    let mut im = InsertMap::new();
                    im.insert("\\A_SIGNED".into(), int_param(0));
                    im.insert("\\B_SIGNED".into(), int_param(0));
                    im.insert("\\A_WIDTH".into(), int_param(1));
                    im.insert("\\B_WIDTH".into(), int_param(1));
                    im.insert("\\Y_WIDTH".into(), int_param(1));
                    im
                },
                connections: vec![
                    ("\\A".into(), SigSpec { bits: vec![sig.bits[0].clone()] }),
                    ("\\B".into(), SigSpec { bits: vec![sig.bits[1].clone()] }),
                    ("\\Y".into(), y.clone()),
                ],
            });
            y
        }
        _ => {
            let cid = new_id(autoidx, "pmuxtree.cc", 37, "or_generator");
            let wid = new_id(autoidx, "rtlil.cc", 3205, "ReduceOr");
            m.wires.push(Wire { name: wid.clone(), width: 1, ..Default::default() });
            let y = wire_sig(&wid, 1);
            m.cells.push(Cell {
                ctype: "$reduce_or".to_string(),
                name: cid,
                attributes: InsertMap::new(),
                parameters: {
                    let mut im = InsertMap::new();
                    im.insert("\\A_SIGNED".into(), int_param(0));
                    im.insert("\\A_WIDTH".into(), int_param(sig.bits.len() as i64));
                    im.insert("\\Y_WIDTH".into(), int_param(1));
                    im
                },
                connections: vec![("\\A".into(), sig.clone()), ("\\Y".into(), y.clone())],
            });
            y
        }
    }
}

/// recursive_mux_generator(pmuxtree.cc:40-66;NEW_ID 錨=:65)。
fn rec(
    m: &mut Module,
    autoidx: &mut i64,
    sig_data: &SigSpec,
    sig_sel: &[SigBit],
    sig_or: &mut SigSpec,
) -> SigSpec {
    if sig_sel.len() == 1 {
        sig_or.bits.push(sig_sel[0].clone());
        return sig_data.clone();
    }
    let left_size = sig_sel.len() / 2;
    let right_size = sig_sel.len() - left_size;
    let stride = sig_data.bits.len() / sig_sel.len();

    let left_data = SigSpec { bits: sig_data.bits[..stride * left_size].to_vec() };
    let right_data =
        SigSpec { bits: sig_data.bits[stride * left_size..stride * left_size + stride * right_size].to_vec() };
    let left_sel = &sig_sel[..left_size];
    let right_sel = &sig_sel[left_size..];

    let mut left_or = SigSpec::default();
    let left_result = rec(m, autoidx, &left_data, left_sel, &mut left_or);
    let right_result = rec(m, autoidx, &right_data, right_sel, sig_or);
    let left_or = or_generator(m, autoidx, &left_or);
    sig_or.bits.push(left_or.bits[0].clone());

    let cid = new_id(autoidx, "pmuxtree.cc", 65, "recursive_mux_generator");
    let wid = new_id(autoidx, "rtlil.cc", 3337, "Mux");
    let w = left_result.bits.len() as i64;
    m.wires.push(Wire { name: wid.clone(), width: w, ..Default::default() });
    let y = wire_sig(&wid, w);
    m.cells.push(Cell {
        ctype: "$mux".to_string(),
        name: cid,
        attributes: InsertMap::new(),
        parameters: {
            let mut im = InsertMap::new();
            im.insert("\\WIDTH".into(), int_param(w));
            im
        },
        connections: vec![
            ("\\A".into(), right_result),
            ("\\B".into(), left_result),
            ("\\S".into(), left_or),
            ("\\Y".into(), y.clone()),
        ],
    });
    y
}

/// 命令面(§11.3-3)。
pub fn run_pmuxtree(
    design: &mut Design,
    args: &[String],
    log: &mut crate::Log,
) -> Result<(), crate::proc::ProcError> {
    log.header("Executing PMUXTREE pass.\n");
    // argidx 迴圈(:96-98)break 即出=零旗標;任何參數→extra_args 面
    // →cmd_error(未知旗標=oracle rc=1 一致;selection 裸字=oracle rc=0
    // 行為面歧=D-ys11,fixture 钉我方硬錯形)。
    if args.len() > 1 {
        return Err(crate::proc::ProcError(
            cmd_error_pass(log, args, 1, HELP_PMUXTREE).0,
        ));
    }
    for mi in (0..design.modules.len()).rev() {
        if module_is_boxed(&design.modules[mi]) {
            continue;
        }
        let mut m = std::mem::take(&mut design.modules[mi]);
        let mut autoidx = design.autoidx;
        let widths = widths_of(&m);
        let _ = &widths;
        for ci in (0..m.cells.len()).rev() {
            let cell = m.cells[ci].clone();
            if cell.ctype != "$pmux" {
                continue;
            }
            let port = |name: &str| -> SigSpec {
                cell.connections
                    .iter()
                    .find(|(p, _)| p == name)
                    .map(|(_, s)| s.clone())
                    .unwrap_or_default()
            };
            let mut sig_data = port("\\B");
            let mut sig_sel = port("\\S");
            let a = port("\\A");
            if !is_fully_undef(&a) {
                sig_data.bits.extend(a.bits.iter().cloned());
                // execute :100 ReduceOr + :101 Not(helper wire=3205/3201)
                let cid = new_id(&mut autoidx, "pmuxtree.cc", 100, "execute");
                let wid = new_id(&mut autoidx, "rtlil.cc", 3205, "ReduceOr");
                m.wires.push(Wire { name: wid.clone(), width: 1, ..Default::default() });
                let sel_or = wire_sig(&wid, 1);
                m.cells.push(Cell {
                    ctype: "$reduce_or".to_string(),
                    name: cid,
                    attributes: InsertMap::new(),
                    parameters: {
                        let mut im = InsertMap::new();
                        im.insert("\\A_SIGNED".into(), int_param(0));
                        im.insert("\\A_WIDTH".into(), int_param(sig_sel.bits.len() as i64));
                        im.insert("\\Y_WIDTH".into(), int_param(1));
                        im
                    },
                    connections: vec![
                        ("\\A".into(), sig_sel.clone()),
                        ("\\Y".into(), sel_or.clone()),
                    ],
                });
                let cid2 = new_id(&mut autoidx, "pmuxtree.cc", 101, "execute");
                let wid2 = new_id(&mut autoidx, "rtlil.cc", 3201, "Not");
                m.wires.push(Wire { name: wid2.clone(), width: 1, ..Default::default() });
                let sel_or_inv = wire_sig(&wid2, 1);
                m.cells.push(Cell {
                    ctype: "$not".to_string(),
                    name: cid2,
                    attributes: InsertMap::new(),
                    parameters: {
                        let mut im = InsertMap::new();
                        im.insert("\\A_SIGNED".into(), int_param(0));
                        im.insert("\\A_WIDTH".into(), int_param(1));
                        im.insert("\\Y_WIDTH".into(), int_param(1));
                        im
                    },
                    connections: vec![("\\A".into(), sel_or), ("\\Y".into(), sel_or_inv.clone())],
                });
                sig_sel.bits.push(sel_or_inv.bits[0].clone());
            }
            let mut result_or = SigSpec::default();
            let result = rec(&mut m, &mut autoidx, &sig_data, &sig_sel.bits, &mut result_or);
            // connect(Y, result)+remove(cell) 序(:104-105;swap-and-pop)。
            let y = port("\\Y");
            if let Some(p) = m.cells.iter().position(|c| c.name == cell.name) {
                m.cells.swap_remove(p);
            }
            crate::opt_dff::module_connect(&mut m, y, result);
        }
        design.autoidx = autoidx;
        design.modules[mi] = m;
    }
    Ok(())
}
