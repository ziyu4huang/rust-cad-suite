//! SV 側提取器(§4.2):走 `Analysis.sv` 逃逸口,五基類
//! reset/fsm/timing/arithmetic/counter;行程分類律=R1-1 三肢判準。
//!
//! 證據行號通路:pp 對主檔 `SourceMap::add(name, src)` 註冊**原始文本**
//! (pp.rs:63),主檔恆 FileId(0);span 行號經 `Analysis::line_col`
//! (公開面),snippet 按 line 區間切**原始源文本**——V4 逐位元組對拍的
//! 兩端同一定義。巨集/include 產生的 span(file≠0)不產證據:整規則
//! 無證即丟(誠實降級,記入實作記錄)。

use std::collections::HashSet;

pub(crate) mod gaps;

use analyzer_core::{Analysis, SourceInput, SvInput};
use rcs_diag::{FileId, Span};
use vl_syntax::ast::{self, EdgeKind, EventCtrl, ModuleItem, Stmt};

use crate::expr::{self, lvalue_base, render, walk_expr};
use crate::schema::{
    Category, Check, CheckForm, CheckPayload, Effect, Evidence, EvidenceClass, ModuleRules,
    Qualifier, Rule, Rulebook, SkippedNote, SkipReason, Trigger,
};

/// TB 啟發判定(R7b):檔含 `$finish` → 整檔跳過(注記入 skipped,不入
/// parse_errors);TB 不是被提取對象。
pub(crate) fn is_tb_file(source: &str) -> bool {
    source.contains("$finish")
}

/// parse_errors 僅收解析錯誤(R2-6:TB/未支援注記走 skipped 欄)。
pub(crate) fn extract_parse_errors(analysis: &Analysis) -> Vec<String> {
    analysis
        .diagnostics()
        .iter()
        .filter(|d| d.severity == rcs_diag::Severity::Error)
        .map(|d| format!("{}: {}", d.code, d.message))
        .collect()
}

/// 原始源文本按行切(行原文,去行尾 `\n` 與 `\r`;V4 拼接規則的行定義,
/// 生成端與 TS checker 同一契約)。
pub(crate) fn split_lines(source: &str) -> Vec<String> {
    source
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l).to_string())
        .collect()
}

pub(crate) struct Ctx<'a> {
    pub file_name: String,
    pub lines: Vec<String>,
    pub analysis: &'a Analysis,
}

impl Ctx<'_> {
    /// span → Evidence(主檔 span 才產;巨集/include span=None)。
    /// 行號規則:end<start(跨檔 merge 垃圾)→ 單行;end 恰在行首
    /// (col==1)→ 上一行(行終止符歸屬);end 鉗到檔尾。
    pub fn evidence(&self, span: Span, class: EvidenceClass) -> Option<Evidence> {
        if span.file.0 != 0 {
            return None;
        }
        let total = self.lines.len() as u32;
        let (sl, _) = self.analysis.line_col(FileId(0), span.start.0).ok()?;
        let mut el = sl;
        let mut ecol = 1u32;
        if span.end.0 >= span.start.0 {
            if let Ok((l, c)) = self.analysis.line_col(FileId(0), span.end.0) {
                el = l;
                ecol = c;
            }
        }
        if ecol == 1 && el > sl {
            el -= 1;
        }
        el = el.max(sl).min(total.max(1));
        let start_idx = (sl as usize).saturating_sub(1).min(self.lines.len().saturating_sub(1));
        let end_idx = (el as usize).clamp(start_idx, self.lines.len());
        let snippet = self.lines[start_idx..end_idx].join("\n");
        Some(Evidence {
            file: self.file_name.clone(),
            line: sl,
            end_line: el,
            class,
            snippet,
            path: None,
        })
    }
}

// ---- 行程分類律(R1-1;對齊 parser 實形) ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProcClass {
    /// edges 空(`@*`/`@(*)`)∨ 全 Any(`@(a or b)`)∨ ctrl=None(裸 always)。
    Comb,
    /// edges 含 Posedge/Negedge(或 Edge)。
    Edge,
}

