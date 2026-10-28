//! `check` pass M0 子集(passes/cmds/check.cc 的移植面)。
//!
//! 覆蓋:未知內部類型警告、常數驅動衝突、多重驅動、used-but-no-driver、
//! 逐模組 `Checking module %s...` 行與 `Found and reported %d problems.`
//! 收尾。組合環檢測(celledges+toposort)=M1 深化範圍(RYOSYS-MVP §3)。
//!
//! 迭代序契約(hashlib:dict/pool 正向迭代=逆插入序):
//! * 模組序=design->modules() 正向=**逆 .il 宣告序**(實測 hierdefparam
//!   迭代序 a,b1010,b1011,top vs 檔序 top,b1011,b1010,a);
//! * cells/wires/processes 同理(ry-ir Vec 存宣告序,取 .rev());
//! * 多重驅動警告按 drivers dict 逆插入序;used 警告按 pool 逆插入序。
//!
//! 物件印名=log_id=unescape(`hierdefparam_top`);log_signal 位元=
//! escape(`\out6 [3]`,實測 §0-9 引文)。

use crate::cells;
use crate::sigmap::SigMap;
use ry_ir::{Bit, Const, Design, Module, SigBit, SigSpec, SyncType};

fn log_signal_bit(module: &Module, bit: &SigBit) -> String {
    match bit {
        // log_signal(SigBit)→dump_sigchunk:寬 1 chunk 一律 `name [offset]`
        // (offset 0 亦然),**裸名僅當 wire 寬度==1**(chunk 寬==wire 寬
        // && offset 0;func_block golden `\out3 [0]` vs loopsrc `wire \y`
        // 釘死,2026-10-27 實錄:舊「offset 0=裸名」規則在寬 32 的
        // \out3 [0] 上誤印裸名)。
        SigBit::Wire { wire, offset } => {
            let w1 = module.find_wire(wire).map(|w| w.width == 1).unwrap_or(false);
            if w1 && *offset == 0 {
                wire.clone()
            } else {
                format!("{wire} [{offset}]")
            }
        }
        SigBit::Const(b) => format!("1'{}", b.as_char()),
    }
}

fn is_true(c: &Const) -> bool {
    matches!(c, Const::Bits { width: 32, bits, .. } if bits.iter().take(1).any(|b| *b == Bit::S1))
}

pub fn run_check(design: &Design, log: &mut String, warnings: &mut Vec<String>) {
    let mut counter = 0usize;
    // selected_whole_modules_warn():正向迭代 + **靜默跳過 boxed 模組**
    // (oracle 實測 ifdef_1/specify:僅含 \blackbox 模組的設計既無
    // `Checking module` 行也無 `Ignoring boxed module` 警告)。
    for module in design.modules.iter().rev().filter(|m| !is_boxed(m)) {
        log.push_str(&format!("Checking module {}...\n", crate::jsonout::unescape(&module.name)));
        counter += check_module(module, design, log, warnings);
    }
    log.push_str(&format!("Found and reported {counter} problems.\n"));
}

/// boxed=\blackbox 或 \whitebox 屬性為真(get_blackbox_attribute 語義;
/// 語料 whitebox 普查=0 例)。
pub fn is_boxed(module: &Module) -> bool {
    for key in ["\\blackbox", "\\whitebox"] {
        if let Some(c) = module.attributes.get(key) {
            if is_true(c) {
                return true;
            }
        }
    }
    false
}

