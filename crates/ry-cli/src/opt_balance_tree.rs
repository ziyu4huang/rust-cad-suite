//! opt_balance_tree(passes/opt/opt_balance_tree.cc;M3 續弧 slice-4 翻實現,
//! RYOSYS-MVP §10)。
//!
//! * **TopoSort=String 字典序比較器**(opt_balance_tree.cc:271
//!   `TopoSort<Cell*, IdString::compare_ptr_by_name<Cell>>`;utils.h:132
//!   TopoSort 的 node_to_index=std::map<T,int,C> 以 C 排序根遍歷、edges=
//!   set<IndirectCmp> 同 C)——本 pass 由 yosys 自指定串序,是 10.9-r
//!   intern 首見序律的**顯式例外**(P8;D-ys8b lt_by_name 字串序後的
//!   字串序消費面第二例)。仲裁=carryadd 錨 byte-diff。
//! * sources=dict<SigSpec,int> **hashlib 插入序**(hashlib.h entries
//!   vector append-only 正向迭代=插入序,erase=swap-and-pop;本輪源碼
//!   重讀仲裁,與詳案 P8 一致)——Vec+線性查改(既有模式)。
//! * BFS 的 pool<Cell*>=插入序(Vec+去重);建名 NEW_ID=autoidx 配對
//!   (leaf cell :97+wire :108、combine cell :132+wire :143;combine
//!   wire 事後被內部 clean -purge 折疊刪名——autoidx 45→77=32 號
//!   carryadd 實錄)。
//! * 尾 `Yosys::run_pass("clean -purge")`(yosys.cc:880-886 同構:
//!   log("\n")+`-- Running command \`clean -purge' --` 橫幅,不佔編號)
//!   =內部 clean 依賴,M3 stub 理由清償(P8)。
//! * experimental 機制首消費者(log.h:530 log_experimental→
//!   log.cc:399-405 add_experimental:首次 Warning+集合去重;driver.cc:
//!   721-722 flush 行)。

use std::collections::HashMap;

use ry_ir::{Cell, Design, SigBit, SigSpec};

use crate::opt::{err, module_is_boxed, module_connect, cell_port, OptError};
use crate::sigmap::SigMap;
use crate::Log;

/// log_experimental(log.cc:399-405 同構):首次=Warning 行+入集合
/// (unique/total 與 experimental 彙總雙計數);重複=靜默。
pub(crate) fn log_experimental(log: &mut Log, feature: &str) {
    if log.experimental.iter().any(|f| f == feature) {
        return;
    }
    let msg = format!("Feature '{feature}' is experimental.");
    log.log(&format!("Warning: {msg}\n"));
    log.warnings.push(msg);
    log.experimental.push(feature.to_string());
}

fn param_int(cell: &Cell, name: &str) -> i64 {
    crate::sigops::const_as_int(&cell.parameters.get(name).cloned().unwrap_or(
        ry_ir::Const::Bits { width: 0, bits: Vec::new(), signed: false, is_unsized: false },
    ))
}

fn param_bool(cell: &Cell, name: &str) -> bool {
    match cell.parameters.get(name) {
        Some(ry_ir::Const::Bits { bits, .. }) => bits.first() == Some(&ry_ir::Bit::S1),
        _ => false,
    }
}

fn const_from_bool(b: bool) -> ry_ir::Const {
    ry_ir::Const::Bits { width: 1, bits: vec![if b { ry_ir::Bit::S1 } else { ry_ir::Bit::S0 }], signed: false, is_unsized: false }
}

fn const_from_int(v: i64) -> ry_ir::Const {
    // oracle fixup_parameters 的 setParam(int)=RTLIL::Const(int)=32 位
    // (dump autoint 十進制形;carryadd 錨實錄 `parameter \A_WIDTH 1`)。
    ry_ir::Const::from_i64_i32_width(v)
}

