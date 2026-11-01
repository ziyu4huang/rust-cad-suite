//! proc 家族 passes(RYOSYS-MVP §8;passes/proc/*.cc + passes/opt/opt_expr.cc
//! 的 -keepdc 子集移植面)。每個子 pass 的 log 文案/迭代序/NEW_ID 錨逐字
//! 對齊 oracle 0.70+179;黃金面由 ys-e2e -pb 層+prefix-chain dev probe
//! 承擔(鐵則:期望一律錄製,禁手寫)。
//!
//! 迭代序契約(RM2-6):modules()/cells()/processes() 正向迭代=逆插入序
//! (hashlib dict 正向=尾到頭;ry-ir Vec 插入序 → `.rev()`)。dict erase
//! =swap-and-pop(InsertMap::swap_pop_remove / Vec swap_remove)。
//! log 文案的 %s 形態(kernel/io.h:375 分派):IdString=原始形
//! (`\name`);Module*/Process*/Cell* 指標=unescape 形;顯式 `.c_str()`=
//! 原始形。

use crate::Log;
use crate::sigops::*;
use ry_ir::{
    Bit, CaseRule, Const, Design, InsertMap, Module, Process, SigBit, SigSpec, SyncRule, SyncType,
    SwitchRule, Wire,
};
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// 錯誤面:proc 家族實現層內的語義硬錯(oracle log_error/log_cmd_error 形)
// ---------------------------------------------------------------------------

pub struct ProcError(pub String);

fn err(msg: impl Into<String>) -> ProcError {
    ProcError(msg.into())
}

// ---------------------------------------------------------------------------
// Module builder(NEW_ID/helper 面;simplemap.rs 的 Mb 泛化)
// ---------------------------------------------------------------------------

pub struct Pm<'a> {
    pub module: &'a mut Module,
    pub autoidx: &'a mut i64,
}

impl<'a> Pm<'a> {
    pub fn module(&mut self) -> &mut Module {
        self.module
    }

    /// NEW_ID:`$auto$<file>:<line>:<func>$<autoidx++>`(yosys_common.h:303)。
    pub fn new_id(&mut self, file: &str, line: u32, func: &str) -> String {
        let n = *self.autoidx;
        *self.autoidx += 1;
        format!("$auto${file}:{line}:{func}${n}")
    }

    /// addWire(name, width):dict 尾插。
    pub fn add_wire(&mut self, name: String, width: i64) -> SigSpec {
        self.module().wires.push(Wire { name: name.clone(), width, ..Default::default() });
        SigSpec {
            bits: (0..width).map(|o| SigBit::Wire { wire: name.clone(), offset: o }).collect(),
        }
    }

    /// addCell + 全套參數/端口(連接=插入序)+ \src 屬性(src=None=不設,
    /// set_src_attribute 語義)。
    #[allow(clippy::too_many_arguments)]
    pub fn add_cell(
        &mut self,
        name: String,
        ctype: &str,
        params: Vec<(&str, Const)>,
        ports: Vec<(&str, SigSpec)>,
        src: Option<&Const>,
    ) -> usize {
        let mut attributes = InsertMap::new();
        if let Some(s) = src {
            attributes.insert("\\src".to_string(), s.clone());
        }
        self.module().cells.push(ry_ir::Cell {
            ctype: ctype.to_string(),
            name,
            attributes,
            parameters: {
                let mut m = InsertMap::new();
                for (k, v) in params {
                    m.insert(k.to_string(), v);
                }
                m
            },
            connections: ports.into_iter().map(|(p, s)| (p.to_string(), s)).collect(),
        });
        self.module().cells.len() - 1
    }