fn check_module(module: &Module, design: &Design, log: &mut String, warnings: &mut Vec<String>) -> usize {
    let mut counter = 0usize;
    let mut sigmap = SigMap::new();
    sigmap.set(module);

    // drivers:dict<SigBit, Vec<String>> —— 保留插入序,消費端走 .rev()。
    let mut wire_drivers: Vec<(SigBit, Vec<String>)> = Vec::new();
    let drivers_push = |bit: &SigBit, desc: String, wd: &mut Vec<(SigBit, Vec<String>)>| {
        if let Some(e) = wd.iter_mut().find(|(b, _)| b == bit) {
            e.1.push(desc);
        } else {
            wd.push((bit.clone(), vec![desc]));
        }
    };
    let mut wire_drivers_count: Vec<(SigBit, usize)> = Vec::new();
    let count_of = |bit: &SigBit, wd: &Vec<(SigBit, usize)>| -> usize {
        wd.iter().find(|(b, _)| b == bit).map(|(_, c)| *c).unwrap_or(0)
    };
    let bump_count = |bit: &SigBit, wd: &mut Vec<(SigBit, usize)>| {
        if let Some(e) = wd.iter_mut().find(|(b, _)| b == bit) {
            e.1 += 1;
        } else {
            wd.push((bit.clone(), 1));
        }
    };
    let mut used_wires: Vec<SigBit> = Vec::new();
    let used_insert = |bit: &SigBit, used: &mut Vec<SigBit>| {
        if !used.iter().any(|b| b == bit) {
            used.push(bit.clone());
        }
    };

    // process 區塊:action/sync 的驅動與使用歸帳(check.cc:142-205)。
    // case 樹遍歷=BFS 向量序(root actions → 其 switch cases 依序入隊…)。
    for proc in module.processes.iter().rev() {
        let mut proc_driven_bits: Vec<SigBit> = Vec::new();
        let mut all_actions: Vec<(SigSpec, SigSpec, &'static str)> = Vec::new();
        let mut queue: Vec<&ry_ir::CaseRule> = vec![&proc.root_case];
        let mut qi = 0usize;
        while qi < queue.len() {
            let case = queue[qi];
            qi += 1;
            for (lhs, rhs) in &case.actions {
                all_actions.push((lhs.clone(), rhs.clone(), "case rule"));
            }
            for sw in &case.switches {
                for c in &sw.cases {
                    queue.push(c);
                    for cmp in &c.compare {
                        for bit in sigmap.map(cmp).bits {
                            if matches!(bit, SigBit::Wire { .. }) {
                                used_insert(&bit, &mut used_wires);
                            }
                        }
                    }
                }
            }
        }
        // sync 段:訊號/記憶體寫的使用歸帳 + action 收集(STi 的驅動不計數)。
        let mut sti_actions: Vec<usize> = Vec::new();
        for sync in &proc.syncs {
            if let Some(sig) = &sync.signal {
                for bit in sigmap.map(sig).bits {
                    if matches!(bit, SigBit::Wire { .. }) {
                        used_insert(&bit, &mut used_wires);
                    }
                }
            }
            for (lhs, rhs) in &sync.actions {
                all_actions.push((lhs.clone(), rhs.clone(), "sync rule"));
                if sync.stype == SyncType::Init {
                    sti_actions.push(all_actions.len() - 1);
                }
            }
            for mw in &sync.mem_writes {
                for sig in [&mw.address, &mw.data, &mw.enable] {
                    for bit in sigmap.map(sig).bits {
                        if matches!(bit, SigBit::Wire { .. }) {
                            used_insert(&bit, &mut used_wires);
                        }
                    }
                }
            }
        }
        let proc_name = &proc.name;
        for (ai, (lhs, rhs, kind)) in all_actions.iter().enumerate() {
            let count_driver = !sti_actions.contains(&ai);
            for bit in sigmap.map(lhs).bits {
                drivers_push(
                    &bit,
                    format!(
                        "action {} <= {} ({}) in process {}",
                        log_signal_spec(lhs),
                        log_signal_spec(rhs),
                        kind,
                        crate::jsonout::unescape(proc_name)
                    ),
                    &mut wire_drivers,
                );
                if matches!(bit, SigBit::Wire { .. }) && count_driver {
                    if !proc_driven_bits.iter().any(|b| b == &bit) {
                        proc_driven_bits.push(bit.clone());
                    }
                }
            }
            for bit in sigmap.map(rhs).bits {
                if matches!(bit, SigBit::Wire { .. }) {
                    used_insert(&bit, &mut used_wires);
                }
            }
        }
        for bit in proc_driven_bits {
            bump_count(&bit, &mut wire_drivers_count);
        }
    }

    // M1-c:celledges+toposort 組合環偵測的邊資料庫與驅動表。
    // node=(wire_name, offset);輔助節點=(cell_name, -1)(edges fallback)。
    let mut topo: Vec<((String, i64), usize)> = Vec::new(); // node -> idx(插入序)
    let mut topo_edges: Vec<Vec<usize>> = Vec::new(); // edges[r] = preds of r(節點值序)
    let mut driver_cells: Vec<(SigBit, String)> = Vec::new(); // 位 -> 驅動 cell 名(後者覆蓋)
    let mut coarsened: Vec<String> = Vec::new(); // fallback cell 名集

    // cells:未知內部類型警告 + 端口位歸帳。
    for cell in module.cells.iter().rev() {
        let unknown_internal = cell.ctype.starts_with('$')
            && !cells::cell_known(&cell.ctype)
            && design.find_module(&cell.ctype).is_none();
        if unknown_internal {
            let msg = format!(
                "Cell {}.{} has unknown internal type {}.\n",
                crate::jsonout::unescape(&module.name),
                crate::jsonout::unescape(&cell.name),
                crate::jsonout::unescape(&cell.ctype)
            );
            log.push_str(&format!("Warning: {msg}"));
            warnings.push(msg);
            counter += 1;
        }
        let known = cells::cell_known(&cell.ctype);
        for (port, sig) in &cell.connections {
            // Cell::input/output 語義(kernel/rtlil.cc:4354):known cell
            // 查方向表;**未知類型回落到設計內模組的端口宣告**——模組
            // 實例的驅動/使用歸帳靠這條(arraycells 例 \y 由 aoi12 實例
            // 的 \y 端口驅動,實測 oracle 報 0 警告)。
            let (input, output) = if known {
                cells::port_dir(&cell.ctype, port)
            } else if let Some(sub) = design.find_module(&cell.ctype) {
                match sub.find_wire(port) {
                    Some(w) => (w.port_input, w.port_output),
                    None => (false, false),
                }
            } else {
                (false, false)
            };
            for (i, bit) in sigmap.map(sig).bits.iter().enumerate() {
                if input && matches!(bit, SigBit::Wire { .. }) {
                    used_insert(bit, &mut used_wires);
                }
                if output && !input && matches!(bit, SigBit::Wire { .. }) {
                    bump_count(bit, &mut wire_drivers_count);
                }
                if output && (matches!(bit, SigBit::Wire { .. }) || !input) {
                    drivers_push(
                        bit,
                        format!(
                            "port {}[{}] of cell {} ({})",
                            crate::jsonout::unescape(port),
                            i,
                            crate::jsonout::unescape(&cell.name),
                            crate::jsonout::unescape(&cell.ctype)
                        ),
                        &mut wire_drivers,
                    );
                }
                if output && matches!(bit, SigBit::Wire { .. }) {
                    // driver_cells[bit]=cell(後掃描覆蓋;oracle 同語義)。
                    if let Some(e) = driver_cells.iter_mut().find(|(b, _)| b == bit) {
                        e.1 = cell.name.clone();
                    } else {
                        driver_cells.push((bit.clone(), cell.name.clone()));
                    }
                }
            }
        }

        // 環邊(check.cc:352 參與條件+celledges.cc dispatch)。參與條件=
        // cell_evaluable || $mem_v2/$memrd/$memrd_v2/$barrier || is_builtin_ff
        // ——**不滿足的 cell(未知/模組實例/$memwr/$assert…)完全不建邊**。
        // 實錄(2026-10-27):$sdff 誤走 fallback 全連通把 next-state 回授
        // 誤判成組合環,16 案例×3 層 stdout 紅;邊緣觸發 ff(ff_op)不建
        // D→Q 銳斷環——oracle 實測 always01 報 0 problems。
        let participates = cells::cell_evaluable(&cell.ctype)
            || matches!(
                cell.ctype.as_str(),
                "$mem_v2" | "$memrd" | "$memrd_v2" | "$barrier"
            )
            || cells::cell_is_builtin_ff(&cell.ctype);
        if participates {
            // check.cc 覆寫版:detail_costly 閘先行(寬度和/積 ≥1024 的
            // $add/$sub/shift/$pmux/$bmux 走 fallback 防邊爆炸)。
            let costly = detail_costly(cell);
            let precise = !costly
                && add_edges_from_cell(cell, &sigmap, &mut topo, &mut topo_edges);
            if !precise {
                // fallback:全輸入→(cell,-1)→全輸出(check.cc:246-266);
                // 參與條件保證 known(cell_evaluable/is_ff/mem/barrier 均內建),
                // 方向表必命中。
                let helper = (cell.name.clone(), -1i64);
                let hidx = topo_node(&mut topo, &helper);
                for (port, sig) in &cell.connections {
                    let (input, output) = cells::port_dir(&cell.ctype, port);
                    let mapped = sigmap.map(sig);
                    if input {
                        for bit in &mapped.bits {
                            if let SigBit::Wire { .. } = bit {
                                let l = topo_node(&mut topo, &bit_key(bit));
                                topo_push_edge(&mut topo_edges, l, hidx);
                            }
                        }
                    }
                    if output {
                        for bit in &mapped.bits {
                            if let SigBit::Wire { .. } = bit {
                                let r = topo_node(&mut topo, &bit_key(bit));
                                topo_push_edge(&mut topo_edges, hidx, r);
                            }
                        }
                    }
                }
                coarsened.push(cell.name.clone());
            }
        }
    }

    // wires:模組輸入=驅動、輸出=使用;in-only 輸入計數。
    for wire in module.wires.iter().rev() {
        let sig = ry_ir::SigSpec {
            bits: (0..wire.width)
                .map(|i| SigBit::Wire { wire: wire.name.clone(), offset: i })
                .collect(),
        };
        let mapped = sigmap.map(&sig);
        if wire.port_input {
            for (i, bit) in mapped.bits.iter().enumerate() {
                if matches!(bit, SigBit::Wire { .. }) || !wire.port_output {
                    drivers_push(
                        bit,
                        format!("module input {}[{}]", wire.name, i),
                        &mut wire_drivers,
                    );
                }
            }
        }
        if wire.port_output {
            for bit in &mapped.bits {
                if matches!(bit, SigBit::Wire { .. }) {
                    used_insert(bit, &mut used_wires);
                }
            }
        }
        if wire.port_input && !wire.port_output {
            for bit in &mapped.bits {
                if matches!(bit, SigBit::Wire { .. }) {
                    bump_count(bit, &mut wire_drivers_count);
                }
            }
        }
    }

    // 常數驅動衝突(S0, S1, Sx 序)。
    for state in [Bit::S0, Bit::S1, Bit::Sx] {
        let key = SigBit::Const(state);
        if let Some((_, drvs)) = wire_drivers.iter().find(|(b, _)| *b == key) {
            let mut msg = format!(
                "Drivers conflicting with a constant {} driver:\n",
                log_signal_bit(module, &key)
            );
            for s in drvs {
                msg.push_str(&format!("    {s}\n"));
            }
            log.push_str(&format!("Warning: {msg}"));
            warnings.push(msg);
            counter += 1;
        }
    }

    // 多重驅動:dict 正向迭代=逆插入序。
    for (bit, drvs) in wire_drivers.iter().rev() {
        if count_of(bit, &wire_drivers_count) > 1 {
            let mut msg = format!(
                "multiple conflicting drivers for {}:{}:\n",
                crate::jsonout::unescape(&module.name),
                log_signal_bit(module, bit)
            );
            for s in drvs {
                msg.push_str(&format!("    {s}\n"));
            }
            log.push_str(&format!("Warning: {msg}"));
            warnings.push(msg);
            counter += 1;
        }
    }

    // used-but-no-driver:pool 正向迭代=逆插入序。
    for bit in used_wires.iter().rev() {
        if !wire_drivers.iter().any(|(b, _)| b == bit) {
            let msg = format!(
                "Wire {}.{} is used but has no driver.\n",
                crate::jsonout::unescape(&module.name),
                log_signal_bit(module, bit)
            );
            log.push_str(&format!("Warning: {msg}"));
            warnings.push(msg);
            counter += 1;
        }
    }

    // 組合環(topo.sort()+MatchingEdgePrinter)。**邊界(D-ys5)**:環
    // 起點旋轉依 oracle 內部序(interning/hash;8 探針實證,{x,y} 恆 x
    // 起、{aa,zz} 隨朝向翻轉),非 clone 源碼可推導——內容(cell/edge/
    // wire/source 行)逐字對,旋轉可能不同;環 fixture 因此不入
    // SUMMARY 語料(防以非 byte-exact golden 假綠),語料無環=零輸出。
    let mut topo_edges = topo_edges;
    for loop_nodes in topo_sort_loops(&topo, &mut topo_edges) {
        let mut msg = format!(
            "found logic loop in module {}:\n",
            crate::jsonout::unescape(&module.name)
        );
        // prev = 環尾最後一個非輔助節點。
        let mut prev: Option<(String, i64)> = None;
        for k in loop_nodes.iter().rev() {
            if k.1 != -1 {
                prev = Some(k.clone());
                break;
            }
        }
        let mut prev_bit: Option<SigBit> = prev.and_then(|k| {
            module
                .find_wire(&k.0)
                .map(|_| SigBit::Wire { wire: k.0.clone(), offset: k.1 })
        });
        for key in &loop_nodes {
            if key.1 == -1 {
                continue;
            }
            let Some(wire) = module.find_wire(&key.0) else { continue };
            let bit = SigBit::Wire { wire: key.0.clone(), offset: key.1 };
            let driver = driver_cells
                .iter()
                .find(|(b, _)| b == &bit)
                .map(|(_, n)| n.clone());
            let Some(dname) = driver else { continue };
            let dcell = module.find_cell(&dname);
            let driver_src = dcell
                .and_then(|c| c.attributes.get("\\src"))
                .and_then(|c| match c {
                    Const::Str(s) => Some(s.clone()),
                    _ => None,
                })
                .map(|s| format!(" source: {s}"))
                .unwrap_or_default();
            msg.push_str(&format!(
                "    cell {} ({}){}\n",
                crate::jsonout::unescape(&dname),
                dcell.map(|c| crate::jsonout::unescape(&c.ctype).to_string()).unwrap_or_default(),
                driver_src
            ));
            let coarsened_hit = coarsened.iter().any(|c| *c == dname);
            if !coarsened_hit {
                // MatchingEdgePrinter:prev→bit 的匹配邊(≤3 條+「…」)。
                if let (Some(pb), Some(dc)) = (&prev_bit, dcell) {
                    let mut nhits = 0;
                    for (fp, tp) in cell_edge_pairs(module, &sigmap, dc) {
                        let (fb, tb) = (fp.1.clone(), tp.1.clone());
                        if sig_bit_eq(&fb, pb) && sig_bit_eq(&tb, &bit) && nhits < 3 {
                            msg.push_str(&format!(
                                "      {}[{}] --> {}[{}]\n",
                                crate::jsonout::unescape(&fp.0),
                                fp.2,
                                crate::jsonout::unescape(&tp.0),
                                tp.2
                            ));
                            nhits += 1;
                        }
                    }
                    if nhits >= 3 {
                        msg.push_str("      ...\n");
                    }
                }
            } else {
                msg.push_str(
                    "      (cell's internal connectivity overapproximated; loop may be a false positive)\n",
                );
            }
            if !wire.name.starts_with('$') {
                let wire_src = wire
                    .attributes
                    .get("\\src")
                    .and_then(|c| match c {
                        Const::Str(s) => Some(s.clone()),
                        _ => None,
                    })
                    .map(|s| format!(" source: {s}"))
                    .unwrap_or_default();
                msg.push_str(&format!(
                    "    wire {}{}\n",
                    log_signal_bit(module, &bit),
                    wire_src
                ));
            }
            prev_bit = Some(bit);
        }
        log.push_str(&format!("Warning: {msg}"));
        warnings.push(msg);
        counter += 1;
    }

    counter
}

// ---------------------------------------------------------------------------
// celledges.cc 的 add_edges_from_cell dispatch(M1-c;kernel/celledges.cc
// 逐分支移植——位語義按埠寬度與參數,順序無可觀測語義)。
// 返回 Some(邊集)=dispatch 認識(精確邊);None=不認識(check.cc 覆寫
// 版走 fallback 全連通+coarsened)。
// ---------------------------------------------------------------------------

fn port_spec<'a>(cell: &'a ry_ir::Cell, name: &str) -> Option<&'a SigSpec> {
    cell.connections
        .iter()
        .find(|(p, _)| crate::jsonout::unescape(p) == name)
        .map(|(_, s)| s)
}