/// is_right_type(opt_balance_tree.cc:49-76):型別匹配+Y_WIDTH>=自然寬
/// ($add=max+1 帶進位/$mul=和/邏輯=max;無截斷防等價性破壞,#5605)。
fn is_right_type(cell: &Cell, cell_type: &str) -> bool {
    if cell.ctype != cell_type {
        return false;
    }
    let y_width = param_int(cell, "\\Y_WIDTH");
    let a_width = param_int(cell, "\\A_WIDTH");
    let b_width = param_int(cell, "\\B_WIDTH");
    let natural_width = match cell_type {
        "$add" => a_width.max(b_width) + 1,
        "$mul" => a_width + b_width,
        _ => a_width.max(b_width),
    };
    y_width >= natural_width
}

/// chain_driver(opt_balance_tree.cc:79-89):輸入端口的 driver 若續鏈
/// 則返回;模組輸入端口位守衛(drv 已 consumed 則放行)。
fn chain_driver(
    m: &ry_ir::Module,
    am: &SigMap,
    sig_to_driver: &HashMap<SigSpec, usize>,
    input_port_sigs: &[SigBit],
    consumed: &[usize],
    ci: usize,
    port: &str,
    cell_type: &str,
) -> Option<usize> {
    let sig = am.map(&cell_port(&m.cells[ci], port));
    let drv = *sig_to_driver.get(&sig)?;
    if !is_right_type(&m.cells[drv], cell_type) {
        return None;
    }
    for b in &sig.bits {
        if input_port_sigs.contains(b) && !consumed.contains(&drv) {
            return None;
        }
    }
    Some(drv)
}

fn pool_insert(v: &mut Vec<usize>, x: usize) {
    if !v.contains(&x) {
        v.push(x);
    }
}

/// create_balanced_tree(opt_balance_tree.cc:91-151 同構):0 源=空、
/// 1 源=直返、2 源=leaf cell+wire(:97/:108)、>2 源=mid=(n+1)/2 分半
/// 遞歸+combine cell+wire(:132/:143)。屬性複製(含 src);寬:add=
/// max+1/mul=和/邏輯=參照 Y_WIDTH;A_SIGNED/B_SIGNED 拷貝參照。
#[allow(clippy::too_many_arguments)]
fn create_balanced_tree(
    m: &mut ry_ir::Module,
    sources: &[SigSpec],
    cell_type: &str,
    ref_ci: usize,
    autoidx: &mut i64,
    ti: usize,
    cell_count: &mut [i64; 5],
) -> SigSpec {
    if sources.is_empty() {
        return SigSpec::default();
    }
    if sources.len() == 1 {
        return sources[0].clone();
    }
    let two_source = sources.len() == 2;
    if !two_source {
        let mid = (sources.len() + 1) / 2;
        let left = create_balanced_tree(m, &sources[..mid], cell_type, ref_ci, autoidx, ti, cell_count);
        let right = create_balanced_tree(m, &sources[mid..], cell_type, ref_ci, autoidx, ti, cell_count);
        let mut ret = create_two_source_cell(
            m, &left, &right, cell_type, ref_ci, autoidx, ti, cell_count, true,
        );
        return std::mem::take(&mut ret);
    }
    let mut ret = create_two_source_cell(
        m, &sources[0], &sources[1], cell_type, ref_ci, autoidx, ti, cell_count, false,
    );
    std::mem::take(&mut ret)
}

