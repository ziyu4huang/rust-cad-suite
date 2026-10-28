//! 遞迴下降 parser:逐流程照抄 oracle `RTLILFrontendWorker`
//! (vendor-reference/yosys frontends/rtlil/rtlil_frontend.cc:411-892)。
//!
//! 與 oracle 的顯式差異(均有實測或規劃依據):
//! * `twines` 區塊、`$priv@`/`$pub@` 名 token、cell `parameter real`、
//!   sigspec 內字串常數 = milestone Y1 硬錯(RYOSYS-MVP §1.3;oracle 對
//!   $priv@ 是靜默錯名,猜=假綠);
//! * connect 兩側寬度不等:oracle 是 log_assert 崩潰(kernel/rtlil.cc:
//!   2995 實測 `ERROR: Assert 'GetSize(conn.first) == GetSize(conn.second)'
//!   failed`),本 crate 以 Err 報同一非法輸入,訊息文本自訂(不冒充
//!   oracle 的 assert 文本——它內嵌構建路徑)。

use crate::ast::*;
use crate::lexer::{Cursor, RyError, WIDTH_LIMIT};

pub fn parse_design(input: &str) -> Result<Design, RyError> {
    let mut p = Parser { cur: Cursor::new(input), design: Design::default(), attrbuf: InsertMap::new() };
    p.run()?;
    Ok(p.design)
}

struct Parser<'a> {
    cur: Cursor<'a>,
    design: Design,
    attrbuf: InsertMap<Const>,
}

impl<'a> Parser<'a> {
    fn err(&self, msg: impl Into<String>) -> RyError {
        RyError::Parse { line: self.cur.line_num, msg: msg.into() }
    }

    fn expect_keyword(&mut self, kw: &str) -> Result<(), RyError> {
        if self.cur.try_parse_keyword(kw) {
            Ok(())
        } else {
            Err(self.err(format!("Expected token `{kw}', got `{ }'.", self.cur.error_token())))
        }
    }

    fn expect_char(&mut self, ch: char) -> Result<(), RyError> {
        if self.cur.try_parse_char(ch) {
            Ok(())
        } else {
            Err(self.err(format!("Expected `{ch}', got `{ }'.", self.cur.error_token())))
        }
    }

    /// oracle parse()(rtlil_frontend.cc:868-892)。
    fn run(&mut self) -> Result<(), RyError> {
        while self.cur.good() {
            if self.cur.try_parse_keyword("attribute") {
                self.parse_attribute()?;
                continue;
            }
            if self.cur.try_parse_keyword("module") {
                self.parse_module()?;
                continue;
            }
            if self.cur.try_parse_keyword("autoidx") {
                let v = self.cur.parse_integer()?;
                self.design.autoidx = self.design.autoidx.max(v);
                self.cur.expect_eol()?;
                continue;
            }
            // 默認 write_rtlil 壓縮形的 twines 區塊:M0 只承諾 -readable 形
            //(RYOSYS-MVP §0-6/§1.3,Y1)。
            if self.cur.try_parse_keyword("twines") {
                return Err(RyError::Unsupported {
                    line: self.cur.line_num,
                    what: "`twines' block",
                    milestone: "Y1",
                });
            }
            return Err(self.err(format!("Unexpected token: { }", self.cur.error_token())));
        }
        if !self.attrbuf.is_empty() {
            return Err(self.err("dangling attribute"));
        }
        // oracle read_rtlil 內部恰好消耗一個 autoidx id(實測 2026-10-27:
        // `autoidx 0'→回放 1;無 autoidx 行→1;`autoidx 0'+`autoidx 5'→5;
        // 三模組無行→仍 1——消耗一次,與模組數無關)。等效 max(parse, 1)。
        self.design.autoidx = self.design.autoidx.max(1);
        Ok(())
    }

    fn parse_attribute(&mut self) -> Result<(), RyError> {
        let id = self.cur.parse_id()?;
        let c = self.cur.parse_const()?;
        self.attrbuf.insert(id, c);
        self.cur.expect_eol()
    }