fn param_bool(cell: &ry_ir::Cell, name: &str) -> bool {
    match cell.parameters.get(name) {
        Some(Const::Bits { bits, .. }) => bits.iter().any(|b| *b == Bit::S1),
        _ => false,
    }
}

fn param_int(cell: &ry_ir::Cell, name: &str) -> i64 {
    match cell.parameters.get(name) {
        // as_int():非定值位按 0 折疊(語料參數全定值;防禦形)。
        Some(Const::Bits { bits, .. }) => {
            let mut v = 0i64;
            for (i, b) in bits.iter().enumerate().take(63) {
                if *b == Bit::S1 {
                    v |= 1 << i;
                }
            }
            v
        }
        _ => 0,
    }
}

fn param_bit(cell: &ry_ir::Cell, name: &str, idx: usize) -> Bit {
    match cell.parameters.get(name) {
        Some(Const::Bits { bits, .. }) => bits.get(idx).copied().unwrap_or(Bit::S0),
        _ => Bit::S0,
    }
}

type Edge = ((String, usize), (String, usize));

fn cell_edges_dispatch(cell: &ry_ir::Cell) -> Option<Vec<Edge>> {
    let t = cell.ctype.as_str();
    let w = |n: &str| port_spec(cell, n).map(|s| s.bits.len()).unwrap_or(0);
    let mut e: Vec<Edge> = Vec::new();
    let push = |fp: &str, fi: usize, tp: &str, ti: usize, e: &mut Vec<Edge>| {
        e.push(((fp.to_string(), fi), (tp.to_string(), ti)));
    };

    match t {
        // bitwise_unary_op($not/$pos/$buf/$barrier)。
        "$not" | "$pos" | "$buf" | "$barrier" => {
            let signed =
                !matches!(t, "$buf" | "$barrier") && param_bool(cell, "\\A_SIGNED");
            let (aw, yw) = (w("A"), w("Y"));
            for i in 0..yw {
                if i < aw {
                    push("A", i, "Y", i, &mut e);
                } else if signed && aw > 0 {
                    push("A", aw - 1, "Y", i, &mut e);
                }
            }
        }
        // bitwise_binary_op。
        "$and" | "$or" | "$xor" | "$xnor" => {
            let signed = param_bool(cell, "\\A_SIGNED");
            let (mut aw, mut bw) = (w("A"), w("B"));
            let yw = w("Y");
            if t == "$and" && !signed {
                let m = aw.min(bw);
                aw = m;
                bw = m;
            }
            for i in 0..yw {
                if i < aw {
                    push("A", i, "Y", i, &mut e);
                } else if signed && aw > 0 {
                    push("A", aw - 1, "Y", i, &mut e);
                }
                if i < bw {
                    push("B", i, "Y", i, &mut e);
                } else if signed && bw > 0 {
                    push("B", bw - 1, "Y", i, &mut e);
                }
            }
        }
        // arith_neg_op。
        "$neg" => {
            let signed = param_bool(cell, "\\A_SIGNED");
            let (aw, mut yw) = (w("A"), w("Y"));
            if signed && aw == 1 {
                yw = yw.min(1);
            }
            for i in 0..yw {
                for k in 0..=i {
                    if k < aw {
                        push("A", k, "Y", i, &mut e);
                    }
                }
            }
        }
        // arith_binary_op。
        "$add" | "$sub" => {
            let signed = param_bool(cell, "\\A_SIGNED");
            let (aw, bw) = (w("A"), w("B"));
            let mut yw = w("Y");
            if !signed && t != "$sub" {
                let ab = aw.max(bw);
                yw = yw.min(ab + 1);
            }
            for i in 0..yw {
                for k in 0..=i {
                    if k < aw {
                        push("A", k, "Y", i, &mut e);
                    }
                    if k < bw {
                        push("B", k, "Y", i, &mut e);
                    }
                }
            }
        }
        // reduce_op / logic_op。
        "$reduce_and" | "$reduce_or" | "$reduce_xor" | "$reduce_xnor"
        | "$reduce_bool" | "$logic_not" | "$logic_and" | "$logic_or" => {
            let aw = w("A");
            let bw = w("B");
            for i in 0..aw {
                push("A", i, "Y", 0, &mut e);
            }
            if matches!(t, "$logic_and" | "$logic_or") {
                for i in 0..bw {
                    push("B", i, "Y", 0, &mut e);
                }
            }
        }
        // slice_op。
        "$slice" => {
            let offset = param_int(cell, "\\OFFSET");
            let aw = w("A");
            let yw = w("Y");
            for i in 0..yw {
                let abit = offset + i as i64;
                if (0..aw as i64).contains(&abit) {
                    push("A", abit as usize, "Y", i, &mut e);
                }
            }
        }
        // concat_op。
        "$concat" => {
            let aw = w("A");
            let bw = w("B");
            for i in 0..aw {
                push("A", i, "Y", i, &mut e);
            }
            for i in 0..bw {
                push("B", i, "Y", aw + i, &mut e);
            }
        }
        // compare_op。
        "$lt" | "$le" | "$eq" | "$ne" | "$eqx" | "$nex" | "$ge" | "$gt" => {
            let aw = w("A");
            let bw = w("B");
            for i in 0..aw {
                push("A", i, "Y", 0, &mut e);
            }
            for i in 0..bw {
                push("B", i, "Y", 0, &mut e);
            }
        }
        // shift_op(celledges.cc:245-340 逐行;寬度推理純整數算術)。
        "$shl" | "$shr" | "$sshl" | "$sshr" | "$shift" | "$shiftx" => {
            let is_signed = param_bool(cell, "\\A_SIGNED");
            let is_b_signed = param_bool(cell, "\\B_SIGNED");
            let (aw, bw, yw) = (w("A"), w("B"), w("Y"));
            let b_cap = bw.min(30);
            let (b_high, b_low): (i64, i64) = if b_cap == 0 {
                (0, 0)
            } else if !is_b_signed {
                ((1i64 << b_cap) - 1, 0)
            } else {
                ((1i64 << (b_cap - 1)) - 1, -(1i64 << (b_cap - 1)))
            };
            let awi = aw as i64;
            let yi = yw as i64;
            for i in 0..yw as i64 {
                // Y[i] 的 A/B 影響範圍。
                let b_range_upper;
                let mut a_range_lower;
                let a_range_upper;
                if matches!(t, "$shl" | "$sshl") {
                    b_range_upper = awi + b_high - if is_signed { 1 } else { 0 };
                    a_range_lower = (i - b_high).max(0);
                    a_range_upper = (i + 1).min(awi);
                } else if matches!(t, "$shr" | "$sshr")
                    || (matches!(t, "$shift" | "$shiftx") && !is_b_signed)
                {
                    b_range_upper = awi;
                    a_range_lower = i.min(awi - 1);
                    a_range_upper = (i + 1 + b_high).min(awi);
                } else {
                    // $shift/$shiftx 且 B 帶號:雙向。
                    b_range_upper = awi - b_low;
                    a_range_lower = (i + b_low).max(0);
                    if is_signed {
                        a_range_lower = a_range_lower.min(awi - 1);
                    }
                    a_range_upper = (i + 1 + b_high).min(awi);
                }
                if i < b_range_upper {
                    for k in a_range_lower..a_range_upper {
                        push("A", k as usize, "Y", i as usize, &mut e);
                    }
                } else if is_signed && aw > 0 {
                    // 只有符號擴展影響。
                    push("A", aw - 1, "Y", i as usize, &mut e);
                }
                for k in 0..b_cap as i64 {
                    let sh = 1i64 << (k + 1);
                    if matches!(t, "$shl" | "$sshl") {
                        if awi == 1 && is_signed {
                            let base = sh - 1;
                            if i % sh != base && i - awi + 2 < 1 << b_cap {
                                push("B", k as usize, "Y", i as usize, &mut e);
                            }
                        } else if is_signed {
                            if i - awi + 2 < 1 << b_cap {
                                push("B", k as usize, "Y", i as usize, &mut e);
                            }
                        } else if i - awi + 1 < 1 << b_cap {
                            push("B", k as usize, "Y", i as usize, &mut e);
                        }
                    } else if matches!(t, "$shr" | "$sshr")
                        || (matches!(t, "$shift" | "$shiftx") && !is_b_signed)
                    {
                        if is_signed {
                            let shift_in_bulk = i < awi - 1;
                            let zpad_jump = (yi - i) & (sh - 1) != 0
                                && ((yi - i) & !(1 << k)) < (1 << b_cap);
                            if shift_in_bulk
                                || (matches!(t, "$shr" | "$shift" | "$shiftx") && zpad_jump)
                            {
                                push("B", k as usize, "Y", i as usize, &mut e);
                            }
                        } else if i < awi {
                            push("B", k as usize, "Y", i as usize, &mut e);
                        }
                    } else if is_signed {
                        if k != b_cap as i64 - 1 {
                            let r_shift_in_bulk = i < awi - 1;
                            let r_zpad_jump = (yi - i) & (sh - 1) != 0
                                && ((yi - i) & !(1 << k)) <= b_high;
                            let mut l = awi - 2 - i >= b_low;
                            if awi == 1 {
                                l = l && (!(i - awi) & (sh - 1)) != 0;
                            }
                            if r_shift_in_bulk || r_zpad_jump || l {
                                push("B", k as usize, "Y", i as usize, &mut e);
                            }
                        } else if yi - i <= b_high || awi - 2 - i >= b_low {
                            push("B", k as usize, "Y", i as usize, &mut e);
                        }
                    } else if awi - 1 - i >= b_low {
                        push("B", k as usize, "Y", i as usize, &mut e);
                    }
                }
            }
        }
        // mux_op($mux/$pmux)。
        "$mux" | "$pmux" => {
            let (aw, bw, sw) = (w("A"), w("B"), w("S"));
            for i in 0..aw {
                push("A", i, "Y", i, &mut e);
                let mut k = i;
                while k < bw {
                    push("B", k, "Y", i, &mut e);
                    k += aw;
                }
                for s in 0..sw {
                    push("S", s, "Y", i, &mut e);
                }
            }
        }
        // bmux_op / demux_op / bweqx_op / bwmux_op。
        "$bmux" | "$demux" => {
            let yw = w("Y");
            let aw = w("A");
            let sw = w("S");
            for i in 0..yw {
                if t == "$bmux" {
                    let mut k = i;
                    while k < aw {
                        push("A", k, "Y", i, &mut e);
                        k += yw;
                    }
                } else {
                    push("A", i % aw, "Y", i, &mut e);
                }
                for s in 0..sw {
                    push("S", s, "Y", i, &mut e);
                }
            }
        }
        "$bweqx" | "$bwmux" => {
            let yw = w("Y");
            let aw = w("A");
            let bw = w("B");
            let sw = w("S");
            let maxw = yw.min(aw).min(bw).min(if t == "$bweqx" { usize::MAX } else { sw });
            for i in 0..maxw {
                push("A", i, "Y", i, &mut e);
                push("B", i, "Y", i, &mut e);
                if t == "$bwmux" {
                    push("S", i, "Y", i, &mut e);
                }
            }
        }
        // mem_op。
        "$mem_v2" => {
            let n_rd = param_int(cell, "\\RD_PORTS") as usize;
            let abits = param_int(cell, "\\ABITS") as usize;
            let width = param_int(cell, "\\WIDTH") as usize;
            for i in 0..n_rd {
                if param_bit(cell, "\\RD_CLK_ENABLE", i) != Bit::S0 {
                    for k in 0..width {
                        push("RD_ARST", i, "RD_DATA", i * width + k, &mut e);
                    }
                    continue;
                }
                for j in 0..abits {
                    for k in 0..width {
                        push("RD_ADDR", i * abits + j, "RD_DATA", i * width + k, &mut e);
                    }
                }
            }
        }
        "$memrd" | "$memrd_v2" => {
            let abits = param_int(cell, "\\ABITS") as usize;
            let width = param_int(cell, "\\WIDTH") as usize;
            if param_bool(cell, "\\CLK_ENABLE") {
                if t == "$memrd_v2" {
                    for k in 0..width {
                        push("ARST", 0, "DATA", k, &mut e);
                    }
                }
            } else {
                for j in 0..abits {
                    for k in 0..width {
                        push("ADDR", j, "DATA", k, &mut e);
                    }
                }
            }
        }
        "$memwr" | "$memwr_v2" | "$meminit" => { /* no edges here */ }
        // ff_op:邊緣觸發 ff **不建 D→Q 邊**(next-state 回授=時序而非組合
        // 環);僅 latch 的 D/EN 與非同步 CLR/SET/ALOAD/AD/ARST 埠建邊
        // (celledges.cc:398-424;閘級 ff 的埠名 S/R/E/C/L 不在此列=無邊,
        // $_ALDFF_* 的 AD 埠除外)。
        _ if cells::cell_is_builtin_ff(t) => {
            let qw = w("Q");
            if matches!(t, "$dlatch" | "$adlatch" | "$dlatchsr") {
                for k in 0..qw {
                    push("D", k, "Q", k, &mut e);
                    push("EN", 0, "Q", k, &mut e);
                }
            }
            let has = |n: &str| port_spec(cell, n).is_some();
            if has("CLR") {
                for k in 0..qw {
                    push("CLR", 0, "Q", k, &mut e);
                }
            }
            if has("SET") {
                for k in 0..qw {
                    push("SET", 0, "Q", k, &mut e);
                }
            }
            if has("ALOAD") {
                for k in 0..qw {
                    push("ALOAD", 0, "Q", k, &mut e);
                }
            }
            if has("AD") {
                for k in 0..qw {
                    push("AD", k, "Q", k, &mut e);
                }
            }
            if has("ARST") {
                for k in 0..qw {
                    push("ARST", 0, "Q", k, &mut e);
                }
            }
        }
        // full_op:全輸入×全輸出($mul 家族/$lut/$sop/$alu/$lcu/$macc/
        // $macc_v2/閘級 19 種 evaluable)。
        "$mul" | "$div" | "$mod" | "$divfloor" | "$modfloor" | "$pow" | "$lut"
        | "$sop" | "$alu" | "$lcu" | "$macc" | "$macc_v2" | "$_BUF_" | "$_NOT_"
        | "$_AND_" | "$_NAND_" | "$_OR_" | "$_NOR_" | "$_XOR_" | "$_XNOR_"
        | "$_ANDNOT_" | "$_ORNOT_" | "$_MUX_" | "$_NMUX_" | "$_MUX4_"
        | "$_MUX8_" | "$_MUX16_" | "$_AOI3_" | "$_OAI3_" | "$_AOI4_"
        | "$_OAI4_" => {
            let ins: Vec<(String, usize)> = cell
                .connections
                .iter()
                .filter(|(p, _)| cells::port_dir(t, p).0)
                .map(|(p, s)| (crate::jsonout::unescape(p).to_string(), s.bits.len()))
                .collect();
            for (port, sig) in &cell.connections {
                if !cells::port_dir(t, port).1 {
                    continue;
                }
                let oname = crate::jsonout::unescape(port).to_string();
                for o in 0..sig.bits.len() {
                    for (iname, iw) in &ins {
                        for k in 0..*iw {
                            e.push(((iname.clone(), k), (oname.clone(), o)));
                        }
                    }
                }
            }
        }
        // no-op 家族(無輸入或無輸出;參與條件其實不含它們,防禦形保留)。
        "$assert" | "$assume" | "$live" | "$fair" | "$cover" | "$initstate"
        | "$anyconst" | "$anyseq" | "$allconst" | "$allseq" => {}
        _ => return None,
    }
    Some(e)
}

