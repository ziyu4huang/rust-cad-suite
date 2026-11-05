//! SV 側破口掃描器(§3.1 G1..G7;DriverMap=驅動/讀取圖基礎)。
//!
//! 判準與置信度對齊正典:G1/G4/G5/G6/G7=結構可證(certain);G2 值域覆蓋
//! 與 G3 驅動圖在 include/宏場景受限=heuristic。每類破口=一個行為未定/
//! 未覆蓋點,suggested_test 機械模板即測試缺口清單(§3.2)。
//!
//! v1 保守化決策(防誤報,R6;記入實作記錄):
//! - 實例連接埠不進驅動圖(本輸入內無跨模組方向證據);實例連接過的
//!   名字豁免 G3(可能由子模組驅動)。
//! - G6 按**驅動源**(連續賦值/各行程)計數:同行程內的位選/字寫合併到
//!   基底後仍屬同一源,不誤報正常記憶體多字寫。
//! - G1 收平輪(R3-①):行程**頂層無條件賦值**(塊首默認賦值形)對沖
//!   分支不對稱——subbytes.v 61/62 行 `next_state/next_data_reg` 默認
//!   賦值+54-57 行 `data_reg_var[0..3]` 無條件賦值實證 3/4 誤報;有頂層
//!   默認的訊號各路徑均有值不鎖存,不報。剩餘報告=無默認+分支不對稱,
//!   Verilog 語義下結構可證,certain 維持(置信度鏈復核,R3-①)。

use std::collections::{HashMap, HashSet, VecDeque};

use analyzer_core::SvInput;
use rcs_diag::Span;
use vl_syntax::ast::{self, Direction, Expr, Module, Stmt};

use crate::expr::{lvalue_base, render, walk_expr};
use crate::schema::{
    Confidence, Evidence, EvidenceClass, GapClass, GapItem, GapReport, SkippedNote, SkipReason,
};
use crate::sv::{classify_proc, extract_parse_errors, is_tb_file, reset_cond_signal, split_lines,
                walk_items, walk_stmt, Ctx, FlatAssign, ProcClass};

enum DriverSource {
    ContAssign,
    Proc(usize),
}

struct DriverSite {
    span: Span,
    source: DriverSource,
}

struct ProcRec<'a> {
    id: usize,
    class: ProcClass,
    span: Span,
    body: &'a Stmt,
    /// reset 臂拆分(與規則提取同一首層 If 判準);None=無復位臂。
    reset: Option<(&'a Stmt, Vec<&'a Stmt>)>,
}

struct AssignRec<'a> {
    proc: Option<usize>, // None=ContAssign
    lhs: &'a Expr,
    rhs: &'a Expr,
    span: Span,
    nb: bool,
}

struct ModuleScan<'a> {
    name: String,
    input_ports: HashSet<String>,
    non_drivable: HashSet<String>,
    /// 已聲明名全集(埠+體內聲明;G3 只報已聲明名,層級/全局引用除外)。
    declared: HashSet<String>,
    decl_width: HashMap<String, u32>,
    drivers: HashMap<String, Vec<DriverSite>>,
    reads: HashMap<String, Vec<Span>>,
    instance_conn_names: HashSet<String>,
    procs: Vec<ProcRec<'a>>,
    assigns: Vec<AssignRec<'a>>,
}

impl ModuleScan<'_> {
    fn proc_class(&self, pid: usize) -> Option<ProcClass> {
        self.procs.iter().find(|p| p.id == pid).map(|p| p.class)
    }
}

pub(crate) fn scan_gaps_sv(input: &SvInput) -> GapReport {
    if is_tb_file(&input.source) {
        return GapReport {
            source: input.name.clone(),
            parse_errors: Vec::new(),
            skipped: vec![SkippedNote {
                file: input.name.clone(),
                reason: SkipReason::TbFile,
                detail: None,
            }],
            items: Vec::new(),
        };
    }
    let analysis = analyzer_core::analyze(&analyzer_core::SourceInput::Sv(input.clone()));
    let parse_errors = extract_parse_errors(&analysis);
    let ctx = Ctx {
        file_name: input.name.clone(),
        lines: split_lines(&input.source),
        analysis: &analysis,
    };
    let mut items = Vec::new();
    let mut skipped: Vec<SkippedNote> = Vec::new();
    if let Some(file) = &analysis.sv {
        for m in &file.modules {
            let sc = build_scan(m);
            let mut counters: HashMap<&'static str, usize> = HashMap::new();
            scan_latches(&ctx, &sc, &mut counters, &mut items);
            scan_incomplete_cases(&ctx, &sc, &mut counters, &mut items);
            scan_undriven(&ctx, &sc, &mut counters, &mut items);
            scan_missing_reset(&ctx, &sc, &mut counters, &mut items);
            scan_comb_loops(&ctx, &sc, &mut counters, &mut items, &mut skipped);
            scan_multi_drivers(&ctx, &sc, &mut counters, &mut items);
            scan_blocking_in_seq(&ctx, &sc, &mut counters, &mut items);
            scan_dead_branches(&ctx, &sc, &mut counters, &mut items);
        }
    }
    GapReport {
        source: input.name.clone(),
        parse_errors,
        skipped,
        items,
    }
}