/// 2 源 cell+wire 建立面(leaf=cc:97/cc:108;combine=cc:132/cc:143)。
#[allow(clippy::too_many_arguments)]
fn create_two_source_cell(
    m: &mut ry_ir::Module,
    a: &SigSpec,
    b: &SigSpec,
    cell_type: &str,
    ref_ci: usize,
    autoidx: &mut i64,
    ti: usize,
    cell_count: &mut [i64; 5],
    combine: bool,
) -> SigSpec {
    let (cell_line, wire_line) =
        if combine { (132, 143) } else { (97, 108) };
    let n_cell = *autoidx;
    let n_wire = *autoidx + 1;
    *autoidx += 2;
    let cname = format!("$auto$opt_balance_tree.cc:{cell_line}:create_balanced_tree${n_cell}");
    let wname = format!("$auto$opt_balance_tree.cc:{wire_line}:create_balanced_tree${n_wire}");
    let ref_cell = m.cells[ref_ci].clone();
    let mut out_width = param_int(&ref_cell, "\\Y_WIDTH");
    if cell_type == "$add" {
        out_width = a.size().max(b.size()) + 1;
    } else if cell_type == "$mul" {
        out_width = a.size() + b.size();
    }
    // addWire(NEW_ID, width):零屬性零 offset 宣告(補 wires 尾)。
    m.wires.push(ry_ir::Wire {
        name: wname.clone(),
        width: out_width,
        start_offset: 0,
        port_id: 0,
        port_input: false,
        port_output: false,
        upto: false,
        is_signed: false,
        attributes: Default::default(),
    });
    let y_spec = SigSpec {
        bits: vec![SigBit::Wire { wire: wname.clone(), offset: 0 }],
    };
    // addCell(NEW_ID)+attributes 複製+setPort A/B/Y+參數
    // (fixup=A_WIDTH/B_WIDTH/Y_WIDTH 由端口寬;setParam signed 拷貝;
    // 參數插入序=oracle dump 形 A_SIGNED/A_WIDTH/B_SIGNED/B_WIDTH/Y_WIDTH)。
    // A_SIGNED/B_SIGNED=setParam 拷貝參照 cell 的**原 Const 對象**
    // (fixup 不動 signed;原形=32 位 autoint→dump 整數形)。
    let a_signed_c = ref_cell.parameters.get("\\A_SIGNED").cloned();
    let b_signed_c = ref_cell.parameters.get("\\B_SIGNED").cloned();
    let mut parameters = ry_ir::InsertMap::default();
    parameters.insert(
        "\\A_SIGNED".to_string(),
        a_signed_c.unwrap_or_else(|| const_from_bool(false)),
    );
    parameters.insert("\\A_WIDTH".to_string(), const_from_int(a.size()));
    parameters.insert(
        "\\B_SIGNED".to_string(),
        b_signed_c.unwrap_or_else(|| const_from_bool(false)),
    );
    parameters.insert("\\B_WIDTH".to_string(), const_from_int(b.size()));
    parameters.insert("\\Y_WIDTH".to_string(), const_from_int(y_spec.size()));
    m.cells.push(Cell {
        ctype: cell_type.to_string(),
        name: cname,
        attributes: ref_cell.attributes.clone(),
        parameters,
        connections: vec![
            ("\\A".to_string(), a.clone()),
            ("\\B".to_string(), b.clone()),
            ("\\Y".to_string(), y_spec.clone()),
        ],
    });
    cell_count[ti] += 1;
    y_spec
}