/// 建圖路徑:dispatch 邊集 → sigmap 位 → topo 邊(僅雙 wire;FwdCellEdges
/// Database::add_edge 同語義)。返回 false=dispatch 不認識(→fallback)。
fn add_edges_from_cell(
    cell: &ry_ir::Cell,
    sigmap: &SigMap,
    topo: &mut Vec<((String, i64), usize)>,
    edges: &mut Vec<Vec<usize>>,
) -> bool {
    let Some(list) = cell_edges_dispatch(cell) else {
        return false;
    };
    for ((fp, fi), (tp, ti)) in list {
        let from = port_spec(cell, &fp).map(|s| sigmap.map(s));
        let to = port_spec(cell, &tp).map(|s| sigmap.map(s));
        if let (Some(f), Some(t)) = (from, to) {
            if let (Some(fb), Some(tb)) = (f.bits.get(fi), t.bits.get(ti)) {
                add_topo_edge(topo, edges, fb, tb);
            }
        }
    }
    true
}

/// detail_costly(check.cc:237-264):$add/$sub/shift/$pmux/$bmux 且
/// 輸入寬和或寬積 ≥1024 → true(邊可二次爆炸,改走 fallback)。
fn detail_costly(cell: &ry_ir::Cell) -> bool {
    if !matches!(
        cell.ctype.as_str(),
        "$add" | "$sub" | "$shl" | "$shr" | "$sshl" | "$sshr" | "$shift"
            | "$shiftx" | "$pmux" | "$bmux"
    ) {
        return false;
    }
    let w = |n: &str| port_spec(cell, n).map(|s| s.bits.len()).unwrap_or(0);
    let (in_w, out_w) = if matches!(cell.ctype.as_str(), "$pmux" | "$bmux") {
        (w("S"), w("Y"))
    } else {
        let (mut i, mut o) = (0usize, 0usize);
        for (port, sig) in &cell.connections {
            let (input, output) = cells::port_dir(&cell.ctype, port);
            if input {
                i += sig.bits.len();
            }
            if output {
                o += sig.bits.len();
            }
        }
        (i, o)
    };
    const THRESHOLD: usize = 1024;
    in_w + out_w >= THRESHOLD || in_w * out_w >= THRESHOLD
}

