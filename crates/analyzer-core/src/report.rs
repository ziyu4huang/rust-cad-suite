//! §13 負載字典的 Rust 端編碼:`Analysis` → ASP/1 analyze result(Json)。
//! 語義能力落 core(10.7):span 解析消費 `Analysis.sm`(pub(crate)),
//! server 只做 face;任何未來面(wasm/FFI)直呼本模組零複製。
//!
//! 字典鍵序=寫出序(§5.3 律 4,向量 byte-exact 的前提):
//! * result:has_errors, modules, diagnostics, diags_text
//! * module:name, params, ports, nets, cells, proc_count, assign_count, span
//! * port:name, dir, width, parametric, signed, span
//!   (parametric 較 §13 草圖多一列——§12-R8 明載 TS 型別帶 `parametric?: true`,
//!   以 §4.2 PortInfo 欄位集為準;記入 §17 實作記錄)
//! * net:name, kind, width, is_port, unpacked_dims, span
//! * cell:name, ctype, port_count, span
//! * diagnostic:severity, code, message, span, notes:[{message, span}]
//! * Span:{file, start_line, start_col, end_line, end_col}(缺省=Null)

use crate::proto::json::Json;
use crate::view::{CellInfo, ModuleInfo, NetInfo, PortInfo, SpanInfo};
use crate::Analysis;
use rcs_diag::{SourceMap, Span};

/// analyze op 的 result 對象(§13)。
pub fn analysis_result(a: &Analysis) -> Json {
    let sm = a.sm.as_ref();
    Json::Obj(vec![
        ("has_errors".to_string(), Json::Bool(a.has_errors())),
        (
            "modules".to_string(),
            Json::Arr(a.modules.iter().map(module_json).collect()),
        ),
        (
            "diagnostics".to_string(),
            Json::Arr(a.diagnostics.iter().map(|d| diagnostic_json(d, sm)).collect()),
        ),
        ("diags_text".to_string(), Json::Str(a.diag_text.clone())),
    ])
}

fn module_json(m: &ModuleInfo) -> Json {
    Json::Obj(vec![
        ("name".to_string(), Json::Str(m.name.clone())),
        (
            "params".to_string(),
            Json::Arr(
                m.params
                    .iter()
                    .map(|p| {
                        Json::Obj(vec![
                            ("name".to_string(), Json::Str(p.name.clone())),
                            ("value".to_string(), Json::Str(p.value.clone())),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("ports".to_string(), Json::Arr(m.ports.iter().map(port_json).collect())),
        ("nets".to_string(), Json::Arr(m.nets.iter().map(net_json).collect())),
        ("cells".to_string(), Json::Arr(m.cells.iter().map(cell_json).collect())),
        ("proc_count".to_string(), Json::Int(m.proc_count as i64)),
        ("assign_count".to_string(), Json::Int(m.assign_count as i64)),
        ("span".to_string(), spaninfo_json(&m.span)),
    ])
}

fn port_json(p: &PortInfo) -> Json {
    Json::Obj(vec![
        ("name".to_string(), Json::Str(p.name.clone())),
        ("dir".to_string(), Json::Str(p.dir.as_str().to_string())),
        ("width".to_string(), Json::Int(i64::from(p.width))),
        ("parametric".to_string(), Json::Bool(p.parametric)),
        ("signed".to_string(), Json::Bool(p.signed)),
        ("span".to_string(), spaninfo_json(&p.span)),
    ])
}

fn net_json(n: &NetInfo) -> Json {
    Json::Obj(vec![
        ("name".to_string(), Json::Str(n.name.clone())),
        ("kind".to_string(), Json::Str(n.kind.as_str().to_string())),
        ("width".to_string(), Json::Int(i64::from(n.width))),
        ("is_port".to_string(), Json::Bool(n.is_port)),
        ("unpacked_dims".to_string(), Json::Int(i64::from(n.unpacked_dims))),
        ("span".to_string(), spaninfo_json(&n.span)),
    ])
}

fn cell_json(c: &CellInfo) -> Json {
    Json::Obj(vec![
        ("name".to_string(), Json::Str(c.name.clone())),
        ("ctype".to_string(), Json::Str(c.ctype.clone())),
        ("port_count".to_string(), Json::Int(c.port_count as i64)),
        ("span".to_string(), spaninfo_json(&c.span)),
    ])
}

fn diagnostic_json(
    d: &rcs_diag::Diagnostic,
    sm: Option<&SourceMap>,
) -> Json {
    Json::Obj(vec![
        (
            "severity".to_string(),
            Json::Str(d.severity.as_str().to_string()),
        ),
        ("code".to_string(), Json::Str(d.code.to_string())),
        ("message".to_string(), Json::Str(d.message.clone())),
        ("span".to_string(), raw_span_json(sm, &d.span)),
        (
            "notes".to_string(),
            Json::Arr(
                d.notes
                    .iter()
                    .map(|(msg, span)| {
                        Json::Obj(vec![
                            ("message".to_string(), Json::Str(msg.clone())),
                            ("span".to_string(), match span {
                                Some(s) => raw_span_json(sm, s),
                                None => Json::Null,
                            }),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

fn spaninfo_json(s: &Option<SpanInfo>) -> Json {
    match s {
        Some(s) => Json::Obj(vec![
            ("file".to_string(), Json::Str(s.file.clone())),
            ("start_line".to_string(), Json::Int(i64::from(s.start_line))),
            ("start_col".to_string(), Json::Int(i64::from(s.start_col))),
            ("end_line".to_string(), Json::Int(i64::from(s.end_line))),
            ("end_col".to_string(), Json::Int(i64::from(s.end_col))),
        ]),
        None => Json::Null,
    }
}

/// 原始診斷 span → Span Json(行列查詢經既有 pub `SourceMap::line_col`;
/// sm 不在場(RTLIL)或文件未註冊 → Null)。
fn raw_span_json(sm: Option<&SourceMap>, span: &Span) -> Json {
    let Some(sm) = sm else { return Json::Null };
    if !sm.has(span.file) {
        return Json::Null;
    }
    let (start_line, start_col) = sm.line_col(span.file, span.start);
    let (end_line, end_col) = sm.line_col(span.file, span.end);
    Json::Obj(vec![
        ("file".to_string(), Json::Str(sm.file_name(span.file).to_string())),
        ("start_line".to_string(), Json::Int(i64::from(start_line))),
        ("start_col".to_string(), Json::Int(i64::from(start_col))),
        ("end_line".to_string(), Json::Int(i64::from(end_line))),
        ("end_col".to_string(), Json::Int(i64::from(end_col))),
    ])
}
