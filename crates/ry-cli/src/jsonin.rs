//! `read_json` 前端(frontends/json/jsonparse.cc 的移植面,M1)。
//!
//! 輸入=本倉 write_json(以及 oracle write_json)的輸出;零依賴手寫
//! JSON parser(dict 節點保留**文件序**,jsonparse 的 yosys dict 正向
//! 迭代=逆文件序,由構建端 `.rev()` 體現)。
//!
//! 構建語義(jsonparse.cc 逐段對應):
//! * 模組按 data_dict 迭代=**逆文件序**匯入(`Importing module %s from
//!   JSON tree.` 逐模組行);屬性/參數同逆序插入。
//! * ports 按 `data_dict_keys`=**文件序**(port_id=1..n);埠線在首觸
//!   時創建。
//! * netnames/cells/memories 逆文件序;cell 連接/屬性/參數逆序插入。
//! * 位編號→SigBit:signal_bits 首見註冊;埠/netname 位命中已註冊編號
//!   → 逐位 `module->connect`(lhs 含常數位過濾後逐位棄常數);
//!   輸出埠連已註冊位=connect(該位, 舊位);否則 connect(舊位, 該位)
//!   並改註冊表(輸入埠搶佔代碼)。
//! * cell 連接的未註冊編號 → addWire(NEW_ID)(自增名 `$N`,autoidx
//!   遞增)。
//! * 無 autoidx 來源 → 從 1 起(實測 read_json 後 .il `autoidx 1`)。

use ry_ir::{Bit, Const, Design, Module, SigBit, SigSpec, Wire};

// ---------------------------------------------------------------------------
// 最小 JSON 樹(文件序 dict)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum JNode {
    Str(String),
    Num(f64),
    Bool(#[allow(dead_code)] bool),
    Arr(Vec<JNode>),
    /// (key, value) 按文件序。
    Dict(Vec<(String, JNode)>),
}

