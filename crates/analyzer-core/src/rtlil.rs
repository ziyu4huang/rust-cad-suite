//! RTLIL 投影:ry-ir Design → `view::ModuleInfo`(ANALYZER-SERVICE §4.2;
//! 既有欄位直讀,零新語義)。語義不對稱如實呈現(§4.3):parse_design 是
//! 首錯硬斷 → 錯誤時單條 AN-E001 診斷 + 零模組,禁為對稱改 ry-ir 語義。

use ry_ir::{Bit, Const, RyError};

use crate::view::{
    CellInfo, Dir, ModuleInfo, NetInfo, NetKindTag, ParamInfo, PortInfo,
};
use crate::{Analysis, InputKind, RtlilInput};

/// RTLIL 管線:ry-ir::parse_rtlil 直呼 + 投影;RyError → 單條 AN-E001
/// (message 帶 ry-ir 原文,§4.4;span 指到錯誤行首,1-based)。
pub(crate) fn analyze_rtlil(input: &RtlilInput) -> Analysis {
    let mut sm = rcs_diag::SourceMap::new();
    let file0 = sm.add(input.name.clone(), input.source.clone());
    let mut modules = Vec::new();
    let mut diagnostics = Vec::new();
    // 空輸入=零模組空 design(oracle read_rtlil 空檔同義)。上游 Cursor
    // 對字面空串會 index panic(lexer.rs:81 lines=[]),ry-ir 零接觸
    //(§11-8),守衛落本層;非空文本(含僅空白/註釋)不受影響。
    let design = if input.source.is_empty() {
        Some(ry_ir::Design::default())
    } else {
        match ry_ir::parse_rtlil(&input.source) {
            Ok(d) => {
                modules = project(&d);
                Some(d)
            }
            Err(e) => {
                let line = match &e {
                    RyError::Parse { line, .. } | RyError::Unsupported { line, .. } => *line,
                };
                let pos = sm.line_start_offset(file0, (line as u32).max(1)) as u32;
                let span = rcs_diag::Span::point(file0, pos);
                diagnostics.push(rcs_diag::Diagnostic::error("AN-E001", span, e.to_string()));
                None
            }
        }
    };
    let mut text = String::new();
    rcs_diag::render_diags(&mut text, &sm, &diagnostics);
    Analysis {
        kind: InputKind::Rtlil,
        diagnostics,
        diag_text: text,
        modules,
        sv: None,
        rtlil: design,
        // RTLIL 側無 SourceMap(§4.2:行列查詢=Err("no span info");診斷
        // 渲染已用局部 sm 完成,Analysis 不再持有)。
        sm: None,
    }
}

pub(crate) fn project(design: &ry_ir::Design) -> Vec<ModuleInfo> {
    design.modules.iter().map(project_module).collect()
}

fn project_module(m: &ry_ir::Module) -> ModuleInfo {
    // 參數:avail_parameters(宣告序)+ parameter_defaults;無預設值 →
    // 空串(§4.5)。
    let mut params: Vec<ParamInfo> = m
        .avail_parameters
        .iter()
        .map(|id| ParamInfo {
            name: id.clone(),
            value: m.parameter_defaults.get(id).map(render_const).unwrap_or_default(),
        })
        .collect();
    // 防禦面:defaults 有而 avail 無(parser 構造上不會發生,補尾不丟名)。
    for (id, c) in &m.parameter_defaults.entries {
        if !m.avail_parameters.contains(id) {
            params.push(ParamInfo { name: id.clone(), value: render_const(c) });
        }
    }

    // 端口:port_id>0 的 wires,按 port_id 升序(fixup_ports 後=宣告序重編號)。
    let mut ported: Vec<&ry_ir::Wire> = m.wires.iter().filter(|w| w.port_id > 0).collect();
    ported.sort_by_key(|w| w.port_id);
    let ports: Vec<PortInfo> = ported
        .iter()
        .map(|w| PortInfo {
            name: w.name.clone(),
            dir: match (w.port_input, w.port_output) {
                (true, false) => Dir::Input,
                (false, true) => Dir::Output,
                // both=true→Inout(§4.2);neither 理論不存在(fixup 歸零),併 Inout。
                _ => Dir::Inout,
            },
            width: clamp_u32(w.width),
            parametric: false, // RTLIL 寬度恆顯式
            signed: w.is_signed,
            span: None,
        })
        .collect();

    // nets:wires 全量(含端口;is_port=port_id>0)+ memories(kind="memory")。
    let mut nets: Vec<NetInfo> = m
        .wires
        .iter()
        .map(|w| NetInfo {
            name: w.name.clone(),
            kind: NetKindTag::wire(),
            width: clamp_u32(w.width),
            is_port: w.port_id > 0,
            unpacked_dims: 0,
            span: None,
        })
        .collect();
    for mem in &m.memories {
        nets.push(NetInfo {
            name: mem.name.clone(),
            kind: NetKindTag::memory(),
            width: clamp_u32(mem.width),
            is_port: false,
            unpacked_dims: 0,
            span: None,
        });
    }

    let cells: Vec<CellInfo> = m
        .cells
        .iter()
        .map(|c| CellInfo {
            name: c.name.clone(),
            ctype: c.ctype.clone(),
            port_count: c.connections.len(),
            span: None,
        })
        .collect();

    ModuleInfo {
        name: m.name.clone(),
        params,
        ports,
        nets,
        cells,
        proc_count: m.processes.len(),
        assign_count: m.connections.len(),
        span: None, // ry-ir AST 無 span(風險 R1/D10)
    }
}

fn clamp_u32(w: i64) -> u32 {
    w.clamp(0, u32::MAX as i64) as u32
}

/// §4.5:Bits → 十六進制 `N's?h<hex>`(寬度精確 ceil(w/4) 位,前導零保留;
/// 含 x/z/m/a 非 01 位 → `N'b<逐位字符>` MSB-first,hex 碼位表達不了它們);
/// Str → 帶引號(反斜杠與引號轉義)。
pub(crate) fn render_const(c: &Const) -> String {
    match c {
        Const::Str(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        Const::Bits { width, bits, signed, .. } => {
            let mut s = format!("{width}'");
            if *signed {
                s.push('s');
            }
            let all01 = bits.iter().all(|b| matches!(b, Bit::S0 | Bit::S1));
            if all01 {
                s.push('h');
                let nibbles = ((*width as usize).div_ceil(4)).max(1);
                for i in (0..nibbles).rev() {
                    let mut v = 0u8;
                    for j in 0..4 {
                        let idx = i * 4 + j;
                        if idx < bits.len() && bits[idx] == Bit::S1 {
                            v |= 1 << j;
                        }
                    }
                    s.push(char::from_digit(v as u32, 16).unwrap_or('0'));
                }
            } else {
                s.push('b');
                for b in bits.iter().rev() {
                    s.push(b.as_char());
                }
            }
            s
        }
    }
}
