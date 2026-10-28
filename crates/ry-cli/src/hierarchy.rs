//! `hierarchy` pass M1 子集(passes/hierarchy/hierarchy.cc 移植面)。
//!
//! 覆蓋:`-auto-top`(屬性發現/自動評分兩徑)與 `-top <name>`;分析
//! 雙跑(while 輪一次+final clean 輪,即實測的 2.1/2.2 兩段);未用模組
//! 刪除;`\top` 屬性收斂(僅 top 保留,Const(1))。
//!
//! 實測釘死(2026-10-27,oracle 0.70+179 二進制):
//! * `\top` 已在:「Attribute `top' found on module `%s'. Setting top
//!   module to %s.」(兩處皆 unescape)→ 直接 2.1 Analyzing;
//! * 無 `\top` + -auto-top:2.1 Finding top(`root of %3d design levels:
//!   %-20s` 左對齊寬 20;`Automatically selected %s as design top
//!   module.`)→ 2.2 Analyzing;
//! * 僅含 blackbox 模組的設計:Finding top 段全靜默(selected_modules
//!   排除 boxed),無 Analyzing 無 Removed,exit 0;
//! * `-top` 未知名:`ERROR: Module `%s' not found!`(log_cmd_error)。
//!
//! 遍歷序:hierarchy_worker 按 cells 正向迭代(=dict 逆插入序=ry-ir
//! `.rev()`)DFS 先序,visited 集去重;`Top module:  %s`=存儲名(含
//! `\`/`$`);`Used module: %*s%s` 縮排=每層 +4。
//!
//! 未覆蓋(M1 邊界,靜默不觸發面):expand_module 的參數推導循環(.il
//! 語料的 cell 均已派生,實測 did_something=false 即單輪)、$abstract/
//! interface 重定頂、libdir 搜尋——未知旗標一律 milestone Y0 硬錯。

use crate::check::{is_boxed, module_is_top};
use crate::Log;
use ry_ir::{Const, Design, Module, SigSpec};



fn worker(design: &Design, used: &mut Vec<String>, m: &Module, indent: usize, log: &mut Log) {
    if used.iter().any(|n| n == &m.name) {
        return;
    }
    if indent == 0 {
        log.log(&format!("Top module:  {}\n", m.name));
    } else if !is_boxed(m) {
        log.log(&format!(
            "Used module: {}{}\n",
            " ".repeat(indent),
            m.name
        ));
    }
    used.push(m.name.clone());
    for cell in m.cells.iter().rev() {
        if let Some(sub) = design.find_module(&cell.ctype) {
            worker(design, used, sub, indent + 4, log);
        }
    }
}

fn find_top_mod_score(design: &Design, m: &Module, db: &mut Vec<(String, i64)>) -> i64 {
    if let Some((_, s)) = db.iter().find(|(n, _)| n == &m.name) {
        return *s;
    }
    db.push((m.name.clone(), 0)); // 環防護:先佔 0
    let mut score = 0i64;
    for cell in m.cells.iter().rev() {
        if let Some(sub) = design.find_module(&cell.ctype) {
            let s = find_top_mod_score(design, sub, db) + 1;
            if s > score {
                score = s;
            }
        }
    }
    if let Some(e) = db.iter_mut().find(|(n, _)| n == &m.name) {
        e.1 = score;
    }
    score
}

/// 執行 hierarchy(`\\top` 收斂寫回 design);`header` 回調由 CLI 層提供
/// (編號計數器在該層)。Err=硬錯訊息(-top 未知名等)。
/// set_keep_print:模組含 $print cell(直接或經子模組實例遞歸)。
fn has_print(design: &Design, m: &Module, seen: &mut Vec<String>) -> bool {
    if seen.iter().any(|n| n == &m.name) {
        return false;
    }
    seen.push(m.name.clone());
    for c in &m.cells {
        if c.ctype == "$print" {
            return true;
        }
        if let Some(sub) = design.find_module(&c.ctype) {
            if has_print(design, sub, seen) {
                return true;
            }
        }
    }
    false
}

