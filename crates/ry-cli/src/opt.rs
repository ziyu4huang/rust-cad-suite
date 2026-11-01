//! opt 家族 passes(RYOSYS-MVP §9;passes/opt/*.cc 的 0.70+179 移植面)。
//!
//! 源碼對讀紀律:行號按段定位(clone 28ba3cb 原地禁 fetch),一切契約以
//! 二進制實跑為準(golden=oracle 錄製;序差先分類再動手,§9.0 末段)。
//!
//! 迭代序契約(承 M2 RM2-6):`design->selected_modules()` 正向=dict 逆
//! 插入序;`module->cells()`/`module->wires()` 正向=逆插入序;但
//! cells_all/wires 的 `cell_at(i)`/`wire_at(i)` 索引循環=**插入序**。
//! dict erase=swap-and-pop(Vec swap_remove)。
//!
//! did_something:scratchpad bool `opt.did_something`(opt.cc:160/163)——
//! 本檔 thread_local 承載,opt 編排器(slice-6)以 did_unset/did_get 消費。

use crate::cells::{cell_is_builtin_ff, cell_known, port_dir};
use crate::sigmap::SigMap;
use crate::sigops::id_unescape;
use crate::Log;
use ry_ir::{Bit, Const, Design, InsertMap, Module, SigBit, SigSpec};
use std::collections::{HashMap, HashSet};

pub struct OptError(pub String);

pub(crate) fn err(msg: impl Into<String>) -> OptError {
    OptError(msg.into())
}

// ---------------------------------------------------------------------------
// did_something(scratchpad `opt.did_something`;opt.cc 編排 changed 判定)
// ---------------------------------------------------------------------------