pub(crate) fn classify_proc(ctrl: Option<&EventCtrl>) -> ProcClass {
    match ctrl {
        None => ProcClass::Comb,
        Some(c) => {
            if c.edges.is_empty() || c.edges.iter().all(|e| e.kind == EdgeKind::Any) {
                ProcClass::Comb
            } else {
                ProcClass::Edge
            }
        }
    }
}

// ---- 生成塊扁平走查(param/arithmetic 的 generate 語境追蹤) ----

pub(crate) fn walk_items<'a>(
    items: &'a [ModuleItem],
    in_generate: bool,
    f: &mut impl FnMut(&'a ModuleItem, bool),
) {
    for it in items {
        match it {
            ModuleItem::GenerateRegion(inner) => walk_items(inner, true, f),
            ModuleItem::GenIf { then, els, .. } => {
                walk_items(then, true, f);
                if let Some(e) = els {
                    walk_items(e, true, f);
                }
            }
            ModuleItem::GenFor { items, .. } => walk_items(items, true, f),
            ModuleItem::GenCase { items, default, .. } => {
                for (_, its) in items {
                    walk_items(its, true, f);
                }
                if let Some(d) = default {
                    walk_items(d, true, f);
                }
            }
            ModuleItem::GenBlock { items, .. } => walk_items(items, true, f),
            _ => f(it, in_generate),
        }
    }
}

/// Stmt 的 span(AST 無 Stmt::span();全變體帶 span 欄位的機械直取)。
pub(crate) fn stmt_span(s: &Stmt) -> Span {
    match s {
        Stmt::Block { span, .. }
        | Stmt::BAssign { span, .. }
        | Stmt::NbAssign { span, .. }
        | Stmt::IntraAssign { span, .. }
        | Stmt::If { span, .. }
        | Stmt::Case { span, .. }
        | Stmt::For { span, .. }
        | Stmt::While { span, .. }
        | Stmt::Repeat { span, .. }
        | Stmt::Forever { span, .. }
        | Stmt::DelayCtrl { span, .. }
        | Stmt::EventCtrl { span, .. }
        | Stmt::SysCall { span, .. }
        | Stmt::SubCall { span, .. }
        | Stmt::Wait { span, .. }
        | Stmt::Fork { span, .. }
        | Stmt::Force { span, .. }
        | Stmt::Release { span, .. }
        | Stmt::Deassign { span, .. }
        | Stmt::ProcContAssign { span, .. }
        | Stmt::Trigger { span, .. }
        | Stmt::Disable { span, .. }
        | Stmt::Null { span } => *span,
    }
}

// ---- 語句扁平走查(條件鏈攜帶;timing/fsm/comb 的共同骨架) ----

pub(crate) struct FlatAssign<'a> {
    pub lhs: &'a ast::Expr,
    pub rhs: &'a ast::Expr,
    pub span: Span,
    pub nb: bool,
    /// 條件鏈(渲染文本,外→內;case 臂=`sel == label` 形)。
    pub conds: Vec<String>,
}

