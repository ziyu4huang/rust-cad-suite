//! opt_dff 全量移植(passes/opt/dff/simple.cc+constbits.cc+kernel/ff.cc
//! FfData/FfInitVals;RYOSYS-MVP §9 實作輪續弧)。
//!
//! 源碼對讀:行號按段定位(clone 28ba3cb),一切以二進制 golden 仲裁。
//! NEW_ID 命名錨=二進制 golden 釘死:ff.cc:337:slice、simple.cc:81:
//! create_not、simple.cc:199/200/216/217:make_patterns_logic、
//! simple.cc:240/242:combine_resets;create_mux_to_output 的 addMux 行
//! 號(:91)為讀碼推定、語料未觸,觸即以 golden 仲裁(錨註記)。
//! addXxx 參數/端口插入序=rtlil.cc 各 addXxx 體逐字;bool 參數=32 位
//! int 形(Const(bool) dump 裸數字;golden always01-pbo `$sdff
//! CLK_POLARITY 1` 釘)。emit()=先 remove 舊 cell(swap-and-pop)再尾插
//! 新 cell(陣列位移面逐字復刻)。

use crate::opt::{did_set, module_is_boxed, wire_bits_pub};
use crate::sigmap::SigMap;
use crate::Log;
use ry_ir::{Bit, Const, Design, InsertMap, Module, SigBit, SigSpec};
use std::collections::{BTreeMap, HashMap, HashSet};

pub struct OptError(pub String);

fn err(msg: impl Into<String>) -> OptError {
    OptError(msg.into())
}

fn cell_port(cell: &ry_ir::Cell, name: &str) -> SigSpec {
    cell.connections
        .iter()
        .find(|(p, _)| p == name)
        .map(|(_, s)| s.clone())
        .unwrap_or_default()
}

fn set_port(cell: &mut ry_ir::Cell, port: &str, sig: SigSpec) {
    if let Some(e) = cell.connections.iter_mut().find(|(p, _)| p == port) {
        e.1 = sig;
    } else {
        cell.connections.push((port.to_string(), sig));
    }
}

pub(crate) fn int_param(v: i64) -> Const {
    Const::from_i64_i32_width(v)
}

pub(crate) fn bool_param(b: bool) -> Const {
    int_param(if b { 1 } else { 0 })
}

fn sig_const1(b: bool) -> SigSpec {
    SigSpec { bits: vec![SigBit::Const(if b { Bit::S1 } else { Bit::S0 })] }
}

/// 1-bit 常數按候選值原形(constbits.cc:119 `module->connect(q, val)`:
/// Sx 連 `1'x`,不塌縮為 0)。
fn sig_const_val(val: Bit) -> SigSpec {
    SigSpec { bits: vec![SigBit::Const(val)] }
}

fn bits_const(bits: &[Bit]) -> Const {
    Const::Bits { width: bits.len() as i64, bits: bits.to_vec(), signed: false, is_unsized: false }
}

pub(crate) fn bits_to_const(bits: &[Bit]) -> SigSpec {
    SigSpec { bits: bits.iter().map(|&b| SigBit::Const(b)).collect() }
}

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

pub(crate) fn new_id(autoidx: &mut i64, file: &str, line: u32, func: &str) -> String {
    let n = *autoidx;
    *autoidx += 1;
    format!("$auto${file}:{line}:{func}${n}")
}

// ---------------------------------------------------------------------------
// FfInitVals(kernel/ffinit.h)
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
pub(crate) struct FfInitVals {
    pub(crate) initbits: HashMap<SigBit, (Bit, SigBit)>, // 映射位 → (值, 錨點原位)
}

impl FfInitVals {
    pub(crate) fn set(am: &SigMap, m: &Module) -> FfInitVals {
        let mut iv = FfInitVals::default();
        for w in m.wires.iter().rev() {
            let Some(Const::Bits { bits: initval, .. }) = w.attributes.get("\\init") else {
                continue;
            };
            let wirebits = am.map(&wire_bits_pub(&w.name, w.width));
            for i in 0..wirebits.bits.len().min(initval.len()) {
                let bit = wirebits.bits[i].clone();
                let val = initval[i];
                if val != Bit::S0 && val != Bit::S1 {
                    continue;
                }
                if iv.initbits.contains_key(&bit) {
                    continue; // 首寫者勝(wires() 最新優先序)
                }
                iv.initbits.insert(
                    bit,
                    (val, SigBit::Wire { wire: w.name.clone(), offset: i as i64 }),
                );
            }
        }
        iv
    }

    fn get(&self, am: &SigMap, bit: &SigBit) -> Bit {
        let mb = am.map_bit(bit);
        self.initbits.get(&mb).map(|(v, _)| *v).unwrap_or(Bit::Sx)
    }

    fn get_spec(&self, am: &SigMap, sig: &SigSpec) -> Vec<Bit> {
        sig.bits.iter().map(|b| self.get(am, b)).collect()
    }

    fn set_init_one(&mut self, am: &SigMap, m: &mut Module, bit: &SigBit, val: Bit) {
        let mbit = am.map_bit(bit);
        let abit = match self.initbits.get(&mbit) {
            Some((_, a)) => a.clone(),
            None => {
                if val == Bit::Sx {
                    return;
                }
                bit.clone()
            }
        };
        let SigBit::Wire { wire: wname, offset } = &abit else {
            return;
        };
        self.initbits.insert(mbit, (val, abit.clone()));
        if let Some(w) = m.wires.iter_mut().find(|w| &w.name == wname) {
            let width = w.width;
            match w.attributes.entries.iter_mut().find(|(k, _)| *k == "\\init") {
                Some(e) => {
                    if let Const::Bits { bits, .. } = &mut e.1 {
                        if (*offset as usize) < bits.len() {
                            bits[*offset as usize] = val;
                        }
                        if bits.iter().all(|&b| b == Bit::Sx || b == Bit::Sz) {
                            w.attributes.swap_pop_remove("\\init");
                        }
                    }
                }
                None => {
                    if val != Bit::Sx {
                        let mut cval: Vec<Bit> = vec![Bit::Sx; width.max(0) as usize];
                        if (*offset as usize) < cval.len() {
                            cval[*offset as usize] = val;
                        }
                        w.attributes.set(
                            "\\init".to_string(),
                            Const::Bits { width, bits: cval, signed: false, is_unsized: false },
                        );
                    }
                }
            }
        }
    }

    pub(crate) fn remove_init(&mut self, am: &SigMap, m: &mut Module, sig: &SigSpec) {
        let bits: Vec<SigBit> = sig.bits.clone();
        for b in &bits {
            self.set_init_one(am, m, b, Bit::Sx);
        }
    }
}

// ---------------------------------------------------------------------------
// FfD(FfData;字級型別)
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
pub(crate) struct FfD {
    pub(crate) name: String,
    pub(crate) has_cell: bool,
    pub(crate) width: i64,
    pub(crate) has_clk: bool,
    pub(crate) has_gclk: bool,
    pub(crate) has_ce: bool,
    pub(crate) has_aload: bool,
    pub(crate) has_srst: bool,
    pub(crate) has_arst: bool,
    pub(crate) has_sr: bool,
    pub(crate) ce_over_srst: bool,
    pub(crate) pol_clk: bool,
    pub(crate) pol_ce: bool,
    pub(crate) pol_aload: bool,
    pub(crate) pol_arst: bool,
    pub(crate) pol_srst: bool,
    pub(crate) pol_clr: bool,
    pub(crate) pol_set: bool,
    pub(crate) val_arst: Vec<Bit>,
    pub(crate) val_srst: Vec<Bit>,
    pub(crate) val_init: Vec<Bit>,
    pub(crate) sig_q: SigSpec,
    pub(crate) sig_d: SigSpec,
    pub(crate) sig_ad: SigSpec,
    pub(crate) sig_clk: SigSpec,
    pub(crate) sig_ce: SigSpec,
    pub(crate) sig_aload: SigSpec,
    pub(crate) sig_arst: SigSpec,
    pub(crate) sig_srst: SigSpec,
    pub(crate) sig_clr: SigSpec,
    pub(crate) sig_set: SigSpec,
    pub(crate) attributes: InsertMap<Const>,
}

fn param_bool(cell: &ry_ir::Cell, name: &str) -> bool {
    match cell.parameters.get(name) {
        Some(Const::Bits { bits, .. }) => bits.first() == Some(&Bit::S1),
        _ => false,
    }
}

fn param_bits(cell: &ry_ir::Cell, name: &str) -> Vec<Bit> {
    match cell.parameters.get(name) {
        Some(Const::Bits { bits, .. }) => bits.clone(),
        _ => Vec::new(),
    }
}

