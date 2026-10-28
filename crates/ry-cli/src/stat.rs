//! `stat` pass 無參子集(passes/cmds/stat.cc 移植,0.70+179 二進制實跑
//! 形態為準——v0.70 源碼的 3 行表頭與面積欄在二進制已改版,實測形態:
//! 表頭 2 行(`        +----------Local Count, excluding submodules.` +
//! `        | ` 含尾隨空格),無面積模式,本地表零值行跳過、全局表零值
//! 印 `        -`)。
//!
//! 層級統計:hierarchy_builder 把「類型=設計內模組」的 cell 從計數移入
//! submodules 並遞歸聚合;`attribute \top 1` 的模組=top → 追加
//! `=== design hierarchy ===` 段與全局彙總表(僅 mod_stat>1 時)。
//!
//! cell 類型表=std::map<sort_by_id_str> =名序;模組迭代=design->modules()
//! 正向=**逆 .il 宣告序**(與 check 同契約)。

use crate::check::module_is_top;
use ry_ir::{Design, Module};
use std::collections::BTreeMap;

#[derive(Default, Clone)]
struct StatData {
    num_wires: u64,
    num_wire_bits: u64,
    num_pub_wires: u64,
    num_pub_wire_bits: u64,
    num_ports: u64,
    num_port_bits: u64,
    num_memories: u64,
    num_memory_bits: u64,
    num_processes: u64,
    num_cells: u64,
    local_num_wires: u64,
    local_num_wire_bits: u64,
    local_num_pub_wires: u64,
    local_num_pub_wire_bits: u64,
    local_num_ports: u64,
    local_num_port_bits: u64,
    local_num_memories: u64,
    local_num_memory_bits: u64,
    local_num_processes: u64,
    local_num_cells: u64,
    num_cells_by_type: BTreeMap<Vec<u8>, u64>,   // key=存儲名 bytes(名序)
    local_num_cells_by_type: BTreeMap<Vec<u8>, u64>,
    num_submodules: u64,
    num_submodules_by_type: BTreeMap<Vec<u8>, u64>,
}

impl StatData {
    fn from_module(m: &Module) -> StatData {
        let mut d = StatData::default();
        for w in &m.wires {
            if w.port_input || w.port_output {
                d.num_ports += 1;
                d.local_num_ports += 1;
                d.num_port_bits += w.width as u64;
                d.local_num_port_bits += w.width as u64;
            }
            if !w.name.starts_with('$') {
                d.num_pub_wires += 1;
                d.local_num_pub_wires += 1;
                d.num_pub_wire_bits += w.width as u64;
                d.local_num_pub_wire_bits += w.width as u64;
            }
            d.num_wires += 1;
            d.local_num_wires += 1;
            d.num_wire_bits += w.width as u64;
            d.local_num_wire_bits += w.width as u64;
        }
        for mem in &m.memories {
            d.num_memories += 1;
            d.local_num_memories += 1;
            d.num_memory_bits += (mem.width * mem.size) as u64;
            d.local_num_memory_bits += (mem.width * mem.size) as u64;
        }
        for c in &m.cells {
            let t = c.ctype.clone();
            d.num_cells += 1;
            d.local_num_cells += 1;
            *d.num_cells_by_type.entry(t.clone().into_bytes()).or_insert(0) += 1;
            *d.local_num_cells_by_type.entry(t.into_bytes()).or_insert(0) += 1;
        }
        for _ in &m.processes {
            d.num_processes += 1;
            d.local_num_processes += 1;
        }
        d
    }

    fn add(&mut self, other: &StatData) {
        self.num_wires += other.num_wires;
        self.num_wire_bits += other.num_wire_bits;
        self.num_pub_wires += other.num_pub_wires;
        self.num_pub_wire_bits += other.num_pub_wire_bits;
        self.num_ports += other.num_ports;
        self.num_port_bits += other.num_port_bits;
        self.num_memories += other.num_memories;
        self.num_memory_bits += other.num_memory_bits;
        self.num_processes += other.num_processes;
        self.num_cells += other.num_cells;
        for (k, v) in &other.num_cells_by_type {
            *self.num_cells_by_type.entry(k.clone()).or_insert(0) += v;
        }
    }
}