// ---------------------------------------------------------------------------
// topo 基礎設施(M1-c)
// ---------------------------------------------------------------------------

fn bit_key(bit: &SigBit) -> (String, i64) {
    match bit {
        SigBit::Wire { wire, offset } => (wire.clone(), *offset),
        SigBit::Const(_) => (String::new(), -2),
    }
}

fn sig_bit_eq(a: &SigBit, b: &SigBit) -> bool {
    bit_key(a) == bit_key(b)
}

fn topo_node(topo: &mut Vec<((String, i64), usize)>, key: &(String, i64)) -> usize {
    topo_node_nogrow(topo, key.clone())
}

fn topo_push_edge(edges: &mut Vec<Vec<usize>>, l: usize, r: usize) {
    while edges.len() <= r {
        edges.push(Vec::new());
    }
    if !edges[r].contains(&l) {
        edges[r].push(l);
    }
}

fn add_topo_edge(
    topo: &mut Vec<((String, i64), usize)>,
    edges: &mut Vec<Vec<usize>>,
    from: &SigBit,
    to: &SigBit,
) {
    if let (SigBit::Wire { .. }, SigBit::Wire { .. }) = (from, to) {
        let l = topo_node_nogrow(topo, bit_key(from));
        let r = topo_node_nogrow(topo, bit_key(to));
        topo_push_edge(edges, l, r);
    }
}