impl JNode {
    pub fn get<'a>(&'a self, key: &str) -> Option<&'a JNode> {
        match self {
            JNode::Dict(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            JNode::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_num(&self) -> Option<i64> {
        match self {
            JNode::Num(n) => Some(*n as i64),
            _ => None,
        }
    }
    pub fn as_arr(&self) -> Option<&Vec<JNode>> {
        match self {
            JNode::Arr(a) => Some(a),
            _ => None,
        }
    }
}

struct JParser<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> JParser<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && (self.b[self.i] as char).is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn parse_value(&mut self) -> Result<JNode, String> {
        self.ws();
        if self.i >= self.b.len() {
            return Err("JSON: unexpected end of input".to_string());
        }
        match self.b[self.i] {
            b'{' => {
                self.i += 1;
                let mut entries = Vec::new();
                self.ws();
                if self.i < self.b.len() && self.b[self.i] == b'}' {
                    self.i += 1;
                    return Ok(JNode::Dict(entries));
                }
                loop {
                    self.ws();
                    let key = match self.parse_value()? {
                        JNode::Str(s) => s,
                        _ => return Err("JSON: object key must be string".to_string()),
                    };
                    self.ws();
                    if self.i >= self.b.len() || self.b[self.i] != b':' {
                        return Err("JSON: expected ':'".to_string());
                    }
                    self.i += 1;
                    let val = self.parse_value()?;
                    entries.push((key, val));
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => {
                            self.i += 1;
                        }
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(JNode::Dict(entries));
                        }
                        _ => return Err("JSON: expected ',' or '}'".to_string()),
                    }
                }
            }
            b'[' => {
                self.i += 1;
                let mut items = Vec::new();
                self.ws();
                if self.i < self.b.len() && self.b[self.i] == b']' {
                    self.i += 1;
                    return Ok(JNode::Arr(items));
                }
                loop {
                    items.push(self.parse_value()?);
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => {
                            self.i += 1;
                        }
                        Some(b']') => {
                            self.i += 1;
                            return Ok(JNode::Arr(items));
                        }
                        _ => return Err("JSON: expected ',' or ']'".to_string()),
                    }
                }
            }
            b'"' => {
                self.i += 1;
                let mut s = String::new();
                while self.i < self.b.len() {
                    match self.b[self.i] {
                        b'"' => {
                            self.i += 1;
                            return Ok(JNode::Str(s));
                        }
                        b'\\' => {
                            self.i += 1;
                            let esc = *self.b.get(self.i).ok_or("JSON: bad escape")?;
                            self.i += 1;
                            match esc {
                                b'n' => s.push('\n'),
                                b't' => s.push('\t'),
                                b'r' => s.push('\r'),
                                b'b' => s.push('\u{8}'),
                                b'f' => s.push('\u{c}'),
                                b'u' => {
                                    let hex = std::str::from_utf8(
                                        &self.b[self.i..self.i + 4],
                                    )
                                    .map_err(|_| "JSON: bad \\u".to_string())?;
                                    let cp = u32::from_str_radix(hex, 16)
                                        .map_err(|_| "JSON: bad \\u".to_string())?;
                                    self.i += 4;
                                    s.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                                }
                                other => s.push(other as char),
                            }
                        }
                        other => {
                            // UTF-8 多字節原樣收集。
                            let start = self.i;
                            let len = utf8_len(other);
                            self.i += len;
                            let chunk =
                                std::str::from_utf8(&self.b[start..self.i.min(self.b.len())])
                                    .map_err(|_| "JSON: bad utf8".to_string())?;
                            s.push_str(chunk);
                        }
                    }
                }
                Err("JSON: unterminated string".to_string())
            }
            c if c == b'-' || c.is_ascii_digit() => {
                let start = self.i;
                self.i += 1;
                while self.i < self.b.len()
                    && (self.b[self.i].is_ascii_digit()
                        || matches!(self.b[self.i], b'.' | b'e' | b'E' | b'+' | b'-'))
                {
                    self.i += 1;
                }
                let text = std::str::from_utf8(&self.b[start..self.i]).unwrap();
                text.parse::<f64>()
                    .map(JNode::Num)
                    .map_err(|_| format!("JSON: bad number {text}"))
            }
            _ => {
                for (kw, val) in [("true", true), ("false", false)] {
                    if self.b[self.i..].starts_with(kw.as_bytes()) {
                        self.i += kw.len();
                        return Ok(JNode::Bool(val));
                    }
                }
                if self.b[self.i..].starts_with(b"null") {
                    self.i += 4;
                    return Ok(JNode::Dict(Vec::new()));
                }
                Err("JSON: unexpected token".to_string())
            }
        }
    }
}

fn utf8_len(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first >> 5 == 0b110 {
        2
    } else if first >> 4 == 0b1110 {
        3
    } else {
        4
    }
}

pub fn parse_json(text: &str) -> Result<JNode, String> {
    let mut p = JParser { b: text.as_bytes(), i: 0 };
    let v = p.parse_value()?;
    p.ws();
    Ok(v)
}

// ---------------------------------------------------------------------------
// Design 構建(jsonparse.cc 對應)
// ---------------------------------------------------------------------------

fn escape_id(name: &str) -> String {
    if name.starts_with('\\') || name.starts_with('$') {
        name.to_string()
    } else {
        format!("\\{name}")
    }
}