pub fn run_hierarchy(
    design: &mut Design,
    auto_top: bool,
    top_arg: Option<&str>,
    log: &mut Log,
) -> Result<(), String> {
    // -top <name>(escape_id:非 `\`/`$` 開頭補 `\`)。
    let mut top_mod: Option<String> = None;
    if let Some(name) = top_arg {
        let stored = if name.starts_with('\\') || name.starts_with('$') {
            name.to_string()
        } else {
            format!("\\{name}")
        };
        match design.find_module(&stored) {
            Some(_) => top_mod = Some(stored),
            None => {
                return Err(format!("Module `{name}' not found!"));
            }
        }
    }

    // 屬性發現徑(design->modules() 正向=逆宣告序,首個帶 \top 者)。
    if top_mod.is_none() {
        for m in design.modules.iter().rev() {
            if module_is_top(m) {
                let un = crate::jsonout::unescape(&m.name);
                log.log(&format!(
                    "Attribute `top' found on module `{un}'. Setting top module to {un}.\n"
                ));
                top_mod = Some(m.name.clone());
                break;
            }
        }
    }

    // 自動評分徑(selected_modules 排除 boxed)。
    if top_mod.is_none() && auto_top {
        log.header("Finding top of design hierarchy..\n");
        let mut db: Vec<(String, i64)> = Vec::new();
        let order: Vec<&Module> = design.modules.iter().rev().filter(|m| !is_boxed(m)).collect();
        for m in &order {
            let score = find_top_mod_score(design, m, &mut db);
            let un = crate::jsonout::unescape(&m.name).to_string();
            let mut padded = un.clone();
            while padded.chars().count() < 20 {
                padded.push(' ');
            }
            log.log(&format!("root of {score:3} design levels: {padded}\n"));
            let better = match &top_mod {
                None => true,
                Some(t) => {
                    let prev = db
                        .iter()
                        .find(|(n, _)| n == t)
                        .map(|(_, s)| *s)
                        .unwrap_or(0);
                    score > prev
                }
            };
            if better {
                top_mod = Some(m.name.clone());
            }
        }
        if let Some(t) = &top_mod {
            log.log(&format!(
                "Automatically selected {} as design top module.\n",
                crate::jsonout::unescape(t)
            ));
        }
    }

    // 分析輪(while 語料形=單輪:expand 無事可做)+ final clean 輪。
    if let Some(top) = top_mod.clone() {
        log.header("Analyzing design hierarchy..\n");
        let mut used: Vec<String> = Vec::new();
        if let Some(m) = design.find_module(&top) {
            worker(design, &mut used, m, 0, log);
        }
        // (expand_module=.il 語料 no-op,見模組註釋;while 單輪。)

        log.header("Analyzing design hierarchy..\n");
        // hierarchy_clean:重走 used 集 + 刪未用非 boxed 模組。
        let mut used2: Vec<String> = Vec::new();
        if let Some(m) = design.find_module(&top) {
            worker(design, &mut used2, m, 0, log);
        }
        let mut del_counter = 0usize;
        let keep: Vec<Module> = design
            .modules
            .iter()
            .filter(|m| used2.iter().any(|n| n == &m.name) || is_boxed(m))
            .cloned()
            .collect();
        for m in &design.modules {
            if !used2.iter().any(|n| n == &m.name) && !is_boxed(m) {
                log.log(&format!("Removing unused module `{}`'.\n", m.name));
                del_counter += 1;
            }
        }
        log.log(&format!("Removed {del_counter} unused modules.\n"));
        design.modules = keep;

        // \top 收斂:僅 top 保留 Const(1)。dict insert=首值保留語義——
        // 已存在的 \top **原位改值不挪位**(oracle 實測 src,top,dynports
        // 順序保留;挪尾=param_attr/string_format 對拍實錄)。
        for m in &mut design.modules {
            let is_top = m.name == top;
            let mut had_top = false;
            for (k, v) in m.attributes.entries.iter_mut() {
                if k == "\\top" || k == "top" {
                    had_top = true;
                    if is_top {
                        *v = Const::from_i64_i32_width(1);
                    }
                }
            }
            m.attributes.entries.retain(|(k, _)| (k == "\\top" || k == "top") == (is_top && had_top) || (k != "\\top" && k != "top"));
            if is_top && !had_top {
                m.attributes
                    .insert("\\top".to_string(), Const::from_i64_i32_width(1));
            }
        }
    }

    // nokeep_prints 塊(hierarchy.cc 尾段):含 $print(直接或經子模組)
    // 的模組補 \keep 屬性+提示行。迭代=design->modules() 正向(逆宣告序)。
    let keep_flags: Vec<bool> = design
        .modules
        .iter()
        .map(|m| {
            let mut seen = Vec::new();
            has_print(design, m, &mut seen)
        })
        .collect();
    for (m, keep) in design.modules.iter_mut().zip(keep_flags) {
        if keep {
            log.log(&format!(
                "Module {} directly or indirectly displays text -> setting \"keep\" attribute.\n",
                crate::jsonout::unescape(&m.name)
            ));
            m.attributes
                .insert("\\keep".to_string(), Const::from_i64_i32_width(1));
        }
    }

    // wand/wor 多驅動解析(hierarchy.cc:1364-1481,design_modules 段;
    // 上游序在 keep_prints 之後)。實測釘死(2026-10-27 wandwor 第二遍
    // hierarchy + 寬 1 探針):
    // * 收集=帶 `\wand`/`\wor` 屬性的 wire;connections 掃描吸收(lhs
    //   chunk 引用者以 S1/S0 基準+replace(offset,rhs) 入 map;**空
    //   new_conn 也 push**——寬 1 探針實測殘留 `connect { } { }`);
    // * cells 輸出端口掃描(known=內建或設計內模組;output=port_dir 或
    //   子模組端口宣告)重定向到 t 線=`$auto$hierarchy.cc:1485:execute$N`
    //   (二進制實錄;clone :1437,漂移 +48 三錨一致);
    // * 重建:寬 1=addReduceAnd/Or(cell 名 `…hierarchy.cc:1515/1517:
    //   execute$N`;1517=寬 1 wor 探針實錄,1515=+48 線性推定,語料
    //   未觸發);寬 n>1=Module::And/Or 鏈(cell 名 `…hierarchy.cc:1524/
    //   1526:execute$N` 實錄,Y 線=`$auto$rtlil.cc:3249:And$N`/
    //   `:3250:Or$N` 實錄);單段僅 connect(w, s);
    // * 迭代序:模組=design->modules() 正向(逆宣告序);cells=正向
    //   (逆插入序,t 線分配序由此而定——wandwor 實測 $14 給 \Y、$15 給
    //   \X,恰=cells 逆插入序先訪 $and$12 後 $or$8);wand/wor wire 處理
    //   序=pool(逆插入)對 wires()(逆插入)的雙重翻轉=**wires 宣告
    //   正序**(wandwor 實測 connect \X 先於 connect \Y)。
    wand_wor_pass(design);

    let _ = top_mod;
    Ok(())
}