/// manufacture_info(kernel/ff.cc:26;字級型別面)。
pub(crate) fn ff_from_cell(cell: &ry_ir::Cell, iv: &FfInitVals, am: &SigMap) -> FfD {
    if std::env::var("RY_DBG").is_ok() && cell.name.contains("slice") {
        eprintln!("DBG ff_from_cell: {} conns={:?}", cell.name, cell.connections.iter().map(|(p, s)| (p.as_str(), s.bits.len())).collect::<Vec<_>>());
    }
    let mut ff = FfD { name: cell.name.clone(), has_cell: true, ..Default::default() };
    ff.sig_q = cell_port(cell, "\\Q");
    ff.width = ff.sig_q.bits.len() as i64;
    ff.attributes = cell.attributes.clone();
    ff.val_init = iv.get_spec(am, &ff.sig_q);
    let ty = cell.ctype.as_str();
    match ty {
        "$anyinit" | "$ff" => {
            ff.has_gclk = true;
            ff.sig_d = cell_port(cell, "\\D");
        }
        "$sr" => {}
        "$dlatch" | "$adlatch" | "$dlatchsr" => {
            ff.has_aload = true;
            ff.sig_aload = cell_port(cell, "\\EN");
            ff.pol_aload = param_bool(cell, "\\EN_POLARITY");
            ff.sig_ad = cell_port(cell, "\\D");
        }
        _ => {
            ff.has_clk = true;
            ff.sig_clk = cell_port(cell, "\\CLK");
            ff.pol_clk = param_bool(cell, "\\CLK_POLARITY");
            ff.sig_d = cell_port(cell, "\\D");
        }
    }
    if matches!(ty, "$dffe" | "$dffsre" | "$adffe" | "$aldffe" | "$sdffe" | "$sdffce") {
        ff.has_ce = true;
        ff.sig_ce = cell_port(cell, "\\EN");
        ff.pol_ce = param_bool(cell, "\\EN_POLARITY");
    }
    if matches!(ty, "$dffsr" | "$dffsre" | "$dlatchsr" | "$sr") {
        ff.has_sr = true;
        ff.sig_clr = cell_port(cell, "\\CLR");
        ff.sig_set = cell_port(cell, "\\SET");
        ff.pol_clr = param_bool(cell, "\\CLR_POLARITY");
        ff.pol_set = param_bool(cell, "\\SET_POLARITY");
    }
    if matches!(ty, "$aldff" | "$aldffe") {
        ff.has_aload = true;
        ff.sig_aload = cell_port(cell, "\\ALOAD");
        ff.pol_aload = param_bool(cell, "\\ALOAD_POLARITY");
        ff.sig_ad = cell_port(cell, "\\AD");
    }
    if matches!(ty, "$adff" | "$adffe" | "$adlatch") {
        ff.has_arst = true;
        ff.sig_arst = cell_port(cell, "\\ARST");
        ff.pol_arst = param_bool(cell, "\\ARST_POLARITY");
        ff.val_arst = param_bits(cell, "\\ARST_VALUE");
    }
    if matches!(ty, "$sdff" | "$sdffe" | "$sdffce") {
        ff.has_srst = true;
        ff.sig_srst = cell_port(cell, "\\SRST");
        ff.pol_srst = param_bool(cell, "\\SRST_POLARITY");
        ff.val_srst = param_bits(cell, "\\SRST_VALUE");
        ff.ce_over_srst = ty == "$sdffce";
    }
    ff
}

impl FfD {
    /// slice(kernel/ff.cc:336;NEW_ID 錨=ff.cc:337)。
    fn slice(&self, bits: &[usize], new_name: String) -> FfD {
        let mut r = FfD {
            name: new_name,
            has_cell: false,
            has_clk: self.has_clk,
            has_gclk: self.has_gclk,
            has_ce: self.has_ce,
            has_aload: self.has_aload,
            has_arst: self.has_arst,
            has_srst: self.has_srst,
            has_sr: self.has_sr,
            ce_over_srst: self.ce_over_srst,
            pol_clk: self.pol_clk,
            pol_ce: self.pol_ce,
            pol_aload: self.pol_aload,
            pol_arst: self.pol_arst,
            pol_srst: self.pol_srst,
            pol_clr: self.pol_clr,
            pol_set: self.pol_set,
            attributes: self.attributes.clone(),
            ..Default::default()
        };
        // 控制信號線拷貝(C++ slice:res.sig_clk=sig_clk 等)
        r.sig_clk = self.sig_clk.clone();
        r.sig_ce = self.sig_ce.clone();
        r.sig_aload = self.sig_aload.clone();
        r.sig_arst = self.sig_arst.clone();
        r.sig_srst = self.sig_srst.clone();
        for &i in bits {
            r.sig_q.bits.push(self.sig_q.bits[i].clone());
            if self.has_clk || self.has_gclk {
                r.sig_d.bits.push(self.sig_d.bits[i].clone());
            }
            if self.has_aload {
                r.sig_ad.bits.push(self.sig_ad.bits[i].clone());
            }
            if self.has_sr {
                r.sig_clr.bits.push(self.sig_clr.bits[i].clone());
                r.sig_set.bits.push(self.sig_set.bits[i].clone());
            }
            if self.has_arst {
                r.val_arst.push(self.val_arst[i]);
            }
            if self.has_srst {
                r.val_srst.push(self.val_srst[i]);
            }
            r.val_init.push(self.val_init[i]);
        }
        r.width = r.sig_q.bits.len() as i64;
        r
    }