/// 屬性/參數值(json_parse_attr_param_value 逐字移植):
/// * 字串全為 [01xz](含空串)→ 位域常數(from_string;空串=0 寬全-x,
///   傾印 `0'x`);
/// * 否則若第一個非 [01xz] 字元起全是空格 → **字串常數**剝尾一字符
///   (write 端補尾空格的逆);
/// * 其餘 → 字串常數原樣;
/// * 數字 → 32-bit 常數,負數帶 signed 旗標。
fn parse_param_value(node: &JNode) -> Const {
    match node {
        JNode::Str(s) => {
            let not01xz = |c: char| !matches!(c, '0' | '1' | 'x' | 'z');
            match s.find(not01xz) {
                None => {
                    let bits: Vec<Bit> = s
                        .chars()
                        .rev()
                        .map(|c| match c {
                            '0' => Bit::S0,
                            '1' => Bit::S1,
                            'x' => Bit::Sx,
                            _ => Bit::Sz,
                        })
                        .collect();
                    let w = bits.len() as i64;
                    Const::Bits { width: w, bits, signed: false, is_unsized: false }
                }
                Some(cursor) => {
                    if s[cursor..].chars().all(|c| c == ' ') {
                        Const::Str(s[..s.len() - 1].to_string())
                    } else {
                        Const::Str(s.clone())
                    }
                }
            }
        }
        JNode::Num(n) => {
            let v = *n as i64;
            let mut c = Const::from_i64_i32_width(v);
            if let Const::Bits { signed, .. } = &mut c {
                *signed = v < 0;
            }
            c
        }
        _ => Const::default(),
    }
}

struct SigBits {
    map: Vec<(i64, SigBit)>,
}

impl SigBits {
    fn get(&self, id: i64) -> Option<&SigBit> {
        self.map.iter().find(|(k, _)| *k == id).map(|(_, v)| v)
    }
    fn insert(&mut self, id: i64, b: SigBit) {
        self.map.push((id, b));
    }
}

/// read_json 主入口:JSON 文本 → Design。log 行(Importing module…)
/// 追加到 `log`。
pub fn read_json(text: &str, log: &mut String) -> Result<Design, String> {
    let root = parse_json(text)?;
    let modules_node = root
        .get("modules")
        .ok_or_else(|| "JSON: no modules node".to_string())?;
    let entries = match modules_node {
        JNode::Dict(e) => e,
        _ => return Err("JSON: modules node is not a dictionary.".to_string()),
    };

    // jsonin 前端不登記 intern 序。邊界(2026-10-30 復審立法):autoidx_base=1
    // 使 `$<N>` 數字尾名(yosys JSON 私有名 `$abc$123` 形)在 opt_expr
    // intern_key 落入建名域 (1,N) 按 N 序、**不**回落 cells 向量序——
    // 語料面不觸(-rj 腳本不跑 opt_expr),擴 read_json→opt_expr 面前
    // 先補 intern 登記或重定義 base 語義(正典=SKILL intern 條前端分界句)。
    let mut design = Design { autoidx: 1, modules: Vec::new(), intern_seq: Default::default(), autoidx_base: 1 };
    // data_dict 正向迭代=逆文件序。
    for (modname, mnode) in entries.iter().rev() {
        let stored = escape_id(modname);
        // oracle 印原始 modname(JSON key 原文,非 escape 形)。
        log.push_str(&format!("Importing module {modname} from JSON tree.\n"));
        let module = import_module(&stored, mnode, &mut design)?;
        design.modules.push(module);
    }
    Ok(design)
}