pub(crate) fn walk_stmt<'a>(
    stmt: &'a Stmt,
    conds: &mut Vec<String>,
    out: &mut Vec<FlatAssign<'a>>,
) {
    match stmt {
        Stmt::Block { body, .. } => {
            for s in body {
                walk_stmt(s, conds, out);
            }
        }
        Stmt::If { cond, then, els, .. } => {
            conds.push(render(cond));
            walk_stmt(then, conds, out);
            conds.pop();
            if let Some(e) = els {
                conds.push(format!("!({})", render(cond)));
                walk_stmt(e, conds, out);
                conds.pop();
            }
        }
        Stmt::Case { expr, items, .. } => {
            for item in items {
                match item {
                    ast::CaseItem::Items(labels, s) => {
                        if let Some(first) = labels.first() {
                            conds.push(format!("{} == {}", render(expr), render(first)));
                            walk_stmt(s, conds, out);
                            conds.pop();
                        } else {
                            walk_stmt(s, conds, out);
                        }
                    }
                    ast::CaseItem::Default(s) => walk_stmt(s, conds, out),
                }
            }
        }
        Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::Repeat { body, .. }
        | Stmt::Forever { body, .. } => walk_stmt(body, conds, out),
        Stmt::DelayCtrl { body, .. } | Stmt::EventCtrl { body, .. } => {
            if let Some(b) = body {
                walk_stmt(b, conds, out);
            }
        }
        Stmt::Wait { body, .. } => {
            if let Some(b) = body {
                walk_stmt(b, conds, out);
            }
        }
        Stmt::Fork { branches, .. } => {
            for b in branches {
                walk_stmt(b, conds, out);
            }
        }
        Stmt::BAssign { lhs, rhs, span, .. } | Stmt::NbAssign { lhs, rhs, span, .. } => {
            out.push(FlatAssign {
                lhs,
                rhs,
                span: *span,
                nb: matches!(stmt, Stmt::NbAssign { .. }),
                conds: conds.clone(),
            });
        }
        Stmt::IntraAssign { lhs, rhs, span, .. } => {
            out.push(FlatAssign {
                lhs,
                rhs,
                span: *span,
                nb: false,
                conds: conds.clone(),
            });
        }
        // SysCall/SubCall/Force/Release/Disable/Trigger/Null/ProcContAssign:
        // v1 不產規則(Force 族=vl-sim 軌語義;ProcContAssign=程序連續賦值
        // 邊緣形,登記於實作記錄)。
        _ => {}
    }
}

/// 行程內全部 NbAssign 的 lhs 基底名集合(FSM 選擇子判準:同名寄存器
/// 在該行程被 NbAssign,§4.2)。
fn nb_lhs_names(stmt: &Stmt) -> HashSet<String> {
    let mut names = HashSet::new();
    let mut conds = Vec::new();
    let mut out = Vec::new();
    walk_stmt(stmt, &mut conds, &mut out);
    for a in &out {
        if a.nb {
            if let Some(b) = lvalue_base(a.lhs) {
                names.insert(b);
            }
        }
    }
    names
}

// ---- 規則草稿(共用欄位;finalize 統一 dedup/截斷/編 id) ----

pub(crate) struct Draft {
    pub category: Category,
    pub qualifier: Qualifier,
    pub statement: String,
    pub evidence: Vec<Evidence>,
    pub check: Check,
}

impl Draft {
    pub(crate) fn fingerprint(&self, source: &str, module: &str) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}",
            source,
            self.category.as_str(),
            module,
            self.check.subject(),
            self.check.when_text(),
            self.check.effects_text()
        )
    }
}

/// dedup(指紋,保首)→ 截斷(記 truncated)→ 編 id(`module.R001`)。
pub(crate) fn finalize_module(
    source: &str,
    module: &str,
    mut drafts: Vec<Draft>,
    max_rules: usize,
    truncated: &mut Vec<String>,
) -> ModuleRules {
    let mut seen = HashSet::new();
    let mut kept: Vec<(String, Draft)> = Vec::new();
    for d in drafts.drain(..) {
        let fp = d.fingerprint(source, module);
        if seen.insert(fp.clone()) {
            kept.push((fp, d));
        }
    }
    if kept.len() > max_rules {
        kept.truncate(max_rules);
        truncated.push(module.to_string());
    }
    let rules = kept
        .into_iter()
        .enumerate()
        .map(|(i, (fp, d))| Rule {
            id: format!("{}.R{:03}", module, i + 1),
            category: d.category,
            module: module.to_string(),
            statement: d.statement,
            qualifier: d.qualifier,
            evidence: d.evidence,
            check: d.check,
            fingerprint: fp,
        })
        .collect();
    ModuleRules {
        module: module.to_string(),
        rules,
    }
}

pub(crate) use gaps::scan_gaps_sv;

// ---- 主入口 ----