fn topo_node_nogrow(topo: &mut Vec<((String, i64), usize)>, key: (String, i64)) -> usize {
    if let Some((_, i)) = topo.iter().find(|(k, _)| *k == key) {
        return *i;
    }
    let i = topo.len();
    topo.push((key.clone(), i));
    i
}

/// TopoSort::sort+loops(utils.h 逐字語義):map 迭代=節點插入序的
/// **值序**…oracle golden 實測:loop2not 根=\y、loop3and 根=\p——
/// 節點序=**首次建節點序**(掃描序),DFS 先序訪問 preds(建序)。
fn topo_sort_loops(
    topo: &Vec<((String, i64), usize)>,
    edges: &mut Vec<Vec<usize>>,
) -> Vec<Vec<(String, i64)>> {
    // node 創建與邊增長不同步(未成為邊目標的節點):補齊到同長。
    while edges.len() < topo.len() {
        edges.push(Vec::new());
    }
    let n = topo.len();
    let mut sorted = vec![false; n];
    let mut on_stack = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut loops: Vec<Vec<(String, i64)>> = Vec::new();

    // 迭代序:節點鍵值序(name,offset)——與 std::map<std::pair> 一致。
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|a, b| topo[*a].0.cmp(&topo[*b].0));

    fn worker(
        root: usize,
        topo: &Vec<((String, i64), usize)>,
        edges: &Vec<Vec<usize>>,
        sorted: &mut Vec<bool>,
        on_stack: &mut Vec<bool>,
        stack: &mut Vec<usize>,
        loops: &mut Vec<Vec<(String, i64)>>,
    ) {
        if on_stack[root] {
            let mut lp: Vec<(String, i64)> = Vec::new();
            for i in (0..stack.len()).rev() {
                lp.push(topo[stack[i]].0.clone());
                if stack[i] == root {
                    break;
                }
            }
            if !loops.contains(&lp) {
                loops.push(lp);
            }
            return;
        }
        if sorted[root] {
            return;
        }
        let empty: Vec<usize> = Vec::new();
        let preds_now = edges.get(root).cloned().unwrap_or(empty);
        if !preds_now.is_empty() {
            stack.push(root);
            on_stack[root] = true;
            let mut preds = preds_now;
            preds.sort_by(|a, b| topo[*a].0.cmp(&topo[*b].0));
            for l in preds {
                worker(l, topo, edges, sorted, on_stack, stack, loops);
            }
            stack.pop();
            on_stack[root] = false;
        }
        sorted[root] = true;
    }

    for root in order {
        if root >= n {
            continue;
        }
        worker(root, topo, edges, &mut sorted, &mut on_stack, &mut stack, &mut loops);
    }
    loops
}