    /// oracle parse_module(rtlil_frontend.cc:411-484):重定義走 blackbox
    /// 語義(新模組帶 blackbox 屬性→忽略重定義;舊模組非 blackbox→硬錯;
    /// 舊為 blackbox→替換);末尾 fixup_ports。
    fn parse_module(&mut self) -> Result<(), RyError> {
        let module_name = self.cur.parse_id()?;
        self.cur.expect_eol()?;

        let mut delete_current_module = false;
        let mut replace_existing = false;
        if let Some(existing) = self.design.find_module(&module_name) {
            let new_is_blackbox = self
                .attrbuf
                .get("\\blackbox")
                .map(|c| const_as_bool(c))
                .unwrap_or(false);
            if new_is_blackbox {
                delete_current_module = true;
            } else if !existing
                .attributes
                .get("\\blackbox")
                .map(|c| const_as_bool(c))
                .unwrap_or(false)
            {
                return Err(self.err(format!("RTLIL error: redefinition of module {module_name}.")));
            } else {
                replace_existing = true;
            }
        }

        let mut module = Module {
            name: module_name,
            attributes: std::mem::take(&mut self.attrbuf),
            ..Module::default()
        };

        loop {
            if self.cur.try_parse_keyword("attribute") {
                self.parse_attribute()?;
                continue;
            }
            if self.cur.try_parse_keyword("parameter") {
                self.parse_parameter(&mut module)?;
                continue;
            }
            if self.cur.try_parse_keyword("connect") {
                self.parse_connect(&mut module)?;
                continue;
            }
            if self.cur.try_parse_keyword("wire") {
                self.parse_wire(&mut module)?;
                continue;
            }
            if self.cur.try_parse_keyword("cell") {
                self.parse_cell(&mut module)?;
                continue;
            }
            if self.cur.try_parse_keyword("memory") {
                self.parse_memory(&mut module)?;
                continue;
            }
            if self.cur.try_parse_keyword("process") {
                self.parse_process(&mut module)?;
                continue;
            }
            if self.cur.try_parse_keyword("end") {
                self.cur.expect_eol()?;
                break;
            }
            return Err(self.err(format!("Unexpected token in module body: { }", self.cur.error_token())));
        }

        if !self.attrbuf.is_empty() {
            return Err(self.err("dangling attribute"));
        }
        fixup_ports(&mut module);
        if replace_existing {
            let pos = self.design.modules.iter().position(|m| m.name == module.name).unwrap();
            self.design.modules[pos] = module;
        } else if !delete_current_module {
            self.design.modules.push(module);
        }
        Ok(())
    }

    /// oracle parse_parameter(rtlil_frontend.cc:494-503)。
    fn parse_parameter(&mut self, module: &mut Module) -> Result<(), RyError> {
        let id = self.cur.parse_id()?;
        if !module.avail_parameters.contains(&id) {
            module.avail_parameters.push(id.clone());
        }
        if self.cur.try_parse_eol() {
            return Ok(());
        }
        let c = self.cur.parse_const()?;
        module.parameter_defaults.insert(id, c);
        self.cur.expect_eol()
    }