fn build_scan<'a>(m: &'a Module) -> ModuleScan<'a> {
    let mut sc = ModuleScan {
        name: m.name.clone(),
        input_ports: m
            .ports
            .iter()
            .filter(|p| p.dir == Some(Direction::Input))
            .map(|p| p.name.clone())
            .collect(),
        non_drivable: HashSet::new(),
        declared: m.ports.iter().map(|p| p.name.clone()).collect(),
        decl_width: HashMap::new(),
        drivers: HashMap::new(),
        reads: HashMap::new(),
        instance_conn_names: HashSet::new(),
        procs: Vec::new(),
        assigns: Vec::new(),
    };
    // 第一遍:聲明面(input 埠補全/寬度/不可驅動名;ANSI 埠寬度;聲明
    // 初始化=驅動源——`wire IN1 = a[i]` 是合法驅動,G3 不報)。
    for p in &m.ports {
        if let Some(r) = &p.range {
            if let Some(w) = literal_width(r) {
                sc.decl_width.insert(p.name.clone(), w);
            }
        }
    }
    walk_items(&m.items, false, &mut |item, _gen| {
        if let ast::ModuleItem::Decl(d) = item {
            match d.kind {
                ast::DeclKind::Parameter
                | ast::DeclKind::Localparam
                | ast::DeclKind::Genvar
                | ast::DeclKind::Event => {
                    for e in &d.entries {
                        sc.non_drivable.insert(e.name.clone());
                    }
                }
                _ => {
                    let w = d.range.as_ref().and_then(literal_width);
                    for e in &d.entries {
                        sc.declared.insert(e.name.clone());
                        if let Some(w) = w {
                            sc.decl_width.insert(e.name.clone(), w);
                        }
                        if d.dir == Some(Direction::Input) {
                            sc.input_ports.insert(e.name.clone());
                        }
                        if let Some(init) = &e.init {
                            sc.drivers.entry(e.name.clone()).or_default().push(DriverSite {
                                span: e.span,
                                source: DriverSource::ContAssign,
                            });
                            collect_reads(init, &mut sc.reads);
                        }
                    }
                }
            }
        }
    });
    // 第三遍:行程/連續賦值/實例連接。
    let mut proc_id = 0usize;
    walk_items(&m.items, false, &mut |item, _gen| match item {
        ast::ModuleItem::Always { span, ctrl, body } => {
            let class = classify_proc(ctrl.as_ref());
            let top: Vec<&Stmt> = match body {
                Stmt::Block { body, .. } => body.iter().collect(),
                s => vec![s],
            };
            let mut reset: Option<(&Stmt, Vec<&Stmt>)> = None;
            if let Some(Stmt::If { cond, then, els, .. }) = top.first() {
                if let Some(sig) = reset_cond_signal(cond) {
                    // 觸發訊號=首個 pos/neg 邊(時鐘);異步復位邊不在其列
                    // (與規則提取面同一判準)。
                    let trig_is_sig = ctrl
                        .as_ref()
                        .and_then(|c| {
                            c.edges
                                .iter()
                                .find(|e| {
                                    matches!(
                                        e.kind,
                                        ast::EdgeKind::Posedge | ast::EdgeKind::Negedge
                                    )
                                })
                                .map(|e| lvalue_base(&e.expr).as_deref() == Some(sig))
                        })
                        .unwrap_or(false);
                    if !trig_is_sig {
                        let mut cont = Vec::new();
                        if let Some(e) = els {
                            cont.push(&**e);
                        }
                        cont.extend(top.iter().skip(1).copied());
                        reset = Some((&**then, cont));
                    }
                }
            }
            sc.procs.push(ProcRec {
                id: proc_id,
                class,
                span: *span,
                body,
                reset,
            });
            proc_id += 1;
        }
        ast::ModuleItem::ContAssign(pairs) => {
            for p in pairs {
                let mut bases = Vec::new();
                lvalue_bases(&p.lhs, &mut bases);
                for base in bases {
                    sc.drivers.entry(base).or_default().push(DriverSite {
                        span: p.span,
                        source: DriverSource::ContAssign,
                    });
                }
                collect_reads(&p.lhs, &mut sc.reads);
                collect_reads(&p.rhs, &mut sc.reads);
                sc.assigns.push(AssignRec {
                    proc: None,
                    lhs: &p.lhs,
                    rhs: &p.rhs,
                    span: p.span,
                    nb: false,
                });
            }
        }
        ast::ModuleItem::Instance(inst) => {
            for u in &inst.insts {
                for c in &u.conns {
                    if let Some(e) = &c.expr {
                        collect_reads(e, &mut sc.reads);
                        if let Some(base) = lvalue_base(e) {
                            sc.instance_conn_names.insert(base);
                        }
                    }
                }
            }
        }
        _ => {}
    });
    // 行程賦值面(flat 走查,借自 AST)。
    let flat_per_proc: Vec<(usize, Vec<FlatAssign<'a>>)> = sc
        .procs
        .iter()
        .map(|p| {
            let mut conds = Vec::new();
            let mut out = Vec::new();
            walk_stmt(p.body, &mut conds, &mut out);
            (p.id, out)
        })
        .collect();
    for (id, flat) in &flat_per_proc {
        for a in flat {
            let mut bases = Vec::new();
            lvalue_bases(a.lhs, &mut bases);
            for base in bases {
                sc.drivers.entry(base).or_default().push(DriverSite {
                    span: a.span,
                    source: DriverSource::Proc(*id),
                });
            }
            collect_reads(a.lhs, &mut sc.reads);
            collect_reads(a.rhs, &mut sc.reads);
            sc.assigns.push(AssignRec {
                proc: Some(*id),
                lhs: a.lhs,
                rhs: a.rhs,
                span: a.span,
                nb: a.nb,
            });
        }
    }
    sc
}