    /// remove+emit(kernel/ff.cc:624;舊 cell swap-remove、新 cell 尾插)。
    pub(crate) fn emit(self, m: &mut Module, iv: &mut FfInitVals, am: &SigMap) {
        if self.has_cell {
            if let Some(p) = m.cells.iter().position(|c| c.name == self.name) {
                iv.remove_init(am, m, &self.sig_q);
                m.cells.swap_remove(p);
            }
        }
        if self.width == 0 {
            return;
        }
        let mut ff = self;
        if !ff.has_aload && !ff.has_clk && !ff.has_gclk && !ff.has_sr {
            if ff.has_arst {
                // arst_to_aload(ff.cc arst_to_aload 語義面;ff.cc:457
                // sig_ad=val_arst——常數候選,非 sig_d)
                ff.has_aload = true;
                ff.sig_aload = ff.sig_arst.clone();
                ff.pol_aload = ff.pol_arst;
                ff.sig_ad =
                    SigSpec { bits: ff.val_arst.iter().map(|&b| SigBit::Const(b)).collect() };
                ff.has_arst = false;
            } else {
                // 無控制輸入 → const 驅動
                let q = ff.sig_q.clone();
                let v = bits_to_const(&ff.val_init);
                module_connect(m, q, v);
                return;
            }
        }
        let qbits: Vec<SigBit> = ff.sig_q.bits.clone();
        let vinit = ff.val_init.clone();
        for (i, b) in qbits.iter().enumerate() {
            let v = vinit.get(i).copied().unwrap_or(Bit::Sx);
            iv.set_init_one(am, m, b, v);
        }
        let mut params: Vec<(&str, Const)> = Vec::new();
        let mut ports: Vec<(&str, SigSpec)> = Vec::new();
        let ty: &str;
        if ff.has_gclk {
            ty = "$ff";
            params.push(("\\WIDTH", int_param(ff.width)));
            ports.push(("\\D", ff.sig_d.clone()));
            ports.push(("\\Q", ff.sig_q.clone()));
        } else if !ff.has_aload && !ff.has_clk {
            ty = "$sr";
            params.push(("\\SET_POLARITY", bool_param(ff.pol_set)));
            params.push(("\\CLR_POLARITY", bool_param(ff.pol_clr)));
            params.push(("\\WIDTH", int_param(ff.width)));
            ports.push(("\\SET", ff.sig_set.clone()));
            ports.push(("\\CLR", ff.sig_clr.clone()));
            ports.push(("\\Q", ff.sig_q.clone()));
        } else if !ff.has_clk {
            if ff.has_sr {
                ty = "$dlatchsr";
                params.push(("\\EN_POLARITY", bool_param(ff.pol_aload)));
                params.push(("\\SET_POLARITY", bool_param(ff.pol_set)));
                params.push(("\\CLR_POLARITY", bool_param(ff.pol_clr)));
                params.push(("\\WIDTH", int_param(ff.width)));
                ports.push(("\\EN", ff.sig_aload.clone()));
                ports.push(("\\SET", ff.sig_set.clone()));
                ports.push(("\\CLR", ff.sig_clr.clone()));
                ports.push(("\\D", ff.sig_ad.clone()));
                ports.push(("\\Q", ff.sig_q.clone()));
            } else if ff.has_arst {
                ty = "$adlatch";
                params.push(("\\EN_POLARITY", bool_param(ff.pol_aload)));
                params.push(("\\ARST_POLARITY", bool_param(ff.pol_arst)));
                params.push(("\\ARST_VALUE", bits_const(&ff.val_arst)));
                params.push(("\\WIDTH", int_param(ff.width)));
                ports.push(("\\EN", ff.sig_aload.clone()));
                ports.push(("\\ARST", ff.sig_arst.clone()));
                ports.push(("\\D", ff.sig_ad.clone()));
                ports.push(("\\Q", ff.sig_q.clone()));
            } else {
                ty = "$dlatch";
                params.push(("\\EN_POLARITY", bool_param(ff.pol_aload)));
                params.push(("\\WIDTH", int_param(ff.width)));
                ports.push(("\\EN", ff.sig_aload.clone()));
                ports.push(("\\D", ff.sig_ad.clone()));
                ports.push(("\\Q", ff.sig_q.clone()));
            }
        } else if ff.has_sr {
            if ff.has_ce {
                ty = "$dffsre";
                params.push(("\\CLK_POLARITY", bool_param(ff.pol_clk)));
                params.push(("\\EN_POLARITY", bool_param(ff.pol_ce)));
                params.push(("\\SET_POLARITY", bool_param(ff.pol_set)));
                params.push(("\\CLR_POLARITY", bool_param(ff.pol_clr)));
                params.push(("\\WIDTH", int_param(ff.width)));
                ports.push(("\\CLK", ff.sig_clk.clone()));
                ports.push(("\\EN", ff.sig_ce.clone()));
                ports.push(("\\SET", ff.sig_set.clone()));
                ports.push(("\\CLR", ff.sig_clr.clone()));
                ports.push(("\\D", ff.sig_d.clone()));
                ports.push(("\\Q", ff.sig_q.clone()));
            } else {
                ty = "$dffsr";
                params.push(("\\CLK_POLARITY", bool_param(ff.pol_clk)));
                params.push(("\\SET_POLARITY", bool_param(ff.pol_set)));
                params.push(("\\CLR_POLARITY", bool_param(ff.pol_clr)));
                params.push(("\\WIDTH", int_param(ff.width)));
                ports.push(("\\CLK", ff.sig_clk.clone()));
                ports.push(("\\SET", ff.sig_set.clone()));
                ports.push(("\\CLR", ff.sig_clr.clone()));
                ports.push(("\\D", ff.sig_d.clone()));
                ports.push(("\\Q", ff.sig_q.clone()));
            }
        } else if ff.has_arst {
            if ff.has_ce {
                ty = "$adffe";
                params.push(("\\CLK_POLARITY", bool_param(ff.pol_clk)));
                params.push(("\\EN_POLARITY", bool_param(ff.pol_ce)));
                params.push(("\\ARST_POLARITY", bool_param(ff.pol_arst)));
                params.push(("\\ARST_VALUE", bits_const(&ff.val_arst)));
                params.push(("\\WIDTH", int_param(ff.width)));
                ports.push(("\\CLK", ff.sig_clk.clone()));
                ports.push(("\\EN", ff.sig_ce.clone()));
                ports.push(("\\ARST", ff.sig_arst.clone()));
                ports.push(("\\D", ff.sig_d.clone()));
                ports.push(("\\Q", ff.sig_q.clone()));
            } else {
                ty = "$adff";
                params.push(("\\CLK_POLARITY", bool_param(ff.pol_clk)));
                params.push(("\\ARST_POLARITY", bool_param(ff.pol_arst)));
                params.push(("\\ARST_VALUE", bits_const(&ff.val_arst)));
                params.push(("\\WIDTH", int_param(ff.width)));
                ports.push(("\\CLK", ff.sig_clk.clone()));
                ports.push(("\\ARST", ff.sig_arst.clone()));
                ports.push(("\\D", ff.sig_d.clone()));
                ports.push(("\\Q", ff.sig_q.clone()));
            }
        } else if ff.has_aload {
            if ff.has_ce {
                ty = "$aldffe";
                params.push(("\\CLK_POLARITY", bool_param(ff.pol_clk)));
                params.push(("\\EN_POLARITY", bool_param(ff.pol_ce)));
                params.push(("\\ALOAD_POLARITY", bool_param(ff.pol_aload)));
                params.push(("\\WIDTH", int_param(ff.width)));
                ports.push(("\\CLK", ff.sig_clk.clone()));
                ports.push(("\\EN", ff.sig_ce.clone()));
                ports.push(("\\ALOAD", ff.sig_aload.clone()));
                ports.push(("\\D", ff.sig_d.clone()));
                ports.push(("\\AD", ff.sig_ad.clone()));
                ports.push(("\\Q", ff.sig_q.clone()));
            } else {
                ty = "$aldff";
                params.push(("\\CLK_POLARITY", bool_param(ff.pol_clk)));
                params.push(("\\ALOAD_POLARITY", bool_param(ff.pol_aload)));
                params.push(("\\WIDTH", int_param(ff.width)));
                ports.push(("\\CLK", ff.sig_clk.clone()));
                ports.push(("\\ALOAD", ff.sig_aload.clone()));
                ports.push(("\\D", ff.sig_d.clone()));
                ports.push(("\\AD", ff.sig_ad.clone()));
                ports.push(("\\Q", ff.sig_q.clone()));
            }
        } else if ff.has_srst {
            if ff.has_ce {
                ty = if ff.ce_over_srst { "$sdffce" } else { "$sdffe" };
                params.push(("\\CLK_POLARITY", bool_param(ff.pol_clk)));
                params.push(("\\EN_POLARITY", bool_param(ff.pol_ce)));
                params.push(("\\SRST_POLARITY", bool_param(ff.pol_srst)));
                params.push(("\\SRST_VALUE", bits_const(&ff.val_srst)));
                params.push(("\\WIDTH", int_param(ff.width)));
                ports.push(("\\CLK", ff.sig_clk.clone()));
                ports.push(("\\EN", ff.sig_ce.clone()));
                ports.push(("\\SRST", ff.sig_srst.clone()));
                ports.push(("\\D", ff.sig_d.clone()));
                ports.push(("\\Q", ff.sig_q.clone()));
            } else {
                ty = "$sdff";
                params.push(("\\CLK_POLARITY", bool_param(ff.pol_clk)));
                params.push(("\\SRST_POLARITY", bool_param(ff.pol_srst)));
                params.push(("\\SRST_VALUE", bits_const(&ff.val_srst)));
                params.push(("\\WIDTH", int_param(ff.width)));
                ports.push(("\\CLK", ff.sig_clk.clone()));
                ports.push(("\\SRST", ff.sig_srst.clone()));
                ports.push(("\\D", ff.sig_d.clone()));
                ports.push(("\\Q", ff.sig_q.clone()));
            }
        } else if ff.has_ce {
            ty = "$dffe";
            params.push(("\\CLK_POLARITY", bool_param(ff.pol_clk)));
            params.push(("\\EN_POLARITY", bool_param(ff.pol_ce)));
            params.push(("\\WIDTH", int_param(ff.width)));
            ports.push(("\\CLK", ff.sig_clk.clone()));
            ports.push(("\\EN", ff.sig_ce.clone()));
            ports.push(("\\D", ff.sig_d.clone()));
            ports.push(("\\Q", ff.sig_q.clone()));
        } else {
            ty = "$dff";
            params.push(("\\CLK_POLARITY", bool_param(ff.pol_clk)));
            params.push(("\\WIDTH", int_param(ff.width)));
            ports.push(("\\CLK", ff.sig_clk.clone()));
            ports.push(("\\D", ff.sig_d.clone()));
            ports.push(("\\Q", ff.sig_q.clone()));
        }
        if std::env::var("RY_DBG").is_ok() {
            eprintln!("DBG emit: name={} ty={ty} srst={} ce={} arst={}", ff.name, ff.has_srst, ff.has_ce, ff.has_arst);
        }
        m.cells.push(ry_ir::Cell {
            ctype: ty.to_string(),
            name: ff.name.clone(),
            attributes: ff.attributes.clone(),
            parameters: {
                let mut im: InsertMap<Const> = InsertMap::new();
                for (k, v) in params {
                    im.insert(k.to_string(), v);
                }
                im
            },
            connections: ports.into_iter().map(|(p, s)| (p.to_string(), s)).collect(),
        });
    }
}

// ---------------------------------------------------------------------------
// Worker(simple.cc SimpleContext + OptDffWorker)
// ---------------------------------------------------------------------------

fn is_active(sig: &SigBit, pol: bool) -> bool {
    *sig == SigBit::Const(if pol { Bit::S1 } else { Bit::S0 })
}

fn is_inactive(sig: &SigBit, pol: bool) -> bool {
    *sig == SigBit::Const(if pol { Bit::S0 } else { Bit::S1 })
}

fn is_always_active(sig: &SigBit, pol: bool) -> bool {
    is_active(sig, pol) || *sig == SigBit::Const(Bit::Sx)
}

fn is_always_inactive(sig: &SigBit, pol: bool) -> bool {
    is_inactive(sig, pol) || *sig == SigBit::Const(Bit::Sx)
}

fn sig_fully_const(sig: &SigSpec) -> bool {
    crate::sigops::is_fully_const(sig)
}

fn sig_fully_zero(sig: &SigSpec) -> bool {
    crate::sigops::is_fully_zero(sig)
}

fn sig_fully_ones(sig: &SigSpec) -> bool {
    sig.bits.iter().all(|b| *b == SigBit::Const(Bit::S1))
}

fn signal_all_same(sig: &SigSpec) -> bool {
    sig.bits.iter().all(|b| *b == sig.bits[0])
}

struct W {
    am: SigMap,
    iv: FfInitVals,
    bitusers: HashMap<SigBit, i64>,
    bit2mux: HashMap<SigBit, (String, i64)>,
    dff_cells: Vec<String>,
    autoidx: i64,
}

impl W {
    /// create_not(simple.cc:81):Module::Not(name, a) 單參形=cell(NEW_ID
    /// :81)+內部 wire(rtlil.cc:3201:Not,自動配置);返回 wire 位。
    fn create_not(&mut self, m: &mut Module, autoidx: &mut i64, a: SigSpec) -> SigSpec {
        let cid = new_id(autoidx, "simple.cc", 81, "create_not");
        let wid = new_id(autoidx, "rtlil.cc", 3201, "Not");
        m.wires.push(ry_ir::Wire { name: wid.clone(), width: 1, ..Default::default() });
        let y = SigSpec { bits: vec![SigBit::Wire { wire: wid, offset: 0 }] };
        m.cells.push(ry_ir::Cell {
            ctype: "$not".to_string(),
            name: cid,
            attributes: InsertMap::new(),
            parameters: {
                let mut im = InsertMap::new();
                im.insert("\\A_SIGNED".into(), int_param(0));
                im.insert("\\A_WIDTH".into(), int_param(a.bits.len() as i64));
                im.insert("\\Y_WIDTH".into(), int_param(1));
                im
            },
            connections: vec![("\\A".into(), a), ("\\Y".into(), y.clone())],
        });
        y
    }