pub(crate) fn extract_rules_sv(input: &SvInput, max_rules: usize) -> Rulebook {
    if is_tb_file(&input.source) {
        return Rulebook {
            source: input.name.clone(),
            parse_errors: Vec::new(),
            skipped: vec![SkippedNote {
                file: input.name.clone(),
                reason: SkipReason::TbFile,
                detail: None,
            }],
            modules: Vec::new(),
            truncated: Vec::new(),
        };
    }
    let analysis = analyzer_core::analyze(&SourceInput::Sv(input.clone()));
    let parse_errors = extract_parse_errors(&analysis);
    let ctx = Ctx {
        file_name: input.name.clone(),
        lines: split_lines(&input.source),
        analysis: &analysis,
    };
    let mut truncated = Vec::new();
    let mut modules = Vec::new();
    if let Some(file) = &analysis.sv {
        for m in &file.modules {
            let drafts = extract_module(&ctx, m);
            modules.push(finalize_module(&input.name, &m.name, drafts, max_rules, &mut truncated));
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

fn extract_module(ctx: &Ctx, m: &ast::Module) -> Vec<Draft> {
    let mut drafts = Vec::new();
    walk_items(&m.items, false, &mut |item, in_generate| match item {
        ModuleItem::Always { ctrl, body, .. } => {
            extract_process(ctx, ctrl.as_ref(), body, &mut drafts);
        }
        ModuleItem::Initial { .. } => {
            // Initial=TB 面,不產規則(§4.2)。
        }
        ModuleItem::ContAssign(pairs) => {
            for p in pairs {
                if expr::has_binary(&p.rhs) {
                    if let Some(ev) = ctx.evidence(
                        p.span,
                        if in_generate {
                            EvidenceClass::Generate
                        } else {
                            EvidenceClass::Assign
                        },
                    ) {
                        let lhs = render(&p.lhs);
                        let rhs = render(&p.rhs);
                        // 連續賦值全域成立 → when=None(R3-②)。
                        drafts.push(Draft {
                            category: Category::Arithmetic,
                            qualifier: Qualifier::Derived,
                            statement: format!("{lhs} ≡ {rhs}(逐拍組合)"),
                            evidence: vec![ev],
                            check: Check {
                                form: CheckForm::Relation,
                                payload: CheckPayload::Relation {
                                    lhs,
                                    rhs,
                                    when: None,
                                    width_note: None,
                                },
                            },
                        });
                    }
                }
            }
        }
        _ => {}
    });
    drafts
}

fn extract_process(ctx: &Ctx, ctrl: Option<&EventCtrl>,
    body: &Stmt,
    drafts: &mut Vec<Draft>,
) {
    match classify_proc(ctrl) {
        ProcClass::Comb => {
            let mut conds = Vec::new();
            let mut out = Vec::new();
            walk_stmt(body, &mut conds, &mut out);
            for a in out {
                if expr::has_binary(a.rhs) {
                    if let Some(ev) = ctx.evidence(a.span, EvidenceClass::Process) {
                        let lhs = render(a.lhs);
                        let rhs = render(a.rhs);
                        // 適用域(R3-②):行程內賦值攜條件鏈入 when(分支域
                        // 轉寫不冒充全域恆等——subbytes R007「state==0 else
                        // 分支被全局誤讀預測狀態環」實錄);空鏈=全域。
                        let when = if a.conds.is_empty() {
                            None
                        } else {
                            Some(a.conds.join(" 且 "))
                        };
                        let statement = match &when {
                            Some(w) => format!("當 {w} 時,{lhs} ≡ {rhs}(逐拍組合)"),
                            None => format!("{lhs} ≡ {rhs}(逐拍組合)"),
                        };
                        drafts.push(Draft {
                            category: Category::Arithmetic,
                            qualifier: Qualifier::Derived,
                            statement,
                            evidence: vec![ev],
                            check: Check {
                                form: CheckForm::Relation,
                                payload: CheckPayload::Relation {
                                    lhs,
                                    rhs,
                                    when,
                                    width_note: None,
                                },
                            },
                        });
                    }
                }
            }
        }
        ProcClass::Edge => extract_edge_process(ctx, ctrl, body, drafts),
    }
}

/// 邊沿行程:復位臂 → FSM 臂 → timing/counter 分流(§4.2)。
fn extract_edge_process(ctx: &Ctx, ctrl: Option<&EventCtrl>,
    body: &Stmt,
    drafts: &mut Vec<Draft>,
) {
    let edges: Vec<&ast::EventEdge> = ctrl
        .map(|c| c.edges.iter().collect())
        .unwrap_or_default();
    let trigger = edges.iter().find_map(|e| match e.kind {
        EdgeKind::Posedge | EdgeKind::Negedge => Some(Trigger {
            edge: Some(match e.kind {
                EdgeKind::Posedge => "posedge".to_string(),
                _ => "negedge".to_string(),
            }),
            signal: Some(render(&e.expr)),
        }),
        _ => None,
    });
    let trigger_text = trigger
        .as_ref()
        .map(|t| {
            format!(
                "{} {}",
                t.signal.as_deref().unwrap_or("?"),
                match t.edge.as_deref() {
                    Some("posedge") => "上升沿",
                    Some("negedge") => "下降沿",
                    _ => "跳變沿",
                }
            )
        })
        .unwrap_or_else(|| "時鐘沿".to_string());

    // 頂層解 Block。
    let top: Vec<&Stmt> = match body {
        Stmt::Block { body, .. } => body.iter().collect(),
        s => vec![s],
    };

    // 復位臂:首層 If 且 cond=單訊號(±否定);復位訊號≠觸發訊號。
    let mut reset_when: Option<String> = None;
    let mut reset_arm: Option<&Stmt> = None;
    let mut continuation: Vec<&Stmt> = Vec::new();
    if let Some(Stmt::If { cond, then, els, .. }) = top.first() {
        if let Some(sig) = reset_cond_signal(cond) {
            let is_trigger = trigger
                .as_ref()
                .and_then(|t| t.signal.as_deref())
                .map(|s| s == sig)
                .unwrap_or(false);
            if !is_trigger {
                reset_when = Some(render(cond));
                reset_arm = Some(&**then);
                if let Some(e) = els {
                    continuation.push(&**e);
                }
                for s in top.iter().skip(1) {
                    continuation.push(s);
                }
            }
        }
    }
    if reset_arm.is_none() {
        continuation.extend(top.iter().copied());
    }

    // 1) reset 規則:then 臂每個賦值一條 guard(evidence=then 臂 span)。
    if let (Some(arm), Some(when)) = (reset_arm, reset_when.clone()) {
        let mut conds = Vec::new();
        let mut out = Vec::new();
        walk_stmt(arm, &mut conds, &mut out);
        let ev = ctx.evidence(stmt_span(arm), EvidenceClass::Process);
        for a in &out {
            let Some(ev) = ev.clone() else { continue };
            let lhs = render(a.lhs);
            let value = render(a.rhs);
            let stmt_text = match &trigger {
                Some(t) => format!(
                    "當 {when} 且 {} {} 時,{lhs} 裝載 {value}",
                    t.signal.as_deref().unwrap_or("?"),
                    match t.edge.as_deref() {
                        Some("posedge") => "上升沿",
                        Some("negedge") => "下降沿",
                        _ => "跳變沿",
                    }
                ),
                None => format!("當 {when} 時,{lhs} 裝載 {value}"),
            };
            drafts.push(Draft {
                category: Category::Reset,
                qualifier: Qualifier::Derived,
                statement: stmt_text,
                evidence: vec![ev],
                check: Check {
                    form: CheckForm::Guard,
                    payload: CheckPayload::Guard {
                        trigger: trigger.clone(),
                        when: Some(when.clone()),
                        effects: vec![Effect {
                            lhs: lhs.clone(),
                            value: value.clone(),
                        }],
                    },
                },
            });
        }
    }

    // 2) FSM:續體內 Case,選擇子=本行程 NbAssign 過的同名寄存器。
    let nb_names = {
        let mut names = HashSet::new();
        for s in &continuation {
            names.extend(nb_lhs_names(s));
        }
        names
    };
    let mut consumed: HashSet<u32> = HashSet::new(); // span.start.0
    let mut cases: Vec<CaseLike> = Vec::new();
    collect_cases(&continuation, &mut cases);
    for case in &cases {
        let sel = lvalue_base(case.selector);
        let Some(sel_name) = sel else { continue };
        if !nb_names.contains(&sel_name) {
            continue;
        }
        for arm in &case.arms {
            // 臂內狀態賦值 → transition 規則(arm=(labels, stmt) 元組)。
            let (labels, arm_stmt) = arm;
            let mut conds = Vec::new();
            let mut out = Vec::new();
            walk_stmt(arm_stmt, &mut conds, &mut out);
            let arm_ev = ctx.evidence(stmt_span(arm_stmt), EvidenceClass::Case);
            for a in &out {
                if !a.nb {
                    continue;
                }
                if lvalue_base(a.lhs).as_deref() != Some(sel_name.as_str()) {
                    continue;
                }
                let Some(ev) = arm_ev.clone() else { continue };
                consumed.insert(a.span.start.0);
                let from = labels
                    .first()
                    .map(render)
                    .unwrap_or_else(|| "?".to_string());
                let to = render(a.rhs);
                // when=該賦值的內層條件鏈(FlatAssign.conds;外層 conds 在
                // walk 後已彈空,不可用)。case 臂條件文本不在其內——from
                // 已承載選擇子語義。
                let when = if a.conds.is_empty() {
                    None
                } else {
                    Some(a.conds.join(" 且 "))
                };
                let stmt_text = match &when {
                    Some(w) => format!("{sel_name}={from} 且 {w} 時,下一拍 {sel_name}={to}"),
                    None => format!("{sel_name}={from} 時,下一拍 {sel_name}={to}"),
                };
                drafts.push(Draft {
                    category: Category::Fsm,
                    qualifier: Qualifier::Derived,
                    statement: stmt_text,
                    evidence: vec![ev],
                    check: Check {
                        form: CheckForm::Transition,
                        payload: CheckPayload::Transition {
                            state_reg: sel_name.clone(),
                            from,
                            to,
                            when,
                        },
                    },
                });
            }
            // default 臂狀態賦值:同樣從 timing 排除(見 CaseLike.default 注)。
            if let Some(d) = case.default {
                let mut dconds = Vec::new();
                let mut dout = Vec::new();
                walk_stmt(d, &mut dconds, &mut dout);
                for a in &dout {
                    if a.nb && lvalue_base(a.lhs).as_deref() == Some(sel_name.as_str()) {
                        consumed.insert(a.span.start.0);
                    }
                }
            }
        }
    }

    // 3) timing/counter:續體全部賦值,排除 FSM 已消費的狀態賦值。
    for s in &continuation {
        let mut conds = Vec::new();
        let mut out = Vec::new();
        walk_stmt(s, &mut conds, &mut out);
        for a in out {
            if consumed.contains(&a.span.start.0) {
                continue;
            }
            let Some(ev) = ctx.evidence(a.span, EvidenceClass::Process) else {
                continue;
            };
            let lhs = render(a.lhs);
            let rhs = render(a.rhs);
            let lhs_base = lvalue_base(a.lhs);
            let anchor = if a.conds.is_empty() {
                trigger_text.clone()
            } else {
                a.conds.join(" 且 ")
            };
            let is_counter = lhs_base
                .as_ref()
                .map(|b| expr_self_ref(a.rhs, b))
                .unwrap_or(false)
                && rhs_has_self_addsub(a.rhs, lhs_base.as_deref().unwrap_or(""));
            let (category, statement) = if is_counter {
                (
                    Category::Counter,
                    format!("{anchor} 成立後的下一拍,{lhs} 變為 {rhs}(計數/環繞)"),
                )
            } else {
                (
                    Category::Timing,
                    format!("{anchor} 成立後的下一拍,{lhs} 變為 {rhs}"),
                )
            };
            drafts.push(Draft {
                category,
                qualifier: Qualifier::Derived,
                statement,
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

/// 復位條件候選:cond 只含一個訊號與否定(§4.2)=Ident 或 `!Ident`。
pub(crate) fn reset_cond_signal(cond: &ast::Expr) -> Option<&str> {
    match cond {
        ast::Expr::Ident { name, .. } => Some(name),
        ast::Expr::Unary {
            op: ast::UnOp::Not,
            expr,
            ..
        } => match &**expr {
            ast::Expr::Ident { name, .. } => Some(name),
            _ => None,
        },
        _ => None,
    }
}

fn expr_self_ref(e: &ast::Expr, name: &str) -> bool {
    walk_expr(e, &mut |x| match x {
        ast::Expr::Ident { name: n, .. } => n == name,
        _ => false,
    })
}

/// rhs 是否含「自身 ± 」形(counter 判據;三元環繞形內含 Add/Sub 同樣命中)。
fn rhs_has_self_addsub(e: &ast::Expr, name: &str) -> bool {
    walk_expr(e, &mut |x| match x {
        ast::Expr::Binary {
            op: ast::BinOp::Add | ast::BinOp::Sub,
            lhs,
            rhs,
            ..
        } => {
            let l = match &**lhs {
                ast::Expr::Ident { name: n, .. } => n == name,
                _ => false,
            };
            let r = match &**rhs {
                ast::Expr::Ident { name: n, .. } => n == name,
                _ => false,
            };
            l || r
        }
        _ => false,
    })
}

// ---- Case 結構中立收集(fsm 提取;§4.2) ----

pub(crate) struct CaseLike<'a> {
    pub selector: &'a ast::Expr,
    /// (labels, 臂語句);規則只對顯式臂。
    pub arms: Vec<(&'a Vec<ast::Expr>, &'a Stmt)>,
    /// default 臂(不產規則;其狀態賦值仍從 timing 排除——state 更新
    /// 語義屬 FSM 面,落入 timing 即「state 無條件下一拍變為…」的假規則)。
    pub default: Option<&'a Stmt>,
}

fn collect_cases<'a>(stmts: &[&'a Stmt], out: &mut Vec<CaseLike<'a>>) {
    for s in stmts {
        collect_cases_stmt(s, out);
    }
}

fn collect_cases_stmt<'a>(s: &'a Stmt, out: &mut Vec<CaseLike<'a>>) {
    match s {
        Stmt::Block { body, .. } => collect_cases(&body.iter().collect::<Vec<_>>(), out),
        Stmt::If { then, els, .. } => {
            collect_cases_stmt(then, out);
            if let Some(e) = els {
                collect_cases_stmt(e, out);
            }
        }
        Stmt::Case { expr, items, .. } => {
            let arms = items
                .iter()
                .filter_map(|it| match it {
                    ast::CaseItem::Items(labels, stmt) => Some((labels, stmt)),
                    ast::CaseItem::Default(_) => None,
                })
                .collect();
            let default = items.iter().find_map(|it| match it {
                ast::CaseItem::Default(stmt) => Some(stmt),
                _ => None,
            });
            out.push(CaseLike {
                selector: expr,
                arms,
                default,
            });
            // 嵌套 case(臂內)遞進。
            for it in items {
                match it {
                    ast::CaseItem::Items(_, stmt) => collect_cases_stmt(stmt, out),
                    ast::CaseItem::Default(stmt) => collect_cases_stmt(stmt, out),
                }
            }
        }
        Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::Repeat { body, .. }
        | Stmt::Forever { body, .. } => collect_cases_stmt(body, out),
        Stmt::Fork { branches, .. } => {
            for b in branches {
                collect_cases_stmt(b, out);
            }
        }
        _ => {}
    }
}