/// lvalue 的基底標識符集(Concat LHS 展開各部件——`assign {Cout,S} = …`
/// 的 Cout/S 都是被驅動的;G3 首跑誤報實錄)。
fn lvalue_bases(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Concat { parts, .. } => {
            for p in parts {
                lvalue_bases(p, out);
            }
        }
        _ => {
            if let Some(b) = lvalue_base(e) {
                out.push(b);
            }
        }
    }
}

fn collect_reads(e: &Expr, out: &mut HashMap<String, Vec<Span>>) {
    walk_expr(e, &mut |x| {
        if let Expr::Ident { span, name } = x {
            out.entry(name.clone()).or_default().push(*span);
        }
        false
    });
}

fn ev(ctx: &Ctx, span: Span) -> Option<Evidence> {
    ctx.evidence(span, EvidenceClass::Process)
}

#[allow(clippy::too_many_arguments)]
fn push_gap(
    sc: &ModuleScan,
    counters: &mut HashMap<&'static str, usize>,
    items: &mut Vec<GapItem>,
    class: GapClass,
    confidence: Confidence,
    message: String,
    evidence: Vec<Evidence>,
    suggested_test: String,
) {
    let n = counters.entry(class.as_str()).or_insert(0);
    *n += 1;
    items.push(GapItem {
        id: format!("{}-{}-{:03}", class.prefix(), sc.name, n),
        class,
        module: sc.name.clone(),
        confidence,
        message,
        evidence,
        suggested_test,
    });
}

// ---- G1 latch 推斷:組合行程路徑敏感 LHS 集合差(§3.1) ----

/// 行程**頂層**(首層 Block 直屬語句,不下探 If/Case/循環/Fork)無條件
/// 賦值的 LHS 基底集=默認賦值對沖集(R3-①):組合行程每條路徑都執行
/// 頂層語句,集內訊號各路徑均有值,不鎖存。
fn top_level_defaults(body: &Stmt) -> HashSet<String> {
    let mut out = HashSet::new();
    let tops: Vec<&Stmt> = match body {
        Stmt::Block { body, .. } => body.iter().collect(),
        s => vec![s],
    };
    for s in tops {
        let mut conds = Vec::new();
        let mut flat = Vec::new();
        walk_stmt(s, &mut conds, &mut flat);
        // 直屬賦值(conds 空=不在條件/循環體內)才計入;walk 會下探嵌套
        // 構造,用 conds 空過濾回頂層語義。
        for a in flat {
            if a.conds.is_empty() {
                let mut bases = Vec::new();
                lvalue_bases(a.lhs, &mut bases);
                out.extend(bases);
            }
        }
    }
    out
}

fn scan_latches(ctx: &Ctx, sc: &ModuleScan, counters: &mut HashMap<&'static str, usize>, items: &mut Vec<GapItem>) {
    for p in &sc.procs {
        if p.class != ProcClass::Comb {
            continue;
        }
        let defaults = top_level_defaults(p.body);
        let mut reported: HashSet<String> = HashSet::new();
        check_paths(ctx, sc, counters, items, p.body, &defaults, &mut reported);
    }
}