    /// oracle parse_wire(rtlil_frontend.cc:505-572)。選項任意序;遇 ID 結束。
    fn parse_wire(&mut self, module: &mut Module) -> Result<(), RyError> {
        let mut width: i64 = 1;
        let mut start_offset: i64 = 0;
        let mut port_id: i64 = 0;
        let mut port_input = false;
        let mut port_output = false;
        let mut upto = false;
        let mut is_signed = false;
        let name: Id;

        loop {
            if let Some(id) = self.cur.try_parse_id()? {
                if module.find_wire(&id).is_some() {
                    return Err(self.err(format!("RTLIL error: redefinition of wire {id}.")));
                }
                name = id;
                break;
            }
            if self.cur.try_parse_keyword("width") {
                let width_val = self.cur.parse_integer()?;
                if !(0..WIDTH_LIMIT).contains(&width_val) {
                    return Err(self.err(format!("Wire width {width_val} out of range before `{ }`.", self.cur.error_token())));
                }
                width = width_val;
            } else if self.cur.try_parse_keyword("upto") {
                upto = true;
            } else if self.cur.try_parse_keyword("signed") {
                is_signed = true;
            } else if self.cur.try_parse_keyword("offset") {
                let offset_val = self.cur.parse_integer()?;
                if offset_val < i32::MIN as i64 || offset_val > i32::MAX as i64 {
                    return Err(self.err(format!("Wire offset {offset_val} out of range before `{ }`.", self.cur.error_token())));
                }
                start_offset = offset_val;
            } else if self.cur.try_parse_keyword("input") {
                port_id = self.cur.parse_integer()?;
                port_input = true;
            } else if self.cur.try_parse_keyword("output") {
                port_id = self.cur.parse_integer()?;
                port_output = true;
            } else if self.cur.try_parse_keyword("inout") {
                port_id = self.cur.parse_integer()?;
                port_input = true;
                port_output = true;
            } else if self.cur.try_parse_eol() {
                return Err(self.err("Missing wire ID"));
            } else {
                return Err(self.err(format!("Unexpected wire option: { }", self.cur.error_token())));
            }
        }

        module.wires.push(Wire {
            name,
            width,
            start_offset,
            port_id,
            port_input,
            port_output,
            upto,
            is_signed,
            attributes: std::mem::take(&mut self.attrbuf),
        });
        self.cur.expect_eol()
    }

    /// oracle parse_memory(rtlil_frontend.cc:574-624)。
    fn parse_memory(&mut self, module: &mut Module) -> Result<(), RyError> {
        let mut width: i64 = 1;
        let mut start_offset: i64 = 0;
        let mut size: i64 = 0;
        let name: Id;

        loop {
            if let Some(id) = self.cur.try_parse_id()? {
                if module.find_memory(&id).is_some() {
                    return Err(self.err(format!("RTLIL error: redefinition of memory {id}.")));
                }
                name = id;
                break;
            }
            if self.cur.try_parse_keyword("width") {
                let width_val = self.cur.parse_integer()?;
                if !(0..WIDTH_LIMIT).contains(&width_val) {
                    return Err(self.err(format!("Memory width {width_val} out of range before `{ }`.", self.cur.error_token())));
                }
                width = width_val;
            } else if self.cur.try_parse_keyword("size") {
                let size_val = self.cur.parse_integer()?;
                if size_val < i32::MIN as i64 || size_val > i32::MAX as i64 {
                    return Err(self.err(format!("Memory size {size_val} out of range before `{ }`.", self.cur.error_token())));
                }
                size = size_val;
            } else if self.cur.try_parse_keyword("offset") {
                let offset_val = self.cur.parse_integer()?;
                if offset_val < i32::MIN as i64 || offset_val > i32::MAX as i64 {
                    return Err(self.err(format!("Memory offset {offset_val} out of range before `{ }`.", self.cur.error_token())));
                }
                start_offset = offset_val;
            } else if self.cur.try_parse_eol() {
                return Err(self.err("Missing memory ID"));
            } else {
                return Err(self.err(format!("Unexpected memory option: { }", self.cur.error_token())));
            }
        }

        module.memories.push(Memory {
            name,
            width,
            start_offset,
            size,
            attributes: std::mem::take(&mut self.attrbuf),
        });
        self.cur.expect_eol()
    }