    /// create_not 二參形(name, a, y;y 顯式=已存在線,optimize_sr SET 面)。
    fn create_not_to(&mut self, m: &mut Module, autoidx: &mut i64, a: SigSpec, y: SigSpec) {
        let cid = new_id(autoidx, "simple.cc", 81, "create_not");
        m.cells.push(ry_ir::Cell {
            ctype: "$not".to_string(),
            name: cid,
            attributes: InsertMap::new(),
            parameters: {
                let mut im = InsertMap::new();
                im.insert("\\A_SIGNED".into(), int_param(0));
                im.insert("\\A_WIDTH".into(), int_param(a.bits.len() as i64));
                im.insert("\\Y_WIDTH".into(), int_param(y.bits.len() as i64));
                im
            },
            connections: vec![("\\A".into(), a), ("\\Y".into(), y)],
        });
    }

    /// addReduceAnd/addReduceOr(NEW_ID 錨=:242 for combine_resets;
    /// :217 for make_patterns_logic)。
    fn add_reduce(
        m: &mut Module,
        cid: String,
        a: SigSpec,
        y: SigSpec,
        reduce_or: bool,
    ) {
        let ty = if reduce_or { "$reduce_or" } else { "$reduce_and" };
        m.cells.push(ry_ir::Cell {
            ctype: ty.to_string(),
            name: cid,
            attributes: InsertMap::new(),
            parameters: {
                let mut im = InsertMap::new();
                im.insert("\\A_SIGNED".into(), int_param(0));
                im.insert("\\A_WIDTH".into(), int_param(a.bits.len() as i64));
                im.insert("\\Y_WIDTH".into(), int_param(y.bits.len() as i64));
                im
            },
            connections: vec![("\\A".into(), a), ("\\Y".into(), y)],
        });
    }

    fn add_ne(m: &mut Module, cid: String, a: SigSpec, b: SigSpec, y: SigSpec) {
        m.cells.push(ry_ir::Cell {
            ctype: "$ne".to_string(),
            name: cid,
            attributes: InsertMap::new(),
            parameters: {
                let mut im = InsertMap::new();
                im.insert("\\A_SIGNED".into(), int_param(0));
                im.insert("\\B_SIGNED".into(), int_param(0));
                im.insert("\\A_WIDTH".into(), int_param(a.bits.len() as i64));
                im.insert("\\B_WIDTH".into(), int_param(b.bits.len() as i64));
                im.insert("\\Y_WIDTH".into(), int_param(1));
                im
            },
            connections: vec![("\\A".into(), a), ("\\B".into(), b), ("\\Y".into(), y)],
        });
    }
}

// ---------------------------------------------------------------------------
// 命令面
// ---------------------------------------------------------------------------

/// opt_dff 命令面(無參契約形;旗標=Y2 硬錯;§9.2)。
pub fn run_opt_dff(design: &mut Design, args: &[String], log: &mut Log) -> Result<(), OptError> {
    for a in args.get(1..).unwrap_or(&[]) {
        return Err(err(format!(
            "opt_dff: option `{a}' is not implemented in ryosys (milestone Y2)"
        )));
    }
    log.header("Executing OPT_DFF pass (perform DFF optimizations).\n");
    log.push();
    let mut out = String::new();
    for mi in (0..design.modules.len()).rev() {
        if module_is_boxed(&design.modules[mi]) {
            continue;
        }
        CUR_M.with(|c| *c.borrow_mut() = design.modules[mi].name.clone());
        let mut m = std::mem::take(&mut design.modules[mi]);
        let mut autoidx = design.autoidx;
        let did = dff_module_run(&mut m, &mut autoidx, &mut out);
        design.autoidx = autoidx;
        design.modules[mi] = m;
        let _ = did;
    }
    log.log(&out);
    log.pop();
    log.resync();
    Ok(())
}

