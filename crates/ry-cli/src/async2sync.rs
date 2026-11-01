//! async2sync(passes/sat/async2sync.cc 同構移植;RYOSYS-MVP §11.3-2)。
//!
//! * 遍歷=selected_modules(rev;盒模組跳過)×selected_cells **值拷貝**
//!   =dict 正向=逆插入序(Q4;`cells.iter().rev()`;RM11-2:錨 script=proc
//!   後直跑,無 opt_clean 前置=無 swap-and-pop 擾序)。
//! * NEW_ID 錨=clone v0.70 28ba3cb 與二進制 0.70+179 **零偏移**(錨 golden
//!   親證:arst face 253/257/262、aload face 220/221/225/226、sr face
//!   155/156/171/186/191/192/193/195/196/197;latch face=clone 行號);
//!   rtlil helper 內部 wire 錨=二進制觀測(Not=3201/And=3249/Or=3250/
//!   Mux=3337;clone rtlil.cc 行號與二進制漂移,禁以 clone 推)。
//! * `$print`/`$check` 面=§11.2 顯式不做:TRG_ENABLE=false 記 have_check
//!   (尾部 chformal -lower 依賴=chformal Y0 stub 如實硬錯);TRG_ENABLE=
//!   true 變換面=Y2 硬錯(語料普查=corpus 零 $check,零觸發)。
//! * 已知旗標 `-nolower`=Y2 硬錯(§11.1-8);未知旗標/selection 裸字=
//!   cmd_error 全形(selection=oracle rc=0 行為面歧,D-ys11)。

use ry_ir::{Bit, Cell, Const, Design, InsertMap, Module, SigBit, SigSpec, Wire};

use crate::help_desc::HELP_ASYNC2SYNC;
use crate::opt::module_is_boxed;
use crate::opt_dff::{ff_from_cell, FfD, FfInitVals, int_param, new_id, widths_of};
use crate::sigmap::SigMap;
use crate::sigops::is_fully_const;

/// oracle 進 FF 分派面=is_builtin_ff(celltypes);我方=ff_from_cell 覆蓋集。
const BUILTIN_FF: [&str; 17] = [
    "$dff", "$dffe", "$adff", "$adffe", "$sdff", "$sdffe", "$sdffce", "$dffsr", "$dffsre",
    "$aldff", "$aldffe", "$dlatch", "$adlatch", "$dlatchsr", "$sr", "$ff", "$anyinit",
];

/// 全寬單線 SigSpec(名字+寬;wl連接面用)。
fn wire_sig(name: &str, width: i64) -> SigSpec {
    SigSpec {
        bits: (0..width).map(|o| SigBit::Wire { wire: name.to_string(), offset: o }).collect(),
    }
}



/// Module::Not(NEW_ID, a)(rtlil.cc:3201 二進制錨):cell N+內部 wire N+1,
/// Y=A 寬。
fn mk_not(
    m: &mut Module,
    autoidx: &mut i64,
    anchor_line: u32,
    a: &SigSpec,
) -> SigSpec {
    let cid = new_id(autoidx, "async2sync.cc", anchor_line, "execute");
    let wid = new_id(autoidx, "rtlil.cc", 3201, "Not");
    let w = a.bits.len() as i64;
    m.wires.push(Wire { name: wid.clone(), width: w, ..Default::default() });
    let y = SigSpec { bits: vec![SigBit::Wire { wire: wid, offset: 0 }] };
    push_word_cell(m, "$not", cid, &[("\\A_SIGNED", 0i64), ("\\A_WIDTH", w), ("\\Y_WIDTH", w)], a, None, &y);
    y
}

/// Module::And/Or(NEW_ID, a, b):cell N+內部 wire N+1,Y=max(a,b) 寬。
fn mk_and_or(
    m: &mut Module,
    autoidx: &mut i64,
    anchor_line: u32,
    a: &SigSpec,
    b: &SigSpec,
    or: bool,
) -> SigSpec {
    let cid = new_id(autoidx, "async2sync.cc", anchor_line, "execute");
    let line = if or { 3250 } else { 3249 };
    let func = if or { "Or" } else { "And" };
    let wid = new_id(autoidx, "rtlil.cc", line, func);
    let w = a.bits.len().max(b.bits.len()) as i64;
    m.wires.push(Wire { name: wid.clone(), width: w, ..Default::default() });
    let y = SigSpec { bits: vec![SigBit::Wire { wire: wid, offset: 0 }] };
    push_word_cell(m, if or { "$or" } else { "$and" }, cid,
        &[("\\A_SIGNED", 0), ("\\B_SIGNED", 0), ("\\A_WIDTH", a.bits.len() as i64), ("\\B_WIDTH", b.bits.len() as i64), ("\\Y_WIDTH", w)],
        a, Some(b), &y);
    y
}