fn check_paths(
    ctx: &Ctx,
    sc: &ModuleScan,
    counters: &mut HashMap<&'static str, usize>,
    items: &mut Vec<GapItem>,
    s: &Stmt,
    defaults: &HashSet<String>,
    reported: &mut HashSet<String>,
) {
    match s {
        Stmt::If { span, cond, then, els } => {
            let l = lhs_set(then);
            let r = els.as_ref().map(|e| lhs_set(e)).unwrap_or_default();
            // F1b(16.0):HashSet 迭代序禁入報告序與 id↔內容綁定——
            // 收集排序去重後遍歷(集合邏輯不變,報告序確定)。
            let mut sigs: Vec<&String> = l.iter().chain(r.iter()).collect();
            sigs.sort();
            sigs.dedup();
            for sig in sigs {
                let in_then = l.contains(sig);
                let in_else = r.contains(sig);
                if in_then == in_else || defaults.contains(sig) {
                    continue;
                }
                if reported.insert(sig.clone()) {
                    let mut evs = Vec::new();
                    if let Some(e) = ev(ctx, *span) {
                        evs.push(e);
                    }
                    let cond_text = render(cond);
                    push_gap(
                         sc,
                        counters,
                        items,
                        GapClass::Latch,
                        Confidence::Certain,
                        format!("組合行程內 {sig} 在條件「{cond_text}」的一側有賦值但對立路徑無賦值(可能鎖存)"),
                        evs,
                        format!("枚舉「{cond_text}」反相輸入,斷言 {sig} 保持值(鎖存)或改為組合(設計缺陷)"),
                    );
                }
            }
            check_paths(ctx, sc, counters, items, then, defaults, reported);
            if let Some(e) = els {
                check_paths(ctx, sc, counters, items, e, defaults, reported);
            }
        }
        Stmt::Case { span, expr, items: arms, .. } => {
            let default_set = arms.iter().find_map(|it| match it {
                ast::CaseItem::Default(s) => Some(lhs_set(s)),
                _ => None,
            });
            if let Some(dset) = default_set {
                for it in arms {
                    if let ast::CaseItem::Items(_, s) = it {
                        // F1b(16.0):同 If 臂——臂內 sig 集排序後遍歷。
                        let mut sigs: Vec<String> =
                            lhs_set(s).into_iter().collect();
                        sigs.sort();
                        for sig in sigs {
                            if !dset.contains(&sig)
                                && !defaults.contains(&sig)
                                && reported.insert(sig.clone())
                            {
                                let mut evs = Vec::new();
                                if let Some(e) = ev(ctx, *span) {
                                    evs.push(e);
                                }
                                push_gap(
                                     sc,
                                    counters,
                                    items,
                                    GapClass::Latch,
                                    Confidence::Certain,
                                    format!("組合行程內 {sig} 在 case 臂有賦值但 default 臂無賦值(可能鎖存;選擇子 {})", render(expr)),
                                    evs,
                                    format!("選擇 {sig} 存活於 default 編碼的輸入,斷言其保持值(鎖存)或補全 case"),
                                );
                            }
                        }
                    }
                }
            }
            for it in arms {
                match it {
                    ast::CaseItem::Items(_, s) | ast::CaseItem::Default(s) => {
                        check_paths(ctx, sc, counters, items, s, defaults, reported)
                    }
                }
            }
        }
        Stmt::Block { body, .. } => {
            for b in body {
                check_paths(ctx, sc, counters, items, b, defaults, reported);
            }
        }
        Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::Repeat { body, .. }
        | Stmt::Forever { body, .. } => {
            check_paths(ctx, sc, counters, items, body, defaults, reported)
        }
        Stmt::Fork { branches, .. } => {
            for b in branches {
                check_paths(ctx, sc, counters, items, b, defaults, reported);
            }
        }
        _ => {}
    }
}

fn lhs_set(s: &Stmt) -> HashSet<String> {
    let mut conds = Vec::new();
    let mut out = Vec::new();
    walk_stmt(s, &mut conds, &mut out);
    out.iter()
        .filter_map(|a| lvalue_base(a.lhs))
        .collect()
}

// ---- G2 不完整 case:無 default(§3.1;heuristic) ----

fn scan_incomplete_cases(ctx: &Ctx, sc: &ModuleScan, counters: &mut HashMap<&'static str, usize>, items: &mut Vec<GapItem>) {
    for p in &sc.procs {
        let mut found = Vec::new();
        collect_cases_for_g2(p.body, &mut found);
        for (span, sel, n_labels, has_default) in found {
            if has_default {
                continue;
            }
            let width = lvalue_base(sel).and_then(|n| sc.decl_width.get(&n).copied());
            let mut evs = Vec::new();
            if let Some(e) = ev(ctx, span) {
                evs.push(e);
            }
            let (msg, test) = match width {
                Some(w) => {
                    let total = if w >= 63 { u64::MAX } else { 1u64 << w };
                    (
                        format!(
                            "case 無 default:選擇子 {} 寬 {w},臂覆蓋 {n_labels}/{total} 個編碼,未覆蓋編碼行為未定義",
                            render(sel)
                        ),
                        format!("施加未覆蓋編碼(共 {total} 個,已覆蓋 {n_labels} 個),釘住輸出行為"),
                    )
                }
                None => (
                    format!("case 無 default(選擇子 {}),未覆蓋編碼行為未定義", render(sel)),
                    "施加未覆蓋編碼,釘住輸出行為".to_string(),
                ),
            };
            push_gap(
                 sc,
                counters,
                items,
                GapClass::IncompleteCase,
                Confidence::Heuristic,
                msg,
                evs,
                test,
            );
        }
    }
}

