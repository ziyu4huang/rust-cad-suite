//! `opt_expr` 的 `-keepdc` 契約子集(RYOSYS-MVP §8.3-3;passes/opt/
//! opt_expr.cc 移植面)。契約形=僅 `-keepdc` 單旗標:execute 體=內層
//! replace_const_cells(consume_x=false, mux_undef=false, mux_bool=false,
//! do_fine=false, keepdc=true, noclkinv=false)不動點+尾
//! replace_const_connections;其餘旗標=Y2 硬錯(詳案 §8.4)。
//!
//! 充分判據=54 例 byte-exact(P4:29 例可觀測面+`$auto$opt_expr.cc:702:
//! replace_const_cells$` ×33 造名)。行號錨以二進制 golden 對拍為準、
//! clone 讀碼值僅初值(RM1-1;702=Not 行經 33 次實錄零漂移)。

use crate::consteval as ce;
use crate::proc::{Pm, ProcError};
use crate::sigops::*;
use ry_ir::{Bit, Const, Design, SigBit, SigSpec};
use std::collections::HashMap;

fn err(msg: impl Into<String>) -> ProcError {
    ProcError(msg.into())
}

// ---------------------------------------------------------------------------
// cell port/param 工具(setPort=operator[] 語義;unset=dict erase)
// ---------------------------------------------------------------------------

fn get_port<'a>(cell: &'a ry_ir::Cell, port: &str) -> Option<&'a SigSpec> {
    cell.connections.iter().find(|(p, _)| p == port).map(|(_, s)| s)
}

fn set_port(cell: &mut ry_ir::Cell, port: &str, sig: SigSpec) {
    if let Some(e) = cell.connections.iter_mut().find(|(p, _)| p == port) {
        e.1 = sig;
    } else {
        cell.connections.push((port.to_string(), sig));
    }
}

fn unset_port(cell: &mut ry_ir::Cell, port: &str) {
    cell.connections.retain(|(p, _)| p != port);
}

fn param_int(cell: &ry_ir::Cell, name: &str) -> i64 {
    cell.parameters.get(name).map(const_as_int).unwrap_or(0)
}

fn param_bool(cell: &ry_ir::Cell, name: &str) -> bool {
    cell.parameters.get(name).map(const_as_bool).unwrap_or(false)
}

fn set_param(cell: &mut ry_ir::Cell, name: &str, v: Const) {
    cell.parameters.set(name.to_string(), v);
}

fn unset_param(cell: &mut ry_ir::Cell, name: &str) {
    cell.parameters.swap_pop_remove(name);
}

fn i32c(v: i64) -> Const {
    Const::from_i64_i32_width(v)
}

fn i32cb(v: bool) -> Const {
    Const::from_i64_i32_width(v as i64)
}

fn one_bit(b: Bit) -> SigSpec {
    SigSpec { bits: vec![SigBit::Const(b)] }
}

fn remove_cell_by_name(module: &mut ry_ir::Module, name: &str) {
    if let Some(i) = module.cells.iter().position(|c| c.name == name) {
        let n = module.cells.len();
        module.cells.swap(i, n - 1);
        module.cells.pop();
    }
}

/// Module::connect(lhs 常數位過濾;rtlil.cc:3064)。
fn connect(module: &mut ry_ir::Module, lhs: SigSpec, rhs: SigSpec) {
    if lhs.has_const() {
        let mut nl = SigSpec::default();
        let mut nr = SigSpec::default();
        for (i, b) in lhs.bits.iter().enumerate() {
            if matches!(b, SigBit::Wire { .. }) {
                nl.bits.push(b.clone());
                nr.bits.push(rhs.bits[i].clone());
            }
        }
        if nl.bits.is_empty() {
            return;
        }
        module.connections.push((nl, nr));
    } else {
        module.connections.push((lhs, rhs));
    }
}

// ---------------------------------------------------------------------------
// AssignMap(SigMap+add 增量)
// ---------------------------------------------------------------------------

struct AssignMap {
    inner: crate::sigmap::SigMap,
}

impl AssignMap {
    fn new(module: &ry_ir::Module) -> AssignMap {
        let mut inner = crate::sigmap::SigMap::new();
        inner.set(module);
        AssignMap { inner }
    }
    fn map(&self, sig: &SigSpec) -> SigSpec {
        self.inner.map(sig)
    }
    fn apply(&self, sig: &mut SigSpec) {
        *sig = self.inner.map(sig);
    }
    fn add(&mut self, from: &SigSpec, to: &SigSpec) {
        self.inner.add(from, to);
    }
}

// ---------------------------------------------------------------------------
// invert_map + polarity 面(opt_expr.cc:400-487)
// ---------------------------------------------------------------------------

fn build_invert_map(module: &ry_ir::Module, am: &AssignMap) -> HashMap<SigBit, SigBit> {
    let mut invert_map: HashMap<SigBit, SigBit> = HashMap::new();
    for cell in module.cells.iter().rev() {
        if !cell.ctype.starts_with('$') {
            continue;
        }
        if matches!(cell.ctype.as_str(), "$_NOT_" | "$not" | "$logic_not")
            && get_port(cell, "\\A").map(|s| s.size()).unwrap_or(0) == 1
            && get_port(cell, "\\Y").map(|s| s.size()).unwrap_or(0) == 1
        {
            let yb = am.map(get_port(cell, "\\Y").unwrap()).bits[0].clone();
            let ab = am.map(get_port(cell, "\\A").unwrap()).bits[0].clone();
            invert_map.insert(yb, ab);
        }
        if matches!(cell.ctype.as_str(), "$mux" | "$_MUX_") {
            let one = one_bit(Bit::S1);
            let zero = one_bit(Bit::S0);
            if get_port(cell, "\\A") == Some(&one) && get_port(cell, "\\B") == Some(&zero) {
                let yb = am.map(get_port(cell, "\\Y").unwrap()).bits[0].clone();
                let sb = am.map(get_port(cell, "\\S").unwrap()).bits[0].clone();
                invert_map.insert(yb, sb);
            }
        }
    }
    invert_map
}

fn handle_polarity_inv(cell: &mut ry_ir::Cell, port: &str, param: &str, am: &AssignMap, invert_map: &HashMap<SigBit, SigBit>) {
    let sig = am.map(get_port(cell, port).unwrap_or(&SigSpec::default()));
    if let Some(b) = sig.bits.first() {
        if let Some(inv) = invert_map.get(b) {
            // oracle log_debug: Inverting EN of $dlatch cell ... (opt_expr.cc:310)
            crate::proc::dbg_inc_what(&format!(
                "Inverting {} of {} cell in module",
                port.trim_start_matches('\\'),
                cell.ctype
            ));
            set_port(cell, port, SigSpec { bits: vec![inv.clone()] });
            let cur = param_bool(cell, param);
            set_param(cell, param, i32cb(!cur));
        }
    }
}

fn handle_clkpol_swap(cell: &mut ry_ir::Cell, t1: &str, t2: &str, port: &str, am: &AssignMap, invert_map: &HashMap<SigBit, SigBit>) {
    if t1.len() != t2.len() || t1.len() != cell.ctype.len() {
        return;
    }
    let bytes = cell.ctype.as_bytes();
    let mut tt1: Vec<char> = t1.chars().collect();
    for i in 0..tt1.len() {
        if tt1[i] == '?' {
            let c = bytes[i] as char;
            if !matches!(c, '0' | '1' | 'N' | 'P') {
                return;
            }
            tt1[i] = c;
        }
    }
    let s1: String = tt1.iter().collect();
    let s2: String = t2.chars().zip(tt1.iter()).map(|(c, &r)| if c == '?' { r } else { c }).collect();
    if cell.ctype == s1 || cell.ctype == s2 {
        let sig = am.map(get_port(cell, port).unwrap_or(&SigSpec::default()));
        if let Some(b) = sig.bits.first() {
            if let Some(inv) = invert_map.get(b) {
                set_port(cell, port, SigSpec { bits: vec![inv.clone()] });
                cell.ctype = if cell.ctype == s1 { s2 } else { s1 };
            }
        }
    }
}