/// worker(OptBalanceTreeWorker ctor 同構):逐 cell_type 索引重建+
/// BFS+TopoSort(字串序)+sources 插入序展開+建樹+connect+consumed 移除。
fn worker(
    m: &mut ry_ir::Module,
    cell_types: &[&str; 5],
    autoidx: &mut i64,
    cell_count: &mut [i64; 5],
) {
    let mut am = SigMap::new();
    am.set(m);
    for (ti, cell_type) in cell_types.iter().enumerate() {
        // 索引面:全部 cell 的輸出/輸入端口(dict 每型重建)
        let mut sig_to_driver: HashMap<SigSpec, usize> = HashMap::new();
        let mut sig_to_sink: HashMap<SigSpec, Vec<usize>> = HashMap::new();
        // selected_cells 遍歷=逆插入序(cell_at(i)=element(i)=最新先,
        // hashlib.h:878;opt.rs 同款 Vec+rev() 模式)
        for ci in (0..m.cells.len()).rev() {
            let conns: Vec<(String, SigSpec)> = m.cells[ci]
                .connections
                .iter()
                .map(|(p, s)| (p.clone(), s.clone()))
                .collect();
            for (port, conn) in conns {
                let (_is_in, is_out) = crate::cells::port_dir(&m.cells[ci].ctype, &port);
                let sig = am.map(&conn);
                if is_out {
                    sig_to_driver.insert(sig, ci);
                } else if _is_in {
                    sig_to_sink.entry(sig).or_default().push(ci);
                }
            }
        }

        // 模組端口面(selected_wires 的 port_input/port_output)
        let mut input_port_sigs: Vec<SigBit> = Vec::new();
        let mut output_port_sigs: Vec<SigBit> = Vec::new();
        let wires: Vec<ry_ir::Wire> = m.wires.clone();
        for w in &wires {
            if !(w.port_input || w.port_output) {
                continue;
            }
            let sig = am.map(&SigSpec {
                bits: (0..w.width)
                    .map(|o| SigBit::Wire { wire: w.name.clone(), offset: o })
                    .collect(),
            });
            for b in sig.bits {
                let b = b.clone();
                if w.port_input && !input_port_sigs.contains(&b) {
                    input_port_sigs.push(b.clone());
                }
                if w.port_output && !output_port_sigs.contains(&b) {
                    output_port_sigs.push(b);
                }
            }
        }

        // Actual logic(per cell_type consumed 清零,源碼同構)
        let mut consumed: Vec<usize> = Vec::new();
        for ci in (0..m.cells.len()).rev() {
            if consumed.contains(&ci) || !is_right_type(&m.cells[ci], cell_type) {
                continue;
            }
            // BFS(pool 層進;sinks 收集)
            let y = am.map(&cell_port(&m.cells[ci], "\\Y"));
            let mut sinks: Vec<usize> = Vec::new();
            let mut current_loads: Vec<usize> =
                sig_to_sink.get(&y).cloned().unwrap_or_default();
            let mut next_loads: Vec<usize> = Vec::new();
            let mut visited: Vec<usize> = Vec::new();
            loop {
                if current_loads.is_empty() {
                    break;
                }
                for &x in current_loads.iter().rev() {
                    if visited.contains(&x) {
                        continue;
                    }
                    visited.push(x);
                    if !is_right_type(&m.cells[x], cell_type) {
                        // 非 right_type 載荷→插起始 cell(源碼特例)
                        pool_insert(&mut sinks, ci);
                        continue;
                    }
                    let xy = am.map(&cell_port(&m.cells[x], "\\Y"));
                    let drives_port = xy
                        .bits
                        .iter()
                        .any(|b| output_port_sigs.contains(b));
                    if drives_port && !consumed.contains(&x) {
                        pool_insert(&mut sinks, x);
                    }
                    if let Some(next) = sig_to_sink.get(&xy) {
                        // sig_to_sink[sig] 的 pool 迭代=逆插入
                        for &z in next.iter().rev() {
                            pool_insert(&mut next_loads, z);
                        }
                    }
                }
                if next_loads.is_empty() {
                    for &s in current_loads.iter().rev() {
                        if is_right_type(&m.cells[s], cell_type) {
                            pool_insert(&mut sinks, s);
                        }
                    }
                    break;
                }
                current_loads = next_loads;
                next_loads = Vec::new();
            }

            // sinks 循環:TopoSort(String 字典序)+sources+建樹
            // (sinks pool 迭代=逆插入序)
            for &head in sinks.iter().rev() {
                if consumed.contains(&head) {
                    continue;
                }
                // TopoSort(node(head)+沿 A/B chain_driver 收邊;queue=
                // LIFO vector)
                let mut nodes: Vec<usize> = vec![head];
                let mut queue: Vec<usize> = vec![head];
                let mut edges: Vec<(usize, usize)> = Vec::new(); // (drv, x)
                while let Some(x) = queue.pop() {
                    for port in ["\\A", "\\B"] {
                        if let Some(drv) = chain_driver(
                            m, &am, &sig_to_driver, &input_port_sigs, &consumed, x,
                            port, cell_type,
                        ) {
                            if !nodes.contains(&drv) {
                                nodes.push(drv);
                                queue.push(drv);
                            }
                            edges.push((drv, x));
                        }
                    }
                }
                // sort():根按名序遍歷+edges(入邊)名序 DFS 後序
                // (utils.h:132-260);組合環=abandon(continue)。
                let sorted = match topo_sort(m, &nodes, &edges) {
                    Some(s) => s,
                    None => continue,
                };
                // sources(dict 插入序;sorted 尾=head 起 reach 傳播)
                let mut sources: Vec<(SigSpec, i64)> = Vec::new();
                let mut signeds: Vec<(SigSpec, bool)> = Vec::new();
                let inner_cells = sorted.len() as i64 - 1;
                let mut reach: HashMap<usize, i64> = HashMap::new();
                reach.insert(head, 1);
                for i in (0..sorted.len()).rev() {
                    let x = sorted[i];
                    for port in ["\\A", "\\B"] {
                        if let Some(drv) = chain_driver(
                            m, &am, &sig_to_driver, &input_port_sigs, &consumed, x,
                            port, cell_type,
                        ) {
                            *reach.entry(drv).or_insert(0) += *reach.get(&x).unwrap_or(&0);
                        } else {
                            let sig = am.map(&cell_port(&m.cells[x], port));
                            let rx = *reach.get(&x).unwrap_or(&0);
                            match sources.iter_mut().find(|(s, _)| *s == sig) {
                                Some((_, c)) => *c += rx,
                                None => sources.push((sig.clone(), rx)),
                            }
                            let sb = if port == "\\A" {
                                param_bool(&m.cells[x], "\\A_SIGNED")
                            } else {
                                param_bool(&m.cells[x], "\\B_SIGNED")
                            };
                            match signeds.iter_mut().find(|(s, _)| *s == sig) {
                                Some((_, v)) => *v = sb,
                                None => signeds.push((sig, sb)),
                            }
                        }
                    }
                }
                if inner_cells > 0 {
                    // log_debug(計數;opt_balance_tree.cc:258)
                    crate::proc::dbg_inc();
                    // 展開 reach 多重性(sources 迭代序=插入序)
                    let mut source_signals: Vec<SigSpec> = Vec::new();
                    let mut signed_flags: Vec<bool> = Vec::new();
                    for (sig, cnt) in sources.iter().rev() {
                        for _ in 0..*cnt {
                            source_signals.push(sig.clone());
                            signed_flags.push(
                                signeds.iter().find(|(s, _)| s == sig).map(|(_, b)| *b).unwrap_or(false),
                            );
                        }
                    }
                    // 全同 signed 才平衡
                    if !signed_flags.iter().all(|f| *f == signed_flags[0]) {
                        continue;
                    }
                    let ref_ci = head;
                    let tree_output = create_balanced_tree(
                        m, &source_signals, cell_type, ref_ci, autoidx, ti, cell_count,
                    );
                    // connect(head_output ↔ tree_output;寬差=符號擴展段)
                    let head_output = am.map(&cell_port(&m.cells[head], "\\Y"));
                    let connect_width = head_output.size().min(tree_output.size());
                    module_connect(
                        m,
                        head_output.extract(0, connect_width),
                        tree_output.extract(0, connect_width),
                    );
                    if head_output.size() > tree_output.size() {
                        let sext_bit = if param_bool(&m.cells[head], "\\A_SIGNED") {
                            head_output.bits[(connect_width - 1) as usize].clone()
                        } else {
                            SigBit::Const(ry_ir::Bit::S0)
                        };
                        module_connect(
                            m,
                            head_output.extract(
                                connect_width,
                                head_output.size() - connect_width,
                            ),
                            SigSpec {
                                bits: vec![sext_bit; (head_output.size() - connect_width) as usize],
                            },
                        );
                    }
                    pool_insert(&mut consumed, head);
                }
            }
        }

        // 移除 consumed(pool range-for=逆插入序;dict erase=swap-and-pop 同構)
        for &ci in consumed.iter().rev() {
            let name = m.cells[ci].name.clone();
            if let Some(pos) = m.cells.iter().position(|c| c.name == name) {
                m.cells.swap_remove(pos);
            }
        }
    }
}