type G2Rec<'a> = (Span, &'a Expr, usize, bool);

fn collect_cases_for_g2<'a>(s: &'a Stmt, out: &mut Vec<G2Rec<'a>>) {
    match s {
        Stmt::Case { span, expr, items, .. } => {
            let n_labels = items
                .iter()
                .filter_map(|it| match it {
                    ast::CaseItem::Items(labels, _) => Some(labels.len()),
                    _ => None,
                })
                .sum();
            let has_default = items
                .iter()
                .any(|it| matches!(it, ast::CaseItem::Default(_)));
            out.push((*span, expr, n_labels, has_default));
            for it in items {
                match it {
                    ast::CaseItem::Items(_, st) | ast::CaseItem::Default(st) => {
                        collect_cases_for_g2(st, out)
                    }
                }
            }
        }
        Stmt::Block { body, .. } => {
            for b in body {
                collect_cases_for_g2(b, out);
            }
        }
        Stmt::If { then, els, .. } => {
            collect_cases_for_g2(then, out);
            if let Some(e) = els {
                collect_cases_for_g2(e, out);
            }
        }
        Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::Repeat { body, .. }
        | Stmt::Forever { body, .. } => collect_cases_for_g2(body, out),
        Stmt::Fork { branches, .. } => {
            for b in branches {
                collect_cases_for_g2(b, out);
            }
        }
        _ => {}
    }
}

// ---- G3 無驅動讀取(§3.1;heuristic) ----

fn scan_undriven(ctx: &Ctx, sc: &ModuleScan, counters: &mut HashMap<&'static str, usize>, items: &mut Vec<GapItem>) {
    let mut names: Vec<&String> = sc.reads.keys().collect();
    names.sort();
    for name in names {
        if sc.drivers.contains_key(name)
            || sc.input_ports.contains(name)
            || sc.non_drivable.contains(name)
            || sc.instance_conn_names.contains(name)
            || !sc.declared.contains(name)
        {
            continue;
        }
        let mut evs = Vec::new();
        if let Some(&sp) = sc.reads.get(name).and_then(|v| v.first()) {
            if let Some(e) = ev(ctx, sp) {
                evs.push(e);
            }
        }
        push_gap(
             sc,
            counters,
            items,
            GapClass::UndrivenRead,
            Confidence::Heuristic,
            format!("{name} 被讀取但無任何驅動源且非輸入埠(恆為未定義)"),
            evs,
            format!("{name} 恆為未定義——補驅動或從測試中剔除"),
        );
    }
}

// ---- G4 缺復位覆蓋(§3.1;certain) ----

fn scan_missing_reset(ctx: &Ctx, sc: &ModuleScan, counters: &mut HashMap<&'static str, usize>, items: &mut Vec<GapItem>) {
    for p in &sc.procs {
        if p.class != ProcClass::Edge {
            continue;
        }
        match &p.reset {
            Some((arm, cont)) => {
                let reset_set = lhs_set(arm);
                let mut cont_assigns: Vec<(String, Span)> = Vec::new();
                for s in cont {
                    let mut conds = Vec::new();
                    let mut out = Vec::new();
                    walk_stmt(s, &mut conds, &mut out);
                    for a in &out {
                        if let Some(base) = lvalue_base(a.lhs) {
                            cont_assigns.push((base, a.span));
                        }
                    }
                }
                let mut reported: HashSet<&str> = HashSet::new();
                for (reg, span) in &cont_assigns {
                    if reset_set.contains(reg) || !reported.insert(reg.as_str()) {
                        continue;
                    }
                    let mut evs = Vec::new();
                    if let Some(e) = ev(ctx, *span) {
                        evs.push(e);
                    }
                    push_gap(
                         sc,
                        counters,
                        items,
                        GapClass::MissingReset,
                        Confidence::Certain,
                        format!("寄存器 {reg} 被時序賦值但不在復位臂 effect 集(缺復位覆蓋)"),
                        evs,
                        format!("復位後直接讀 {reg},釘住其上電值"),
                    );
                }
            }
            None => {
                // F1a(16.0):無復位臂寄存器集的取樣必須有序——HashSet
                // 迭代序隨機抖 suggested_test;收集排序後取字典序首名。
                let mut regs: Vec<String> = lhs_set(p.body).into_iter().collect();
                regs.sort();
                let mut evs = Vec::new();
                if let Some(e) = ev(ctx, p.span) {
                    evs.push(e);
                }
                let first = regs.first().cloned().unwrap_or_default();
                push_gap(
                     sc,
                    counters,
                    items,
                    GapClass::MissingReset,
                    Confidence::Certain,
                    format!("邊沿行程完全無復位臂,{} 個寄存器上電值未定義", regs.len()),
                    evs,
                    if first.is_empty() {
                        "復位後釘住行程全部寄存器的上電值".to_string()
                    } else {
                        format!("復位後直接讀 {first},釘住其上電值")
                    },
                );
            }
        }
    }
}