const SWAPS: &[(&str, &str, &str)] = &[
    ("$_SR_N?_", "$_SR_P?_", "\\S"),
    ("$_SR_?N_", "$_SR_?P_", "\\R"),
    ("$_DFF_N_", "$_DFF_P_", "\\C"),
    ("$_DFFE_N?_", "$_DFFE_P?_", "\\C"),
    ("$_DFFE_?N_", "$_DFFE_?P_", "\\E"),
    ("$_DFF_N??_", "$_DFF_P??_", "\\C"),
    ("$_DFF_?N?_", "$_DFF_?P?_", "\\R"),
    ("$_DFFE_N???_", "$_DFFE_P???_", "\\C"),
    ("$_DFFE_?N??_", "$_DFFE_?P??_", "\\R"),
    ("$_DFFE_???N_", "$_DFFE_???P_", "\\E"),
    ("$_SDFF_N??_", "$_SDFF_P??_", "\\C"),
    ("$_SDFF_?N?_", "$_SDFF_?P?_", "\\R"),
    ("$_SDFFE_N???_", "$_SDFFE_P???_", "\\C"),
    ("$_SDFFE_?N??_", "$_SDFFE_?P??_", "\\R"),
    ("$_SDFFE_???N_", "$_SDFFE_???P_", "\\E"),
    ("$_SDFFCE_N???_", "$_SDFFCE_P???_", "\\C"),
    ("$_SDFFCE_?N??_", "$_SDFFCE_?P??_", "\\R"),
    ("$_SDFFCE_???N_", "$_SDFFCE_???P_", "\\E"),
    ("$_ALDFF_N?_", "$_ALDFF_P?_", "\\C"),
    ("$_ALDFF_?N_", "$_ALDFF_?P_", "\\L"),
    ("$_ALDFFE_N??_", "$_ALDFFE_P??_", "\\C"),
    ("$_ALDFFE_?N?_", "$_ALDFFE_?P?_", "\\L"),
    ("$_ALDFFE_??N_", "$_ALDFFE_??P_", "\\E"),
    ("$_DFFSR_N??_", "$_DFFSR_P??_", "\\C"),
    ("$_DFFSR_?N?_", "$_DFFSR_?P?_", "\\S"),
    ("$_DFFSR_??N_", "$_DFFSR_??P_", "\\R"),
    ("$_DFFSRE_N???_", "$_DFFSRE_P???_", "\\C"),
    ("$_DFFSRE_?N??_", "$_DFFSRE_?P??_", "\\S"),
    ("$_DFFSRE_??N?_", "$_DFFSRE_??P?_", "\\R"),
    ("$_DFFSRE_???N_", "$_DFFSRE_???P_", "\\E"),
    ("$_DLATCH_N_", "$_DLATCH_P_", "\\E"),
    ("$_DLATCH_N??_", "$_DLATCH_P??_", "\\E"),
    ("$_DLATCH_?N?_", "$_DLATCH_?P?_", "\\R"),
    ("$_DLATCHSR_N??_", "$_DLATCHSR_P??_", "\\E"),
    ("$_DLATCHSR_?N?_", "$_DLATCHSR_?P?_", "\\S"),
    ("$_DLATCHSR_??N_", "$_DLATCHSR_??P_", "\\R"),
];

// ---------------------------------------------------------------------------
// TopoSort(kernel/utils.h;起點=cell 名序;edges 入邊按名序;DFS 後序)
// ---------------------------------------------------------------------------

struct Topo {
    names: Vec<String>,
    edges: Vec<Vec<usize>>,
    sorted: Vec<usize>,
    found_loops: bool,
}

impl Topo {
    fn node(&mut self, name: &str) -> usize {
        if let Some(i) = self.names.iter().position(|n| n == name) {
            return i;
        }
        self.names.push(name.to_string());
        self.edges.push(Vec::new());
        self.names.len() - 1
    }
    fn edge(&mut self, l: usize, r: usize) {
        if !self.edges[r].contains(&l) {
            self.edges[r].push(l);
        }
    }
    fn sort(&mut self) -> bool {
        self.sorted.clear();
        self.found_loops = false;
        let n = self.names.len();
        let mut on_sorted = vec![false; n];
        let mut on_stack = vec![false; n];
        let mut stack: Vec<usize> = Vec::new();
        let mut order: Vec<usize> = (0..n).collect();
        let names = self.names.clone();
        order.sort_by(|a, b| names[*a].cmp(&names[*b]));
        for root in order {
            self.worker(root, &mut on_sorted, &mut on_stack, &mut stack);
        }
        !self.found_loops
    }
    fn worker(&mut self, root: usize, on_sorted: &mut [bool], on_stack: &mut [bool], stack: &mut Vec<usize>) {
        if on_stack[root] {
            self.found_loops = true;
            return;
        }
        if on_sorted[root] {
            return;
        }
        if !self.edges[root].is_empty() {
            stack.push(root);
            on_stack[root] = true;
            let mut succ: Vec<usize> = self.edges[root].clone();
            let names = self.names.clone();
            succ.sort_by(|a, b| names[*a].cmp(&names[*b]));
            for l in succ {
                self.worker(l, on_sorted, on_stack, stack);
            }
            stack.pop();
            on_stack[root] = false;
        }
        on_sorted[root] = true;
        self.sorted.push(root);
    }
}

// ---------------------------------------------------------------------------
// SigSpec::match(rtlil.cc:5674;' '/'*'/'0'/'1')
// ---------------------------------------------------------------------------

