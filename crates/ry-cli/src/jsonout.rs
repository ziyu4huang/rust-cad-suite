//! `write_json` 後端(backends/json/json.cc 的 byte-exact 移植)。
//!
//! 結構性契約(RYOSYS-MVP §4.3,禁源碼外推論、禁兩出口互推):
//! * `write_design` 先 `design->sort()`:模組、wires、cells、memories、
//!   cell 連接/參數/屬性、wire/memory 屬性按 `lt_by_name`(存儲名逐
//!   byte 比較,`$`<`\`)排序;**module attributes 不排序**——dict 正向
//!   迭代=逆插入序(實測 .il `src,top` → JSON `top,src`)。
//! * 每模組 sigidcounter 從 2 起;位編號按首次出現序分配:ports(port_id
//!   序)→ cells(名排序)的 connections(端口名排序)→ netnames(wire
//!   名排序);get_bits 先過 SigMap。
//! * 參數值:字串常數帶引號([01xz ]* 形補尾空格);位域=MSB-first
//!   二進位串(json.cc write_parameter_value,非 compat-int 模式)。
//! * 含 process 的模組=硬錯(json.cc:155):`Module %s contains
//!   processes, which are not supported by JSON backend (run `proc`
//!   first).`——檔案已按截斷式開啟,oracle 緩衝未落盤=0-byte,ryosys
//!   同樣留下空檔。

use crate::cells;
use crate::sigmap::SigMap;
use ry_ir::{Bit, Const, Design, Module, SigBit, SigSpec};
use std::collections::HashMap;

/// yosys IdString::unescape:剝前導 `\`(保持 `$` 形)。
pub fn unescape(name: &str) -> &str {
    if let Some(stripped) = name.strip_prefix('\\') {
        if stripped.starts_with('$')
            || stripped.starts_with('\\')
            || stripped.starts_with(|c: char| c.is_ascii_digit())
        {
            name
        } else {
            stripped
        }
    } else {
        name
    }
}

/// lt_by_name 等價:存儲名逐 byte 比較。
pub fn name_lt(a: &str, b: &str) -> std::cmp::Ordering {
    a.as_bytes().cmp(b.as_bytes())
}

fn json_escape(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Const::as_string 等價:MSB-first 位串(kernel/rtlil.cc:663)。
fn const_as_string(c: &Const) -> String {
    match c {
        Const::Str(s) => s.clone(),
        Const::Bits { bits, .. } => {
            let mut out = String::new();
            for b in bits.iter().rev() {
                out.push(b.as_char());
            }
            out
        }
    }
}

/// write_parameter_value 的字串常數補空格規則(json.cc:104-122)。
fn string_const_value(s: &str) -> String {
    let mut state = 0u8;
    for c in s.chars() {
        if state == 0 {
            if matches!(c, '0' | '1' | 'x' | 'z') {
                state = 0;
            } else if c == ' ' {
                state = 1;
            } else {
                state = 2;
            }
        } else if state == 1 && c != ' ' {
            state = 2;
        }
    }
    let mut t = s.to_string();
    if state < 2 {
        t.push(' ');
    }
    json_escape(&t)
}

fn write_parameter_value(c: &Const) -> String {
    match c {
        Const::Str(s) => string_const_value(s),
        Const::Bits { .. } => json_escape(&const_as_string(c)),
    }
}

struct JsonWriter<'a> {
    sigmap: SigMap,
    sigids: HashMap<SigBit, String>,
    sigidcounter: i64,
    design: &'a Design,
}

impl<'a> JsonWriter<'a> {
    fn get_bits(&mut self, sig: &SigSpec) -> String {
        let mut out = String::from("[");
        let mut first = true;
        for bit in self.sigmap.map(sig).bits {
            out.push_str(if first { " " } else { ", " });
            first = false;
            if !self.sigids.contains_key(&bit) {
                let s = match &bit {
                    SigBit::Const(Bit::S0) => "\"0\"".to_string(),
                    SigBit::Const(Bit::S1) => "\"1\"".to_string(),
                    SigBit::Const(Bit::Sz) => "\"z\"".to_string(),
                    SigBit::Const(_) => "\"x\"".to_string(),
                    SigBit::Wire { .. } => {
                        let id = self.sigidcounter;
                        self.sigidcounter += 1;
                        id.to_string()
                    }
                };
                self.sigids.insert(bit.clone(), s);
            }
            out.push_str(&self.sigids[&bit]);
        }
        out.push_str(" ]");
        out
    }

    /// write_parameters:entries 依傳入序;cell/wire/memory 屬性已由上層
    /// 排序,模組屬性以逆插入序傳入(dict 正向迭代)。
    fn write_parameters(&mut self, params: &[(String, Const)], indent: &str, out: &mut String) {
        let mut first = true;
        for (name, value) in params {
            out.push_str(if first { "" } else { "," });
            out.push('\n');
            out.push_str(&format!("{indent}{}: ", json_escape(unescape(name))));
            out.push_str(&write_parameter_value(value));
            first = false;
        }
    }