    /// Module::connect(rtlil.cc:3064):lhs 常數位過濾(空=不加)。
    pub fn connect(&mut self, lhs: SigSpec, rhs: SigSpec) {
        let module = self.module();
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

    // -- DEF_METHOD helper 組(rtlil.cc:3199-3350;wire 名錨=DEF_METHOD 展開
    //    行即 rtlil.cc 定義行,cell 名=呼叫方 NEW_ID;行號=二進制實錄)--

    fn add_1in(
        &mut self,
        name: String,
        ctype: &str,
        wire_line: u32,
        wire_func: &str,
        ysize: i64,
        sig_a: &SigSpec,
        src: Option<&Const>,
    ) -> SigSpec {
        let sig_y = {
            let wid = self.new_id("rtlil.cc", wire_line, wire_func);
            self.add_wire(wid, ysize)
        };
        self.add_cell(
            name,
            ctype,
            vec![
                ("\\A_SIGNED", Const::from_i64_i32_width(0)),
                ("\\A_WIDTH", Const::from_i64_i32_width(sig_a.size())),
                ("\\Y_WIDTH", Const::from_i64_i32_width(sig_y.size())),
            ],
            vec![("\\A", sig_a.clone()), ("\\Y", sig_y.clone())],
            src,
        );
        sig_y
    }

    fn add_2in(
        &mut self,
        name: String,
        ctype: &str,
        wire_line: u32,
        wire_func: &str,
        ysize: i64,
        sig_a: &SigSpec,
        sig_b: &SigSpec,
        b_signed_same: bool,
        src: Option<&Const>,
    ) -> SigSpec {
        let sig_y = {
            let wid = self.new_id("rtlil.cc", wire_line, wire_func);
            self.add_wire(wid, ysize)
        };
        let b_signed = if b_signed_same { Const::from_i64_i32_width(0) } else { Const::from_i64_i32_width(0) };
        let _ = b_signed;
        self.add_cell(
            name,
            ctype,
            vec![
                ("\\A_SIGNED", Const::from_i64_i32_width(0)),
                ("\\B_SIGNED", b_signed),
                ("\\A_WIDTH", Const::from_i64_i32_width(sig_a.size())),
                ("\\B_WIDTH", Const::from_i64_i32_width(sig_b.size())),
                ("\\Y_WIDTH", Const::from_i64_i32_width(sig_y.size())),
            ],
            vec![
                ("\\A", sig_a.clone()),
                ("\\B", sig_b.clone()),
                ("\\Y", sig_y.clone()),
            ],
            src,
        );
        sig_y
    }


    // -- 呼叫方行號參數化形(opt_expr/proc_dlatch 的 cell 名 NEW_ID 錨=
    //    呼叫行 opt_expr.cc:NNN / proc_dlatch.cc:NNN;wire 錨=rtlil.cc)--

    pub fn helper_not_at(&mut self, cell_id: String, wire_line: u32, sig_a: &SigSpec, src: Option<&Const>) -> SigSpec {
        self.add_1in(cell_id, "$not", wire_line, "Not", sig_a.size(), sig_a, src)
    }
    pub fn helper_and_at(&mut self, cell_id: String, wire_line: u32, a: &SigSpec, b: &SigSpec, src: Option<&Const>) -> SigSpec {
        self.add_2in(cell_id, "$and", wire_line, "And", a.size().max(b.size()), a, b, true, src)
    }
    pub fn helper_or_at(&mut self, cell_id: String, wire_line: u32, a: &SigSpec, b: &SigSpec, src: Option<&Const>) -> SigSpec {
        self.add_2in(cell_id, "$or", wire_line, "Or", a.size().max(b.size()), a, b, true, src)
    }
    pub fn helper_xor_at(&mut self, cell_id: String, wire_line: u32, a: &SigSpec, b: &SigSpec, src: Option<&Const>) -> SigSpec {
        self.add_2in(cell_id, "$xor", wire_line, "Xor", a.size().max(b.size()), a, b, true, src)
    }
    pub fn helper_reduce_or_at(&mut self, cell_id: String, wire_line: u32, a: &SigSpec, src: Option<&Const>) -> SigSpec {
        self.add_1in(cell_id, "$reduce_or", wire_line, "ReduceOr", 1, a, src)
    }
    pub fn helper_eq_at(&mut self, cell_id: String, wire_line: u32, a: &SigSpec, b: &SigSpec, src: Option<&Const>) -> SigSpec {
        self.add_2in(cell_id, "$eq", wire_line, "Eq", 1, a, b, true, src)
    }
    pub fn helper_mux_at(&mut self, cell_id: String, wire_line: u32, a: &SigSpec, b: &SigSpec, s: &SigSpec, src: Option<&Const>) -> SigSpec {
        let sig_y = {
            let wid = self.new_id("rtlil.cc", wire_line, "Mux");
            self.add_wire(wid, a.size())
        };
        self.add_cell(
            cell_id,
            "$mux",
            vec![("\\WIDTH", Const::from_i64_i32_width(a.size()))],
            vec![("\\A", a.clone()), ("\\B", b.clone()), ("\\S", s.clone()), ("\\Y", sig_y.clone())],
            src,
        );
        sig_y
    }
    pub fn add_wire_id(&mut self, id: String, width: i64) -> SigSpec {
        self.add_wire(id, width)
    }
    /// NEW_ID(任意檔案錨;opt_expr 的 cell 名 `$auto$opt_expr.cc:<L>:
    /// replace_const_cells$N` 用)。
    pub fn new_id_at(&mut self, file: &str, line: u32, func: &str) -> String {
        self.new_id(file, line, func)
    }








}

// ---------------------------------------------------------------------------
// log_debug 抑制計數(register.cc:129 post_execute 的 log_suppressed 面:
// debug 關閉時 log_debug 計數、pass 末 `<suppressed ~N debug messages>` 印+清)
// ---------------------------------------------------------------------------

thread_local! {
    static DEBUG_COUNT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

pub fn dbg_inc() {
    dbg_inc_what("");
}

pub fn dbg_inc_what(what: &str) {
    if std::env::var("RY_DBG").is_ok() {
        eprintln!("DBG: {what}");
    }
    DEBUG_COUNT.with(|c| c.set(c.get() + 1));
}

pub fn dbg_take() -> u32 {
    DEBUG_COUNT.with(|c| c.replace(0))
}

// ---------------------------------------------------------------------------
// process 樹遍歷共用
// ---------------------------------------------------------------------------

/// get_bool_attribute(存在且值非零位)。
fn get_bool_attr(attrs: &InsertMap<Const>, key: &str) -> bool {
    attrs.get(key).map(const_as_bool).unwrap_or(false)
}

/// set_bool_attribute(operator[] 賦值=Const(1))。
fn set_bool_attr(attrs: &mut InsertMap<Const>, key: &str) {
    attrs.set(key.to_string(), Const::from_i64_i32_width(1));
}

/// IdString 形 `%s.%s`(原始形,帶 `\` 前綴)。
fn pair_raw(mod_name: &str, proc_name: &str) -> String {
    format!("{mod_name}.{proc_name}")
}

/// 指標形 %s(unescape)的 mod/proc 兩段(rtlil.h:280 精確規則)。
fn unescape_pair(mod_name: &str, proc_name: &str) -> (String, String) {
    (
        id_unescape(mod_name),
        id_unescape(proc_name),
    )
}

// ===========================================================================
// proc_clean(passes/proc/proc_clean.cc)
// ===========================================================================

fn case_empty(cs: &CaseRule) -> bool {
    cs.actions.is_empty() && cs.switches.is_empty()
}

fn proc_clean_switch(sw: &mut SwitchRule, parent: &mut CaseRule, parent_first: bool, did: &mut bool, count: &mut i32, max_depth: i32) {
    if !sw.signal.bits.is_empty() && is_fully_def(&sw.signal) {
        let mut found_matching: i64 = -1;
        let mut ci = 0usize;
        while ci < sw.cases.len() && found_matching < 0 {
            {
                let cs = &mut sw.cases[ci];
                if cs.compare.is_empty() {
                    break;
                }
                let mut j = 0usize;
                while j < cs.compare.len() {
                    if is_fully_def(&cs.compare[j]) {
                        if cs.compare[j] == sw.signal {
                            cs.compare.clear();
                            found_matching = ci as i64;
                            break;
                        } else {
                            cs.compare.remove(j);
                            continue;
                        }
                    }
                    j += 1;
                }
            }
            if found_matching < 0 && sw.cases[ci].compare.is_empty() {
                sw.cases.remove(ci);
                continue;
            }
            ci += 1;
        }
        while found_matching >= 0 && sw.cases.len() as i64 > found_matching + 1 {
            sw.cases.pop();
        }
        if found_matching == 0 {
            sw.signal = SigSpec::default();
        }
    }

    if parent_first && sw.cases.len() == 1 && (sw.signal.bits.is_empty() || sw.cases[0].compare.is_empty()) {
        *did = true;
        for a in std::mem::take(&mut sw.cases[0].actions) {
            parent.actions.push(a);
        }
        let moved = std::mem::take(&mut sw.cases[0].switches);
        // oracle proc_clean.cc:72 = range-insert-at-begin,保相對序
        // (逐元素 insert(0) 會翻轉 [read, WR]→[WR, read],mem_arst 實錄)。
        parent.switches.splice(0..0, moved);
        sw.cases.clear();
    } else {
        for cs in sw.cases.iter_mut() {
            if max_depth != 0 {
                proc_clean_case(cs, did, count, max_depth - 1);
            }
        }

        let is_parallel = get_bool_attr(&sw.attributes, "\\parallel_case");
        let is_full = get_bool_attr(&sw.attributes, "\\full_case");

        if is_full {
            if sw.cases.iter().all(case_empty) {
                *did = true;
                sw.cases.clear();
            }
        } else if is_parallel {
            let mut ci = 0usize;
            while ci < sw.cases.len() {
                if case_empty(&sw.cases[ci]) {
                    *did = true;
                    sw.cases.remove(ci);
                } else {
                    ci += 1;
                }
            }
        } else {
            while !sw.cases.is_empty() && case_empty(sw.cases.last().unwrap()) {
                *did = true;
                sw.cases.pop();
            }
        }
    }
}

fn proc_clean_case(cs: &mut CaseRule, did: &mut bool, count: &mut i32, max_depth: i32) {
    let mut ai = 0usize;
    while ai < cs.actions.len() {
        if cs.actions[ai].0.bits.is_empty() {
            *did = true;
            cs.actions.remove(ai);
        } else {
            ai += 1;
        }
    }
    let mut si = 0usize;
    while si < cs.switches.len() {
        if cs.switches[si].cases.is_empty() {
            cs.switches.remove(si);
            *did = true;
            *count += 1;
        } else if max_depth != 0 {
            let is_first = si == 0;
            let mut sw = cs.switches.remove(si);
            proc_clean_switch(&mut sw, cs, is_first, did, count, max_depth - 1);
            cs.switches.insert(si, sw);
            si += 1;
        } else {
            si += 1;
        }
    }
}

/// proc_clean(mod, proc)(proc_clean.cc:157)。
fn proc_clean_proc(mod_name: &str, proc: &mut Process, total_count: &mut i32, quiet: bool, out: &mut String) {
    let mut count: i32 = 0;
    let mut did_something = true;
    let mut si = 0usize;
    while si < proc.syncs.len() {
        let mut ai = 0usize;
        while ai < proc.syncs[si].actions.len() {
            if proc.syncs[si].actions[ai].0.bits.is_empty() {
                proc.syncs[si].actions.remove(ai);
            } else {
                ai += 1;
            }
        }
        if proc.syncs[si].actions.is_empty() && proc.syncs[si].mem_writes.is_empty() {
            proc.syncs.remove(si);
        } else {
            si += 1;
        }
    }
    while did_something {
        did_something = false;
        proc_clean_case(&mut proc.root_case, &mut did_something, &mut count, -1);
    }
    if count > 0 && !quiet {
        out.push_str(&format!(
            "Found and cleaned up {count} empty switch{} in `{}'.\n",
            if count == 1 { "" } else { "es" },
            pair_raw(mod_name, &proc.name)
        ));
    }
    *total_count += count;
}

/// `proc_clean` 命令面(無參形+`-quiet` 契約旗標;其餘旗標=Y2)。
pub fn run_proc_clean(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), ProcError> {
    let mut quiet = false;
    for a in args.get(1..).unwrap_or(&[]) {
        if a == "-quiet" {
            quiet = true;
        } else if a.starts_with('-') {
            return Err(err(format!(
                "proc_clean: option `{a}' is not implemented in ryosys (milestone Y2)"
            )));
        } else {
            return Err(err("selection is not implemented in ryosys (milestone Y0)"));
        }
    }
    if !quiet {
        log.header("Executing PROC_CLEAN pass (remove empty switches from decision trees).\n");
    }
    let mut out = std::mem::take(&mut log.out);
    let mut total_count: i32 = 0;
    for mi in (0..design.modules.len()).rev() {
        let mut delme: Vec<String> = Vec::new();
        for pi in (0..design.modules[mi].processes.len()).rev() {
            let mod_name = design.modules[mi].name.clone();
            let proc = &mut design.modules[mi].processes[pi];
            proc_clean_proc(&mod_name, proc, &mut total_count, quiet, &mut out);
            if proc.syncs.is_empty() && proc.root_case.switches.is_empty() && proc.root_case.actions.is_empty() {
                if !quiet {
                    let (m, p) = unescape_pair(&mod_name, &proc.name);
                    out.push_str(&format!("Removing empty process `{m}.{p}'.\n"));
                }
                delme.push(proc.name.clone());
            }
        }
        for name in delme {
            let v = &mut design.modules[mi].processes;
            if let Some(i) = v.iter().position(|p| p.name == name) {
                let n = v.len();
                v.swap(i, n - 1);
                v.pop();
            }
        }
    }
    log.out = out;
    if !quiet {
        log.log(&format!(
            "Cleaned up {total_count} empty switch{}.\n",
            if total_count == 1 { "" } else { "es" }
        ));
    }
    log.resync();
    Ok(())
}

// ===========================================================================
// proc_rmdead(passes/proc/proc_rmdead.cc;BitPatternPool=kernel/bitpattern.h)
// ===========================================================================

/// kernel/bitpattern.h BitPatternPool(database 內部序=hash 序,不可觀測:
/// take/has_all 皆純謂詞,僅影響 cases erase 決策)。
struct BitPatternPool {
    width: usize,
    database: Vec<Vec<Bit>>, // bits_t:每 entry 寬=width;Sa=don't care
}

fn sig2bits(sig: &SigSpec) -> Option<Vec<Bit>> {
    let mut out = Vec::with_capacity(sig.bits.len());
    for b in &sig.bits {
        match b {
            SigBit::Const(x) => out.push(if matches!(x, Bit::S0 | Bit::S1) { *x } else { Bit::Sa }),
            SigBit::Wire { .. } => return None, // 呼叫端保證 fully const
        }
    }
    Some(out)
}

fn covers_nothing(sig: &SigSpec) -> bool {
    sig.bits.iter().any(|b| matches!(b, SigBit::Const(Bit::Sx) | SigBit::Const(Bit::Sz)))
}

impl BitPatternPool {
    fn new_width(width: i64) -> BitPatternPool {
        let mut database = Vec::new();
        if width > 0 {
            database.push(vec![Bit::Sa; width as usize]);
        }
        BitPatternPool { width: width as usize, database }
    }

    /// BitPatternPool(RTLIL::SigSpec)(bitpattern.h:40):常數位取值、
    /// wire 位 Sa。
    fn from_signal(sig: &SigSpec) -> BitPatternPool {
        let width = sig.bits.len();
        let mut database = Vec::new();
        if width > 0 {
            let mut pattern = vec![Bit::Sa; width];
            for (i, b) in sig.bits.iter().enumerate() {
                if let SigBit::Const(x) = b {
                    if matches!(x, Bit::S0 | Bit::S1) {
                        pattern[i] = *x;
                    }
                }
            }
            database.push(pattern);
        }
        BitPatternPool { width, database }
    }

    fn cube_match(a: &[Bit], b: &[Bit]) -> bool {
        for i in 0..a.len() {
            if matches!(a[i], Bit::S0 | Bit::S1) && matches!(b[i], Bit::S0 | Bit::S1) && a[i] != b[i] {
                return false;
            }
        }
        true
    }

    fn has_all(&self, sig: &SigSpec) -> bool {
        if covers_nothing(sig) {
            return true;
        }
        let Some(bits) = sig2bits(sig) else { return false };
        for it in &self.database {
            if Self::cube_match(it, &bits) {
                let mut ok = true;
                for i in 0..self.width {
                    if !matches!(bits[i], Bit::S0 | Bit::S1) && matches!(it[i], Bit::S0 | Bit::S1) {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    return true;
                }
            }
        }
        false
    }

    fn take(&mut self, sig: &SigSpec) -> bool {
        if covers_nothing(sig) {
            return false;
        }
        let Some(bits) = sig2bits(sig) else { return false };
        let mut status = false;
        // hashlib pool 精確模擬:begin()=尾(尾到頭迭代);erase=swap-and-pop
        // (被刪位收到原尾元素,iterator 不前進=重查同位);news 尾插。
        let mut i = self.database.len();
        while i > 0 {
            i -= 1;
            if i >= self.database.len() {
                continue;
            }
            if Self::cube_match(&self.database[i], &bits) {
                let cube = self.database[i].clone();
                let mut news: Vec<Vec<Bit>> = Vec::new();
                for k in 0..self.width {
                    if cube[k] != Bit::Sa || bits[k] == Bit::Sa {
                        continue;
                    }
                    let mut np = cube.clone();
                    np[k] = if bits[k] == Bit::S1 { Bit::S0 } else { Bit::S1 };
                    news.push(np);
                }
                let last = self.database.len() - 1;
                self.database.swap(i, last);
                self.database.pop();
                for np in news {
                    self.database.push(np);
                }
                status = true;
            }
        }
        status
    }

    fn take_all(&mut self) {
        self.database.clear();
    }

    fn is_empty(&self) -> bool {
        self.database.is_empty()
    }
}

/// FullyDefinedPool(proc_rmdead.cc:51)。
struct FullyDefinedPool {
    patterns: HashSet<SigSpec>,
    default_reached: bool,
    max_patterns: usize,
}

impl FullyDefinedPool {
    fn new(signal: &SigSpec) -> FullyDefinedPool {
        FullyDefinedPool {
            patterns: HashSet::new(),
            default_reached: false,
            max_patterns: if signal.size() >= 32 { 0 } else { 1usize << signal.size() },
        }
    }
    fn take(&mut self, sig: &SigSpec) -> bool {
        if self.default_reached || self.patterns.contains(sig) {
            return false;
        }
        self.patterns.insert(sig.clone());
        true
    }
    fn take_all(&mut self) {
        self.default_reached = true;
    }
    fn is_empty(&self) -> bool {
        self.default_reached || (self.max_patterns != 0 && self.max_patterns == self.patterns.len())
    }
}

fn can_use_fully_defined_pool(sw: &SwitchRule) -> bool {
    if sw.signal.bits.is_empty() {
        return false;
    }
    for b in &sw.signal.bits {
        if matches!(b, SigBit::Const(_)) {
            return false;
        }
    }
    for cs in &sw.cases {
        for sig in &cs.compare {
            if !is_fully_def(sig) {
                return false;
            }
        }
    }
    true
}

enum Pool {
    Bits(BitPatternPool),
    Fully(FullyDefinedPool),
}

impl Pool {
    fn is_empty(&self) -> bool {
        match self {
            Pool::Bits(p) => p.is_empty(),
            Pool::Fully(p) => p.is_empty(),
        }
    }
}

fn pool_take(pool: &mut Pool, sig: &SigSpec) -> bool {
    match pool {
        Pool::Bits(p) => p.take(sig),
        Pool::Fully(p) => p.take(sig),
    }
}

fn proc_rmdead_impl(uses_fully: bool, sw: &mut SwitchRule, counter: &mut i32, full_case_counter: &mut i32) {
    let mut pool = if uses_fully {
        Pool::Fully(FullyDefinedPool::new(&sw.signal))
    } else {
        // oracle:BitPatternPool(sw->signal)(常數位具體化);僅 proc_mux 的
        // pgroups 用 new_width(全 Sa)。
        if sw.signal.bits.is_empty() {
            Pool::Bits(BitPatternPool::new_width(0))
        } else {
            Pool::Bits(BitPatternPool::from_signal(&sw.signal))
        }
    };

    let mut i = 0usize;
    while i < sw.cases.len() {
        let is_default = sw.cases[i].compare.is_empty() && (!pool.is_empty() || sw.signal.size() == 0);

        let mut j = 0usize;
        while j < sw.cases[i].compare.len() {
            if is_fully_const(&sw.cases[i].compare[j]) {
                if !pool_take(&mut pool, &sw.cases[i].compare[j]) {
                    sw.cases[i].compare.remove(j);
                    continue;
                }
            }
            j += 1;
        }

        if !is_default && sw.cases[i].compare.is_empty() {
            sw.cases.remove(i);
            *counter += 1;
            continue;
        }

        for sw2 in sw.cases[i].switches.iter_mut() {
            proc_rmdead(sw2, counter, full_case_counter);
        }

        if is_default {
            match &mut pool {
                Pool::Fully(p) => p.take_all(),
                Pool::Bits(p) => p.take_all(),
            }
        }
        i += 1;
    }

    if pool.is_empty() && !get_bool_attr(&sw.attributes, "\\full_case") {
        set_bool_attr(&mut sw.attributes, "\\full_case");
        *full_case_counter += 1;
    }
}

fn proc_rmdead(sw: &mut SwitchRule, counter: &mut i32, full_case_counter: &mut i32) {
    let uses_fully = can_use_fully_defined_pool(sw);
    proc_rmdead_impl(uses_fully, sw, counter, full_case_counter);
}

/// `proc_rmdead` 命令面(無參形)。
pub fn run_proc_rmdead(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), ProcError> {
    check_no_args("proc_rmdead", args)?;
    log.header("Executing PROC_RMDEAD pass (remove dead branches from decision trees).\n");
    let mut out = std::mem::take(&mut log.out);
    let mut total_counter: i32 = 0;
    for mi in (0..design.modules.len()).rev() {
        for pi in (0..design.modules[mi].processes.len()).rev() {
            let mut counter: i32 = 0;
            let mut full_case_counter: i32 = 0;
            for sw in design.modules[mi].processes[pi].root_case.switches.iter_mut() {
                proc_rmdead(sw, &mut counter, &mut full_case_counter);
            }
            if counter > 0 {
                let (m, p) = unescape_pair(&design.modules[mi].name, &design.modules[mi].processes[pi].name);
                out.push_str(&format!("Removed {counter} dead cases from process {p} in module {m}.\n"));
            }
            if full_case_counter > 0 {
                let (m, p) = unescape_pair(&design.modules[mi].name, &design.modules[mi].processes[pi].name);
                out.push_str(&format!(
                    "Marked {full_case_counter} switch rules as full_case in process {p} in module {m}.\n"
                ));
            }
            total_counter += counter;
        }
    }
    log.out = out;
    log.log(&format!("Removed a total of {total_counter} dead cases.\n"));
    log.resync();
    Ok(())
}

// ===========================================================================
// proc_prune(passes/proc/proc_prune.cc)
// ===========================================================================

struct PruneWorker {
    sigmap: crate::sigmap::SigMap,
    removed_count: i32,
    promoted_count: i32,
    pending: Vec<(SigSpec, SigSpec)>,
}

impl PruneWorker {
    fn new(module: &Module) -> PruneWorker {
        let mut sigmap = crate::sigmap::SigMap::new();
        sigmap.set(module);
        PruneWorker { sigmap, removed_count: 0, promoted_count: 0, pending: Vec::new() }
    }

    fn do_switch(&mut self, sw: &mut SwitchRule, assigned: HashSet<SigBit>, affected: &mut HashSet<SigBit>) -> HashSet<SigBit> {
        let mut all_assigned: HashSet<SigBit> = HashSet::new();
        let mut full_case = get_bool_attr(&sw.attributes, "\\full_case");
        let mut first = true;
        let mut assigned = assigned;
        for cs in sw.cases.iter_mut() {
            if cs.compare.is_empty() {
                full_case = true;
            }
            let case_assigned = self.do_case(cs, assigned.clone(), affected, false);
            if first {
                first = false;
                all_assigned = case_assigned;
            } else {
                let keep: Vec<SigBit> =
                    all_assigned.iter().filter(|b| case_assigned.contains(*b)).cloned().collect();
                all_assigned = keep.into_iter().collect();
            }
        }
        if full_case {
            for b in all_assigned {
                assigned.insert(b);
            }
        }
        assigned
    }

    fn do_case(&mut self, cs: &mut CaseRule, assigned: HashSet<SigBit>, affected: &mut HashSet<SigBit>, root: bool) -> HashSet<SigBit> {
        let mut assigned = assigned;
        for si in (0..cs.switches.len()).rev() {
            let sw_assigned = {
                let sw = &mut cs.switches[si];
                self.do_switch(sw, assigned.clone(), affected)
            };
            for b in sw_assigned {
                assigned.insert(b);
            }
        }
        let mut ai = cs.actions.len();
        while ai > 0 {
            ai -= 1;
            let lhs = self.sigmap.map(&cs.actions[ai].0);
            let rhs = self.sigmap.map(&cs.actions[ai].1);
            let mut new_lhs = SigSpec::default();
            let mut new_rhs = SigSpec::default();
            let mut conn_lhs = SigSpec::default();
            let mut conn_rhs = SigSpec::default();
            for i in 0..lhs.bits.len() {
                let bit = lhs.bits[i].clone();
                if matches!(bit, SigBit::Wire { .. }) && !assigned.contains(&bit) {
                    if !affected.contains(&bit) && root {
                        conn_lhs.bits.push(bit.clone());
                        conn_rhs.bits.push(rhs.bits[i].clone());
                    } else {
                        new_lhs.bits.push(bit.clone());
                        new_rhs.bits.push(rhs.bits[i].clone());
                    }
                    assigned.insert(bit);
                    affected.insert(lhs.bits[i].clone());
                }
            }
            let conn_empty = conn_lhs.bits.is_empty();
            if !conn_empty {
                self.promoted_count += 1;
                self.pending.push((conn_lhs, conn_rhs));
            }
            if new_lhs.bits.is_empty() {
                if conn_empty {
                    self.removed_count += 1;
                }
                cs.actions.remove(ai);
            } else {
                cs.actions[ai].0 = new_lhs;
                cs.actions[ai].1 = new_rhs;
            }
        }
        assigned
    }

    fn do_process(&mut self, pr: &mut Process) {
        let mut affected: HashSet<SigBit> = HashSet::new();
        self.do_case(&mut pr.root_case, HashSet::new(), &mut affected, true);
    }
}

/// `proc_prune` 命令面(無參形)。
pub fn run_proc_prune(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), ProcError> {
    check_no_args("proc_prune", args)?;
    log.header("Executing PROC_PRUNE pass (remove redundant assignments in processes).\n");
    let out = std::mem::take(&mut log.out);
    let mut total_removed: i32 = 0;
    let mut total_promoted: i32 = 0;
    for mi in (0..design.modules.len()).rev() {
        let mut worker = PruneWorker::new(&design.modules[mi]);
        for pi in (0..design.modules[mi].processes.len()).rev() {
            worker.do_process(&mut design.modules[mi].processes[pi]);
        }
        let pend = std::mem::take(&mut worker.pending);
        if !pend.is_empty() {
            let module = &mut design.modules[mi];
            let autoidx = &mut design.autoidx;
            let mut pm = Pm { module, autoidx };
            for (l, r) in pend {
                pm.connect(l, r);
            }
        }
        total_removed += worker.removed_count;
        total_promoted += worker.promoted_count;
    }
    log.out = out;
    log.log(&format!(
        "Removed {total_removed} redundant assignment{}.\n",
        if total_removed == 1 { "" } else { "s" }
    ));
    log.log(&format!(
        "Promoted {total_promoted} assignment{} to connection{}.\n",
        if total_promoted == 1 { "" } else { "s" },
        if total_promoted == 1 { "" } else { "s" }
    ));
    log.resync();
    Ok(())
}

// ===========================================================================
// proc_init(passes/proc/proc_init.cc)
// ===========================================================================

fn proc_init_proc(module: &mut Module, pi: usize, widths: &std::collections::HashMap<String, i64>, out: &mut String) -> Result<(), ProcError> {
    let syncs = module.processes[pi].syncs.clone();
    for (si, sync) in syncs.iter().enumerate() {
        if sync.stype != SyncType::Init {
            continue;
        }
        out.push_str(&format!(
            "Found init rule in `{}'.\n",
            pair_raw(&module.name, &module.processes[pi].name)
        ));
        let actions = sync.actions.clone();
        for (lhs, rhs_raw) in &actions {
            let mut sm = crate::sigmap::SigMap::new();
            sm.set(module);
            let rhs = sm.map(rhs_raw);
            if !is_fully_const(&rhs) {
                return Err(err(format!(
                    "Failed to get a constant init value for {}: {}",
                    log_signal(lhs, widths),
                    log_signal(&rhs, widths)
                )));
            }
            let mut offset: i64 = 0;
            for chunk in ry_ir::printer::sig_chunks(lhs) {
                if let ry_ir::printer::ChunkView::Wire { wire, offset: coff, width } = &chunk {
                    let valuesig = rhs.extract(offset, *width);
                    if !is_fully_const(&valuesig) {
                        return Err(err(format!(
                            "Non-const initialization value: {} = {}",
                            chunk_log_signal(wire, *width, *coff, widths),
                            log_signal(&valuesig, widths)
                        )));
                    }
                    let value_bits: Vec<Bit> = valuesig
                        .bits
                        .iter()
                        .map(|b| match b {
                            SigBit::Const(x) => *x,
                            _ => Bit::Sx,
                        })
                        .collect();
                    let widx = module.wires.iter().position(|w| &w.name == wire).unwrap();
                    let wwidth = module.wires[widx].width;
                    let mut init = module.wires[widx].attributes.get("\\init").cloned().unwrap_or_default();
                    let mut ibits: Vec<Bit> = match &init {
                        Const::Bits { bits, .. } => bits.clone(),
                        Const::Str(_) => Vec::new(),
                    };
                    if (ibits.len() as i64) < wwidth {
                        ibits.resize(wwidth.max(0) as usize, Bit::Sx);
                    }
                    for i in 0..*width {
                        let index = (i + coff) as usize;
                        let initbit = ibits[index];
                        let valuebit = value_bits[i as usize];
                        if initbit != Bit::Sx && initbit != valuebit {
                            return Err(err(format!(
                                "Conflicting initialization values for {}.",
                                chunk_log_signal(wire, *width, *coff, widths)
                            )));
                        }
                        ibits[index] = valuebit;
                    }
                    init = Const::Bits {
                        width: ibits.len() as i64,
                        bits: ibits,
                        signed: false,
                        is_unsized: false,
                    };
                    if let Const::Bits { bits, .. } = &init {
                        let cs = SigSpec { bits: bits.iter().map(|&b| SigBit::Const(b)).collect() };
                        out.push_str(&format!(
                            "  Set init value: {} = {}\n",
                            wire_log_signal(wire, widths),
                            log_signal(&cs, widths)
                        ));
                    }
                    module.wires[widx].attributes.set("\\init".to_string(), init);
                }
                offset += chunk_width(&chunk);
            }
        }
        module.processes[pi].syncs[si].actions.clear();
    }
    Ok(())
}

fn chunk_width(c: &ry_ir::printer::ChunkView) -> i64 {
    match c {
        ry_ir::printer::ChunkView::Wire { width, .. } => *width,
        ry_ir::printer::ChunkView::Const { len, .. } => *len as i64,
    }
}

/// log_signal(單 chunk wire 形:`name`/`name [off]`/`name [hi:lo]`;
/// dump_sigchunk 的裸名判定=寬==宣告寬且 offset 0)。
fn chunk_log_signal(wire: &str, width: i64, offset: i64, widths: &std::collections::HashMap<String, i64>) -> String {
    let decl = widths.get(wire).copied().unwrap_or(width);
    if width == decl && offset == 0 {
        wire.to_string()
    } else if width == 1 {
        format!("{wire} [{offset}]")
    } else {
        format!("{wire} [{}:{}]", offset + width - 1, offset)
    }
}

/// log_signal(整條 wire)=裸名(SigSpec(Wire*)=全寬 chunk)。
fn wire_log_signal(wire: &str, _widths: &std::collections::HashMap<String, i64>) -> String {
    wire.to_string()
}

/// `proc_init` 命令面(無參形)。
pub fn run_proc_init(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), ProcError> {
    check_no_args("proc_init", args)?;
    log.header("Executing PROC_INIT pass (extract init attributes).\n");
    let mut out = std::mem::take(&mut log.out);
    let result = (|| -> Result<(), ProcError> {
        for mi in (0..design.modules.len()).rev() {
            let widths: std::collections::HashMap<String, i64> =
                design.modules[mi].wires.iter().map(|w| (w.name.clone(), w.width)).collect();
            for pi in (0..design.modules[mi].processes.len()).rev() {
                proc_init_proc(&mut design.modules[mi], pi, &widths, &mut out)?;
            }
        }
        Ok(())
    })();
    log.out = out;
    log.resync();
    result
}

// ---------------------------------------------------------------------------
// 共用:無參契約(多餘旗標=Y2、selection=Y0;鐵則 3)
// ---------------------------------------------------------------------------

fn check_no_args(cmd: &str, args: &[String]) -> Result<(), ProcError> {
    for a in args.get(1..).unwrap_or(&[]) {
        if a.starts_with('-') {
            return Err(ProcError(format!(
                "{cmd}: option `{a}' is not implemented in ryosys (milestone Y2)"
            )));
        }
        return Err(ProcError("selection is not implemented in ryosys (milestone Y0)".to_string()));
    }
    Ok(())
}

// ===========================================================================
// proc_arst(passes/proc/proc_arst.cc)
// ===========================================================================

/// check_signal(proc_arst.cc:33;cells 迭代序=逆插入序)。
fn check_signal(module: &Module, signal: &SigSpec, ref_sig: &SigSpec, polarity: &mut bool) -> bool {
    if signal.size() != 1 {
        return false;
    }
    if signal == ref_sig {
        return true;
    }
    for cell in module.cells.iter().rev() {
        let ctype = cell.ctype.as_str();
        let y = get_port_c(cell, "\\Y");
        if y.map(|s| s == signal).unwrap_or(false) {
            if ctype == "$reduce_or" || ctype == "$reduce_bool" {
                return check_signal(module, &get_port_c(cell, "\\A").cloned().unwrap_or_default(), ref_sig, polarity);
            }
            if ctype == "$logic_not" || ctype == "$not" {
                *polarity = !*polarity;
                return check_signal(module, &get_port_c(cell, "\\A").cloned().unwrap_or_default(), ref_sig, polarity);
            }
            if matches!(ctype, "$eq" | "$eqx") {
                let a = get_port_c(cell, "\\A").cloned().unwrap_or_default();
                let b = get_port_c(cell, "\\B").cloned().unwrap_or_default();
                if is_fully_const(&a) {
                    if !sig_const_as_bool(&a) {
                        *polarity = !*polarity;
                    }
                    return check_signal(module, &b, ref_sig, polarity);
                }
                if is_fully_const(&b) {
                    if !sig_const_as_bool(&b) {
                        *polarity = !*polarity;
                    }
                    return check_signal(module, &a, ref_sig, polarity);
                }
            }
            if matches!(ctype, "$ne" | "$nex") {
                let a = get_port_c(cell, "\\A").cloned().unwrap_or_default();
                let b = get_port_c(cell, "\\B").cloned().unwrap_or_default();
                if is_fully_const(&a) {
                    if sig_const_as_bool(&a) {
                        *polarity = !*polarity;
                    }
                    return check_signal(module, &b, ref_sig, polarity);
                }
                if is_fully_const(&b) {
                    if sig_const_as_bool(&b) {
                        *polarity = !*polarity;
                    }
                    return check_signal(module, &a, ref_sig, polarity);
                }
            }
        }
    }
    false
}

fn get_port_c<'a>(cell: &'a ry_ir::Cell, port: &str) -> Option<&'a SigSpec> {
    cell.connections.iter().find(|(p, _)| p == port).map(|(_, s)| s)
}

/// SigSpec::as_bool(全常數 1 位語義;Const::as_bool=位 0)。
fn sig_const_as_bool(sig: &SigSpec) -> bool {
    sig.bits.first() == Some(&SigBit::Const(Bit::S1))
}

/// apply_const(proc_arst.cc:88)。
fn apply_const(
    module: &Module,
    rspec: &mut SigSpec,
    rval: &mut SigSpec,
    cs: &CaseRule,
    const_sig: &SigSpec,
    polarity: bool,
    unknown: bool,
) {
    for action in &cs.actions {
        if unknown {
            let with = sig_const(Bit::Sm, action.1.size());
            replace_into(&action.0, &with, rspec, rval);
        } else {
            replace_into(&action.0, &action.1, rspec, rval);
        }
    }
    for sw in &cs.switches {
        if sw.signal.bits.is_empty() {
            for cs2 in &sw.cases {
                apply_const(module, rspec, rval, cs2, const_sig, polarity, unknown);
            }
        }
        let mut this_polarity = polarity;
        if check_signal(module, &sw.signal, const_sig, &mut this_polarity) {
            'outer: for cs2 in &sw.cases {
                for comp in &cs2.compare {
                    if *comp == one_bit_p(this_polarity) {
                        apply_const(module, rspec, rval, cs2, const_sig, polarity, false);
                        break 'outer;
                    }
                }
                if cs2.compare.is_empty() {
                    apply_const(module, rspec, rval, cs2, const_sig, polarity, false);
                    break 'outer;
                }
            }
        } else {
            for cs2 in &sw.cases {
                apply_const(module, rspec, rval, cs2, const_sig, polarity, true);
            }
        }
    }
}

