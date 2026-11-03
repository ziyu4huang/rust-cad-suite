//! RTLIL 側提取與掃描子集(§4.2 D2):走 `Analysis.rtlil` 逃逸口。
//!
//! **v1 子集契約(誠實邊界,記入實作記錄)**:目標形=oracle proc 後的
//! -readable 形(corpus/ys-e2e case.il 正典形:root_case switch + sync
//! update,$0\\W 臨時名)。映射:
//! - timing=pos/neg sync 的 update 動作(下一拍寄存器語義直讀);
//! - reset=root_case 內 1-bit 選擇子 switch + 常量臂(復位臂判準;1-bit
//!   選擇子把 FSM 譯碼 switch[多 bit 狀態]分開);
//! - fsm=選擇子恰為本行程 update 寄存器的 switch,顯式臂(from=臂比較值,
//!   to=臂內該寄存器下一值);
//! - 破口 G1=root_case switch 無 default(空 compare)臂(RTLIL 語義=
//!   無匹配保持原值=鎖存,certain);G5=connect 自讀自環。
//! 其餘類別(counter/arithmetic/memory/…)在 RTLIL 形暫不產(oracle 臨時
//! 名遮蔽自身引用;屬後續弧),gap G2/G3/G4/G6/G7 同——**不硬湊**。
//! evidence=結構路徑證形(D2):file=輸入名,line=0,snippet="",path=
//! `module/<m>/process[i]/sync.posedge[j]` 形;V4 逐位元組溯源僅 SV 側。

use analyzer_core::RtlilInput;
use ry_ir::ast::{CaseRule, Process, SigBit, SigSpec, SyncType};
use std::collections::HashMap;

use crate::schema::{
    Category, Check, CheckForm, CheckPayload, Confidence, Effect, Evidence, EvidenceClass,
    GapClass, GapItem, GapReport, Qualifier, Rulebook, Trigger,
};
use crate::sv::{extract_parse_errors, finalize_module, Draft};

struct IlCtx {
    file_name: String,
}

/// SigSpec 渲染(ry-ir printer::log_signal;wire 寬度表=傾印同款裸名規則)。
fn render_sig(sig: &SigSpec, widths: &HashMap<&str, i64>) -> String {
    ry_ir::printer::log_signal(sig, &widths_of(sig, widths))
}

fn widths_of<'a>(sig: &'a SigSpec, widths: &'a HashMap<&str, i64>) -> HashMap<&'a str, i64> {
    // log_signal 的 widths 只影響裸名規則;直接給全表。
    let mut out = widths.clone();
    for b in &sig.bits {
        if let SigBit::Wire { wire, .. } = b {
            out.entry(wire.as_str()).or_insert(0);
        }
    }
    out
}

/// 選擇子是否 1-bit 純 wire(復位臂判準的一肢)。
fn is_one_bit_wire(sig: &SigSpec) -> bool {
    sig.size() == 1 && matches!(sig.bits.first(), Some(SigBit::Wire { .. }))
}

fn sig_const_value(sig: &SigSpec) -> Option<String> {
    // 純常數 SigSpec → 位串文本。
    if sig.bits.iter().all(|b| matches!(b, SigBit::Const(_))) && !sig.bits.is_empty() {
        let mut s = format!("{}'", sig.size());
        for b in &sig.bits {
            s.push(match b {
                SigBit::Const(ry_ir::ast::Bit::S0) => '0',
                SigBit::Const(ry_ir::ast::Bit::S1) => '1',
                SigBit::Const(ry_ir::ast::Bit::Sx) => 'x',
                SigBit::Const(ry_ir::ast::Bit::Sz) => 'z',
                SigBit::Const(ry_ir::ast::Bit::Sm) => 'm',
                SigBit::Const(ry_ir::ast::Bit::Sa) => 'a',
                _ => unreachable!(),
            });
        }
        Some(s)
    } else {
        None
    }
}

fn ev_path(file_name: &str, path: String, class: EvidenceClass) -> Evidence {
    Evidence {
        file: file_name.to_string(),
        line: 0,
        end_line: 0,
        class,
        snippet: String::new(),
        path: Some(path),
    }
}

