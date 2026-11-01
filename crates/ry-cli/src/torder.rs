//! torder(passes/cmds/torder.cc 同構移植;RYOSYS-MVP §11.3-6)。
//!
//! * TopoSort<IdString, sort_by_id_str>(torder.cc:81)=**自指定字串序**
//!   比較器——intern 名序律(AGENTS 10.9-r)**例外面第三例**(前兩例=
//!   D-ys8b lt_by_name+opt_balance_tree compare_ptr_by_name;Q7 源碼
//!   親讀+入場分類)。lt_by_name=Substrings 分段memcmp,autoidx 前綴+
//!   十進尾碼=文本形 ⇒ 等價整串 byte 比較(jsonout::name_lt 同源)。
//! * 節點序=std::map 迭代=**字串序**(非插入序);edges[r]=set<IndirectCmp>
//!   =入邊按源節點字串序+去重;DFS 後序=sorted。loops=std::set<vector>
//!   (輸出序面=intern 序 lexicographic;語料零觸發,我方=首見序,遇
//!   diff 按 §8.8-2 分類)。
//! * autostop 預設集(:89-92):Q/CTRL_OUT/RD_DATA+$memrd/$memrd_v2 DATA
//!   (cell_known 門);-stop/-noautostop 已知旗標=Y2 硬錯(真實行為=
//!   行為面歧);未知旗標/selection=cmd_error(D-ys11)。

use std::collections::HashMap;

use ry_ir::{Cell, Design, Module};

use crate::help_desc::HELP_TORDER;
use crate::opt::{cmd_error_pass, module_is_boxed};
use crate::sigmap::SigMap;

/// std::map<IdString,int,sort_by_id_str> 迭代序=字串序;edges set 排序同。
fn name_ord(a: &str, b: &str) -> std::cmp::Ordering {
    crate::jsonout::name_lt(a, b)
}

struct TorderTopo {
    names: Vec<String>,
    /// names 下標按字串序(=std::map 迭代序)。
    order: Vec<usize>,
    edges: Vec<Vec<usize>>,
    sorted: Vec<usize>,
    /// loops(首見序;std::set<vector> 的 intern 序 lexicographic 輸出面
    /// =語料零觸發邊界,見檔頭註)。
    loops: Vec<Vec<usize>>,
}

impl TorderTopo {
    fn new() -> TorderTopo {
        TorderTopo { names: Vec::new(), order: Vec::new(), edges: Vec::new(), sorted: Vec::new(), loops: Vec::new() }
    }

    fn node(&mut self, n: &str) -> usize {
        if let Some(i) = self.names.iter().position(|x| x == n) {
            return i;
        }
        self.names.push(n.to_string());
        self.edges.push(Vec::new());
        let i = self.names.len() - 1;
        self.order.push(i);
        self.order.sort_by(|&a, &b| name_ord(&self.names[a], &self.names[b]));
        i
    }

    fn edge(&mut self, l: usize, r: usize) {
        let e = &mut self.edges[r];
        if !e.contains(&l) {
            e.push(l);
        }
        let names = &self.names;
        e.sort_by(|&a, &b| name_ord(&names[a], &names[b]));
    }

    fn sort(&mut self) {
        let n = self.names.len();
        let mut on_stack = vec![false; n];
        let mut done = vec![false; n];
        let mut stack: Vec<usize> = Vec::new();
        let order = self.order.clone();
        for i in order {
            self.worker(i, &mut done, &mut on_stack, &mut stack);
        }
    }

    fn worker(&mut self, root: usize, done: &mut [bool], on_stack: &mut [bool], stack: &mut Vec<usize>) {
        if on_stack[root] {
            // loop:stack 頂向下游走至 root。
            let mut loop_: Vec<usize> = Vec::new();
            for &idx in stack.iter().rev() {
                loop_.push(idx);
                if idx == root {
                    break;
                }
            }
            if !self.loops.contains(&loop_) {
                self.loops.push(loop_);
            }
            return;
        }
        if done[root] {
            return;
        }
        if !self.edges[root].is_empty() {
            stack.push(root);
            on_stack[root] = true;
            let into: Vec<usize> = self.edges[root].clone();
            for left in into {
                self.worker(left, done, on_stack, stack);
            }
            stack.pop();
            on_stack[root] = false;
        }
        done[root] = true;
        self.sorted.push(root);
    }
}