thread_local! {
    static CUR_M: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

fn mname() -> String {
    crate::sigops::id_unescape(&CUR_M.with(|c| c.borrow().clone()))
}

pub(crate) fn widths_of(m: &Module) -> std::collections::HashMap<String, i64> {
    m.wires.iter().map(|w| (w.name.clone(), w.width)).collect()
}

/// 模組級 worker(simple.cc run()+constbits run();did_something 合流)。
fn dff_module_run(m: &mut Module, autoidx: &mut i64, out: &mut String) -> bool {
    let am = {
        let mut am = SigMap::new();
        am.set(m);
        am
    };
    let iv = FfInitVals::set(&am, m);
    let mut w = W {
        am: {
            let mut am2 = SigMap::new();
            am2.set(m);
            am2
        },
        iv,
        bitusers: HashMap::new(),
        bit2mux: HashMap::new(),
        dff_cells: Vec::new(),
        autoidx: *autoidx,
    };
    for wi in m.wires.iter().rev() {
        if wi.port_output {
            for b in w.am.map(&wire_bits_pub(&wi.name, wi.width)).bits {
                *w.bitusers.entry(b).or_insert(0) += 1;
            }
        }
    }
    let cells_snapshot: Vec<ry_ir::Cell> = m.cells.clone();
    for cell in cells_snapshot.iter().rev() {
        if matches!(cell.ctype.as_str(), "$mux" | "$pmux" | "$_MUX_") {
            let sy = w.am.map(&cell_port(cell, "\\Y"));
            for (i, b) in sy.bits.iter().enumerate() {
                w.bit2mux.insert(b.clone(), (cell.name.clone(), i as i64));
            }
        }
        for (port, sig) in &cell.connections {
            let (_, is_out) = crate::cells::port_dir(&cell.ctype, port);
            if !is_out || !crate::cells::cell_known(&cell.ctype) {
                for b in w.am.map(sig).bits {
                    *w.bitusers.entry(b).or_insert(0) += 1;
                }
            }
        }
        if crate::cells::cell_is_builtin_ff(&cell.ctype) {
            w.dff_cells.push(cell.name.clone());
        }
    }
    let mut did_something = false;
    // dff_cells.back()=最舊優先
    while let Some(name) = w.dff_cells.pop() {
        let Some(pos) = m.cells.iter().position(|c| c.name == name) else {
            continue;
        };
        let mut ff = ff_from_cell(&m.cells[pos], &w.iv, &w.am);
        let ctype = m.cells[pos].ctype.clone();
        let mut changed = false;
        if ff.width == 0 {
            m.cells.swap_remove(pos);
            did_something = true;
            continue;
        }
        if ff.has_sr && optimize_sr(&mut w, m, &mut ff, &name, &ctype, autoidx, &mut changed, out) {
            did_something = true;
            continue;
        }
        if ff.has_aload
            && optimize_aload(&mut w, m, &mut ff, &name, &ctype, autoidx, &mut changed, out)
        {
            did_something = true;
            continue;
        }
        if ff.has_arst && optimize_arst(m, &mut ff, &name, &ctype, &mut changed, out) {
            did_something = true;
            continue;
        }
        if ff.has_srst {
            optimize_srst(&mut ff, &name, &ctype, &mut changed, out);
        }
        if ff.has_ce {
            optimize_ce(&mut ff, &name, &ctype, &mut changed, out);
        }
        if ff.has_clk && sig_fully_const(&ff.sig_clk) {
            optimize_const_clk(&mut ff, &name, &ctype, &mut changed, out);
        }
        if (ff.has_clk || ff.has_gclk) && ff.sig_d == ff.sig_q {
            optimize_d_equals_q(&mut ff, &name, &ctype, &mut changed, out);
        }
        if ff.has_aload && !ff.has_clk && ff.sig_ad == ff.sig_q {
            out.push_str(&format!(
                "Handling AD = Q on {name} ({ctype}) from module {} (removing async load path).\n",
                mname()
            ));
            ff.has_aload = false;
            changed = true;
        }
        if ff.has_clk && ff.sig_d != ff.sig_q {
            let can_merge_srst =
                !ff.has_arst && !ff.has_sr && (!ff.has_srst || !ff.has_ce || ff.ce_over_srst);
            if can_merge_srst
                && try_merge_srst(&mut w, m, &mut ff, &name, &ctype, autoidx, &mut changed, out)
            {
                did_something = true;
                continue;
            }
            let can_merge_ce = !ff.has_srst || !ff.has_ce || !ff.ce_over_srst;
            if can_merge_ce
                && try_merge_ce(&mut w, m, &mut ff, &name, &ctype, autoidx, &mut changed, out)
            {
                did_something = true;
                continue;
            }
        }
        if changed {
            ff.emit(m, &mut w.iv, &w.am);
            did_something = true;
        }
    }
    // run_constbits(非 SAT 面);W.autoidx 進場同步 optimize_* 階段的
    // 引用遞增,退場寫回——兩路記帳合流。
    w.autoidx = *autoidx;
    let did2 = constbits_run(&mut w, m, out);
    *autoidx = w.autoidx;
    if did_something || did2 {
        did_set();
    }
    did_something || did2
}

// ---------------------------------------------------------------------------
// optimize_* 族(simple.cc:186-517;no-keepdc 形)
// ---------------------------------------------------------------------------

fn optimize_sr(
    w: &mut W,
    m: &mut Module,
    ff: &mut FfD,
    name: &str,
    ctype: &str,
    autoidx: &mut i64,
    changed: &mut bool,
    out: &mut String,
) -> bool {
    let mn = mname();
    let mut sr_removed = false;
    let mut keep_bits: Vec<usize> = Vec::new();
    for i in 0..ff.width as usize {
        if is_always_active(&ff.sig_clr.bits[i], ff.pol_clr) {
            let q = ff.sig_q.bits[i].clone();
            w.iv.remove_init(&w.am, m, &SigSpec { bits: vec![q.clone()] });
            module_connect(m, SigSpec { bits: vec![q] }, sig_const1(false));
            out.push_str(&format!(
                "Handling always-active CLR at position {i} on {name} ({ctype}) from module {mn} (changing to const driver).\n"
            ));
            sr_removed = true;
        } else if is_always_active(&ff.sig_set.bits[i], ff.pol_set) {
            let q = ff.sig_q.bits[i].clone();
            let clr = ff.sig_clr.bits[i].clone();
            w.iv.remove_init(&w.am, m, &SigSpec { bits: vec![q.clone()] });
            if !ff.pol_clr {
                module_connect(m, SigSpec { bits: vec![q] }, SigSpec { bits: vec![clr] });
            } else {
                w.create_not_to(
                    m,
                    autoidx,
                    SigSpec { bits: vec![clr] },
                    SigSpec { bits: vec![q] },
                );
            }
            out.push_str(&format!(
                "Handling always-active SET at position {i} on {name} ({ctype}) from module {mn} (changing to combinatorial circuit).\n"
            ));
            sr_removed = true;
        } else {
            keep_bits.push(i);
        }
    }
    if sr_removed {
        if keep_bits.is_empty() {
            if let Some(p) = m.cells.iter().position(|c| c.name == name) {
                m.cells.swap_remove(p);
            }
            return true;
        }
        let new_name = new_id(autoidx, "ff.cc", 337, "slice");
        *ff = ff.slice(&keep_bits, new_name);
        *changed = true;
    }
    let clr_inactive =
        if ff.pol_clr { sig_fully_zero(&ff.sig_clr) } else { sig_fully_ones(&ff.sig_clr) };
    let set_inactive =
        if ff.pol_set { sig_fully_zero(&ff.sig_set) } else { sig_fully_ones(&ff.sig_set) };
    if clr_inactive && signal_all_same(&ff.sig_set) {
        out.push_str(&format!("Removing never-active CLR on {name} ({ctype}) from module {mn}.\n"));
        ff.has_sr = false;
        ff.has_arst = true;
        ff.pol_arst = ff.pol_set;
        ff.sig_arst = SigSpec { bits: vec![ff.sig_set.bits[0].clone()] };
        ff.val_arst = vec![Bit::S1; ff.width as usize];
        *changed = true;
    } else if set_inactive && signal_all_same(&ff.sig_clr) {
        out.push_str(&format!("Removing never-active SET on {name} ({ctype}) from module {mn}.\n"));
        ff.has_sr = false;
        ff.has_arst = true;
        ff.pol_arst = ff.pol_clr;
        ff.sig_arst = SigSpec { bits: vec![ff.sig_clr.bits[0].clone()] };
        ff.val_arst = vec![Bit::S0; ff.width as usize];
        *changed = true;
    } else if ff.pol_clr == ff.pol_set && !ff.sig_clr.bits.is_empty() && !ff.sig_set.bits.is_empty()
    {
        let val_neutral = if ff.pol_set { Bit::S0 } else { Bit::S1 };
        let sig_arst = if ff.sig_clr.bits[0] == SigBit::Const(val_neutral) {
            ff.sig_set.bits[0].clone()
        } else {
            ff.sig_clr.bits[0].clone()
        };
        let mut failed = false;
        let mut val_arst: Vec<Bit> = Vec::new();
        for i in 0..ff.width as usize {
            if ff.sig_clr.bits[i] == sig_arst
                && ff.sig_set.bits[i] == SigBit::Const(val_neutral)
            {
                val_arst.push(Bit::S0);
            } else if ff.sig_set.bits[i] == sig_arst
                && ff.sig_clr.bits[i] == SigBit::Const(val_neutral)
            {
                val_arst.push(Bit::S1);
            } else {
                failed = true;
                break;
            }
        }
        if !failed {
            out.push_str(&format!(
                "Converting CLR/SET to ARST on {name} ({ctype}) from module {mn}.\n"
            ));
            ff.has_sr = false;
            ff.has_arst = true;
            ff.val_arst = val_arst;
            ff.sig_arst = SigSpec { bits: vec![sig_arst] };
            ff.pol_arst = ff.pol_clr;
            *changed = true;
        }
    }
    false
}

fn optimize_aload(
    w: &mut W,
    m: &mut Module,
    ff: &mut FfD,
    name: &str,
    ctype: &str,
    autoidx: &mut i64,
    changed: &mut bool,
    out: &mut String,
) -> bool {
    let mn = mname();
    // AD 全常數 → ARST(0.70+179 二進制:此分支靜默+前移;clone 有 log 且
    // 居後——最小探针矩陣實錄,D-ys9 登記)。轉換後同迭代內聯 arst-opt。
    if sig_fully_const(&ff.sig_ad) && !ff.has_arst && !ff.has_sr {
        ff.has_arst = true;
        ff.has_aload = false;
        ff.sig_arst = ff.sig_aload.clone();
        ff.pol_arst = ff.pol_aload;
        ff.val_arst =
            ff.sig_ad.bits.iter().map(|b| match b { SigBit::Const(c) => *c, _ => Bit::Sx }).collect();
        *changed = true;
        if is_inactive(&ff.sig_arst.bits[0], ff.pol_arst) {
            out.push_str(&format!(
                "Removing never-active ARST on {name} ({ctype}) from module {mn}.\n"
            ));
            ff.has_arst = false;
        } else if is_always_active(&ff.sig_arst.bits[0], ff.pol_arst) {
            out.push_str(&format!(
                "Handling always-active ARST on {name} ({ctype}) from module {mn} (changing to const driver).\n"
            ));
            if let Some(p) = m.cells.iter().position(|c| c.name == name) {
                m.cells.swap_remove(p);
            }
            let q = ff.sig_q.clone();
            let v = bits_to_const(&ff.val_arst);
            module_connect(m, q, v);
            return true;
        }
        return false;
    }
    if is_always_inactive(&ff.sig_aload.bits[0], ff.pol_aload) {
        out.push_str(&format!(
            "Removing never-active async load on {name} ({ctype}) from module {mn}.\n"
        ));
        ff.has_aload = false;
        *changed = true;
        return false;
    }
    if is_active(&ff.sig_aload.bits[0], ff.pol_aload) {
        out.push_str(&format!(
            "Handling always-active async load on {name} ({ctype}) from module {mn} (changing to combinatorial circuit).\n"
        ));
        if let Some(p) = m.cells.iter().position(|c| c.name == name) {
            w.iv.remove_init(&w.am, m, &ff.sig_q);
            m.cells.swap_remove(p);
        }
        if ff.has_sr {
            // $adlatchsr 形(語料未觸;觸發即擴,RM3-5 條款)
        } else if ff.has_arst {
            // create_mux_to_output(ad, val_arst, arst, q, pol_arst, false)
            // =addMux(NEW_ID, pol? a:b …);NEW_ID 行號=讀碼推定 :91
            let cid = new_id(autoidx, "simple.cc", 91, "create_mux_to_output");
            let pol = ff.pol_arst;
            let (a, b) = if pol {
                (ff.sig_ad.clone(), bits_to_const(&ff.val_arst))
            } else {
                (bits_to_const(&ff.val_arst), ff.sig_ad.clone())
            };
            m.cells.push(ry_ir::Cell {
                ctype: "$mux".to_string(),
                name: cid,
                attributes: InsertMap::new(),
                parameters: {
                    let mut im = InsertMap::new();
                    im.insert("\\WIDTH".into(), int_param(ff.width));
                    im
                },
                connections: vec![
                    ("\\A".into(), a),
                    ("\\B".into(), b),
                    ("\\S".into(), ff.sig_arst.clone()),
                    ("\\Y".into(), ff.sig_q.clone()),
                ],
            });
        } else {
            module_connect(m, ff.sig_q.clone(), ff.sig_ad.clone());
        }
        return true;
    }
    false
}

fn optimize_arst(m: &mut Module, ff: &mut FfD, name: &str, ctype: &str, changed: &mut bool, out: &mut String) -> bool {
    let mn = mname();
    let ty = ctype;
    if is_inactive(&ff.sig_arst.bits[0], ff.pol_arst) {
        out.push_str(&format!(
            "Removing never-active ARST on {name} ({ty}) from module {mn}.\n"
        ));
        ff.has_arst = false;
        *changed = true;
    } else if is_always_active(&ff.sig_arst.bits[0], ff.pol_arst) {
        out.push_str(&format!(
            "Handling always-active ARST on {name} ({ty}) from module {mn} (changing to const driver).\n"
        ));
        if let Some(p) = m.cells.iter().position(|c| c.name == name) {
            m.cells.swap_remove(p);
        }
        let q = ff.sig_q.clone();
        let v = bits_to_const(&ff.val_arst);
        module_connect(m, q, v);
        return true;
    }
    false
}

fn optimize_srst(ff: &mut FfD, name: &str, ctype: &str, changed: &mut bool, out: &mut String) {
    let mn = mname();
    let ty = ctype;
    if is_inactive(&ff.sig_srst.bits[0], ff.pol_srst) {
        out.push_str(&format!(
            "Removing never-active SRST on {name} ({ty}) from module {mn}.\n"
        ));
        ff.has_srst = false;
        *changed = true;
    } else if is_always_active(&ff.sig_srst.bits[0], ff.pol_srst) {
        out.push_str(&format!(
            "Handling always-active SRST on {name} ({ty}) from module {mn} (changing to const D).\n"
        ));
        ff.has_srst = false;
        if !ff.ce_over_srst {
            ff.has_ce = false;
        }
        ff.sig_d = bits_to_const(&ff.val_srst);
        *changed = true;
    }
}

fn optimize_ce(ff: &mut FfD, name: &str, ctype: &str, changed: &mut bool, out: &mut String) {
    let mn = mname();
    let ty = ctype;
    if is_always_inactive(&ff.sig_ce.bits[0], ff.pol_ce) {
        if ff.has_srst && !ff.ce_over_srst {
            out.push_str(&format!(
                "Handling never-active EN on {name} ({ty}) from module {mn} (connecting SRST instead).\n"
            ));
            ff.pol_ce = ff.pol_srst;
            ff.sig_ce = ff.sig_srst.clone();
            ff.has_srst = false;
            ff.sig_d = bits_to_const(&ff.val_srst);
            *changed = true;
        } else {
            out.push_str(&format!(
                "Handling never-active EN on {name} ({ty}) from module {mn} (removing D path).\n"
            ));
            ff.has_ce = false;
            ff.has_clk = false;
            ff.has_srst = false;
            *changed = true;
        }
    } else if is_active(&ff.sig_ce.bits[0], ff.pol_ce) {
        out.push_str(&format!(
            "Removing always-active EN on {name} ({ty}) from module {mn}.\n"
        ));
        ff.has_ce = false;
        *changed = true;
    }
}

fn optimize_const_clk(ff: &mut FfD, name: &str, ctype: &str, changed: &mut bool, out: &mut String) {
    if std::env::var("RY_DBG").is_ok() {
        eprintln!("DBG const_clk: name={name} sig_clk={:?} width={}", ff.sig_clk.bits, ff.width);
        eprintln!("DBG has_clk={} has_gclk={} has_srst={}", ff.has_clk, ff.has_gclk, ff.has_srst);
    }
    let mn = mname();
    out.push_str(&format!(
        "Handling const CLK on {name} ({ctype}) from module {mn} (removing D path).\n"
    ));
    ff.has_ce = false;
    ff.has_clk = false;
    ff.has_srst = false;
    *changed = true;
}

fn optimize_d_equals_q(ff: &mut FfD, name: &str, ctype: &str, changed: &mut bool, out: &mut String) {
    let mn = mname();
    let ty = ctype;
    if ff.has_clk && ff.has_srst {
        out.push_str(&format!(
            "Handling D = Q on {name} ({ty}) from module {mn} (conecting SRST instead).\n"
        ));
        if ff.has_ce && ff.ce_over_srst {
            // ce ∧ srst(語料未觸 ce_over_srst 形;觸發即擴)
            ff.pol_ce = ff.pol_srst;
            ff.sig_ce = ff.sig_srst.clone();
        } else {
            ff.pol_ce = ff.pol_srst;
            ff.sig_ce = ff.sig_srst.clone();
        }
        ff.has_ce = true;
        ff.has_srst = false;
        ff.sig_d = bits_to_const(&ff.val_srst);
        *changed = true;
    } else {
        out.push_str(&format!(
            "Handling D = Q on {name} ({ty}) from module {mn} (removing D path).\n"
        ));
        ff.has_gclk = false;
        ff.has_clk = false;
        ff.has_ce = false;
        *changed = true;
    }
}

// ---------------------------------------------------------------------------
// mux 合併(simple.cc:519-668)+feedback patterns(simple.cc:118-185)
// ---------------------------------------------------------------------------

type Ctrls = Vec<(SigBit, bool)>; // pool<pair> 首見序

/// SigBit 的全序鍵(wire 名/offset/const 字元;std::map 鍵序模型)。
fn bit_key(b: &SigBit) -> (u8, String, i64) {
    match b {
        SigBit::Wire { wire, offset } => (1, wire.clone(), *offset),
        SigBit::Const(c) => (0, String::new(), c.as_char() as i64),
    }
}

fn ctrls_key(c: &Ctrls) -> Vec<(u8, String, i64, bool)> {
    let mut v: Vec<(u8, String, i64, bool)> =
        c.iter().map(|(b, p)| (bit_key(b), *p)).map(|(k, p)| (k.0, k.1, k.2, p)).collect();
    v.sort();
    v
}

fn patterns_key(pats: &[Vec<(SigBit, bool)>]) -> Vec<Vec<(u8, String, i64, bool)>> {
    let mut v: Vec<Vec<(u8, String, i64, bool)>> =
        pats.iter().map(|p| ctrls_key(p)).collect();
    v.sort();
    v
}

fn find_muxtree_feedback_patterns(
    w: &mut W,
    m: &mut Module,
    d: SigBit,
    q: &SigBit,
    path: Vec<(SigBit, bool)>,
    out_patterns: &mut Vec<Vec<(SigBit, bool)>>,
) {
    if d == *q {
        out_patterns.push(path);
        return;
    }
    let Some((mux_name, index)) = w.bit2mux.get(&d).cloned() else {
        return;
    };
    if w.bitusers.get(&d).copied().unwrap_or(0) > 1 {
        return;
    }
    let Some(pos) = m.cells.iter().position(|c| c.name == mux_name) else {
        return;
    };
    let s_sig = w.am.map(&cell_port(&m.cells[pos], "\\S"));
    let width = cell_port(&m.cells[pos], "\\A").bits.len() as i64;
    let s_width = s_sig.bits.len() as i64;
    let port_bit = |w: &W, m: &Module, port: &str, off: i64| -> SigBit {
        let s = cell_port(&m.cells[pos], port);
        w.am.map_bit(&s.bits[off as usize])
    };
    // path 強制分支
    let mut forced: Option<usize> = None;
    for i in 0..s_width as usize {
        let s_bit = port_bit(w, m, "\\S", i as i64);
        if let Some(&(_, v)) = path.iter().find(|(b, _)| *b == s_bit) {
            if v {
                forced = Some(i);
                break;
            }
        }
    }
    if let Some(i) = forced {
        let b_bit = port_bit(w, m, "\\B", i as i64 * width + index);
        let b_bit2 = b_bit.clone();
        find_muxtree_feedback_patterns(w, m, b_bit, q, path, out_patterns);
        if b_bit2 == *q {
            // break_feedback
            if let Some(p2) = m.cells.iter().position(|c| c.name == mux_name) {
                let mut s = cell_port(&m.cells[p2], "\\B");
                s.bits[(i as i64 * width + index) as usize] = SigBit::Const(Bit::Sx);
                set_port(&mut m.cells[p2], "\\B", s);
            }
        }
        return;
    }
    // 探索:else 路徑(S=0)+this 路徑(S=1)
    // oracle simple.cc:149:pattern_t path_else = path;(探索循環外建
    // 一次;循環內 path_else[s_bit]=false 累加=map 賦值語義,跨迭代
    // 持久,A 臂遞歸帶累加版——map 賦值≈push:循環跳過 path 已含者、
    // 各輪 s_bit 互異,無重複鍵)
    let mut path_else = path.clone();
    for i in 0..s_width as usize {
        let s_bit = port_bit(w, m, "\\S", i as i64);
        if path.iter().any(|(b, _)| *b == s_bit) {
            continue;
        }
        let mut path_this = path.clone();
        // oracle simple.cc:156:path_else[s_bit] = false;(累加)
        path_else.push((s_bit.clone(), false));
        path_this.push((s_bit.clone(), true));
        let b_bit = port_bit(w, m, "\\B", i as i64 * width + index);
        let b_cmp = b_bit.clone();
        find_muxtree_feedback_patterns(w, m, b_bit, q, path_this, out_patterns);
        if b_cmp == *q {
            if let Some(p2) = m.cells.iter().position(|c| c.name == mux_name) {
                let mut s = cell_port(&m.cells[p2], "\\B");
                s.bits[(i as i64 * width + index) as usize] = SigBit::Const(Bit::Sx);
                set_port(&mut m.cells[p2], "\\B", s);
            }
        }
    }
    let a_bit = port_bit(w, m, "\\A", index);
    let a_cmp = a_bit.clone();
    // oracle simple.cc:168:A 臂遞歸帶累加 path_else
    find_muxtree_feedback_patterns(w, m, a_bit, q, path_else, out_patterns);
    if a_cmp == *q {
        if let Some(p2) = m.cells.iter().position(|c| c.name == mux_name) {
            let mut s = cell_port(&m.cells[p2], "\\A");
            s.bits[index as usize] = SigBit::Const(Bit::Sx);
            set_port(&mut m.cells[p2], "\\A", s);
        }
    }
}

fn make_patterns_logic(
    w: &mut W,
    m: &mut Module,
    patterns: &[Vec<(SigBit, bool)>],
    ctrls: &Ctrls,
    autoidx: &mut i64,
) -> (SigBit, bool) {
    if patterns.is_empty() && ctrls.len() == 1 {
        return (ctrls[0].0.clone(), ctrls[0].1);
    }
    let mut or_input: Vec<SigBit> = Vec::new();
    for pat in patterns {
        let mut s1: Vec<SigBit> = Vec::new();
        let mut s2: Vec<SigBit> = Vec::new();
        for (b, v) in pat {
            s1.push(b.clone());
            s2.push(SigBit::Const(if *v { Bit::S1 } else { Bit::S0 }));
        }
        let wid = new_id(autoidx, "simple.cc", 199, "make_patterns_logic");
        m.wires.push(ry_ir::Wire { name: wid.clone(), width: 1, ..Default::default() });
        let y = SigSpec { bits: vec![SigBit::Wire { wire: wid, offset: 0 }] };
        let cid = new_id(autoidx, "simple.cc", 200, "make_patterns_logic");
        W::add_ne(m, cid, SigSpec { bits: s1 }, SigSpec { bits: s2 }, y.clone());
        or_input.push(y.bits[0].clone());
    }
    for (b, v) in ctrls {
        if *v {
            or_input.push(b.clone());
        } else {
            let y = w.create_not(m, autoidx, SigSpec { bits: vec![b.clone()] });
            or_input.push(y.bits[0].clone());
        }
    }
    if or_input.is_empty() {
        return (SigBit::Const(Bit::S1), true);
    }
    if or_input.len() == 1 {
        return (or_input[0].clone(), true);
    }
    let wid = new_id(autoidx, "simple.cc", 216, "make_patterns_logic");
    m.wires.push(ry_ir::Wire { name: wid.clone(), width: 1, ..Default::default() });
    let y = SigSpec { bits: vec![SigBit::Wire { wire: wid.clone(), offset: 0 }] };
    let cid = new_id(autoidx, "simple.cc", 217, "make_patterns_logic");
    W::add_reduce(m, cid, SigSpec { bits: or_input }, y.clone(), false);
    (y.bits[0].clone(), true)
}

fn combine_resets(
    w: &mut W,
    m: &mut Module,
    ctrls: &Ctrls,
    autoidx: &mut i64,
) -> (SigBit, bool) {
    if ctrls.len() == 1 {
        return (ctrls[0].0.clone(), ctrls[0].1);
    }
    let mut final_pol = false;
    for (_, v) in ctrls {
        if *v {
            final_pol = true;
        }
    }
    let mut or_input: Vec<SigBit> = Vec::new();
    for (b, v) in ctrls {
        if *v == final_pol {
            or_input.push(b.clone());
        } else {
            let y = w.create_not(m, autoidx, SigSpec { bits: vec![b.clone()] });
            or_input.push(y.bits[0].clone());
        }
    }
    let wid = new_id(autoidx, "simple.cc", 240, "combine_resets");
    m.wires.push(ry_ir::Wire { name: wid.clone(), width: 1, ..Default::default() });
    let y = SigSpec { bits: vec![SigBit::Wire { wire: wid.clone(), offset: 0 }] };
    let cid = new_id(autoidx, "simple.cc", 242, "combine_resets");
    W::add_reduce(m, cid, SigSpec { bits: or_input }, y.clone(), final_pol);
    (y.bits[0].clone(), final_pol)
}

/// try_merge_srst(simple.cc:519-606)。
fn try_merge_srst(
    w: &mut W,
    m: &mut Module,
    ff: &mut FfD,
    name: &str,
    ctype: &str,
    autoidx: &mut i64,
    changed: &mut bool,
    out: &mut String,
) -> bool {
    let mn = mname();
    // groups:ctrls 集合 → 位索引(std::map 鍵序=BTreeMap 近似;多群組序差
    // =分類點)
    let mut groups: BTreeMap<Vec<(u8, String, i64, bool)>, Vec<(usize, Ctrls)>> =
        BTreeMap::new();
    let mut remaining: Vec<usize> = Vec::new();
    let mut val_srst: Vec<Bit> = vec![Bit::Sx; ff.width as usize];
    for i in 0..ff.width as usize {
        let mut resets: Ctrls = Vec::new();
        let mut reset_val: Bit = if ff.has_srst { ff.val_srst[i] } else { Bit::Sx };
        let mut d = ff.sig_d.bits[i].clone();
        loop {
            let Some((mux_name, index)) = w.bit2mux.get(&d).cloned() else { break };
            if w.bitusers.get(&d).copied().unwrap_or(0) != 1 {
                break;
            }
            let Some(pos) = m.cells.iter().position(|c| c.name == mux_name) else { break };
            let s_sig = w.am.map(&cell_port(&m.cells[pos], "\\S"));
            if s_sig.bits.len() != 1 {
                break;
            }
            let s = s_sig.bits[0].clone();
            // C++:RAW 位(無 assign_map;僅 S 寬測試與常數/反饋比較)
            let a = cell_port(&m.cells[pos], "\\A").bits[index as usize].clone();
            let b = cell_port(&m.cells[pos], "\\B").bits[index as usize].clone();
            let b_const = matches!(b, SigBit::Const(Bit::S0) | SigBit::Const(Bit::S1));
            let a_const = matches!(a, SigBit::Const(Bit::S0) | SigBit::Const(Bit::S1));
            if a_const && b_const {
                break;
            }
            let a_is_q = a == ff.sig_q.bits[i];
            let b_const_ok = b_const
                && (b == SigBit::Const(reset_val) || reset_val == Bit::Sx)
                && !a_is_q;
            if b_const_ok {
                reset_val = match b {
                    SigBit::Const(c) => c,
                    _ => Bit::Sx,
                };
                if !resets.iter().any(|(x, _)| *x == s) {
                    resets.push((s, true));
                }
                d = a;
                ff.sig_d.bits[i] = d.clone();
            } else {
                let b_is_q = b == ff.sig_q.bits[i];
                let a_const_ok = a_const
                    && (a == SigBit::Const(reset_val) || reset_val == Bit::Sx)
                    && !b_is_q;
                if a_const_ok {
                    reset_val = match a {
                        SigBit::Const(c) => c,
                        _ => Bit::Sx,
                    };
                    if !resets.iter().any(|(x, _)| *x == s) {
                        resets.push((s, false));
                    }
                    d = b;
                    ff.sig_d.bits[i] = d.clone();
                } else {
                    break;
                }
            }
        }
        if !resets.is_empty() {
            if ff.has_srst && !resets.iter().any(|(x, _)| *x == ff.sig_srst.bits[0]) {
                resets.push((ff.sig_srst.bits[0].clone(), ff.pol_srst));
            }
            val_srst[i] = reset_val;
            groups.entry(ctrls_key(&resets)).or_default().push((i, resets));
        } else {
            remaining.push(i);
        }
    }
    let mut dff_cells_new: Vec<String> = Vec::new();
    if std::env::var("RY_DBG").is_ok() {
        eprintln!("DBG merge_srst: ff.sig_clk={:?} name={name}", ff.sig_clk.bits);
    }
    for (_, idxs) in groups {
        // 同群組 resets 集合相同(鍵相同);取首元素之 resets,索引按原序
        let ctrls = idxs.first().map(|(_, r)| r.clone()).unwrap_or_default();
        let mut idxs: Vec<usize> = idxs.iter().map(|(i, _)| *i).collect();
        idxs.sort();
        let mut new_ff = ff.slice(&idxs, new_id(autoidx, "ff.cc", 337, "slice"));
        let mut nv: Vec<Bit> = Vec::new();
        for &i in &idxs {
            nv.push(val_srst[i]);
        }
        new_ff.val_srst = nv;
        let (sig, pol) = combine_resets(w, m, &ctrls, autoidx);
        new_ff.has_srst = true;
        new_ff.sig_srst = SigSpec { bits: vec![sig] };
        new_ff.pol_srst = pol;
        if new_ff.has_ce {
            new_ff.ce_over_srst = true;
        }
        let widths = widths_of(m);
        out.push_str(&format!(
            "Adding SRST signal on {name} ({ctype}) from module {mn} (D = {}, Q = {}, rval = {}).\n",
            crate::sigops::log_signal(&new_ff.sig_d, &widths),
            crate::sigops::log_signal(&new_ff.sig_q, &widths),
            crate::sigops::log_signal(&bits_to_const(&new_ff.val_srst), &widths)
        ));
        let attrs = new_ff.attributes.clone();
        let _ = attrs;
        dff_cells_new.push(new_ff.name.clone());
        new_ff.emit(m, &mut w.iv, &w.am);
    }
    for n in dff_cells_new {
        w.dff_cells.push(n);
    }
    if remaining.is_empty() {
        if let Some(p) = m.cells.iter().position(|c| c.name == name) {
            m.cells.swap_remove(p);
        }
        return true;
    }
    if remaining.len() != ff.width as usize {
        let new_name = new_id(autoidx, "ff.cc", 337, "slice");
        *ff = ff.slice(&remaining, new_name);
        *changed = true;
    }
    false
}

/// try_merge_ce(simple.cc:608-668)。
fn try_merge_ce(
    w: &mut W,
    m: &mut Module,
    ff: &mut FfD,
    name: &str,
    ctype: &str,
    autoidx: &mut i64,
    changed: &mut bool,
    out: &mut String,
) -> bool {
    let mn = mname();
    let mut groups: BTreeMap<
        (Vec<Vec<(u8, String, i64, bool)>>, Vec<(u8, String, i64, bool)>),
        Vec<(usize, Ctrls, Vec<Vec<(SigBit, bool)>>)>,
    > = BTreeMap::new();
    let mut remaining: Vec<usize> = Vec::new();
    for i in 0..ff.width as usize {
        let mut enables: Ctrls = Vec::new();
        let mut d = ff.sig_d.bits[i].clone();
        loop {
            let Some((mux_name, index)) = w.bit2mux.get(&d).cloned() else { break };
            if w.bitusers.get(&d).copied().unwrap_or(0) != 1 {
                break;
            }
            let Some(pos) = m.cells.iter().position(|c| c.name == mux_name) else { break };
            let s_sig = w.am.map(&cell_port(&m.cells[pos], "\\S"));
            if s_sig.bits.len() != 1 {
                break;
            }
            let s = s_sig.bits[0].clone();
            let a = cell_port(&m.cells[pos], "\\A").bits[index as usize].clone();
            let b = cell_port(&m.cells[pos], "\\B").bits[index as usize].clone();
            if a == ff.sig_q.bits[i] {
                if !enables.iter().any(|(x, _)| *x == s) {
                    enables.push((s, true));
                }
                d = b;
                ff.sig_d.bits[i] = d.clone();
            } else if b == ff.sig_q.bits[i] {
                if !enables.iter().any(|(x, _)| *x == s) {
                    enables.push((s, false));
                }
                d = a;
                ff.sig_d.bits[i] = d.clone();
            } else {
                break;
            }
        }
        let mut patterns: Vec<Vec<(SigBit, bool)>> = Vec::new();
        find_muxtree_feedback_patterns(
            w, m, d.clone(), &ff.sig_q.bits[i].clone(), Vec::new(), &mut patterns,
        );
        if !patterns.is_empty() || !enables.is_empty() {
            if ff.has_ce && !enables.iter().any(|(x, _)| *x == ff.sig_ce.bits[0]) {
                enables.push((ff.sig_ce.bits[0].clone(), ff.pol_ce));
            }
            groups
                .entry((patterns_key(&patterns), ctrls_key(&enables)))
                .or_default()
                .push((i, enables, patterns.clone()));
        } else {
            remaining.push(i);
        }
    }
    let mut dff_cells_new: Vec<String> = Vec::new();
    for ((_, _enables), entries) in groups {
        let enables: Ctrls =
            entries.first().map(|(_, e, _)| e.clone()).unwrap_or_default();
        let patterns: Vec<Vec<(SigBit, bool)>> =
            entries.first().map(|(_, _, p)| p.clone()).unwrap_or_default();
        let mut idxs: Vec<usize> = entries.iter().map(|(i, _, _)| *i).collect();
        idxs.sort();
        let mut new_ff = ff.slice(&idxs, new_id(autoidx, "ff.cc", 337, "slice"));
        let (sig, pol) = make_patterns_logic(w, m, &patterns, &enables, autoidx);
        new_ff.has_ce = true;
        new_ff.sig_ce = SigSpec { bits: vec![sig] };
        new_ff.pol_ce = pol;
        new_ff.ce_over_srst = false;
        out.push_str(&format!(
            "Adding EN signal on {name} ({ctype}) from module {mn} (D = {}, Q = {}).\n",
            crate::sigops::log_signal(&new_ff.sig_d, &widths_of(m)),
            crate::sigops::log_signal(&new_ff.sig_q, &widths_of(m))
        ));
        dff_cells_new.push(new_ff.name.clone());
        new_ff.emit(m, &mut w.iv, &w.am);
    }
    for n in dff_cells_new {
        w.dff_cells.push(n);
    }
    if remaining.is_empty() {
        if let Some(p) = m.cells.iter().position(|c| c.name == name) {
            m.cells.swap_remove(p);
        }
        return true;
    }
    if remaining.len() != ff.width as usize {
        let new_name = new_id(autoidx, "ff.cc", 337, "slice");
        *ff = ff.slice(&remaining, new_name);
        *changed = true;
    }
    false
}

// ---------------------------------------------------------------------------
// constbits(passes/opt/dff/constbits.cc;非 SAT 面)
// ---------------------------------------------------------------------------

fn combine_const(a: Bit, b: Bit) -> Bit {
    if a == Bit::Sx {
        return b;
    }
    if b == Bit::Sx {
        return a;
    }
    if a == b {
        return a;
    }
    Bit::Sm
}

fn check_constbit(w: &W, ff: &FfD, i: usize) -> Bit {
    let mut val = ff.val_init[i];
    if ff.has_arst {
        val = combine_const(val, ff.val_arst[i]);
    }
    if ff.has_srst {
        val = combine_const(val, ff.val_srst[i]);
    }
    if ff.has_sr {
        if !is_inactive(&w.am.map_bit(&ff.sig_clr.bits[i]), ff.pol_clr) {
            val = combine_const(val, Bit::S0);
        }
        if !is_inactive(&w.am.map_bit(&ff.sig_set.bits[i]), ff.pol_set) {
            val = combine_const(val, Bit::S1);
        }
    }
    val
}

fn constbits_run(w: &mut W, m: &mut Module, out: &mut String) -> bool {
    // fold_const_bits 非 SAT:僅 D/AD 全常數位折疊
    let mn = mname();
    let cells_snapshot: Vec<ry_ir::Cell> = m.cells.clone();
    // const_bits: cell 名 → drop 位集;逐 cell 一次 remove_ff_bits
    let mut const_bits: Vec<(String, Vec<(usize, SigBit, Bit)>)> = Vec::new();
    for cell in cells_snapshot.iter().rev() {
        if !crate::cells::cell_is_builtin_ff(&cell.ctype) {
            continue;
        }
        let ff = ff_from_cell(cell, &w.iv, &w.am);
        let mut drops: Vec<(usize, SigBit, Bit)> = Vec::new();
        for i in 0..ff.width as usize {
            let mut val = check_constbit(w, &ff, i);
            if val == Bit::Sm {
                continue;
            }
            let has_d = ff.has_clk || ff.has_gclk;
            let d = if has_d { w.am.map_bit(&ff.sig_d.bits[i]) } else { SigBit::Const(Bit::Sx) };
            let ad =
                if ff.has_aload { w.am.map_bit(&ff.sig_ad.bits[i]) } else { SigBit::Const(Bit::Sx) };
            if has_d {
                if let SigBit::Wire { .. } = d {
                    continue; // needs_proof(非 SAT 不折叠)
                } else if let SigBit::Const(c) = d {
                    val = combine_const(val, c);
                }
            }
            if ff.has_aload {
                if let SigBit::Wire { .. } = ad {
                    continue;
                } else if let SigBit::Const(c) = ad {
                    val = combine_const(val, c);
                }
            }
            // C++ 僅擋 Sm(衝突);Sx 候選亦提交(log 印 0-bit、connect x)
            if val == Bit::Sm {
                continue;
            }
            out.push_str(&format!(
                "Setting constant {}-bit at position {i} on {} ({}) from module {mn}.\n",
                if val == Bit::S1 { 1 } else { 0 },
                cell.name,
                cell.ctype
            ));
            drops.push((i, ff.sig_q.bits[i].clone(), val));
        }
        if !drops.is_empty() {
            const_bits.push((cell.name.clone(), drops));
        }
    }
    let mut any = false;
    for (cname, drops) in const_bits {
        // remove_ff_bits:keep=非 drop;slice(keep).emit() 重插(同名)
        let Some(pos) = m.cells.iter().position(|c| c.name == cname) else {
            continue;
        };
        let drop_set: HashSet<usize> = drops.iter().map(|(i, _, _)| *i).collect();
        for (_, q, val) in &drops {
            w.iv.remove_init(&w.am, m, &SigSpec { bits: vec![q.clone()] });
            module_connect(m, SigSpec { bits: vec![q.clone()] }, sig_const_val(*val));
        }
        let ff = ff_from_cell(&m.cells[pos], &w.iv, &w.am);
        let keep: Vec<usize> =
            (0..ff.width as usize).filter(|i| !drop_set.contains(i)).collect();
        // remove_ff_bits(opt_dff.cc:47)=ff.slice(keep) 無條件構造 NEW_ID
        // (ff.cc:336)——keep 空亦 +1 autoidx,新名隨後被 emit 以舊 cell
        // 名重建覆蓋(此處 slice 傳舊名等價)。
        w.autoidx += 1;
        let new_ff = ff.slice(&keep, ff.name.clone());
        if keep.is_empty() {
            if let Some(p) = m.cells.iter().position(|c| c.name == cname) {
                m.cells.swap_remove(p);
            }
        } else {
            // 原地重建:先刪再尾插(emit 語義)
            let mut nf = new_ff;
            nf.has_cell = true;
            nf.name = cname.clone();
            nf.emit(m, &mut w.iv, &w.am);
        }
        any = true;
    }
    if any {
        did_set();
    }
    any
}