/// Module::addMux(NEW_ID, a, b, s, y) 顯式 Y 形:cell 只帶 \WIDTH。
fn add_mux_to(m: &mut Module, cid: String, a: SigSpec, b: SigSpec, s: SigSpec, y: SigSpec) {
    m.cells.push(Cell {
        ctype: "$mux".to_string(),
        name: cid,
        attributes: InsertMap::new(),
        parameters: {
            let mut im = InsertMap::new();
            im.insert("\\WIDTH".into(), int_param(a.bits.len() as i64));
            im
        },
        connections: vec![
            ("\\A".into(), a),
            ("\\B".into(), b),
            ("\\S".into(), s),
            ("\\Y".into(), y),
        ],
    });
}

/// Module::addBwmux(NEW_ID, a, b, s, y) 顯式 Y 形(\B=Const(Sx) 面可空串形)。
fn add_bwmux_to(m: &mut Module, cid: String, a: SigSpec, b: SigSpec, s: SigSpec, y: SigSpec) {
    m.cells.push(Cell {
        ctype: "$bwmux".to_string(),
        name: cid,
        attributes: InsertMap::new(),
        parameters: {
            let mut im = InsertMap::new();
            im.insert("\\WIDTH".into(), int_param(a.bits.len() as i64));
            im
        },
        connections: vec![
            ("\\A".into(), a),
            ("\\B".into(), b),
            ("\\S".into(), s),
            ("\\Y".into(), y),
        ],
    });
}

#[allow(clippy::too_many_arguments)]
fn push_word_cell(
    m: &mut Module,
    ty: &str,
    cid: String,
    int_params: &[(&str, i64)],
    a: &SigSpec,
    b: Option<&SigSpec>,
    y: &SigSpec,
) {
    let mut conns = vec![("\\A".into(), a.clone())];
    if let Some(bb) = b {
        conns.push(("\\B".into(), bb.clone()));
    }
    conns.push(("\\Y".into(), y.clone()));
    m.cells.push(Cell {
        ctype: ty.to_string(),
        name: cid,
        attributes: InsertMap::new(),
        parameters: {
            let mut im = InsertMap::new();
            for (k, v) in int_params {
                im.insert((*k).into(), int_param(*v));
            }
            im
        },
        connections: conns,
    });
}

/// FfData::unmap_ce(kernel/ff.cc:583;字級形;NEW_ID 錨=:592 pol_ce/:594
/// else;ce_over_srst 時先 unmap_srst)。
pub(crate) fn unmap_ce(ff: &mut FfD, m: &mut Module, autoidx: &mut i64) {
    if !ff.has_ce {
        return;
    }
    if ff.has_srst && ff.ce_over_srst {
        unmap_srst(ff, m, autoidx);
    }
    let (a, b) = if ff.pol_ce {
        (ff.sig_q.clone(), ff.sig_d.clone())
    } else {
        (ff.sig_d.clone(), ff.sig_q.clone())
    };
    let line = if ff.pol_ce { 592 } else { 594 };
    let cid = new_id(autoidx, "ff.cc", line, "unmap_ce");
    let wid = new_id(autoidx, "rtlil.cc", 3337, "Mux");
    m.wires.push(Wire { name: wid.clone(), width: a.bits.len() as i64, ..Default::default() });
    let y = SigSpec { bits: vec![SigBit::Wire { wire: wid, offset: 0 }] };
    add_mux_to(m, cid, a, b, ff.sig_ce.clone(), y.clone());
    ff.sig_d = y;
    ff.has_ce = false;
}