/// 命令面(§11.3-6)。
pub fn run_torder(
    design: &mut Design,
    args: &[String],
    log: &mut crate::Log,
) -> Result<(), crate::proc::ProcError> {
    log.header("Executing TORDER pass (print cells in topological order).\n");
    // argidx 迴圈(:75-86):-stop(雙參)/-noautostop 已知旗標=Y2(真實
    // 行為=行為面歧;`-stop` 單投=oracle cmd_error 形亦=Y2 內);其餘
    // break→extra_args 面=cmd_error(selection 裸字=D-ys11)。
    for a in args.get(1..).unwrap_or(&[]) {
        if matches!(a.as_str(), "-stop" | "-noautostop") {
            return Err(crate::proc::ProcError(format!(
                "torder: option `{a}' is not implemented in ryosys (milestone Y2)"
            )));
        }
        break;
    }
    if args.len() > 1 {
        return Err(crate::proc::ProcError(
            cmd_error_pass(log, args, 1, HELP_TORDER).0,
        ));
    }
    for mi in (0..design.modules.len()).rev() {
        if module_is_boxed(&design.modules[mi]) {
            continue;
        }
        let m: &Module = &design.modules[mi];
        log.log(&format!("module {}\n", m.name.trim_start_matches('\\')));
        let am = {
            let mut am = SigMap::new();
            am.set(m);
            am
        };
        let mut topo = TorderTopo::new();
        let mut bit_users: HashMap<ry_ir::SigBit, Vec<String>> = HashMap::new();
        let mut bit_drivers: HashMap<ry_ir::SigBit, Vec<String>> = HashMap::new();
        for cell in m.cells.iter().rev() {
            for (port, sig) in cell.connections.iter().rev() {
                // autostop 預設集(:89-92;cell_known 門)。
                if crate::cells::cell_known(&cell.ctype) {
                    if matches!(port.as_str(), "\\Q" | "\\CTRL_OUT" | "\\RD_DATA") {
                        continue;
                    }
                    if matches!(cell.ctype.as_str(), "$memrd" | "$memrd_v2") && port == "\\DATA" {
                        continue;
                    }
                }
                let (is_in, is_out) = crate::cells::port_dir(&cell.ctype, port);
                let mapped = am.map(sig);
                if is_in {
                    for b in &mapped.bits {
                        let v = bit_users.entry(b.clone()).or_default();
                        if !v.contains(&cell.name) {
                            v.push(cell.name.clone());
                        }
                    }
                }
                if is_out {
                    for b in &mapped.bits {
                        let v = bit_drivers.entry(b.clone()).or_default();
                        if !v.contains(&cell.name) {
                            v.push(cell.name.clone());
                        }
                    }
                }
                topo.node(&cell.name);
            }
        }
        // bit_users dict 迭代序=逆插入(HashMap 序無關緊要:edges set
        // 自帶字串序)。
        for (bit, users) in &bit_users {
            let Some(drivers) = bit_drivers.get(bit) else { continue };
            for d in drivers {
                for u in users {
                    let di = topo.node(d);
                    let ui = topo.node(u);
                    topo.edge(di, ui);
                }
            }
        }
        topo.sort();
        for loop_ in &topo.loops {
            let mut msg = String::from("  loop");
            for &i in loop_ {
                msg.push(' ');
                msg.push_str(&topo.names[i]);
            }
            log.log(&format!("{msg}\n"));
        }
        for i in &topo.sorted {
            log.log(&format!("  cell {}\n", topo.names[*i]));
        }
    }
    Ok(())
}

/// 防 unused Cell import(Cell 於連接面 clone 用;保留形參照)。
#[allow(unused)]
fn _t(c: &Cell) {}