/// TopoSort::sort() 同構(utils.h:132-260):根按名序遍歷;edges=入邊
/// (edge(drv,x)→edges[x] 收 drv)按名序 set;DFS 後序(sorted=源…匯);
/// 環=found_loops→None。
fn topo_sort(m: &ry_ir::Module, nodes: &[usize], edges: &[(usize, usize)]) -> Option<Vec<usize>> {
    let key = |&ci: &usize| m.cells[ci].name.as_bytes().to_vec();
    // node_to_index:std::map<T,int,C>(C=名序)→名序遍歷根
    let mut by_name: Vec<usize> = nodes.to_vec();
    by_name.sort_by(|a, b| key(a).cmp(&key(b)));
    // edges[i]=入邊集合(set<IndirectCmp>=名序)
    let mut in_edges: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    let idx = |ci: usize| nodes.iter().position(|&n| n == ci).unwrap();
    for &(drv, x) in edges {
        let di = idx(drv);
        let xi = idx(x);
        if !in_edges[xi].contains(&di) {
            in_edges[xi].push(di);
        }
    }
    for e in in_edges.iter_mut() {
        e.sort_by(|a, b| key(&nodes[*a]).cmp(&key(&nodes[*b])));
    }
    let mut is_sorted = vec![false; nodes.len()];
    let mut on_stack = vec![false; nodes.len()];
    let mut found_loop = false;
    let mut sorted: Vec<usize> = Vec::with_capacity(nodes.len());
    for &root in &by_name {
        let ri = idx(root);
        sort_worker(
            ri,
            nodes,
            &in_edges,
            &mut is_sorted,
            &mut on_stack,
            &mut found_loop,
            &mut sorted,
        );
    }
    if found_loop {
        None
    } else {
        Some(sorted.iter().map(|&i| nodes[i]).collect())
    }
}