/// FfData::unmap_srst(kernel/ff.cc:604;NEW_ID 錨=:612 pol_srst/:614 else;
/// has_ce && !ce_over_srst 時先 unmap_ce)。
pub(crate) fn unmap_srst(ff: &mut FfD, m: &mut Module, autoidx: &mut i64) {
    if !ff.has_srst {
        return;
    }
    if ff.has_ce && !ff.ce_over_srst {
        unmap_ce(ff, m, autoidx);
    }
    let val = SigSpec { bits: ff.val_srst.iter().map(|&b| SigBit::Const(b)).collect() };
    let (a, b) = if ff.pol_srst {
        (ff.sig_d.clone(), val)
    } else {
        (val, ff.sig_d.clone())
    };
    let line = if ff.pol_srst { 612 } else { 614 };
    let cid = new_id(autoidx, "ff.cc", line, "unmap_srst");
    let wid = new_id(autoidx, "rtlil.cc", 3337, "Mux");
    m.wires.push(Wire { name: wid.clone(), width: a.bits.len() as i64, ..Default::default() });
    let y = SigSpec { bits: vec![SigBit::Wire { wire: wid, offset: 0 }] };
    add_mux_to(m, cid, a, b, ff.sig_srst.clone(), y.clone());
    ff.sig_d = y;
    ff.has_srst = false;
}

fn module_cell_log_name(m: &Module, cell: &Cell) -> String {
    // log("%s.%s", module, cell):oracle 觀測形=模組名剝前綴反斜線
    // ("MyMem.$procdff$309");cell 名原樣。
    format!("{}.{}", m.name.trim_start_matches('\\'), cell.name)
}