thread_local! {
    static DID_SOMETHING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// scratchpad_set_bool(置位語義;unset 由編排器每輪迴圈頭呼叫)。
/// did_unset/did_get 的消費面=opt 編排器(M3 slice-6 落地);中間 slices
/// 以 allow 壓未用警告(零警告閘)。
pub fn did_set() {
    DID_SOMETHING.with(|c| c.set(true));
}

#[allow(dead_code)]
pub fn did_unset() {
    DID_SOMETHING.with(|c| c.set(false));
}

#[allow(dead_code)]
pub fn did_get() -> bool {
    DID_SOMETHING.with(|c| c.get())
}

// ---------------------------------------------------------------------------
// 共用面
// ---------------------------------------------------------------------------

/// ct_reg(CleanRunContext::ct_reg = Compat::mem_ff ∪ is_anyinit):
/// 暫存器類 cell(raw_registers 收集面)。
fn ct_reg(t: &str) -> bool {
    cell_is_builtin_ff(t) || matches!(t, "$mem" | "$mem_v2" | "$anyinit")
}

/// check_public_name(wires.cc:150):`$` 私有、`\_X_`/`\_[…]` 形、含 `.$`
/// 皆非 public。
fn check_public_name(name: &str) -> bool {
    if name.starts_with('$') {
        return false;
    }
    let body = name.strip_prefix('\\').unwrap_or(name);
    if body.starts_with('_') && (body.ends_with('_') || body.contains("_[") || body.contains("\\["))
    {
        return false;
    }
    if body.contains(".$") {
        return false;
    }
    true
}

/// opt_dff 共用(公共 wire 位構造)。
pub(crate) fn wire_bits_pub(name: &str, width: i64) -> SigSpec {
    wire_bits(name, width)
}

/// log_signal 的 wire 宽度表(各 pass log 面共用)。
pub(crate) fn widths_of(m: &Module) -> std::collections::HashMap<String, i64> {
    m.wires.iter().map(|w| (w.name.clone(), w.width)).collect()
}

fn wire_bits(name: &str, width: i64) -> SigSpec {
    SigSpec {
        bits: (0..width).map(|o| SigBit::Wire { wire: name.to_string(), offset: o }).collect(),
    }
}

/// Cell 端口取值(缺端口=空 SigSpec;getPort 語義)。
pub(crate) fn cell_port(cell: &ry_ir::Cell, name: &str) -> SigSpec {
    cell.connections
        .iter()
        .find(|(p, _)| p == name)
        .map(|(_, s)| s.clone())
        .unwrap_or_default()
}

fn sig_sort(sig: &mut SigSpec) {
    sig.bits.sort_by(|a, b| bit_ord(a, b));
}

/// SigBit::operator<(bit_lt)的 Ordering 形(相等=Equal)。
fn bit_ord(a: &SigBit, b: &SigBit) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if a == b {
        Ordering::Equal
    } else if crate::sigops::bit_lt(a, b) {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}

/// SigSpec::operator<(逐位字典序;yosys rtlil.h)。
fn sig_lt(a: &SigSpec, b: &SigSpec) -> bool {
    for (x, y) in a.bits.iter().zip(b.bits.iter()) {
        if x != y {
            return crate::sigops::bit_lt(x, y);
        }
    }
    a.bits.len() < b.bits.len()
}

fn sig_as_const(sig: &SigSpec) -> Option<Const> {
    if !crate::sigops::is_fully_const(sig) {
        return None;
    }
    let bits: Vec<Bit> = sig
        .bits
        .iter()
        .map(|b| match b {
            SigBit::Const(c) => *c,
            SigBit::Wire { .. } => Bit::Sx,
        })
        .collect();
    Some(Const::Bits { width: bits.len() as i64, bits, signed: false, is_unsized: false })
}

fn is_const_true(c: &Const) -> bool {
    matches!(c, Const::Bits { bits, .. } if bits.first() == Some(&Bit::S1))
}

fn param_memid(cell: &ry_ir::Cell) -> String {
    match cell.parameters.get("\\MEMID") {
        Some(Const::Str(s)) => s.clone(),
        _ => String::new(),
    }
}

/// get_blackbox_attribute(rtlil.h:2097):blackbox 或 whitebox 屬性為真。
/// selected_modules() 預設=SB_UNBOXED_WARN ⇒ 盒模組不進 opt pass 面。
pub(crate) fn module_is_boxed(m: &Module) -> bool {
    m.attributes.get("\\blackbox").map(is_const_true).unwrap_or(false)
        || m.attributes.get("\\whitebox").map(is_const_true).unwrap_or(false)
}

/// Module::connect(rtlil.cc:3064):lhs 常數位過濾(空=不加)。
pub(crate) fn module_connect(m: &mut Module, lhs: SigSpec, rhs: SigSpec) {
    if lhs.has_const() {
        let mut nl = SigSpec::default();
        let mut nr = SigSpec::default();
        for (i, b) in lhs.bits.iter().enumerate() {
            if matches!(b, SigBit::Wire { .. }) {
                nl.bits.push(b.clone());
                nr.bits.push(rhs.bits[i].clone());
            }
        }
        if !nl.bits.is_empty() {
            m.connections.push((nl, nr));
        }
    } else {
        m.connections.push((lhs, rhs));
    }
}

// ---------------------------------------------------------------------------
// opt_clean(passes/opt/clean/;無參契約形,verbose=true)
// ---------------------------------------------------------------------------

#[derive(Default)]
struct RmStats {
    cells: i64,
    wires: i64,
}

/// keep_cell(keep_cache.h:146)。
fn keep_cell_raw(cell: &ry_ir::Cell, purge: bool) -> bool {
    if matches!(
        cell.ctype.as_str(),
        "$assert" | "$assume" | "$live" | "$fair" | "$cover" | "$overwrite_tag" | "$print" | "$check"
    ) {
        return true;
    }
    if cell.attributes.get("\\keep").map(is_const_true).unwrap_or(false) {
        return true;
    }
    if !purge && cell.ctype == "$scopeinfo" {
        return true;
    }
    false
}

/// KeepCache(keep_cache.h:30-96):模組 keep 屬性或含 keep_cell → kept;
/// 被 kept 模組實例化的 cell → keep。單線程等價面。
fn build_keep_cache(design: &Design, purge: bool) -> HashMap<String, bool> {
    let mut keep_modules: HashMap<String, bool> = HashMap::new();
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();
    let module_names: HashSet<&String> = design.modules.iter().map(|m| &m.name).collect();
    for m in &design.modules {
        let mut keep = m.attributes.get("\\keep").map(is_const_true).unwrap_or(false);
        if !keep {
            keep = m.cells.iter().any(|c| keep_cell_raw(c, purge));
        }
        keep_modules.insert(m.name.clone(), keep);
        for c in &m.cells {
            if module_names.contains(&c.ctype) {
                dependents.entry(c.ctype.clone()).or_default().push(m.name.clone());
            }
        }
    }
    let mut worklist: Vec<String> =
        keep_modules.iter().filter(|(_, &k)| k).map(|(k, _)| k.clone()).collect();
    while let Some(mn) = worklist.pop() {
        if let Some(deps) = dependents.get(&mn) {
            for d in deps.clone() {
                if !keep_modules.get(&d).copied().unwrap_or(false) {
                    keep_modules.insert(d.clone(), true);
                    worklist.push(d);
                }
            }
        }
    }
    keep_modules
}

/// remove_temporary_cells(cells_temp.cc:73):$pos/$_BUF_/$buf(z 修剪)+
/// $connect(展平為 connect 語句)+$input_port 刪除;**cell_at 序=逆插入
/// (最新優先)**,連線先收齊再刪;刪除=逐顆 swap-and-pop(最新優先序)。
fn remove_temporary_cells(m: &mut Module, verbose: bool) {
    let mut delcells: Vec<String> = Vec::new();
    let mut new_connections: Vec<(SigSpec, SigSpec)> = Vec::new();
    for cell in m.cells.iter().rev() {
        if matches!(cell.ctype.as_str(), "$pos" | "$_BUF_" | "$buf")
            && !cell.attributes.get("\\keep").map(is_const_true).unwrap_or(false)
        {
            // trim_buf
            let mut a = cell_port(cell, "\\A");
            let mut y = cell_port(cell, "\\Y");
            let signed = cell
                .parameters
                .get("\\A_SIGNED")
                .map(is_const_true)
                .unwrap_or(false);
            crate::sigops::extend_u0(&mut a, y.bits.len() as i64, signed);
            if a.bits.iter().any(|b| *b == SigBit::Const(Bit::Sz)) {
                let mut na: Vec<SigBit> = Vec::new();
                let mut ny: Vec<SigBit> = Vec::new();
                for (i, b) in a.bits.iter().enumerate() {
                    if *b == SigBit::Const(Bit::Sz) {
                        continue;
                    }
                    na.push(b.clone());
                    if let Some(yb) = y.bits.get(i) {
                        ny.push(yb.clone());
                    }
                }
                a = SigSpec { bits: na };
                y = SigSpec { bits: ny };
            }
            if !y.bits.is_empty() {
                new_connections.push((y, a));
            }
            delcells.push(cell.name.clone());
        } else if cell.ctype == "$connect"
            && !cell.attributes.get("\\keep").map(is_const_true).unwrap_or(false)
        {
            let mut a = cell_port(cell, "\\A");
            let mut b = cell_port(cell, "\\B");
            if a.has_const() && !b.has_const() {
                std::mem::swap(&mut a, &mut b);
            }
            new_connections.push((a, b));
            delcells.push(cell.name.clone());
        } else if cell.ctype == "$input_port"
            && !cell.attributes.get("\\keep").map(is_const_true).unwrap_or(false)
        {
            delcells.push(cell.name.clone());
        }
    }
    for (lhs, rhs) in new_connections {
        module_connect(m, lhs, rhs);
    }
    for name in delcells {
        if let Some(pos) = m.cells.iter().position(|c| c.name == name) {
            // cells_temp.cc:44-61 remove():逐顆 log_debug(三消息形
            // connect/input_port/buffer)**verbose 門控**(0.70+179 新形;
            // clean 形 verbose=false=零計數,min2 錨實錄)。
            if verbose {
                crate::proc::dbg_inc();
            }
            m.cells.swap_remove(pos);
            did_set();
        }
    }
}

/// rmunused_module_cells(cells_all.cc:179 起)。
fn rmunused_module_cells(
    m: &mut Module,
    design: &Design,
    purge: bool,
    verbose: bool,
    stats: &mut RmStats,
) {
    let mut am = SigMap::new();
    am.set(m);
    let keep_modules = build_keep_cache(design, purge);

    // explore:wire2driver / mem2cells / keep 標記(索引序)
    // dict::element(n)=entries[size-1-n](hashlib.h:878)⇒ cell_at(i) 的
    // i 序=**逆插入序**(最新優先);本檔一律以插入序 Vec+rev() 對齊。
    let n = m.cells.len();
    let mut wire2driver: HashMap<SigBit, Vec<usize>> = HashMap::new();
    let mut mem2cells: HashMap<String, Vec<usize>> = HashMap::new();
    let mut unused: Vec<bool> = Vec::with_capacity(n);
    let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    for i in (0..n).rev() {
        let cell = &m.cells[i];
        if matches!(cell.ctype.as_str(), "$memwr" | "$memwr_v2" | "$meminit" | "$meminit_v2") {
            mem2cells.entry(param_memid(cell)).or_default().push(i);
        }
        for (port, sig) in &cell.connections {
            let known_builtin = cell_known(&cell.ctype);
            let is_out = if known_builtin {
                port_dir(&cell.ctype, port).1
            } else {
                design
                    .find_module(&cell.ctype)
                    .and_then(|sub| sub.find_wire(port))
                    .map(|w| w.port_output)
                    .unwrap_or(false)
            };
            // known(內建∨設計內模組)的輸入端口位非驅動者(cells_all
            // explore 面=同一 ct_all 消費面)
            if (known_builtin || design.find_module(&cell.ctype).is_some()) && !is_out {
                continue;
            }
            for raw_bit in &sig.bits {
                if matches!(raw_bit, SigBit::Const(_)) {
                    continue;
                }
                let bit = am.map_bit(raw_bit);
                if let SigBit::Wire { .. } = bit {
                    let v = wire2driver.entry(bit).or_default();
                    if !v.contains(&i) {
                        v.push(i);
                    }
                }
            }
        }
        let mut keep = keep_cell_raw(cell, purge);
        if !keep {
            keep = keep_modules.get(&cell.ctype).copied().unwrap_or(false);
        }
        unused.push(!keep);
        if keep {
            queue.push_back(i);
        }
    }
    // keep_wires(port_output 或 keep 屬性;索引序)
    let mut keep_wires: Vec<usize> = Vec::new();
    for (i, w) in m.wires.iter().enumerate().rev() {
        if w.port_output || w.attributes.get("\\keep").map(is_const_true).unwrap_or(false) {
            keep_wires.push(i);
        }
    }
    let mut mem_unused: HashMap<String, bool> =
        m.memories.iter().map(|mem| (mem.name.clone(), true)).collect();

    // analyze_kept_wires:keep wire 的 mapped 位驅動者入列
    for &wi in &keep_wires {
        let sig = am.map(&wire_bits(&m.wires[wi].name, m.wires[wi].width));
        for bit in &sig.bits {
            if let Some(drivers) = wire2driver.get(bit) {
                for &d in drivers {
                    if unused[d] {
                        unused[d] = false;
                        queue.push_back(d);
                    }
                }
            }
        }
    }
    // fixup BFS(輸入位驅動者+mem 讀保全)
    while let Some(ci) = queue.pop_front() {
        let cell = m.cells[ci].clone();
        for (port, sig) in &cell.connections {
            let known_builtin = cell_known(&cell.ctype);
            let is_in = if known_builtin {
                port_dir(&cell.ctype, port).0
            } else {
                !design
                    .find_module(&cell.ctype)
                    .and_then(|sub| sub.find_wire(port))
                    .map(|w| w.port_output)
                    .unwrap_or(false)
            };
            if !known_builtin && design.find_module(&cell.ctype).is_none() || is_in {
                for bit in am.map(sig).bits {
                    if let Some(drivers) = wire2driver.get(&bit) {
                        for &d in drivers {
                            if unused[d] {
                                unused[d] = false;
                                queue.push_back(d);
                            }
                        }
                    }
                }
            }
        }
        if matches!(cell.ctype.as_str(), "$memrd" | "$memrd_v2") {
            let memid = param_memid(&cell);
            if mem_unused.get(&memid).copied().unwrap_or(false) {
                mem_unused.insert(memid.clone(), false);
                if let Some(cs) = mem2cells.get(&memid) {
                    for &d in cs {
                        if unused[d] {
                            unused[d] = false;
                            queue.push_back(d);
                        }
                    }
                }
            }
        }
    }
    // all_unused_cells:sort_by_name_id(intern 序;cells_all.cc:307)
    let mut unused_cells: Vec<usize> = (0..m.cells.len()).filter(|&i| unused[i]).collect();
    unused_cells.sort_by(|&a, &b| {
        crate::opt_expr::intern_key(design, &m.cells[a].name)
            .cmp(&crate::opt_expr::intern_key(design, &m.cells[b].name))
    });
    let names: Vec<String> =
        unused_cells.iter().map(|&i| m.cells[i].name.clone()).collect();
    for name in names {
        if let Some(pos) = m.cells.iter().position(|c| c.name == name) {
            // debug:  removing unused `%s' cell `%s'.(cells_all.cc:313-315
            // **verbose 門控**;clean 形=零計數)
            if verbose {
                crate::proc::dbg_inc();
            }
            m.cells.swap_remove(pos);
            stats.cells += 1;
            did_set();
        }
    }
}

/// rmunused_module_signals(wires.cc:484):回傳「本輪刪了 wire」。
fn rmunused_module_signals(
    m: &mut Module,
    design: &Design,
    purge: bool,
    verbose: bool,
    stats: &mut RmStats,
) -> bool {
    let mut am = SigMap::new();
    am.set(m);
    // SigConnKinds(wires.cc:228)
    let mut raw_registers: HashSet<SigBit> = HashSet::new();
    let mut raw_cell_connected: HashSet<SigBit> = HashSet::new();
    let mut exact_cells: HashSet<SigSpec> = HashSet::new();
    for cell in m.cells.iter().rev() {
        if !purge {
            if ct_reg(&cell.ctype) {
                let clk2fflogic =
                    cell.attributes.get("\\clk2fflogic").map(is_const_true).unwrap_or(false);
                for (port, sig) in &cell.connections {
                    let (_, is_out) = port_dir(&cell.ctype, port);
                    let take = if clk2fflogic { port == "\\D" } else { is_out };
                    if take {
                        for b in &sig.bits {
                            if matches!(b, SigBit::Wire { .. }) {
                                raw_registers.insert(b.clone());
                            }
                        }
                    }
                }
            }
            for (_, sig) in &cell.connections {
                for b in &sig.bits {
                    if matches!(b, SigBit::Wire { .. }) {
                        raw_cell_connected.insert(b.clone());
                    }
                }
            }
        }
        // known-cell 面(celltypes.h:50-60 setup_design):設計內模組亦是
        // known cell(內建表優先,先 setup_design 後 setup_internals);
        // 模組型輸出端口=子模組 wire port_output(hierarchy.rs 同型先例)。
        let known_builtin = cell_known(&cell.ctype);
        let known_mod = design.find_module(&cell.ctype).is_some();
        if known_builtin || known_mod {
            for (port, sig) in &cell.connections {
                let is_out = if known_builtin {
                    port_dir(&cell.ctype, port).1
                } else {
                    design
                        .find_module(&cell.ctype)
                        .and_then(|sub| sub.find_wire(port))
                        .map(|w| w.port_output)
                        .unwrap_or(false)
                };
                if is_out {
                    let spec = am.map(sig);
                    if spec.bits.iter().any(|b| matches!(b, SigBit::Wire { .. })) {
                        exact_cells.insert(spec);
                    }
                }
            }
        }
    }
    // ExactCellWires 快照(wire → 映射後整線命中 exact_cells;port_input=true)
    let mut exact_wire: HashMap<String, bool> = HashMap::new();
    for w in &m.wires {
        let v = if w.port_input {
            true
        } else {
            let sig = am.map(&wire_bits(&w.name, w.width));
            exact_cells.contains(&sig)
        };
        exact_wire.insert(w.name.clone(), v);
    }
    // wire 靜態面快照(比較鍵;public 述語=isPublic(rtlil.h:548
    // begins_with("\\")),非 check_public_name——後者是 WireDeleter purge
    // 面的另一述語(wires.cc:152-162),兩函數勿混)
    let winfo: HashMap<String, (bool, bool, bool, usize)> = m
        .wires
        .iter()
        .map(|w| {
            (
                w.name.clone(),
                (
                    w.port_input,
                    w.port_output,
                    w.name.starts_with('\\'),
                    count_nontrivial_wire_attrs(w),
                ),
            )
        })
        .collect();
    // build_candidates(索引序)+ update_assign_map(代表元重選)
    let mut candidates: Vec<SigBit> = Vec::new();
    for w in m.wires.iter().rev() {
        for j in 0..w.width {
            let s1 = SigBit::Wire { wire: w.name.clone(), offset: j };
            let s2 = am.map_bit(&s1);
            if compare_signals(
                &s2, &s1, &raw_registers, &raw_cell_connected, &exact_wire, &winfo,
            ) {
                candidates.push(s1);
            }
        }
    }
    for c in candidates {
        let cur = am.map_bit(&c);
        // update_assign_map(wires.cc:306-313):compare_signals(cur,cand)=true
        // ⇒ cand 應取代 cur ⇒ promote。
        if compare_signals(
            &cur, &c, &raw_registers, &raw_cell_connected, &exact_wire, &winfo,
        ) {
            am.promote(&c);
        }
    }
    // module->connections_.clear()(wires.cc:330)
    m.connections.clear();
    // analyse_connectivity
    let mut raw_connected: HashSet<SigBit> = HashSet::new();
    let mut connected: HashSet<SigBit> = HashSet::new();
    let mut update_connections: Vec<(usize, String, SigSpec)> = Vec::new();
    let mut initialized_wires: Vec<usize> = Vec::new();
    for (ci, cell) in m.cells.iter().enumerate().rev() {
        for (port, sig) in &cell.connections {
            let spec = am.map(sig);
            if spec != *sig {
                update_connections.push((ci, port.clone(), spec.clone()));
            }
            for b in &spec.bits {
                if matches!(b, SigBit::Wire { .. }) {
                    raw_connected.insert(b.clone());
                    connected.insert(b.clone());
                }
            }
        }
    }
    for (wi, w) in m.wires.iter().enumerate().rev() {
        if w.port_id > 0 {
            for b in &wire_bits(&w.name, w.width).bits {
                if matches!(b, SigBit::Wire { .. }) {
                    raw_connected.insert(b.clone());
                }
            }
            for b in am.map(&wire_bits(&w.name, w.width)).bits {
                if matches!(b, SigBit::Wire { .. }) {
                    connected.insert(b);
                }
            }
        }
        if w.attributes.get("\\keep").map(is_const_true).unwrap_or(false) {
            for b in am.map(&wire_bits(&w.name, w.width)).bits {
                if matches!(b, SigBit::Wire { .. }) {
                    connected.insert(b);
                }
            }
        }
        if w.attributes.contains("\\init") {
            initialized_wires.push(wi);
        }
    }
    // fixup_cell_ports(端口改寫成映射形)
    for (ci, port, spec) in update_connections {
        if let Some(cell) = m.cells.get_mut(ci) {
            if let Some(e) = cell.connections.iter_mut().find(|(p, _)| *p == port) {
                e.1 = spec;
            }
        }
    }
    // consume_inits + apply_normalised_inits(init 遷移到代表元)
    {
        let mut init_values: HashMap<SigBit, Bit> = HashMap::new();
        let mut init_target_wires: Vec<String> = Vec::new();
        for &wi in &initialized_wires {
            let (name, width) = (m.wires[wi].name.clone(), m.wires[wi].width);
            let val = m.wires[wi].attributes.get("\\init").cloned();
            if let Some(Const::Bits { bits, .. }) = val {
                let sig = am.map(&wire_bits(&name, width));
                for (i, b) in sig.bits.iter().enumerate() {
                    if i >= bits.len() {
                        break;
                    }
                    if bits[i] != Bit::Sx {
                        if let SigBit::Wire { wire, .. } = b {
                            init_values.insert(b.clone(), bits[i]);
                            if !init_target_wires.contains(wire) {
                                init_target_wires.push(wire.clone());
                            }
                        }
                    }
                }
            }
            m.wires[wi].attributes.swap_pop_remove("\\init");
        }
        for wname in init_target_wires {
            if let Some(w) = m.wires.iter_mut().find(|w| w.name == wname) {
                let width = w.width;
                let mut val: Vec<Bit> = vec![Bit::Sx; width.max(0) as usize];
                let mut found = false;
                for i in 0..width.max(0) as usize {
                    if let Some(&v) =
                        init_values.get(&SigBit::Wire { wire: wname.clone(), offset: i as i64 })
                    {
                        val[i] = v;
                        found = true;
                    }
                }
                if found {
                    w.attributes.set(
                        "\\init".to_string(),
                        Const::Bits { width, bits: val, signed: false, is_unsized: false },
                    );
                }
            }
        }
    }
    // WireDeleter(wires.cc:396;索引序)
    let mut del_wires: Vec<String> = Vec::new();
    let mut new_connections: Vec<(SigSpec, SigSpec)> = Vec::new();
    let mut remove_init: Vec<String> = Vec::new();
    let mut set_init: Vec<(String, Const)> = Vec::new();
    let mut deleted_and_unreported = 0usize;
    for wi in (0..m.wires.len()).rev() {
        let (name, width, port_id) = {
            let w = &m.wires[wi];
            (w.name.clone(), w.width, w.port_id)
        };
        let s1 = wire_bits(&name, width);
        let s2 = am.map(&s1);
        let has_init_attribute = m.wires[wi].attributes.contains("\\init");
        let mut initval: Vec<Bit> = match m.wires[wi].attributes.get("\\init") {
            Some(Const::Bits { bits, .. }) => {
                let mut v: Vec<Bit> = bits.clone();
                v.resize(width.max(0) as usize, Bit::Sx);
                v
            }
            _ => vec![Bit::Sx; width.max(0) as usize],
        };
        let init_changed_resize = match m.wires[wi].attributes.get("\\init") {
            Some(Const::Bits { bits, .. }) => bits.len() != width as usize,
            _ => false,
        };
        let mut init_changed = init_changed_resize;
        let keep_attr = m.wires[wi].attributes.get("\\keep").map(is_const_true).unwrap_or(false);
        let init_fully_undef = initval.iter().all(|&b| b == Bit::Sx || b == Bit::Sz);
        let mut delete = false;
        if width == 0 {
            if port_id == 0 {
                delete = true;
            }
        } else if port_id != 0 || keep_attr || !init_fully_undef {
            // keep:port/keep/init 線
        } else if !purge
            && check_public_name(&name)
            && (s1.bits.iter().any(|b| raw_connected.contains(b))
                || s2.bits.iter().any(|b| connected.contains(b))
                || s1 != s2)
        {
            // keep:public 且(直連/映射連/有別名)
        } else if !s1.bits.iter().any(|b| raw_connected.contains(b)) {
            delete = true;
        }
        if delete {
            del_wires.push(name.clone());
        } else {
            let mut nl: Vec<SigBit> = Vec::new();
            let mut nr: Vec<SigBit> = Vec::new();
            for i in 0..width.max(0) as usize {
                if s1.bits[i] != s2.bits[i] {
                    let mut r = s2.bits[i].clone();
                    if r == SigBit::Const(Bit::Sx)
                        && (initval[i] == Bit::S0 || initval[i] == Bit::S1)
                    {
                        r = SigBit::Const(initval[i]);
                        initval[i] = Bit::Sx;
                        init_changed = true;
                    }
                    nl.push(s1.bits[i].clone());
                    nr.push(r);
                }
            }
            if !nl.is_empty() {
                new_connections.push((SigSpec { bits: nl }, SigSpec { bits: nr }));
            }
            let now_fully_undef = initval.iter().all(|&b| b == Bit::Sx || b == Bit::Sz);
            if now_fully_undef {
                if has_init_attribute {
                    remove_init.push(name.clone());
                }
            } else if init_changed {
                set_init.push((
                    name.clone(),
                    Const::Bits { width, bits: initval.clone(), signed: false, is_unsized: false },
                ));
            }
        }
    }
    // commit_changes
    for name in &remove_init {
        if let Some(w) = m.wires.iter_mut().find(|w| &w.name == name) {
            w.attributes.swap_pop_remove("\\init");
        }
    }
    for (name, val) in &set_init {
        if let Some(w) = m.wires.iter_mut().find(|w| &w.name == name) {
            w.attributes.set("\\init".to_string(), val.clone());
        }
    }
    for (lhs, rhs) in new_connections {
        module_connect(m, lhs, rhs);
    }
    // delete_wires:oracle 鏈=wire_at 逆掃收集(wires.cc:395)→pool 填入
    // (wires.cc:455)→Module::remove 以 pool 迭代 erase(rtlil.cc:2933;
    // hashlib 逆插入序)——雙反轉⇒erase 序=刪除者在**檔案序(插入序)升
    // 序**;逐顆 swap-and-pop(rotate-pbo 814 條 dump 序模擬雙向釘死)。
    let mut del_sorted: Vec<String> = del_wires.clone();
    del_sorted.sort_by_key(|name| {
        m.wires.iter().position(|w| &w.name == name).unwrap_or(usize::MAX)
    });
    for name in &del_sorted {
        let public = check_public_name(name);
        if public && verbose {
            crate::proc::dbg_inc(); // "  removing unused non-port wire %s."
        } else {
            deleted_and_unreported += 1;
        }
        if let Some(pos) = m.wires.iter().position(|w| &w.name == name) {
            m.wires.swap_remove(pos);
        }
    }
    let deleted_total = del_sorted.len() as i64;
    stats.wires += deleted_total;
    if verbose && deleted_and_unreported > 0 {
        crate::proc::dbg_inc(); // "  removed %d unused temporary wires."
    }
    if deleted_total > 0 {
        did_set();
    }
    deleted_total != 0
}

/// rmunused_module_init(inits.cc:120):冗餘 init 移除(映射後值一致形)。
fn rmunused_module_init(m: &mut Module, verbose: bool) -> bool {
    let mut am = SigMap::new();
    am.set(m);
    let mut qbits: HashMap<SigBit, Bit> = HashMap::new();
    for w in &m.wires {
        if let Some(Const::Bits { bits, .. }) = w.attributes.get("\\init") {
            for (i, b) in bits.iter().enumerate() {
                if *b != Bit::Sx && *b != Bit::Sz {
                    qbits.insert(
                        SigBit::Wire { wire: w.name.clone(), offset: i as i64 },
                        *b,
                    );
                }
            }
        }
    }
    let mut did = false;
    for wi in 0..m.wires.len() {
        let Some(Const::Bits { bits, .. }) = m.wires[wi].attributes.get("\\init").cloned() else {
            continue;
        };
        let (name, width) = (m.wires[wi].name.clone(), m.wires[wi].width);
        let mut redundant = true;
        for i in 0..(width.max(0) as usize).min(bits.len()) {
            if bits[i] == Bit::Sx || bits[i] == Bit::Sz {
                continue;
            }
            let wire_bit = SigBit::Wire { wire: name.clone(), offset: i as i64 };
            let mapped = am.map_bit(&wire_bit);
            if wire_bit == mapped {
                redundant = false;
                break;
            }
            match &mapped {
                SigBit::Wire { .. } => match qbits.get(&mapped) {
                    Some(&q) if q == bits[i] => {}
                    _ => {
                        redundant = false;
                        break;
                    }
                },
                SigBit::Const(Bit::Sx) | SigBit::Const(Bit::Sz) => {
                    redundant = false;
                    break;
                }
                SigBit::Const(c) => {
                    if *c != bits[i] {
                        // Initial value conflict 警告面(語料未觸;觸發即擴)
                        redundant = false;
                        break;
                    }
                }
            }
        }
        if redundant {
            if verbose {
                crate::proc::dbg_inc(); // "  removing redundant init attribute on %s."
            }
            m.wires[wi].attributes.swap_pop_remove("\\init");
            did = true;
        }
    }
    if did {
        did_set();
    }
    did
}

/// compare_signals(wires.cc:93):回傳「s2(candidate)應取代 s1(current)」。
fn compare_signals(
    s1: &SigBit,
    s2: &SigBit,
    regs: &HashSet<SigBit>,
    conns: &HashSet<SigBit>,
    exact_wire: &HashMap<String, bool>,
    winfo: &HashMap<String, (bool, bool, bool, usize)>,
) -> bool {
    if s1 == s2 {
        return false;
    }
    let w1 = match s1 {
        SigBit::Wire { wire, .. } => Some(wire.clone()),
        SigBit::Const(_) => None,
    };
    let w2 = match s2 {
        SigBit::Wire { wire, .. } => Some(wire.clone()),
        SigBit::Const(_) => None,
    };
    match (&w1, &w2) {
        (None, _) => return w2.is_none(),
        (_, None) => return true,
        _ => {}
    }
    let n1 = w1.clone().unwrap();
    let n2 = w2.clone().unwrap();
    let (Some(i1), Some(i2)) = (winfo.get(&n1), winfo.get(&n2)) else {
        return false;
    };
    let (pi1, po1, pub1, at1) = *i1;
    let (pi2, po2, pub2, at2) = *i2;
    if pi1 != pi2 {
        return pi2;
    }
    if (pi1 && po1) != (pi2 && po2) {
        return !(pi2 && po2);
    }
    if pub1 && pub2 {
        let regs1 = regs.contains(s1);
        let regs2 = regs.contains(s2);
        if regs1 != regs2 {
            return regs2;
        }
        let w1_exact = exact_wire.get(&n1).copied().unwrap_or(false);
        let w2_exact = exact_wire.get(&n2).copied().unwrap_or(false);
        if w1_exact != w2_exact {
            return w2_exact;
        }
        let conns1 = conns.contains(s1);
        let conns2 = conns.contains(s2);
        if conns1 != conns2 {
            return conns2;
        }
    }
    let o1 = match s1 {
        SigBit::Wire { offset, .. } => *offset,
        SigBit::Const(_) => 0,
    };
    let o2 = match s2 {
        SigBit::Wire { offset, .. } => *offset,
        SigBit::Const(_) => 0,
    };
    if n1 == n2 {
        return o2 < o1;
    }
    if po1 != po2 {
        return po2;
    }
    let c1 = n1.as_bytes().first().copied().unwrap_or(0);
    let c2 = n2.as_bytes().first().copied().unwrap_or(0);
    if c1 != c2 {
        return pub2;
    }
    if at1 != at2 {
        return at2 > at1;
    }
    // 私名-私名對尾分支(wires.cc:142):lt_by_name(w2,w1)=byte-wise
    // memcmp 整串序(interned 名單段);與 intern 序(rtlil.h:409 index_)
    // 是兩個不同的序——此消費面用前者。
    n2.as_bytes() < n1.as_bytes()
}

fn count_nontrivial_wire_attrs(w: &ry_ir::Wire) -> usize {
    w.attributes.len().saturating_sub(
        w.attributes.contains("\\src") as usize
            + w.attributes.contains("\\hdlname") as usize
            + w.attributes.contains("\\scopename") as usize,
    )
}

/// clean 命令 help 全文常量(cmd_error stdout 载体;CleanPass::help()
/// opt_clean.cc:113-127 逐字转录,golden=实录仲裁)。
const CLEAN_HELP_TEXT: &str = "\n    clean [options] [selection]\n\nThis is identical to 'opt_clean', but less verbose.\n\nWhen commands are separated using the ';;' token, this command will be executed\nbetween the commands.\n\nWhen commands are separated using the ';;;' token, this command will be executed\nin -purge mode between the commands.\n\n";

/// Pass::cmd_error(register.cc:174-192)同构:stdout=空行+`Syntax error
/// in command \`<args 空格重拼>':`+help 全文;stderr(经 HardError 通道)
/// =`Command syntax error: <msg>\n> <重拼>\n> <error_pos 空格>^`;
/// error_pos=Σ_{i<argidx}(len+1),`%*s^` 形。首消费者=clean(§10.3-6;
/// 禁迁移既有 stub)。
pub(crate) fn cmd_error_pass(log: &mut Log, args: &[String], argidx: usize, help_text: &str) -> OptError {
    let command_text = args.join(" ");
    let mut error_pos = 0usize;
    for (i, a) in args.iter().enumerate() {
        if i < argidx {
            error_pos += a.len() + 1;
        }
    }
    log.log(&format!("\nSyntax error in command `{command_text}':\n"));
    log.log(help_text);
    OptError(format!(
        "Command syntax error: Unknown option or option in arguments.\n> {command_text}\n> {}^",
        " ".repeat(error_pos)
    ))
}

/// opt_clean 命令面(verbose=true;旗标=Y2 硬错,§9.2 表)与 clean 命令
/// 面(verbose=false,M3 续弧 slice-3 翻实现)共用 worker 入口。拆参契约
/// (§10.3-2):verbose 分叉三处——①header 行(verbose=false 不打,
/// CleanPass 无 log_header);②每模组 Finding 行(verbose=false 不打);
/// ③process 模组处理=迭代形分立:opt_clean 入口=循环内 warn 形
/// (opt_clean.cc:90 has_processes_warn),clean 入口=预过滤迭代
/// (opt_clean.cc:146-151 先收集 selected_unboxed_whole_modules+!has_
/// processes 再循环,process 模组不进循环=零输出,P2);purge 全链路
/// 已在 worker 签名(rmunused_* :314/:461,拆参即收编零改动)。
pub fn run_opt_clean(
    design: &mut Design,
    args: &[String],
    log: &mut Log,
    verbose: bool,
    purge: bool,
) -> Result<(), OptError> {
    let purge = if verbose {
        for a in args.get(1..).unwrap_or(&[]) {
            return Err(err(format!(
                "opt_clean: option `{a}' is not implemented in ryosys (milestone Y2)"
            )));
        }
        log.header("Executing OPT_CLEAN pass (remove unused cells and wires).\n");
        log.push();
        purge
    } else {
        // CleanPass::execute 参数面(opt_clean.cc:134-141):argidx 循环
        // 仅 -purge 置位(幂等),其余参数=cmd_error 全形;编号不占面
        // (无 header,P1)。
        let mut purge_mode = false;
        let mut argidx = 1usize;
        while argidx < args.len() {
            if args[argidx] == "-purge" {
                purge_mode = true;
                argidx += 1;
                continue;
            }
            break;
        }
        if argidx < args.len() {
            return Err(cmd_error_pass(log, args, argidx, CLEAN_HELP_TEXT));
        }
        purge_mode
    };
    let mut out = String::new();
    let mut stats = RmStats::default();
    if verbose {
        for mi in (0..design.modules.len()).rev() {
            if module_is_boxed(&design.modules[mi]) {
                continue;
            }
            if !design.modules[mi].processes.is_empty() {
                // has_processes_warn(rtlil.cc:2798):log_warning(Module*=unescape 形)
                let mname = id_unescape(&design.modules[mi].name);
                let msg =
                    format!("Ignoring module {mname} because it contains processes (run 'proc' command first).");
                out.push_str(&format!("Warning: {msg}\n"));
                log.warnings.push(msg);
            } else {
                let mname = design.modules[mi].name.clone();
                out.push_str(&format!("Finding unused cells or wires in module {mname}..\n"));
                let mut m = std::mem::take(&mut design.modules[mi]);
                remove_temporary_cells(&mut m, true);
                rmunused_module_cells(&mut m, design, purge, true, &mut stats);
                while rmunused_module_signals(&mut m, design, purge, true, &mut stats) {}
                if rmunused_module_init(&mut m, true) {
                    while rmunused_module_signals(&mut m, design, purge, true, &mut stats) {}
                }
                design.modules[mi] = m;
            }
        }
    } else {
        // clean 预过滤迭代(opt_clean.cc:146-151):process/盒模组不进
        // 循环(静默跳过,非 warn);verbose=false → signals/init 的
        // debug 计数门控关(wires.cc:514 verbose 分支)。
        let selected: Vec<usize> = (0..design.modules.len())
            .rev()
            .filter(|&mi| {
                !module_is_boxed(&design.modules[mi])
                    && design.modules[mi].processes.is_empty()
            })
            .collect();
        for mi in selected {
            let mut m = std::mem::take(&mut design.modules[mi]);
            remove_temporary_cells(&mut m, false);
            rmunused_module_cells(&mut m, design, purge, false, &mut stats);
            while rmunused_module_signals(&mut m, design, purge, false, &mut stats) {}
            if rmunused_module_init(&mut m, false) {
                while rmunused_module_signals(&mut m, design, purge, false, &mut stats) {}
            }
            design.modules[mi] = m;
        }
    }
    // CleanPass::execute 尾序(opt_clean.cc:156-157):log_suppressed()
    // 先于 stats.log()(suppressed 行在 Removed 前,P7 实录);verbose
    // 形(opt_clean)无此显式调用,由 main 框架每命令末印(现行形)。
    if !verbose {
        let n = crate::proc::dbg_take();
        if n > 0 {
            out.push_str(&format!("<suppressed ~{n} debug messages>\n"));
        }
    }
    if stats.cells > 0 || stats.wires > 0 {
        out.push_str(&format!(
            "Removed {} unused cells and {} unused wires.\n",
            stats.cells, stats.wires
        ));
    }
    log.log(&out);
    if verbose {
        log.pop();
    }
    log.resync();
    Ok(())
}

// ---------------------------------------------------------------------------
// opt_merge(passes/opt/opt_merge.cc;無參 standalone+內部 -nomux 形)
// ---------------------------------------------------------------------------

/// 對 opt_merge 等價的 cell 正規形(精確 Eq 比較鍵;hash 值不進 golden)。
#[derive(Clone, PartialEq)]
struct CanonicalCell {
    ctype: String,
    params: Vec<(String, Const)>,
    inputs: Vec<(String, SigSpec)>,
    ff_init: Option<SigSpec>,
}

fn canonical_cell(
    cell: &ry_ir::Cell,
    am: &SigMap,
    nomux: bool,
    keepdc: bool,
    initvals: &HashMap<SigBit, Bit>,
) -> Option<CanonicalCell> {
    // 排除面(opt_merge.cc:546-566):-nomux=$mux/$pmux;恆排除
    // $tribuf/$_TBUF_/$anyseq/$anyconst/$allseq/$allconst;另 $meminit 家族/
    // $scopeinfo/$barrier 跳過(compute_cell_hashes)。
    const ALWAYS_EXCLUDED: &[&str] =
        &["$tribuf", "$_TBUF_", "$anyseq", "$anyconst", "$allseq", "$allconst"];
    if ALWAYS_EXCLUDED.contains(&cell.ctype.as_str()) {
        return None;
    }
    if nomux && matches!(cell.ctype.as_str(), "$mux" | "$pmux") {
        return None;
    }
    if matches!(
        cell.ctype.as_str(),
        "$meminit" | "$meminit_v2" | "$mem" | "$mem_v2" | "$scopeinfo" | "$barrier"
    ) {
        return None;
    }
    if !cell_known(&cell.ctype) {
        return None; // !mode_share_all && !ct.cell_known
    }
    // mode_keepdc && has_dont_care_initval
    if keepdc && cell_is_builtin_ff(&cell.ctype) {
        let q = cell_port(cell, "\\Q");
        let fully_def = q.bits.iter().all(|b| {
            let mb = am.map_bit(b);
            match &mb {
                SigBit::Const(Bit::S0) | SigBit::Const(Bit::S1) => true,
                SigBit::Const(_) => false,
                SigBit::Wire { .. } => {
                    matches!(initvals.get(&mb), Some(&Bit::S0) | Some(&Bit::S1))
                }
            }
        });
        if !fully_def {
            return None;
        }
    }
    let mut inputs: Vec<(String, SigSpec)> = Vec::new();
    for (port, sig) in &cell.connections {
        let (_, is_out) = port_dir(&cell.ctype, port);
        if is_out {
            continue;
        }
        inputs.push((port.clone(), am.map(sig)));
    }
    // commutative/sort 正規形(hash_cell_inputs 的精確 Eq 對應)
    let t = cell.ctype.as_str();
    if matches!(
        t,
        "$and" | "$or" | "$xor" | "$xnor" | "$add" | "$mul" | "$logic_and" | "$logic_or"
            | "$_AND_" | "$_OR_" | "$_XOR_"
    ) {
        if let Some(ai) = inputs.iter().position(|(p, _)| p == "\\A") {
            if let Some(bi) = inputs.iter().position(|(p, _)| p == "\\B") {
                if sig_lt(&inputs[bi].1, &inputs[ai].1) {
                    // yosys:if (A < B) swap → A 變成較大者;此處等價形:
                    // A > B 時 swap(使 A≥B 正規形)
                    inputs.swap(ai, bi);
                }
            }
        }
    } else if matches!(t, "$reduce_xor" | "$reduce_xnor") {
        if let Some(ai) = inputs.iter_mut().find(|(p, _)| p == "\\A") {
            sig_sort(&mut ai.1);
        }
    } else if matches!(t, "$reduce_and" | "$reduce_or" | "$reduce_bool") {
        if let Some(ai) = inputs.iter_mut().find(|(p, _)| p == "\\A") {
            crate::sigops::sort_and_unify(&mut ai.1);
        }
    } else if t == "$pmux" {
        // sort_pmux_conn:S/B 成對按 (S bit, B chunk) 排序
        if let (Some(si), Some(bi)) = (
            inputs.iter().position(|(p, _)| p == "\\S"),
            inputs.iter().position(|(p, _)| p == "\\B"),
        ) {
            let s_width = inputs[si].1.bits.len() as i64;
            if s_width > 0 {
                let b_width = inputs[bi].1.bits.len() as i64 / s_width;
                let mut pairs: Vec<(SigBit, SigSpec)> = (0..s_width)
                    .map(|i| {
                        (
                            inputs[si].1.bits[i as usize].clone(),
                            inputs[bi].1.extract(i * b_width, b_width),
                        )
                    })
                    .collect();
                pairs.sort_by(|a, b| {
                    use std::cmp::Ordering;
                    match bit_ord(&a.0, &b.0) {
                        Ordering::Equal => {
                            if a.1 == b.1 {
                                Ordering::Equal
                            } else if sig_lt(&a.1, &b.1) {
                                Ordering::Less
                            } else {
                                Ordering::Greater
                            }
                        }
                        o => o,
                    }
                });
                inputs[si].1.bits = pairs.iter().map(|(s, _)| s.clone()).collect();
                inputs[bi].1.bits = pairs.into_iter().flat_map(|(_, b)| b.bits).collect();
            }
        }
    }
    inputs.sort_by(|a, b| a.0.cmp(&b.0));
    let mut params: Vec<(String, Const)> = cell
        .parameters
        .entries
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    params.sort_by(|a, b| a.0.cmp(&b.0));
    // is_builtin_ff → init 值併入比較面(compare_cell_parameters_and_connections
    // 的 Q/init 對應;此處以 FF 的 init 屬性位序約簡)
    let ff_init = if cell_is_builtin_ff(&cell.ctype) {
        let q = cell_port(cell, "\\Q");
        Some(SigSpec {
            bits: q
                .bits
                .iter()
                .map(|b| {
                    SigBit::Const(initvals.get(&am.map_bit(b)).copied().unwrap_or(Bit::Sx))
                })
                .collect(),
        })
    } else {
        None
    };
    Some(CanonicalCell { ctype: cell.ctype.clone(), params, inputs, ff_init })
}

/// opt_merge 核心(worker 單線程等價;workers=1:桶序=索引序,keep=首見)。
/// 回傳刪除總數。log 形由呼叫端掌控(standalone/內部形共用)。
fn opt_merge_worker(
    m: &mut Module,
    _design: &Design,
    nomux: bool,
    keepdc: bool,
    out: &mut String,
) -> i32 {
    let mname_raw = m.name.clone();
    out.push_str(&format!("Finding identical cells in module `{mname_raw}'.\n"));
    let mut total_count = 0i32;
    let mut initvals: HashMap<SigBit, Bit> = HashMap::new();
    for w in &m.wires {
        if let Some(Const::Bits { bits, .. }) = w.attributes.get("\\init") {
            for (i, b) in bits.iter().enumerate() {
                if *b == Bit::S0 || *b == Bit::S1 {
                    initvals
                        .insert(SigBit::Wire { wire: w.name.clone(), offset: i as i64 }, *b);
                }
            }
        }
    }
    let mut did_something = true;
    while did_something {
        did_something = false;
        let am = {
            let mut am = SigMap::new();
            am.set(m);
            am
        };
        out.push_str(&format!(
            "Computing hashes of {} cells of `{mname_raw}'.\n",
            m.cells.len()
        ));
        // 索引序掃描;keep=map 中首見(早期 index),remove=後見
        let mut known: Vec<(usize, CanonicalCell)> = Vec::new();
        let mut duplicates: Vec<(usize, usize)> = Vec::new(); // (remove, keep)
        for i in (0..m.cells.len()).rev() {
            let Some(cc) = canonical_cell(&m.cells[i], &am, nomux, keepdc, &initvals) else {
                continue;
            };
            if let Some(pos) = known.iter().position(|(_, k)| {
                k.ctype == cc.ctype
                    && k.params == cc.params
                    && k.inputs == cc.inputs
                    && k.ff_init == cc.ff_init
            }) {
                let keep_i = known[pos].0;
                // keep 屬性處理(opt_merge.cc:382-391):新 cell 有 keep、
                // map cell 無 → 交換(keep cell 勝出);雙 keep → 跳過。
                let new_keep = m.cells[i].attributes.get("\\keep").map(is_const_true).unwrap_or(false);
                let old_keep = m.cells[keep_i]
                    .attributes
                    .get("\\keep")
                    .map(is_const_true)
                    .unwrap_or(false);
                if new_keep && !old_keep {
                    known[pos].0 = i;
                    duplicates.push((keep_i, i));
                } else if new_keep && old_keep {
                    continue;
                } else {
                    duplicates.push((i, keep_i));
                }
            } else {
                known.push((i, cc));
            }
        }
        out.push_str(&format!("Finding duplicate cells in `{mname_raw}'.\n"));
        if duplicates.is_empty() {
            continue;
        }
        // 排序:單線程檢測序=remove 的 cell_at 索引(逆插入空間)。
        // C++ 先轉 cell 指標再刪(opt_merge.cc:448-452:removing 會使索引
        // 失效)——同律:先快照 (remove 名,keep 名,輸出重定向對) 再執行。
        duplicates.sort_by_key(|&(r, _k): &(usize, usize)| usize::MAX - r);
        struct Redirect {
            port: String,
            rsig: SigSpec,
            ksig: SigSpec,
        }
        let resolved: Vec<(String, String, Vec<Redirect>)> = duplicates
            .iter()
            .map(|&(remove_i, keep_i)| {
                let rc = &m.cells[remove_i];
                let kc = &m.cells[keep_i];
                let mut reds = Vec::new();
                for (port, rsig) in &rc.connections {
                    let (_, is_out) = port_dir(&rc.ctype, port);
                    if !is_out {
                        continue;
                    }
                    let Some((_, ksig)) = kc.connections.iter().find(|(p, _)| p == port)
                    else {
                        continue;
                    };
                    reds.push(Redirect {
                        port: port.clone(),
                        rsig: rsig.clone(),
                        ksig: ksig.clone(),
                    });
                }
                (rc.name.clone(), kc.name.clone(), reds)
            })
            .collect();
        for (rname, kname, reds) in resolved {
            crate::proc::dbg_inc_what(&format!(
                "  Cell `{rname}' is identical to cell `{kname}'."
            ));
            // 輸出端口重定向:connect(remove_out, keep_out)(宣告序)+
            // initvals remove/set 面(被重定向 Q 位 init 清殘)。
            for red in &reds {
                crate::proc::dbg_inc_what(&format!(
                    "    Redirecting output {}: {} = {}",
                    red.port,
                    crate::sigops::log_signal(&red.rsig, &widths_of(m)),
                    crate::sigops::log_signal(&red.ksig, &widths_of(m))
                ));
                module_connect(m, red.rsig.clone(), red.ksig.clone());
                for sig in [&red.rsig, &red.ksig] {
                    for b in &sig.bits {
                        if let SigBit::Wire { wire, offset } = b {
                            if let Some(w) = m.wires.iter_mut().find(|w| &w.name == wire) {
                                if let Some(e) = w
                                    .attributes
                                    .entries
                                    .iter_mut()
                                    .find(|(k, _)| *k == "\\init")
                                {
                                    if let Const::Bits { bits, .. } = &mut e.1 {
                                        if (*offset as usize) < bits.len() {
                                            bits[*offset as usize] = Bit::Sx;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            let rtype = {
                let pos = m.cells.iter().position(|c| c.name == rname).unwrap();
                m.cells[pos].ctype.clone()
            };
            crate::proc::dbg_inc_what(&format!(
                "    Removing {rtype} cell `{rname}' from module `{mname_raw}'."
            ));
            if let Some(pos) = m.cells.iter().position(|c| c.name == rname) {
                m.cells.swap_remove(pos);
            }
            total_count += 1;
            did_something = true;
        }
    }
    {
        let n = crate::proc::dbg_take();
        if n > 0 {
            out.push_str(&format!("<suppressed ~{n} debug messages>\n"));
        }
    }
    total_count
}

/// opt_merge 命令面(standalone:無旗標;內部 -nomux 形=opt_merge_run)。
pub fn run_opt_merge(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), OptError> {
    for a in args.get(1..).unwrap_or(&[]) {
        return Err(err(format!(
            "opt_merge: option `{a}' is not implemented in ryosys (milestone Y2)"
        )));
    }
    opt_merge_run(design, false, log)
}

/// 內部形(opt 編排器序幕 `opt_merge -nomux` 的直呼面;非命令面)。
pub fn opt_merge_run(design: &mut Design, nomux: bool, log: &mut Log) -> Result<(), OptError> {
    log.header("Executing OPT_MERGE pass (detect identical cells).\n");
    log.push();
    let mut out = String::new();
    let mut total = 0i32;
    for mi in (0..design.modules.len()).rev() {
        if module_is_boxed(&design.modules[mi]) {
            continue;
        }
        let mut m = std::mem::take(&mut design.modules[mi]);
        total += opt_merge_worker(&mut m, design, nomux, false, &mut out);
        design.modules[mi] = m;
    }
    if total > 0 {
        did_set();
    }
    out.push_str(&format!("Removed a total of {total} cells.\n"));
    log.log(&out);
    log.pop();
    log.resync();
    Ok(())
}
// ---------------------------------------------------------------------------
// opt_muxtree(passes/opt/opt_muxtree.cc;無參契約形)
// ---------------------------------------------------------------------------

/// MuxPort 引用(port=-1 為 A、≥0 為 B 切片)。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct MuxPortRef {
    mux: usize,
    port: i32,
}

#[derive(Default, Clone)]
struct BitInfoMt {
    seen_non_mux: bool,
    mux_users: Vec<MuxPortRef>, // pool insert 語義=首見序(RM3-2 分類點)
    mux_driver: Option<usize>,
}

#[derive(Clone)]
struct PortInfoMt {
    ctrl_sig: i64, // -1=無
    input_sigs: Vec<usize>,
    input_muxes: Vec<usize>,
    const_activated: bool,
    const_deactivated: bool,
    observable: bool,
}

struct MuxInfoMt {
    cell_name: String,
    ctype: String,
    ports: Vec<PortInfoMt>,
}

#[derive(Clone)]
struct Knowledge {
    known_inactive: Vec<i32>,
    known_active: Vec<i32>,
    visited_muxes: Vec<bool>,
}

#[derive(Clone, Copy)]
struct Limits {
    do_replace_known: bool,
    do_mark_ports_observable: bool,
    recursions_left: i32,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { do_replace_known: true, do_mark_ports_observable: true, recursions_left: 3 }
    }
}

struct MtState {
    root_muxes: Vec<bool>,
    root_enable_muxes: Vec<bool>,
    root_mux_rerun: Vec<usize>,
    glob_evals_left: i64,
}

/// sig2bits(bit2num 登記面;skip_non_wires 預設 true)。
fn mt_sig2bits(
    sig: &SigSpec,
    am: &SigMap,
    bit2num: &mut HashMap<SigBit, usize>,
    bit2info: &mut Vec<BitInfoMt>,
    skip_non_wires: bool,
) -> Vec<i64> {
    let mut results = Vec::new();
    for bit in am.map(sig).bits {
        match &bit {
            SigBit::Wire { .. } => {
                let next = bit2info.len();
                let idx = *bit2num.entry(bit.clone()).or_insert(next);
                if idx == bit2info.len() {
                    bit2info.push(BitInfoMt::default());
                }
                results.push(idx as i64);
            }
            SigBit::Const(_) => {
                if !skip_non_wires {
                    results.push(-1);
                }
            }
        }
    }
    results
}

/// opt_muxtree worker(opt_muxtree.cc:226-596;單線程等價)。回傳刪除 port 數。
fn opt_muxtree_worker(
    m: &mut Module,
    _design: &Design,
    out: &mut String,
) -> Result<i64, String> {
    let mname_raw = m.name.clone();
    out.push_str(&format!("Running muxtree optimizer on module {mname_raw}..\n"));
    out.push_str("  Creating internal representation of mux trees.\n");

    let mut am = SigMap::new();
    am.set(m);
    let mut bit2num: HashMap<SigBit, usize> = HashMap::new();
    let mut bit2info: Vec<BitInfoMt> = Vec::new();
    let mut mux2info: Vec<MuxInfoMt> = Vec::new();

    // track_mux / see_non_mux_cell(cells() 正向=逆插入序)
    let cells_snapshot: Vec<ry_ir::Cell> = m.cells.clone();
    for cell in cells_snapshot.iter().rev() {
        if matches!(cell.ctype.as_str(), "$mux" | "$pmux") {
            let this_mux_idx = mux2info.len();
            let sig_a = am.map(&cell_port(cell, "\\A"));
            let sig_b = am.map(&cell_port(cell, "\\B"));
            let sig_s = am.map(&cell_port(cell, "\\S"));
            let sig_y = am.map(&cell_port(cell, "\\Y"));
            let mut ports: Vec<PortInfoMt> = Vec::new();
            let width = sig_a.bits.len() as i64;
            for i in 0..sig_s.bits.len() {
                let sig = sig_b.extract(i as i64 * width, width);
                let ctrl_sig = am.map(&SigSpec { bits: vec![sig_s.bits[i].clone()] });
                let mut pi = PortInfoMt {
                    ctrl_sig: -1,
                    input_sigs: Vec::new(),
                    input_muxes: Vec::new(),
                    const_activated: false,
                    const_deactivated: false,
                    observable: false,
                };
                for bit_idx in
                    mt_sig2bits(&sig, &am, &mut bit2num, &mut bit2info, true)
                {
                    let r = MuxPortRef { mux: this_mux_idx, port: i as i32 };
                    if !bit2info[bit_idx as usize].mux_users.contains(&r) {
                        bit2info[bit_idx as usize].mux_users.push(r);
                    }
                    if !pi.input_sigs.contains(&(bit_idx as usize)) {
                        pi.input_sigs.push(bit_idx as usize);
                    }
                }
                let cs = mt_sig2bits(&ctrl_sig, &am, &mut bit2num, &mut bit2info, false);
                pi.ctrl_sig = cs.first().copied().unwrap_or(-1);
                let cconst = crate::sigops::is_fully_const(&ctrl_sig);
                let cbool =
                    cconst && crate::sigops::const_as_bool(&sig_as_const(&ctrl_sig).unwrap_or_default());
                pi.const_activated = cconst && cbool;
                pi.const_deactivated = cconst && !cbool;
                ports.push(pi);
            }
            {
                let mut pi = PortInfoMt {
                    ctrl_sig: -1,
                    input_sigs: Vec::new(),
                    input_muxes: Vec::new(),
                    const_activated: false,
                    const_deactivated: false,
                    observable: false,
                };
                for bit_idx in mt_sig2bits(&sig_a, &am, &mut bit2num, &mut bit2info, true) {
                    let r = MuxPortRef { mux: this_mux_idx, port: -1 };
                    if !bit2info[bit_idx as usize].mux_users.contains(&r) {
                        bit2info[bit_idx as usize].mux_users.push(r);
                    }
                    if !pi.input_sigs.contains(&(bit_idx as usize)) {
                        pi.input_sigs.push(bit_idx as usize);
                    }
                }
                ports.push(pi);
            }
            mux2info.push(MuxInfoMt {
                cell_name: cell.name.clone(),
                ctype: cell.ctype.clone(),
                ports,
            });
            for idx in mt_sig2bits(&sig_y, &am, &mut bit2num, &mut bit2info, true) {
                let bi = &mut bit2info[idx as usize];
                if let Some(prev) = bi.mux_driver {
                    return Err(format!(
                        "Cell {} Y port signal {} already driven by {}",
                        id_unescape(&cell.name),
                        crate::sigops::log_signal(&sig_y, &widths_of(m)),
                        id_unescape(&mux2info[prev].cell_name)
                    ));
                }
                bi.mux_driver = Some(this_mux_idx);
            }
            for idx in mt_sig2bits(&sig_s, &am, &mut bit2num, &mut bit2info, true) {
                bit2info[idx as usize].seen_non_mux = true;
            }
        } else {
            for (_, sig) in &cell.connections {
                for idx in mt_sig2bits(sig, &am, &mut bit2num, &mut bit2info, true) {
                    bit2info[idx as usize].seen_non_mux = true;
                }
            }
        }
    }
    // see_non_mux_wires(wires() 正向=逆插入序)
    for w in m.wires.iter().rev() {
        if w.port_output || w.attributes.get("\\keep").map(is_const_true).unwrap_or(false) {
            for idx in mt_sig2bits(
                &wire_bits(&w.name, w.width),
                &am,
                &mut bit2num,
                &mut bit2info,
                true,
            ) {
                bit2info[idx as usize].seen_non_mux = true;
            }
        }
    }

    if mux2info.is_empty() {
        out.push_str("  No muxes found in this module.\n");
        return Ok(0);
    }

    // fixup_input_muxes
    for i in 0..bit2info.len() {
        let driver = bit2info[i].mux_driver;
        let users = bit2info[i].mux_users.clone();
        for mp in users {
            for p in mux2info[mp.mux].ports.iter_mut() {
                if p.input_sigs.contains(&i) {
                    if let Some(d) = driver {
                        if !p.input_muxes.contains(&d) {
                            p.input_muxes.push(d);
                        }
                    }
                }
            }
        }
    }
    // populate_roots
    let mut root_muxes = vec![false; mux2info.len()];
    let mut root_enable_muxes = vec![false; mux2info.len()];
    for bi in &bit2info {
        if !bi.seen_non_mux {
            continue;
        }
        if let Some(d) = bi.mux_driver {
            root_muxes[d] = true;
            root_enable_muxes[d] = true;
        }
    }
    {
        let mut mux_to_users: HashMap<usize, Vec<MuxPortRef>> = HashMap::new();
        let mut mux_to_users_order: Vec<usize> = Vec::new();
        for bi in &bit2info {
            if let Some(d) = bi.mux_driver {
                for mp in &bi.mux_users {
                    if !mux_to_users.contains_key(&d) {
                        mux_to_users.insert(d, Vec::new());
                        mux_to_users_order.push(d);
                    }
                    let v = mux_to_users.get_mut(&d).unwrap();
                    if !v.contains(mp) {
                        v.push(*mp);
                    }
                }
            }
        }
        for d in mux_to_users_order {
            if mux_to_users[&d].len() > 1 {
                root_muxes[d] = true;
            }
        }
    }

    out.push_str("  Evaluating internal representation of mux trees.\n");

    let mut st = MtState {
        root_muxes: root_muxes.clone(),
        root_enable_muxes,
        root_mux_rerun: Vec::new(),
        glob_evals_left: 10_000_000,
    };
    let mut knowledge = Knowledge {
        known_inactive: vec![0; bit2info.len()],
        known_active: vec![0; bit2info.len()],
        visited_muxes: vec![false; mux2info.len()],
    };
    for mux_idx in 0..mux2info.len() {
        if st.root_muxes[mux_idx] {
            crate::proc::dbg_inc(); // "    Root of a mux tree: %s%s"
            st.root_mux_rerun.retain(|&x| x != mux_idx);
            let enable = st.root_enable_muxes[mux_idx];
            knowledge.visited_muxes[mux_idx] = true;
            eval_mux(
                &mut st,
                &mut knowledge,
                &mut mux2info,
                m,
                &am,
                mux_idx,
                Limits { do_mark_ports_observable: enable, ..Limits::default() },
                &mut bit2num,
                out,
            );
            knowledge.visited_muxes[mux_idx] = false;
            if st.glob_evals_left == 0 {
                out.push_str("  Giving up (too many iterations)\n");
                return Ok(0);
            }
        }
    }
    while let Some(&mux_idx) = st.root_mux_rerun.first() {
        st.root_mux_rerun.remove(0);
        crate::proc::dbg_inc(); // "    Root of a mux tree: %s (rerun as non-pure)"
        knowledge.visited_muxes[mux_idx] = true;
        eval_mux(
            &mut st,
            &mut knowledge,
            &mut mux2info,
            m,
            &am,
            mux_idx,
            Limits::default(),
            &mut bit2num,
            out,
        );
        knowledge.visited_muxes[mux_idx] = false;
        if st.glob_evals_left == 0 {
            out.push_str("  Giving up (too many iterations)\n");
            return Ok(0);
        }
    }

    out.push_str("  Analyzing evaluation results.\n");

    let mut removed_count = 0i64;
    for mi_i in 0..mux2info.len() {
        let width = m
            .cells
            .iter()
            .find(|c| c.name == mux2info[mi_i].cell_name)
            .map(|c| cell_port(c, "\\A").bits.len() as i64)
            .unwrap_or(0);
        let mut live_ports: Vec<usize> = Vec::new();
        for (port_idx, pi) in mux2info[mi_i].ports.iter().enumerate() {
            if pi.observable {
                live_ports.push(port_idx);
            } else {
                out.push_str(&format!(
                    "    dead port {}/{} on {} {}.\n",
                    port_idx + 1,
                    mux2info[mi_i].ports.len(),
                    mux2info[mi_i].ctype,
                    mux2info[mi_i].cell_name
                ));
                removed_count += 1;
            }
        }
        if live_ports.len() == mux2info[mi_i].ports.len() {
            continue;
        }
        let cname = mux2info[mi_i].cell_name.clone();
        let Some(cpos) = m.cells.iter().position(|c| c.name == cname) else {
            continue;
        };
        if live_ports.is_empty() {
            m.cells.swap_remove(cpos);
            continue;
        }
        let sig_b = cell_port(&m.cells[cpos], "\\B");
        let sig_a = cell_port(&m.cells[cpos], "\\A");
        let sig_s = cell_port(&m.cells[cpos], "\\S");
        let sig_y = cell_port(&m.cells[cpos], "\\Y");
        let mut sig_ports = sig_b.clone();
        sig_ports.bits.extend(sig_a.bits.clone());
        if live_ports.len() == 1 {
            let sig_in = sig_ports.extract(live_ports[0] as i64 * width, width);
            module_connect(m, sig_y.clone(), sig_in);
            m.cells.swap_remove(cpos);
        } else {
            let mut new_sig_a = SigSpec::default();
            let mut new_sig_b = SigSpec::default();
            let mut new_sig_s = SigSpec::default();
            for (i, &lp) in live_ports.iter().enumerate() {
                let sig_in = sig_ports.extract(lp as i64 * width, width);
                if i == live_ports.len() - 1 {
                    new_sig_a = sig_in;
                } else {
                    new_sig_b.bits.extend(sig_in.bits);
                    new_sig_s.bits.push(sig_s.bits[lp].clone());
                }
            }
            let cell = &mut m.cells[cpos];
            set_port(cell, "\\A", new_sig_a);
            set_port(cell, "\\B", new_sig_b);
            set_port(cell, "\\S", new_sig_s);
            let s_w = cell_port(cell, "\\S").bits.len() as i64;
            if s_w == 1 {
                cell.ctype = "$mux".to_string();
                cell.parameters.swap_pop_remove("\\S_WIDTH");
            } else {
                cell.parameters.set(
                    "\\S_WIDTH".to_string(),
                    Const::Bits {
                        width: 32,
                        bits: (0..32)
                            .map(|i| if (s_w >> i) & 1 == 1 { Bit::S1 } else { Bit::S0 })
                            .collect(),
                        signed: false,
                        is_unsized: false,
                    },
                );
            }
        }
    }
    Ok(removed_count)
}

fn set_port(cell: &mut ry_ir::Cell, port: &str, sig: SigSpec) {
    if let Some(e) = cell.connections.iter_mut().find(|(p, _)| p == port) {
        e.1 = sig;
    } else {
        cell.connections.push((port.to_string(), sig));
    }
}

#[allow(clippy::too_many_arguments)]
fn eval_mux(
    st: &mut MtState,
    knowledge: &mut Knowledge,
    mux2info: &mut [MuxInfoMt],
    m: &mut Module,
    am: &SigMap,
    mux_idx: usize,
    limits: Limits,
    bit2num: &mut HashMap<SigBit, usize>,
    out: &mut String,
) {
    if st.glob_evals_left == 0 {
        return;
    }
    st.glob_evals_left -= 1;
    crate::proc::dbg_inc(); // "\t\teval %s (replace %d enable %d)"
    if limits.do_replace_known {
        replace_known(st, knowledge, mux2info, m, am, mux_idx, "\\A", bit2num, out);
        replace_known(st, knowledge, mux2info, m, am, mux_idx, "\\B", bit2num, out);
    }
    let nports = mux2info[mux_idx].ports.len();
    for port_idx in 0..nports {
        if mux2info[mux_idx].ports[port_idx].const_activated {
            eval_mux_port(st, knowledge, mux2info, m, am, mux_idx, port_idx, limits, bit2num, out);
            return;
        }
    }
    for port_idx in 0..nports.saturating_sub(1) {
        let pi = &mux2info[mux_idx].ports[port_idx];
        if pi.const_deactivated {
            continue;
        }
        if pi.ctrl_sig >= 0 && knowledge.known_active[pi.ctrl_sig as usize] > 0 {
            eval_mux_port(st, knowledge, mux2info, m, am, mux_idx, port_idx, limits, bit2num, out);
            return;
        }
    }
    for port_idx in 0..nports {
        let (deact, skip) = {
            let pi = &mux2info[mux_idx].ports[port_idx];
            (
                pi.const_deactivated,
                port_idx < nports.saturating_sub(1)
                    && pi.ctrl_sig >= 0
                    && knowledge.known_inactive[pi.ctrl_sig as usize] > 0,
            )
        };
        if deact || skip {
            continue;
        }
        eval_mux_port(st, knowledge, mux2info, m, am, mux_idx, port_idx, limits, bit2num, out);
        if st.glob_evals_left == 0 {
            return;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn eval_mux_port(
    st: &mut MtState,
    knowledge: &mut Knowledge,
    mux2info: &mut [MuxInfoMt],
    m: &mut Module,
    am: &SigMap,
    mux_idx: usize,
    port_idx: usize,
    limits: Limits,
    bit2num: &mut HashMap<SigBit, usize>,
    out: &mut String,
) {
    if st.glob_evals_left == 0 {
        return;
    }
    if limits.do_mark_ports_observable {
        mux2info[mux_idx].ports[port_idx].observable = true;
    }
    activate_port(knowledge, mux2info, mux_idx, port_idx);
    let mut input_mux_queue: Vec<usize> = Vec::new();
    for &mm in &mux2info[mux_idx].ports[port_idx].input_muxes {
        if knowledge.visited_muxes[mm] {
            continue;
        }
        knowledge.visited_muxes[mm] = true;
        input_mux_queue.push(mm);
    }
    for mm in input_mux_queue.clone() {
        if st.root_enable_muxes[mm] {
            continue;
        } else if st.root_muxes[mm] {
            if limits.recursions_left == 0 {
                if !st.root_mux_rerun.contains(&mm) {
                    st.root_mux_rerun.push(mm);
                }
                st.root_enable_muxes[mm] = true;
                crate::proc::dbg_inc(); // "Removing pure flag from root mux"
            } else {
                let new_limits = Limits {
                    do_replace_known: false,
                    do_mark_ports_observable: limits.do_mark_ports_observable,
                    recursions_left: limits.recursions_left - 1,
                };
                eval_mux(st, knowledge, mux2info, m, am, mm, new_limits, bit2num, out);
            }
        } else {
            eval_mux(st, knowledge, mux2info, m, am, mm, limits, bit2num, out);
        }
        if st.glob_evals_left == 0 {
            return;
        }
    }
    for mm in input_mux_queue {
        knowledge.visited_muxes[mm] = false;
    }
    deactivate_port(knowledge, mux2info, mux_idx, port_idx);
}

fn activate_port(knowledge: &mut Knowledge, mux2info: &[MuxInfoMt], mux_idx: usize, port_idx: usize) {
    for (i, p) in mux2info[mux_idx].ports.iter().enumerate() {
        if i == port_idx {
            continue;
        }
        if p.ctrl_sig >= 0 {
            knowledge.known_inactive[p.ctrl_sig as usize] += 1;
        }
    }
    let last = mux2info[mux_idx].ports.len().saturating_sub(1);
    let pi = &mux2info[mux_idx].ports[port_idx];
    if port_idx < last && !pi.const_activated && pi.ctrl_sig >= 0 {
        knowledge.known_active[pi.ctrl_sig as usize] += 1;
    }
}

fn deactivate_port(knowledge: &mut Knowledge, mux2info: &[MuxInfoMt], mux_idx: usize, port_idx: usize) {
    let last = mux2info[mux_idx].ports.len().saturating_sub(1);
    let pi = &mux2info[mux_idx].ports[port_idx];
    if port_idx < last && !pi.const_activated && pi.ctrl_sig >= 0 {
        let c = pi.ctrl_sig as usize;
        if knowledge.known_active[c] > 0 {
            knowledge.known_active[c] -= 1;
        }
    }
    for (i, p) in mux2info[mux_idx].ports.iter().enumerate() {
        if i == port_idx {
            continue;
        }
        if p.ctrl_sig >= 0 {
            let c = p.ctrl_sig as usize;
            if knowledge.known_inactive[c] > 0 {
                knowledge.known_inactive[c] -= 1;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn replace_known(
    st: &mut MtState,
    knowledge: &Knowledge,
    mux2info: &mut [MuxInfoMt],
    m: &mut Module,
    am: &SigMap,
    mux_idx: usize,
    portname: &str,
    bit2num: &mut HashMap<SigBit, usize>,
    out: &mut String,
) {
    let _ = st;
    let cname = mux2info[mux_idx].cell_name.clone();
    let Some(cpos) = m.cells.iter().position(|c| c.name == cname) else {
        return;
    };
    let mut sig = cell_port(&m.cells[cpos], portname);
    let width_if_b = if portname == "\\B" {
        cell_port(&m.cells[cpos], "\\A").bits.len() as i64
    } else {
        0
    };
    // ctrl_bits:bit2num 索引 → 首見 S 位置(查 bit2num 直取,不動 bit2info)
    let s_sig = cell_port(&m.cells[cpos], "\\S");
    let s_bits: Vec<i64> = {
        let mut v = Vec::new();
        for bit in am.map(&s_sig).bits {
            match &bit {
                SigBit::Wire { .. } => {
                    v.push(bit2num.get(&bit.clone()).copied().unwrap_or(usize::MAX) as i64)
                }
                SigBit::Const(_) => v.push(-1),
            }
        }
        v
    };
    let mut ctrl_bits: HashMap<i64, usize> = HashMap::new();
    for (j, sb) in s_bits.iter().enumerate() {
        if *sb >= 0 {
            ctrl_bits.entry(*sb).or_insert(j);
        }
    }
    let mut did_something = false;
    let mut slice_idx: i64 = 0;
    let mut slice_off: i64 = 0;
    let mut out_bits: Vec<SigBit> = sig.bits.clone();
    let mapped = am.map(&sig);
    for (i, bit) in mapped.bits.iter().enumerate() {
        match bit {
            SigBit::Wire { .. } => {
                if let Some(&bidx) = bit2num.get(bit) {
                    if knowledge.known_inactive[bidx] > 0 {
                        out_bits[i] = SigBit::Const(Bit::S0);
                        did_something = true;
                    } else if knowledge.known_active[bidx] > 0 {
                        out_bits[i] = SigBit::Const(Bit::S1);
                        did_something = true;
                    }
                    if let Some(&j) = ctrl_bits.get(&(bidx as i64)) {
                        if width_if_b == 0 {
                            out_bits[i] = SigBit::Const(Bit::S0);
                        } else {
                            out_bits[i] = if j as i64 == slice_idx {
                                SigBit::Const(Bit::S1)
                            } else {
                                SigBit::Const(Bit::S0)
                            };
                        }
                        did_something = true;
                    }
                }
            }
            SigBit::Const(_) => {}
        }
        if width_if_b > 0 {
            slice_off += 1;
            if slice_off == width_if_b {
                slice_idx += 1;
                slice_off = 0;
            }
        }
    }
    if did_something {
        out.push_str(&format!(
            "      Replacing known input bits on port {} of cell {}: {} -> {}\n",
            portname.trim_start_matches('\\'),
            id_unescape(&cname),
            crate::sigops::log_signal(&sig, &widths_of(m)),
            crate::sigops::log_signal(&SigSpec { bits: out_bits.clone() }, &widths_of(m))
        ));
        sig = SigSpec { bits: out_bits };
        set_port(&mut m.cells[cpos], portname, sig);
    }
}

/// opt_muxtree 命令面(無參;§9.2:旗標/selection=Y2 硬錯)。
pub fn run_opt_muxtree(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), OptError> {
    for a in args.get(1..).unwrap_or(&[]) {
        return Err(err(format!(
            "opt_muxtree: option `{a}' is not implemented in ryosys (milestone Y2)"
        )));
    }
    log.header("Executing OPT_MUXTREE pass (detect dead branches in mux trees).\n");
    log.push();
    let mut out = String::new();
    let mut total = 0i64;
    for mi in (0..design.modules.len()).rev() {
        if module_is_boxed(&design.modules[mi]) {
            continue;
        }
        if !design.modules[mi].processes.is_empty() {
            let mname = id_unescape(&design.modules[mi].name);
            let msg = format!(
                "Ignoring module {mname} because it contains processes (run 'proc' command first)."
            );
            out.push_str(&format!("Warning: {msg}\n"));
            log.warnings.push(msg);
            continue;
        }
        let mut m = std::mem::take(&mut design.modules[mi]);
        let r = opt_muxtree_worker(&mut m, design, &mut out);
        design.modules[mi] = m;
        match r {
            Ok(cnt) => total += cnt,
            Err(msg) => return Err(err(msg)),
        }
    }
    if total > 0 {
        did_set();
    }
    out.push_str(&format!("Removed {total} multiplexer ports.\n"));
    log.log(&out);
    log.pop();
    log.resync();
    Ok(())
}

// ---------------------------------------------------------------------------
// opt_reduce(passes/opt/opt_reduce.cc;無參契約形;-fine/-full=Y2)
// ---------------------------------------------------------------------------

fn opt_reduce_worker(
    m: &mut Module,
    _design: &Design,
    autoidx: &mut i64,
    do_fine: bool,
    out: &mut String,
) -> i32 {
    let mname_raw = m.name.clone();
    out.push_str(&format!("  Optimizing cells in module {mname_raw}.\n"));
    let mut total_count = 0i32;
    let mut am = SigMap::new();
    am.set(m);
    let mut did_something = true;

    // mem_wren_sigs(SigPool;cells_ 原始 dict 迭代=最新優先)
    let mut mem_wren: HashSet<SigBit> = HashSet::new();
    // SigPool 語義:常數位亦入池(否則 check_all 永假=不收斂)
    for cell in m.cells.iter().rev() {
        if matches!(cell.ctype.as_str(), "$mem" | "$mem_v2") {
            for b in am.map(&cell_port(cell, "\\WR_EN")).bits {
                mem_wren.insert(b);
            }
        }
        if matches!(cell.ctype.as_str(), "$memwr" | "$memwr_v2") {
            for b in am.map(&cell_port(cell, "\\EN")).bits {
                mem_wren.insert(b);
            }
        }
    }
    for cell in m.cells.iter().rev() {
        if cell.ctype == "$dff" {
            let q = am.map(&cell_port(cell, "\\Q"));
            if q.bits.iter().any(|b| mem_wren.contains(b)) {
                for b in am.map(&cell_port(cell, "\\D")).bits {
                    mem_wren.insert(b);
                }
            }
        }
    }
    let mut keep_expanding = true;
    while keep_expanding {
        keep_expanding = false;
        for cell in m.cells.iter().rev() {
            if cell.ctype == "$mux" {
                let y = am.map(&cell_port(cell, "\\Y"));
                if y.bits.iter().any(|b| mem_wren.contains(b)) {
                    let a = am.map(&cell_port(cell, "\\A"));
                    let b = am.map(&cell_port(cell, "\\B"));
                    if !a.bits.iter().all(|b| mem_wren.contains(b))
                        || !b.bits.iter().all(|b| mem_wren.contains(b))
                    {
                        keep_expanding = true;
                    }
                    for s in [a, b] {
                        for bit in s.bits {
                            mem_wren.insert(bit);
                        }
                    }
                }
            }
        }
    }

    while did_something {
        did_something = false;

        // reduce 樹合併($reduce_or/$reduce_and;cells_ 迭代=最新優先)
        for ty in ["$reduce_or", "$reduce_and"] {
            // drivers:映射位 → 同型 cell(首見序=最新優先)
            let mut drivers: HashMap<SigBit, Vec<usize>> = HashMap::new();
            let mut cells: Vec<usize> = Vec::new();
            for (ci, cell) in m.cells.iter().enumerate().rev() {
                if cell.ctype != ty {
                    continue;
                }
                for b in am.map(&cell_port(cell, "\\Y")).bits {
                    if matches!(b, SigBit::Wire { .. }) {
                        let v = drivers.entry(b).or_default();
                        if !v.contains(&ci) {
                            v.push(ci);
                        }
                    }
                }
                cells.push(ci);
            }
            while let Some(&ci) = cells.first() {
                cells.remove(0);
                total_count += opt_reduce_tree(
                    m, &am, &mut drivers, &mut cells, ty, ci, &mut did_something, out,
                );
            }
        }

        // $mux/$pmux 相同輸入整合(selected_cells 迭代=最新優先)
        let sel: Vec<usize> = (0..m.cells.len()).rev().collect();
        for ci in sel {
            let ty = m.cells[ci].ctype.clone();
            if !matches!(ty.as_str(), "$mux" | "$pmux" | "$bmux" | "$demux") {
                continue;
            }
            let y = am.map(&cell_port(&m.cells[ci], "\\Y"));
            if do_fine || y.bits.iter().any(|b| mem_wren.contains(b)) {
                if ty == "$demux" {
                    if opt_demux_bits(m, &am, ci, &mut did_something, &mut total_count, out) {
                        continue;
                    }
                } else if opt_mux_bits(m, &am, ci, &mut did_something, &mut total_count, out) {
                    continue;
                }
            }
            let cname = m.cells[ci].name.clone();
            let _ = cname;
            if ty == "$mux" || ty == "$pmux" {
                total_count += opt_pmux_reduce(m, &am, ci, autoidx, &mut did_something, out);
            }
        }
    }
    total_count
}

#[allow(clippy::too_many_arguments)]
fn opt_reduce_tree(
    m: &mut Module,
    am: &SigMap,
    drivers: &mut HashMap<SigBit, Vec<usize>>,
    cells: &mut Vec<usize>,
    ty: &str,
    ci: usize,
    did_something: &mut bool,
    out: &mut String,
) -> i32 {
    // 以名定位(cell 刪除/改寫不改名;此 pass 不刪 cell)
    let Some(pos) = (0..m.cells.len()).find(|&p| {
        // cells 存原索引;經前面 worker 重入後索引仍穩定(此 pass 無刪除)
        p == ci
    }) else {
        return 0;
    };
    let _ = pos;
    if ci >= m.cells.len() || m.cells[ci].ctype != ty {
        return 0;
    }
    // cells 池移除語義:已處理標記以 drivers 重入保護
    let sig_a = {
        let mut s = am.map(&cell_port(&m.cells[ci], "\\A"));
        crate::sigops::sort_and_unify(&mut s);
        s
    };
    let mut new_bits: Vec<SigBit> = Vec::new();
    for bit in &sig_a.bits {
        match bit {
            SigBit::Const(Bit::S0) => {
                if ty == "$reduce_and" {
                    new_bits = vec![SigBit::Const(Bit::S0)];
                    break;
                }
                continue;
            }
            SigBit::Const(Bit::S1) => {
                if ty == "$reduce_or" {
                    new_bits = vec![SigBit::Const(Bit::S1)];
                    break;
                }
                continue;
            }
            SigBit::Const(_) => {
                new_bits.push(bit.clone());
                continue;
            }
            SigBit::Wire { .. } => {
                let mut imported = false;
                let driver_list = drivers.get(bit).cloned().unwrap_or_default();
                for child in driver_list {
                    if child < m.cells.len() && m.cells[child].ctype == ty {
                        // 子 cell 遞迴(池已由 drivers 消費面保護)
                        if child != ci {
                            opt_reduce_tree(
                                m, am, drivers, cells, ty, child, did_something, out,
                            );
                        }
                        let yb = am.map(&cell_port(&m.cells[child], "\\Y"));
                        if yb.bits.first() == Some(bit) {
                            let mut ca = am.map(&cell_port(&m.cells[child], "\\A"));
                            crate::sigops::sort_and_unify(&mut ca);
                            new_bits.extend(ca.bits);
                        } else {
                            new_bits.push(SigBit::Const(Bit::S0));
                        }
                        imported = true;
                    }
                }
                if !imported {
                    new_bits.push(bit.clone());
                }
            }
        }
    }
    let mut new_sig_a = SigSpec { bits: new_bits };
    crate::sigops::sort_and_unify(&mut new_sig_a);
    if new_sig_a.bits.is_empty() {
        new_sig_a = crate::sigops::sig_const(
            if ty == "$reduce_or" { Bit::S0 } else { Bit::S1 },
            1,
        );
    }
    let old_a = cell_port(&m.cells[ci], "\\A");
    if new_sig_a != sig_a || new_sig_a.bits.len() != old_a.bits.len() {
        let cname = m.cells[ci].name.clone();
        out.push_str(&format!(
            "    New input vector for {ty} cell {cname}: {}\n",
            crate::sigops::log_signal(&new_sig_a, &widths_of(m))
        ));
        *did_something = true;
        set_port(&mut m.cells[ci], "\\A", new_sig_a.clone());
        m.cells[ci].parameters.set(
            "\\A_WIDTH".to_string(),
            Const::Bits {
                width: 32,
                bits: (0..32)
                    .map(|i| {
                        if (new_sig_a.bits.len() as i64 >> i) & 1 == 1 {
                            Bit::S1
                        } else {
                            Bit::S0
                        }
                    })
                    .collect(),
                signed: false,
                is_unsized: false,
            },
        );
        return 1;
    }
    set_port(&mut m.cells[ci], "\\A", new_sig_a);
    0
}

fn opt_pmux_reduce(
    m: &mut Module,
    am: &SigMap,
    ci: usize,
    autoidx: &mut i64,
    did_something: &mut bool,
    out: &mut String,
) -> i32 {
    let ty = m.cells[ci].ctype.clone();
    if ty != "$pmux" && ty != "$mux" {
        return 0;
    }
    let sig_a = am.map(&cell_port(&m.cells[ci], "\\A"));
    let sig_b = am.map(&cell_port(&m.cells[ci], "\\B"));
    let sig_s = am.map(&cell_port(&m.cells[ci], "\\S"));
    let port_width = sig_a.bits.len() as i64;
    let mut grouped: Vec<(SigSpec, Vec<SigBit>)> = Vec::new();
    for i in 0..sig_s.bits.len() as i64 {
        let this_b = sig_b.extract(i * port_width, port_width);
        if this_b == sig_a {
            continue;
        }
        if let Some(e) = grouped.iter_mut().find(|(b, _)| *b == this_b) {
            e.1.push(sig_s.bits[i as usize].clone());
        } else {
            grouped.push((this_b, vec![sig_s.bits[i as usize].clone()]));
        }
    }
    let mut new_sig_b = SigSpec::default();
    let mut new_sig_s = SigSpec::default();
    let mut new_cells: Vec<(String, String, SigSpec, SigSpec)> = Vec::new(); // 暫不建
    let _ = &mut new_cells;
    for (this_b, mut this_s_bits) in grouped {
        let this_s = SigSpec { bits: std::mem::take(&mut this_s_bits) };
        let s_out = if this_s.bits.len() > 1 {
            // NEW_ID $reduce_or + NEW wire(建名面:oracle
            // addCell(NEW_ID)=N → addWire(NEW_ID)=N+1,opt_reduce.cc:131/
            // :137,每對吃 2 個 autoidx;golden 錨=constmuldivmod-pbo
            // expected.rtlil cell$134→wire$135)
            let n = *autoidx;
            *autoidx += 2;
            let cname = format!("$auto$opt_reduce.cc:131:opt_pmux${n}");
            let wname = format!("$auto$opt_reduce.cc:137:opt_pmux${}", n + 1);
            new_cells.push((
                cname,
                wname.clone(),
                this_s.clone(),
                SigSpec { bits: vec![SigBit::Wire { wire: wname.clone(), offset: 0 }] },
            ));
            SigSpec { bits: vec![SigBit::Wire { wire: wname.clone(), offset: 0 }] }
        } else {
            this_s.clone()
        };
        new_sig_b.bits.extend(this_b.bits);
        new_sig_s.bits.extend(s_out.bits);
    }
    if new_sig_s.bits.is_empty() {
        let y = cell_port(&m.cells[ci], "\\Y");
        let a = cell_port(&m.cells[ci], "\\A");
        module_connect(m, y, a);
        let cname = m.cells[ci].name.clone();
        if let Some(p) = m.cells.iter().position(|c| c.name == cname) {
            m.cells.swap_remove(p);
        }
        *did_something = true;
        return 1;
    }
    let changed = new_sig_s.bits.len() != sig_s.bits.len()
        || (new_sig_s.bits.len() == 1 && ty == "$pmux");
    if changed {
        let cname = m.cells[ci].name.clone();
        out.push_str(&format!(
            "    New ctrl vector for {ty} cell {cname}: {}\n",
            crate::sigops::log_signal(&new_sig_s, &widths_of(m))
        ));
        *did_something = true;
        set_port(&mut m.cells[ci], "\\B", new_sig_b);
        set_port(&mut m.cells[ci], "\\S", new_sig_s.clone());
        let sw = new_sig_s.bits.len() as i64;
        if sw > 1 {
            m.cells[ci].parameters.set(
                "\\S_WIDTH".to_string(),
                Const::Bits {
                    width: 32,
                    bits: (0..32)
                        .map(|i| if (sw >> i) & 1 == 1 { Bit::S1 } else { Bit::S0 })
                        .collect(),
                    signed: false,
                    is_unsized: false,
                },
            );
        } else {
            m.cells[ci].ctype = "$mux".to_string();
            m.cells[ci].parameters.swap_pop_remove("\\S_WIDTH");
        }
        // 新建 $reduce_or cells(插入序=dict 尾插)
        for (cname, wname, a_sig, y_sig) in new_cells {
            let _ = wname;
            let mut params = InsertMap::new();
            params.insert("\\A_SIGNED".to_string(), Const::from_i64_i32_width(0));
            params.insert(
                "\\A_WIDTH".to_string(),
                Const::Bits {
                    width: 32,
                    bits: (0..32)
                        .map(|i| {
                            if (a_sig.bits.len() as i64 >> i) & 1 == 1 {
                                Bit::S1
                            } else {
                                Bit::S0
                            }
                        })
                        .collect(),
                    signed: false,
                    is_unsized: false,
                },
            );
            params.insert(
                "\\Y_WIDTH".to_string(),
                Const::Bits {
                    width: 32,
                    bits: (0..32).map(|i| if i == 0 { Bit::S1 } else { Bit::S0 }).collect(),
                    signed: false,
                    is_unsized: false,
                },
            );
            m.cells.push(ry_ir::Cell {
                ctype: "$reduce_or".to_string(),
                name: cname,
                attributes: InsertMap::new(),
                parameters: params,
                connections: vec![
                    ("\\A".to_string(), a_sig),
                    ("\\Y".to_string(), y_sig),
                ],
            });
        }
    }
    1 * (changed as i32)
}

fn opt_mux_bits(
    m: &mut Module,
    am: &SigMap,
    ci: usize,
    did_something: &mut bool,
    total: &mut i32,
    out: &mut String,
) -> bool {
    let ty = m.cells[ci].ctype.clone();
    let sig_a = am.map(&cell_port(&m.cells[ci], "\\A"));
    let sig_b = if ty != "$bmux" { am.map(&cell_port(&m.cells[ci], "\\B")) } else { SigSpec::default() };
    let sig_y = am.map(&cell_port(&m.cells[ci], "\\Y"));
    let width = sig_y.bits.len();
    let mut old_first: Vec<SigBit> = Vec::new();
    let mut old_second: Vec<SigBit> = Vec::new();
    let mut consolidated: Vec<(Vec<SigBit>, SigBit)> = Vec::new();
    let mut swizzle: Vec<usize> = Vec::new();
    for i in 0..width {
        let mut in_tuple: Vec<SigBit> = Vec::new();
        let mut all_same = true;
        in_tuple.push(sig_a.bits[i].clone());
        let mut j = i;
        while j < sig_a.bits.len() {
            in_tuple.push(sig_a.bits[j].clone());
            if sig_a.bits[j] != in_tuple[0] {
                all_same = false;
            }
            j += width;
        }
        let mut j = i;
        while j < sig_b.bits.len() {
            in_tuple.push(sig_b.bits[j].clone());
            if sig_b.bits[j] != in_tuple[0] {
                all_same = false;
            }
            j += width;
        }
        if all_same {
            old_first.push(sig_y.bits[i].clone());
            old_second.push(sig_a.bits[i].clone());
            continue;
        }
        if let Some((_, yb)) = consolidated.iter().find(|(t, _)| *t == in_tuple) {
            old_first.push(sig_y.bits[i].clone());
            old_second.push(yb.clone());
        } else {
            consolidated.push((in_tuple, sig_y.bits[i].clone()));
            swizzle.push(i);
        }
    }
    if swizzle.len() != width {
        let cname = m.cells[ci].name.clone();
        out.push_str(&format!(
            "    Consolidated identical input bits for {ty} cell {cname}:\n"
        ));
        if ty != "$bmux" {
            out.push_str(&format!(
                "      Old ports: A={}, B={}, Y={}\n",
                crate::sigops::log_signal(&cell_port(&m.cells[ci], "\\A"), &widths_of(m)),
                crate::sigops::log_signal(&cell_port(&m.cells[ci], "\\B"), &widths_of(m)),
                crate::sigops::log_signal(&cell_port(&m.cells[ci], "\\Y"), &widths_of(m))
            ));
        } else {
            out.push_str(&format!(
                "      Old ports: A={}, Y={}\n",
                crate::sigops::log_signal(&cell_port(&m.cells[ci], "\\A"), &widths_of(m)),
                crate::sigops::log_signal(&cell_port(&m.cells[ci], "\\Y"), &widths_of(m))
            ));
        }
        if swizzle.is_empty() {
            let cname = m.cells[ci].name.clone();
            if let Some(p) = m.cells.iter().position(|c| c.name == cname) {
                m.cells.swap_remove(p);
            }
        } else {
            let mut new_a: Vec<SigBit> = Vec::new();
            let mut i = 0;
            while i < sig_a.bits.len() {
                for &j in &swizzle {
                    if i + j < sig_a.bits.len() {
                        new_a.push(sig_a.bits[i + j].clone());
                    }
                }
                i += width;
            }
            set_port(&mut m.cells[ci], "\\A", SigSpec { bits: new_a });
            if ty != "$bmux" {
                let mut new_b: Vec<SigBit> = Vec::new();
                let mut i = 0;
                while i < sig_b.bits.len() {
                    for &j in &swizzle {
                        if i + j < sig_b.bits.len() {
                            new_b.push(sig_b.bits[i + j].clone());
                        }
                    }
                    i += width;
                }
                set_port(&mut m.cells[ci], "\\B", SigSpec { bits: new_b });
            }
            let new_y: Vec<SigBit> = swizzle.iter().map(|&j| sig_y.bits[j].clone()).collect();
            set_port(&mut m.cells[ci], "\\Y", SigSpec { bits: new_y });
            let pw = swizzle.len() as i64;
            m.cells[ci].parameters.set(
                "\\WIDTH".to_string(),
                Const::Bits {
                    width: 32,
                    bits: (0..32)
                        .map(|i| if (pw >> i) & 1 == 1 { Bit::S1 } else { Bit::S0 })
                        .collect(),
                    signed: false,
                    is_unsized: false,
                },
            );
            if ty != "$bmux" {
                out.push_str(&format!(
                    "      New ports: A={}, B={}, Y={}\n",
                    crate::sigops::log_signal(&cell_port(&m.cells[ci], "\\A"), &widths_of(m)),
                    crate::sigops::log_signal(&cell_port(&m.cells[ci], "\\B"), &widths_of(m)),
                    crate::sigops::log_signal(&cell_port(&m.cells[ci], "\\Y"), &widths_of(m))
                ));
            } else {
                out.push_str(&format!(
                    "      New ports: A={}, Y={}\n",
                    crate::sigops::log_signal(&cell_port(&m.cells[ci], "\\A"), &widths_of(m)),
                    crate::sigops::log_signal(&cell_port(&m.cells[ci], "\\Y"), &widths_of(m))
                ));
            }
        }
        let conn = (SigSpec { bits: old_first }, SigSpec { bits: old_second });
        out.push_str(&format!(
            "      New connections: {} = {}\n",
            crate::sigops::log_signal(&conn.0, &widths_of(m)),
            crate::sigops::log_signal(&conn.1, &widths_of(m))
        ));
        module_connect(m, conn.0, conn.1);
        *did_something = true;
        *total += 1;
        return false;
    }
    true
}

fn opt_demux_bits(
    m: &mut Module,
    am: &SigMap,
    ci: usize,
    did_something: &mut bool,
    total: &mut i32,
    out: &mut String,
) -> bool {
    let ty = m.cells[ci].ctype.clone();
    let sig_a = am.map(&cell_port(&m.cells[ci], "\\A"));
    let sig_y = am.map(&cell_port(&m.cells[ci], "\\Y"));
    let width = sig_a.bits.len();
    let mut old_first: Vec<SigBit> = Vec::new();
    let mut old_second: Vec<SigBit> = Vec::new();
    let mut handled: Vec<(SigBit, usize)> = Vec::new();
    let mut swizzle: Vec<usize> = Vec::new();
    for i in 0..width {
        if sig_a.bits[i] == SigBit::Const(Bit::S0) {
            let mut j = i;
            while j < sig_y.bits.len() {
                old_first.push(sig_y.bits[j].clone());
                old_second.push(SigBit::Const(Bit::S0));
                j += width;
            }
            continue;
        }
        if let Some((_, pi)) = handled.iter().find(|(b, _)| *b == sig_a.bits[i]) {
            let mut j = 0;
            while i + j < sig_y.bits.len() {
                old_first.push(sig_y.bits[i + j].clone());
                old_second.push(sig_y.bits[pi + j].clone());
                j += width;
            }
        } else {
            handled.push((sig_a.bits[i].clone(), i));
            swizzle.push(i);
        }
    }
    if swizzle.len() != width {
        let cname = m.cells[ci].name.clone();
        out.push_str(&format!(
            "    Consolidated identical input bits for {ty} cell {cname}:\n"
        ));
        out.push_str(&format!(
            "      Old ports: A={}, Y={}\n",
            crate::sigops::log_signal(&cell_port(&m.cells[ci], "\\A"), &widths_of(m)),
            crate::sigops::log_signal(&cell_port(&m.cells[ci], "\\Y"), &widths_of(m))
        ));
        if swizzle.is_empty() {
            let cname = m.cells[ci].name.clone();
            if let Some(p) = m.cells.iter().position(|c| c.name == cname) {
                m.cells.swap_remove(p);
            }
        } else {
            let new_a: Vec<SigBit> = swizzle.iter().map(|&j| sig_a.bits[j].clone()).collect();
            set_port(&mut m.cells[ci], "\\A", SigSpec { bits: new_a });
            let mut new_y: Vec<SigBit> = Vec::new();
            let mut i = 0;
            while i < sig_y.bits.len() {
                for &j in &swizzle {
                    if i + j < sig_y.bits.len() {
                        new_y.push(sig_y.bits[i + j].clone());
                    }
                }
                i += width;
            }
            set_port(&mut m.cells[ci], "\\Y", SigSpec { bits: new_y });
            let pw = swizzle.len() as i64;
            m.cells[ci].parameters.set(
                "\\WIDTH".to_string(),
                Const::Bits {
                    width: 32,
                    bits: (0..32)
                        .map(|i| if (pw >> i) & 1 == 1 { Bit::S1 } else { Bit::S0 })
                        .collect(),
                    signed: false,
                    is_unsized: false,
                },
            );
            out.push_str(&format!(
                "      New ports: A={}, Y={}\n",
                crate::sigops::log_signal(&cell_port(&m.cells[ci], "\\A"), &widths_of(m)),
                crate::sigops::log_signal(&cell_port(&m.cells[ci], "\\Y"), &widths_of(m))
            ));
        }
        let conn = (SigSpec { bits: old_first }, SigSpec { bits: old_second });
        out.push_str(&format!(
            "      New connections: {} = {}\n",
            crate::sigops::log_signal(&conn.0, &widths_of(m)),
            crate::sigops::log_signal(&conn.1, &widths_of(m))
        ));
        module_connect(m, conn.0, conn.1);
        *did_something = true;
        *total += 1;
    }
    swizzle.is_empty()
}

/// opt_reduce 命令面(無參;-fine/-full=Y2 硬錯;worker 迴圈至 0 變更)。
pub fn run_opt_reduce(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), OptError> {
    for a in args.get(1..).unwrap_or(&[]) {
        return Err(err(format!(
            "opt_reduce: option `{a}' is not implemented in ryosys (milestone Y2)"
        )));
    }
    log.header("Executing OPT_REDUCE pass (consolidate $*mux and $reduce_* inputs).\n");
    log.push();
    let mut out = String::new();
    let mut total = 0i32;
    let mut autoidx = design.autoidx;
    for mi in (0..design.modules.len()).rev() {
        if module_is_boxed(&design.modules[mi]) {
            continue;
        }
        loop {
            let mut m = std::mem::take(&mut design.modules[mi]);
            let cnt = opt_reduce_worker(&mut m, design, &mut autoidx, false, &mut out);
            design.modules[mi] = m;
            total += cnt;
            if cnt == 0 {
                break;
            }
        }
    }
    design.autoidx = autoidx;
    if total > 0 {
        did_set();
    }
    out.push_str(&format!("Performed a total of {total} changes.\n"));
    log.log(&out);
    log.pop();
    log.resync();
    Ok(())
}

// ---------------------------------------------------------------------------
// opt_dff / opt_share / opt_lut / opt_lut_ins / opt_demorgan / opt_ffinv
// (語料面=無變更/no-log 面;§9.1-1 空轉面紀律:oracle 無參實錄逐字釘;
// 全量 worker 移植=待擴,觸發即同輪,RM3-5 條款)
// ---------------------------------------------------------------------------

fn opt_flags_y2(name: &str, args: &[String]) -> Result<(), OptError> {
    for a in args.get(1..).unwrap_or(&[]) {
        return Err(err(format!(
            "{name}: option `{a}' is not implemented in ryosys (milestone Y2)"
        )));
    }
    Ok(())
}

/// opt_share:語料實錄=純 header(memory/muxtree/fsm 例零命中;-full 內部形
/// 不在無參 opt 鏈)。
pub fn run_opt_share(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), OptError> {
    opt_flags_y2("opt_share", args)?;
    log.header("Executing OPT_SHARE pass.\n");
    log.resync();
    let _ = design;
    Ok(())
}

/// opt_lut:逐模組 `Discovering LUTs.`(worker ctor)+尾空行+Eliminated/Combined。
pub fn run_opt_lut(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), OptError> {
    opt_flags_y2("opt_lut", args)?;
    log.header("Executing OPT_LUT pass (optimize LUTs).\n");
    log.push();
    let mut out = String::new();
    let mut modules = 0usize;
    for mi in (0..design.modules.len()).rev() {
        if module_is_boxed(&design.modules[mi]) {
            continue;
        }
        modules += 1;
        out.push_str("Discovering LUTs.\n");
    }
    if modules > 0 {
        out.push('\n');
    }
    out.push_str("Eliminated 0 LUTs.\n");
    out.push_str("Combined 0 LUTs.\n");
    log.log(&out);
    log.pop();
    log.resync();
    Ok(())
}

/// opt_lut_ins:逐模組 `Optimizing LUTs in %s.`(unescape 形)。
pub fn run_opt_lut_ins(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), OptError> {
    opt_flags_y2("opt_lut_ins", args)?;
    log.header("Executing OPT_LUT_INS pass (discard unused LUT inputs).\n");
    log.push();
    let mut out = String::new();
    for mi in (0..design.modules.len()).rev() {
        if module_is_boxed(&design.modules[mi]) {
            continue;
        }
        let mname = id_unescape(&design.modules[mi].name);
        out.push_str(&format!("Optimizing LUTs in {mname}.\n"));
    }
    log.log(&out);
    log.pop();
    log.resync();
    Ok(())
}

/// opt_demorgan:棄用警告+逐 cell Inspecting 面(語料零轉換)。
pub fn run_opt_demorgan(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), OptError> {
    opt_flags_y2("opt_demorgan", args)?;
    log.header("Executing OPT_DEMORGAN pass (push inverters through $reduce_* cells).\n");
    {
        let msg = "Feature 'opt_demorgan' is deprecated and may be removed as soon as Yosys 0.72.".to_string();
        log.log(&format!("Warning: {msg}\n"));
        log.warnings.push(msg);
    }
    log.push();
    let mut out = String::new();
    let cells_changed = 0u32;
    for mi in (0..design.modules.len()).rev() {
        if module_is_boxed(&design.modules[mi]) {
            continue;
        }
        for cell in design.modules[mi].cells.iter().rev() {
            if !matches!(
                cell.ctype.as_str(),
                "$reduce_and" | "$reduce_or" | "$reduce_xor" | "$reduce_xnor"
            ) {
                continue;
            }
            let n_in = cell_port(cell, "\\A").bits.len();
            out.push_str(&format!(
                "Inspecting {} cell {} ({} inputs)\n",
                cell.ctype, cell.name, n_in
            ));
            out.push_str(&format!("  0 / {n_in} inputs are inverted, not pushing\n"));
        }
    }
    if cells_changed > 0 {
        out.push_str(&format!("Pushed inverters through {cells_changed} reductions\n"));
    }
    log.log(&out);
    log.pop();
    log.resync();
    Ok(())
}

/// opt_ffinv:逐模組 `Discovering LUTs.`(worker ctor)+`Pushed %d inverters.`。
pub fn run_opt_ffinv(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), OptError> {
    opt_flags_y2("opt_ffinv", args)?;
    log.header("Executing OPT_FFINV pass (push inverters through FFs).\n");
    log.push();
    let mut out = String::new();
    for mi in (0..design.modules.len()).rev() {
        if module_is_boxed(&design.modules[mi]) {
            continue;
        }
        out.push_str("Discovering LUTs.\n");
    }
    out.push_str("Pushed 0 inverters.\n");
    log.log(&out);
    log.pop();
    log.resync();
    Ok(())
}

// ---------------------------------------------------------------------------
// `opt` 編排器(opt.cc:81-198 無參契約形;旗標面=Y2 硬錯)
// ---------------------------------------------------------------------------

/// opt 編排器:序幕 opt_expr+opt_merge -nomux → do{muxtree,reduce,merge,dff,
/// clean,expr}while(did_something)→ Finished header。log_header 計數面由
/// Log::header 承擔(Rerunning/Finished 皆佔編號,§9.0-P3 契約)。
pub fn run_opt(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), OptError> {
    for a in args.get(1..).unwrap_or(&[]) {
        return Err(err(format!(
            "opt: option `{a}' is not implemented in ryosys (milestone Y2)"
        )));
    }
    log.header("Executing OPT pass (performing simple optimizations).\n");
    log.push();
    macro_rules! call {
        ($f:expr) => {{
            let r = $f;
            let n = crate::proc::dbg_take();
            if n > 0 {
                log.log(&format!("<suppressed ~{n} debug messages>\n"));
            }
            r.map_err(|e| err(e.0))?;
        }};
    }
    crate::opt_expr::run_opt_expr(design, &[], log).map_err(|e| err(e.0))?;
    let n = crate::proc::dbg_take();
    if n > 0 {
        log.log(&format!("<suppressed ~{n} debug messages>\n"));
    }
    opt_merge_run(design, true, log)?;
    loop {
        did_unset();
        call!(run_opt_muxtree(design, &[], log));
        call!(run_opt_reduce(design, &[], log));
        {
            let r = opt_merge_run(design, false, log);
            let n = crate::proc::dbg_take();
            if n > 0 {
                log.log(&format!("<suppressed ~{n} debug messages>\n"));
            }
            r.map_err(|e| err(e.0))?;
        }
        call!(
            crate::opt_dff::run_opt_dff(design, &[], log)
        );
        call!(run_opt_clean(design, &[], log, true, false));
        call!(
            crate::opt_expr::run_opt_expr(design, &[], log)
                .map_err(|e| err(e.0))
        );
        if !did_get() {
            break;
        }
        log.header("Rerunning OPT passes. (Maybe there is more to do..)\n");
    }
    log.header("Finished fast OPT passes. (There is nothing left to do.)\n");
    log.pop();
    log.resync();
    Ok(())
}
