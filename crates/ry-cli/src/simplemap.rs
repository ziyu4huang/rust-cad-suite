//! `simplemap` pass M1-d(passes/techmap/simplemap.cc + kernel/ff.cc 移植面)。
//!
//! 覆蓋:simplemap.h 的 13 個 mapper 函數族 + simplemap.cc 的 45 條
//! dispatch 表(clone 實數 44 條+binary help 摘要;二進制行為=表外 cell
//! `continue` 靜默跳過)+ ff 家族 FfData(kernel/ff.cc)粗→fine 的
//! slice/emit。每映射一格打一行 `Mapping <mod>.<cell> (<type>).`。
//!
//! 實測釘死(2026-10-27,oracle 0.70+179 二進制;87 例 P-free 語料
//! `read_rtlil; simplemap; check; stat; write_rtlil -readable; write_json`
//! 全量普查 exit 87/87=0、stderr 87/87 空、1392 條 Mapping 行):
//! * `Mapping` 行格式=`Mapping %s.%s (%s).`——mod/cell/type 皆 unescape
//!   (剝 `\` 前綴;`$` 開頭原樣),`Mapping uut_always01.$auto$ff.cc:337:
//!   slice$5 ($sdff).` 實錄;
//! * NEW_ID=`$auto$<basename>:<line>:<func>$<autoidx++>`(yosys_common.h
//!   :303 的 create_id_prefix 剝目錄取 basename);**行號錨以二進制
//!   golden 為準**——普查全量核對 16 個 `$auto$simplemap.cc:<L>…` 錨
//!   與 clone simplemap.cc **零漂移**;ff.cc 的 slice NEW_ID=二進制
//!   :337(clone 亦 :337);未覆蓋 mapper(tribuf/bwmux/bmux/lut/sop)
//!   行號取 clone(語料未觸發,無差分錨,PROVENANCE 登記);
//! * **hashlib dict 語義**(kernel/hashlib.h):entries=vector、insert=
//!   尾部追加、erase=**swap-and-pop**(把 back 移到被刪位)、正向迭代
//!   =尾到頭(=ry-ir Vec `.rev()`)、dump(rbegin)=entries 正序——
//!   simplemap 的 add/remove 順序直接反映 dump 行序(always01 實測:
//!   remove($sdff) 把 slice$9 挪到最前,dump=`add,slice$9,$6,$7,$8`);
//! * Module::connect(kernel/rtlil.cc:3064):lhs 含常數位=逐位過濾掉
//!   常數位(全過濾空則不加),否則 push_back(不拆 chunk);
//! * transfer_src:非 ff mapper 只轉移 `\src` 屬性;ff 家族 emit 的
//!   `cell->attributes = attributes` =**全量繼承**原 cell 屬性;
//! * 迭代序:`design->modules()`/`mod->cells()` 正向=逆插入序(ry-ir
//!   `.rev()`),Mapping 行序=cells snapshot 迭代序;blackbox 模組跳過。

use crate::check::is_boxed;
use crate::jsonout::unescape;
use crate::Log;
use ry_ir::{Bit, Cell, Const, Design, InsertMap, Module, SigBit, SigSpec, Wire};

// ---------------------------------------------------------------------------
// NEW_ID 行號錨(simplemap.cc 二進制=clone 零漂移,16 錨全量普查實錄;
// 語料未覆蓋 mapper 的行號取 clone 讀碼值,PROVENANCE 登記無錨)
// ---------------------------------------------------------------------------

const NOT_CELL: u32 = 46; // simplemap_not addCell($_NOT_)
const BITOP_CELL: u32 = 106; // simplemap_bitop addCell(gate_type)
const REDUCE_WIRE: u32 = 148; // simplemap_reduce addWire(sig_t)
const REDUCE_CELL: u32 = 157; // simplemap_reduce addCell(gate_type)
const XNOR_WIRE: u32 = 169; // simplemap_reduce(xnor) addWire(sig_t)
const XNOR_CELL: u32 = 170; // simplemap_reduce(xnor) addCell($_NOT_)
const LOGRED_WIRE: u32 = 189; // logic_reduce addWire(sig_t)
const LOGRED_CELL: u32 = 198; // logic_reduce addCell($_OR_)
const LOGNOT_CELL: u32 = 227; // simplemap_lognot addCell($_NOT_)
const LOGBIN_CELL: u32 = 256; // simplemap_logbin addCell(gate_type)
const EQNE_XOR_WIRE: u32 = 271; // simplemap_eqne addWire(xor_out)
const EQNE_XOR_CELL: u32 = 272; // clone addXor 行(中間名被 remove,僅耗 autoidx)
const EQNE_RED_WIRE: u32 = 277; // simplemap_eqne addWire(reduce_out)
const MUX_CELL: u32 = 298; // simplemap_mux addCell($_MUX_)
const BWMUX_CELL: u32 = 315; // simplemap_bwmux addCell($_MUX_)[未覆蓋,clone]
const TRIBUF_CELL: u32 = 331; // simplemap_tribuf addCell($_TBUF_)[未覆蓋,clone]
const BMUX_WIRE: u32 = 346; // simplemap_bmux addWire(new_data)[未覆蓋,clone]
const BMUX_CELL: u32 = 349; // simplemap_bmux addCell($_MUX_)[未覆蓋,clone]
const LUT_WIRE: u32 = 370; // simplemap_lut addWire(new_lut_data)[未覆蓋,clone]
const LUT_CELL: u32 = 372; // simplemap_lut addCell($_MUX_)[未覆蓋,clone]
const SOP_EQ: u32 = 409; // simplemap_sop module->Eq[未覆蓋,clone]
const SOP_RED: u32 = 412; // simplemap_sop module->ReduceOr[未覆蓋,clone]
const PMUX_AND_CELL: u32 = 460; // simplemap_pmux addCell($_AND_)
const PMUX_AND_WIRE: u32 = 465; // simplemap_pmux addWire(and_y)
const PMUX_MUX_CELL: u32 = 474; // simplemap_pmux addCell($_MUX_)
const FF_SLICE: u32 = 337; // kernel/ff.cc FfData::slice 的 NEW_ID(二進制實錄)

/// dispatch 表(simplemap.cc:483-530 `simplemap_get_mappers`;表外=靜默跳過)。
fn mappable(ctype: &str) -> bool {
    matches!(
        ctype,
        "$not" | "$pos"
            | "$buf"
            | "$and"
            | "$or"
            | "$xor"
            | "$xnor"
            | "$bweqx"
            | "$reduce_and"
            | "$reduce_or"
            | "$reduce_xor"
            | "$reduce_xnor"
            | "$reduce_bool"
            | "$logic_not"
            | "$logic_and"
            | "$logic_or"
            | "$eq"
            | "$eqx"
            | "$ne"
            | "$nex"
            | "$mux"
            | "$pmux"
            | "$bwmux"
            | "$tribuf"
            | "$bmux"
            | "$lut"
            | "$sop"
            | "$slice"
            | "$concat"
            | "$sr"
            | "$ff"
            | "$dff"
            | "$dffe"
            | "$dffsr"
            | "$dffsre"
            | "$adff"
            | "$adffe"
            | "$aldff"
            | "$aldffe"
            | "$sdff"
            | "$sdffe"
            | "$sdffce"
            | "$dlatch"
            | "$adlatch"
            | "$dlatchsr"
    )
}

// ---------------------------------------------------------------------------
// Module builder(hashlib dict 語義的精確模擬)
// ---------------------------------------------------------------------------

struct Mb<'a> {
    design: &'a mut Design,
    mi: usize,
}