    /// oracle parse_cell(rtlil_frontend.cc:637-701)。`parameter real`=Y1 硬錯。
    fn parse_cell(&mut self, module: &mut Module) -> Result<(), RyError> {
        let cell_type = self.cur.parse_id()?;
        let cell_name = self.cur.parse_id()?;
        self.cur.expect_eol()?;

        if module.find_cell(&cell_name).is_some() {
            return Err(self.err(format!("RTLIL error: redefinition of cell {cell_name}.")));
        }
        let mut cell = Cell {
            ctype: cell_type,
            name: cell_name,
            attributes: std::mem::take(&mut self.attrbuf),
            parameters: InsertMap::new(),
            connections: Vec::new(),
        };

        loop {
            if self.cur.try_parse_keyword("parameter") {
                // 單旗標互斥(oracle if/else-if 鏈)。
                if self.cur.try_parse_keyword("signed") {
                    let param_name = self.cur.parse_id()?;
                    let mut val = self.cur.parse_const()?;
                    if let Const::Bits { signed, .. } = &mut val {
                        *signed = true;
                    }
                    cell.parameters.insert(param_name, val);
                    self.cur.expect_eol()?;
                } else if self.cur.try_parse_keyword("real") {
                    return Err(RyError::Unsupported {
                        line: self.cur.line_num,
                        what: "`parameter real'",
                        milestone: "Y1",
                    });
                } else if self.cur.try_parse_keyword("unsized") {
                    let param_name = self.cur.parse_id()?;
                    let mut val = self.cur.parse_const()?;
                    if let Const::Bits { is_unsized, .. } = &mut val {
                        *is_unsized = true;
                    }
                    cell.parameters.insert(param_name, val);
                    self.cur.expect_eol()?;
                } else {
                    let param_name = self.cur.parse_id()?;
                    let val = self.cur.parse_const()?;
                    cell.parameters.insert(param_name, val);
                    self.cur.expect_eol()?;
                }
            } else if self.cur.try_parse_keyword("connect") {
                let port_name = self.cur.parse_id()?;
                if cell.has_port(&port_name) {
                    return Err(self.err(format!("RTLIL error: redefinition of cell port {port_name}.")));
                }
                let sig = self.parse_sigspec(module)?;
                cell.connections.push((port_name, sig));
                self.cur.expect_eol()?;
            } else if self.cur.try_parse_keyword("end") {
                self.cur.expect_eol()?;
                break;
            } else {
                return Err(self.err(format!("Unexpected token in cell body: { }", self.cur.error_token())));
            }
        }
        module.cells.push(cell);
        Ok(())
    }

    /// oracle parse_connect(rtlil_frontend.cc:703-716)+ Module::connect 的
    /// lhs 常數位過濾(kernel/rtlil.cc:3065-3080)。
    fn parse_connect(&mut self, module: &mut Module) -> Result<(), RyError> {
        if !self.attrbuf.is_empty() {
            return Err(self.err("dangling attribute"));
        }
        let s1 = self.parse_sigspec(module)?;
        let s2 = self.parse_sigspec(module)?;
        if s1.size() != s2.size() {
            // oracle:log_assert 崩潰(實測 rtlil.cc:2995);此處同樣拒絕該輸入。
            return Err(self.err(format!(
                "connect operands have different sizes ({} vs {})",
                s1.size(),
                s2.size()
            )));
        }
        let (lhs, rhs) = if s1.has_const() {
            // 對常數賦常數的位全部丟棄;lhs 過濾後為空則整條 connect 丟棄。
            let mut new_lhs = SigSpec::default();
            let mut new_rhs = SigSpec::default();
            for (b1, b2) in s1.bits.iter().zip(s2.bits.iter()) {
                if matches!(b1, SigBit::Wire { .. }) {
                    new_lhs.bits.push(b1.clone());
                    new_rhs.bits.push(b2.clone());
                }
            }
            if new_lhs.bits.is_empty() {
                // lhs 全為常數位:整條 connect 丟棄(oracle 同)。
                self.cur.expect_eol()?;
                return Ok(());
            }
            (new_lhs, new_rhs)
        } else {
            (s1, s2)
        };
        module.connections.push((lhs, rhs));
        self.cur.expect_eol()
    }