fn hierarchy_builder<'m>(
    design: &'m Design,
    module: &'m Module,
    mod_stat: &mut Vec<(String, StatData)>,
) -> StatData {
    let mut d = StatData::from_module(module);
    for cell in &module.cells {
        if let Some(sub) = design.find_module(&cell.ctype) {
            if sub.attributes.get("\\blackbox").is_some() {
                continue; // blackbox:非 M0 語料路徑(普查 0 例)
            }
            let sub_data = hierarchy_builder(design, sub, mod_stat);
            d.add(&sub_data);
            *d.num_submodules_by_type.entry(cell.ctype.clone().into_bytes()).or_insert(0) += 1;
            d.num_submodules += 1;
            if let Some(cnt) = d.num_cells_by_type.get(cell.ctype.as_bytes()).copied() {
                d.num_cells -= cnt;
                d.num_cells_by_type.remove(cell.ctype.as_bytes());
            }
            if let Some(cnt) = d.local_num_cells_by_type.get(cell.ctype.as_bytes()).copied() {
                d.local_num_cells -= cnt;
                d.local_num_cells_by_type.remove(cell.ctype.as_bytes());
            }
        }
    }
    set_mod_stat(mod_stat, module.name.clone(), d.clone());
    d
}

fn get_mod_stat<'a>(mod_stat: &'a Vec<(String, StatData)>, name: &str) -> Option<&'a StatData> {
    mod_stat.iter().find(|(n, _)| n == name).map(|(_, d)| d)
}

fn set_mod_stat(mod_stat: &mut Vec<(String, StatData)>, name: String, d: StatData) {
    if let Some(e) = mod_stat.iter_mut().find(|(n, _)| *n == name) {
        e.1 = d;
    } else {
        mod_stat.push((name, d));
    }
}

/// f_val(stat.cc):8 欄右對齊;0/極小=「       -」;整數無尾零。
fn f_val_int(v: u64) -> String {
    if v == 0 {
        return "       -".to_string();
    }
    let s = v.to_string();
    format!("{}{}", " ".repeat(8usize.saturating_sub(s.len())), s)
}

fn print_log_line(
    log: &mut String,
    name: &str,
    count_local: u64,
    count_global: u64,
    spacer: usize,
    global_only: bool,
) {
    let indent = " ".repeat(2 * spacer);
    if global_only {
        log.push_str(&format!(" {} {}{}\n", f_val_int(count_global), indent, name));
    } else if count_local > 0 {
        log.push_str(&format!(" {} {}{}\n", f_val_int(count_local), indent, name));
    }
}

fn print_log_header(log: &mut String, global_only: bool) {
    let label = if global_only {
        "Count including submodules."
    } else {
        "Local Count, excluding submodules."
    };
    log.push_str(&format!("        +----------{label}\n"));
    log.push_str("        | \n");
}

fn log_data_local(log: &mut String, d: &StatData) {
    print_log_header(log, false);
    print_log_line(log, "wires", d.local_num_wires, 0, 0, false);
    print_log_line(log, "wire bits", d.local_num_wire_bits, 0, 0, false);
    print_log_line(log, "public wires", d.local_num_pub_wires, 0, 0, false);
    print_log_line(log, "public wire bits", d.local_num_pub_wire_bits, 0, 0, false);
    print_log_line(log, "ports", d.local_num_ports, 0, 0, false);
    print_log_line(log, "port bits", d.local_num_port_bits, 0, 0, false);
    print_log_line(log, "memories", d.local_num_memories, 0, 0, false);
    print_log_line(log, "memory bits", d.local_num_memory_bits, 0, 0, false);
    print_log_line(log, "processes", d.local_num_processes, 0, 0, false);
    print_log_line(log, "cells", d.local_num_cells, 0, 0, false);
    for (t, cnt) in &d.local_num_cells_by_type {
        if *cnt > 0 {
            print_log_line(
                log,
                &unescape_bytes(t),
                *cnt,
                0,
                1,
                false,
            );
        }
    }
    if d.num_submodules > 0 {
        print_log_line(log, "submodules", d.num_submodules, 0, 0, false);
        for (t, cnt) in &d.num_submodules_by_type {
            if *cnt > 0 {
                print_log_line(log, &unescape_bytes(t), *cnt, *cnt, 1, false);
            }
        }
    }
}