// ---- G5 組合環(§3.1;certain) ----

/// F2 圖規模守衛閾值(§16.0;R11「注記附實數」)。釘選集 37 檔實測
/// 最大 nodes=36/edges=24(macros.v test_ifdef,S8 探針普查);閾值取
/// 100×+ 餘量=釘選集零觸發,同時把超限圖的 O(E·(V+E)) 環檢測壓在
/// 秒級。scopes.v 復驗後真根因=bfs 死循環(非規模),守衛保留以防
/// 未來真大圖面積。超限=「守衛截斷」注記(skipped/comb_loop_guard)
/// 後終止環檢測,非靜默截斷。
const COMB_LOOP_NODE_LIMIT: usize = 4096;
const COMB_LOOP_EDGE_LIMIT: usize = 16384;

/// F2 守衛判定+注記構造(抽函數供 src 單測;超限=Some(注記))。
/// 「超限」語義=嚴格大於(恰在閾值=不觸發,守衛只擋面積失控圖)。
fn comb_loop_guard_note(
    file: &str,
    nodes: usize,
    edges: usize,
) -> Option<SkippedNote> {
    (nodes > COMB_LOOP_NODE_LIMIT || edges > COMB_LOOP_EDGE_LIMIT).then(|| SkippedNote {
        file: file.to_string(),
        reason: SkipReason::CombLoopGuard,
        detail: Some(format!(
            "nodes={nodes} edges={edges}(守衛截斷,環檢測未執行)"
        )),
    })
}

#[cfg(test)]
mod guard_tests {
    use super::*;

    #[test]
    fn guard_note_pins_selected_set_headroom() {
        // R11 數字錨:釘選集 37 檔實測最大 nodes=36/edges=24(macros.v
        // test_ifdef,S8 探針普查)——閾值必須留 100×+ 餘量,釘選集零觸發。
        assert!(COMB_LOOP_NODE_LIMIT > 36 * 100);
        assert!(COMB_LOOP_EDGE_LIMIT > 24 * 100);
    }

    #[test]
    fn guard_note_boundary_strict_gt() {
        assert!(comb_loop_guard_note("f.v", COMB_LOOP_NODE_LIMIT, 0).is_none());
        assert!(comb_loop_guard_note("f.v", 0, COMB_LOOP_EDGE_LIMIT).is_none());
        let hit = comb_loop_guard_note("f.v", COMB_LOOP_NODE_LIMIT + 1, 0).unwrap();
        assert_eq!(hit.reason.as_str(), "comb_loop_guard");
        assert_eq!(hit.file, "f.v");
        assert!(hit
            .detail
            .as_deref()
            .unwrap()
            .contains(&format!("nodes={}", COMB_LOOP_NODE_LIMIT + 1)));
        let hit2 = comb_loop_guard_note("f.v", 0, COMB_LOOP_EDGE_LIMIT + 1).unwrap();
        assert!(hit2
            .detail
            .as_deref()
            .unwrap()
            .contains(&format!("edges={}", COMB_LOOP_EDGE_LIMIT + 1)));
    }
}

fn scan_comb_loops(ctx: &Ctx, sc: &ModuleScan, counters: &mut HashMap<&'static str, usize>, items: &mut Vec<GapItem>, skipped: &mut Vec<SkippedNote>) {
    // 節點=組合 LHS 基底名;邊=rhs Ident → lhs(組合傳播方向)。
    let mut comb_lhs: HashSet<String> = HashSet::new();
    let mut edges: Vec<(String, String, Span)> = Vec::new();
    for a in &sc.assigns {
        if a.nb {
            continue; // 組合環圖只取阻塞形(ContAssign 與組合行程 BAssign)
        }
        let Some(base) = lvalue_base(a.lhs) else {
            continue;
        };
        let in_comb = a.proc.is_none()
            || sc.proc_class(a.proc.unwrap()) == Some(ProcClass::Comb);
        if !in_comb {
            continue;
        }
        comb_lhs.insert(base.clone());
        for (rd, _) in reads_of(a.rhs) {
            edges.push((rd, base.clone(), a.span));
        }
    }
    let edges: Vec<_> = edges
        .into_iter()
        .filter(|(f, t, _)| comb_lhs.contains(f) && comb_lhs.contains(t))
        .collect();
    // F2 規模守衛:超限=顯式「守衛截斷」注記後終止環檢測(禁靜默截斷、
    // 禁無限爬;詳數入 skipped detail,R11)。
    if let Some(note) =
        comb_loop_guard_note(&ctx.file_name, comb_lhs.len(), edges.len())
    {
        skipped.push(note);
        return;
    }
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for (f, t, _) in &edges {
        adj.entry(f.as_str()).or_default().push(t.as_str());
    }
    let mut reported_cycles: HashSet<Vec<String>> = HashSet::new();
    for (f, t, span) in &edges {
        if reported_cycles.len() >= 8 {
            break;
        }
        if f == t {
            let cyc = vec![f.clone()];
            if reported_cycles.insert(cyc.clone()) {
                let mut evs = Vec::new();
                if let Some(e) = ev(ctx, *span) {
                    evs.push(e);
                }
                push_gap(
                     sc,
                    counters,
                    items,
                    GapClass::CombLoop,
                    Confidence::Certain,
                    format!("組合自環:{f} 的賦值讀取自身(振盪/未定義)"),
                    evs,
                    format!("驅動 {f} 後立即觀測,斷言振盪/鎖死可觀測"),
                );
            }
            continue;
        }
        if let Some(path) = bfs_path(&adj, t, f) {
            // path = t..f;環=f → t → .. → f。
            let mut cyc = vec![f.clone()];
            cyc.extend(path.clone());
            let mut canon = cyc.clone();
            canon.sort();
            if reported_cycles.insert(canon) {
                let mut evs = Vec::new();
                if let Some(e) = ev(ctx, *span) {
                    evs.push(e);
                }
                push_gap(
                     sc,
                    counters,
                    items,
                    GapClass::CombLoop,
                    Confidence::Certain,
                    format!("組合環:{}", cyc.join(" → ")),
                    evs,
                    format!(
                        "構造使環上訊號({})同拍相互依賴的輸入,斷言振盪/鎖死可觀測",
                        cyc.join("/")
                    ),
                );
            }
        }
    }
}