// Module::And/Or 的內部 Y 線名與 hierarchy 呼叫行 cell 名(二進制實錄)。
const HW_T_WIRE_LINE: u32 = 1485; // addWire(NEW_ID, GetSize(c))
const HW_WAND_REDUCE_LINE: u32 = 1515; // addReduceAnd(寬 1 wand;+48 推定)
const HW_WOR_REDUCE_LINE: u32 = 1517; // addReduceOr(寬 1 wor;探針實錄)
const HW_AND_CELL_LINE: u32 = 1524; // module->And 鏈
const HW_OR_CELL_LINE: u32 = 1526; // module->Or 鏈

fn hw_new_id(design: &mut Design, line: u32) -> String {
    let n = design.autoidx;
    design.autoidx += 1;
    format!("$auto$hierarchy.cc:{line}:execute${n}")
}

fn hw_wire_new_id(design: &mut Design, line: u32, func: &str) -> String {
    let n = design.autoidx;
    design.autoidx += 1;
    format!("$auto$rtlil.cc:{line}:{func}${n}")
}

/// SigSpec::replace(offset, with):把 with 覆蓋到位 offset 起。
fn sig_replace(base: &mut SigSpec, offset: i64, with: &SigSpec) {
    for (i, b) in with.bits.iter().enumerate() {
        let at = (offset as usize) + i;
        if at < base.bits.len() {
            base.bits[at] = b.clone();
        }
    }
}