fn import_module(stored_name: &str, node: &JNode, design: &mut Design) -> Result<Module, String> {
    let mut module = Module { name: stored_name.to_string(), ..Default::default() };
    let mut sigbits = SigBits { map: Vec::new() };

    if let Some(attrs) = node.get("attributes") {
        if let JNode::Dict(entries) = attrs {
            for (k, v) in entries.iter().rev() {
                module
                    .attributes
                    .insert(escape_id(k), parse_param_value(v));
            }
        }
    }
    if let Some(defs) = node.get("parameter_default_values") {
        if let JNode::Dict(entries) = defs {
            for (k, v) in entries.iter().rev() {
                module
                    .parameter_defaults
                    .insert(escape_id(k), parse_param_value(v));
            }
        }
    }

    // ports:文件序(data_dict_keys),port_id=1..n。
    if let Some(ports) = node.get("ports") {
        if let JNode::Dict(entries) = ports {
            for (port_id, (pname, pnode)) in entries.iter().enumerate() {
                let port_name = escape_id(pname);
                let bits = pnode
                    .get("bits")
                    .and_then(JNode::as_arr)
                    .ok_or_else(|| format!("JSON port node '{pname}' has no bits attribute."))?;
                let width = bits.len() as i64;
                let mut w = ensure_wire(&mut module, &port_name, width);
                apply_wire_flags(&mut w, pnode);
                let direction = pnode
                    .get("direction")
                    .and_then(JNode::as_str)
                    .ok_or_else(|| format!("JSON port node '{pname}' has no direction attribute."))?;
                match direction {
                    "input" => w.port_input = true,
                    "output" => w.port_output = true,
                    "inout" => {
                        w.port_input = true;
                        w.port_output = true;
                    }
                    other => {
                        return Err(format!(
                            "JSON port node '{pname}' has invalid '{other}' direction attribute."
                        ))
                    }
                }
                w.port_id = port_id as i64 + 1;
                let is_output = w.port_output;
                set_wire(&mut module, w);
                for (i, bv) in bits.iter().enumerate() {
                    let sigbit = SigBit::Wire { wire: port_name.clone(), offset: i as i64 };
                    import_bit(
                        &mut module, bv, sigbit, is_output, &mut sigbits, false,
                    )?;
                }
            }
        }
    }

    // netnames:逆文件序。
    if let Some(nets) = node.get("netnames") {
        if let JNode::Dict(entries) = nets {
            for (nname, nnode) in entries.iter().rev() {
                let net_name = escape_id(nname);
                let bits = nnode
                    .get("bits")
                    .and_then(JNode::as_arr)
                    .ok_or_else(|| format!("JSON netname node '{nname}' has no bits attribute."))?;
                let width = bits.len() as i64;
                // 已存在的線(=埠線)跳過位循環——0.70+179 二進制實測:
                // 埠位推連線只在 ports 階段發生一次,netnames 對埠線
                // 重跑會加倍(t1/t3 分離實驗,hierarchy-rj 32 vs 64)。
                let preexisted = module.find_wire(&net_name).is_some();
                let mut wire = ensure_wire(&mut module, &net_name, width);
                apply_wire_flags(&mut wire, nnode);
                set_wire(&mut module, wire);
                if !preexisted {
                    for (i, bv) in bits.iter().enumerate() {
                        let sigbit = SigBit::Wire { wire: net_name.clone(), offset: i as i64 };
                        import_bit(&mut module, bv, sigbit, false, &mut sigbits, true)?;
                    }
                }
                if let Some(attrs) = nnode.get("attributes") {
                    if let JNode::Dict(aentries) = attrs {
                        // 原位改(take+set 會因 set 找不到而丟線,實錄)。
                        if let Some(w) = module.wires.iter_mut().find(|x| x.name == net_name) {
                            for (k, v) in aentries.iter().rev() {
                                w.attributes.insert(escape_id(k), parse_param_value(v));
                            }
                        }
                    }
                }
            }
        }
    }

    // cells:逆文件序。
    if let Some(cells) = node.get("cells") {
        if let JNode::Dict(entries) = cells {
            for (cname, cnode) in entries.iter().rev() {
                let cell_name = escape_id(cname);
                let ctype = cnode
                    .get("type")
                    .and_then(JNode::as_str)
                    .ok_or_else(|| format!("JSON cells node '{cname}' has no type attribute."))?;
                let mut cell = ry_ir::Cell {
                    ctype: escape_id(ctype),
                    name: cell_name,
                    ..Default::default()
                };
                let conns = cnode
                    .get("connections")
                    .ok_or_else(|| format!("JSON cells node '{cname}' has no connections attribute."))?;
                if let JNode::Dict(centries) = conns {
                    for (port, cnode_bits) in centries.iter().rev() {
                        let port_name = escape_id(port);
                        let bits = cnode_bits
                            .as_arr()
                            .ok_or_else(|| format!("JSON cells node '{cname}' connection '{port}' is not an array."))?;
                        let mut sig = SigSpec { bits: Vec::new() };
                        for bv in bits {
                            match bv {
                                JNode::Str(s) => sig.bits.push(SigBit::Const(match s.as_str() {
                                    "0" => Bit::S0,
                                    "1" => Bit::S1,
                                    "x" => Bit::Sx,
                                    "z" => Bit::Sz,
                                    other => {
                                        return Err(format!(
                                            "JSON cells node '{cname}' connection '{port}' has invalid '{other}' bit string value."
                                        ))
                                    }
                                })),
                                JNode::Num(n) => {
                                    let id = *n as i64;
                                    let bit = match sigbits.get(id) {
                                        Some(b) => b.clone(),
                                        None => {
                                            // addWire(NEW_ID):自增 $N。
                                            let name = format!("${}", design.autoidx);
                                            design.autoidx += 1;
                                            let w = ensure_wire(&mut module, &name, 1);
                                            let _ = w;
                                            let bit =
                                                SigBit::Wire { wire: name, offset: 0 };
                                            sigbits.insert(id, bit.clone());
                                            bit
                                        }
                                    };
                                    sig.bits.push(bit);
                                }
                                _ => {
                                    return Err(format!(
                                        "JSON cells node '{cname}' connection '{port}' has invalid bit value."
                                    ))
                                }
                            }
                        }
                        cell.connections.push((port_name, sig));
                    }
                }
                if let Some(attrs) = cnode.get("attributes") {
                    if let JNode::Dict(aentries) = attrs {
                        for (k, v) in aentries.iter().rev() {
                            cell.attributes.insert(escape_id(k), parse_param_value(v));
                        }
                    }
                }
                if let Some(params) = cnode.get("parameters") {
                    if let JNode::Dict(pentries) = params {
                        for (k, v) in pentries.iter().rev() {
                            cell.parameters.insert(escape_id(k), parse_param_value(v));
                        }
                    }
                }
                module.cells.push(cell);
            }
        }
    }

    // memories:逆文件序。
    if let Some(mems) = node.get("memories") {
        if let JNode::Dict(entries) = mems {
            for (mname, mnode) in entries.iter().rev() {
                let mut mem = ry_ir::Memory {
                    name: escape_id(mname),
                    ..Default::default()
                };
                mem.width = mnode
                    .get("width")
                    .and_then(JNode::as_num)
                    .ok_or_else(|| format!("JSON memory node '{mname}' has no width attribute."))?;
                mem.start_offset = mnode.get("start_offset").and_then(JNode::as_num).unwrap_or(0);
                mem.size = mnode
                    .get("size")
                    .and_then(JNode::as_num)
                    .ok_or_else(|| format!("JSON memory node '{mname}' has no size attribute."))?;
                if let Some(attrs) = mnode.get("attributes") {
                    if let JNode::Dict(aentries) = attrs {
                        for (k, v) in aentries.iter().rev() {
                            mem.attributes.insert(escape_id(k), parse_param_value(v));
                        }
                    }
                }
                module.memories.push(mem);
            }
        }
    }

    // 傾印序=push 序反轉(見 connect_bit 註釋)。
    module.connections.reverse();
    Ok(module)
}