impl<'a> Mb<'a> {
    fn module(&mut self) -> &mut Module {
        &mut self.design.modules[self.mi]
    }

    fn modref(&self) -> &Module {
        &self.design.modules[self.mi]
    }

    /// NEW_ID:`$auto$<file>:<line>:<func>$<autoidx++>`(create_id_prefix)。
    fn new_id(&mut self, file: &str, line: u32, func: &str) -> String {
        let n = self.design.autoidx;
        self.design.autoidx += 1;
        format!("$auto${file}:{line}:{func}${n}")
    }

    /// addWire(NEW_ID, width):wires dict 尾部追加。
    fn add_wire(&mut self, line: u32, func: &str, width: i64) -> SigSpec {
        let name = self.new_id("simplemap.cc", line, func);
        self.module().wires.push(Wire {
            name: name.clone(),
            width,
            ..Default::default()
        });
        wire_sig(&name, width)
    }

    /// addCell+setPort 序(gate cell:無參數,connections=插入序)。
    /// `src`=\src 屬性(transfer_src:非 ff mapper 只轉移 \src)。
    fn add_gate(
        &mut self,
        line: u32,
        func: &str,
        ctype: &str,
        ports: Vec<(&str, SigSpec)>,
        src: Option<&Const>,
    ) -> usize {
        let name = self.new_id("simplemap.cc", line, func);
        let mut attributes = InsertMap::new();
        if let Some(c) = src {
            attributes.insert("\\src".to_string(), c.clone());
        }
        self.module().cells.push(Cell {
            ctype: ctype.to_string(),
            name,
            attributes,
            parameters: InsertMap::new(),
            connections: ports
                .into_iter()
                .map(|(p, s)| (p.to_string(), s))
                .collect(),
        });
        self.module().cells.len() - 1
    }

    /// 中間 cell($xor/$reduce_or/$logic_not)完整構造(參數面=rtlil.cc
    /// 的 addXor/addReduceOr/addLogicNot:setParam 全套)。
    fn add_cell_named(&mut self, cell: Cell) -> usize {
        self.module().cells.push(cell);
        self.module().cells.len() - 1
    }

    /// Module::connect(rtlil.cc:3064):lhs 含常數位=逐位過濾(空=不加),
    /// 否則 push_back。
    fn connect(&mut self, lhs: SigSpec, rhs: SigSpec) {
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
                self.module().connections.push((nl, nr));
            }
            return;
        }
        self.module().connections.push((lhs, rhs));
    }

    /// hashlib dict do_erase=**swap-and-pop**(back 移到被刪位)。
    fn remove_cell(&mut self, ci: usize) {
        let cells = &mut self.module().cells;
        let last = cells.len() - 1;
        if ci != last {
            cells.swap(ci, last);
        }
        cells.pop();
    }

    /// 改已插入 cell 的端口值(dict insert 語義:已存在=原位改值)。
    fn set_port(&mut self, ci: usize, port: &str, sig: SigSpec) {
        for (p, s) in self.module().cells[ci].connections.iter_mut() {
            if p == port {
                *s = sig;
                return;
            }
        }
        self.module().cells[ci].connections.push((port.to_string(), sig));
    }

    fn port(&self, ci: usize, name: &str) -> SigSpec {
        self.modref().cells[ci]
            .connections
            .iter()
            .find(|(p, _)| p == name)
            .map(|(_, s)| s.clone())
            .unwrap_or_default()
    }

    fn param_bool(&self, ci: usize, name: &str) -> bool {
        match self.modref().cells[ci].parameters.get(name) {
            Some(Const::Bits { bits, .. }) => bits.first() == Some(&Bit::S1),
            Some(Const::Str(s)) => !s.is_empty(),
            None => false,
        }
    }

    fn param_const(&self, ci: usize, name: &str) -> Const {
        self.modref().cells[ci]
            .parameters
            .get(name)
            .cloned()
            .unwrap_or_default()
    }

    fn param_i64(&self, ci: usize, name: &str) -> i64 {
        match self.param_const(ci, name) {
            Const::Bits { bits, .. } => {
                let mut v = 0i64;
                for (i, b) in bits.iter().enumerate().take(63) {
                    if *b == Bit::S1 {
                        v |= 1 << i;
                    }
                }
                v
            }
            Const::Str(_) => 0,
        }
    }

    fn src_attr(&self, ci: usize) -> Option<Const> {
        self.modref().cells[ci].attributes.get("\\src").cloned()
    }
}

fn wire_sig(name: &str, width: i64) -> SigSpec {
    SigSpec {
        bits: (0..width)
            .map(|o| SigBit::Wire { wire: name.to_string(), offset: o })
            .collect(),
    }
}

fn const_bits(bit: Bit, width: i64) -> SigSpec {
    SigSpec { bits: vec![SigBit::Const(bit); width as usize] }
}

fn bit_at(sig: &SigSpec, i: i64) -> SigSpec {
    SigSpec { bits: vec![sig.bits[i as usize].clone()] }
}