/// 命令面(§11.3-2)。
pub fn run_async2sync(design: &mut Design, args: &[String], log: &mut crate::Log) -> Result<(), crate::proc::ProcError> {
    log.header("Executing ASYNC2SYNC pass.\n");
    // 參數面:async2sync.cc:96-104 argidx 迴圈僅 -nolower;已知旗標=Y2
    // 硬錯(§11.1-8;真實功能=跳過 chformal -lower,行為面歧);其餘
    // token=extra_args 面 → 未知旗標/selection 裸字=cmd_error 全形。
    let argidx = 1usize;
    while argidx < args.len() {
        if args[argidx] == "-nolower" {
            return Err(crate::proc::ProcError(format!(
                "async2sync: option `-nolower' is not implemented in ryosys (milestone Y2)"
            )));
        }
        break;
    }
    if argidx < args.len() {
        return Err(crate::proc::ProcError(
            crate::opt::cmd_error_pass(log, args, argidx, HELP_ASYNC2SYNC).0,
        ));
    }

    let mut have_check = false;
    for mi in (0..design.modules.len()).rev() {
        if module_is_boxed(&design.modules[mi]) {
            continue;
        }
        let mut m = std::mem::take(&mut design.modules[mi]);
        let mut autoidx = design.autoidx;
        let am = {
            let mut am = SigMap::new();
            am.set(&m);
            am
        };
        let mut iv = FfInitVals::set(&am, &m);
        let widths = widths_of(&m);

        // selected_cells 值拷貝=逆插入序(Q4;ci 逆向+emit swap_remove 不擾
        // 未處理原 cell 位置)。
        for ci in (0..m.cells.len()).rev() {
            let cell = m.cells[ci].clone();
            if cell.ctype == "$print" || cell.ctype == "$check" {
                // §11.2 邊界:TRG_ENABLE 變換面不做;false 形記 have_check
                // (尾部 chformal -lower=stub Y0 如實硬錯)。
                let trg_enable = matches!(
                    cell.parameters.get("\\TRG_ENABLE"),
                    Some(Const::Bits { bits, .. }) if bits.first() == Some(&Bit::S1)
                );
                if !trg_enable {
                    if cell.ctype == "$check" {
                        have_check = true;
                    }
                    continue;
                }
                design.autoidx = autoidx;
                design.modules[mi] = m;
                return Err(crate::proc::ProcError(String::from(
                    "async2sync: $check/$print TRG_ENABLE transform is not implemented in ryosys (milestone Y2)",
                )));
            }
            if !BUILTIN_FF.contains(&cell.ctype.as_str()) {
                continue;
            }
            if cell.ctype.starts_with("$_") {
                design.autoidx = autoidx;
                design.modules[mi] = m;
                return Err(crate::proc::ProcError(String::from(
                    "async2sync: fine-cell ($_DFF_ family) form is not implemented in ryosys (milestone Y2)",
                )));
            }
            let mut ff = ff_from_cell(&cell, &iv, &am);
            // Skip for $_FF_ and $ff cells。
            if ff.has_gclk {
                continue;
            }
            if ff.has_clk && is_fully_const(&ff.sig_clk) {
                ff.has_ce = false;
                ff.has_clk = false;
                ff.has_srst = false;
            }
            if ff.has_clk {
                if ff.has_sr {
                    unmap_ce(&mut ff, &mut m, &mut autoidx);
                    unmap_srst(&mut ff, &mut m, &mut autoidx);
                    log.log(&format!(
                        "Replacing {} ({}) : SET={}, CLR={}, D={}, Q={}\n",
                        module_cell_log_name_placeholder(&m, &cell),
                        cell.ctype,
                        crate::sigops::log_signal(&ff.sig_set, &widths),
                        crate::sigops::log_signal(&ff.sig_clr, &widths),
                        crate::sigops::log_signal(&ff.sig_d, &widths),
                        crate::sigops::log_signal(&ff.sig_q, &widths),
                    ));
                    iv.remove_init(&am, &mut m, &ff.sig_q);
                    let nd = new_id(&mut autoidx, "async2sync.cc", 155, "execute");
                    let nq = new_id(&mut autoidx, "async2sync.cc", 156, "execute");
                    m.wires.push(Wire {
                        name: nd.clone(),
                        width: ff.width,
                        ..Default::default()
                    });
                    m.wires.push(Wire {
                        name: nq.clone(),
                        width: ff.width,
                        ..Default::default()
                    });
                    let new_d = wire_sig(&nd, ff.width);
                    let new_q = wire_sig(&nq, ff.width);
                    let mut sig_set = ff.sig_set.clone();
                    let mut sig_clr = ff.sig_clr.clone();
                    let mut sig_clr_inv = ff.sig_clr.clone();
                    if !ff.pol_set {
                        sig_set = mk_not(&mut m, &mut autoidx, 164, &sig_set);
                    }
                    if ff.pol_clr {
                        sig_clr_inv = mk_not(&mut m, &mut autoidx, 171, &sig_clr);
                    } else {
                        sig_clr = mk_not(&mut m, &mut autoidx, 176, &sig_clr);
                    }
                    let set_and_clr =
                        mk_and_or(&mut m, &mut autoidx, 186, &sig_set, &sig_clr, false);
                    // d 臂:Or(d,set)→And(·,clr_inv)→Bwmux(·, Sx, sc, new_d)
                    let tmp1 = mk_and_or(&mut m, &mut autoidx, 191, &ff.sig_d, &sig_set, true);
                    let tmp2 = mk_and_or(&mut m, &mut autoidx, 192, &tmp1, &sig_clr_inv, false);
                    let c1 = new_id(&mut autoidx, "async2sync.cc", 193, "execute");
                    add_bwmux_to(
                        &mut m,
                        c1,
                        tmp2,
                        SigSpec { bits: vec![SigBit::Const(Bit::Sx); ff.width as usize] },
                        set_and_clr.clone(),
                        new_d.clone(),
                    );
                    // q 臂:Or(new_q,set)→And(·,clr_inv)→Bwmux(·, Sx, sc, q)
                    let tmp3 = mk_and_or(&mut m, &mut autoidx, 195, &new_q, &sig_set, true);
                    let tmp4 = mk_and_or(&mut m, &mut autoidx, 196, &tmp3, &sig_clr_inv, false);
                    let c2 = new_id(&mut autoidx, "async2sync.cc", 197, "execute");
                    let q_old = ff.sig_q.clone();
                    add_bwmux_to(
                        &mut m,
                        c2,
                        tmp4,
                        SigSpec { bits: vec![SigBit::Const(Bit::Sx); ff.width as usize] },
                        set_and_clr,
                        q_old,
                    );
                    ff.sig_d = new_d;
                    ff.sig_q = new_q;
                    ff.has_sr = false;
                } else if ff.has_aload {
                    unmap_ce(&mut ff, &mut m, &mut autoidx);
                    unmap_srst(&mut ff, &mut m, &mut autoidx);
                    log.log(&format!(
                        "Replacing {} ({}): ALOAD={}, AD={}, D={}, Q={}\n",
                        module_cell_log_name_placeholder(&m, &cell),
                        cell.ctype,
                        crate::sigops::log_signal(&ff.sig_aload, &widths),
                        crate::sigops::log_signal(&ff.sig_ad, &widths),
                        crate::sigops::log_signal(&ff.sig_d, &widths),
                        crate::sigops::log_signal(&ff.sig_q, &widths),
                    ));
                    iv.remove_init(&am, &mut m, &ff.sig_q);
                    let nd = new_id(&mut autoidx, "async2sync.cc", 220, "execute");
                    let nq = new_id(&mut autoidx, "async2sync.cc", 221, "execute");
                    m.wires.push(Wire {
                        name: nd.clone(),
                        width: ff.width,
                        ..Default::default()
                    });
                    m.wires.push(Wire {
                        name: nq.clone(),
                        width: ff.width,
                        ..Default::default()
                    });
                    let new_d = wire_sig(&nd, ff.width);
                    let new_q = wire_sig(&nq, ff.width);
                    let aload = ff.sig_aload.clone();
                    let ad = ff.sig_ad.clone();
                    let q_old = ff.sig_q.clone();
                    if ff.pol_aload {
                        let c1 = new_id(&mut autoidx, "async2sync.cc", 225, "execute");
                        add_mux_to(&mut m, c1, new_q.clone(), ad.clone(), aload.clone(), q_old);
                        let c2 = new_id(&mut autoidx, "async2sync.cc", 226, "execute");
                        add_mux_to(&mut m, c2, ff.sig_d.clone(), ad, aload, new_d.clone());
                    } else {
                        let c1 = new_id(&mut autoidx, "async2sync.cc", 233, "execute");
                        add_mux_to(&mut m, c1, ad.clone(), new_q.clone(), aload.clone(), q_old);
                        let c2 = new_id(&mut autoidx, "async2sync.cc", 234, "execute");
                        add_mux_to(&mut m, c2, ad, ff.sig_d.clone(), aload, new_d.clone());
                    }
                    ff.sig_d = new_d;
                    ff.sig_q = new_q;
                    ff.has_aload = false;
                } else if ff.has_arst {
                    unmap_srst(&mut ff, &mut m, &mut autoidx);
                    log.log(&format!(
                        "Replacing {} ({}): ARST={}, D={}, Q={}\n",
                        module_cell_log_name(&m, &cell),
                        cell.ctype,
                        crate::sigops::log_signal(&ff.sig_arst, &widths),
                        crate::sigops::log_signal(&ff.sig_d, &widths),
                        crate::sigops::log_signal(&ff.sig_q, &widths),
                    ));
                    iv.remove_init(&am, &mut m, &ff.sig_q);
                    let nq = new_id(&mut autoidx, "async2sync.cc", 253, "execute");
                    m.wires.push(Wire {
                        name: nq.clone(),
                        width: ff.width,
                        ..Default::default()
                    });
                    let new_q = wire_sig(&nq, ff.width);
                    let val_arst =
                        SigSpec { bits: ff.val_arst.iter().map(|&b| SigBit::Const(b)).collect() };
                    let arst = ff.sig_arst.clone();
                    let q_old = ff.sig_q.clone();
                    let line = if ff.pol_arst { 257 } else { 262 };
                    let cid = new_id(&mut autoidx, "async2sync.cc", line, "execute");
                    if ff.pol_arst {
                        add_mux_to(&mut m, cid, new_q.clone(), val_arst, arst, q_old);
                    } else {
                        add_mux_to(&mut m, cid, val_arst, new_q.clone(), arst, q_old);
                    }
                    ff.sig_q = new_q;
                    ff.has_arst = false;
                    ff.has_srst = true;
                    ff.ce_over_srst = false;
                    ff.val_srst = ff.val_arst.clone();
                    ff.sig_srst = ff.sig_arst.clone();
                    ff.pol_srst = ff.pol_arst;
                } else {
                    continue;
                }
            } else {
                // Latch(async2sync.cc:279)。
                log.log(&format!(
                    "Replacing {} ({}): EN={}, D={}, Q={}\n",
                    module_cell_log_name(&m, &cell),
                    cell.ctype,
                    crate::sigops::log_signal(&ff.sig_aload, &widths),
                    crate::sigops::log_signal(&ff.sig_ad, &widths),
                    crate::sigops::log_signal(&ff.sig_q, &widths),
                ));
                iv.remove_init(&am, &mut m, &ff.sig_q);
                let nq = new_id(&mut autoidx, "async2sync.cc", 285, "execute");
                m.wires.push(Wire {
                    name: nq.clone(),
                    width: ff.width,
                    ..Default::default()
                });
                let new_q = wire_sig(&nq, ff.width);
                let new_d = if ff.has_aload {
                    let nd = new_id(&mut autoidx, "async2sync.cc", 289, "execute");
                    m.wires.push(Wire {
                        name: nd.clone(),
                        width: ff.width,
                        ..Default::default()
                    });
                    let nd_sig = wire_sig(&nd, ff.width);
                    let aload = ff.sig_aload.clone();
                    let ad = ff.sig_ad.clone();
                    let line = if ff.pol_aload { 292 } else { 297 };
                    let cid = new_id(&mut autoidx, "async2sync.cc", line, "execute");
                    if ff.pol_aload {
                        add_mux_to(&mut m, cid, new_q.clone(), ad, aload, nd_sig.clone());
                    } else {
                        add_mux_to(&mut m, cid, ad, new_q.clone(), aload, nd_sig.clone());
                    }
                    nd_sig
                } else {
                    new_q.clone()
                };
                if ff.has_sr {
                    let mut sig_set = ff.sig_set.clone();
                    let mut sig_clr = ff.sig_clr.clone();
                    if !ff.pol_set {
                        sig_set = mk_not(&mut m, &mut autoidx, 305, &sig_set);
                    }
                    if ff.pol_clr {
                        sig_clr = mk_not(&mut m, &mut autoidx, 310, &sig_clr);
                    }
                    let tmp = mk_and_or(&mut m, &mut autoidx, 326, &new_d, &sig_set, true);
                    let c = new_id(&mut autoidx, "async2sync.cc", 327, "execute");
                    let q_old = ff.sig_q.clone();
                    m.cells.push(Cell {
                        ctype: "$and".to_string(),
                        name: c,
                        attributes: InsertMap::new(),
                        parameters: {
                            let mut im = InsertMap::new();
                            im.insert("\\A_SIGNED".into(), int_param(0));
                            im.insert("\\B_SIGNED".into(), int_param(0));
                            im.insert("\\A_WIDTH".into(), int_param(tmp.bits.len() as i64));
                            im.insert("\\B_WIDTH".into(), int_param(sig_clr.bits.len() as i64));
                            im.insert("\\Y_WIDTH".into(), int_param(q_old.bits.len() as i64));
                            im
                        },
                        connections: vec![
                            ("\\A".into(), tmp),
                            ("\\B".into(), sig_clr),
                            ("\\Y".into(), q_old),
                        ],
                    });
                } else if ff.has_arst {
                    let val_arst = SigSpec {
                        bits: ff.val_arst.iter().map(|&b| SigBit::Const(b)).collect(),
                    };
                    let arst = ff.sig_arst.clone();
                    let q_old = ff.sig_q.clone();
                    let line = if ff.pol_arst { 335 } else { 340 };
                    let cid = new_id(&mut autoidx, "async2sync.cc", line, "execute");
                    if ff.pol_arst {
                        add_mux_to(&mut m, cid, new_d.clone(), val_arst, arst, q_old);
                    } else {
                        add_mux_to(&mut m, cid, val_arst, new_d.clone(), arst, q_old);
                    }
                } else {
                    let q_old = ff.sig_q.clone();
                    crate::opt_dff::module_connect(&mut m, q_old, new_d.clone());
                }
                ff.sig_d = new_d;
                ff.sig_q = new_q;
                ff.has_aload = false;
                ff.has_arst = false;
                ff.has_sr = false;
                ff.has_gclk = true;
            }
            ff.emit(&mut m, &mut iv, &am);
        }
        design.autoidx = autoidx;
        design.modules[mi] = m;
    }
    if have_check {
        // async2sync.cc:340-345:Pass::call(design, "chformal -lower");
        // chformal=registry Y0 stub → 如實硬錯(能力邊界)。
        return Err(crate::proc::ProcError(String::from(
            "chformal: not implemented in ryosys (milestone Y0)",
        )));
    }
    Ok(())
}

/// sr/aload face 的 log 形為 `(%s)` 帶空格變體已勘正——oracle 實錄形:
/// `Replacing MyMem.$procdff$309 ($adff): ARST=…`(冒號前無空格);
/// 此函數僅為佔位防誤用,統一走 module_cell_log_name。
fn module_cell_log_name_placeholder(m: &Module, cell: &Cell) -> String {
    module_cell_log_name(m, cell)
}