#[allow(clippy::too_many_arguments)]
fn sort_worker(
    root: usize,
    nodes: &[usize],
    in_edges: &[Vec<usize>],
    is_sorted: &mut [bool],
    on_stack: &mut [bool],
    found_loop: &mut bool,
    sorted: &mut Vec<usize>,
) {
    if on_stack[root] {
        *found_loop = true;
        return;
    }
    if is_sorted[root] {
        return;
    }
    if !in_edges[root].is_empty() {
        on_stack[root] = true;
        for &left in &in_edges[root] {
            sort_worker(left, nodes, in_edges, is_sorted, on_stack, found_loop, sorted);
        }
        on_stack[root] = false;
    }
    is_sorted[root] = true;
    sorted.push(root);
}

/// opt_balance_tree 命令面(opt_balance_tree.cc:359-414 同構):
/// log_header→log_experimental→參數(-arith/-logic=Y2 硬錯,§10.1-4)
/// →逐模組 worker(selected_modules 正序=源碼;盒模組跳過)→Converted
/// 行按 cell_types 數組序→尾 run_pass("clean -purge")。
pub fn run_opt_balance_tree(
    design: &mut Design,
    args: &[String],
    log: &mut Log,
) -> Result<(), OptError> {
    log.header("Executing OPT_BALANCE_TREE pass (cell cascades to trees).\n");
    log_experimental(log, "open_balance_tree");
    for a in args.get(1..).unwrap_or(&[]) {
        return Err(err(format!(
            "opt_balance_tree: option `{a}' is not implemented in ryosys (milestone Y2)"
        )));
    }
    const CELL_TYPES: [&str; 5] = ["$and", "$or", "$xor", "$add", "$mul"];
    let mut cell_count = [0i64; 5];
    let mut autoidx = design.autoidx;
    for mi in 0..design.modules.len() {
        if module_is_boxed(&design.modules[mi]) {
            continue;
        }
        worker(&mut design.modules[mi], &CELL_TYPES, &mut autoidx, &mut cell_count);
    }
    design.autoidx = autoidx;
    let mut out = String::new();
    for (ti, t) in CELL_TYPES.iter().enumerate() {
        out.push_str(&format!(
            "Converted {} {t} cells into trees.\n",
            cell_count[ti]
        ));
    }
    log.log(&out);
    log.resync();
    // 尾 run_pass("clean -purge")(yosys.cc:880-886:log("\n")+橫幅)
    log.log("\n");
    log.log("-- Running command `clean -purge' --\n");
    let cargs = vec!["clean".to_string(), "-purge".to_string()];
    crate::opt::run_opt_clean(design, &cargs, log, false, false)?;
    Ok(())
}