/// SigSpec::extend_u0(rtlil.cc:5373):截到 width;不足則補
/// is_signed ? MSB(空=Sx) : S0。
fn extend_u0(sig: &SigSpec, width: i64, is_signed: bool) -> SigSpec {
    let mut out = sig.clone();
    if out.size() > width {
        out = SigSpec { bits: out.bits[..width as usize].to_vec() };
    }
    if out.size() < width {
        let padding = match out.bits.last() {
            Some(b) if is_signed => b.clone(),
            Some(_) => SigBit::Const(Bit::S0),
            None => SigBit::Const(Bit::Sx),
        };
        while out.size() < width {
            out.bits.push(padding.clone());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// mapper 核心(simplemap.cc 逐函數移植;ci=被映射 cell 的 index)
// ---------------------------------------------------------------------------

fn sm_not(b: &mut Mb, ci: usize) {
    let sig_a = b.port(ci, "\\A");
    let sig_y = b.port(ci, "\\Y");
    let a_signed = b.param_bool(ci, "\\A_SIGNED");
    let src = b.src_attr(ci);
    let sig_a = extend_u0(&sig_a, sig_y.size(), a_signed);
    for i in 0..sig_y.size() {
        b.add_gate(
            NOT_CELL,
            "simplemap_not",
            "$_NOT_",
            vec![("\\A", bit_at(&sig_a, i)), ("\\Y", bit_at(&sig_y, i))],
            src.as_ref(),
        );
    }
}

fn sm_pos(b: &mut Mb, ci: usize) {
    let sig_a = b.port(ci, "\\A");
    let sig_y = b.port(ci, "\\Y");
    let a_signed = b.param_bool(ci, "\\A_SIGNED");
    let sig_a = extend_u0(&sig_a, sig_y.size(), a_signed);
    b.connect(sig_y, sig_a);
}

fn sm_buf(b: &mut Mb, ci: usize) {
    let sig_a = b.port(ci, "\\A");
    let sig_y = b.port(ci, "\\Y");
    // 過濾 sig_a 的 z 位(State::Sz)。
    let mut new_a = SigSpec::default();
    let mut new_y = SigSpec::default();
    for i in 0..sig_a.size() {
        if sig_a.bits[i as usize] == SigBit::Const(Bit::Sz) {
            continue;
        }
        new_a.bits.push(sig_a.bits[i as usize].clone());
        new_y.bits.push(sig_y.bits[i as usize].clone());
    }
    if !new_y.bits.is_empty() {
        b.connect(new_y, new_a);
    }
}

fn sm_bitop(b: &mut Mb, ci: usize) {
    let ctype = b.module().cells[ci].ctype.clone();
    let sig_a = b.port(ci, "\\A");
    let sig_b = b.port(ci, "\\B");
    let sig_y = b.port(ci, "\\Y");
    let (sig_a, sig_b) = if ctype != "$bweqx" {
        (
            extend_u0(&sig_a, sig_y.size(), b.param_bool(ci, "\\A_SIGNED")),
            extend_u0(&sig_b, sig_y.size(), b.param_bool(ci, "\\B_SIGNED")),
        )
    } else {
        (sig_a, sig_b)
    };
    let gate_type = match ctype.as_str() {
        "$and" => "$_AND_",
        "$or" => "$_OR_",
        "$xor" => "$_XOR_",
        "$xnor" | "$bweqx" => "$_XNOR_",
        _ => unreachable!(),
    };
    let src = b.src_attr(ci);
    for i in 0..sig_y.size() {
        b.add_gate(
            BITOP_CELL,
            "simplemap_bitop",
            gate_type,
            vec![
                ("\\A", bit_at(&sig_a, i)),
                ("\\B", bit_at(&sig_b, i)),
                ("\\Y", bit_at(&sig_y, i)),
            ],
            src.as_ref(),
        );
    }
}

fn sm_reduce(b: &mut Mb, ci: usize) {
    let ctype = b.module().cells[ci].ctype.clone();
    let sig_a0 = b.port(ci, "\\A");
    let mut sig_y = b.port(ci, "\\Y");
    if sig_y.size() == 0 {
        return;
    }
    let src = b.src_attr(ci);
    if sig_a0.size() == 0 {
        let c = match ctype.as_str() {
            "$reduce_and" | "$reduce_xnor" => Bit::S1,
            _ => Bit::S0,
        };
        b.connect(sig_y.clone(), const_bits(c, sig_y.size()));
        return;
    }
    if sig_y.size() > 1 {
        let n = sig_y.size() - 1;
        b.connect(sig_y.extract(1, n), const_bits(Bit::S0, n));
        sig_y = sig_y.extract(0, 1);
    }
    let gate_type = match ctype.as_str() {
        "$reduce_and" => "$_AND_",
        "$reduce_or" | "$reduce_bool" => "$_OR_",
        "$reduce_xor" | "$reduce_xnor" => "$_XOR_",
        _ => unreachable!(),
    };
    let mut sig_a = sig_a0;
    let mut last_output_cell: Option<usize> = None;
    while sig_a.size() > 1 {
        let mut sig_t = b.add_wire(REDUCE_WIRE, "simplemap_reduce", sig_a.size() / 2);
        let mut i = 0;
        while i < sig_a.size() {
            if i + 1 == sig_a.size() {
                sig_t.bits.push(sig_a.bits[i as usize].clone());
                i += 2;
                continue;
            }
            let g = b.add_gate(
                REDUCE_CELL,
                "simplemap_reduce",
                gate_type,
                vec![
                    ("\\A", bit_at(&sig_a, i)),
                    ("\\B", bit_at(&sig_a, i + 1)),
                    ("\\Y", bit_at(&sig_t, i / 2)),
                ],
                src.as_ref(),
            );
            last_output_cell = Some(g);
            i += 2;
        }
        sig_a = sig_t;
    }
    if ctype == "$reduce_xnor" {
        let sig_t = b.add_wire(XNOR_WIRE, "simplemap_reduce", 1);
        let g = b.add_gate(
            XNOR_CELL,
            "simplemap_reduce",
            "$_NOT_",
            vec![("\\A", sig_a.clone()), ("\\Y", sig_t.clone())],
            src.as_ref(),
        );
        last_output_cell = Some(g);
        sig_a = sig_t;
    }
    match last_output_cell {
        None => b.connect(sig_y, sig_a),
        Some(g) => b.set_port(g, "\\Y", sig_y),
    }
}

/// static logic_reduce(simplemap.cc:185):$_OR_ 鏈把 sig 歸約到寬 ≤1。
fn logic_reduce(b: &mut Mb, sig: &mut SigSpec, src: Option<&Const>) {
    while sig.size() > 1 {
        let mut sig_t = b.add_wire(LOGRED_WIRE, "logic_reduce", sig.size() / 2);
        let mut i = 0;
        while i < sig.size() {
            if i + 1 == sig.size() {
                sig_t.bits.push(sig.bits[i as usize].clone());
                i += 2;
                continue;
            }
            b.add_gate(
                LOGRED_CELL,
                "logic_reduce",
                "$_OR_",
                vec![
                    ("\\A", bit_at(sig, i)),
                    ("\\B", bit_at(sig, i + 1)),
                    ("\\Y", bit_at(&sig_t, i / 2)),
                ],
                src,
            );
            i += 2;
        }
        *sig = sig_t;
    }
    if sig.size() == 0 {
        *sig = const_bits(Bit::S0, 1);
    }
}

fn sm_lognot(b: &mut Mb, ci: usize) {
    let mut sig_a = b.port(ci, "\\A");
    let src = b.src_attr(ci);
    logic_reduce(b, &mut sig_a, src.as_ref());
    let mut sig_y = b.port(ci, "\\Y");
    if sig_y.size() == 0 {
        return;
    }
    if sig_y.size() > 1 {
        let n = sig_y.size() - 1;
        b.connect(sig_y.extract(1, n), const_bits(Bit::S0, n));
        sig_y = sig_y.extract(0, 1);
    }
    b.add_gate(
        LOGNOT_CELL,
        "simplemap_lognot",
        "$_NOT_",
        vec![("\\A", sig_a), ("\\Y", sig_y)],
        src.as_ref(),
    );
}

fn sm_logbin(b: &mut Mb, ci: usize) {
    let ctype = b.module().cells[ci].ctype.clone();
    let mut sig_a = b.port(ci, "\\A");
    let mut sig_b = b.port(ci, "\\B");
    let src = b.src_attr(ci);
    logic_reduce(b, &mut sig_a, src.as_ref());
    logic_reduce(b, &mut sig_b, src.as_ref());
    let mut sig_y = b.port(ci, "\\Y");
    if sig_y.size() == 0 {
        return;
    }
    if sig_y.size() > 1 {
        let n = sig_y.size() - 1;
        b.connect(sig_y.extract(1, n), const_bits(Bit::S0, n));
        sig_y = sig_y.extract(0, 1);
    }
    let gate_type = if ctype == "$logic_and" { "$_AND_" } else { "$_OR_" };
    b.add_gate(
        LOGBIN_CELL,
        "simplemap_logbin",
        gate_type,
        vec![("\\A", sig_a), ("\\B", sig_b), ("\\Y", sig_y)],
        src.as_ref(),
    );
}

fn sm_mux(b: &mut Mb, ci: usize) {
    let sig_a = b.port(ci, "\\A");
    let sig_b = b.port(ci, "\\B");
    let sig_s = b.port(ci, "\\S");
    let sig_y = b.port(ci, "\\Y");
    let src = b.src_attr(ci);
    for i in 0..sig_y.size() {
        b.add_gate(
            MUX_CELL,
            "simplemap_mux",
            "$_MUX_",
            vec![
                ("\\A", bit_at(&sig_a, i)),
                ("\\B", bit_at(&sig_b, i)),
                ("\\S", sig_s.clone()),
                ("\\Y", bit_at(&sig_y, i)),
            ],
            src.as_ref(),
        );
    }
}

fn sm_bwmux(b: &mut Mb, ci: usize) {
    let sig_a = b.port(ci, "\\A");
    let sig_b = b.port(ci, "\\B");
    let sig_s = b.port(ci, "\\S");
    let sig_y = b.port(ci, "\\Y");
    let src = b.src_attr(ci);
    for i in 0..sig_y.size() {
        b.add_gate(
            BWMUX_CELL,
            "simplemap_bwmux",
            "$_MUX_",
            vec![
                ("\\A", bit_at(&sig_a, i)),
                ("\\B", bit_at(&sig_b, i)),
                ("\\S", bit_at(&sig_s, i)),
                ("\\Y", bit_at(&sig_y, i)),
            ],
            src.as_ref(),
        );
    }
}

fn sm_tribuf(b: &mut Mb, ci: usize) {
    let sig_a = b.port(ci, "\\A");
    let sig_e = b.port(ci, "\\EN");
    let sig_y = b.port(ci, "\\Y");
    let src = b.src_attr(ci);
    for i in 0..sig_y.size() {
        b.add_gate(
            TRIBUF_CELL,
            "simplemap_tribuf",
            "$_TBUF_",
            vec![
                ("\\A", bit_at(&sig_a, i)),
                ("\\E", sig_e.clone()),
                ("\\Y", bit_at(&sig_y, i)),
            ],
            src.as_ref(),
        );
    }
}

fn sm_bmux(b: &mut Mb, ci: usize) {
    let sel = b.port(ci, "\\S");
    let mut data = b.port(ci, "\\A");
    let width = b.port(ci, "\\Y").size();
    let src = b.src_attr(ci);
    for idx in 0..sel.size() {
        let sbit = bit_at(&sel, idx);
        let new_data = b.add_wire(BMUX_WIRE, "simplemap_bmux", data.size() / 2);
        let mut i = 0;
        while i < new_data.size() {
            let mut k = 0;
            while k < width {
                b.add_gate(
                    BMUX_CELL,
                    "simplemap_bmux",
                    "$_MUX_",
                    vec![
                        ("\\A", bit_at(&data, i * 2 + k)),
                        ("\\B", bit_at(&data, i * 2 + width + k)),
                        ("\\S", sbit.clone()),
                        ("\\Y", bit_at(&new_data, i + k)),
                    ],
                    src.as_ref(),
                );
                k += 1;
            }
            i += width;
        }
        data = new_data;
    }
    let y = b.port(ci, "\\Y");
    b.connect(y, data);
}

fn sm_lut(b: &mut Mb, ci: usize) {
    let lut_ctrl = b.port(ci, "\\A");
    let lut_param = b.param_const(ci, "\\LUT");
    let width = b.param_i64(ci, "\\WIDTH");
    let src = b.src_attr(ci);
    let mut lut_data = const_to_sig(&lut_param);
    lut_data = extend_u0(&lut_data, 1 << width, false);
    let mut idx = 0;
    while lut_data.size() > 1 {
        let new_lut_data = b.add_wire(LUT_WIRE, "simplemap_lut", lut_data.size() / 2);
        let mut i = 0;
        while i < lut_data.size() {
            b.add_gate(
                LUT_CELL,
                "simplemap_lut",
                "$_MUX_",
                vec![
                    ("\\A", bit_at(&lut_data, i)),
                    ("\\B", bit_at(&lut_data, i + 1)),
                    ("\\S", bit_at(&lut_ctrl, idx)),
                    ("\\Y", bit_at(&new_lut_data, i / 2)),
                ],
                src.as_ref(),
            );
            i += 2;
        }
        lut_data = new_lut_data;
        idx += 1;
    }
    let y = b.port(ci, "\\Y");
    b.connect(y, lut_data);
}

fn const_to_sig(c: &Const) -> SigSpec {
    match c {
        Const::Bits { bits, .. } => SigSpec { bits: bits.iter().map(|&b| SigBit::Const(b)).collect() },
        Const::Str(_) => SigSpec::default(),
    }
}

/// simplemap_eqne:構造中間 $xor/$reduce_or/$logic_not cell(完整參數面,
/// rtlil.cc addXor/addReduceOr/addLogicNot)、遞迴映射、remove——中間
/// cell 的插入/刪除=swap-and-pop 順序影響,必須真插真刪。
fn sm_eqne(b: &mut Mb, ci: usize) {
    let is_ne = matches!(b.module().cells[ci].ctype.as_str(), "$ne" | "$nex");
    let is_signed = b.param_bool(ci, "\\A_SIGNED");
    let sig_a = b.port(ci, "\\A");
    let sig_b = b.port(ci, "\\B");
    let sig_y = b.port(ci, "\\Y");
    let src = b.src_attr(ci);

    // addWire(xor_out) + addXor(NEW_ID, a, b, xor_out, is_signed)。
    let xor_out = b.add_wire(EQNE_XOR_WIRE, "simplemap_eqne", sig_a.size().max(sig_b.size()));
    let xc = {
        let cell = mk_bin_cell(b, "$xor", &sig_a, &sig_b, &xor_out, is_signed);
        b.add_cell_named(cell)
    };
    transfer_src(b, xc, src.as_ref());
    sm_bitop(b, xc);
    b.remove_cell(xc);

    let reduce_out = if is_ne {
        sig_y.clone()
    } else {
        b.add_wire(EQNE_RED_WIRE, "simplemap_eqne", 1)
    };
    let rc = {
        let cell = mk_unary_cell(
            b,
            "$reduce_or",
            &xor_out,
            &reduce_out,
            is_signed,
            EQNE_RED_CELL,
            "simplemap_eqne",
        );
        b.add_cell_named(cell)
    };
    transfer_src(b, rc, src.as_ref());
    sm_reduce(b, rc);
    b.remove_cell(rc);

    if !is_ne {
        let nc = {
            let cell = mk_unary_cell(
                b,
                "$logic_not",
                &reduce_out,
                &sig_y,
                is_signed,
                EQNE_NOT_CELL,
                "simplemap_eqne",
            );
            b.add_cell_named(cell)
        };
        transfer_src(b, nc, src.as_ref());
        sm_lognot(b, nc);
        b.remove_cell(nc);
    }
}

/// rtlil.cc addXor 參數面(A_SIGNED/B_SIGNED/A_WIDTH/B_WIDTH/Y_WIDTH)。
/// 中間 cell 被 remove,名字不入輸出——唯一可觀測影響=autoidx 消耗序。
const EQNE_RED_CELL: u32 = 278; // clone addReduceOr 行(名被 remove,僅耗 autoidx)
const EQNE_NOT_CELL: u32 = 285; // clone addLogicNot 行(同上)

fn mk_bin_cell(
    b: &mut Mb,
    ctype: &str,
    a: &SigSpec,
    bb: &SigSpec,
    y: &SigSpec,
    is_signed: bool,
) -> Cell {
    let name = b.new_id("simplemap.cc", EQNE_XOR_CELL, "simplemap_eqne");
    let mut parameters = InsertMap::new();
    parameters.insert("\\A_SIGNED".to_string(), bool_const(is_signed));
    parameters.insert("\\B_SIGNED".to_string(), bool_const(is_signed));
    parameters.insert("\\A_WIDTH".to_string(), i32_const(a.size()));
    parameters.insert("\\B_WIDTH".to_string(), i32_const(bb.size()));
    parameters.insert("\\Y_WIDTH".to_string(), i32_const(y.size()));
    Cell {
        ctype: ctype.to_string(),
        name,
        attributes: InsertMap::new(),
        parameters,
        connections: vec![
            ("\\A".to_string(), a.clone()),
            ("\\B".to_string(), bb.clone()),
            ("\\Y".to_string(), y.clone()),
        ],
    }
}

/// rtlil.cc addReduceOr/addLogicNot 參數面(A_SIGNED/A_WIDTH/Y_WIDTH)。
fn mk_unary_cell(
    b: &mut Mb,
    ctype: &str,
    a: &SigSpec,
    y: &SigSpec,
    is_signed: bool,
    line: u32,
    func: &str,
) -> Cell {
    let name = b.new_id("simplemap.cc", line, func);
    let mut parameters = InsertMap::new();
    parameters.insert("\\A_SIGNED".to_string(), bool_const(is_signed));
    parameters.insert("\\A_WIDTH".to_string(), i32_const(a.size()));
    parameters.insert("\\Y_WIDTH".to_string(), i32_const(y.size()));
    Cell {
        ctype: ctype.to_string(),
        name,
        attributes: InsertMap::new(),
        parameters,
        connections: vec![
            ("\\A".to_string(), a.clone()),
            ("\\Y".to_string(), y.clone()),
        ],
    }
}

fn transfer_src(b: &mut Mb, ci: usize, src: Option<&Const>) {
    if let Some(c) = src {
        b.module().cells[ci].attributes.insert("\\src".to_string(), c.clone());
    }
}

fn bool_const(v: bool) -> Const {
    Const::Bits {
        width: 1,
        bits: vec![if v { Bit::S1 } else { Bit::S0 }],
        signed: false,
        is_unsized: false,
    }
}

fn i32_const(v: i64) -> Const {
    Const::from_i64_i32_width(v)
}

fn sm_pmux(b: &mut Mb, ci: usize) {
    let sig_a = b.port(ci, "\\A");
    let sig_b = b.port(ci, "\\B");
    let sig_s = b.port(ci, "\\S");
    let sig_y = b.port(ci, "\\Y");
    let width = sig_a.size();
    let s_width = sig_s.size();
    let src = b.src_attr(ci);
    let mut any_s = sig_s.clone();
    logic_reduce(b, &mut any_s, src.as_ref());
    for i in 0..width {
        let mut b_and_bits = SigSpec::default();
        for j in 0..s_width {
            let g = b.add_gate(
                PMUX_AND_CELL,
                "simplemap_pmux",
                "$_AND_",
                vec![
                    ("\\A", bit_at(&sig_b, j * width + i)),
                    ("\\B", bit_at(&sig_s, j)),
                ],
                src.as_ref(),
            );
            let and_y = b.add_wire(PMUX_AND_WIRE, "simplemap_pmux", 1);
            b.set_port(g, "\\Y", and_y.clone());
            b_and_bits.bits.push(and_y.bits[0].clone());
        }
        logic_reduce(b, &mut b_and_bits, src.as_ref());
        b.add_gate(
            PMUX_MUX_CELL,
            "simplemap_pmux",
            "$_MUX_",
            vec![
                ("\\A", bit_at(&sig_a, i)),
                ("\\B", b_and_bits),
                ("\\S", any_s.clone()),
                ("\\Y", bit_at(&sig_y, i)),
            ],
            src.as_ref(),
        );
    }
}

fn sm_slice(b: &mut Mb, ci: usize) {
    let offset = b.param_i64(ci, "\\OFFSET");
    let sig_a = b.port(ci, "\\A");
    let sig_y = b.port(ci, "\\Y");
    let rhs = sig_a.extract(offset, sig_y.size());
    b.connect(sig_y, rhs);
}

fn sm_concat(b: &mut Mb, ci: usize) {
    let mut sig_ab = b.port(ci, "\\A");
    let sig_b = b.port(ci, "\\B");
    sig_ab.bits.extend(sig_b.bits);
    let sig_y = b.port(ci, "\\Y");
    b.connect(sig_y, sig_ab);
}

// ---------------------------------------------------------------------------
// ff 家族:FfData(kernel/ff.cc)粗→fine 的 manufacture/slice/emit 子集
// ---------------------------------------------------------------------------

struct FfData {
    has_clk: bool,
    has_gclk: bool,
    has_ce: bool,
    has_aload: bool,
    has_srst: bool,
    has_arst: bool,
    has_sr: bool,
    ce_over_srst: bool,
    is_fine: bool,
    is_anyinit: bool,
    pol_clk: bool,
    pol_ce: bool,
    pol_aload: bool,
    pol_arst: bool,
    pol_srst: bool,
    pol_clr: bool,
    pol_set: bool,
    val_arst: Const,
    val_srst: Const,
    sig_q: SigSpec,
    sig_d: SigSpec,
    sig_ad: SigSpec,
    sig_clk: SigSpec,
    sig_ce: SigSpec,
    sig_aload: SigSpec,
    sig_arst: SigSpec,
    sig_srst: SigSpec,
    sig_clr: SigSpec,
    sig_set: SigSpec,
    width: i64,
    attributes: InsertMap<Const>,
    /// slice 的 NEW_ID(ff.cc:337)=emit 的 cell 名透傳。
    slice_name: String,
}

impl FfData {
    fn new() -> FfData {
        FfData {
            has_clk: false,
            has_gclk: false,
            has_ce: false,
            has_aload: false,
            has_srst: false,
            has_arst: false,
            has_sr: false,
            ce_over_srst: false,
            is_fine: false,
            is_anyinit: false,
            pol_clk: false,
            pol_ce: false,
            pol_aload: false,
            pol_arst: false,
            pol_srst: false,
            pol_clr: false,
            pol_set: false,
            val_arst: Const::default(),
            val_srst: Const::default(),
            sig_q: SigSpec::default(),
            sig_d: SigSpec::default(),
            sig_ad: SigSpec::default(),
            sig_clk: SigSpec::default(),
            sig_ce: SigSpec::default(),
            sig_aload: SigSpec::default(),
            sig_arst: SigSpec::default(),
            sig_srst: SigSpec::default(),
            sig_clr: SigSpec::default(),
            sig_set: SigSpec::default(),
            width: 0,
            attributes: InsertMap::new(),
            slice_name: String::new(),
        }
    }
}

/// manufacture_info(ff.cc:26-322)的粗類型分支+const-D dlatch 特例。
fn ff_manufacture(b: &Mb, ci: usize) -> FfData {
    let ctype = b.modref().cells[ci].ctype.clone();
    let mut f = FfData::new();
    f.sig_q = b.port(ci, "\\Q");
    f.width = f.sig_q.size();
    f.attributes = b.modref().cells[ci].attributes.clone();
    match ctype.as_str() {
        "$anyinit" | "$ff" => {
            f.has_gclk = true;
            f.sig_d = b.port(ci, "\\D");
            f.is_anyinit = ctype == "$anyinit";
        }
        "$sr" => {}
        "$dlatch" | "$adlatch" | "$dlatchsr" => {
            f.has_aload = true;
            f.sig_aload = b.port(ci, "\\EN");
            f.pol_aload = b.param_bool(ci, "\\EN_POLARITY");
            f.sig_ad = b.port(ci, "\\D");
        }
        _ => {
            f.has_clk = true;
            f.sig_clk = b.port(ci, "\\CLK");
            f.pol_clk = b.param_bool(ci, "\\CLK_POLARITY");
            f.sig_d = b.port(ci, "\\D");
        }
    }
    if matches!(
        ctype.as_str(),
        "$dffe" | "$dffsre" | "$adffe" | "$aldffe" | "$sdffe" | "$sdffce"
    ) {
        f.has_ce = true;
        f.sig_ce = b.port(ci, "\\EN");
        f.pol_ce = b.param_bool(ci, "\\EN_POLARITY");
    }
    if matches!(ctype.as_str(), "$dffsr" | "$dffsre" | "$dlatchsr" | "$sr") {
        f.has_sr = true;
        f.sig_clr = b.port(ci, "\\CLR");
        f.sig_set = b.port(ci, "\\SET");
        f.pol_clr = b.param_bool(ci, "\\CLR_POLARITY");
        f.pol_set = b.param_bool(ci, "\\SET_POLARITY");
    }
    if matches!(ctype.as_str(), "$aldff" | "$aldffe") {
        f.has_aload = true;
        f.sig_aload = b.port(ci, "\\ALOAD");
        f.pol_aload = b.param_bool(ci, "\\ALOAD_POLARITY");
        f.sig_ad = b.port(ci, "\\AD");
    }
    if matches!(ctype.as_str(), "$adff" | "$adffe" | "$adlatch") {
        f.has_arst = true;
        f.sig_arst = b.port(ci, "\\ARST");
        f.pol_arst = b.param_bool(ci, "\\ARST_POLARITY");
        f.val_arst = b.param_const(ci, "\\ARST_VALUE");
    }
    if matches!(ctype.as_str(), "$sdff" | "$sdffe" | "$sdffce") {
        f.has_srst = true;
        f.sig_srst = b.port(ci, "\\SRST");
        f.pol_srst = b.param_bool(ci, "\\SRST_POLARITY");
        f.val_srst = b.param_const(ci, "\\SRST_VALUE");
        f.ce_over_srst = ctype == "$sdffce";
    }
    // have_cell 特例:純 D latch(const D)→ arst 表示。
    if f.has_aload && !f.has_clk && !f.has_sr && !f.has_arst && is_fully_const(&f.sig_ad) {
        f.has_aload = false;
        f.has_arst = true;
        f.sig_arst = f.sig_aload.clone();
        f.pol_arst = f.pol_aload;
        f.val_arst = sig_as_const(&f.sig_ad);
    }
    f
}

fn is_fully_const(sig: &SigSpec) -> bool {
    sig.bits.iter().all(|b| matches!(b, SigBit::Const(_)))
}

fn sig_as_const(sig: &SigSpec) -> Const {
    Const::Bits {
        width: sig.size(),
        bits: sig
            .bits
            .iter()
            .map(|b| match b {
                SigBit::Const(c) => *c,
                SigBit::Wire { .. } => Bit::Sx,
            })
            .collect(),
        signed: false,
        is_unsized: false,
    }
}

fn const_bit(c: &Const, i: i64) -> Bit {
    match c {
        Const::Bits { bits, .. } => bits.get(i as usize).copied().unwrap_or(Bit::Sx),
        Const::Str(_) => Bit::Sx,
    }
}

fn const_as_bool(c: &Const) -> bool {
    match c {
        Const::Bits { bits, .. } => bits.first() == Some(&Bit::S1),
        Const::Str(s) => !s.is_empty(),
    }
}

impl FfData {
    /// FfData::slice(ff.cc:336-397)——NEW_ID=ff.cc:337(二進制實錄)。
    fn slice(&self, b: &mut Mb, bits: &[i64]) -> FfData {
        let name = b.new_id("ff.cc", FF_SLICE, "slice");
        let mut res = FfData::new();
        res.sig_clk = self.sig_clk.clone();
        res.sig_ce = self.sig_ce.clone();
        res.sig_aload = self.sig_aload.clone();
        res.sig_arst = self.sig_arst.clone();
        res.sig_srst = self.sig_srst.clone();
        res.has_clk = self.has_clk;
        res.has_gclk = self.has_gclk;
        res.has_ce = self.has_ce;
        res.has_aload = self.has_aload;
        res.has_arst = self.has_arst;
        res.has_srst = self.has_srst;
        res.has_sr = self.has_sr;
        res.ce_over_srst = self.ce_over_srst;
        res.is_fine = self.is_fine;
        res.is_anyinit = self.is_anyinit;
        res.pol_clk = self.pol_clk;
        res.pol_ce = self.pol_ce;
        res.pol_aload = self.pol_aload;
        res.pol_arst = self.pol_arst;
        res.pol_srst = self.pol_srst;
        res.pol_clr = self.pol_clr;
        res.pol_set = self.pol_set;
        res.attributes = self.attributes.clone();
        let n = bits.len() as i64;
        let mut arst_bits = Vec::new();
        let mut srst_bits = Vec::new();
        for &i in bits {
            res.sig_q.bits.push(self.sig_q.bits[i as usize].clone());
            if self.has_clk || self.has_gclk {
                res.sig_d.bits.push(self.sig_d.bits[i as usize].clone());
            }
            if self.has_aload {
                res.sig_ad.bits.push(self.sig_ad.bits[i as usize].clone());
            }
            if self.has_sr {
                res.sig_clr.bits.push(self.sig_clr.bits[i as usize].clone());
                res.sig_set.bits.push(self.sig_set.bits[i as usize].clone());
            }
            arst_bits.push(const_bit(&self.val_arst, i));
            srst_bits.push(const_bit(&self.val_srst, i));
        }
        if self.has_arst {
            res.val_arst = Const::Bits { width: n, bits: arst_bits, signed: false, is_unsized: false };
        }
        if self.has_srst {
            res.val_srst = Const::Bits { width: n, bits: srst_bits, signed: false, is_unsized: false };
        }
        // initvals=nullptr(simplemap_ff 傳 nullptr)→ val_init 不複製。
        res.width = res.sig_q.size();
        res.slice_name = name;
        res
    }
}

impl FfData {
    fn emit_fine(&self, b: &mut Mb, name: &str) {
        // remove():cell=nullptr → no-op。width 由 slice 保證 ≥1。
        if !self.has_aload && !self.has_clk && !self.has_gclk && !self.has_sr {
            // 語料未觸發(const 驅動/arst→aload 轉換分支)。
            if self.has_arst {
                // arst_to_aload
                let mut f2 = clone_shallow(self);
                f2.pol_aload = f2.pol_arst;
                f2.sig_aload = f2.sig_arst.clone();
                f2.sig_ad = sig_from_const(&f2.val_arst);
                f2.has_aload = true;
                f2.has_arst = false;
                f2.emit_fine(b, name);
                return;
            }
            // initvals=nullptr → val_init 恆空(const 驅動分支,語料未觸發)。
            b.connect(self.sig_q.clone(), sig_from_const(&Const::default()));
            return;
        }
        let (ctype, ports) = if self.has_gclk {
            // $_FF_:D,Q
            (
                "$_FF_".to_string(),
                vec![("\\D", self.sig_d.clone()), ("\\Q", self.sig_q.clone())],
            )
        } else if !self.has_aload && !self.has_clk {
            // $_SR_<S><R>_:S,R,Q
            (
                format!("$_SR_{}{}_", p(self.pol_set), p(self.pol_clr)),
                vec![
                    ("\\S", self.sig_set.clone()),
                    ("\\R", self.sig_clr.clone()),
                    ("\\Q", self.sig_q.clone()),
                ],
            )
        } else if !self.has_clk {
            if self.has_sr {
                (
                    format!(
                        "$_DLATCHSR_{}{}{}_",
                        p(self.pol_aload),
                        p(self.pol_set),
                        p(self.pol_clr)
                    ),
                    vec![
                        ("\\E", self.sig_aload.clone()),
                        ("\\S", self.sig_set.clone()),
                        ("\\R", self.sig_clr.clone()),
                        ("\\D", self.sig_ad.clone()),
                        ("\\Q", self.sig_q.clone()),
                    ],
                )
            } else if self.has_arst {
                (
                    format!(
                        "$_DLATCH_{}{}{}_",
                        p(self.pol_aload),
                        p(self.pol_arst),
                        b01(const_as_bool(&self.val_arst))
                    ),
                    vec![
                        ("\\E", self.sig_aload.clone()),
                        ("\\R", self.sig_arst.clone()),
                        ("\\D", self.sig_ad.clone()),
                        ("\\Q", self.sig_q.clone()),
                    ],
                )
            } else {
                (
                    format!("$_DLATCH_{}_", p(self.pol_aload)),
                    vec![
                        ("\\E", self.sig_aload.clone()),
                        ("\\D", self.sig_ad.clone()),
                        ("\\Q", self.sig_q.clone()),
                    ],
                )
            }
        } else if self.has_sr {
            if self.has_ce {
                (
                    format!(
                        "$_DFFSRE_{}{}{}{}_",
                        p(self.pol_clk),
                        p(self.pol_set),
                        p(self.pol_clr),
                        p(self.pol_ce)
                    ),
                    vec![
                        ("\\C", self.sig_clk.clone()),
                        ("\\S", self.sig_set.clone()),
                        ("\\R", self.sig_clr.clone()),
                        ("\\E", self.sig_ce.clone()),
                        ("\\D", self.sig_d.clone()),
                        ("\\Q", self.sig_q.clone()),
                    ],
                )
            } else {
                (
                    format!(
                        "$_DFFSR_{}{}{}_",
                        p(self.pol_clk),
                        p(self.pol_set),
                        p(self.pol_clr)
                    ),
                    vec![
                        ("\\C", self.sig_clk.clone()),
                        ("\\S", self.sig_set.clone()),
                        ("\\R", self.sig_clr.clone()),
                        ("\\D", self.sig_d.clone()),
                        ("\\Q", self.sig_q.clone()),
                    ],
                )
            }
        } else if self.has_arst {
            if self.has_ce {
                (
                    format!(
                        "$_DFFE_{}{}{}{}_",
                        p(self.pol_clk),
                        p(self.pol_arst),
                        b01(const_as_bool(&self.val_arst)),
                        p(self.pol_ce)
                    ),
                    vec![
                        ("\\C", self.sig_clk.clone()),
                        ("\\R", self.sig_arst.clone()),
                        ("\\E", self.sig_ce.clone()),
                        ("\\D", self.sig_d.clone()),
                        ("\\Q", self.sig_q.clone()),
                    ],
                )
            } else {
                (
                    format!(
                        "$_DFF_{}{}{}_",
                        p(self.pol_clk),
                        p(self.pol_arst),
                        b01(const_as_bool(&self.val_arst))
                    ),
                    vec![
                        ("\\C", self.sig_clk.clone()),
                        ("\\R", self.sig_arst.clone()),
                        ("\\D", self.sig_d.clone()),
                        ("\\Q", self.sig_q.clone()),
                    ],
                )
            }
        } else if self.has_aload {
            if self.has_ce {
                (
                    format!(
                        "$_ALDFFE_{}{}{}_",
                        p(self.pol_clk),
                        p(self.pol_aload),
                        p(self.pol_ce)
                    ),
                    vec![
                        ("\\C", self.sig_clk.clone()),
                        ("\\L", self.sig_aload.clone()),
                        ("\\E", self.sig_ce.clone()),
                        ("\\D", self.sig_d.clone()),
                        ("\\AD", self.sig_ad.clone()),
                        ("\\Q", self.sig_q.clone()),
                    ],
                )
            } else {
                (
                    format!("$_ALDFF_{}{}_", p(self.pol_clk), p(self.pol_aload)),
                    vec![
                        ("\\C", self.sig_clk.clone()),
                        ("\\L", self.sig_aload.clone()),
                        ("\\D", self.sig_d.clone()),
                        ("\\AD", self.sig_ad.clone()),
                        ("\\Q", self.sig_q.clone()),
                    ],
                )
            }
        } else if self.has_srst {
            if self.has_ce {
                if self.ce_over_srst {
                    (
                        format!(
                            "$_SDFFCE_{}{}{}{}_",
                            p(self.pol_clk),
                            p(self.pol_srst),
                            b01(const_as_bool(&self.val_srst)),
                            p(self.pol_ce)
                        ),
                        vec![
                            ("\\C", self.sig_clk.clone()),
                            ("\\R", self.sig_srst.clone()),
                            ("\\E", self.sig_ce.clone()),
                            ("\\D", self.sig_d.clone()),
                            ("\\Q", self.sig_q.clone()),
                        ],
                    )
                } else {
                    (
                        format!(
                            "$_SDFFE_{}{}{}{}_",
                            p(self.pol_clk),
                            p(self.pol_srst),
                            b01(const_as_bool(&self.val_srst)),
                            p(self.pol_ce)
                        ),
                        vec![
                            ("\\C", self.sig_clk.clone()),
                            ("\\R", self.sig_srst.clone()),
                            ("\\E", self.sig_ce.clone()),
                            ("\\D", self.sig_d.clone()),
                            ("\\Q", self.sig_q.clone()),
                        ],
                    )
                }
            } else {
                (
                    format!(
                        "$_SDFF_{}{}{}_",
                        p(self.pol_clk),
                        p(self.pol_srst),
                        b01(const_as_bool(&self.val_srst))
                    ),
                    vec![
                        ("\\C", self.sig_clk.clone()),
                        ("\\R", self.sig_srst.clone()),
                        ("\\D", self.sig_d.clone()),
                        ("\\Q", self.sig_q.clone()),
                    ],
                )
            }
        } else if self.has_ce {
            (
                format!("$_DFFE_{}{}_", p(self.pol_clk), p(self.pol_ce)),
                vec![
                    ("\\C", self.sig_clk.clone()),
                    ("\\E", self.sig_ce.clone()),
                    ("\\D", self.sig_d.clone()),
                    ("\\Q", self.sig_q.clone()),
                ],
            )
        } else {
            (
                format!("$_DFF_{}_", p(self.pol_clk)),
                vec![
                    ("\\C", self.sig_clk.clone()),
                    ("\\D", self.sig_d.clone()),
                    ("\\Q", self.sig_q.clone()),
                ],
            )
        };
        // cell->attributes = attributes(全量繼承)。
        b.module().cells.push(Cell {
            ctype: ctype.to_string(),
            name: name.to_string(),
            attributes: self.attributes.clone(),
            parameters: InsertMap::new(),
            connections: ports.into_iter().map(|(p, s)| (p.to_string(), s)).collect(),
        });
    }
}

// 淺 clone 輔助(arst→aload 遞迴用;語料未觸發路徑)。
#[allow(clippy::field_reassign_with_default)]
fn clone_shallow(f: &FfData) -> FfData {
    let mut r = FfData::new();
    r.has_clk = f.has_clk;
    r.has_gclk = f.has_gclk;
    r.has_ce = f.has_ce;
    r.has_aload = f.has_aload;
    r.has_srst = f.has_srst;
    r.has_arst = f.has_arst;
    r.has_sr = f.has_sr;
    r.ce_over_srst = f.ce_over_srst;
    r.is_fine = f.is_fine;
    r.is_anyinit = f.is_anyinit;
    r.pol_clk = f.pol_clk;
    r.pol_ce = f.pol_ce;
    r.pol_aload = f.pol_aload;
    r.pol_arst = f.pol_arst;
    r.pol_srst = f.pol_srst;
    r.pol_clr = f.pol_clr;
    r.pol_set = f.pol_set;
    r.val_arst = f.val_arst.clone();
    r.val_srst = f.val_srst.clone();
    r.sig_q = f.sig_q.clone();
    r.sig_d = f.sig_d.clone();
    r.sig_ad = f.sig_ad.clone();
    r.sig_clk = f.sig_clk.clone();
    r.sig_ce = f.sig_ce.clone();
    r.sig_aload = f.sig_aload.clone();
    r.sig_arst = f.sig_arst.clone();
    r.sig_srst = f.sig_srst.clone();
    r.sig_clr = f.sig_clr.clone();
    r.sig_set = f.sig_set.clone();
    r.width = f.width;
    r.attributes = f.attributes.clone();
    r
}

fn p(pol: bool) -> char {
    if pol { 'P' } else { 'N' }
}

fn b01(v: bool) -> char {
    if v { '1' } else { '0' }
}

fn sig_from_const(c: &Const) -> SigSpec {
    const_to_sig(c)
}

/// simplemap_ff(simplemap.cc:431):per-bit slice+is_fine+emit。
fn sm_ff(b: &mut Mb, ci: usize) {
    let ff = ff_manufacture(b, ci);
    for i in 0..ff.width {
        let mut fff = ff.slice(b, &[i]);
        fff.is_fine = true;
        let name = std::mem::take(&mut fff.slice_name);
        fff.emit_fine(b, &name);
    }
}

// ---------------------------------------------------------------------------
// execute(SimplemapPass::execute,simplemap.cc:566-588)
// ---------------------------------------------------------------------------

pub fn run_simplemap(design: &mut Design, log: &mut Log) {
    let module_names: Vec<String> = design
        .modules
        .iter()
        .rev()
        .filter(|m| !is_boxed(m))
        .map(|m| m.name.clone())
        .collect();
    for mname in module_names {
        let mi = design.modules.iter().position(|m| m.name == mname).unwrap();
        let cell_names: Vec<String> = design.modules[mi].cells.iter().rev().map(|c| c.name.clone()).collect();
        for cname in cell_names {
            let ci = design.modules[mi]
                .cells
                .iter()
                .position(|c| c.name == cname)
                .unwrap();
            let ctype = design.modules[mi].cells[ci].ctype.clone();
            if !mappable(&ctype) {
                continue;
            }
            log.log(&format!(
                "Mapping {}.{} ({}).\n",
                unescape(&mname),
                unescape(&cname),
                unescape(&ctype)
            ));
            let mut b = Mb { design, mi };
            match ctype.as_str() {
                "$not" => sm_not(&mut b, ci),
                "$pos" => sm_pos(&mut b, ci),
                "$buf" => sm_buf(&mut b, ci),
                "$and" | "$or" | "$xor" | "$xnor" | "$bweqx" => sm_bitop(&mut b, ci),
                "$reduce_and" | "$reduce_or" | "$reduce_xor" | "$reduce_xnor" | "$reduce_bool" => {
                    sm_reduce(&mut b, ci)
                }
                "$logic_not" => sm_lognot(&mut b, ci),
                "$logic_and" | "$logic_or" => sm_logbin(&mut b, ci),
                "$eq" | "$eqx" | "$ne" | "$nex" => sm_eqne(&mut b, ci),
                "$mux" => sm_mux(&mut b, ci),
                "$pmux" => sm_pmux(&mut b, ci),
                "$bwmux" => sm_bwmux(&mut b, ci),
                "$tribuf" => sm_tribuf(&mut b, ci),
                "$bmux" => sm_bmux(&mut b, ci),
                "$lut" => sm_lut(&mut b, ci),
                "$sop" => sm_sop(&mut b, ci),
                "$slice" => sm_slice(&mut b, ci),
                "$concat" => sm_concat(&mut b, ci),
                _ => sm_ff(&mut b, ci), // 16 種 ff/latch 類型
            }
            b.remove_cell(ci);
        }
    }
}

/// simplemap_sop(語料未覆蓋;Module::Eq/ReduceOr 中間名無差分錨,
/// clone 行號 409/412 讀碼移植)。
fn sm_sop(b: &mut Mb, ci: usize) {
    let ctrl = b.port(ci, "\\A");
    let table = b.param_const(ci, "\\TABLE");
    let width = b.param_i64(ci, "\\WIDTH");
    let depth = b.param_i64(ci, "\\DEPTH");
    let src = b.src_attr(ci);
    let table = match table {
        Const::Bits { mut bits, .. } => {
            while (bits.len() as i64) < 2 * width * depth {
                bits.push(Bit::S0);
            }
            bits
        }
        Const::Str(_) => vec![Bit::S0; (2 * width * depth) as usize],
    };
    let mut products = SigSpec::default();
    for i in 0..depth {
        let mut input = SigSpec::default();
        let mut pat: Vec<Bit> = Vec::new();
        for j in 0..width {
            let idx = 2 * i * width + 2 * j;
            if table.get(idx as usize) == Some(&Bit::S1) {
                input.bits.push(ctrl.bits[j as usize].clone());
                pat.push(Bit::S0);
            }
            if table.get((idx + 1) as usize) == Some(&Bit::S1) {
                input.bits.push(ctrl.bits[j as usize].clone());
                pat.push(Bit::S1);
            }
        }
        if !input.bits.is_empty() {
            // module->Eq(NEW_ID, in, pat):$eq cell+內部 Y wire(寬 1)。
            let y = b.add_wire(SOP_EQ, "simplemap_sop", 1);
            let name = b.new_id("simplemap.cc", SOP_EQ, "simplemap_sop");
            let mut parameters = InsertMap::new();
            parameters.insert("\\A_SIGNED".to_string(), bool_const(false));
            parameters.insert("\\B_SIGNED".to_string(), bool_const(false));
            parameters.insert("\\A_WIDTH".to_string(), i32_const(input.size()));
            parameters.insert("\\B_WIDTH".to_string(), i32_const(input.size()));
            parameters.insert("\\Y_WIDTH".to_string(), i32_const(1));
            let pat_sig = SigSpec { bits: pat.iter().map(|&c| SigBit::Const(c)).collect() };
            b.module().cells.push(Cell {
                ctype: "$eq".to_string(),
                name,
                attributes: InsertMap::new(),
                parameters,
                connections: vec![
                    ("\\A".to_string(), input),
                    ("\\B".to_string(), pat_sig),
                    ("\\Y".to_string(), y.clone()),
                ],
            });
            products.bits.extend(y.bits);
        } else {
            products.bits.push(SigBit::Const(Bit::S1));
        }
    }
    // module->ReduceOr(NEW_ID, products):$reduce_or cell+內部 Y wire。
    let y = b.add_wire(SOP_RED, "simplemap_sop", 1);
    let name = b.new_id("simplemap.cc", SOP_RED, "simplemap_sop");
    let mut parameters = InsertMap::new();
    parameters.insert("\\A_SIGNED".to_string(), bool_const(false));
    parameters.insert("\\A_WIDTH".to_string(), i32_const(products.size()));
    parameters.insert("\\Y_WIDTH".to_string(), i32_const(1));
    b.module().cells.push(Cell {
        ctype: "$reduce_or".to_string(),
        name,
        attributes: InsertMap::new(),
        parameters,
        connections: vec![
            ("\\A".to_string(), products),
            ("\\Y".to_string(), y.clone()),
        ],
    });
    let _ = src;
    let out = b.port(ci, "\\Y");
    b.connect(out, y);
}