fn bfs_path<'a>(
    adj: &HashMap<&'a str, Vec<&'a str>>,
    start: &'a str,
    goal: &'a str,
) -> Option<Vec<String>> {
    // F2(scopes.v 實錄,2026-11-03):起點必須入 visited。原實現用
    // prev.contains_key 防重——start 不在 prev,自環邊(adj[start] 含
    // start)會把 prev[start] 寫成 start(自映射),路徑重建 while 沿
    // 自映射永不停(native 4s+ 掛死樣本棧=bfs_path→HashMap::get;
    // wasm 腿表現為秒級 trap。scopes.v 實測圖僅 nodes=2/edges=7,
    // 真根因=死循環非規模)。
    let mut visited: HashSet<&str> = HashSet::new();
    let mut prev: HashMap<&str, &str> = HashMap::new();
    let mut queue = VecDeque::new();
    visited.insert(start);
    queue.push_back(start);
    while let Some(cur) = queue.pop_front() {
        if cur == goal {
            let mut path = vec![cur];
            let mut cur = cur;
            // 重建沿 prev 走到 start 為止:start 永無 prev 映射
            // (visited 擋回),鏈必終止;`?`=defensive(不可達前驅
            // 視為無路徑,不 panic)。
            while cur != start {
                let &p = prev.get(cur)?;
                path.push(p);
                cur = p;
            }
            path.reverse();
            return Some(path.iter().map(|s| s.to_string()).collect());
        }
        if let Some(nexts) = adj.get(cur) {
            for &n in nexts {
                if !visited.contains(n) {
                    visited.insert(n);
                    prev.insert(n, cur);
                    queue.push_back(n);
                }
            }
        }
    }
    None
}

fn reads_of(e: &Expr) -> Vec<(String, Span)> {
    let mut out = Vec::new();
    walk_expr(e, &mut |x| {
        if let Expr::Ident { span, name } = x {
            out.push((name.clone(), *span));
        }
        false
    });
    out
}

// ---- G6 多驅動(§3.1;certain;按源計數) ----

fn scan_multi_drivers(ctx: &Ctx, sc: &ModuleScan, counters: &mut HashMap<&'static str, usize>, items: &mut Vec<GapItem>) {
    let mut names: Vec<&String> = sc.drivers.keys().collect();
    names.sort();
    for name in names {
        let sites = &sc.drivers[name];
        let mut sources: HashSet<usize> = HashSet::new();
        let mut has_cont = false;
        for s in sites {
            match &s.source {
                DriverSource::ContAssign => has_cont = true,
                DriverSource::Proc(id) => {
                    sources.insert(*id);
                }
            }
        }
        let distinct = sources.len() + usize::from(has_cont);
        if distinct < 2 {
            continue;
        }
        let mut evs = Vec::new();
        for s in sites.iter().take(3) {
            if let Some(e) = ev(ctx, s.span) {
                evs.push(e);
            }
        }
        push_gap(
             sc,
            counters,
            items,
            GapClass::MultiDriver,
            Confidence::Certain,
            format!("{name} 被 {distinct} 個驅動源賦值(多驅動衝突)"),
            evs,
            format!("構造兩驅動源同拍衝突輸入,釘住 {name} 的解析序(依 AGENTS §2 調度確定性契約,ADR-0001)"),
        );
    }
}

// ---- G7 邊沿行程阻塞寫(§3.1;certain) ----