fn one_bit_p(b: bool) -> SigSpec {
    sig_const(if b { Bit::S1 } else { Bit::S0 }, 1)
}

/// eliminate_const(proc_arst.cc:121)。
fn eliminate_const(module: &Module, cs: &mut CaseRule, const_sig: &SigSpec, polarity: bool) {
    for sw in cs.switches.iter_mut() {
        let mut this_polarity = polarity;
        if check_signal(module, &sw.signal, const_sig, &mut this_polarity) {
            // oracle 語義(proc_arst.cc:127-140):hit→erase(**不置 found**);
            // 無 hit 且已 found→erase;無 hit 且 !found→**保留+clear+置 found**
            //(default case 的 compare.clear 正是單 case 提升的條件)。
            let mut found_rem_path = false;
            let mut i = 0usize;
            while i < sw.cases.len() {
                let hit = sw.cases[i].compare.iter().any(|comp| *comp == one_bit_p(this_polarity));
                if hit || found_rem_path {
                    sw.cases.remove(i);
                    continue;
                }
                found_rem_path = true;
                sw.cases[i].compare.clear();
                i += 1;
            }
            sw.signal = SigSpec::default();
        } else {
            for cs2 in sw.cases.iter_mut() {
                eliminate_const(module, cs2, const_sig, polarity);
            }
        }
    }

    let mut dummy_count: i32 = 0;
    let mut did_something = true;
    while did_something {
        did_something = false;
        proc_clean_case(cs, &mut did_something, &mut dummy_count, 1);
    }
}

/// apply_reset(proc_arst.cc:156)。
#[allow(clippy::too_many_arguments)]
fn apply_reset(
    module: &Module,
    proc: &Process,
    sync_signal: &SigSpec,
    assign_map: &crate::sigmap::SigMap,
    root_sig: &SigSpec,
    polarity: bool,
    sig: &SigSpec,
    log_sig: &SigSpec,
) -> Result<SigSpec, ProcError> {
    let mut rspec = assign_map.map(sig);
    let mut rval = sig_const(Bit::Sm, rspec.size());
    for i in 0..rval.size() as usize {
        if matches!(rspec.bits[i], SigBit::Const(_)) {
            rval.bits[i] = rspec.bits[i].clone();
        }
    }
    let mut count = 0;
    loop {
        let last_rval = rval.clone();
        apply_const(module, &mut rspec, &mut rval, &proc.root_case, root_sig, polarity, false);
        let rv = assign_map.map(&rval);
        rval = rv;
        if is_fully_const(&rval) {
            break;
        }
        if rval == last_rval {
            // rval != last_rval 條件:相等即退出迴圈(oracle for 迴圈條件)。
            break;
        }
        count += 1;
        if count > 100 {
            let widths = std::collections::HashMap::new();
            return Err(err(format!(
                "Async reset {} yields endless loop at value {} for signal {}.",
                log_signal(sync_signal, &widths),
                log_signal(&rval, &widths),
                log_signal(log_sig, &widths)
            )));
        }
        rspec = rval.clone();
    }
    if has_marked_bits(&rval) {
        let widths = std::collections::HashMap::new();
        return Err(err(format!(
            "Async reset {} yields non-constant value {} for signal {}.",
            log_signal(sync_signal, &widths),
            log_signal(&rval, &widths),
            log_signal(log_sig, &widths)
        )));
    }
    Ok(rval)
}

/// proc_arst(proc_arst.cc:180)。
fn proc_arst_proc(
    module: &Module,
    proc: &mut Process,
    assign_map: &crate::sigmap::SigMap,
    out: &mut String,
) -> Result<(), ProcError> {
    let mut arst_syncs: Vec<SyncRule> = Vec::new();
    let mut edge_syncs: Vec<SyncRule> = Vec::new();
    let mut other_syncs: Vec<SyncRule> = Vec::new();
    // 分桶讀(clone);原 syncs 不動,did_something 時才重排(oracle 引用分桶)。
    for sync in proc.syncs.clone() {
        match sync.stype {
            SyncType::Low | SyncType::High => arst_syncs.push(sync),
            SyncType::Posedge | SyncType::Negedge => edge_syncs.push(sync),
            _ => other_syncs.push(sync),
        }
    }

    let mut did_something = false;
    while proc.root_case.switches.len() == 1 {
        let root_sig = proc.root_case.switches[0].signal.clone();
        let mut found = false;
        for ei in 0..edge_syncs.len() {
            let polarity = edge_syncs[ei].stype == SyncType::Posedge;
            let sync_signal = edge_syncs[ei].signal.clone().unwrap_or_default();
            let mut pol = polarity;
            if check_signal(module, &root_sig, &sync_signal, &mut pol) {
                let polarity = pol;
                if edge_syncs.len() > 1 {
                    let widths = wire_widths_pub(module);
                    out.push_str(&format!(
                        "Found async reset {} in `{}'.\n",
                        log_signal(&sync_signal, &widths),
                        pair_raw(&module.name, &proc.name)
                    ));
                    let mut sync = edge_syncs.remove(ei);
                    sync.stype = if sync.stype == SyncType::Posedge { SyncType::High } else { SyncType::Low };
                    let acts = sync.actions.clone();
                    for (i, action) in acts.iter().enumerate() {
                        let nv = apply_reset(module, proc, &sync.signal.clone().unwrap_or_default(), assign_map, &root_sig, polarity, &action.1, &action.0)?;
                        sync.actions[i].1 = nv;
                    }
                    let mw = sync.mem_writes.clone();
                    for (i, m) in mw.iter().enumerate() {
                        let en = apply_reset(module, proc, &sync.signal.clone().unwrap_or_default(), assign_map, &root_sig, polarity, &m.enable, &m.enable)?;
                        if !is_fully_zero(&en) {
                            return Err(err(format!(
                                "Async reset {} causes memory write to {}.",
                                log_signal(&sync.signal.clone().unwrap_or_default(), &wire_widths_pub(module)),
                                m.memid.strip_prefix('\\').unwrap_or(&m.memid)
                            )));
                        }
                        let ad = apply_reset(module, proc, &sync.signal.clone().unwrap_or_default(), assign_map, &root_sig, polarity, &m.address, &m.address)?;
                        sync.mem_writes[i].address = ad;
                        let da = apply_reset(module, proc, &sync.signal.clone().unwrap_or_default(), assign_map, &root_sig, polarity, &m.data, &m.data)?;
                        sync.mem_writes[i].data = da;
                    }
                    sync.mem_writes.clear();
                    arst_syncs.push(sync);
                    let mut root = std::mem::take(&mut proc.root_case);
                    eliminate_const(module, &mut root, &root_sig, polarity);
                    proc.root_case = root;
                } else {
                    let widths = wire_widths_pub(module);
                    out.push_str(&format!(
                        "Found VHDL-style edge-trigger {} in `{}'.\n",
                        log_signal(&sync_signal, &widths),
                        pair_raw(&module.name, &proc.name)
                    ));
                    let mut root = std::mem::take(&mut proc.root_case);
                    eliminate_const(module, &mut root, &root_sig, !polarity);
                    proc.root_case = root;
                }
                did_something = true;
                found = true;
                break;
            }
        }
        if !found {
            break;
        }
    }

    if did_something {
        proc.syncs.clear();
        proc.syncs.extend(arst_syncs);
        proc.syncs.extend(edge_syncs);
        proc.syncs.extend(other_syncs);
    }
    Ok(())
}

fn wire_widths_pub(module: &Module) -> std::collections::HashMap<String, i64> {
    module.wires.iter().map(|w| (w.name.clone(), w.width)).collect()
}

/// `proc_arst` 命令面(無參形;-global_arst=Y2 詳案 §8.4)。
pub fn run_proc_arst(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), ProcError> {
    check_no_args("proc_arst", args)?;
    log.header("Executing PROC_ARST pass (detect async resets in processes).\n");
    let mut out = std::mem::take(&mut log.out);
    for mi in (0..design.modules.len()).rev() {
        let mut assign_map = crate::sigmap::SigMap::new();
        assign_map.set(&design.modules[mi]);
        let module_snapshot = design.modules[mi].clone();
        for pi in (0..design.modules[mi].processes.len()).rev() {
            let module_ref = &mut design.modules[mi];
            proc_arst_proc_with_snapshot(&module_snapshot, module_ref, pi, &assign_map, &mut out)?;
        }
    }
    log.out = out;
    log.resync();
    Ok(())
}

/// proc_arst 的 check_signal 只讀 module(cells 面),process 樹可變——
/// 以快照讀+可變寫分離(check_signal 慣用的 module 只含 cells/wires)。
fn proc_arst_proc_with_snapshot(
    snapshot: &Module,
    target: &mut Module,
    pi: usize,
    assign_map: &crate::sigmap::SigMap,
    out: &mut String,
) -> Result<(), ProcError> {
    proc_arst_proc(snapshot, &mut target.processes[pi], assign_map, out)
}


// ===========================================================================
// proc_rom(passes/proc/proc_rom.cc;Mem::emit 精簡形=kernel/mem.cc:280-405)
// ===========================================================================

fn ceil_log2(x: i64) -> i64 {
    if x <= 1 {
        return 0;
    }
    let mut v = x - 1;
    let mut n: i64 = 0;
    while v > 0 {
        n += 1;
        v >>= 1;
    }
    n
}

fn bit_key(b: &SigBit) -> (String, i64, u8) {
    match b {
        SigBit::Wire { wire, offset } => (wire.clone(), *offset, 1),
        SigBit::Const(x) => (String::new(), 0, match x {
            Bit::S0 => 10,
            Bit::S1 => 11,
            Bit::Sx => 12,
            Bit::Sz => 13,
            Bit::Sm => 14,
            Bit::Sa => 15,
        }),
    }
}

fn bits_of_const_sig(sig: &SigSpec) -> Vec<Bit> {
    sig.bits.iter().map(|b| match b {
        SigBit::Const(x) => *x,
        _ => Bit::Sx,
    }).collect()
}

fn const_str_param(s: &str) -> Const {
    Const::Str(s.to_string())
}

fn ce_b2c(bits: Vec<Bit>) -> Const {
    Const::Bits { width: bits.len() as i64, bits, signed: false, is_unsized: false }
}