pub(crate) fn extract_rules_rtlil(input: &RtlilInput, max_rules: usize) -> Rulebook {
    let analysis = analyzer_core::analyze(&analyzer_core::SourceInput::Rtlil(input.clone()));
    let parse_errors = extract_parse_errors(&analysis);
    let ctx = IlCtx {
        file_name: input.name.clone(),
    };
    let mut modules = Vec::new();
    let mut truncated = Vec::new();
    if let Some(design) = &analysis.rtlil {
        for m in &design.modules {
            let widths: HashMap<&str, i64> =
                m.wires.iter().map(|w| (w.name.as_str(), w.width)).collect();
            let mut drafts = Vec::new();
            for (pi, p) in m.processes.iter().enumerate() {
                extract_process(&ctx, &widths, m.name.as_str(), pi, p, &mut drafts);
            }
            modules.push(finalize_module(
                &input.name,
                m.name.as_str(),
                drafts,
                max_rules,
                &mut truncated,
            ));
        }
    }
    Rulebook {
        source: input.name.clone(),
        parse_errors,
        skipped: Vec::new(),
        modules,
        truncated,
    }
}

fn extract_process(
    ctx: &IlCtx,
    widths: &HashMap<&str, i64>,
    module: &str,
    proc_idx: usize,
    p: &Process,
    drafts: &mut Vec<Draft>,
) {
    // 觸發=首個 pos/neg sync。
    let trig = p.syncs.iter().find_map(|s| match s.stype {
        SyncType::Posedge | SyncType::Negedge => Some((
            match s.stype {
                SyncType::Posedge => "posedge",
                _ => "negedge",
            },
            s.signal
                .as_ref()
                .map(|g| render_sig(g, widths))
                .unwrap_or_else(|| "?".to_string()),
        )),
        _ => None,
    });
    let Some((trig_edge, trig_sig)) = trig else {
        return; // 純組合 process:v1 不產(sync 面才有寄存器語義)
    };

    // update 寄存器集(pos/neg sync 的 update lhs wire 名)。
    let update_wires: Vec<String> = p
        .syncs
        .iter()
        .filter(|s| matches!(s.stype, SyncType::Posedge | SyncType::Negedge))
        .flat_map(|s| {
            s.actions.iter().filter_map(|(l, _)| {
                l.bits.first().and_then(|b| match b {
                    SigBit::Wire { wire, offset } if *offset == 0 => Some(wire.clone()),
                    _ => None,
                })
            })
        })
        .collect();

    let trigger = Some(Trigger {
        edge: Some(trig_edge.to_string()),
        signal: Some(trig_sig.clone()),
    });

    // root_case 開關走查:reset(1-bit 選擇子+常量臂)與 fsm(選擇子=update 寄存器)。
    let mut switches: Vec<(usize, &ry_ir::ast::SwitchRule)> = Vec::new();
    collect_switches(&p.root_case, 0, &mut switches);
    for (sw_idx, sw) in &switches {
        let sel = render_sig(&sw.signal, widths);
        let sel_is_update = sw
            .signal
            .bits
            .first()
            .and_then(|b| match b {
                SigBit::Wire { wire, offset } if *offset == 0 => Some(wire.clone()),
                _ => None,
            })
            .map(|w| update_wires.iter().any(|u| u == &w))
            .unwrap_or(false);
        for (ci, arm) in sw.cases.iter().enumerate() {
            let path = format!(
                "module/{module}/process[{proc_idx}]/switch[{sw_idx}]/case[{ci}]"
            );
            let cmp_text = if arm.compare.is_empty() {
                String::new()
            } else {
                arm.compare
                    .iter()
                    .map(|c| render_sig(c, widths))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            // fsm:選擇子=update 寄存器,顯式臂內該寄存器的下一值。
            if sel_is_update && !arm.compare.is_empty() {
                let arm_ev = ev_path(&ctx.file_name, path.clone(), EvidenceClass::Case);
                for (l, r) in &arm.actions {
                    let lhs_name = l.bits.first().and_then(|b| match b {
                        SigBit::Wire { wire, .. } => Some(wire.clone()),
                        _ => None,
                    });
                    // 下一值 temp(`$0\\W…`)或直寫 W。
                    // 下一值 temp(`$0\W…`)或直寫 W:剝 `$0` 前綴後以
                    // update wire 名為前綴命中。
                    let is_next = lhs_name.as_ref().is_some_and(|n| {
                        update_wires.iter().any(|u| {
                            n == u
                                || n.strip_prefix("$0")
                                    .is_some_and(|rest| rest.starts_with(u.as_str()))
                        })
                    });
                    if !is_next {
                        continue;
                    }
                    let ev = arm_ev.clone();
                    let to = render_sig(r, widths);
                    let from = if cmp_text.is_empty() {
                        "?".to_string()
                    } else {
                        cmp_text.clone()
                    };
                    drafts.push(Draft {
                        category: Category::Fsm,
                        qualifier: Qualifier::Derived,
                        statement: format!(
                            "{sel}={from} 時,下一拍 {sel}={to}"
                        ),
                        evidence: vec![ev],
                        check: Check {
                            form: CheckForm::Transition,
                            payload: CheckPayload::Transition {
                                state_reg: sel.clone(),
                                from,
                                to,
                                when: None,
                            },
                        },
                    });
                }
                continue;
            }
            // reset:1-bit 選擇子 + 顯式比較 + 臂動作全為常量賦值。
            if is_one_bit_wire(&sw.signal) && !arm.compare.is_empty() && !arm.actions.is_empty() {
                let all_const = arm.actions.iter().all(|(_, r)| sig_const_value(r).is_some());
                if !all_const {
                    continue;
                }
                let arm_ev = ev_path(&ctx.file_name, path.clone(), EvidenceClass::Case);
                for (l, r) in &arm.actions {
                    let ev = arm_ev.clone();
                    let lhs = render_sig(l, widths);
                    let value = render_sig(r, widths);
                    drafts.push(Draft {
                        category: Category::Reset,
                        qualifier: Qualifier::Derived,
                        statement: format!(
                            "當 {sel} == {cmp_text} 且 {trig_sig} {zh} 時,{lhs} 裝載 {value}",
                            zh = if trig_edge == "posedge" { "上升沿" } else { "下降沿" }
                        ),
                        evidence: vec![ev],
                        check: Check {
                            form: CheckForm::Guard,
                            payload: CheckPayload::Guard {
                                trigger: trigger.clone(),
                                when: Some(format!("{sel} == {cmp_text}")),
                                effects: vec![Effect {
                                    lhs: lhs.clone(),
                                    value: value.clone(),
                                }],
                            },
                        },
                    });
                }
            }
        }
    }

    // timing:pos/neg sync update 動作(排除已被 fsm 消費的 update 寄存器)。
    let fsm_regs: Vec<String> = switches
        .iter()
        .filter(|(_, sw)| {
            sw.signal
                .bits
                .first()
                .and_then(|b| match b {
                    SigBit::Wire { wire, offset } if *offset == 0 => Some(wire.clone()),
                    _ => None,
                })
                .map(|w| update_wires.iter().any(|u| u == &w))
                .unwrap_or(false)
        })
        .filter_map(|(_, sw)| {
            sw.signal.bits.first().and_then(|b| match b {
                SigBit::Wire { wire, .. } => Some(wire.clone()),
                _ => None,
            })
        })
        .collect();
    for (si, s) in p.syncs.iter().enumerate() {
        if !matches!(s.stype, SyncType::Posedge | SyncType::Negedge) {
            continue;
        }
        let edge = match s.stype {
            SyncType::Posedge => "posedge",
            _ => "negedge",
        };
        let anchor = format!(
            "{} {}",
            s.signal
                .as_ref()
                .map(|g| render_sig(g, widths))
                .unwrap_or_else(|| "?".to_string()),
            if edge == "posedge" { "上升沿" } else { "下降沿" }
        );
        for (ai, (l, r)) in s.actions.iter().enumerate() {
            let lhs_name = l.bits.first().and_then(|b| match b {
                SigBit::Wire { wire, .. } => Some(wire.clone()),
                _ => None,
            });
            if lhs_name
                .as_ref()
                .map(|n| fsm_regs.iter().any(|f| f == n))
                .unwrap_or(false)
            {
                continue; // fsm 面已覆蓋
            }
            let path = format!(
                "module/{module}/process[{proc_idx}]/sync.{edge}[{si}]/action[{ai}]"
            );
            let ev = ev_path(&ctx.file_name, path, EvidenceClass::Process);
            let lhs = render_sig(l, widths);
            let rhs = render_sig(r, widths);
            let anchor = anchor.clone();
            drafts.push(Draft {
                category: Category::Timing,
                qualifier: Qualifier::Derived,
                statement: format!("{anchor} 成立後的下一拍,{lhs} 變為 {rhs}"),
                evidence: vec![ev],
                check: Check {
                    form: CheckForm::Sequence,
                    payload: CheckPayload::Sequence {
                        signal: lhs,
                        becomes: rhs,
                        anchor,
                        latency: 1,
                        when: None,
                    },
                },
            });
        }
    }
}

fn collect_switches<'a>(
    case: &'a CaseRule,
    depth: usize,
    out: &mut Vec<(usize, &'a ry_ir::ast::SwitchRule)>,
) {
    if depth > 32 {
        return; // 防禦深度(結構樹界)
    }
    for (i, sw) in case.switches.iter().enumerate() {
        out.push((i, sw));
        for arm in &sw.cases {
            collect_switches(arm, depth + 1, out);
        }
    }
}

pub(crate) fn scan_gaps_rtlil(input: &RtlilInput) -> GapReport {
    let analysis = analyzer_core::analyze(&analyzer_core::SourceInput::Rtlil(input.clone()));
    let parse_errors = extract_parse_errors(&analysis);
    let mut items = Vec::new();
    if let Some(design) = &analysis.rtlil {
        for m in &design.modules {
            let widths: HashMap<&str, i64> =
                m.wires.iter().map(|w| (w.name.as_str(), w.width)).collect();
            // G1:root_case switch 無 default 臂(RTLIL 語義=保持值=鎖存)。
            for (pi, p) in m.processes.iter().enumerate() {
                let mut switches: Vec<(usize, &ry_ir::ast::SwitchRule)> = Vec::new();
                collect_switches(&p.root_case, 0, &mut switches);
                for (sw_idx, sw) in &switches {
                    if sw.cases.iter().any(|a| a.compare.is_empty()) {
                        continue;
                    }
                    items.push(GapItem {
                        id: format!("G1-{}-{:03}", m.name, items.len() + 1),
                        class: GapClass::Latch,
                        module: m.name.clone(),
                        confidence: Confidence::Certain,
                        message: format!(
                            "RTLIL switch {} 無 default 臂:無匹配編碼時全部動作保持原值(鎖存語義直讀)",
                            render_sig(&sw.signal, &widths)
                        ),
                        evidence: vec![Evidence {
                            file: input.name.clone(),
                            line: 0,
                            end_line: 0,
                            class: EvidenceClass::Case,
                            snippet: String::new(),
                            path: Some(format!(
                                "module/{}/process[{pi}]/switch[{sw_idx}]",
                                m.name
                            )),
                        }],
                        suggested_test: format!(
                            "施加 switch {} 未覆蓋編碼,釘住保持值(鎖存)或補 default 臂",
                            render_sig(&sw.signal, &widths)
                        ),
                    });
                }
            }
            // G5:connect 自讀自環(lhs 與 rhs 含同名 wire)。
            for (ci, (l, r)) in m.connections.iter().enumerate() {
                let l_wires: Vec<&str> = l
                    .bits
                    .iter()
                    .filter_map(|b| match b {
                        SigBit::Wire { wire, .. } => Some(wire.as_str()),
                        _ => None,
                    })
                    .collect();
                let self_loop = r.bits.iter().any(|b| match b {
                    SigBit::Wire { wire, .. } => l_wires.contains(&wire.as_str()),
                    _ => false,
                });
                if self_loop {
                    items.push(GapItem {
                        id: format!("G5-{}-{:03}", m.name, items.len() + 1),
                        class: GapClass::CombLoop,
                        module: m.name.clone(),
                        confidence: Confidence::Certain,
                        message: format!(
                            "connect 自環:{} 的賦值讀取自身(振盪/未定義)",
                            l_wires.join("/")
                        ),
                        evidence: vec![Evidence {
                            file: input.name.clone(),
                            line: 0,
                            end_line: 0,
                            class: EvidenceClass::Assign,
                            snippet: String::new(),
                            path: Some(format!("module/{}/connect[{ci}]", m.name)),
                        }],
                        suggested_test: "驅動環上訊號後立即觀測,斷言振盪/鎖死可觀測".to_string(),
                    });
                }
            }
        }
    }
    GapReport {
        source: input.name.clone(),
        parse_errors,
        skipped: Vec::new(),
        items,
    }
}