fn scan_blocking_in_seq(ctx: &Ctx, sc: &ModuleScan, counters: &mut HashMap<&'static str, usize>, items: &mut Vec<GapItem>) {
    for a in &sc.assigns {
        let Some(pid) = a.proc else {
            continue;
        };
        if a.nb || sc.proc_class(pid) != Some(ProcClass::Edge) {
            continue;
        }
        let Some(base) = lvalue_base(a.lhs) else {
            continue;
        };
        let other_writer = sc.drivers.get(&base).is_some_and(|sites| {
            sites
                .iter()
                .any(|s| !matches!(s.source, DriverSource::Proc(p) if p == pid))
        });
        if other_writer {
            let mut evs = Vec::new();
            if let Some(e) = ev(ctx, a.span) {
                evs.push(e);
            }
            push_gap(
                 sc,
                counters,
                items,
                GapClass::BlockingInSeq,
                Confidence::Certain,
                format!("邊沿行程內對 {base} 的阻塞賦值與其他行程的賦值競爭(跨行程同邊沿面)"),
                evs,
                format!("構造兩行程同邊沿競爭場景釘住 {base},或標 race-ok 豁免"),
            );
        }
    }
}

// ---- 共用小件 ----

// ---- G8 常量死分支(§17 拓展性實證;certain) ----
// 判定:If 條件為純字面量(NumLit),值可判定且無 x/z 位(digits 含
// x/z → lit_u64 None → 不報,X-safe):恆假(值=0)→ then 臂不可達;
// 恆真(值≠0)→ else 臂不可達(若有)。param 名不求值(§11.5 非目標)。
// 有序契約(§16.1.5 契約③):語句樹序遍歷,零 HashSet 迭代面。

fn collect_ifs<'a>(s: &'a Stmt, out: &mut Vec<&'a Stmt>) {
    match s {
        Stmt::Block { body, .. } => body.iter().for_each(|b| collect_ifs(b, out)),
        Stmt::If { then, els, .. } => {
            out.push(s);
            collect_ifs(then, out);
            if let Some(e) = els {
                collect_ifs(e, out);
            }
        }
        Stmt::Case { items, .. } => {
            for it in items {
                match it {
                    ast::CaseItem::Items(_, st) => collect_ifs(st, out),
                    ast::CaseItem::Default(st) => collect_ifs(st, out),
                }
            }
        }
        Stmt::For { body, .. }
        | Stmt::While { body, .. }
        | Stmt::Repeat { body, .. }
        | Stmt::Forever { body, .. } => collect_ifs(body, out),
        Stmt::Fork { branches, .. } => branches.iter().for_each(|b| collect_ifs(b, out)),
        // 非容器葉子(BAssign/DelayCtrl/Null 等)。**新增容器型 Stmt 變體
        // 須在此展開**(extension recipe §17.2 步驟 b;漏展開=靜默漏報)。
        _ => {}
    }
}

fn scan_dead_branches(ctx: &Ctx, sc: &ModuleScan, counters: &mut HashMap<&'static str, usize>, items: &mut Vec<GapItem>) {
    for p in &sc.procs {
        let mut ifs: Vec<&Stmt> = Vec::new();
        collect_ifs(&p.body, &mut ifs);
        for s in ifs {
            let Stmt::If { span, cond, els, .. } = s else {
                continue;
            };
            let Some(v) = lit_u64(cond) else {
                continue;
            };
            let (dead_then, msg) = if v == 0 {
                (true, format!("條件 {} 恆假,then 臂不可達(死分支)", render(cond)))
            } else {
                (false, format!("條件 {} 恆真,else 臂不可達(死分支)", render(cond)))
            };
            if dead_then || els.is_some() {
                let mut evs = Vec::new();
                if let Some(e) = ev(ctx, *span) {
                    evs.push(e);
                }
                push_gap(
                    sc,
                    counters,
                    items,
                    GapClass::DeadBranch,
                    Confidence::Certain,
                    msg,
                    evs,
                    "移除死臂或把條件改為可變表達式;測試釘住活臂行為".to_string(),
                );
            }
        }
    }
}

fn literal_width(r: &ast::Range) -> Option<u32> {
    let msb = lit_u64(&r.msb)?;
    let lsb = lit_u64(&r.lsb)?;
    Some(msb.abs_diff(lsb).saturating_add(1).min(u32::MAX as u64) as u32)
}

fn lit_u64(e: &Expr) -> Option<u64> {
    let n = match e {
        Expr::NumLit(n) => n,
        _ => return None,
    };
    let radix = match n.base {
        ast::Base::Dec => 10,
        ast::Base::Bin => 2,
        ast::Base::Oct => 8,
        ast::Base::Hex => 16,
    };
    let digits: String = n.digits.chars().filter(|&c| c != '_').collect();
    if digits.is_empty() {
        return None;
    }
    u64::from_str_radix(&digits, radix).ok()
}