fn proc_rom_switch(pm: &mut Pm, sw: &mut SwitchRule, count: &mut i32) -> Result<(), ProcError> {
    for ci in 0..sw.cases.len() {
        proc_rom_case(pm, &mut sw.cases[ci], count)?;
    }
    if sw.cases.is_empty() {
        dbg_inc(); // rejecting switch: no cases
        return Ok(());
    }

    let mut lhs = SigSpec::default();
    let mut lhs_lookup: std::collections::HashMap<(String, i64, u8), usize> = std::collections::HashMap::new();
    for it in &sw.cases[0].actions {
        for bit in &it.0.bits {
            let key = bit_key(bit);
            if !lhs_lookup.contains_key(&key) {
                lhs_lookup.insert(key, lhs.size() as usize);
                lhs.bits.push(bit.clone());
            }
        }
    }
    if lhs.bits.is_empty() {
        dbg_inc(); // rejecting switch: lhs empty
        return Ok(());
    }

    let mut swsigbits: i64 = 0;
    for i in 0..sw.signal.size() as usize {
        if sw.signal.bits[i] != SigBit::Const(Bit::S0) {
            swsigbits = i as i64 + 1;
        }
    }

    let mut vals: std::collections::HashMap<i64, Const> = std::collections::HashMap::new();
    let mut default_val: Const = Const::default();
    let mut got_default = false;
    let mut maxaddr: i64 = 0;
    for cs in &sw.cases {
        if !cs.switches.is_empty() {
            dbg_inc(); // rejecting switch: has nested switches
            return Ok(());
        }
        let mut val = vec![Bit::Sm; lhs.size() as usize];
        for it in &cs.actions {
            if !is_fully_const(&it.1) {
                dbg_inc(); // rejecting switch: rhs not const
                return Ok(());
            }
            for i in 0..it.0.size() as usize {
                let Some(&pos) = lhs_lookup.get(&bit_key(&it.0.bits[i])) else {
                    dbg_inc(); // rejecting switch: lhs not uniform
                    return Ok(());
                };
                val[pos] = match &it.1.bits[i] {
                    SigBit::Const(x) => *x,
                    _ => Bit::Sx,
                };
            }
        }
        if val.iter().any(|&b| b == Bit::Sm) {
            dbg_inc(); // rejecting switch: lhs not uniform
            return Ok(());
        }

        for addr in &cs.compare {
            if !is_fully_def(addr) {
                dbg_inc(); // rejecting switch: case value has undef bits
                return Ok(());
            }
            let mut c = bits_of_const_sig(addr);
            while !c.is_empty() && *c.last().unwrap() == Bit::S0 {
                c.pop();
            }
            if c.len() as i64 > swsigbits {
                continue;
            }
            if c.len() > 30 {
                dbg_inc(); // rejecting switch: address too large
                return Ok(());
            }
            let mut a: i64 = 0;
            for (i, b) in c.iter().enumerate() {
                if *b == Bit::S1 {
                    a |= 1 << i;
                }
            }
            if vals.contains_key(&a) {
                continue;
            }
            vals.insert(a, ce_b2c(val.clone()));
            if a > maxaddr {
                maxaddr = a;
            }
        }
        if cs.compare.is_empty() {
            default_val = ce_b2c(val);
            got_default = true;
            break;
        }
    }
    let abits = ceil_log2(maxaddr + 1);
    if !got_default && (swsigbits > 30 || vals.len() as i64 != (1i64 << swsigbits)) {
        dbg_inc(); // rejecting switch: not all values are covered
        return Ok(());
    }
    if (vals.len() as i64) < 8 {
        dbg_inc(); // rejecting switch: not enough values
        return Ok(());
    }
    if (1i64 << abits) / vals.len() as i64 > 4 {
        dbg_inc(); // rejecting switch: not enough density
        return Ok(());
    }

    // —— 執行(轉換;NEW_ID 錨=proc_rom.cc:154/155 + mem.cc:303/349)——
    let lhs_size = lhs.size();
    let sw_attrs = sw.attributes.clone();
    let memsize = 1i64 << abits;
    let rdata = {
        let id = pm.new_id_at("proc_rom.cc", 154, "do_switch");
        pm.add_wire_id(id, lhs_size)
    };
    let memid = pm.new_id_at("proc_rom.cc", 155, "do_switch");
    let src_attr = sw_attrs.get("\\src").cloned();

    // memory 實體(Mem::emit else 分支:module->memories[memid])。
    pm.module().memories.push(ry_ir::Memory {
        name: memid.clone(),
        width: lhs_size,
        start_offset: 0,
        size: memsize,
        attributes: sw_attrs.clone(),
    });

    // init data(逐地址:vals 有=值、無=default)。
    let mut data: Vec<Bit> = Vec::new();
    for i in 0..memsize {
        let v = match vals.get(&i) {
            Some(v) => v.clone(),
            None => default_val.clone(),
        };
        if let Const::Bits { bits, .. } = &v {
            data.extend(bits.iter().copied());
        }
    }

    // $memrd_v2(MemRd 預設面;CLK/ARST/SRST=空 SigSpec)。
    let rd_id = pm.new_id_at("mem.cc", 303, "emit");
    let rd_params = vec![
        ("\\MEMID", const_str_param(&memid)),
        ("\\ABITS", Const::from_i64_i32_width(abits)),
        ("\\WIDTH", Const::from_i64_i32_width(lhs_size)),
        ("\\CLK_ENABLE", Const::from_i64_i32_width(0)),
        ("\\CLK_POLARITY", Const::from_i64_i32_width(1)),
        ("\\CE_OVER_SRST", Const::from_i64_i32_width(0)),
        ("\\ARST_VALUE", Const::Bits { width: lhs_size, bits: vec![Bit::Sx; lhs_size.max(0) as usize], signed: false, is_unsized: false }),
        ("\\SRST_VALUE", Const::Bits { width: lhs_size, bits: vec![Bit::Sx; lhs_size.max(0) as usize], signed: false, is_unsized: false }),
        ("\\INIT_VALUE", Const::Bits { width: lhs_size, bits: vec![Bit::Sx; lhs_size.max(0) as usize], signed: false, is_unsized: false }),
        ("\\TRANSPARENCY_MASK", Const::Bits { width: 0, bits: vec![], signed: false, is_unsized: false }),
        ("\\COLLISION_X_MASK", Const::Bits { width: 0, bits: vec![], signed: false, is_unsized: false }),
    ];
    let rd_addr = sw.signal.extract(0, abits);
    let rd_ports = vec![
        ("\\CLK", SigSpec { bits: vec![SigBit::Const(Bit::Sx)] }),
        ("\\EN", one_bit_p(true)),
        ("\\ARST", one_bit_p(false)),
        ("\\SRST", one_bit_p(false)),
        ("\\ADDR", rd_addr),
        ("\\DATA", rdata.clone()),
    ];
    let rd_cell = pm.add_cell(rd_id, "$memrd_v2", rd_params, rd_ports, None);
    if let Some(s) = &src_attr {
        pm.module().cells[rd_cell].attributes.set("\\src".to_string(), s.clone());
    }

    // $meminit(en 全 1 → 非 v2;ABITS=32 位地址)。
    let init_id = pm.new_id_at("mem.cc", 349, "emit");
    let init_params = vec![
        ("\\MEMID", const_str_param(&memid)),
        ("\\ABITS", Const::from_i64_i32_width(32)),
        ("\\WIDTH", Const::from_i64_i32_width(lhs_size)),
        ("\\WORDS", Const::from_i64_i32_width(data.len() as i64 / lhs_size)),
        ("\\PRIORITY", Const::from_i64_i32_width(0)),
    ];
    let addr0 = sig_const(Bit::S0, 32);
    let init_ports = vec![("\\ADDR", addr0), ("\\DATA", SigSpec { bits: data.iter().map(|&b| SigBit::Const(b)).collect() })];
    let init_cell = pm.add_cell(init_id, "$meminit", init_params, init_ports, None);
    if let Some(s) = &src_attr {
        pm.module().cells[init_cell].attributes.set("\\src".to_string(), s.clone());
    }

    // cases 重造。
    sw.cases.clear();
    sw.signal = sw.signal.extract(0, swsigbits);
    if abits == sw.signal.size() {
        sw.signal = SigSpec::default();
        let cs = CaseRule {
            attributes: InsertMap::new(),
            compare: Vec::new(),
            actions: vec![(lhs.clone(), rdata.clone())],
            switches: Vec::new(),
        };
        sw.cases.push(cs);
    } else {
        sw.signal = extract_end(&sw.signal, abits);
        let cs1 = CaseRule {
            attributes: InsertMap::new(),
            compare: vec![sig_const(Bit::S0, sw.signal.size())],
            actions: vec![(lhs.clone(), rdata.clone())],
            switches: Vec::new(),
        };
        let cs2 = CaseRule {
            attributes: InsertMap::new(),
            compare: Vec::new(),
            actions: vec![(lhs.clone(), sig_from_const(&default_val))],
            switches: Vec::new(),
        };
        sw.cases.push(cs1);
        sw.cases.push(cs2);
    }
    *count += 1;
    Ok(())
}

fn proc_rom_case(pm: &mut Pm, cs: &mut CaseRule, count: &mut i32) -> Result<(), ProcError> {
    for sw in cs.switches.iter_mut() {
        proc_rom_switch(pm, sw, count)?;
    }
    Ok(())
}

/// `proc_rom` 命令面(無參形)。
pub fn run_proc_rom(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), ProcError> {
    check_no_args("proc_rom", args)?;
    log.header("Executing PROC_ROM pass (convert switches to ROMs).\n");
    let out = std::mem::take(&mut log.out);
    let mut total_count: i32 = 0;
    for mi in (0..design.modules.len()).rev() {
        let mut count: i32 = 0;
        for pi in (0..design.modules[mi].processes.len()).rev() {
            let module = &mut design.modules[mi];
            let autoidx = &mut design.autoidx;
            let mut pm = Pm { module, autoidx };
            let mut root = std::mem::take(&mut pm.module().processes[pi].root_case);
            let res = proc_rom_case(&mut pm, &mut root, &mut count);
            pm.module().processes[pi].root_case = root;
            res?;
        }
        total_count += count;
    }
    log.out = out;
    log.log(&format!(
        "Converted {total_count} switch{}.\n",
        if total_count == 1 { "" } else { "es" }
    ));
    log.resync();
    Ok(())
}

// ===========================================================================
// proc_mux(passes/proc/proc_mux.cc;SigSnippets/SnippetSwCache/pgroups)
// ===========================================================================

/// idict<SigSpec>+bit2snippet+snippets(pool;正向迭代=逆插入序)。
struct SigSnippets {
    sigs: Vec<SigSpec>,                          // idict:首見序編號
    bit2snippet: std::collections::HashMap<(String, i64, u8), usize>,
    snippets: Vec<usize>,                        // pool:插入序(迭代用 .rev())
}

impl SigSnippets {
    fn new() -> SigSnippets {
        SigSnippets { sigs: Vec::new(), bit2snippet: std::collections::HashMap::new(), snippets: Vec::new() }
    }
    fn sigidx(&mut self, sig: SigSpec) -> usize {
        if let Some(i) = self.sigs.iter().position(|s| *s == sig) {
            return i;
        }
        self.sigs.push(sig);
        self.sigs.len() - 1
    }
    fn contains(&self, key: usize) -> bool {
        self.snippets.contains(&key)
    }
    fn insert_snippet(&mut self, key: usize) {
        if !self.snippets.contains(&key) {
            self.snippets.push(key);
        }
    }
    fn bit_key(b: &SigBit) -> (String, i64, u8) {
        bit_key(b)
    }

    fn insert_sig(&mut self, sig: &SigSpec) {
        if sig.bits.is_empty() {
            return;
        }
        let key = self.sigidx(sig.clone());
        if self.contains(key) {
            return;
        }
        let mut new_sig = SigSpec::default();
        let mut i = 0usize;
        while i < sig.bits.len() {
            let bkey = Self::bit_key(&sig.bits[i]);
            let other_key = *self.bit2snippet.get(&bkey).unwrap_or(&usize::MAX);
            if other_key == usize::MAX {
                new_sig.bits.push(sig.bits[i].clone());
                i += 1;
                continue;
            }
            if !new_sig.bits.is_empty() {
                let nk = self.sigidx(new_sig.clone());
                self.insert_snippet(nk);
                for b in &new_sig.bits {
                    self.bit2snippet.insert(Self::bit_key(b), nk);
                }
                new_sig = SigSpec::default();
            }
            let other_sig = self.sigs[other_key].clone();
            let mut k = 0usize;
            let mut n = 1usize;
            while other_sig.bits[k] != sig.bits[i] {
                k += 1;
            }
            while i + n < sig.bits.len() && k + n < other_sig.bits.len() && sig.bits[i + n] == other_sig.bits[k + n] {
                n += 1;
            }
            let sig1 = other_sig.extract(0, k as i64);
            let sig2 = other_sig.extract(k as i64, n as i64);
            let sig3 = other_sig.extract((k + n) as i64, (other_sig.bits.len() - k - n) as i64);
            for b in &other_sig.bits {
                self.bit2snippet.remove(&Self::bit_key(b));
            }
            // snippets=hashlib pool:erase=swap-and-pop(尾元素移到被刪位)。
            if let Some(pos) = self.snippets.iter().position(|&x| x == other_key) {
                let last = self.snippets.len() - 1;
                self.snippets.swap(pos, last);
                self.snippets.pop();
            }
            self.insert_sig(&sig1);
            self.insert_sig(&sig2);
            self.insert_sig(&sig3);
            i += n;
        }
        if !new_sig.bits.is_empty() {
            let nk = self.sigidx(new_sig.clone());
            self.insert_snippet(nk);
            for b in &new_sig.bits {
                self.bit2snippet.insert(Self::bit_key(b), nk);
            }
        }
    }

    fn insert_case(&mut self, cs: &CaseRule) {
        for action in &cs.actions {
            self.insert_sig(&action.0);
        }
        for sw in &cs.switches {
            for cs2 in &sw.cases {
                self.insert_case(cs2);
            }
        }
    }
}

/// SnippetSwCache(全展開;sw 標識=路徑鍵 "s<c>s<c>…"——oracle 用指標
/// 恆等;u64 編碼在深嵌套(loops 16 層)溢出,改字串路徑)。
struct SwCache {
    /// 路徑 → snippet 集合。
    cache: std::collections::HashMap<String, std::collections::HashSet<usize>>,
    full_case_bits: std::collections::HashMap<String, std::collections::HashSet<(String, i64, u8)>>,
    current_snippet: usize,
}

impl SwCache {
    fn new() -> SwCache {
        SwCache { cache: std::collections::HashMap::new(), full_case_bits: std::collections::HashMap::new(), current_snippet: 0 }
    }
    fn check(&self, sid: &str) -> bool {
        self.cache.get(sid).map(|s| s.contains(&self.current_snippet)).unwrap_or(false)
    }
}

fn apply_attrs_strpool(cell_attributes: &mut InsertMap<Const>, sw_attrs: &InsertMap<Const>, cs_attrs: &InsertMap<Const>) {
    *cell_attributes = sw_attrs.clone();
    // add_strpool_attribute(\src, cs 的 src strpool):union 後按 pool 迭代序
    // (逆插入序)以 | join。
    if let Some(src) = cs_attrs.get("\\src") {
        if let Const::Str(s) = src {
            let mut union_tokens: Vec<String> = Vec::new();
            // get_strpool(既有;split '|',按序插入)
            if let Some(existing) = cell_attributes.get("\\src") {
                if let Const::Str(es) = existing {
                    for t in es.split('|') {
                        if !union_tokens.contains(&t.to_string()) {
                            union_tokens.push(t.to_string());
                        }
                    }
                }
            }
            for t in s.split('|') {
                if !union_tokens.contains(&t.to_string()) {
                    union_tokens.push(t.to_string());
                }
            }
            let joined: String = union_tokens.iter().rev().cloned().collect::<Vec<String>>().join("|");
            cell_attributes.set("\\src".to_string(), Const::Str(joined));
        }
    }
}

/// gen_cmp(proc_mux.cc:153;sstr=自取 autoidx++)。
#[allow(clippy::too_many_arguments)]
fn gen_cmp(
    pm: &mut Pm,
    signal: &SigSpec,
    compare: &[SigSpec],
    sw_attrs: &InsertMap<Const>,
    cs_attrs: &InsertMap<Const>,
) -> SigSpec {
    let base = format!("$procmux${}", new_mux_autoidx(pm));
    let cmp_wire_name = format!("{base}_CMP");
    let mut cmp_width: i64 = 0;
    {
        let id = cmp_wire_name.clone();
        let w = pm.add_wire_id(id, 0);
        let _ = w;
    }

    for comp in compare {
        let mut sig = signal.clone();
        let mut comp = comp.clone();
        // 剝 don't-care(Sa)位。
        let mut i = 0usize;
        while i < comp.bits.len() {
            if comp.bits[i] == SigBit::Const(Bit::Sa) {
                sig.bits.remove(i);
                comp.bits.remove(i);
                continue;
            }
            i += 1;
        }
        if comp.bits.is_empty() {
            return SigSpec::default();
        }

        if sig.bits.len() == 1 && comp == one_bit_p(true) {
            // connect(cmp_wire bit, sig);cmp_wire->width++
            let lhs = SigSpec { bits: vec![SigBit::Wire { wire: cmp_wire_name.clone(), offset: cmp_width }] };
            cmp_width += 1;
            let widx = pm.module().wires.iter().position(|w| w.name == cmp_wire_name).unwrap();
            pm.module().wires[widx].width = cmp_width;
            pm.connect(lhs, sig);
        } else {
            let cell_name = format!("{base}_CMP{cmp_width}");
            let y_bit = SigSpec { bits: vec![SigBit::Wire { wire: cmp_wire_name.clone(), offset: cmp_width }] };
            cmp_width += 1;
            let mut attributes = InsertMap::new();
            apply_attrs_strpool(&mut attributes, sw_attrs, cs_attrs);
            let cwidth = pm.module().wires.iter().position(|w| w.name == cmp_wire_name).unwrap();
            pm.module().wires[cwidth].width = cmp_width;
            pm.add_cell(
                cell_name,
                "$eq",
                vec![
                    ("\\A_SIGNED", Const::from_i64_i32_width(0)),
                    ("\\B_SIGNED", Const::from_i64_i32_width(0)),
                    ("\\A_WIDTH", Const::from_i64_i32_width(sig.size())),
                    ("\\B_WIDTH", Const::from_i64_i32_width(comp.size())),
                    ("\\Y_WIDTH", Const::from_i64_i32_width(1)),
                ],
                vec![("\\A", sig), ("\\B", comp), ("\\Y", y_bit)],
                None,
            );
            // add_cell 後覆寫屬性(apply_attrs=attributes 全量+src strpool)。
            let n = pm.module().cells.len() - 1;
            let mut attrs = InsertMap::new();
            apply_attrs_strpool(&mut attrs, sw_attrs, cs_attrs);
            pm.module().cells[n].attributes = attrs;
        }
    }

    if cmp_width == 1 {
        return SigSpec { bits: vec![SigBit::Wire { wire: cmp_wire_name, offset: 0 }] };
    }
    let ctrl_name = format!("{base}_CTRL");
    {
        let id = ctrl_name.clone();
        pm.add_wire_id(id, 1);
    }
    let any_name = format!("{base}_ANY");
    {
        let mut attributes = InsertMap::new();
        apply_attrs_strpool(&mut attributes, sw_attrs, cs_attrs);
        let cmp_sig = SigSpec {
            bits: (0..cmp_width).map(|o| SigBit::Wire { wire: cmp_wire_name.clone(), offset: o }).collect(),
        };
        pm.add_cell(
            any_name,
            "$reduce_or",
            vec![
                ("\\A_SIGNED", Const::from_i64_i32_width(0)),
                ("\\A_WIDTH", Const::from_i64_i32_width(cmp_width)),
                ("\\Y_WIDTH", Const::from_i64_i32_width(1)),
            ],
            vec![("\\A", cmp_sig), ("\\Y", SigSpec { bits: vec![SigBit::Wire { wire: ctrl_name.clone(), offset: 0 }] })],
            None,
        );
        let n = pm.module().cells.len() - 1;
        let mut attrs = InsertMap::new();
        apply_attrs_strpool(&mut attrs, sw_attrs, cs_attrs);
        pm.module().cells[n].attributes = attrs;
    }
    SigSpec { bits: vec![SigBit::Wire { wire: ctrl_name, offset: 0 }] }
}

/// gen_mux(proc_mux.cc:221);返回 result(Y wire)。
#[allow(clippy::too_many_arguments)]
fn gen_mux(
    pm: &mut Pm,
    signal: &SigSpec,
    compare: &[SigSpec],
    when_signal: &SigSpec,
    else_signal: &SigSpec,
    sw_attrs: &InsertMap<Const>,
    cs_attrs: &InsertMap<Const>,
    base: &str,
) -> (SigSpec, Option<String>) {
    if compare.is_empty() || when_signal == else_signal {
        return (when_signal.clone(), None);
    }
    let ctrl_sig = gen_cmp(pm, signal, compare, sw_attrs, cs_attrs);
    if ctrl_sig.bits.is_empty() {
        return (when_signal.clone(), None);
    }
    let result_name = format!("{base}_Y");
    {
        let id = result_name.clone();
        pm.add_wire_id(id, when_signal.size());
    }
    let cell_name = base.to_string();
    let mut attributes = InsertMap::new();
    apply_attrs_strpool(&mut attributes, sw_attrs, cs_attrs);
    let result_sig = SigSpec {
        bits: (0..when_signal.size()).map(|o| SigBit::Wire { wire: result_name.clone(), offset: o }).collect(),
    };
    pm.add_cell(
        cell_name.clone(),
        "$mux",
        vec![("\\WIDTH", Const::from_i64_i32_width(when_signal.size()))],
        vec![
            ("\\A", else_signal.clone()),
            ("\\B", when_signal.clone()),
            ("\\S", ctrl_sig.clone()),
            ("\\Y", result_sig.clone()),
        ],
        None,
    );
    let n = pm.module().cells.len() - 1;
    let mut attrs = InsertMap::new();
    apply_attrs_strpool(&mut attrs, sw_attrs, cs_attrs);
    pm.module().cells[n].attributes = attrs;
    (result_sig, Some(cell_name))
}