fn const_sig(bit: ry_ir::Bit, width: i64) -> SigSpec {
    SigSpec {
        bits: vec![ry_ir::SigBit::Const(bit); width as usize],
    }
}

fn wire_sig(name: &str, width: i64) -> SigSpec {
    SigSpec {
        bits: (0..width)
            .map(|o| ry_ir::SigBit::Wire { wire: name.to_string(), offset: o })
            .collect(),
    }
}

/// wand/wor 多驅動解析(hierarchy.cc:1364-1481)。
fn wand_wor_pass(design: &mut Design) {
    use ry_ir::printer::sig_chunks;
    use ry_ir::printer::ChunkView;
    // design_modules 快照=design->modules() 正向=逆宣告序。
    let module_names: Vec<String> = design.modules.iter().rev().map(|m| m.name.clone()).collect();
    for mname in module_names {
        // 1) 收集 wand/wor wires(宣告正序;見模組註釋的雙重翻轉論證)。
        let idx: Vec<(String, bool)> = {
            let m = design.modules.iter().find(|m| m.name == mname).unwrap();
            m.wires
                .iter()
                .filter_map(|w| {
                    let wand = attr_bool(&w.attributes, "\\wand");
                    let wor = attr_bool(&w.attributes, "\\wor");
                    if wand || wor {
                        Some((w.name.clone(), wand))
                    } else {
                        None
                    }
                })
                .collect()
        };
        if idx.is_empty() {
            continue;
        }
        let mi = design.modules.iter().position(|m| m.name == mname).unwrap();

        // 2) connections 掃描吸收(空 new_conn 也 push)。
        let conns = std::mem::take(&mut design.modules[mi].connections);
        let mut new_connections: Vec<(SigSpec, SigSpec)> = Vec::new();
        // map: wire 名 -> (is_wand, 拼接的段)。
        let mut maps: Vec<(String, bool, SigSpec)> = idx
            .iter()
            .map(|(n, wand)| (n.clone(), *wand, SigSpec::default()))
            .collect();
        for (lhs, rhs) in conns {
            let mut new_lhs = SigSpec::default();
            let mut new_rhs = SigSpec::default();
            let mut cursor = 0i64;
            let chunks = sig_chunks(&lhs);
            for c in &chunks {
                let (width, is_wire) = match c {
                    ChunkView::Wire { width, .. } => (*width, true),
                    ChunkView::Const { len, .. } => (*len as i64, false),
                };
                if is_wire {
                    let (wire, offset) = match c {
                        ChunkView::Wire { wire, offset, .. } => (wire.clone(), *offset),
                        ChunkView::Const { .. } => unreachable!(),
                    };
                    let rhs_seg = rhs.extract(cursor, width);
                    match maps.iter().position(|(n, _, _)| *n == wire) {
                        Some(k) => {
                            let wand = maps[k].1;
                            let wwidth = design.modules[mi]
                                .wires
                                .iter()
                                .find(|w| w.name == wire)
                                .map(|w| w.width)
                                .unwrap_or(width);
                            let mut sig = const_sig(
                                if wand { ry_ir::Bit::S1 } else { ry_ir::Bit::S0 },
                                wwidth,
                            );
                            sig_replace(&mut sig, offset, &rhs_seg);
                            maps[k].2.bits.extend(sig.bits);
                        }
                        None => {
                            let seg = lhs.extract(cursor, width);
                            new_lhs.bits.extend(seg.bits);
                            new_rhs.bits.extend(rhs_seg.bits);
                        }
                    }
                } else {
                    let start = match c {
                        ChunkView::Const { start, .. } => *start as i64,
                        ChunkView::Wire { .. } => unreachable!(),
                    };
                    let seg = lhs.extract(start, width);
                    new_lhs.bits.extend(seg.bits);
                    new_rhs.bits.extend(rhs.extract(cursor, width).bits);
                }
                cursor += width;
            }
            new_connections.push((new_lhs, new_rhs));
        }
        design.modules[mi].connections = new_connections;

        // 3) cells 輸出端口掃描重定向(cells 正向=逆插入序)。
        let cell_names: Vec<String> = design.modules[mi].cells.iter().rev().map(|c| c.name.clone()).collect();
        for cname in cell_names {
            let ci = design.modules[mi].cells.iter().position(|c| c.name == cname).unwrap();
            let ctype = design.modules[mi].cells[ci].ctype.clone();
            let known = crate::cells::cell_known(&ctype)
                || design.find_module(&ctype).is_some();
            if !known {
                continue;
            }
            let ports: Vec<(String, SigSpec)> = design.modules[mi].cells[ci].connections.clone();
            for (port, sig) in ports {
                let is_out = if crate::cells::cell_known(&ctype) {
                    crate::cells::port_dir(&ctype, &port).1
                } else if let Some(sub) = design.find_module(&ctype) {
                    sub.find_wire(&port).map(|w| w.port_output).unwrap_or(false)
                } else {
                    false
                };
                if !is_out {
                    continue;
                }
                let mut new_sig = SigSpec::default();
                let mut update_port = false;
                let schunks = sig_chunks(&sig);
                for c in &schunks {
                    match c {
                        ChunkView::Wire { wire, offset, width } => {
                            let (wire, offset, width) = (wire.clone(), *offset, *width);
                            match maps.iter().position(|(n, _, _)| *n == wire) {
                                Some(k) => {
                                    let wand = maps[k].1;
                                    let tname = hw_new_id(design, HW_T_WIRE_LINE);
                                    design.modules[mi].wires.push(ry_ir::Wire {
                                        name: tname.clone(),
                                        width,
                                        ..Default::default()
                                    });
                                    let t = wire_sig(&tname, width);
                                    new_sig.bits.extend(t.bits.clone());
                                    update_port = true;
                                    let wwidth = design.modules[mi]
                                        .wires
                                        .iter()
                                        .find(|w| w.name == wire)
                                        .map(|w| w.width)
                                        .unwrap_or(width);
                                    let mut sig_seg = const_sig(
                                        if wand { ry_ir::Bit::S1 } else { ry_ir::Bit::S0 },
                                        wwidth,
                                    );
                                    sig_replace(&mut sig_seg, offset, &t);
                                    maps[k].2.bits.extend(sig_seg.bits);
                                }
                                None => {
                                    let seg = wire_chunk_bits(&design.modules[mi], &wire, offset, width);
                                    new_sig.bits.extend(seg);
                                }
                            }
                        }
                        ChunkView::Const { start, len } => {
                            new_sig.bits.extend(sig.extract(*start as i64, *len as i64).bits);
                        }
                    }
                }
                if update_port {
                    for (p, s) in design.modules[mi].cells[ci].connections.iter_mut() {
                        if p == &port {
                            *s = new_sig;
                            break;
                        }
                    }
                }
            }
        }

        // 4) 重建(wand/wor wire 宣告正序)。
        let maps = maps;
        for (wname, wand, sigs) in &maps {
            if sigs.size() == 0 {
                continue;
            }
            let wwidth = design.modules[mi]
                .wires
                .iter()
                .find(|w| &w.name == wname)
                .map(|w| w.width)
                .unwrap_or(0);
            let w_sig = wire_sig(wname, wwidth);
            if wwidth == 1 {
                // addReduceAnd/Or(NEW_ID, sigs, w)。
                let cname = hw_new_id(
                    design,
                    if *wand { HW_WAND_REDUCE_LINE } else { HW_WOR_REDUCE_LINE },
                );
                let mut parameters = ry_ir::InsertMap::new();
                parameters.insert("\\A_SIGNED".to_string(), Const::from_i64_i32_width(0));
                parameters.insert("\\A_WIDTH".to_string(), Const::from_i64_i32_width(sigs.size()));
                parameters.insert("\\Y_WIDTH".to_string(), Const::from_i64_i32_width(1));
                design.modules[mi].cells.push(ry_ir::Cell {
                    ctype: if *wand { "$reduce_and".into() } else { "$reduce_or".into() },
                    name: cname,
                    attributes: ry_ir::InsertMap::new(),
                    parameters,
                    connections: vec![
                        ("\\A".to_string(), sigs.clone()),
                        ("\\Y".to_string(), w_sig),
                    ],
                });
                continue;
            }
            let mut s = sigs.extract(0, wwidth);
            let mut i = wwidth;
            while i < sigs.size() {
                let seg = sigs.extract(i, wwidth);
                let yname = hw_wire_new_id(
                    design,
                    if *wand { 3249 } else { 3250 },
                    if *wand { "And" } else { "Or" },
                );
                design.modules[mi].wires.push(ry_ir::Wire {
                    name: yname.clone(),
                    width: s.size().max(seg.size()),
                    ..Default::default()
                });
                let y = wire_sig(&yname, s.size().max(seg.size()));
                let cname = hw_new_id(
                    design,
                    if *wand { HW_AND_CELL_LINE } else { HW_OR_CELL_LINE },
                );
                let mut parameters = ry_ir::InsertMap::new();
                parameters.insert("\\A_SIGNED".to_string(), Const::from_i64_i32_width(0));
                parameters.insert("\\B_SIGNED".to_string(), Const::from_i64_i32_width(0));
                parameters.insert("\\A_WIDTH".to_string(), Const::from_i64_i32_width(s.size()));
                parameters.insert("\\B_WIDTH".to_string(), Const::from_i64_i32_width(seg.size()));
                parameters.insert("\\Y_WIDTH".to_string(), Const::from_i64_i32_width(y.size()));
                design.modules[mi].cells.push(ry_ir::Cell {
                    ctype: if *wand { "$and".into() } else { "$or".into() },
                    name: cname,
                    attributes: ry_ir::InsertMap::new(),
                    parameters,
                    connections: vec![
                        ("\\A".to_string(), s.clone()),
                        ("\\B".to_string(), seg.clone()),
                        ("\\Y".to_string(), y.clone()),
                    ],
                });
                s = y;
                i += wwidth;
            }
            design.modules[mi].connections.push((w_sig, s));
        }
    }
}

fn attr_bool(attrs: &ry_ir::InsertMap<Const>, name: &str) -> bool {
    match attrs.get(name) {
        Some(Const::Bits { bits, .. }) => bits.first() == Some(&ry_ir::Bit::S1),
        Some(Const::Str(s)) => !s.is_empty(),
        None => false,
    }
}

/// 從 module 的 wires 取 chunk 的位(wire 寬度可能小於 offset+width?
/// 正常不會;防禦性飽和)。
fn wire_chunk_bits(m: &ry_ir::Module, wire: &str, offset: i64, width: i64) -> Vec<ry_ir::SigBit> {
    let _ = m;
    (0..width)
        .map(|i| ry_ir::SigBit::Wire { wire: wire.to_string(), offset: offset + i })
        .collect()
}