/// 埠/網名位的統一匯入(jsonparse 兩處同構的 bitval 循環)。
/// `is_output`=當前線是否輸出埠;`netnames`=netnames 段形(命中已註冊
/// 位一律 connect(sigbit, 舊位),無輸入搶佔分支)。
fn import_bit(
    module: &mut Module,
    bv: &JNode,
    sigbit: SigBit,
    is_output: bool,
    sigbits: &mut SigBits,
    netnames: bool,
) -> Result<(), String> {
    match bv {
        JNode::Str(s) => {
            let b = match s.as_str() {
                "0" => Bit::S0,
                "1" => Bit::S1,
                "x" => Bit::Sx,
                "z" => Bit::Sz,
                _ => return Err("JSON: invalid bit string value".to_string()),
            };
            connect_bit(module, sigbit, SigBit::Const(b));
            Ok(())
        }
        JNode::Num(n) => {
            let id = *n as i64;
            match sigbits.get(id) {
                Some(prev) => {
                    if prev == &sigbit {
                        return Ok(());
                    }
                    if is_output {
                        // 輸出埠驅動既有網:connect(該位, 舊位)。
                        connect_bit(module, sigbit.clone(), prev.clone());
                    } else if netnames {
                        connect_bit(module, sigbit.clone(), prev.clone());
                    } else {
                        // 輸入埠搶佔:connect(舊位, 該位) 並改註冊表。
                        connect_bit(module, prev.clone(), sigbit.clone());
                        sigbits.insert(id, sigbit);
                    }
                }
                None => {
                    sigbits.insert(id, sigbit);
                }
            }
            Ok(())
        }
        _ => Err("JSON: invalid bit value".to_string()),
    }
}