/// append_pmux(proc_mux.cc:255):last_mux_cell 改 $pmux 並 append ctrl/when。
fn append_pmux(
    pm: &mut Pm,
    signal: &SigSpec,
    compare: &[SigSpec],
    when_signal: &SigSpec,
    last_mux: &str,
    sw_attrs: &InsertMap<Const>,
    cs_attrs: &InsertMap<Const>,
) {
    {
        let module = pm.module();
        let ci = module.cells.iter().position(|c| c.name == last_mux).unwrap();
        if when_signal == get_port_c(&module.cells[ci], "\\A").unwrap() {
            return;
        }
    }
    let ctrl_sig = gen_cmp(pm, signal, compare, sw_attrs, cs_attrs);
    let module = pm.module();
    let ci = module.cells.iter().position(|c| c.name == last_mux).unwrap();
    module.cells[ci].ctype = "$pmux".to_string();
    let mut new_s = get_port_c(&module.cells[ci], "\\S").cloned().unwrap_or_default();
    new_s.bits.extend(ctrl_sig.bits.iter().cloned());
    set_port_mux(module, ci, "\\S", new_s.clone());
    let mut new_b = get_port_c(&module.cells[ci], "\\B").cloned().unwrap_or_default();
    new_b.bits.extend(when_signal.bits.iter().cloned());
    set_port_mux(module, ci, "\\B", new_b);
    let s_width = new_s.size();
    set_param_mux(module, ci, "\\S_WIDTH", Const::from_i64_i32_width(s_width));
}

fn set_port_mux(module: &mut Module, ci: usize, port: &str, sig: SigSpec) {
    if let Some(e) = module.cells[ci].connections.iter_mut().find(|(p, _)| p == port) {
        e.1 = sig;
    } else {
        module.cells[ci].connections.push((port.to_string(), sig));
    }
}

fn set_param_mux(module: &mut Module, ci: usize, name: &str, v: Const) {
    module.cells[ci].parameters.set(name.to_string(), v);
}

/// signal_to_mux_tree(proc_mux.cc:321)。
#[allow(clippy::too_many_arguments)]
fn signal_to_mux_tree(
    pm: &mut Pm,
    snippets: &SigSnippets,
    swcache: &mut SwCache,
    swpara: &mut std::collections::HashMap<String, bool>,
    cs: &mut CaseRule,
    sid: &str,
    sig: &SigSpec,
    defval: &SigSpec,
    widths: &std::collections::HashMap<String, i64>,
    _log_out: &mut String,
) -> SigSpec {
    let mut result = defval.clone();

    let actions_snapshot = cs.actions.clone();
    for action in &actions_snapshot {
        replace_into(&action.0, &action.1, sig, &mut result);
    }
    for action in &actions_snapshot {
        // action.first.remove2(sig, &action.second)——就地消耗。
        for a in cs.actions.iter_mut() {
            if *a == *action {
                let mut lhs = a.0.clone();
                let mut rhs = a.1.clone();
                remove2(sig, &mut lhs, &mut rhs);
                a.0 = lhs;
                a.1 = rhs;
                break;
            }
        }
    }

    for si in 0..cs.switches.len() {
        let sid_sw = format!("{sid}s{si}");
        if !swcache.check(&sid_sw) {
            continue;
        }

        // pgroups 計算。
        let ncases = cs.switches[si].cases.len();
        let mut pgroups: Vec<i64> = vec![0; ncases];
        let mut is_simple_parallel_case = true;
        if !get_bool_attr(&cs.switches[si].attributes, "\\parallel_case") {
            let entry = swpara.entry(sid_sw.clone());
            match entry {
                std::collections::hash_map::Entry::Occupied(e) => {
                    is_simple_parallel_case = *e.get();
                }
                std::collections::hash_map::Entry::Vacant(e) => {
                    let mut case_values: Vec<Const> = Vec::new();
                    'outer: for cs2 in &cs.switches[si].cases {
                        for pat in &cs2.compare {
                            if !is_fully_def(pat) {
                                is_simple_parallel_case = false;
                                break 'outer;
                            }
                            let cpat = as_const_pm(pat);
                            if case_values.contains(&cpat) {
                                is_simple_parallel_case = false;
                                break 'outer;
                            }
                            case_values.push(cpat);
                        }
                    }
                    e.insert(is_simple_parallel_case);
                }
            }
        }

        if !is_simple_parallel_case {
            let mut pool = BitPatternPool::new_width(cs.switches[si].signal.size());
            let mut extra_group_for_next_case = false;
            for i in 0..ncases {
                let cs2 = &cs.switches[si].cases[i];
                if i != 0 {
                    pgroups[i] = pgroups[i - 1];
                    if extra_group_for_next_case {
                        pgroups[i] = pgroups[i - 1] + 1;
                        extra_group_for_next_case = false;
                    }
                    for pat in &cs2.compare {
                        if !is_fully_const(pat) || !pool.has_all(pat) {
                            pgroups[i] = pgroups[i - 1] + 1;
                        }
                    }
                    if cs2.compare.is_empty() {
                        pgroups[i] = pgroups[i - 1] + 1;
                    }
                    if pgroups[i] != pgroups[i - 1] {
                        pool = BitPatternPool::new_width(cs.switches[si].signal.size());
                    }
                }
                for pat in &cs2.compare {
                    if !is_fully_const(pat) {
                        extra_group_for_next_case = true;
                    } else {
                        pool.take(pat);
                    }
                }
            }
        }

        // full_case 位掩碼。
        {
            let fcb = get_full_case_bits(swcache, &cs.switches[si], &sid_sw.clone());
            let sig_key: Vec<(String, i64, u8)> = sig.bits.iter().map(bit_key).collect();
            for i in 0..result.size() as usize {
                if fcb.contains(&sig_key[i]) {
                    result.bits[i] = SigBit::Const(Bit::Sx);
                }
            }
        }

        // 逆序 cases(首條=最高優先)。
        let initial_val = result.clone();
        let mut last_mux: Option<String> = None;
        for i in 0..ncases {
            let case_idx = ncases - i - 1;
            let mut value = {
                let sw_c = &mut cs.switches[si].cases[case_idx];
                signal_to_mux_tree(pm, snippets, swcache, swpara, sw_c, &format!("{sid_sw}c{case_idx}"), sig, &initial_val, widths, _log_out)
            };
            if let Some(ref last) = last_mux {
                if pgroups[case_idx] == pgroups[case_idx + 1] {
                    let sw_attrs = cs.switches[si].attributes.clone();
                    let cs_attrs = cs.switches[si].cases[case_idx].attributes.clone();
                    append_pmux(pm, &cs.switches[si].signal, &cs.switches[si].cases[case_idx].compare, &value, last, &sw_attrs, &cs_attrs);
                    continue;
                }
            }
            let sw_attrs = cs.switches[si].attributes.clone();
            let cs_attrs = cs.switches[si].cases[case_idx].attributes.clone();
            let base = format!("$procmux${}", new_mux_autoidx(pm));
            let (res, new_last) = gen_mux(pm, &cs.switches[si].signal, &cs.switches[si].cases[case_idx].compare, &value, &result, &sw_attrs, &cs_attrs, &base);
            value = res;
            last_mux = new_last;
            result = value;
        }
    }

    result
}

fn as_const_pm(sig: &SigSpec) -> Const {
    ce_b2c(bits_of_const_sig(sig))
}

/// $procmux$N 的 autoidx++(全域;proc_mux.cc 的 sstr 計數)。
fn new_mux_autoidx(pm: &mut Pm) -> i64 {
    let n = *pm.autoidx;
    *pm.autoidx += 1;
    n
}

/// get_full_case_bits(proc_mux.cc:278;按 sid 緩存;4096 編碼同上)。
fn get_full_case_bits(swcache: &mut SwCache, sw: &SwitchRule, sid: &str) -> std::collections::HashSet<(String, i64, u8)> {
    if let Some(b) = swcache.full_case_bits.get(sid) {
        return b.clone();
    }
    let mut bits: std::collections::HashSet<(String, i64, u8)> = std::collections::HashSet::new();
    if get_bool_attr(&sw.attributes, "\\full_case") {
        let mut first = true;
        for (c, cs) in sw.cases.iter().enumerate() {
            let mut case_bits: std::collections::HashSet<(String, i64, u8)> = std::collections::HashSet::new();
            for action in &cs.actions {
                for b in &action.0.bits {
                    case_bits.insert(bit_key(b));
                }
            }
            for (j, sub) in cs.switches.iter().enumerate() {
                for b in get_full_case_bits(swcache, sub, &format!("{sid}c{c}s{j}")) {
                    case_bits.insert(b);
                }
            }
            if first {
                first = false;
                bits = case_bits;
            } else {
                bits = bits.intersection(&case_bits).cloned().collect();
            }
        }
    }
    swcache.full_case_bits.insert(sid.to_string(), bits.clone());
    bits
}

/// proc_mux 主體(proc_mux.cc:413)。
fn proc_mux_proc(pm: &mut Pm, pi: usize, widths: &std::collections::HashMap<String, i64>, out: &mut String) {
    let mut sigsnip = SigSnippets::new();
    {
        let root = &pm.module().processes[pi].root_case;
        sigsnip.insert_case(root);
    }
    let mut swcache = SwCache::new();
    {
        // insert(root_case, sw_stack):action 位→所屬 snippet,對棧上所有 sw 記。
        let root = &pm.module().processes[pi].root_case;
        let mut stack: Vec<String> = Vec::new();
        cache_insert(&mut swcache, &sigsnip, root, &mut stack, "");
    }
    let mut swpara: std::collections::HashMap<String, bool> = std::collections::HashMap::new();

    let mut cnt = 0;
    let snippet_list: Vec<usize> = sigsnip.snippets.iter().rev().cloned().collect(); // pool 正向=逆插入序
    let total = snippet_list.len();
    let mod_pname = {
        let module = pm.module();
        pair_raw(&module.name, &module.processes[pi].name)
    };
    out.push_str(&format!("Creating decoders for process `{mod_pname}'.\n"));
    for idx in snippet_list {
        swcache.current_snippet = idx;
        let sig = sigsnip.sigs[idx].clone();
        cnt += 1;
        out.push_str(&format!("{:6}/{total}: {}\n", cnt, log_signal(&sig, widths)));

        let defval = sig_const(Bit::Sx, sig.size());
        let mut root = std::mem::take(&mut pm.module().processes[pi].root_case);
        let value = signal_to_mux_tree(pm, &sigsnip, &mut swcache, &mut swpara, &mut root, "", &sig, &defval, widths, out);
        pm.module().processes[pi].root_case = root;
        pm.connect(sig, value);
    }
}

/// SnippetSwCache::insert(cs, sw_stack)。sid 編碼=4096 進制路徑:
/// sw sid=parent*4096+idx+1、case sid=sw_sid*4096+case_idx+1(與
/// signal_to_mux_tree/get_full_case_bits 同一編碼;oracle 用指標恆等,
/// 此處以路徑唯一性替代)。
fn cache_insert(swcache: &mut SwCache, sigsnip: &SigSnippets, cs: &CaseRule, stack: &mut Vec<String>, sid: &str) {
    for action in &cs.actions {
        for b in &action.0.bits {
            let sn = sigsnip.bit2snippet.get(&bit_key(b)).copied().unwrap_or(usize::MAX);
            if sn == usize::MAX {
                continue;
            }
            let snapshot: Vec<String> = stack.clone();
            for sw in snapshot {
                swcache.cache.entry(sw).or_default().insert(sn);
            }
        }
    }
    for (j, sw) in cs.switches.iter().enumerate() {
        let sid_sw = format!("{sid}s{j}");
        stack.push(sid_sw.clone());
        for (c, cs2) in sw.cases.iter().enumerate() {
            cache_insert(swcache, sigsnip, cs2, stack, &format!("{sid_sw}c{c}"));
        }
        stack.pop();
    }
}

/// `proc_mux` 命令面(無參形;-ifx=Y2)。
pub fn run_proc_mux(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), ProcError> {
    check_no_args("proc_mux", args)?;
    log.header("Executing PROC_MUX pass (convert decision trees to multiplexers).\n");
    let mut out = std::mem::take(&mut log.out);
    for mi in (0..design.modules.len()).rev() {
        let widths: std::collections::HashMap<String, i64> =
            design.modules[mi].wires.iter().map(|w| (w.name.clone(), w.width)).collect();
        for pi in (0..design.modules[mi].processes.len()).rev() {
            let module = &mut design.modules[mi];
            let autoidx = &mut design.autoidx;
            let mut pm = Pm { module, autoidx };
            proc_mux_proc(&mut pm, pi, &widths, &mut out);
        }
    }
    log.out = out;
    log.resync();
    Ok(())
}

// ===========================================================================
// proc_dlatch(passes/proc/proc_dlatch.cc;分階段:cells 只讀→可變)
// ===========================================================================

/// FfInitVals 精簡面(wire \init 屬性的位視圖)。
fn initval_bit(module: &Module, wire: &str, offset: i64) -> Option<Bit> {
    let w = module.wires.iter().find(|w| w.name == wire)?;
    let init = w.attributes.get("\\init")?;
    match init {
        Const::Bits { bits, .. } => bits.get(offset as usize).copied(),
        Const::Str(_) => None,
    }
}

fn initval_remove(module: &mut Module, wire: &str, offset: i64) {
    let Some(wi) = module.wires.iter().position(|w| w.name == wire) else { return };
    let width = module.wires[wi].width;
    let mut bits: Vec<Bit> = match module.wires[wi].attributes.get("\\init") {
        Some(Const::Bits { bits, .. }) => {
            let mut b = bits.clone();
            if (b.len() as i64) < width {
                b.resize(width as usize, Bit::Sx);
            }
            b
        }
        _ => return,
    };
    if let Some(slot) = bits.get_mut(offset as usize) {
        *slot = Bit::Sx;
    }
    if bits.iter().all(|&b| b == Bit::Sx || b == Bit::Sz) {
        module.wires[wi].attributes.swap_pop_remove("\\init");
    } else {
        module.wires[wi].attributes.set(
            "\\init".to_string(),
            Const::Bits { width: bits.len() as i64, bits, signed: false, is_unsized: false },
        );
    }
}

/// proc_dlatch 資料庫(cells 索引制)。
struct DlatchDb {
    /// sig_y 位 → (cell 名, 位號)。
    mux_drivers: std::collections::HashMap<(String, i64, u8), (String, i64)>,
    /// cell 名 → mux src wire 位序(逆插入序轉 vector)。
    mux_srcbits: std::collections::HashMap<String, Vec<(String, i64, u8)>>,
    sigusers: std::collections::HashMap<(String, i64, u8), i32>,
    active_bits: std::collections::HashSet<(String, i64, u8)>,
}

fn kkey(b: &SigBit) -> (String, i64, u8) {
    bit_key(b)
}

impl DlatchDb {
    fn new(module: &Module) -> DlatchDb {
        let mut sm = crate::sigmap::SigMap::new();
        sm.set(module);
        let mut db = DlatchDb {
            mux_drivers: std::collections::HashMap::new(),
            mux_srcbits: std::collections::HashMap::new(),
            sigusers: std::collections::HashMap::new(),
            active_bits: std::collections::HashSet::new(),
        };
        for cell in module.cells.iter().rev() {
            if matches!(cell.ctype.as_str(), "$mux" | "$pmux" | "$bwmux") {
                let sy = sm.map(get_port_c(cell, "\\Y").unwrap_or(&SigSpec::default()));
                for (i, b) in sy.bits.iter().enumerate() {
                    db.mux_drivers.insert(kkey(b), (cell.name.clone(), i as i64));
                }
                let mut pool: Vec<(String, i64, u8)> = Vec::new();
                for port in ["\\A", "\\B"] {
                    for b in &sm.map(get_port_c(cell, port).unwrap_or(&SigSpec::default())).bits {
                        let k = kkey(b);
                        if !pool.contains(&k) {
                            pool.push(k);
                        }
                    }
                }
                let wire_only: Vec<(String, i64, u8)> = pool
                    .into_iter()
                    .filter(|(_, _, tag)| *tag == 1)
                    .collect();
                db.mux_srcbits.insert(cell.name.clone(), wire_only);
            }
            let ctype = cell.ctype.clone();
            let known = crate::cells::cell_known(&ctype);
            for (port, conn) in &cell.connections {
                let (is_in, is_out) = crate::cells::port_dir(&ctype, port);
                if !known || is_in {
                    for b in &sm.map(conn).bits {
                        *db.sigusers.entry(kkey(b)).or_insert(0) += 1;
                    }
                }
                let _ = is_out;
            }
        }
        for w in &module.wires {
            if w.port_input {
                for b in &sm.map(&SigSpec {
                    bits: (0..w.width).map(|o| SigBit::Wire { wire: w.name.clone(), offset: o }).collect(),
                }).bits {
                    *db.sigusers.entry(kkey(b)).or_insert(0) += 1;
                }
            }
        }
        db
    }

    fn quickcheck(&self, module: &Module, haystack: &SigSpec, needle: &SigSpec) -> bool {
        let mut sm = crate::sigmap::SigMap::new();
        sm.set(module);
        let mut bits_queue: Vec<(String, i64, u8)> = sm.map(haystack).bits.iter().map(kkey).collect();
        let needle_bits: std::collections::HashSet<(String, i64, u8)> = sm.map(needle).bits.iter().map(kkey).collect();
        let mut cells_visited: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut bits_visited: std::collections::HashSet<(String, i64, u8)> = std::collections::HashSet::new();
        while !bits_queue.is_empty() {
            let mut cells_queue: Vec<String> = Vec::new();
            for bit in &bits_queue {
                if let Some((cn, _)) = self.mux_drivers.get(bit) {
                    if !cells_visited.contains(cn) {
                        cells_queue.push(cn.clone());
                    }
                }
                bits_visited.insert(bit.clone());
            }
            bits_queue.clear();
            for c in cells_queue {
                cells_visited.insert(c.clone());
                if let Some(srcbits) = self.mux_srcbits.get(&c) {
                    for bit in srcbits {
                        if needle_bits.contains(bit) {
                            return true;
                        }
                        if !bits_visited.contains(bit) {
                            bits_queue.push(bit.clone());
                        }
                    }
                }
            }
        }
        false
    }
}

const TRUE_NODE: i32 = 1;
const FALSE_NODE: i32 = 2;

/// rule_node idict(rules_db)+ rules_sig。
struct RulesDb {
    nodes: Vec<RuleNode>,
    sig: std::collections::HashMap<i32, SigBit>,
}

#[derive(Clone, PartialEq)]
struct RuleNode {
    signal: (String, i64, u8),
    match_bit: u8,
    children: Vec<i32>,
}