fn log_data_global(log: &mut String, d: &StatData) {
    print_log_header(log, true);
    print_log_line(log, "wires", 0, d.num_wires, 0, true);
    print_log_line(log, "wire bits", 0, d.num_wire_bits, 0, true);
    print_log_line(log, "public wires", 0, d.num_pub_wires, 0, true);
    print_log_line(log, "public wire bits", 0, d.num_pub_wire_bits, 0, true);
    print_log_line(log, "ports", 0, d.num_ports, 0, true);
    print_log_line(log, "port bits", 0, d.num_port_bits, 0, true);
    print_log_line(log, "memories", 0, d.num_memories, 0, true);
    print_log_line(log, "memory bits", 0, d.num_memory_bits, 0, true);
    print_log_line(log, "processes", 0, d.num_processes, 0, true);
    print_log_line(log, "cells", 0, d.num_cells, 0, true);
    for (t, cnt) in &d.num_cells_by_type {
        if *cnt > 0 {
            print_log_line(log, &unescape_bytes(t), 0, *cnt, 1, true);
        }
    }
    if d.num_submodules > 0 {
        print_log_line(log, "submodules", 0, d.num_submodules, 0, true);
        for (t, cnt) in &d.num_submodules_by_type {
            if *cnt > 0 {
                print_log_line(log, &unescape_bytes(t), 0, *cnt, 1, true);
            }
        }
    }
}

fn unescape_bytes(b: &[u8]) -> String {
    let s = String::from_utf8_lossy(b).into_owned();
    crate::jsonout::unescape(&s).to_string()
}

fn hierarchy_worker(
    mod_stat: &Vec<(String, StatData)>,
    top: &str,
    level: usize,
    log: &mut String,
) {
    if let Some(d) = get_mod_stat(mod_stat, top) {
        let subs: Vec<(Vec<u8>, u64)> = d
            .num_submodules_by_type
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        for (t, _) in subs {
            let name = String::from_utf8_lossy(&t).into_owned();
            if let Some(sub) = get_mod_stat(mod_stat, &name) {
                print_log_line(
                    log,
                    &crate::jsonout::unescape(&name),
                    sub.local_num_cells,
                    sub.num_cells,
                    level,
                    false,
                );
                hierarchy_worker(mod_stat, &name, level + 1, log);
            }
        }
    }
}

pub fn run_stat(design: &Design, log: &mut String) {    let mut mod_stat: Vec<(String, StatData)> = Vec::new();
    // 迭代=design->selected_modules()=modules() 正向=逆宣告序;boxed
    // 模組靜默跳過(同 check 的選擇契約,ifdef_1 實測無 === 段)。
    let order: Vec<&Module> = design
        .modules
        .iter()
        .rev()
        .filter(|m| !crate::check::is_boxed(m))
        .collect();
    for m in &order {
        if get_mod_stat(&mod_stat, &m.name).is_none() {
            hierarchy_builder(design, m, &mut mod_stat);
        }
    }

    let mut top_mod: Option<String> = None;
    for m in &order {
        if top_mod.is_none() && module_is_top(m) {
            top_mod = Some(m.name.clone());
        }
        let d = get_mod_stat(&mod_stat, &m.name).unwrap();
        log.push('\n');
        log.push_str(&format!(
            "=== {} ===\n",
            crate::jsonout::unescape(&m.name)
        ));
        log.push('\n');
        log_data_local(log, d);
    }

    if let Some(top) = top_mod {
        if mod_stat.len() > 1 {
            log.push('\n');
            log.push_str("=== design hierarchy ===\n");
            log.push('\n');
            let d = get_mod_stat(&mod_stat, &top).unwrap();
            print_log_header(log, true);
            print_log_line(
                log,
                crate::jsonout::unescape(&top),
                d.local_num_cells,
                d.num_cells,
                0,
                true,
            );
            hierarchy_worker(&mod_stat, &top, 0, log);
            log.push('\n');
            log_data_global(log, d);
        }
    }
    log.push('\n');
}