/// module->connect(SigBit, SigBit):lhs 含常數位 → 逐位過濾(單位=整條棄)。
/// 0.70+179 二進制實測:connect 的傾印序=push 序**反轉**(rt.il 連線
/// 組序=netnames 文件序、位序 [7]→[0]=逆 push);模組收尾統一反轉。
fn connect_bit(module: &mut Module, lhs: SigBit, rhs: SigBit) {
    if matches!(lhs, SigBit::Const(_)) {
        return; // 常數賦常數/含常數 lhs:過濾後空 → 棄
    }
    module.connections.push((
        SigSpec { bits: vec![lhs] },
        SigSpec { bits: vec![rhs] },
    ));
}

fn ensure_wire(module: &mut Module, name: &str, width: i64) -> Wire {
    if let Some(w) = module.find_wire(name) {
        return w.clone();
    }
    let w = Wire { name: name.to_string(), width, ..Default::default() };
    module.wires.push(w.clone());
    w
}

fn set_wire(module: &mut Module, w: Wire) {
    if let Some(e) = module.wires.iter_mut().find(|x| x.name == w.name) {
        *e = w;
    }
}

fn apply_wire_flags(w: &mut Wire, node: &JNode) {
    if let Some(v) = node.get("upto").and_then(JNode::as_num) {
        w.upto = v != 0;
    }
    if let Some(v) = node.get("signed").and_then(JNode::as_num) {
        w.is_signed = v != 0;
    }
    if let Some(v) = node.get("offset").and_then(JNode::as_num) {
        w.start_offset = v;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_and_netnames_create_wires() {
        let text = r#"{
  "creator": "x",
  "modules": {
    "m": {
      "attributes": {},
      "ports": { "a": { "direction": "input", "bits": [ 2 ] } },
      "cells": {},
      "netnames": { "a": { "hide_name": 0, "bits": [ 2 ], "attributes": {} } }
    }
  }
}"#;
        let mut log = String::new();
        let d = read_json(text, &mut log).unwrap();
        let m = &d.modules[0];
        assert_eq!(m.name, "\\m");
        assert_eq!(m.wires.len(), 1, "wires: {:?}", m.wires);
        assert!(m.wires[0].port_input);
        assert_eq!(m.wires[0].port_id, 1);
    }

    #[test]
    fn json_parse_orders_preserved() {
        let t = r#"{"b":1,"a":2,"c":{"z":1,"y":2}}"#;
        let n = parse_json(t).unwrap();
        match n {
            JNode::Dict(e) => {
                let keys: Vec<&str> = e.iter().map(|(k, _)| k.as_str()).collect();
                assert_eq!(keys, vec!["b", "a", "c"]);
            }
            _ => panic!(),
        }
    }
}