/// 匹配邊枚舉(MatchingEdgePrinter 的數據源):返回 (port,bit,idx) 對。
/// MatchingEdgePrinter 走抽象 dispatch(非 check.cc 覆寫版,無 fallback)
/// ——與 cell_edges_dispatch 同一邊集,sigmap 後逐位。
fn cell_edge_pairs(
    _module: &Module,
    sigmap: &crate::sigmap::SigMap,
    cell: &ry_ir::Cell,
) -> Vec<((String, SigBit, usize), (String, SigBit, usize))> {
    let mut out = Vec::new();
    let Some(list) = cell_edges_dispatch(cell) else {
        return out;
    };
    for ((fp, fi), (tp, ti)) in list {
        let fs = port_spec(cell, &fp).map(|s| sigmap.map(s));
        let ts = port_spec(cell, &tp).map(|s| sigmap.map(s));
        if let (Some(f), Some(t)) = (fs, ts) {
            if let (Some(fb), Some(tb)) = (f.bits.get(fi), t.bits.get(ti)) {
                out.push(((fp, fb.clone(), fi), (tp, tb.clone(), ti)));
            }
        }
    }
    out
}

/// log_signal(SigSpec):完整 spec 的傾印形(驅動描述串內嵌用)。
fn log_signal_spec(sig: &SigSpec) -> String {
    let mut parts: Vec<String> = Vec::new();
    for bit in &sig.bits {
        parts.push(match bit {
            SigBit::Wire { wire, offset } => format!("{wire} [{offset}]"),
            SigBit::Const(b) => format!("1'{}", b.as_char()),
        });
    }
    if parts.len() == 1 {
        return parts.pop().unwrap();
    }
    format!("{{ {} }}", parts.join(" "))
}

/// `attribute \top 1` 判定(stat 的 top 偵測共用)。屬性鍵以 .il 轉義形
/// 存儲(`\top`),兩形都查。
pub fn module_is_top(module: &Module) -> bool {
    module
        .attributes
        .get("\\top")
        .or_else(|| module.attributes.get("top"))
        .map(is_true)
        .unwrap_or(false)
}