fn sig_match(sig: &SigSpec, pattern: &str) -> bool {
    let plen = pattern.len();
    if plen != sig.bits.len() {
        return false;
    }
    let pb = pattern.as_bytes();
    for i in 0..plen {
        let ch = pb[i] as char;
        if ch == ' ' {
            continue;
        }
        let bit = &sig.bits[plen - 1 - i];
        match ch {
            '*' => {
                if !matches!(bit, SigBit::Const(Bit::Sx) | SigBit::Const(Bit::Sz)) {
                    return false;
                }
            }
            '0' => {
                if *bit != SigBit::Const(Bit::S0) {
                    return false;
                }
            }
            '1' => {
                if *bit != SigBit::Const(Bit::S1) {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

fn as_const(sig: &SigSpec) -> Const {
    ce::b2c(sig.bits.iter().map(|b| match b {
        SigBit::Const(x) => *x,
        _ => Bit::Sx,
    }).collect())
}

// ---------------------------------------------------------------------------
// replace_const_cells(-keepdc 形;opt_expr.cc:395 起)
// ---------------------------------------------------------------------------

fn replace_const_cells(design: &mut Design, mi: usize, log_out: &mut String) -> bool {
    let mut am = AssignMap::new(&design.modules[mi]);
    let invert_map = build_invert_map(&design.modules[mi], &am);
    let mut did = false;

    // polarity 面(noclkinv=false)。
    {
        let module = &mut design.modules[mi];
        for ci in 0..module.cells.len() {
            let ctype = module.cells[ci].ctype.clone();
            if matches!(
                ctype.as_str(),
                "$dff" | "$dffe" | "$dffsr" | "$dffsre" | "$adff" | "$adffe" | "$aldff" | "$aldffe"
                    | "$sdff" | "$sdffe" | "$sdffce" | "$fsm" | "$memrd" | "$memrd_v2" | "$memwr" | "$memwr_v2"
            ) {
                handle_polarity_inv(&mut module.cells[ci], "\\CLK", "\\CLK_POLARITY", &am, &invert_map);
            }
            if matches!(ctype.as_str(), "$sr" | "$dffsr" | "$dffsre" | "$dlatchsr") {
                handle_polarity_inv(&mut module.cells[ci], "\\SET", "\\SET_POLARITY", &am, &invert_map);
                handle_polarity_inv(&mut module.cells[ci], "\\CLR", "\\CLR_POLARITY", &am, &invert_map);
            }
            if matches!(ctype.as_str(), "$adff" | "$adffe" | "$adlatch") {
                handle_polarity_inv(&mut module.cells[ci], "\\ARST", "\\ARST_POLARITY", &am, &invert_map);
            }
            if matches!(ctype.as_str(), "$aldff" | "$aldffe") {
                handle_polarity_inv(&mut module.cells[ci], "\\ALOAD", "\\ALOAD_POLARITY", &am, &invert_map);
            }
            if matches!(ctype.as_str(), "$sdff" | "$sdffe" | "$sdffce") {
                handle_polarity_inv(&mut module.cells[ci], "\\SRST", "\\SRST_POLARITY", &am, &invert_map);
            }
            if matches!(
                ctype.as_str(),
                "$dffe" | "$adffe" | "$aldffe" | "$sdffe" | "$sdffce" | "$dffsre" | "$dlatch" | "$adlatch" | "$dlatchsr"
            ) {
                handle_polarity_inv(&mut module.cells[ci], "\\EN", "\\EN_POLARITY", &am, &invert_map);
            }
            if ctype.starts_with("$_") {
                for (t1, t2, port) in SWAPS {
                    handle_clkpol_swap(&mut module.cells[ci], t1, t2, port, &am, &invert_map);
                }
            }
        }
    }

    // TopoSort(evaluable cells;輸出位→cell 名)。
    let mut topo = Topo { names: Vec::new(), edges: Vec::new(), sorted: Vec::new(), found_loops: false };
    let mut outbit_to_cell: HashMap<SigBit, String> = HashMap::new();
    {
        let module = &design.modules[mi];
        for cell in &module.cells {
            if crate::cells::cell_evaluable(&cell.ctype) {
                for (port, conn) in &cell.connections {
                    let (_, is_out) = crate::cells::port_dir(&cell.ctype, port);
                    if is_out {
                        for bit in &am.map(conn).bits {
                            outbit_to_cell.insert(bit.clone(), cell.name.clone());
                        }
                    }
                }
                topo.node(&cell.name);
            }
        }
        for cell in &module.cells {
            if crate::cells::cell_evaluable(&cell.ctype) {
                let r = topo.node(&cell.name);
                for (port, conn) in &cell.connections {
                    let (is_in, _) = crate::cells::port_dir(&cell.ctype, port);
                    if is_in {
                        for bit in &am.map(conn).bits {
                            if let Some(src) = outbit_to_cell.get(bit) {
                                let l = topo.node(src);
                                topo.edge(l, r);
                            }
                        }
                    }
                }
            }
        }
    }
    let sorted_ok = topo.sort();
    if !sorted_ok {
        let mname = &design.modules[mi].name;
        let m = mname.strip_prefix('\\').unwrap_or(mname);
        log_out.push_str(&format!(
            "Couldn't topologically sort cells, optimizing module {m} may take a longer time.\n"
        ));
    }
    let sorted_names: Vec<String> = topo.sorted.iter().map(|&i| topo.names[i].clone()).collect();

    // 逐 cell fold(按 topo 序;cell 已被刪=按名查缺跳)。
    for name in sorted_names {
        let Some(snap) = design.modules[mi].cells.iter().find(|c| c.name == name).map(|c| (c.ctype.clone(), c.parameters.clone(), c.attributes.get("\\src").cloned())) else {
            continue;
        };
        let ctype = snap.0.clone();

        // ---- detect_const_and/or(:525-579)----
        {
            let cell = &design.modules[mi].cells.iter().find(|c| c.name == name).unwrap();
            let mut detect_and = false;
            let mut detect_or = false;
            if matches!(ctype.as_str(), "$reduce_and" | "$_AND_") {
                detect_and = true;
            }
            if matches!(ctype.as_str(), "$and" | "$logic_and")
                && get_port(cell, "\\A").map(|s| s.size()) == Some(1)
                && get_port(cell, "\\B").map(|s| s.size()) == Some(1)
                && !param_bool(cell, "\\A_SIGNED")
            {
                detect_and = true;
            }
            if matches!(ctype.as_str(), "$reduce_or" | "$reduce_bool" | "$_OR_") {
                detect_or = true;
            }
            if matches!(ctype.as_str(), "$or" | "$logic_or")
                && get_port(cell, "\\A").map(|s| s.size()) == Some(1)
                && get_port(cell, "\\B").map(|s| s.size()) == Some(1)
                && !param_bool(cell, "\\A_SIGNED")
            {
                detect_or = true;
            }
            if detect_and || detect_or {
                let mut input_bits: Vec<SigBit> = Vec::new();
                if let Some(a) = get_port(cell, "\\A") {
                    for b in &am.map(a).bits {
                        if !input_bits.contains(b) {
                            input_bits.push(b.clone());
                        }
                    }
                }
                if let Some(bp) = get_port(cell, "\\B") {
                    for b in &am.map(bp).bits {
                        if !input_bits.contains(b) {
                            input_bits.push(b.clone());
                        }
                    }
                }
                let mut found_zero = false;
                let mut found_one = false;
                let mut found_undef = false;
                let mut found_inv = false;
                let mut many_conconst = false;
                let mut non_const_input: SigBit = SigBit::Const(Bit::Sm);
                for bit in &input_bits {
                    match bit {
                        SigBit::Wire { .. } => {
                            if let Some(ib) = invert_map.get(bit) {
                                if input_bits.contains(ib) {
                                    found_inv = true;
                                }
                            }
                            if non_const_input != SigBit::Const(Bit::Sm) {
                                many_conconst = true;
                            }
                            if many_conconst {
                                non_const_input = SigBit::Const(Bit::Sm);
                            } else {
                                non_const_input = bit.clone();
                            }
                        }
                        SigBit::Const(Bit::S0) => found_zero = true,
                        SigBit::Const(Bit::S1) => found_one = true,
                        _ => found_undef = true,
                    }
                }
                let mut action: Option<SigSpec> = None;
                if detect_and && (found_zero || found_inv) {
                    action = Some(one_bit(Bit::S0));
                } else if detect_or && (found_one || found_inv) {
                    action = Some(one_bit(Bit::S1));
                } else if non_const_input != SigBit::Const(Bit::Sm) && !found_undef {
                    action = Some(SigSpec { bits: vec![non_const_input] });
                }
                if let Some(val) = action {
                    let module = &mut design.modules[mi];
                    let y = get_port(module.cells.iter().find(|c| c.name == name).unwrap(), "\\Y").cloned().unwrap_or_default();
                    let mut val = val;
                    extend_u0(&mut val, y.size(), false);
                    am.add(&y, &val);
                    connect(module, y, val);
                    remove_cell_by_name(module, &name);
                    did = true;
                    crate::proc::dbg_inc_what("?");
                    continue;
                }
            }
        }

        // ---- $xor/$xnor 單位(:581-627)----
        {
            let cell = &design.modules[mi].cells.iter().find(|c| c.name == name).unwrap();
            let is_xor_family = matches!(ctype.as_str(), "$_XOR_" | "$_XNOR_")
                || (matches!(ctype.as_str(), "$xor" | "$xnor")
                    && get_port(cell, "\\A").map(|s| s.size()) == Some(1)
                    && get_port(cell, "\\B").map(|s| s.size()) == Some(1)
                    && !param_bool(cell, "\\A_SIGNED"));
            if is_xor_family {
                let mut sig_a = am.map(get_port(cell, "\\A").unwrap()).bits[0].clone();
                let mut sig_b = am.map(get_port(cell, "\\B").unwrap()).bits[0].clone();
                if matches!(sig_a, SigBit::Const(_)) {
                    std::mem::swap(&mut sig_a, &mut sig_b);
                }
                if matches!(sig_b, SigBit::Const(Bit::S0) | SigBit::Const(Bit::S1)) {
                    let b_one = sig_b == SigBit::Const(Bit::S1);
                    let is_xor = matches!(ctype.as_str(), "$xor" | "$_XOR_");
                    let need_not = if is_xor { b_one } else { !b_one };
                    let ywidth = param_int(cell, "\\Y_WIDTH");
                    let mut sig_y: SigSpec;
                    if need_not {
                        let module = &mut design.modules[mi];
                        let autoidx = &mut design.autoidx;
                        let mut pm = Pm { module, autoidx };
                        let id = pm.new_id_at("opt_expr.cc", if is_xor { 605 } else { 615 }, "replace_const_cells");
                        sig_y = pm.helper_not_at(id, 3201, &SigSpec { bits: vec![sig_a.clone()] }, None);
                    } else {
                        sig_y = SigSpec { bits: vec![sig_a.clone()] };
                    }
                    if !is_xor && ctype == "$xnor" {
                        for _ in 1..ywidth.max(0) {
                            sig_y.bits.push(SigBit::Const(Bit::S1));
                        }
                    }
                    let module = &mut design.modules[mi];
                    let y = get_port(module.cells.iter().find(|c| c.name == name).unwrap(), "\\Y").cloned().unwrap_or_default();
                    let mut val = sig_y;
                    extend_u0(&mut val, y.size(), false);
                    am.add(&y, &val);
                    connect(module, y, val);
                    remove_cell_by_name(module, &name);
                    did = true;
                    crate::proc::dbg_inc_what("?");
                    continue;
                }
            }
        }

        // ---- unary buffer(:629-641)----
        {
            let cell = &design.modules[mi].cells.iter().find(|c| c.name == name).unwrap();
            if matches!(
                ctype.as_str(),
                "$reduce_and" | "$reduce_or" | "$reduce_bool" | "$reduce_xor" | "$reduce_xnor" | "$neg"
            ) && get_port(cell, "\\A").map(|s| s.size()) == Some(1)
                && get_port(cell, "\\Y").map(|s| s.size()) == Some(1)
            {
                if ctype == "$reduce_xnor" {
                    let module = &mut design.modules[mi];
                    let ci = module.cells.iter().position(|c| c.name == name).unwrap();
                    module.cells[ci].ctype = "$not".to_string();
                    did = true;
                    crate::proc::dbg_inc_what("?");
                    continue;
                } else {
                    let a = get_port(cell, "\\A").cloned().unwrap();
                    let module = &mut design.modules[mi];
                    let y = get_port(module.cells.iter().find(|c| c.name == name).unwrap(), "\\Y").cloned().unwrap_or_default();
                    let mut val = a;
                    extend_u0(&mut val, y.size(), false);
                    am.add(&y, &val);
                    connect(module, y, val);
                    remove_cell_by_name(module, &name);
                    did = true;
                    crate::proc::dbg_inc_what("?");
                    continue;
                }
            }
        }

        // ---- $and/$or/$xor/$xnor 單常數輸入(:643-721)----
        {
            let cell = &design.modules[mi].cells.iter().find(|c| c.name == name).unwrap();
            if matches!(ctype.as_str(), "$and" | "$or" | "$xor" | "$xnor") {
                let sig_a_raw = am.map(get_port(cell, "\\A").unwrap_or(&SigSpec::default()));
                let sig_b_raw = am.map(get_port(cell, "\\B").unwrap_or(&SigSpec::default()));
                let a_fc = is_fully_const(&sig_a_raw) && !is_fully_undef(&sig_a_raw);
                let b_fc = is_fully_const(&sig_b_raw) && !is_fully_undef(&sig_b_raw);
                if a_fc != b_fc {
                    let width = get_port(cell, "\\Y").map(|s| s.size()).unwrap_or(0);
                    let mut sig_a = sig_a_raw.clone();
                    let mut sig_b = sig_b_raw.clone();
                    extend_u0(&mut sig_a, width, param_bool(cell, "\\A_SIGNED"));
                    extend_u0(&mut sig_b, width, param_bool(cell, "\\B_SIGNED"));
                    if !a_fc {
                        std::mem::swap(&mut sig_a, &mut sig_b);
                    }
                    let mut b_g0 = SigSpec::default();
                    let mut b_g1 = SigSpec::default();
                    let mut b_gx = SigSpec::default();
                    let mut y_g0 = SigSpec::default();
                    let mut y_g1 = SigSpec::default();
                    let mut y_gx = SigSpec::default();
                    let sig_y = am.map(get_port(cell, "\\Y").unwrap_or(&SigSpec::default()));
                    for i in 0..width as usize {
                        match sig_a.bits.get(i) {
                            Some(SigBit::Const(Bit::S0)) => {
                                b_g0.bits.push(sig_b.bits[i].clone());
                                y_g0.bits.push(sig_y.bits[i].clone());
                            }
                            Some(SigBit::Const(Bit::S1)) => {
                                b_g1.bits.push(sig_b.bits[i].clone());
                                y_g1.bits.push(sig_y.bits[i].clone());
                            }
                            _ => {
                                b_gx.bits.push(sig_b.bits[i].clone());
                                y_gx.bits.push(sig_y.bits[i].clone());
                            }
                        }
                    }
                    if ctype == "$xnor" {
                        std::mem::swap(&mut b_g0, &mut b_g1);
                        std::mem::swap(&mut y_g0, &mut y_g1);
                    }
                    let mut y_new_0 = SigSpec::default();
                    let mut y_new_1 = SigSpec::default();
                    let mut y_new_x = SigSpec::default();
                    {
                        let module = &mut design.modules[mi];
                        let autoidx = &mut design.autoidx;
                        let mut pm = Pm { module, autoidx };
                        match ctype.as_str() {
                            "$and" => {
                                if !y_g0.bits.is_empty() {
                                    y_new_0 = sig_const(Bit::S0, y_g0.size());
                                }
                                if !y_g1.bits.is_empty() {
                                    y_new_1 = b_g1.clone();
                                }
                                if !y_gx.bits.is_empty() {
                                    let id = pm.new_id_at("opt_expr.cc", 687, "replace_const_cells");
                                    y_new_x = pm.helper_and_at(id, 3249, &sig_const(Bit::Sx, y_gx.size()), &b_gx, None);
                                }
                            }
                            "$or" => {
                                if !y_g0.bits.is_empty() {
                                    y_new_0 = b_g0.clone();
                                }
                                if !y_g1.bits.is_empty() {
                                    y_new_1 = sig_const(Bit::S1, y_g1.size());
                                }
                                if !y_gx.bits.is_empty() {
                                    let id = pm.new_id_at("opt_expr.cc", 696, "replace_const_cells");
                                    y_new_x = pm.helper_or_at(id, 3250, &sig_const(Bit::Sx, y_gx.size()), &b_gx, None);
                                }
                            }
                            "$xor" | "$xnor" => {
                                if !y_g0.bits.is_empty() {
                                    y_new_0 = b_g0.clone();
                                }
                                if !y_g1.bits.is_empty() {
                                    let id = pm.new_id_at("opt_expr.cc", 702, "replace_const_cells");
                                    y_new_1 = pm.helper_not_at(id, 3201, &b_g1, None);
                                }
                                if !y_gx.bits.is_empty() {
                                    let id = pm.new_id_at("opt_expr.cc", 705, "replace_const_cells");
                                    y_new_x = pm.helper_xor_at(id, 3251, &sig_const(Bit::Sx, y_gx.size()), &b_gx, None);
                                }
                            }
                            _ => {}
                        }
                    }
                    let module = &mut design.modules[mi];
                    am.add(&y_g0, &y_new_0);
                    connect(module, y_g0, y_new_0);
                    am.add(&y_g1, &y_new_1);
                    connect(module, y_g1, y_new_1);
                    am.add(&y_gx, &y_new_x);
                    connect(module, y_gx, y_new_x);
                    remove_cell_by_name(module, &name);
                    did = true;
                    crate::proc::dbg_inc_what("?");
                    continue;
                }
            }
        }

        // ---- x-bit in input(:1010-1035)----
        if matches!(
            ctype.as_str(),
            "$reduce_xor" | "$reduce_xnor" | "$shift" | "$shiftx" | "$shl" | "$shr" | "$sshl" | "$sshr"
                | "$lt" | "$le" | "$ge" | "$gt" | "$neg" | "$add" | "$sub" | "$mul" | "$div" | "$mod"
                | "$divfloor" | "$modfloor" | "$pow"
        ) {
            let cell = &design.modules[mi].cells.iter().find(|c| c.name == name).unwrap();
            let mut sig_a = am.map(get_port(cell, "\\A").unwrap_or(&SigSpec::default()));
            let sig_b = if get_port(cell, "\\B").is_some() {
                am.map(get_port(cell, "\\B").unwrap())
            } else {
                SigSpec::default()
            };
            if matches!(ctype.as_str(), "$shl" | "$shr" | "$sshl" | "$sshr" | "$shift" | "$shiftx") {
                sig_a = SigSpec::default();
            }
            let has_x = sig_a.bits.iter().any(|b| *b == SigBit::Const(Bit::Sx))
                || sig_b.bits.iter().any(|b| *b == SigBit::Const(Bit::Sx));
            if has_x {
                let val = if matches!(ctype.as_str(), "$reduce_xor" | "$reduce_xnor" | "$lt" | "$le" | "$ge" | "$gt") {
                    one_bit(Bit::Sx)
                } else {
                    let w = get_port(cell, "\\Y").map(|s| s.size()).unwrap_or(0);
                    sig_const(Bit::Sx, w)
                };
                let module = &mut design.modules[mi];
                let y = get_port(module.cells.iter().find(|c| c.name == name).unwrap(), "\\Y").cloned().unwrap_or_default();
                let mut val = val;
                extend_u0(&mut val, y.size(), false);
                am.add(&y, &val);
                connect(module, y, val);
                remove_cell_by_name(module, &name);
                did = true;
                    crate::proc::dbg_inc_what("?");
                continue;
            }
        }

        // ---- $shiftx/$shift trim(:1037-1056)----
        if ctype == "$shiftx" || (ctype == "$shift" && !snap.0.is_empty() && {
            let cell = &design.modules[mi].cells.iter().find(|c| c.name == name).unwrap();
            !param_bool(cell, "\\A_SIGNED")
        }) {
            let cell = &design.modules[mi].cells.iter().find(|c| c.name == name).unwrap();
            let mut sig_a = am.map(get_port(cell, "\\A").unwrap_or(&SigSpec::default()));
            let trim_x = ctype == "$shiftx";
            let trim_0 = ctype == "$shift";
            let mut width = sig_a.size();
            while width > 1 {
                let b = sig_a.bits[(width - 1) as usize].clone();
                if (trim_x && b == SigBit::Const(Bit::Sx)) || (trim_0 && b == SigBit::Const(Bit::S0)) {
                    width -= 1;
                    continue;
                }
                break;
            }
            if width < sig_a.size() {
                sig_a.bits.truncate(width as usize);
                let module = &mut design.modules[mi];
                let ci = module.cells.iter().position(|c| c.name == name).unwrap();
                set_port(&mut module.cells[ci], "\\A", sig_a);
                set_param(&mut module.cells[ci], "\\A_WIDTH", i32c(width));
                did = true;
                continue;
            }
        }

        // ---- double_invert(:1058-1062)----
        if matches!(ctype.as_str(), "$_NOT_" | "$not" | "$logic_not")
            && {
                let cell = &design.modules[mi].cells.iter().find(|c| c.name == name).unwrap();
                get_port(cell, "\\Y").map(|s| s.size()) == Some(1)
            }
        {
            let cell = &design.modules[mi].cells.iter().find(|c| c.name == name).unwrap();
            let ab = am.map(get_port(cell, "\\A").unwrap()).bits[0].clone();
            if let Some(inv) = invert_map.get(&ab).cloned() {
                let module = &mut design.modules[mi];
                let y = get_port(module.cells.iter().find(|c| c.name == name).unwrap(), "\\Y").cloned().unwrap_or_default();
                let mut val = SigSpec { bits: vec![inv] };
                extend_u0(&mut val, y.size(), false);
                am.add(&y, &val);
                connect(module, y, val);
                remove_cell_by_name(module, &name);
                did = true;
                    crate::proc::dbg_inc_what("?");
                continue;
            }
        }

        // ---- mux select invert(:1064-1072)----
        if matches!(ctype.as_str(), "$_MUX_" | "$mux") {
            let cell = &design.modules[mi].cells.iter().find(|c| c.name == name).unwrap();
            let sb = am.map(get_port(cell, "\\S").unwrap()).bits[0].clone();
            if let Some(inv) = invert_map.get(&sb).cloned() {
                let module = &mut design.modules[mi];
                let ci = module.cells.iter().position(|c| c.name == name).unwrap();
                let a = get_port(&module.cells[ci], "\\A").cloned().unwrap_or_default();
                let b = get_port(&module.cells[ci], "\\B").cloned().unwrap_or_default();
                set_port(&mut module.cells[ci], "\\A", b);
                set_port(&mut module.cells[ci], "\\B", a);
                set_port(&mut module.cells[ci], "\\S", SigSpec { bits: vec![inv] });
                did = true;
                    crate::proc::dbg_inc_what("?");
                continue;
            }
        }

        // ---- gate match 家族(:1074-1163)----
        {
            let cell = &design.modules[mi].cells.iter().find(|c| c.name == name).unwrap();
            let mut gate_action: Option<(String, SigSpec)> = None;
            let mut gate_transform = false;
            match ctype.as_str() {
                "$_NOT_" => {
                    let mut input = am.map(get_port(cell, "\\A").unwrap_or(&SigSpec::default()));
                    am.apply(&mut input);
                    if sig_match(&input, "1") {
                        gate_action = Some(("\\Y".into(), one_bit(Bit::S0)));
                    } else if sig_match(&input, "0") {
                        gate_action = Some(("\\Y".into(), one_bit(Bit::S1)));
                    } else if sig_match(&input, "*") {
                        gate_action = Some(("\\Y".into(), one_bit(Bit::Sx)));
                    }
                }
                "$_AND_" | "$_OR_" | "$_XOR_" => {
                    let mut input = SigSpec::default();
                    input.bits.extend(am.map(get_port(cell, "\\B").unwrap_or(&SigSpec::default())).bits);
                    input.bits.extend(am.map(get_port(cell, "\\A").unwrap_or(&SigSpec::default())).bits);
                    am.apply(&mut input);
                    let is_and = ctype == "$_AND_";
                    let is_or = ctype == "$_OR_";
                    if is_and
                        && (sig_match(&input, " 0") || sig_match(&input, "0 ") || sig_match(&input, "11")
                            || sig_match(&input, "**") || sig_match(&input, "1*") || sig_match(&input, "*1"))
                    {
                        gate_action = Some(("\\Y".into(), one_bit(Bit::S0)));
                    } else if is_or
                        && (sig_match(&input, " 1") || sig_match(&input, "1 ") || sig_match(&input, "00")
                            || sig_match(&input, "**") || sig_match(&input, "0*") || sig_match(&input, "*0"))
                    {
                        gate_action = Some(("\\Y".into(), one_bit(Bit::S1)));
                    } else if is_and && sig_match(&input, " 1") {
                        gate_action = Some(("\\Y".into(), input.extract(1, 1)));
                    } else if is_and && sig_match(&input, "1 ") {
                        gate_action = Some(("\\Y".into(), input.extract(0, 1)));
                    } else if is_or && sig_match(&input, " 0") {
                        gate_action = Some(("\\Y".into(), input.extract(1, 1)));
                    } else if is_or && sig_match(&input, "0 ") {
                        gate_action = Some(("\\Y".into(), input.extract(0, 1)));
                    } else if ctype == "$_XOR_" {
                        if sig_match(&input, "00") || sig_match(&input, "11") {
                            gate_action = Some(("\\Y".into(), one_bit(Bit::S0)));
                        } else if sig_match(&input, "01") || sig_match(&input, "10") {
                            gate_action = Some(("\\Y".into(), one_bit(Bit::S1)));
                        }
                    }
                }
                "$_MUX_" => {
                    let mut input = SigSpec::default();
                    input.bits.extend(am.map(get_port(cell, "\\S").unwrap_or(&SigSpec::default())).bits);
                    input.bits.extend(am.map(get_port(cell, "\\B").unwrap_or(&SigSpec::default())).bits);
                    input.bits.extend(am.map(get_port(cell, "\\A").unwrap_or(&SigSpec::default())).bits);
                    am.apply(&mut input);
                    if input.extract(2, 1) == input.extract(1, 1) {
                        gate_action = Some(("\\Y".into(), input.extract(2, 1)));
                    } else if sig_match(&input, "  0") {
                        gate_action = Some(("\\Y".into(), input.extract(2, 1)));
                    } else if sig_match(&input, "  1") {
                        gate_action = Some(("\\Y".into(), input.extract(1, 1)));
                    } else if sig_match(&input, "01 ") {
                        gate_action = Some(("\\Y".into(), input.extract(0, 1)));
                    } else if sig_match(&input, "10 ") {
                        gate_transform = true; // 變 $_NOT_(A=S)
                    } else if sig_match(&input, "11 ") {
                        gate_action = Some(("\\Y".into(), one_bit(Bit::S1)));
                    } else if sig_match(&input, "00 ") {
                        gate_action = Some(("\\Y".into(), one_bit(Bit::S0)));
                    } else if sig_match(&input, "** ") {
                        gate_action = Some(("\\Y".into(), one_bit(Bit::Sx)));
                    } else if sig_match(&input, "01*") {
                        gate_action = Some(("\\Y".into(), one_bit(Bit::Sx)));
                    } else if sig_match(&input, "10*") {
                        gate_action = Some(("\\Y".into(), one_bit(Bit::Sx)));
                    }
                }
                "$_TBUF_" | "$tribuf" => {
                    let eport = if ctype == "$_TBUF_" { "\\E" } else { "\\EN" };
                    let mut input = am.map(get_port(cell, eport).unwrap_or(&SigSpec::default()));
                    let a = am.map(get_port(cell, "\\A").unwrap_or(&SigSpec::default()));
                    am.apply(&mut input);
                    if input == one_bit(Bit::S1) {
                        gate_action = Some(("\\Y".into(), get_port(cell, "\\A").cloned().unwrap_or_default()));
                    } else if input == one_bit(Bit::S0) && !is_fully_undef(&a) {
                        let module = &mut design.modules[mi];
                        let ci = module.cells.iter().position(|c| c.name == name).unwrap();
                        set_port(&mut module.cells[ci], "\\A", sig_const(Bit::Sx, a.size()));
                        did = true;
                    crate::proc::dbg_inc_what("?");
                        continue;
                    }
                }
                _ => {}
            }
            if gate_transform {
                let module = &mut design.modules[mi];
                let ci = module.cells.iter().position(|c| c.name == name).unwrap();
                module.cells[ci].ctype = "$_NOT_".to_string();
                let s = get_port(&module.cells[ci], "\\S").cloned().unwrap_or_default();
                set_port(&mut module.cells[ci], "\\A", s);
                unset_port(&mut module.cells[ci], "\\B");
                unset_port(&mut module.cells[ci], "\\S");
                continue;
            }
            if let Some((port, val)) = gate_action {
                let module = &mut design.modules[mi];
                let y = get_port(module.cells.iter().find(|c| c.name == name).unwrap(), &port).cloned().unwrap_or_default();
                let mut val = val;
                extend_u0(&mut val, y.size(), false);
                am.add(&y, &val);
                connect(module, y, val);
                remove_cell_by_name(module, &name);
                did = true;
                    crate::proc::dbg_inc_what("?");
                continue;
            }
        }

        // ---- $eq/$ne/$eqx/$nex(:1181-1221)----
        if matches!(ctype.as_str(), "$eq" | "$ne" | "$eqx" | "$nex") {
            let cell = &design.modules[mi].cells.iter().find(|c| c.name == name).unwrap();
            let mut a = get_port(cell, "\\A").cloned().unwrap_or_default();
            let mut b = get_port(cell, "\\B").cloned().unwrap_or_default();
            let aw = param_int(cell, "\\A_WIDTH");
            let bw = param_int(cell, "\\B_WIDTH");
            if aw != bw {
                let width = aw.max(bw);
                let both = param_bool(cell, "\\A_SIGNED") && param_bool(cell, "\\B_SIGNED");
                extend_u0(&mut a, width, both);
                extend_u0(&mut b, width, both);
            }
            let mut new_a = SigSpec::default();
            let mut new_b = SigSpec::default();
            let mut hit_isneq = false;
            for i in 0..a.size() as usize {
                let ab = &a.bits[i];
                let bb = &b.bits[i];
                if matches!(ab, SigBit::Const(_)) && matches!(bb, SigBit::Const(_)) && ab != bb
                    && matches!(ab, SigBit::Const(Bit::S0) | SigBit::Const(Bit::S1))
                    && matches!(bb, SigBit::Const(Bit::S0) | SigBit::Const(Bit::S1))
                {
                    hit_isneq = true;
                    break;
                }
                if ab == bb {
                    continue;
                }
                new_a.bits.push(ab.clone());
                new_b.bits.push(bb.clone());
            }
            if hit_isneq {
                let ywidth = param_int(cell, "\\Y_WIDTH");
                let base = if matches!(ctype.as_str(), "$eq" | "$eqx") { Bit::S0 } else { Bit::S1 };
                let mut yv = sig_const(base, 1);
                extend_u0(&mut yv, ywidth, false);
                let module = &mut design.modules[mi];
                let y = get_port(module.cells.iter().find(|c| c.name == name).unwrap(), "\\Y").cloned().unwrap_or_default();
                extend_u0(&mut yv, y.size(), false);
                am.add(&y, &yv);
                connect(module, y, yv);
                remove_cell_by_name(module, &name);
                did = true;
                    crate::proc::dbg_inc_what("?");
                continue;
            }
            if new_a.bits.is_empty() {
                let ywidth = param_int(cell, "\\Y_WIDTH");
                let base = if matches!(ctype.as_str(), "$eq" | "$eqx") { Bit::S1 } else { Bit::S0 };
                let mut yv = sig_const(base, 1);
                extend_u0(&mut yv, ywidth, false);
                let module = &mut design.modules[mi];
                let y = get_port(module.cells.iter().find(|c| c.name == name).unwrap(), "\\Y").cloned().unwrap_or_default();
                extend_u0(&mut yv, y.size(), false);
                am.add(&y, &yv);
                connect(module, y, yv);
                remove_cell_by_name(module, &name);
                did = true;
                    crate::proc::dbg_inc_what("?");
                continue;
            }
            if new_a.size() < a.size() || new_b.size() < b.size() {
                let (na_sz, nb_sz) = (new_a.size(), new_b.size());
                let module = &mut design.modules[mi];
                let ci = module.cells.iter().position(|c| c.name == name).unwrap();
                set_port(&mut module.cells[ci], "\\A", new_a);
                set_port(&mut module.cells[ci], "\\B", new_b);
                set_param(&mut module.cells[ci], "\\A_WIDTH", i32c(na_sz));
                set_param(&mut module.cells[ci], "\\B_WIDTH", i32c(nb_sz));
            }
        }

        // ---- $eq/$ne 寬 1×3(:1223-1253)----
        if matches!(ctype.as_str(), "$eq" | "$ne") {
            let cell = &design.modules[mi].cells.iter().find(|c| c.name == name).unwrap();
            if param_int(cell, "\\Y_WIDTH") == 1
                && param_int(cell, "\\A_WIDTH") == 1
                && param_int(cell, "\\B_WIDTH") == 1
            {
                let mut a = am.map(get_port(cell, "\\A").unwrap_or(&SigSpec::default()));
                let mut b = am.map(get_port(cell, "\\B").unwrap_or(&SigSpec::default()));
                if is_fully_const(&a) && !is_fully_const(&b) {
                    let module = &mut design.modules[mi];
                    let ci = module.cells.iter().position(|c| c.name == name).unwrap();
                    let av = a.clone();
                    set_port(&mut module.cells[ci], "\\A", b.clone());
                    set_port(&mut module.cells[ci], "\\B", av);
                    std::mem::swap(&mut a, &mut b);
                }
                if is_fully_const(&b) {
                    if is_fully_undef(&b) {
                        let yw = {
                            let cell = &design.modules[mi].cells.iter().find(|c| c.name == name).unwrap();
                            get_port(cell, "\\Y").map(|s| s.size()).unwrap_or(1)
                        };
                        let module = &mut design.modules[mi];
                        let y = get_port(module.cells.iter().find(|c| c.name == name).unwrap(), "\\Y").cloned().unwrap_or_default();
                        let mut val = sig_const(Bit::Sx, yw);
                        extend_u0(&mut val, y.size(), false);
                        am.add(&y, &val);
                        connect(module, y, val);
                        remove_cell_by_name(module, &name);
                        did = true;
                    crate::proc::dbg_inc_what("?");
                        continue;
                    }
                    let b_bool = const_as_bool(&as_const(&b));
                    if b_bool == (ctype == "$eq") {
                        let av = {
                            let cell = &design.modules[mi].cells.iter().find(|c| c.name == name).unwrap();
                            get_port(cell, "\\A").cloned().unwrap_or_default()
                        };
                        let module = &mut design.modules[mi];
                        let y = get_port(module.cells.iter().find(|c| c.name == name).unwrap(), "\\Y").cloned().unwrap_or_default();
                        let mut val = av;
                        extend_u0(&mut val, y.size(), false);
                        am.add(&y, &val);
                        connect(module, y, val);
                        remove_cell_by_name(module, &name);
                        did = true;
                    crate::proc::dbg_inc_what("?");
                        continue;
                    } else {
                        let module = &mut design.modules[mi];
                        let ci = module.cells.iter().position(|c| c.name == name).unwrap();
                        module.cells[ci].ctype = "$not".to_string();
                        unset_param(&mut module.cells[ci], "\\B_WIDTH");
                        unset_param(&mut module.cells[ci], "\\B_SIGNED");
                        unset_port(&mut module.cells[ci], "\\B");
                        did = true;
                    crate::proc::dbg_inc_what("?");
                        continue;
                    }
                }
            }
        }

        // ---- $eq/$ne 全零輸入(:1255-1271)----
        if matches!(ctype.as_str(), "$eq" | "$ne") {
            let cell = &design.modules[mi].cells.iter().find(|c| c.name == name).unwrap();
            let a = am.map(get_port(cell, "\\A").unwrap_or(&SigSpec::default()));
            let b = am.map(get_port(cell, "\\B").unwrap_or(&SigSpec::default()));
            if is_fully_zero(&a) || is_fully_zero(&b) {
                let module = &mut design.modules[mi];
                let ci = module.cells.iter().position(|c| c.name == name).unwrap();
                if ctype == "$eq" {
                    module.cells[ci].ctype = "$logic_not".to_string();
                } else {
                    module.cells[ci].ctype = "$reduce_bool".to_string();
                }
                if is_fully_zero(&a) {
                    let bv = get_port(&module.cells[ci], "\\B").cloned().unwrap_or_default();
                    set_port(&mut module.cells[ci], "\\A", bv);
                    let bs = module.cells[ci].parameters.get("\\B_SIGNED").cloned();
                    if let Some(v) = bs {
                        set_param(&mut module.cells[ci], "\\A_SIGNED", v);
                    }
                    let bw = param_int(&module.cells[ci], "\\B_WIDTH");
                    set_param(&mut module.cells[ci], "\\A_WIDTH", i32c(bw));
                }
                unset_port(&mut module.cells[ci], "\\B");
                unset_param(&mut module.cells[ci], "\\B_SIGNED");
                unset_param(&mut module.cells[ci], "\\B_WIDTH");
                did = true;
                    crate::proc::dbg_inc_what("?");
                continue;
            }
        }

        // ---- shift 固定接線(:1273-1313;keepdc=is_fully_def)----
        if matches!(ctype.as_str(), "$shl" | "$shr" | "$sshl" | "$sshr" | "$shift" | "$shiftx") {
            let cell = &design.modules[mi].cells.iter().find(|c| c.name == name).unwrap();
            let b = am.map(get_port(cell, "\\B").unwrap_or(&SigSpec::default()));
            if is_fully_def(&b) {
                let sign_ext = ctype == "$sshr" && param_bool(cell, "\\A_SIGNED");
                let b_sign_ext = matches!(ctype.as_str(), "$shift" | "$shiftx") && param_bool(cell, "\\B_SIGNED");
                let mut shift_bits = ce::const_as_int_sat(&as_const(&b), b_sign_ext);
                if matches!(ctype.as_str(), "$shl" | "$sshl") {
                    shift_bits = -shift_bits;
                }
                let mut sig_a = am.map(get_port(cell, "\\A").unwrap_or(&SigSpec::default()));
                let ywidth = param_int(cell, "\\Y_WIDTH");
                let mut sig_y = sig_const(if ctype == "$shiftx" { Bit::Sx } else { Bit::S0 }, ywidth);
                if ctype != "$shiftx" && sig_a.size() < ywidth {
                    extend_u0(&mut sig_a, ywidth, param_bool(cell, "\\A_SIGNED"));
                }
                if shift_bits > sig_a.size() {
                    shift_bits = sig_a.size();
                }
                for i in 0..ywidth.max(0) as usize {
                    let idx = i as i64 + shift_bits;
                    if 0 <= idx && idx < sig_a.size() {
                        sig_y.bits[i] = sig_a.bits[idx as usize].clone();
                    } else if sig_a.size() <= idx && sign_ext && !sig_a.bits.is_empty() {
                        sig_y.bits[i] = sig_a.bits.last().unwrap().clone();
                    }
                }
                let module = &mut design.modules[mi];
                let y = get_port(module.cells.iter().find(|c| c.name == name).unwrap(), "\\Y").cloned().unwrap_or_default();
                am.add(&y, &sig_y);
                connect(module, y, sig_y);
                remove_cell_by_name(module, &name);
                did = true;
                    crate::proc::dbg_inc_what("?");
                continue;
            }
        }

        // ---- FOLD 家族(:1598-1648)----
        if fold_const_cells(design, mi, &name, &ctype, &mut am) {
            did = true;
                    crate::proc::dbg_inc_what("FOLD 家族(:1598-1648)");
            continue;
        }

        // ---- $mux 保守(:1651-1659)----
        if ctype == "$mux" {
            let cell = &design.modules[mi].cells.iter().find(|c| c.name == name).unwrap();
            let input = am.map(get_port(cell, "\\S").unwrap_or(&SigSpec::default()));
            let in_a = am.map(get_port(cell, "\\A").unwrap_or(&SigSpec::default()));
            let in_b = am.map(get_port(cell, "\\B").unwrap_or(&SigSpec::default()));
            let mut action: Option<SigSpec> = None;
            if is_fully_const(&input) && is_fully_def(&input) {
                let sel = const_as_bool(&as_const(&input));
                action = Some(if sel {
                    get_port(cell, "\\B").cloned().unwrap_or_default()
                } else {
                    get_port(cell, "\\A").cloned().unwrap_or_default()
                });
            } else if in_a == in_b {
                action = Some(get_port(cell, "\\A").cloned().unwrap_or_default());
            }
            if let Some(val) = action {
                let module = &mut design.modules[mi];
                let y = get_port(module.cells.iter().find(|c| c.name == name).unwrap(), "\\Y").cloned().unwrap_or_default();
                let mut val = val;
                extend_u0(&mut val, y.size(), false);
                am.add(&y, &val);
                connect(module, y, val);
                remove_cell_by_name(module, &name);
                did = true;
                    crate::proc::dbg_inc_what("?");
                continue;
            }
        }

        // ---- $pow onehot(:1660-1694)----
        if ctype == "$pow" {
            let cell = &design.modules[mi].cells.iter().find(|c| c.name == name).unwrap();
            let a = get_port(cell, "\\A").cloned().unwrap_or_default();
            if is_fully_const(&a) && !param_bool(cell, "\\B_SIGNED") {
                let sig_a = am.map(&a);
                if let Some(bit_idx) = ce::const_is_onehot(&as_const(&sig_a)) {
                    let a_signed = param_bool(cell, "\\A_SIGNED");
                    if !a_signed || (bit_idx as i64) < sig_a.size() - 1 {
                        let src = snap.2.clone();
                        let y_size = get_port(cell, "\\Y").map(|s| s.size()).unwrap_or(0);
                        let module = &mut design.modules[mi];
                        let autoidx = &mut design.autoidx;
                        let mut pm = Pm { module, autoidx };
                        let ci = pm.module().cells.iter().position(|c| c.name == name).unwrap();
                        set_param(&mut pm.module().cells[ci], "\\A_SIGNED", i32cb(false));
                        if bit_idx == 1 {
                            pm.module().cells[ci].ctype = "$shl".to_string();
                            set_param(&mut pm.module().cells[ci], "\\A_WIDTH", i32c(1));
                            set_port(&mut pm.module().cells[ci], "\\A", one_bit(Bit::S1));
                        } else {
                            pm.module().cells[ci].ctype = "$mul".to_string();
                            let aw = param_int(&pm.module().cells[ci], "\\A_WIDTH");
                            let mut aval = sig_const(Bit::S0, aw);
                            // Const(bit_idx, A_WIDTH)
                            let bc = i32c(bit_idx as i64);
                            if let Const::Bits { bits, .. } = &bc {
                                for (i, b) in bits.iter().enumerate().take(aw as usize) {
                                    aval.bits[i] = SigBit::Const(*b);
                                }
                            }
                            set_port(&mut pm.module().cells[ci], "\\A", aval);
                            let y_wire_id = pm.new_id_at("opt_expr.cc", 1686, "replace_const_cells");
                            let y_wire = pm.add_wire_id(y_wire_id, y_size);
                            set_port(&mut pm.module().cells[ci], "\\Y", y_wire.clone());
                            let shlid = pm.new_id_at("opt_expr.cc", 1689, "replace_const_cells");
                            // addShl(NEW_ID, Const(S1,1), y_wire, sig_y)
                            let _ = add_shl(&mut pm, shlid, &sig_const(Bit::S1, 1), &y_wire, &sig_const(Bit::S0, y_size), src.as_ref());
                        }
                        did = true;
                    crate::proc::dbg_inc_what("?");
                        continue;
                    }
                }
            }
        }
    }
    did
}

/// addShl(NEW_ID, a, b, y)(rtlil.cc DEF_METHOD Shl 組:B_SIGNED=false)。
fn add_shl(pm: &mut Pm, name: String, a: &SigSpec, b: &SigSpec, y: &SigSpec, src: Option<&Const>) -> SigSpec {
    pm.add_cell(
        name,
        "$shl",
        vec![
            ("\\A_SIGNED", Const::from_i64_i32_width(0)),
            ("\\B_SIGNED", Const::from_i64_i32_width(0)),
            ("\\A_WIDTH", Const::from_i64_i32_width(a.size())),
            ("\\B_WIDTH", Const::from_i64_i32_width(b.size())),
            ("\\Y_WIDTH", Const::from_i64_i32_width(y.size())),
        ],
        vec![
            ("\\A", a.clone()),
            ("\\B", b.clone()),
            ("\\Y", y.clone()),
        ],
        src,
    );
    y.clone()
}

/// FOLD_1ARG/2ARG/MUX/SIMPLE 家族(全常數輸入→const 求值→replace)。
fn fold_const_cells(design: &mut Design, mi: usize, name: &str, ctype: &str, am: &mut AssignMap) -> bool {
    let Some(cell) = design.modules[mi].cells.iter().find(|c| c.name == name) else {
        return false;
    };
    macro_rules! fold1 {
        ($ev:expr) => {{
            let a = get_port(cell, "\\A").cloned().unwrap_or_default();
            let mut am_a = am.map(&a);
            am.apply(&mut am_a);
            if is_fully_const(&am_a) {
                let yw = param_int(cell, "\\Y_WIDTH");
                let asig = param_bool(cell, "\\A_SIGNED");
                let dummy = Const::Bits { width: 1, bits: vec![Bit::S0], signed: false, is_unsized: false };
                let y: Const = $ev(&as_const(&am_a), &dummy, asig, false, yw);
                let val = sig_from_const_fold(&y);
                let _ = &cell;
                replace_after_fold(design, mi, name, "\\Y", val, am);
                return true;
            }
        }};
    }
    macro_rules! fold2 {
        ($ev:expr) => {{
            let a = get_port(cell, "\\A").cloned().unwrap_or_default();
            let b = get_port(cell, "\\B").cloned().unwrap_or_default();
            let mut am_a = am.map(&a);
            let mut am_b = am.map(&b);
            am.apply(&mut am_a);
            am.apply(&mut am_b);
            if is_fully_const(&am_a) && is_fully_const(&am_b) {
                let yw = param_int(cell, "\\Y_WIDTH");
                let asig = param_bool(cell, "\\A_SIGNED");
                let bsig = param_bool(cell, "\\B_SIGNED");
                let y: Const = $ev(&as_const(&am_a), &as_const(&am_b), asig, bsig, yw);
                let val = sig_from_const_fold(&y);
                let _ = &cell;
                replace_after_fold(design, mi, name, "\\Y", val, am);
                return true;
            }
        }};
    }
    macro_rules! fold2simple {
        ($ev:expr, $bid:expr) => {{
            let a = get_port(cell, "\\A").cloned().unwrap_or_default();
            let b = get_port(cell, $bid).cloned().unwrap_or_default();
            let mut am_a = am.map(&a);
            let mut am_b = am.map(&b);
            am.apply(&mut am_a);
            am.apply(&mut am_b);
            if is_fully_const(&am_a) && is_fully_const(&am_b) {
                let y: Const = $ev(&as_const(&am_a), &as_const(&am_b));
                let val = sig_from_const_fold(&y);
                let _ = &cell;
                replace_after_fold(design, mi, name, "\\Y", val, am);
                return true;
            }
        }};
    }
    macro_rules! foldmux {
        ($ev:expr) => {{
            let a = get_port(cell, "\\A").cloned().unwrap_or_default();
            let b = get_port(cell, "\\B").cloned().unwrap_or_default();
            let s = get_port(cell, "\\S").cloned().unwrap_or_default();
            let mut am_a = am.map(&a);
            let mut am_b = am.map(&b);
            let mut am_s = am.map(&s);
            am.apply(&mut am_a);
            am.apply(&mut am_b);
            am.apply(&mut am_s);
            if is_fully_const(&am_a) && is_fully_const(&am_b) && is_fully_const(&am_s) {
                let y: Const = $ev(&as_const(&am_a), &as_const(&am_b), &as_const(&am_s));
                let val = sig_from_const_fold(&y);
                let _ = &cell;
                replace_after_fold(design, mi, name, "\\Y", val, am);
                return true;
            }
        }};
    }

    match ctype {
        "$not" => fold1!(ce::const_not),
        "$and" => fold2!(ce::const_and),
        "$or" => fold2!(ce::const_or),
        "$xor" => fold2!(ce::const_xor),
        "$xnor" => fold2!(ce::const_xnor),
        "$reduce_and" => fold1!(ce::const_reduce_and),
        "$reduce_or" => fold1!(ce::const_reduce_or),
        "$reduce_xor" => fold1!(ce::const_reduce_xor),
        "$reduce_xnor" => fold1!(ce::const_reduce_xnor),
        "$reduce_bool" => fold1!(ce::const_reduce_bool),
        "$logic_not" => fold1!(ce::const_logic_not),
        "$logic_and" => fold2!(ce::const_logic_and),
        "$logic_or" => fold2!(ce::const_logic_or),
        "$shl" => fold2!(ce::const_shl),
        "$shr" => fold2!(ce::const_shr),
        "$sshl" => fold2!(ce::const_sshl),
        "$sshr" => fold2!(ce::const_sshr),
        "$shift" => fold2!(ce::const_shift),
        "$shiftx" => fold2!(ce::const_shiftx),
        "$lt" => fold2!(ce::const_lt),
        "$le" => fold2!(ce::const_le),
        "$eq" => fold2!(ce::const_eq),
        "$ne" => fold2!(ce::const_ne),
        "$gt" => fold2!(ce::const_gt),
        "$ge" => fold2!(ce::const_ge),
        "$eqx" => fold2!(ce::const_eqx),
        "$nex" => fold2!(ce::const_nex),
        "$add" => fold2!(ce::const_add),
        "$sub" => fold2!(ce::const_sub),
        "$mul" => fold2!(ce::const_mul),
        "$div" => fold2!(ce::const_div),
        "$mod" => fold2!(ce::const_mod),
        "$divfloor" => fold2!(ce::const_divfloor),
        "$modfloor" => fold2!(ce::const_modfloor),
        "$pow" => fold2!(ce::const_pow),
        "$pos" => fold1!(ce::const_pos),
        "$neg" => fold1!(ce::const_neg),
        "$mux" => foldmux!(ce::const_mux),
        "$pmux" => foldmux!(ce::const_pmux),
        "$bmux" => fold2simple!(ce::const_bmux, "\\S"),
        "$demux" => fold2simple!(ce::const_demux, "\\S"),
        "$bweqx" => fold2simple!(ce::const_bweqx, "\\B"),
        "$bwmux" => foldmux!(ce::const_bwmux),
        _ => {}
    }
    false
}

fn sig_from_const_fold(c: &Const) -> SigSpec {
    match c {
        Const::Str(_) => SigSpec::default(),
        Const::Bits { bits, .. } => SigSpec { bits: bits.iter().map(|&b| SigBit::Const(b)).collect() },
    }
}

fn replace_after_fold(design: &mut Design, mi: usize, name: &str, out_port: &str, out_val: SigSpec, am: &mut AssignMap) {
    let Some(ci) = design.modules[mi].cells.iter().position(|c| c.name == name) else {
        return;
    };
    let y = get_port(&design.modules[mi].cells[ci], out_port).cloned().unwrap_or_default();
    let mut val = out_val;
    extend_u0(&mut val, y.size(), false);
    am.add(&y, &val);
    let module = &mut design.modules[mi];
    connect(module, y, val);
    remove_cell_by_name(module, name);
}

/// replace_const_connections(opt_expr.cc:1756)。
fn replace_const_connections(module: &mut ry_ir::Module) -> bool {
    let am = AssignMap::new(module);
    let mut did = false;
    for ci in 0..module.cells.len() {
        let mut changes: Vec<(String, SigSpec)> = Vec::new();
        for (port, conn) in &module.cells[ci].connections {
            let mapped = am.map(conn);
            if *conn != mapped && is_fully_const(&mapped) {
                changes.push((port.clone(), mapped));
            }
        }
        if !changes.is_empty() {
            did = true;
        }
        for (port, sig) in changes {
            set_port(&mut module.cells[ci], &port, sig);
        }
    }
    did
}

/// `opt_expr` 命令面(僅 `-keepdc` 契約形;其餘旗標=Y2)。
pub fn run_opt_expr(design: &mut Design, args: &[String], log: &mut crate::Log) -> Result<(), ProcError> {
    for a in args.get(1..).unwrap_or(&[]) {
        if a == "-keepdc" {
            continue;
        }
        if a.starts_with('-') {
            return Err(err(format!(
                "opt_expr: option `{a}' is not implemented in ryosys (milestone Y2)"
            )));
        }
        return Err(err("selection is not implemented in ryosys (milestone Y0)"));
    }
    log.header("Executing OPT_EXPR pass (perform const folding).\n");
    log.push();
    let mut out = std::mem::take(&mut log.out);
    for mi in (0..design.modules.len()).rev() {
        let mname = design.modules[mi].name.clone();
        let m = mname.strip_prefix('\\').unwrap_or(&mname).to_string();
        out.push_str(&format!("Optimizing module {m}.\n"));
        // do { do { …(consume_x=false)… } while(did); if(!keepdc)… } while(did)
        // keepdc=true → consume_x 腿不跑,外層僅一輪。
        let mut did = replace_const_cells(design, mi, &mut out);
        while did {
            did = replace_const_cells(design, mi, &mut out);
        }
        let _ = replace_const_connections(&mut design.modules[mi]);
    }
    log.out = out;
    log.pop();
    log.resync();
    Ok(())
}