    /// oracle parse_sigspec(rtlil_frontend.cc:335-409)。
    /// `{ A B C }`:parts 按文字序(MSB→LSB)收集,內部位序列倒序串接
    /// (值序 LSB 在前);chunk 重建是傾印期視圖,parse 端不做。
    fn parse_sigspec(&mut self, module: &Module) -> Result<SigSpec, RyError> {
        let mut sig = SigSpec::default();

        if self.cur.try_parse_char('{') {
            let mut parts: Vec<SigSpec> = Vec::new();
            while !self.cur.try_parse_char('}') {
                parts.push(self.parse_sigspec(module)?);
            }
            for part in parts.iter().rev() {
                sig.bits.extend(part.bits.iter().cloned());
            }
        } else if let Some(id) = self.cur.try_parse_id()? {
            match module.find_wire(&id) {
                Some(wire) => {
                    for off in 0..wire.width {
                        sig.bits.push(SigBit::Wire { wire: id.clone(), offset: off });
                    }
                }
                None => return Err(self.err(format!("Wire `{id}' not found."))),
            }
        } else {
            let c = self.cur.parse_const()?;
            match c {
                Const::Bits { bits, .. } => sig.bits.extend(bits.into_iter().map(SigBit::Const)),
                Const::Str(_) => {
                    return Err(RyError::Unsupported {
                        line: self.cur.line_num,
                        what: "string constant in sigspec",
                        milestone: "Y1",
                    });
                }
            }
        }

        while self.cur.try_parse_char('[') {
            let left = self.cur.parse_integer()?;
            if left >= sig.size() || left < 0 {
                return Err(self.err(format!("bit index {left} out of range")));
            }
            if self.cur.try_parse_char(':') {
                let right = self.cur.parse_integer()?;
                if right < 0 {
                    return Err(self.err(format!("bit index {right} out of range")));
                }
                if left < right {
                    return Err(self.err(format!("invalid slice [{left}:{right}]")));
                }
                sig = sig.extract(right, left - right + 1);
            } else {
                sig = sig.extract(left, 1);
            }
            self.expect_char(']')?;
        }
        Ok(sig)
    }

    /// oracle parse_case_body(rtlil_frontend.cc:718-741)。
    fn parse_case_body(&mut self, case_rule: &mut CaseRule, module: &Module) -> Result<(), RyError> {
        loop {
            if self.cur.try_parse_keyword("attribute") {
                self.parse_attribute()?;
            } else if self.cur.try_parse_keyword("switch") {
                self.parse_switch(case_rule, module)?;
            } else if self.cur.try_parse_keyword("assign") {
                if !self.attrbuf.is_empty() {
                    return Err(self.err("dangling attribute"));
                }
                let s1 = self.parse_sigspec(module)?;
                let s2 = self.parse_sigspec(module)?;
                case_rule.actions.push((s1, s2));
                self.cur.expect_eol()?;
            } else {
                return Ok(());
            }
        }
    }

    /// oracle parse_switch(rtlil_frontend.cc:743-783)。switch 的屬性在
    /// signal 解析**之後**立刻自 attrbuf move(oracle :746-748);
    /// case 前的 attribute 行歸屬下一個 case。
    fn parse_switch(&mut self, parent: &mut CaseRule, module: &Module) -> Result<(), RyError> {
        let signal = self.parse_sigspec(module)?;
        let mut rule = SwitchRule {
            attributes: std::mem::take(&mut self.attrbuf),
            signal,
            cases: Vec::new(),
        };
        self.cur.expect_eol()?;

        loop {
            if self.cur.try_parse_keyword("attribute") {
                self.parse_attribute()?;
                continue;
            }
            if self.cur.try_parse_keyword("end") {
                self.cur.expect_eol()?;
                break;
            }
            self.expect_keyword("case")?;
            let mut case_rule = CaseRule {
                attributes: std::mem::take(&mut self.attrbuf),
                compare: Vec::new(),
                actions: Vec::new(),
                switches: Vec::new(),
            };
            if !self.cur.try_parse_eol() {
                loop {
                    case_rule.compare.push(self.parse_sigspec(module)?);
                    if self.cur.try_parse_eol() {
                        break;
                    }
                    self.expect_char(',')?;
                }
            }
            self.parse_case_body(&mut case_rule, module)?;
            rule.cases.push(case_rule);
        }
        parent.switches.push(rule);
        Ok(())
    }

