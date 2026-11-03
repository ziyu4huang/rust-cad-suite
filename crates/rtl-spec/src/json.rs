//! JSON 寫出(**只寫不讀**;鍵序=寫出序 byte-exact 契約,單測釘死)。
//!
//! 編碼器復用 `analyzer_core::proto::json::Json`(寫出與 analyzer-core
//! report.rs 同形同源;轉義律由 ASP/1 codec 向量單測背書;D12/R1-5 拒的
//! 是 ::parse——本模組零讀取路徑,opts 標量化後庫內無 JSON parser 依賴)。

use analyzer_core::proto::json::Json;

use crate::schema::{
    AnalysisSummary, CheckPayload, CheckForm, Evidence, GapClass, GapItem, GapReport, ModuleRules,
    ModuleSummary, Rule, Rulebook, SkippedNote,
};

fn s(v: impl Into<String>) -> Json {
    Json::Str(v.into())
}
fn i(v: u32) -> Json {
    Json::Int(i64::from(v))
}
fn b(v: bool) -> Json {
    Json::Bool(v)
}

/// 溢出結構:`{"error": "..."}`(wasm 面「錯誤內嵌,不拋」契約)。
pub fn error_json(msg: impl Into<String>) -> String {
    Json::Obj(vec![("error".to_string(), s(msg))]).render()
}

fn opt_str(v: &Option<String>) -> Json {
    match v {
        Some(x) => s(x.clone()),
        None => Json::Null,
    }
}

fn evidence_json(e: &Evidence) -> Json {
    let mut fields = vec![
        ("file".to_string(), s(e.file.clone())),
        ("line".to_string(), i(e.line)),
        ("end_line".to_string(), i(e.end_line)),
        ("class".to_string(), s(e.class.as_str())),
        ("snippet".to_string(), s(e.snippet.clone())),
    ];
    // path 欄=RTLIL 結構路徑證形專屬(D2);SV 側不寫(§2.1)。
    if let Some(p) = &e.path {
        fields.push(("path".to_string(), s(p.clone())));
    }
    Json::Obj(fields)
}

fn check_json(form: CheckForm, payload: &CheckPayload) -> Json {
    let mut fields = vec![("form".to_string(), s(form.as_str()))];
    fields.extend(payload_fields(payload));
    Json::Obj(fields)
}

fn payload_fields(payload: &CheckPayload) -> Vec<(String, Json)> {
    match payload {
        CheckPayload::Guard {
            trigger,
            when,
            effects,
        } => {
            let mut fields = vec![(
                "trigger".to_string(),
                match trigger {
                    Some(t) => Json::Obj(vec![
                        ("edge".to_string(), opt_str(&t.edge)),
                        ("signal".to_string(), opt_str(&t.signal)),
                    ]),
                    None => Json::Null,
                },
            )];
            fields.push(("when".to_string(), opt_str(when)));
            fields.push((
                "effects".to_string(),
                Json::Arr(
                    effects
                        .iter()
                        .map(|e| {
                            Json::Obj(vec![
                                ("lhs".to_string(), s(e.lhs.clone())),
                                ("value".to_string(), s(e.value.clone())),
                            ])
                        })
                        .collect(),
                ),
            ));
            fields
        }
        CheckPayload::Transition {
            state_reg,
            from,
            to,
            when,
        } => vec![
            ("state_reg".to_string(), s(state_reg.clone())),
            ("from".to_string(), s(from.clone())),
            ("to".to_string(), s(to.clone())),
            ("when".to_string(), opt_str(when)),
        ],
        CheckPayload::Sequence {
            signal,
            becomes,
            anchor,
            latency,
            when,
        } => vec![
            ("signal".to_string(), s(signal.clone())),
            ("becomes".to_string(), s(becomes.clone())),
            (
                "anchor".to_string(),
                Json::Obj(vec![("expr".to_string(), s(anchor.clone()))]),
            ),
            ("latency".to_string(), i(*latency)),
            ("when".to_string(), opt_str(when)),
        ],
        CheckPayload::Relation {
            lhs,
            rhs,
            when,
            width_note,
        } => vec![
            ("lhs".to_string(), s(lhs.clone())),
            ("rhs".to_string(), s(rhs.clone())),
            // 適用域(R3-②):分支域恆等式攜 when;全域=None。
            ("when".to_string(), opt_str(when)),
            ("width_note".to_string(), opt_str(width_note)),
        ],
        CheckPayload::Invariant {
            subject,
            kind,
            values,
            expr,
        } => vec![
            ("subject".to_string(), s(subject.clone())),
            ("kind".to_string(), s(kind.as_str())),
            (
                "values".to_string(),
                Json::Arr(values.iter().map(|v| s(v.clone())).collect()),
            ),
            ("expr".to_string(), opt_str(expr)),
        ],
    }
}

fn rule_json(r: &Rule) -> Json {
    Json::Obj(vec![
        ("id".to_string(), s(r.id.clone())),
        ("category".to_string(), s(r.category.as_str())),
        ("module".to_string(), s(r.module.clone())),
        ("statement".to_string(), s(r.statement.clone())),
        ("qualifier".to_string(), s(r.qualifier.as_str())),
        (
            "evidence".to_string(),
            Json::Arr(r.evidence.iter().map(evidence_json).collect()),
        ),
        (
            "check".to_string(),
            check_json(r.check.form, &r.check.payload),
        ),
        ("fingerprint".to_string(), s(r.fingerprint.clone())),
    ])
}