    fn write_module(&mut self, module: &Module, out: &mut String) -> Result<(), String> {
        self.sigmap = SigMap::new();
        self.sigmap.set(module);
        self.sigids.clear();
        self.sigidcounter = 2;

        if !module.processes.is_empty() {
            return Err(format!(
                "Module {} contains processes, which are not supported by JSON backend (run `proc` first).",
                unescape(&module.name)
            ));
        }

        out.push_str(&format!("    {}: {{\n", json_escape(unescape(&module.name))));

        out.push_str("      \"attributes\": {");
        // 模組屬性不排序:dict 正向迭代=逆插入序。
        let mut attrs: Vec<(String, Const)> = module.attributes.entries.clone();
        attrs.reverse();
        self.write_parameters(&attrs, "        ", out);
        out.push_str("\n      },\n");

        if !module.parameter_defaults.is_empty() {
            out.push_str("      \"parameter_default_values\": {");
            let mut defs = module.parameter_defaults.entries.clone();
            defs.sort_by(|a, b| name_lt(&a.0, &b.0));
            self.write_parameters(&defs, "        ", out);
            out.push_str("\n      },\n");
        }

        out.push_str("      \"ports\": {");
        // ports 按 port_id 序(ry-ir 解析層保證 wires 按宣告序;此處按
        // port_id 重排)。
        let mut port_wires: Vec<&ry_ir::Wire> =
            module.wires.iter().filter(|w| w.port_id > 0).collect();
        port_wires.sort_by_key(|w| w.port_id);
        let mut first = true;
        for w in &port_wires {
            out.push_str(if first { "" } else { "," });
            out.push('\n');
            out.push_str(&format!("        {}: {{\n", json_escape(unescape(&w.name))));
            let dir = match (w.port_input, w.port_output) {
                (true, true) => "inout",
                (true, false) => "input",
                _ => "output",
            };
            out.push_str(&format!("          \"direction\": \"{dir}\",\n"));
            if w.start_offset != 0 {
                out.push_str(&format!("          \"offset\": {},\n", w.start_offset));
            }
            if w.upto {
                out.push_str("          \"upto\": 1,\n");
            }
            if w.is_signed {
                out.push_str(&format!("          \"signed\": {},\n", w.is_signed as i32));
            }
            let bits = self.get_bits(&wire_sig(w));
            out.push_str(&format!("          \"bits\": {bits}\n"));
            out.push_str("        }");
            first = false;
        }
        out.push_str("\n      },\n");

        out.push_str("      \"cells\": {");
        let mut cells_sorted: Vec<&ry_ir::Cell> = module.cells.iter().collect();
        cells_sorted.sort_by(|a, b| name_lt(&a.name, &b.name));
        let mut first = true;
        for c in &cells_sorted {
            out.push_str(if first { "" } else { "," });
            out.push('\n');
            out.push_str(&format!("        {}: {{\n", json_escape(unescape(&c.name))));
            out.push_str(&format!(
                "          \"hide_name\": {},\n",
                if c.name.starts_with('$') { 1 } else { 0 }
            ));
            out.push_str(&format!("          \"type\": {},\n", json_escape(unescape(&c.ctype))));
            out.push_str("          \"parameters\": {");
            let mut params = c.parameters.entries.clone();
            params.sort_by(|a, b| name_lt(&a.0, &b.0));
            self.write_parameters(&params, "            ", out);
            out.push_str("\n          },\n");
            out.push_str("          \"attributes\": {");
            let mut attrs = c.attributes.entries.clone();
            attrs.sort_by(|a, b| name_lt(&a.0, &b.0));
            self.write_parameters(&attrs, "            ", out);
            out.push_str("\n          },\n");
            // Cell::known(rtlil.cc:4345):celltypes 命中 **或類型=設計內
            // 模組**(模組實例照樣印 port_directions,方向=子模組端口宣告,
            // 實測 arraycells 的 aoi12 實例;`$paramod…` 同理)。
            let known = cells::cell_known(&c.ctype)
                || self.design.find_module(&c.ctype).is_some();
            if known {
                out.push_str("          \"port_directions\": {");
                let mut conns = c.connections.clone();
                conns.sort_by(|a, b| name_lt(&a.0, &b.0));
                let mut first2 = true;
                for (port, _) in &conns {
                    out.push_str(if first2 { "" } else { "," });
                    out.push('\n');
                    let (inp, outp) = if cells::cell_known(&c.ctype) {
                        cells::port_dir(&c.ctype, port)
                    } else if let Some(sub) = self.design.find_module(&c.ctype) {
                        match sub.find_wire(port) {
                            Some(w) => (w.port_input, w.port_output),
                            None => (false, false),
                        }
                    } else {
                        (false, false)
                    };
                    let dir = match (inp, outp) {
                        (true, true) => "inout",
                        (true, false) => "input",
                        _ => "output",
                    };
                    out.push_str(&format!(
                        "            {}: \"{}\"",
                        json_escape(unescape(port)),
                        dir
                    ));
                    first2 = false;
                }
                out.push_str("\n          },\n");
            }
            out.push_str("          \"connections\": {");
            let mut conns = c.connections.clone();
            conns.sort_by(|a, b| name_lt(&a.0, &b.0));
            let mut first2 = true;
            for (port, sig) in &conns {
                out.push_str(if first2 { "" } else { "," });
                out.push('\n');
                let bits = self.get_bits(sig);
                out.push_str(&format!(
                    "            {}: {}",
                    json_escape(unescape(port)),
                    bits
                ));
                first2 = false;
            }
            out.push_str("\n          }\n");
            out.push_str("        }");
            first = false;
        }
        out.push_str("\n      },\n");

        if !module.memories.is_empty() {
            out.push_str("      \"memories\": {");
            let mut mems: Vec<&ry_ir::Memory> = module.memories.iter().collect();
            mems.sort_by(|a, b| name_lt(&a.name, &b.name));
            let mut first = true;
            for m in &mems {
                out.push_str(if first { "" } else { "," });
                out.push('\n');
                out.push_str(&format!("        {}: {{\n", json_escape(unescape(&m.name))));
                out.push_str(&format!(
                    "          \"hide_name\": {},\n",
                    if m.name.starts_with('$') { 1 } else { 0 }
                ));
                out.push_str("          \"attributes\": {");
                let mut attrs = m.attributes.entries.clone();
                attrs.sort_by(|a, b| name_lt(&a.0, &b.0));
                self.write_parameters(&attrs, "            ", out);
                out.push_str("\n          },\n");
                out.push_str(&format!("          \"width\": {},\n", m.width));
                out.push_str(&format!("          \"start_offset\": {},\n", m.start_offset));
                out.push_str(&format!("          \"size\": {}\n", m.size));
                out.push_str("        }");
                first = false;
            }
            out.push_str("\n      },\n");
        }

        out.push_str("      \"netnames\": {");
        let mut wires: Vec<&ry_ir::Wire> = module.wires.iter().collect();
        wires.sort_by(|a, b| name_lt(&a.name, &b.name));
        let mut first = true;
        for w in &wires {
            out.push_str(if first { "" } else { "," });
            out.push('\n');
            out.push_str(&format!("        {}: {{\n", json_escape(unescape(&w.name))));
            out.push_str(&format!(
                "          \"hide_name\": {},\n",
                if w.name.starts_with('$') { 1 } else { 0 }
            ));
            let bits = self.get_bits(&wire_sig(w));
            out.push_str(&format!("          \"bits\": {bits},\n"));
            if w.start_offset != 0 {
                out.push_str(&format!("          \"offset\": {},\n", w.start_offset));
            }
            if w.upto {
                out.push_str("          \"upto\": 1,\n");
            }
            if w.is_signed {
                out.push_str(&format!("          \"signed\": {},\n", w.is_signed as i32));
            }
            out.push_str("          \"attributes\": {");
            let mut attrs = w.attributes.entries.clone();
            attrs.sort_by(|a, b| name_lt(&a.0, &b.0));
            self.write_parameters(&attrs, "            ", out);
            out.push_str("\n          }\n");
            out.push_str("        }");
            first = false;
        }
        out.push_str("\n      }\n");
        out.push_str("    }");
        Ok(())
    }
}

fn wire_sig(w: &ry_ir::Wire) -> SigSpec {
    SigSpec {
        bits: (0..w.width)
            .map(|i| SigBit::Wire { wire: w.name.clone(), offset: i })
            .collect(),
    }
}

/// write_design(design->sort() 就地排序:模組按名)。
/// Err=process 硬錯(訊息帶模組名,CLI 層補 `ERROR: ` 前綴)。
pub fn write_json(design: &Design, creator: &str) -> Result<String, String> {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"creator\": {},\n", json_escape(creator)));
    out.push_str("  \"modules\": {\n");
    let mut mods: Vec<&Module> = design.modules.iter().collect();
    mods.sort_by(|a, b| name_lt(&a.name, &b.name));
    let mut writer = JsonWriter {
        sigmap: SigMap::new(),
        sigids: HashMap::new(),
        sigidcounter: 2,
        design,
    };
    let _ = writer.design;
    let mut first = true;
    for m in &mods {
        if !first {
            out.push_str(",\n");
        }
        writer.write_module(m, &mut out)?;
        first = false;
    }
    out.push_str("\n  }");
    out.push_str("\n}\n");
    Ok(out)
}