    /// oracle parse_process(rtlil_frontend.cc:785-864)。
    fn parse_process(&mut self, module: &mut Module) -> Result<(), RyError> {
        let proc_name = self.cur.parse_id()?;
        self.cur.expect_eol()?;

        if module.find_process(&proc_name).is_some() {
            return Err(self.err(format!("RTLIL error: redefinition of process {proc_name}.")));
        }
        let mut proc = Process {
            name: proc_name,
            attributes: std::mem::take(&mut self.attrbuf),
            root_case: CaseRule::default(),
            syncs: Vec::new(),
        };

        self.parse_case_body(&mut proc.root_case, module)?;

        while self.cur.try_parse_keyword("sync") {
            let stype = if self.cur.try_parse_keyword("low") {
                SyncType::Low
            } else if self.cur.try_parse_keyword("high") {
                SyncType::High
            } else if self.cur.try_parse_keyword("posedge") {
                SyncType::Posedge
            } else if self.cur.try_parse_keyword("negedge") {
                SyncType::Negedge
            } else if self.cur.try_parse_keyword("edge") {
                SyncType::Edge
            } else if self.cur.try_parse_keyword("always") {
                SyncType::Always
            } else if self.cur.try_parse_keyword("global") {
                SyncType::Global
            } else if self.cur.try_parse_keyword("init") {
                SyncType::Init
            } else {
                return Err(self.err(format!("Unexpected sync type: { }", self.cur.error_token())));
            };

            let signal = match stype {
                SyncType::Always | SyncType::Global | SyncType::Init => None,
                _ => Some(self.parse_sigspec(module)?),
            };
            let mut rule = SyncRule { stype, signal, actions: Vec::new(), mem_writes: Vec::new() };
            self.cur.expect_eol()?;

            let mut attributes_in_update_list = false;
            loop {
                if self.cur.try_parse_keyword("update") {
                    let s1 = self.parse_sigspec(module)?;
                    let s2 = self.parse_sigspec(module)?;
                    rule.actions.push((s1, s2));
                    self.cur.expect_eol()?;
                    continue;
                }
                if self.cur.try_parse_keyword("attribute") {
                    attributes_in_update_list = true;
                    self.parse_attribute()?;
                    continue;
                }
                if !self.cur.try_parse_keyword("memwr") {
                    break;
                }
                let act = MemWriteAction {
                    attributes: std::mem::take(&mut self.attrbuf),
                    memid: self.cur.parse_id()?,
                    address: self.parse_sigspec(module)?,
                    data: self.parse_sigspec(module)?,
                    enable: self.parse_sigspec(module)?,
                    priority_mask: self.cur.parse_const()?,
                };
                rule.mem_writes.push(act);
                self.cur.expect_eol()?;
            }
            // oracle:屬性可穿透 sync 但不可懸掛在 update 清單尾。
            if attributes_in_update_list && !self.attrbuf.is_empty() {
                return Err(self.err("dangling attribute"));
            }
            proc.syncs.push(rule);
        }

        self.expect_keyword("end")?;
        self.cur.expect_eol()?;
        module.processes.push(proc);
        Ok(())
    }
}

/// oracle Const::as_bool 的黑盒子判定用途(屬性存在且值非零位)。
fn const_as_bool(c: &Const) -> bool {
    match c {
        Const::Bits { bits, .. } => bits.first() == Some(&Bit::S1),
        Const::Str(_) => false,
    }
}

/// oracle Module::fixup_ports(kernel/rtlil.cc:3124-3150):非埠 wire 的
/// port_id 清零;埠按 (port_id, name) 排序後重編 1..N。排序語義照抄
/// fixup_ports_compare:**port_id 為 0 的排在最後**,同名按 port_id。
/// 規範形輸入(oracle 產出,埠 id 連續 1..N)為恆等變換。
fn fixup_ports(module: &mut Module) {
    for w in module.wires.iter_mut() {
        if !(w.port_input || w.port_output) {
            w.port_id = 0;
        }
    }
    let mut indexed: Vec<(i64, Id, usize)> = module
        .wires
        .iter()
        .enumerate()
        .filter(|(_, w)| w.port_input || w.port_output)
        .map(|(i, w)| (w.port_id, w.name.clone(), i))
        .collect();
    indexed.sort_by(|a, b| {
        let ka = (if a.0 == 0 { i64::MAX } else { a.0 }, &a.1);
        let kb = (if b.0 == 0 { i64::MAX } else { b.0 }, &b.1);
        ka.cmp(&kb)
    });
    for (n, (_, _, idx)) in indexed.iter().enumerate() {
        module.wires[*idx].port_id = n as i64 + 1;
    }
}