pub fn rulebook_json(rb: &Rulebook) -> String {
    let v = Json::Obj(vec![
        ("source".to_string(), s(rb.source.clone())),
        (
            "parse_errors".to_string(),
            Json::Arr(rb.parse_errors.iter().map(|e| s(e.clone())).collect()),
        ),
        (
            "skipped".to_string(),
            Json::Arr(rb.skipped.iter().map(skipped_json).collect()),
        ),
        (
            "modules".to_string(),
            Json::Arr(rb.modules.iter().map(module_rules_json).collect()),
        ),
        (
            "truncated".to_string(),
            Json::Arr(rb.truncated.iter().map(|t| s(t.clone())).collect()),
        ),
    ]);
    v.render()
}

fn skipped_json(n: &SkippedNote) -> Json {
    Json::Obj(vec![
        ("file".to_string(), s(n.file.clone())),
        ("reason".to_string(), s(n.reason.as_str())),
    ])
}

fn module_rules_json(m: &ModuleRules) -> Json {
    Json::Obj(vec![
        ("module".to_string(), s(m.module.clone())),
        (
            "rules".to_string(),
            Json::Arr(m.rules.iter().map(rule_json).collect()),
        ),
    ])
}

pub fn gap_report_json(gr: &GapReport) -> String {
    let v = Json::Obj(vec![
        ("source".to_string(), s(gr.source.clone())),
        (
            "parse_errors".to_string(),
            Json::Arr(gr.parse_errors.iter().map(|e| s(e.clone())).collect()),
        ),
        (
            "skipped".to_string(),
            Json::Arr(gr.skipped.iter().map(skipped_json).collect()),
        ),
        (
            "items".to_string(),
            Json::Arr(gr.items.iter().map(gap_item_json).collect()),
        ),
    ]);
    v.render()
}

fn gap_item_json(g: &GapItem) -> Json {
    Json::Obj(vec![
        ("id".to_string(), s(g.id.clone())),
        (
            "class".to_string(),
            s(GapClass::as_str(g.class)),
        ),
        ("module".to_string(), s(g.module.clone())),
        (
            "confidence".to_string(),
            s(
                crate::schema::Confidence::as_str(g.confidence),
            ),
        ),
        ("message".to_string(), s(g.message.clone())),
        (
            "evidence".to_string(),
            Json::Arr(g.evidence.iter().map(evidence_json).collect()),
        ),
        ("suggested_test".to_string(), s(g.suggested_test.clone())),
    ])
}

pub fn analysis_json(sum: &AnalysisSummary) -> String {
    let v = Json::Obj(vec![
        ("source".to_string(), s(sum.source.clone())),
        ("has_errors".to_string(), b(sum.has_errors)),
        ("diagnostics_count".to_string(), Json::Int(sum.diagnostics_count as i64)),
        (
            "modules".to_string(),
            Json::Arr(sum.modules.iter().map(module_summary_json).collect()),
        ),
        (
            "instance_edges".to_string(),
            Json::Arr(
                sum.instance_edges
                    .iter()
                    .map(|(a, b_)| Json::Arr(vec![s(a.clone()), s(b_.clone())]))
                    .collect(),
            ),
        ),
    ]);
    v.render()
}

fn module_summary_json(m: &ModuleSummary) -> Json {
    Json::Obj(vec![
        ("name".to_string(), s(m.name.clone())),
        (
            "params".to_string(),
            Json::Arr(
                m.params
                    .iter()
                    .map(|(n, v)| {
                        Json::Obj(vec![
                            ("name".to_string(), s(n.clone())),
                            ("value".to_string(), s(v.clone())),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "ports".to_string(),
            Json::Arr(
                m.ports
                    .iter()
                    .map(|(n, d, w, sg)| {
                        Json::Obj(vec![
                            ("name".to_string(), s(n.clone())),
                            ("dir".to_string(), s(d.clone())),
                            ("width".to_string(), i(*w)),
                            ("signed".to_string(), b(*sg)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "nets".to_string(),
            Json::Arr(
                m.nets
                    .iter()
                    .map(|(n, k, w, ip)| {
                        Json::Obj(vec![
                            ("name".to_string(), s(n.clone())),
                            ("kind".to_string(), s(k.clone())),
                            ("width".to_string(), i(*w)),
                            ("is_port".to_string(), b(*ip)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "cells".to_string(),
            Json::Arr(
                m.cells
                    .iter()
                    .map(|(n, t, pc)| {
                        Json::Obj(vec![
                            ("name".to_string(), s(n.clone())),
                            ("ctype".to_string(), s(t.clone())),
                            ("port_count".to_string(), Json::Int(*pc as i64)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("proc_count".to_string(), Json::Int(m.proc_count as i64)),
        ("assign_count".to_string(), Json::Int(m.assign_count as i64)),
    ])
}