impl RulesDb {
    fn new() -> RulesDb {
        RulesDb { nodes: Vec::new(), sig: std::collections::HashMap::new() }
    }
    fn intern(&mut self, node: RuleNode) -> i32 {
        if let Some(i) = self.nodes.iter().position(|n| *n == node) {
            return i as i32 + 3;
        }
        self.nodes.push(node);
        self.nodes.len() as i32 - 1 + 3
    }
    fn make_inner1(&mut self, signal: (String, i64, u8), m: u8, child: i32) -> i32 {
        self.intern(RuleNode { signal, match_bit: m, children: vec![child] })
    }
    fn make_inner_children(&mut self, mut children: Vec<i32>) -> i32 {
        children.sort();
        children.dedup();
        self.intern(RuleNode { signal: (String::new(), 0, 10), match_bit: 10, children })
    }
}

#[allow(clippy::too_many_arguments)]
fn find_mux_feedback(
    module: &mut Module,
    db: &mut DlatchDb,
    rules: &mut RulesDb,
    haystack: (String, i64, u8),
    needle: &(String, i64, u8),
    set_undef: bool,
) -> i32 {
    let mut set_undef = set_undef;
    if db.sigusers.get(&haystack).copied().unwrap_or(0) > 1 {
        set_undef = false;
    }
    if &haystack == needle {
        return TRUE_NODE;
    }
    let Some((cell_name, index)) = db.mux_drivers.get(&haystack).cloned() else {
        return FALSE_NODE;
    };
    if !db.active_bits.insert(haystack.clone()) {
        unreachable!("proc_dlatch enter_mux_bit logic loop (錯誤路徑;語料未觸)");
    }
    let ci = module.cells.iter().position(|c| c.name == cell_name).unwrap();
    let sig_a = sigmap_module(module, get_port_c(&module.cells[ci], "\\A").unwrap_or(&SigSpec::default()));
    let sig_b = sigmap_module(module, get_port_c(&module.cells[ci], "\\B").unwrap_or(&SigSpec::default()));
    let sig_s = sigmap_module(module, get_port_c(&module.cells[ci], "\\S").unwrap_or(&SigSpec::default()));
    let width = sig_a.size();
    let is_bwmux = module.cells[ci].ctype == "$bwmux";

    let mut children: Vec<i32> = Vec::new();

    let n = find_mux_feedback(module, db, rules, kkey(&sig_a.bits[index as usize]), needle, set_undef);
    if n != FALSE_NODE {
        if set_undef && sig_a.bits[index as usize] == mk_bit(needle) {
            let ci2 = module.cells.iter().position(|c| c.name == cell_name).unwrap();
            let mut sig = get_port_c(&module.cells[ci2], "\\A").cloned().unwrap();
            sig.bits[index as usize] = SigBit::Const(Bit::Sx);
            set_port_mux(module, ci2, "\\A", sig);
        }
        let srange = if is_bwmux { 1 } else { sig_s.size() as usize };
        let mut sibling_undef = true;
        for i in 0..srange {
            if sig_b.bits[(i as i64 * width + index) as usize] != SigBit::Const(Bit::Sx) {
                sibling_undef = false;
            }
        }
        if !sibling_undef {
            let mut n2 = n;
            if !is_bwmux {
                for i in 0..sig_s.size() as usize {
                    n2 = rules.make_inner1(kkey(&sig_s.bits[i]), 10, n2);
                }
            } else {
                n2 = rules.make_inner1(kkey(&sig_s.bits[index as usize]), 10, n2);
            }
            children.push(n2);
        } else {
            children.push(n);
        }
    }

    let srange = if is_bwmux { 1 } else { sig_s.size() as usize };
    for i in 0..srange {
        let n = find_mux_feedback(module, db, rules, kkey(&sig_b.bits[(i as i64 * width + index) as usize]), needle, set_undef);
        if n != FALSE_NODE {
            if set_undef && sig_b.bits[(i as i64 * width + index) as usize] == mk_bit(needle) {
                let ci2 = module.cells.iter().position(|c| c.name == cell_name).unwrap();
                let mut sig = get_port_c(&module.cells[ci2], "\\B").cloned().unwrap();
                sig.bits[(i as i64 * width + index) as usize] = SigBit::Const(Bit::Sx);
                set_port_mux(module, ci2, "\\B", sig);
            }
            let mut sibling_undef = sig_a.bits[index as usize] == SigBit::Const(Bit::Sx);
            if !is_bwmux {
                for j in 0..sig_s.size() as usize {
                    if j != i && sig_b.bits[(j as i64 * width + index) as usize] != SigBit::Const(Bit::Sx) {
                        sibling_undef = false;
                    }
                }
            }
            if !sibling_undef {
                let sn = if is_bwmux { index as usize } else { i };
                let n2 = rules.make_inner1(kkey(&sig_s.bits[sn]), 11, n);
                children.push(n2);
            } else {
                children.push(n);
            }
        }
    }

    db.active_bits.remove(&haystack);
    if children.is_empty() {
        return FALSE_NODE;
    }
    // pool<int> 迭代序轉 vector(逆插入序)+ sort——sort 後等價。
    let mut uniq = children.clone();
    uniq.sort();
    uniq.dedup();
    rules.make_inner_children(uniq)
}

fn mk_bit(k: &(String, i64, u8)) -> SigBit {
    if k.2 == 1 {
        SigBit::Wire { wire: k.0.clone(), offset: k.1 }
    } else {
        SigBit::Const(match k.2 {
            10 => Bit::S0,
            11 => Bit::S1,
            12 => Bit::Sx,
            13 => Bit::Sz,
            14 => Bit::Sm,
            _ => Bit::Sa,
        })
    }
}

fn sigmap_module(module: &Module, sig: &SigSpec) -> SigSpec {
    let mut sm = crate::sigmap::SigMap::new();
    sm.set(module);
    sm.map(sig)
}

#[allow(clippy::too_many_arguments)]
fn find_mux_constant(
    module: &mut Module,
    db: &mut DlatchDb,
    rules: &mut RulesDb,
    haystack: (String, i64, u8),
    needle: Bit,
    set_undef: bool,
) -> i32 {
    let mut set_undef = set_undef;
    if db.sigusers.get(&haystack).copied().unwrap_or(0) > 1 {
        set_undef = false;
    }
    if haystack == const_key(needle) {
        return TRUE_NODE;
    }
    let Some((cell_name, index)) = db.mux_drivers.get(&haystack).cloned() else {
        return FALSE_NODE;
    };
    if !db.active_bits.insert(haystack.clone()) {
        unreachable!("proc_dlatch enter_mux_bit logic loop (錯誤路徑;語料未觸)");
    }
    let ci = module.cells.iter().position(|c| c.name == cell_name).unwrap();
    let sig_a = sigmap_module(module, get_port_c(&module.cells[ci], "\\A").unwrap_or(&SigSpec::default()));
    let sig_b = sigmap_module(module, get_port_c(&module.cells[ci], "\\B").unwrap_or(&SigSpec::default()));
    let sig_s = sigmap_module(module, get_port_c(&module.cells[ci], "\\S").unwrap_or(&SigSpec::default()));
    let width = sig_a.size();
    let is_bwmux = module.cells[ci].ctype == "$bwmux";

    let mut children: Vec<i32> = Vec::new();

    let n = find_mux_constant(module, db, rules, kkey(&sig_a.bits[index as usize]), needle, set_undef);
    if n != FALSE_NODE {
        if set_undef && sig_a.bits[index as usize] == SigBit::Const(needle) {
            let ci2 = module.cells.iter().position(|c| c.name == cell_name).unwrap();
            let mut sig = get_port_c(&module.cells[ci2], "\\A").cloned().unwrap();
            sig.bits[index as usize] = SigBit::Const(Bit::Sx);
            set_port_mux(module, ci2, "\\A", sig);
        }
        let mut n2 = n;
        if !is_bwmux {
            for i in 0..sig_s.size() as usize {
                n2 = rules.make_inner1(kkey(&sig_s.bits[i]), 10, n2);
            }
        } else {
            n2 = rules.make_inner1(kkey(&sig_s.bits[index as usize]), 10, n2);
        }
        children.push(n2);
    }

    let srange = if is_bwmux { 1 } else { sig_s.size() as usize };
    for i in 0..srange {
        let n = find_mux_constant(module, db, rules, kkey(&sig_b.bits[(i as i64 * width + index) as usize]), needle, set_undef);
        if n != FALSE_NODE {
            if set_undef && sig_b.bits[(i as i64 * width + index) as usize] == SigBit::Const(needle) {
                let ci2 = module.cells.iter().position(|c| c.name == cell_name).unwrap();
                let mut sig = get_port_c(&module.cells[ci2], "\\B").cloned().unwrap();
                sig.bits[(i as i64 * width + index) as usize] = SigBit::Const(Bit::Sx);
                set_port_mux(module, ci2, "\\B", sig);
            }
            let sn = if is_bwmux { index as usize } else { i };
            let n2 = rules.make_inner1(kkey(&sig_s.bits[sn]), 11, n);
            children.push(n2);
        }
    }

    db.active_bits.remove(&haystack);
    if children.is_empty() {
        return FALSE_NODE;
    }
    let mut uniq = children;
    uniq.sort();
    uniq.dedup();
    rules.make_inner_children(uniq)
}

fn const_key(b: Bit) -> (String, i64, u8) {
    (String::new(), 0, match b {
        Bit::S0 => 10,
        Bit::S1 => 11,
        Bit::Sx => 12,
        Bit::Sz => 13,
        Bit::Sm => 14,
        Bit::Sa => 15,
    })
}

/// make_hold(proc_dlatch.cc:319;錨=Not 337/ReduceOr 346/And 350;Eq=339
/// 語料未觸,錨列取 clone 讀碼)。
fn make_hold(
    module: &mut Module,
    autoidx: &mut i64,
    rules: &mut RulesDb,
    n: i32,
    src: Option<&Const>,
) -> SigBit {
    if n == TRUE_NODE {
        return SigBit::Const(Bit::S1);
    }
    if n == FALSE_NODE {
        return SigBit::Const(Bit::S0);
    }
    if let Some(b) = rules.sig.get(&n) {
        return b.clone();
    }
    let node = rules.nodes[(n - 3) as usize].clone();
    let mut and_bits = SigSpec::default();
    let sig = mk_bit(&node.signal);
    let mb = node.match_bit;
    if !(mb == 1 && node.signal.0.is_empty()) && sig != mk_bit_match(&node) {
        if mb == 11 {
            and_bits.bits.push(sig);
        } else if mb == 10 {
            let mut pm = Pm { module, autoidx };
            let id = pm.new_id_at("proc_dlatch.cc", 337, "make_hold");
            let r = pm.helper_not_at(id, 3201, &SigSpec { bits: vec![sig] }, src);
            and_bits.bits.push(r.bits[0].clone());
        } else {
            let mut pm = Pm { module, autoidx };
            let id = pm.new_id_at("proc_dlatch.cc", 339, "make_hold");
            let r = pm.helper_eq_at(id, 3252, &SigSpec { bits: vec![sig] }, &SigSpec { bits: vec![mk_bit_match(&node)] }, src);
            and_bits.bits.push(r.bits[0].clone());
        }
    }
    if !node.children.is_empty() {
        let mut or_bits = SigSpec::default();
        for k in &node.children {
            or_bits.bits.push(make_hold(module, autoidx, rules, *k, src));
        }
        let mut pm = Pm { module, autoidx };
        let id = pm.new_id_at("proc_dlatch.cc", 346, "make_hold");
        let r = pm.helper_reduce_or_at(id, 3205, &or_bits, src);
        and_bits.bits.push(r.bits[0].clone());
    }
    if and_bits.size() == 2 {
        let a0 = and_bits.bits[0].clone();
        let a1 = and_bits.bits[1].clone();
        let mut pm = Pm { module, autoidx };
        let id = pm.new_id_at("proc_dlatch.cc", 350, "make_hold");
        let r = pm.helper_and_at(id, 3249, &SigSpec { bits: vec![a0] }, &SigSpec { bits: vec![a1] }, src);
        and_bits = r;
    }
    let b = and_bits.bits[0].clone();
    rules.sig.insert(n, b.clone());
    b
}

/// rule_node 的 match 位(rule.match 恆 State)。
fn mk_bit_match(node: &RuleNode) -> SigBit {
    SigBit::Const(match node.match_bit {
        10 => Bit::S0,
        11 => Bit::S1,
        12 => Bit::Sx,
        13 => Bit::Sz,
        14 => Bit::Sm,
        _ => Bit::Sa,
    })
}

/// proc_dlatch 主體(proc_dlatch.cc:437;分階段避借用:cells 可變+process
/// 樹讀寫錯峰)。
fn proc_dlatch_proc(
    module: &mut Module,
    autoidx: &mut i64,
    pi: usize,
    widths: &std::collections::HashMap<String, i64>,
    out: &mut String,
    generated_dlatches: &mut std::collections::HashSet<String>,
    warns: &mut Vec<String>,
) -> Result<(), ProcError> {
    let mut latches_out_in: Vec<((String, i64, u8), (String, i64, u8))> = Vec::new();
    let mut nolatches_lhs = SigSpec::default();
    let mut nolatches_rhs = SigSpec::default();
    // db/rules 貫穿整個 pass(oracle:proc_dlatch_db_t 每 module 一個、於
    // proc 迴圈前構造;sigusers/mux_drivers 不被生成 cell 汙染;rules_db
    // 節點 id 跨 find_mux_*/make_hold 共享)。
    let mut rules = RulesDb::new();
    let mut db_shared = DlatchDb::new(module);

    // 階段 A:STa sync 的 actions 分類(quickcheck 只讀 cells)。
    {
        let mut sm = crate::sigmap::SigMap::new();
        sm.set(module);
        let db = DlatchDb::new(module);
        let syncs_clone = module.processes[pi].syncs.clone();
        for (si, sr) in syncs_clone.iter().enumerate() {
            if sr.stype != SyncType::Always {
                continue;
            }
            for ss in &sr.actions {
                let lhs = sm.map(&ss.0);
                let rhs = sm.map(&ss.1);
                if !db.quickcheck(module, &rhs, &lhs) {
                    nolatches_lhs.bits.extend(lhs.bits.iter().cloned());
                    nolatches_rhs.bits.extend(rhs.bits.iter().cloned());
                    continue;
                }
                for i in 0..lhs.size() as usize {
                    let k = kkey(&lhs.bits[i]);
                    let v = kkey(&rhs.bits[i]);
                    latches_out_in.retain(|(a, _)| *a != k);
                    latches_out_in.push((k, v));
                }
            }
            module.processes[pi].syncs[si].actions.clear();
        }
    }

    // dict sort(按 key SigBit 排序)。
    latches_out_in.sort_by(|a, b| {
        let (x, y) = (&mk_bit(&a.0), &mk_bit(&b.0));
        if crate::sigops::bit_lt(x, y) {
            std::cmp::Ordering::Less
        } else if crate::sigops::bit_lt(y, x) {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    });

    let mut latches_lhs = SigSpec::default();
    let mut latches_rhs = SigSpec::default();
    let mut holds: Vec<((String, i64, u8), i32)> = Vec::new();
    let mut nrsts: Vec<((String, i64, u8), i32)> = Vec::new();
    let mut nsets: Vec<((String, i64, u8), i32)> = Vec::new();

    // 階段 B:find_mux_feedback/constant(cells 可變:set_undef 改端口)。
    for (ob, ib) in latches_out_in.clone() {

        let mut db = DlatchDb::new(module);
        if find_mux_feedback(module, &mut db, &mut rules, ib.clone(), &ob, false) == FALSE_NODE {
            nolatches_lhs.bits.push(mk_bit(&ob));
            nolatches_rhs.bits.push(mk_bit(&ib));
            continue;
        }
        latches_lhs.bits.push(mk_bit(&ob));
        latches_rhs.bits.push(mk_bit(&ib));

        let nrst = {
            find_mux_constant(module, &mut db_shared, &mut rules, ib.clone(), Bit::S0, false)
        };
        let nset = {
            find_mux_constant(module, &mut db_shared, &mut rules, ib.clone(), Bit::S1, false)
        };
        let has_rst = nrst != FALSE_NODE;
        let has_set = nset != FALSE_NODE;
        let nrst2 = if has_rst && !has_set {
            find_mux_constant(module, &mut db_shared, &mut rules, ib.clone(), Bit::S0, true)
        } else if has_set && !has_rst {
            find_mux_constant(module, &mut db_shared, &mut rules, ib.clone(), Bit::S1, true)
        } else {
            FALSE_NODE
        };
        let nset2 = if has_rst && !has_set { FALSE_NODE } else if has_set && !has_rst { nset } else { FALSE_NODE };

        let mut db = DlatchDb::new(module);

        let n = find_mux_feedback(module, &mut db, &mut rules, ib.clone(), &ob, true);
        holds.push((ob.clone(), n));
        nrsts.push((ob.clone(), nrst2));
        nsets.push((ob, nset2));
    }

    // 階段 C:nolatches → connect+init 清除;latches → cell 生成。
    let mut offset = 0i64;
    for chunk in ry_ir::printer::sig_chunks(&nolatches_lhs) {
        let w = chunk_width(&chunk);
        let lhs = nolatches_lhs.extract(offset, w);
        let rhs = nolatches_rhs.extract(offset, w);
        let mut is_nosync = true;
        for b in &lhs.bits {
            match b {
                SigBit::Wire { wire, .. } => {
                    let nosync = module
                        .wires
                        .iter()
                        .find(|w| &w.name == wire)
                        .map(|w| get_bool_attr(&w.attributes, "\\nosync"))
                        .unwrap_or(false);
                    if !nosync {
                        is_nosync = false;
                        break;
                    }
                }
                _ => {
                    is_nosync = false;
                    break;
                }
            }
        }
        if !is_nosync {
            // db.module->name.c_str()=原始形(帶 \);proc->name 指針=unescape。
            let m = module.name.clone();
            let pname = id_unescape(&module.processes[pi].name);
            out.push_str(&format!(
                "No latch inferred for signal `{}.{}' from process `{}.{}'.\n",
                m,
                log_signal(&lhs, widths),
                m,
                pname
            ));
        }
        for b in &lhs.bits {
            if let SigBit::Wire { wire, offset: off } = b {
                let val = initval_bit(module, wire, *off);
                if let Some(v) = val {
                    if v != Bit::Sx {
                        let m = module.name.clone();
                        let pname = module.processes[pi].name.strip_prefix('\\').unwrap_or(&module.processes[pi].name).to_string();
                        let vsig = SigSpec { bits: vec![SigBit::Const(v)] };
                        let bsig = SigSpec { bits: vec![b.clone()] };
                        out.push_str(&format!(
                            "Removing init bit {} for non-memory siginal `{}.{} in process `{}.{}'.\n",
                            log_signal(&vsig, widths),
                            m,
                            log_signal(&bsig, widths),
                            m,
                            pname
                        ));
                    }
                    initval_remove(module, wire, *off);
                }
            }
        }
        module.connections.push((lhs, rhs));
        offset += w;
    }

    let mut offset = 0i64;
    while offset < latches_lhs.size() {
        let mut width = 1i64;
        let obit = latches_lhs.bits[offset as usize].clone();
        let obkey = kkey(&obit);
        let Some(&n) = holds.iter().find(|(k, _)| *k == obkey).map(|(_, v)| v) else {
            offset += width;
            continue;
        };
        let nrst = nrsts.iter().find(|(k, _)| *k == obkey).map(|(_, v)| *v).unwrap_or(FALSE_NODE);
        let nset = nsets.iter().find(|(k, _)| *k == obkey).map(|(_, v)| *v).unwrap_or(FALSE_NODE);
        let wire_name = match &obit {
            SigBit::Wire { wire, .. } => Some(wire.clone()),
            _ => None,
        };
        if let Some(wname) = &wire_name {
            while offset + width < latches_lhs.size() {
                let nb = latches_lhs.bits[(offset + width) as usize].clone();
                let nbkey = kkey(&nb);
                let same_hold = holds.iter().find(|(k, _)| *k == nbkey).map(|(_, v)| *v) == Some(n)
                    && nrsts.iter().find(|(k, _)| *k == nbkey).map(|(_, v)| *v) == Some(nrst)
                    && nsets.iter().find(|(k, _)| *k == nbkey).map(|(_, v)| *v) == Some(nset);
                let same_wire = match &nb {
                    SigBit::Wire { wire, .. } => wire == wname,
                    _ => false,
                };
                if same_hold && same_wire {
                    width += 1;
                } else {
                    break;
                }
            }
            let lhs = latches_lhs.extract(offset, width);
            let rhs = latches_rhs.extract(offset, width);
            let src = module.processes[pi].attributes.get("\\src").cloned();
            let mut pm = Pm { module, autoidx };
            // clone 形(proc_dlatch.cc:549/555/557/559):en=Not(NEW_ID@549,
            // make_hold(n))——NEW_ID 先取、make_hold 後求值(C++ 參數從左到
            // 右);EN=en、POLARITY=1。尾 opt_expr 的 handle_polarity_inv 會
            // 把它化成 EN=make_hold 直連+POLARITY=0(loops/subbytes golden
            // 的 P0 形即 opt 後形)。
            let not_id = pm.new_id_at("proc_dlatch.cc", 549, "proc_dlatch");
            let hold_sig = make_hold(pm.module, pm.autoidx, &mut rules, n, src.as_ref());
            let en_sig = pm.helper_not_at(not_id, 3201, &SigSpec { bits: vec![hold_sig.clone()] }, None);
            let cell_name = if nrst != FALSE_NODE {
                let id = pm.new_id_at("proc_dlatch.cc", 555, "proc_dlatch");
                let rst_hold = make_hold(pm.module, pm.autoidx, &mut rules, nrst, src.as_ref());
                let rst_bit = SigSpec { bits: vec![rst_hold] };
                let params = vec![
                    ("\\EN_POLARITY", Const::from_i64_i32_width(1)),
                    ("\\CLR_POLARITY", Const::from_i64_i32_width(1)),
                    ("\\WIDTH", Const::from_i64_i32_width(width)),
                    ("\\ARST_POLARITY", Const::from_i64_i32_width(0)),
                    ("\\ARST_VALUE", Const::Bits { width, bits: vec![Bit::S0; width as usize], signed: false, is_unsized: false }),
                ];
                let ports = vec![
                    ("\\EN", en_sig.clone()),
                    ("\\ARST", rst_bit),
                    ("\\D", rhs.clone()),
                    ("\\Q", lhs.clone()),
                ];
                pm.add_cell(id, "$adlatch", params, ports, None);
                let n = pm.module().cells.len() - 1;
                let cname = pm.module().cells[n].name.clone();
                cname
            } else if nset != FALSE_NODE {
                let id = pm.new_id_at("proc_dlatch.cc", 557, "proc_dlatch");
                let set_hold = make_hold(pm.module, pm.autoidx, &mut rules, nset, src.as_ref());
                let set_bit = SigSpec { bits: vec![set_hold] };
                let params = vec![
                    ("\\EN_POLARITY", Const::from_i64_i32_width(1)),
                    ("\\SET_POLARITY", Const::from_i64_i32_width(1)),
                    ("\\WIDTH", Const::from_i64_i32_width(width)),
                    ("\\ARST_POLARITY", Const::from_i64_i32_width(1)),
                    ("\\ARST_VALUE", Const::Bits { width, bits: vec![Bit::S1; width as usize], signed: false, is_unsized: false }),
                ];
                let ports = vec![
                    ("\\EN", en_sig.clone()),
                    ("\\ARST", set_bit),
                    ("\\D", rhs.clone()),
                    ("\\Q", lhs.clone()),
                ];
                pm.add_cell(id, "$adlatch", params, ports, None);
                let n = pm.module().cells.len() - 1;
                pm.module().cells[n].name.clone()
            } else {
                let id = pm.new_id_at("proc_dlatch.cc", 559, "proc_dlatch");
                let params = vec![
                    ("\\EN_POLARITY", Const::from_i64_i32_width(1)),
                    ("\\WIDTH", Const::from_i64_i32_width(width)),
                ];
                let ports = vec![
                    ("\\EN", en_sig.clone()),
                    ("\\D", rhs.clone()),
                    ("\\Q", lhs.clone()),
                ];
                pm.add_cell(id, "$dlatch", params, ports, None);
                let n = pm.module().cells.len() - 1;
                pm.module().cells[n].name.clone()
            };
            {
            }
            // set_src_attribute(src)
            if let Some(s) = &src {
                let module2 = pm.module();
                let ci = module2.cells.iter().position(|c| c.name == cell_name).unwrap();
                module2.cells[ci].attributes.set("\\src".to_string(), s.clone());
            }
            generated_dlatches.insert(cell_name.clone());
            let _ = cell_name;
            let (m, mraw, pname) = {
                let module2 = pm.module();
                let mn = module2.name.clone();
                let pn = module2.processes[pi].name.clone();
                (
                mn.clone(),
                    mn.clone(),
                pn.clone(),
                )
            };
            let cun = cell_name.strip_prefix('\\').unwrap_or(&cell_name).to_string();
            let latch_msg = format!(
                "Latch inferred for signal `{}.{}' from process `{mraw}.{pname}': {cun}",
                m,
                log_signal(&lhs, widths)
            );
            warns.push(latch_msg.clone());
            out.push_str(&format!("Warning: {latch_msg}\n"));
        }
        offset += width;
    }
    Ok(())
}

/// fixup_mux(proc_dlatch.cc:357):剝全 undef B 行、pmux/mux 降型。
fn fixup_mux(module: &mut Module, ci: usize) {
    let sig_a = get_port_c(&module.cells[ci], "\\A").cloned().unwrap_or_default();
    let sig_b = get_port_c(&module.cells[ci], "\\B").cloned().unwrap_or_default();
    let sig_s = get_port_c(&module.cells[ci], "\\S").cloned().unwrap_or_default();
    let width = sig_a.size();
    let mut sig_any_valid_b = SigSpec::default();
    let mut sig_new_b = SigSpec::default();
    let mut sig_new_s = SigSpec::default();
    for i in 0..sig_s.size() as usize {
        let b = sig_b.extract(i as i64 * width, width);
        if !is_fully_undef(&b) {
            sig_any_valid_b = b.clone();
            sig_new_b.bits.extend(b.bits.iter().cloned());
            sig_new_s.bits.push(sig_s.bits[i].clone());
        }
    }
    if sig_new_s.bits.is_empty() {
        sig_new_b = sig_a.clone();
        sig_new_s = one_bit_p(false);
    }
    if is_fully_undef(&sig_a) && !sig_any_valid_b.bits.is_empty() {
        set_port_mux(module, ci, "\\A", sig_any_valid_b);
    }
    if sig_new_s.size() == 1 {
        module.cells[ci].ctype = "$mux".to_string();
        unset_param_mux(module, ci, "\\S_WIDTH");
    } else {
        module.cells[ci].ctype = "$pmux".to_string();
        set_param_mux(module, ci, "\\S_WIDTH", Const::from_i64_i32_width(sig_new_s.size()));
    }
    set_port_mux(module, ci, "\\B", sig_new_b);
    set_port_mux(module, ci, "\\S", sig_new_s);
}

fn unset_param_mux(module: &mut Module, ci: usize, name: &str) {
    module.cells[ci].parameters.swap_pop_remove(name);
}

/// fixup_muxes(proc_dlatch.cc:394;generated_dlatches 上游擴散)。
fn fixup_muxes(module: &mut Module, generated_dlatches: &std::collections::HashSet<String>) {
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut upstream_cell2net: std::collections::HashMap<String, Vec<(String, i64, u8)>> = std::collections::HashMap::new();
    let mut upstream_net2cell: std::collections::HashMap<(String, i64, u8), Vec<String>> = std::collections::HashMap::new();

    for cell in &module.cells {
        let ctype = cell.ctype.clone();
        for (port, conn) in &cell.connections {
            let (is_in, is_out) = crate::cells::port_dir(&ctype, port);
            let mapped = sigmap_module(module, conn);
            if is_in {
                for bit in &mapped.bits {
                    upstream_cell2net.entry(cell.name.clone()).or_default().push(kkey(bit));
                }
            }
            if is_out {
                for bit in &mapped.bits {
                    upstream_net2cell.entry(kkey(bit)).or_default().push(cell.name.clone());
                }
            }
        }
    }

    let mut queue: Vec<String> = generated_dlatches.iter().cloned().collect();
    queue.sort();

    while !queue.is_empty() {
        let mut next_queue: std::collections::HashSet<String> = std::collections::HashSet::new();
        for cell_name in &queue {
            let ci = module.cells.iter().position(|c| &c.name == cell_name);
            if let Some(ci) = ci {
                if matches!(module.cells[ci].ctype.as_str(), "$mux" | "$pmux") {
                    fixup_mux(module, ci);
                }
                if let Some(nets) = upstream_cell2net.get(cell_name) {
                    for bit in nets {
                        if let Some(drivers) = upstream_net2cell.get(bit) {
                            for d in drivers {
                                next_queue.insert(d.clone());
                            }
                        }
                    }
                }
                visited.insert(cell_name.clone());
            }
        }
        queue = next_queue
            .into_iter()
            .filter(|c| !visited.contains(c))
            .filter(|c| {
                module
                    .cells
                    .iter()
                    .find(|cell| &cell.name == c)
                    .map(|cell| crate::cells::cell_known(&cell.ctype))
                    .unwrap_or(false)
            })
            .collect();
        queue.sort();
    }
}

/// `proc_dlatch` 命令面(無參形;-latches=Y2)。
pub fn run_proc_dlatch(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), ProcError> {
    check_no_args("proc_dlatch", args)?;
    log.header("Executing PROC_DLATCH pass (convert process syncs to latches).\n");
    let mut out = std::mem::take(&mut log.out);
    let result = (|| -> Result<(), ProcError> {
        for mi in (0..design.modules.len()).rev() {
            let widths: std::collections::HashMap<String, i64> =
                design.modules[mi].wires.iter().map(|w| (w.name.clone(), w.width)).collect();
            let mut generated_dlatches: std::collections::HashSet<String> = std::collections::HashSet::new();
            for pi in (0..design.modules[mi].processes.len()).rev() {
                let module = &mut design.modules[mi];
                let autoidx = &mut design.autoidx;
                proc_dlatch_proc(module, autoidx, pi, &widths, &mut out, &mut generated_dlatches, &mut log.warnings)?;
            }
            fixup_muxes(&mut design.modules[mi], &generated_dlatches);
        }
        Ok(())
    })();
    log.out = out;
    log.resync();
    result
}

// ===========================================================================
// proc_dff(passes/proc/proc_dff.cc)
// ===========================================================================

/// find_any_lvalue(proc_dff.cc:31)。
fn find_any_lvalue(proc: &Process) -> SigSpec {
    let mut lvalue = SigSpec::default();
    for sync in &proc.syncs {
        for action in &sync.actions {
            if !action.0.bits.is_empty() {
                lvalue = action.0.clone();
                sort_and_unify(&mut lvalue);
                break;
            }
        }
        // 注意:oracle 的 break 只出內層(首個非空 action 全量取)。
    }
    for sync in &proc.syncs {
        let mut this_lvalue = SigSpec::default();
        for action in &sync.actions {
            this_lvalue.bits.extend(action.0.bits.iter().cloned());
        }
        sort_and_unify(&mut this_lvalue);
        // extract(lvalue):this 中出現 lvalue 的位(this 序)。
        let common: SigSpec = SigSpec {
            bits: this_lvalue.bits.iter().filter(|b| lvalue.bits.contains(b)).cloned().collect(),
        };
        if !common.bits.is_empty() {
            lvalue = common;
        }
    }
    lvalue
}

/// gen_dff(proc_dff.cc:113;cell 名=$procdff$N@autoidx++)。
#[allow(clippy::too_many_arguments)]
fn gen_dff(
    pm: &mut Pm,
    sig_in: &SigSpec,
    val_rst: &Const,
    sig_out: &SigSpec,
    clk_polarity: bool,
    arst_polarity: bool,
    clk: &SigSpec,
    arst: Option<&SigSpec>,
    proc_attrs: &InsertMap<Const>,
    out: &mut String,
) {
    let cell_name = format!("$procdff${}", new_mux_autoidx(pm));
    let cell_name_disp = cell_name.clone();
    let has_clk = !clk.bits.is_empty();
    let ctype = if has_clk {
        if arst.is_some() { "$adff" } else { "$dff" }
    } else {
        "$ff"
    };
    let mut params: Vec<(&str, Const)> = vec![("\\WIDTH", Const::from_i64_i32_width(sig_in.size()))];
    if arst.is_some() {
        // 實錄形(mem_arst/omsp golden 釘死):true→1'1(1 位)、false→int 0
        // (32 位 autoint)——oracle 的 Const(bool) 兩形。
        params.push(("\\ARST_POLARITY", if arst_polarity {
            Const::Bits { width: 1, bits: vec![Bit::S1], signed: false, is_unsized: false }
        } else {
            Const::from_i64_i32_width(0)
        }));
        params.push(("\\ARST_VALUE", val_rst.clone()));
    }
    if has_clk {
        params.push(("\\CLK_POLARITY", Const::Bits { width: 1, bits: vec![if clk_polarity { Bit::S1 } else { Bit::S0 }], signed: false, is_unsized: false }));
    }
    let mut ports: Vec<(&str, SigSpec)> = vec![("\\D", sig_in.clone()), ("\\Q", sig_out.clone())];
    if let Some(a) = arst {
        ports.push(("\\ARST", a.clone()));
    }
    if has_clk {
        ports.push(("\\CLK", clk.clone()));
    }
    let mut attributes = proc_attrs.clone();
    let _ = &mut attributes;
    let ci = pm.add_cell(cell_name, ctype, params, ports, None);
    pm.module().cells[ci].attributes = proc_attrs.clone();

    let ty = ctype;
    let message = if has_clk {
        format!(
            "  created {ty} cell `{cell_name_disp}' with {} edge clock",
            if clk_polarity { "positive" } else { "negative" }
        )
    } else {
        format!("  created {ty} cell `{cell_name_disp}' with global clock")
    };
    let message = if arst.is_some() {
        format!("{message} and {} level reset", if arst_polarity { "positive" } else { "negative" })
    } else {
        message
    };
    out.push_str(&format!("{message}.\n"));
}

/// gen_dffsr_complex(proc_dff.cc:56;錨=Mux 74/78、Not 69/77)。
fn gen_dffsr_complex(
    pm: &mut Pm,
    sig_d: &SigSpec,
    sig_q: &SigSpec,
    clk: &SigSpec,
    clk_polarity: bool,
    async_rules: &[(SigSpec, SyncType, SigSpec)], // (value, type, signal)
    proc_attrs: &InsertMap<Const>,
    out: &mut String,
) {
    let mut sig_sr_set = sig_const(Bit::S0, sig_d.size());
    let mut sig_sr_clr = sig_const(Bit::S0, sig_d.size());
    for (sync_value, stype, signal) in async_rules.iter().rev() {
        let pos_trig = if *stype == SyncType::High {
            signal.clone()
        } else {
            let id = pm.new_id_at("proc_dff.cc", 69, "gen_dffsr_complex");
            pm.helper_not_at(id, 3201, signal, None)
        };
        let id = pm.new_id_at("proc_dff.cc", 74, "gen_dffsr_complex");
        sig_sr_set = pm.helper_mux_at(id, 3337, &sig_sr_set, sync_value, &pos_trig, None);
        let id2 = pm.new_id_at("proc_dff.cc", 77, "gen_dffsr_complex");
        let sync_value_inv = pm.helper_not_at(id2, 3201, sync_value, None);
        let id3 = pm.new_id_at("proc_dff.cc", 78, "gen_dffsr_complex");
        sig_sr_clr = pm.helper_mux_at(id3, 3337, &sig_sr_clr, &sync_value_inv, &pos_trig, None);
    }
    let cell_name = format!("$procdff${}", new_mux_autoidx(pm));
    // addDffsr(rtlil.cc:3681)的參數/端口序:CLK/SET/CLR_POLARITY、WIDTH;
    // CLK/SET/CLR/D/Q。
    let params = vec![
        ("\\CLK_POLARITY", Const::from_i64_i32_width(clk_polarity as i64)),
        ("\\SET_POLARITY", Const::from_i64_i32_width(1)),
        ("\\CLR_POLARITY", Const::from_i64_i32_width(1)),
        ("\\WIDTH", Const::from_i64_i32_width(sig_d.size())),
    ];
    let ports = vec![
        ("\\CLK", clk.clone()),
        ("\\SET", sig_sr_set),
        ("\\CLR", sig_sr_clr),
        ("\\D", sig_d.clone()),
        ("\\Q", sig_q.clone()),
    ];
    let ci = pm.add_cell(cell_name.clone(), "$dffsr", params, ports, None);
    pm.module().cells[ci].attributes = proc_attrs.clone();
    out.push_str(&format!(
        "  created $dffsr cell `{cell_name}' with {} edge clock and multiple level-sensitive resets.\n",
        if clk_polarity { "positive" } else { "negative" }
    ));
}

/// gen_aldff(proc_dff.cc:91;語料未觸,錨取 clone)。
fn gen_aldff(
    pm: &mut Pm,
    sig_in: &SigSpec,
    sig_set: &SigSpec,
    sig_out: &SigSpec,
    clk_polarity: bool,
    set_polarity: bool,
    clk: &SigSpec,
    set: &SigSpec,
    proc_attrs: &InsertMap<Const>,
    out: &mut String,
) {
    let cell_name = format!("$procdff${}", new_mux_autoidx(pm));
    let params = vec![
        ("\\WIDTH", Const::from_i64_i32_width(sig_in.size())),
        ("\\ALOAD_POLARITY", Const::from_i64_i32_width(set_polarity as i64)),
        ("\\CLK_POLARITY", Const::from_i64_i32_width(clk_polarity as i64)),
    ];
    let ports = vec![
        ("\\D", sig_in.clone()),
        ("\\Q", sig_out.clone()),
        ("\\AD", sig_set.clone()),
        ("\\CLK", clk.clone()),
        ("\\ALOAD", set.clone()),
    ];
    let ci = pm.add_cell(cell_name.clone(), "$aldff", params, ports, None);
    pm.module().cells[ci].attributes = proc_attrs.clone();
    out.push_str(&format!(
        "  created $aldff cell `{cell_name}' with {} edge clock and {} level non-const reset.\n",
        if clk_polarity { "positive" } else { "negative" },
        if set_polarity { "positive" } else { "negative" }
    ));
}

/// proc_dff 主體(proc_dff.cc:148)。ce_eval=rstval 求值(全常數=true;
/// 語料 rstval 經 assign_map 即常數,eval 面由 probe 對拍兜底)。
fn proc_dff_proc(
    pm: &mut Pm,
    pi: usize,
    widths: &std::collections::HashMap<String, i64>,
    out: &mut String,
    warns: &mut Vec<String>,
) -> Result<(), ProcError> {
    loop {
        let (mut sig, syncs_snapshot, proc_attrs) = {
            let module = pm.module();
            let proc = &module.processes[pi];
            let sig = find_any_lvalue(proc);
            if sig.bits.is_empty() {
                return Ok(());
            }
            (sig, proc.syncs.clone(), proc.attributes.clone())
        };

        {
            let m = pm.module().name.clone();
            let p = pm.module().processes[pi].name.clone();
            out.push_str(&format!(
                "Creating register for signal `{m}.{}' using process `{m}.{}'.\n",
                log_signal(&sig, widths),
                p
            ));
        }

        let mut insig = sig_const(Bit::Sz, sig.size());
        let mut sync_edge: Option<(SyncType, SigSpec)> = None; // (type, signal)
        let mut sync_always = false;
        let mut global_clock = false;
        let mut async_rules: Vec<(SigSpec, SyncType, SigSpec)> = Vec::new();

        for sync in &syncs_snapshot {
            for action in &sync.actions {
                // action.first.extract(sig):交集非空才續。
                let inter: SigSpec = SigSpec {
                    bits: action.0.bits.iter().filter(|b| sig.bits.contains(b)).cloned().collect(),
                };
                if inter.bits.is_empty() {
                    continue;
                }
                match sync.stype {
                    SyncType::Low | SyncType::High => {
                        let mut rstval = sig_const(Bit::Sz, sig.size());
                        replace_into(&action.0, &action.1, &sig, &mut rstval);
                        async_rules.push((rstval, sync.stype, sync.signal.clone().unwrap_or_default()));
                    }
                    SyncType::Posedge | SyncType::Negedge => {
                        if let Some((t, _)) = &sync_edge {
                            if *t != sync.stype {
                                return Err(err("Multiple edge sensitive events found for this signal!"));
                            }
                        }
                        replace_into(&action.0, &action.1, &sig, &mut insig);
                        sync_edge = Some((sync.stype, sync.signal.clone().unwrap_or_default()));
                    }
                    SyncType::Always => {
                        if sync_always {
                            return Err(err("Multiple always events found for this signal!"));
                        }
                        replace_into(&action.0, &action.1, &sig, &mut insig);
                        sync_always = true;
                    }
                    SyncType::Global => {
                        replace_into(&action.0, &action.1, &sig, &mut insig);
                        global_clock = true;
                    }
                    _ => {
                        return Err(err("Event with any-edge sensitivity found for this signal!"));
                    }
                }
                // action.first.remove2(sig, &action.second)——就地消耗。
                for si in 0..syncs_snapshot.len() {
                    let _ = si;
                }
                {
                    let module = pm.module();
                    let sync_real = module.processes[pi].syncs.iter_mut().find(|s| s.stype == sync.stype && s.signal == sync.signal).unwrap();
                    for a in sync_real.actions.iter_mut() {
                        if a.0 == action.0 && a.1 == action.1 {
                            let mut lhs = a.0.clone();
                            let mut rhs = a.1.clone();
                            remove2(&sig, &mut lhs, &mut rhs);
                            a.0 = lhs;
                            a.1 = rhs;
                            break;
                        }
                    }
                }
            }
        }

        // async 全同值合併。
        if !async_rules.is_empty() && async_rules.iter().all(|r| r.0 == async_rules[0].0) {
            let rstval = async_rules[0].0.clone();
            let mut triggers = SigSpec::default();
            for (_, stype, signal) in &async_rules {
                if *stype == SyncType::High {
                    triggers.bits.extend(signal.bits.iter().cloned());
                } else {
                    let id = pm.new_id_at("proc_dff.cc", 221, "proc_dff");
                    let nv = pm.helper_not_at(id, 3201, signal, None);
                    triggers.bits.extend(nv.bits.iter().cloned());
                }
            }
            let id = pm.new_id_at("proc_dff.cc", 226, "proc_dff");
            let red = pm.helper_reduce_or_at(id, 3205, &triggers, None);
            // single_async_rule{ST1, red, [(sig, rstval)]}
            async_rules = vec![(rstval, SyncType::High, red)];
        }

        let sig_q = sig.clone();
        // ce.assign_map.apply(insig/sig)(proc_dff.cc:235-236;proc_mux 生成的
        // connect 經 SigMap 把中間 wire 映射到代表元)。
        {
            let mut am = crate::sigmap::SigMap::new();
            am.set(pm.module());
            insig = am.map(&insig);
            sig = am.map(&sig);
        }

        // rstval==sig 自身:併入輸入並刪規則。
        if async_rules.len() == 1 && async_rules[0].0 == sig {
            let (rstval, stype, signal) = async_rules[0].clone();
            let _ = rstval;
            if stype == SyncType::High {
                let id = pm.new_id_at("proc_dff.cc", 243, "proc_dff");
                insig = pm.helper_mux_at(id, 3337, &insig, &sig, &signal, None);
            } else {
                let id = pm.new_id_at("proc_dff.cc", 245, "proc_dff");
                insig = pm.helper_mux_at(id, 3337, &sig, &insig, &signal, None);
            }
            async_rules.clear();
        }

        if sync_always {
            if sync_edge.is_some() || !async_rules.is_empty() {
                return Err(err("Mixed always event with edge and/or level sensitive events!"));
            }
            out.push_str("  created direct connection (no actual register cell created).\n");
            pm.connect(sig.clone(), insig);
            continue;
        }

        if sync_edge.is_none() && !global_clock {
            return Err(err("Missing edge-sensitive event for this signal!"));
        }

        if async_rules.len() > 1 {
            warns.push(format!("Complex async reset for dff `{}'", log_signal(&sig, widths)));
            out.push_str(&format!(
                "Warning: Complex async reset for dff `{}'.\n",
                log_signal(&sig, widths)
            ));
            let (stype, clk) = sync_edge.clone().unwrap();
            gen_dffsr_complex(pm, &insig, &sig, &clk, stype == SyncType::Posedge, &async_rules, &proc_attrs, out);
            continue;
        }

        let (mut rstval, sync_level) = if async_rules.is_empty() {
            (sig_const(Bit::Sz, sig.size()), None)
        } else {
            let r = async_rules[0].clone();
            (r.0, Some((r.1, r.2)))
        };
        {
            let am = crate::sigmap::SigMap::new();
            rstval = am.map(&rstval);
            let _ = &rstval;
        }

        if !is_fully_const(&rstval) {
            // eval 面:語料觸發即 diff 紅燈(aldff 分支錨=clone 讀碼)。
            warns.push(format!(
                "Async reset value `{}' is not constant!",
                log_signal(&rstval, widths)
            ));
            out.push_str(&format!(
                "Warning: Async reset value `{}' is not constant!\n",
                log_signal(&rstval, widths)
            ));
            let (etype, eclk) = sync_edge.clone().unwrap();
            let (ltype, lsignal) = sync_level.clone().unwrap();
            gen_aldff(pm, &insig, &rstval, &sig_q, etype == SyncType::Posedge, ltype == SyncType::High, &eclk, &lsignal, &proc_attrs, out);
            continue;
        }

        let rst_const = as_const_pm(&rstval);
        let (_etype, eclk) = match &sync_edge {
            Some((t, s)) => (*t, s.clone()),
            None => (SyncType::Always, SigSpec::default()),
        };
        let arst = sync_level.as_ref().map(|(_, s)| s.clone());
        gen_dff(
            pm,
            &insig,
            &rst_const,
            &sig_q,
            sync_edge.as_ref().map(|(t, _)| *t == SyncType::Posedge).unwrap_or(false),
            sync_level.as_ref().map(|(t, _)| *t == SyncType::High).unwrap_or(false),
            &eclk,
            arst.as_ref(),
            &proc_attrs,
            out,
        );
    }
}

/// `proc_dff` 命令面(無參形)。
pub fn run_proc_dff(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), ProcError> {
    check_no_args("proc_dff", args)?;
    log.header("Executing PROC_DFF pass (convert process syncs to FFs).\n");
    let mut out = std::mem::take(&mut log.out);
    let result = (|| -> Result<(), ProcError> {
        for mi in (0..design.modules.len()).rev() {
            let widths: std::collections::HashMap<String, i64> =
                design.modules[mi].wires.iter().map(|w| (w.name.clone(), w.width)).collect();
            for pi in (0..design.modules[mi].processes.len()).rev() {
                let module = &mut design.modules[mi];
                let autoidx = &mut design.autoidx;
                let mut pm = Pm { module, autoidx };
                proc_dff_proc(&mut pm, pi, &widths, &mut out, &mut log.warnings)?;
            }
        }
        Ok(())
    })();
    log.out = out;
    log.resync();
    result
}

// ===========================================================================
// proc_memwr(passes/proc/proc_memwr.cc)
// ===========================================================================

fn proc_memwr_proc(pm: &mut Pm, pi: usize, next_port_id: &mut std::collections::HashMap<String, i64>, _out: &mut String) -> Result<(), ProcError> {
    let syncs = pm.module().processes[pi].syncs.clone();
    for (si, sr) in syncs.iter().enumerate() {
        let mut prev_port_ids: Vec<i64> = Vec::new();
        for (mi_, memwr) in sr.mem_writes.iter().enumerate() {
            let port_id = next_port_id.entry(memwr.memid.clone()).or_insert(0);
            let pid = *port_id;
            *port_id += 1;
            let mut priority_mask = vec![Bit::S0; pid as usize];
            for (i, ppid) in prev_port_ids.iter().enumerate() {
                let pm_bits: Vec<Bit> = match &memwr.priority_mask {
                    Const::Bits { bits, .. } => bits.clone(),
                    Const::Str(_) => Vec::new(),
                };
                if let Some(Bit::S1) = pm_bits.get(i) {
                    let idx = *ppid as usize;
                    if idx < priority_mask.len() {
                        priority_mask[idx] = Bit::S1;
                    }
                }
            }
            prev_port_ids.push(pid);

            let cell_name = pm.new_id_at("proc_memwr.cc", 46, "proc_memwr");
            let abits = {
                let addr = &memwr.address;
                addr.size()
            };
            let width = memwr.data.size();
            let params = vec![
                ("\\MEMID", const_str_param(&memwr.memid)),
                ("\\ABITS", Const::from_i64_i32_width(abits)),
                ("\\WIDTH", Const::from_i64_i32_width(width)),
                ("\\PORTID", Const::from_i64_i32_width(pid)),
                ("\\PRIORITY_MASK", Const::Bits { width: priority_mask.len() as i64, bits: priority_mask, signed: false, is_unsized: false }),
            ];
            // enable 的 ST0/ST1 mux 面(語料未觸;錨=proc_memwr.cc:58/61)。
            let mut enable = memwr.enable.clone();
            for sr2 in &syncs {
                if sr2.stype == SyncType::Low {
                    let id = pm.new_id_at("proc_memwr.cc", 58, "proc_memwr");
                    enable = pm.helper_mux_at(id, 3337, &sig_const(Bit::S0, enable.size()), &enable, &sr2.signal.clone().unwrap_or_default(), None);
                } else if sr2.stype == SyncType::High {
                    let id = pm.new_id_at("proc_memwr.cc", 61, "proc_memwr");
                    enable = pm.helper_mux_at(id, 3337, &enable, &sig_const(Bit::S0, enable.size()), &sr2.signal.clone().unwrap_or_default(), None);
                }
            }
            let (clk_enable, clk_polarity, clk_port): (Const, Const, SigSpec) = match sr.stype {
                SyncType::Always => (
                    Const::from_i64_i32_width(0),
                    Const::Bits { width: 1, bits: vec![Bit::Sx], signed: false, is_unsized: false },
                    SigSpec { bits: vec![SigBit::Const(Bit::Sx)] },
                ),
                SyncType::Posedge => (
                    Const::Bits { width: 1, bits: vec![Bit::S1], signed: false, is_unsized: false },
                    Const::Bits { width: 1, bits: vec![Bit::S1], signed: false, is_unsized: false },
                    sr.signal.clone().unwrap_or_default(),
                ),
                SyncType::Negedge => (
                    Const::Bits { width: 1, bits: vec![Bit::S1], signed: false, is_unsized: false },
                    Const::Bits { width: 1, bits: vec![Bit::S0], signed: false, is_unsized: false },
                    sr.signal.clone().unwrap_or_default(),
                ),
                _ => {
                    let (m, pname) = { let mm = pm.module(); let mn = mm.name.clone(); let pn = mm.processes[pi].name.clone(); (mn.strip_prefix('\\').unwrap_or(&mn).to_string(), pn.strip_prefix('\\').unwrap_or(&pn).to_string()) };
                    let _ = pname;
                    return Err(err(format!("process memory write with unsupported sync type in {m}.{pname}")));
                }
            };
            let mut params = params;
            params.push(("\\CLK_ENABLE", clk_enable));
            params.push(("\\CLK_POLARITY", clk_polarity));
            let ports = vec![
                ("\\ADDR", memwr.address.clone()),
                ("\\DATA", memwr.data.clone()),
                ("\\EN", enable),
                ("\\CLK", clk_port),
            ];
            let ci = pm.add_cell(cell_name, "$memwr_v2", params, ports, None);
            pm.module().cells[ci].attributes = memwr.attributes.clone();
            let _ = mi_;
        }
        // sr->mem_write_actions.clear()
        pm.module().processes[pi].syncs[si].mem_writes.clear();
    }
    Ok(())
}

/// `proc_memwr` 命令面(無參形)。
pub fn run_proc_memwr(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), ProcError> {
    check_no_args("proc_memwr", args)?;
    log.header("Executing PROC_MEMWR pass (convert process memory writes to cells).\n");
    let mut out = std::mem::take(&mut log.out);
    let result = (|| -> Result<(), ProcError> {
        for mi in (0..design.modules.len()).rev() {
            let mut next_port_id: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
            {
                let module = &design.modules[mi];
                for cell in module.cells.iter().rev() {
                    if matches!(cell.ctype.as_str(), "$memwr" | "$memwr_v2") {
                        let is_compat = cell.ctype == "$memwr";
                        let memid = match cell.parameters.get("\\MEMID") {
                            Some(Const::Str(s)) => s.clone(),
                            _ => continue,
                        };
                        let key = if is_compat { "\\PRIORITY" } else { "\\PORTID" };
                        let port_id = cell.parameters.get(key).map(const_as_int).unwrap_or(0);
                        let e = next_port_id.entry(memid).or_insert(0);
                        if port_id >= *e {
                            *e = port_id + 1;
                        }
                    }
                }
            }
            for pi in (0..design.modules[mi].processes.len()).rev() {
                let module = &mut design.modules[mi];
                let autoidx = &mut design.autoidx;
                let mut pm = Pm { module, autoidx };
                proc_memwr_proc(&mut pm, pi, &mut next_port_id, &mut out)?;
            }
        }
        Ok(())
    })();
    log.out = out;
    log.resync();
    result
}

// ===========================================================================
// proc 編排器(passes/proc/proc.cc;11 調用序+尾巴 opt_expr -keepdc)
// ===========================================================================

/// `proc` 命令面(無參形;旗標面=Y2,詳案 §8.3-1)。
pub fn run_proc(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), ProcError> {
    for a in &args[1..] {
        if a.starts_with('-') {
            return Err(ProcError(format!(
                "proc: option `{a}' is not implemented in ryosys (milestone Y2)"
            )));
        }
        return Err(ProcError("selection is not implemented in ryosys (milestone Y0)".to_string()));
    }
    log.header("Executing PROC pass (convert processes to netlists).\n");
    log.push();
    // Pass::post_execute(register.cc:129):每個子 pass(Pass::call)末
    // log_suppressed()——印 debug 抑制計數並清零。
    macro_rules! call {
        ($f:expr, $name:expr) => {{
            let r = $f;
            let n = dbg_take();
            if n > 0 {
                log.log(&format!("<suppressed ~{n} debug messages>\n"));
            }
            let _name = $name;
            r?;
        }};
    }
    call!(run_proc_clean(design, &[], log), "c1");
    call!(run_proc_rmdead(design, &[], log), "rm");
    call!(run_proc_prune(design, &[], log), "pr");
    call!(run_proc_init(design, &[], log), "in");
    call!(run_proc_arst(design, &[], log), "ar");
    call!(run_proc_rom(design, &[], log), "ro");
    call!(run_proc_mux(design, &[], log), "mu");
    call!(run_proc_dlatch(design, &[], log), "dl");
    call!(run_proc_dff(design, &[], log), "df");
    call!(run_proc_memwr(design, &[], log), "mw");
    call!(run_proc_clean(design, &[], log), "c2");
    call!(
        crate::opt_expr::run_opt_expr(
            design,
            &["opt_expr".to_string(), "-keepdc".to_string()],
            log
        ),
        "oe"
    );
    log.pop();
    Ok(())
}
