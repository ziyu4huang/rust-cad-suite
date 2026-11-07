//! SV 側提取器(§4.2):走 `Analysis.sv` 逃逸口,五基類
//! reset/fsm/timing/arithmetic/counter;行程分類律=R1-1 三肢判準。
//!
//! 證據行號通路:pp 對主檔 `SourceMap::add(name, src)` 註冊**原始文本**
//! (pp.rs:63),主檔恆 FileId(0);span 行號經 `Analysis::line_col`
//! (公開面),snippet 按 line 區間切**原始源文本**——V4 逐位元組對拍的
//! 兩端同一定義。巨集/include 產生的 span(file≠0)不產證據:整規則
//! 無證即丟(誠實降級,記入實作記錄)。

use std::collections::{BTreeSet, HashMap, HashSet};

pub(crate) mod gaps;

use analyzer_core::view::ModuleInfo;
use analyzer_core::{Analysis, SourceInput, SvInput};
use rcs_diag::{FileId, Span};
use vl_syntax::ast::{self, EdgeKind, EventCtrl, ModuleItem, Stmt};

use crate::expr::{self, lvalue_base, render, walk_expr};
use crate::lint;
use crate::schema::{
    Category, Check, CheckForm, CheckPayload, Contract, ContractParam, ContractPort,
    ContractSignal, DeclLint, Effect, Evidence, EvidenceClass, ModuleRules, Qualifier, Rule,
    Rulebook, SkippedNote, SkipReason, Trigger,
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
            // M2(§5.2 S4)前置掃描:全部顯式標簽條件(default 臂推「未被任何
            // 臂覆蓋」=對每個顯式標簽取否定合取;多標簽臂每個標簽一條)。
            let label_conds: Vec<String> = items
                .iter()
                .filter_map(|it| match it {
                    ast::CaseItem::Items(ls, _) => Some(
                        ls.iter()
                            .map(|l| format!("{} == {}", render(expr), render(l)))
                            .collect::<Vec<_>>(),
                    ),
                    _ => None,
                })
                .flatten()
                .collect();
            for item in items {
                match item {
                    ast::CaseItem::Items(labels, s) => {
                        // M1(§5.2 S4):多標簽臂逐標簽展開——每個標簽一條適用
                        // 域(2'b01 觸發域可見),不再只取首標簽(單標簽臂形不變)。
                        if labels.is_empty() {
                            walk_stmt(s, conds, out);
                        } else {
                            for l in labels {
                                conds.push(format!("{} == {}", render(expr), render(l)));
                                walk_stmt(s, conds, out);
                                conds.pop();
                            }
                        }
                    }
                    ast::CaseItem::Default(s) => {
                        // M2(§5.2 S4):default 臂=未被任何顯式臂覆蓋的編碼域
                        // (機械可導;進指紋可區分域)。無顯式臂=條件不可導,
                        // 保持無條件行走。
                        if label_conds.is_empty() {
                            walk_stmt(s, conds, out);
                        } else {
                            let base = conds.len();
                            for c in &label_conds {
                                conds.push(format!("!({c})"));
                            }
                            walk_stmt(s, conds, out);
                            conds.truncate(base);
                        }
                    }
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
    contract: Contract,
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
        contract,
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
            decl_lint: DeclLint::default(),
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
    let mut decl_b_set: BTreeSet<String> = BTreeSet::new();
    // B 面(投影,兩腿同查詢形):宣告名按歸一律入集;呼叫端擴充面見 lint.rs 註。
    for mi in analysis.modules() {
        for p in &mi.params {
            lint::insert_decl_name(&mut decl_b_set, &p.name);
        }
        for p in &mi.ports {
            lint::insert_decl_name(&mut decl_b_set, &p.name);
        }
        for n in &mi.nets {
            lint::insert_decl_name(&mut decl_b_set, &n.name);
        }
    }
    // 模組名入 B(規則/證據行可引他處模組名的場景=宣告面同源)。
    if let Some(file) = &analysis.sv {
        let mi_by_name: HashMap<&str, &ModuleInfo> = analysis
            .modules()
            .iter()
            .map(|mi| (mi.name.as_str(), mi))
            .collect();
        for m in &file.modules {
            decl_b_set.insert(m.name.clone());
            // localparam/函數/任務名=檔內具名宣告(AST 補收;投影面不載,
            // 見 analyzer-core view.rs params 裁定注)。漏收會讓 lint 對
            // 合法引用誤報(ST_* 編碼/任務呼叫=基線非綠)。
            collect_local_decl_names(&m.items, &mut decl_b_set);
            let mi = mi_by_name.get(m.name.as_str());
            let contract = match mi {
                Some(mi) => build_contract(mi, m),
                None => Contract {
                    params: Vec::new(),
                    ports: Vec::new(),
                    signals: Vec::new(),
                },
            };
            let drafts = extract_module(&ctx, m);
            modules.push(finalize_module(
                &input.name,
                &m.name,
                contract,
                drafts,
                max_rules,
                &mut truncated,
            ));
        }
    }
    // N5 lint(審讀 #6):A=規則書全部 evidence/check 行引用標識符 vs
    // B=原檔宣告集;差集非空=提取器懸空引用。隨輸出 golden 每輪重算。
    let a_set = lint::rulebook_ref_idents(modules.iter().flat_map(|m| &m.rules));
    let decl_lint = DeclLint {
        dangling_refs: lint::diff_sets(&a_set, &decl_b_set),
    };
    Rulebook {
        source: input.name.clone(),
        parse_errors,
        skipped: Vec::new(),
        modules,
        truncated,
        decl_lint,
    }
}

/// 檔內具名宣告 AST 補收(localparam/函數/任務;generate 作用域含)。
/// 【S1 實測擴充記錄】37 檔 lint 實測(canonical B=投影+localparam+函數名)
/// 懸空 3/37,逐案歸類皆為「檔內具名宣告面」而 canon 括號清單未列——目的
/// 條款「規則書引用了原檔不存在的信號=提取器 bug」下報之皆誤報,故按證據
/// 擴入:①命名 generate 塊標籤(carryadd `begin:STAGE`/generate.v
/// `begin : foo`,被層級引用 `STAGE[i-1].C`);②行程命名塊+塊內聲明
/// (sincos `begin: DESIGN_PROCESSOR` 內 reg i/state/dz/dx/dy/y/x/z);
/// ③函數/任務頭尾聲明(ports+decls)。三面皆=原檔具名宣告,與「端口+
/// wire/reg+localparam/參數」同屬宣告面;逐案證據入切片對賬。
fn collect_local_decl_names(items: &[ModuleItem], out: &mut BTreeSet<String>) {
    // 自遞歸(不走 walk_items:GenFor/GenBlock 被 walk_items 消費,閉包看不到
    // 其 block/name 欄位——GenFor.block=`for (...) begin : STAGE` 標籤載體)。
    for item in items {
        match item {
            ModuleItem::GenerateRegion(inner) => collect_local_decl_names(inner, out),
            ModuleItem::GenIf { then, els, .. } => {
                collect_local_decl_names(then, out);
                if let Some(e) = els {
                    collect_local_decl_names(e, out);
                }
            }
            ModuleItem::GenFor { block, items: inner, .. } => {
                if let Some(n) = block {
                    out.insert(n.clone());
                }
                collect_local_decl_names(inner, out);
            }
            ModuleItem::GenCase { items, default, .. } => {
                for (_, its) in items {
                    collect_local_decl_names(its, out);
                }
                if let Some(d) = default {
                    collect_local_decl_names(d, out);
                }
            }
            ModuleItem::GenBlock { name, items: inner, .. } => {
                if let Some(n) = name {
                    out.insert(n.clone());
                }
                collect_local_decl_names(inner, out);
            }
            ModuleItem::Decl(d) => {
                if d.kind == ast::DeclKind::Localparam {
                    for e in &d.entries {
                        out.insert(e.name.clone());
                    }
                }
            }
            ModuleItem::Function(f) | ModuleItem::Task(f) => {
                out.insert(f.name.clone());
                for d in f.ports.iter().chain(f.decls.iter()) {
                    for e in &d.entries {
                        out.insert(e.name.clone());
                    }
                }
                for s in &f.body {
                    collect_stmt_decl_names(s, out);
                }
            }
            ModuleItem::Always { body, .. } | ModuleItem::Initial { body, .. } => {
                collect_stmt_decl_names(body, out);
            }
            _ => {}
        }
    }
}

/// 語句樹具名面收集(命名 begin 塊標籤+塊內聲明;嵌套全展開)。
fn collect_stmt_decl_names(s: &Stmt, out: &mut BTreeSet<String>) {
    match s {
        Stmt::Block { name, decls, body, .. } => {
            if let Some(n) = name {
                out.insert(n.clone());
            }
            for d in decls {
                for e in &d.entries {
                    out.insert(e.name.clone());
                }
            }
            for b in body {
                collect_stmt_decl_names(b, out);
            }
        }
        Stmt::If { then, els, .. } => {
            collect_stmt_decl_names(then, out);
            if let Some(e) = els {
                collect_stmt_decl_names(e, out);
            }
        }
        Stmt::Case { items, .. } => {
            for it in items {
                match it {
                    ast::CaseItem::Items(_, st) => collect_stmt_decl_names(st, out),
                    ast::CaseItem::Default(st) => collect_stmt_decl_names(st, out),
                }
            }
        }
        Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::Repeat { body, .. }
        | Stmt::Forever { body, .. } => collect_stmt_decl_names(body, out),
        Stmt::DelayCtrl { body, .. } | Stmt::EventCtrl { body, .. } | Stmt::Wait { body, .. } => {
            if let Some(b) = body {
                collect_stmt_decl_names(b, out);
            }
        }
        Stmt::Fork { branches, .. } => {
            for b in branches {
                collect_stmt_decl_names(b, out);
            }
        }
        _ => {}
    }
}

/// 模組接口契約(§5.2 S1):①端口/參數面+②內部聲明清單;來源=結構投影
/// 既有聲明面(ModuleInfo 直投,零新語義),localparam 由 AST 補收(值=
/// AST 原文渲染,對拍友善;投影 params 面裁定不收 localparam)。
fn build_contract(mi: &ModuleInfo, m: &ast::Module) -> Contract {
    let mut params: Vec<ContractParam> = mi
        .params
        .iter()
        .map(|p| ContractParam {
            name: p.name.clone(),
            value: p.value.clone(),
            local: false,
        })
        .collect();
    walk_items(&m.items, false, &mut |item, _| {
        if let ModuleItem::Decl(d) = item {
            if d.kind == ast::DeclKind::Localparam {
                for e in &d.entries {
                    params.push(ContractParam {
                        name: e.name.clone(),
                        value: e.init.as_ref().map(render).unwrap_or_default(),
                        local: true,
                    });
                }
            }
        }
    });
    let ports = mi
        .ports
        .iter()
        .map(|p| ContractPort {
            name: p.name.clone(),
            dir: p.dir.as_str().to_string(),
            width: p.width,
            parametric: p.parametric,
            signed: p.signed,
        })
        .collect();
    // 內部聲明=nets 剔除端口面(!is_port);width=0=參數化範圍佔位(§4.2)。
    let signals = mi
        .nets
        .iter()
        .filter(|n| !n.is_port)
        .map(|n| ContractSignal {
            name: n.name.clone(),
            kind: n.kind.as_str().to_string(),
            width: n.width,
            unpacked_dims: n.unpacked_dims,
        })
        .collect();
    Contract {
        params,
        ports,
        signals,
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
                // S2(N3)分界:ContAssign 無分支域(in_branch_domain=false)——
                // assign 純常量直賦/直通別名不產(負 fixture 釘死);結構形
                // RHS(PartSelect/Concat/Repeat)與含二元運算者一律產。
                if expr::relation_worthy(&p.rhs, false) {
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
                // S2(N3)分界:分支域=conds 非空(case/if 臂內;default 臂
                // walk_stmt 不 push cond=非分支域,歸 S4/M2)。結構形一律產;
                // 純常量僅分支域內;無條件直賦純常量/直通別名不產(A12 教訓
                // 寫進 relation_worthy 註釋)。
                if expr::relation_worthy(a.rhs, !a.conds.is_empty()) {
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
    // 頂層解 Block。
    let top: Vec<&Stmt> = match body {
        Stmt::Block { body, .. } => body.iter().collect(),
        s => vec![s],
    };
    // D-r1c(§19.2 C-S1):先由頂層 If 求復位候選,再取觸發沿=敏感列表中首個
    // 訊號 ≠復位候選的 pos/neg 沿(無則回退首沿);select_trigger=規則面與破口面
    // (gaps.rs 復位臂識別)的單一語義源。修前取首沿:復位前置序
    // `@(negedge rst_n or posedge clk)` 把復位沿誤作觸發→復位臂失明(mem_arst 實錄)。
    let reset_cand: Option<&str> = match top.first() {
        Some(Stmt::If { cond, .. }) => reset_cond_signal(cond),
        _ => None,
    };
    let trigger = select_trigger(&edges, reset_cand).map(|e| Trigger {
        edge: Some(match e.kind {
            EdgeKind::Posedge => "posedge".to_string(),
            _ => "negedge".to_string(),
        }),
        signal: Some(render(&e.expr)),
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

    // 復位臂:首層 If 且 cond=單訊號(±否定);復位訊號≠觸發訊號。
    // M3(§5.2):else 臂賦值補 !(reset 條件) 包裹——非復位行為的適用域
    // 顯式攜「非復位」,與復位臂 guard 邏輯互斥(muxtree R003/R004 現狀
    // 自相矛盾 audit 在檔);if 後的頂層語句兩臂共用,不帶基條件。
    let mut reset_when: Option<String> = None;
    let mut reset_arm: Option<&Stmt> = None;
    let mut continuation: Vec<(&Stmt, Option<String>)> = Vec::new();
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
                let base = format!("!({})", render(cond));
                if let Some(e) = els {
                    continuation.push((&**e, Some(base)));
                }
                for s in top.iter().skip(1) {
                    continuation.push((s, None));
                }
            }
        }
    }
    if reset_arm.is_none() {
        continuation.extend(top.iter().map(|s| (*s, None)));
    }

    // 1) reset 規則:then 臂每個賦值一條 guard(evidence=then 臂 span+
    // process 敏感列表 span——N4 async 分類的可審計面)。
    if let (Some(arm), Some(when)) = (reset_arm, reset_when.clone()) {
        let mut conds = Vec::new();
        let mut out = Vec::new();
        walk_stmt(arm, &mut conds, &mut out);
        let ev = ctx.evidence(stmt_span(arm), EvidenceClass::Process);
        // N4(§5.2):全邊收集分類——復位訊號帶自有沿(如 negedge rst_n)=
        // 異步;敏感列表原文入 evidence(第二證)。
        let reset_sig = top.first().and_then(|s| match s {
            Stmt::If { cond, .. } => reset_cond_signal(cond).map(|x| x.to_string()),
            _ => None,
        });
        let r#async = reset_sig
            .map(|s| {
                edges.iter().any(|e| {
                    matches!(e.kind, EdgeKind::Posedge | EdgeKind::Negedge) && render(&e.expr) == s
                })
            })
            .unwrap_or(false);
        let sens_ev = ctrl.and_then(|c| ctx.evidence(c.span, EvidenceClass::Process));
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
            let mut evidence = vec![ev];
            if let Some(se) = sens_ev.clone() {
                evidence.push(se);
            }
            drafts.push(Draft {
                category: Category::Reset,
                qualifier: Qualifier::Derived,
                statement: stmt_text,
                evidence,
                check: Check {
                    form: CheckForm::Guard,
                    payload: CheckPayload::Guard {
                        trigger: trigger.clone(),
                        when: Some(when.clone()),
                        r#async,
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
        for (s, _) in &continuation {
            names.extend(nb_lhs_names(s));
        }
        names
    };
    let mut consumed: HashSet<u32> = HashSet::new(); // span.start.0
    let mut cases: Vec<CaseLike> = Vec::new();
    collect_cases(&continuation.iter().map(|(s, _)| *s).collect::<Vec<_>>(), &mut cases);
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
    // N1(§5.2):Sequence when 維並入 conds(指紋可區分適用域——i2c
    // HOLD/IDLE 臂 tip/bitcnt 條件賦值現狀被指紋去重吞掉,E2 gap 關閉);
    // M3:else 臂語句以 !(reset 條件) 為外層條件(seed)。
    for (s, base) in &continuation {
        let mut conds = Vec::new();
        if let Some(b) = base {
            conds.push(b.clone());
        }
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
            let when = if a.conds.is_empty() {
                None
            } else {
                Some(a.conds.join(" 且 "))
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
                        when,
                    },
                },
            });
        }
    }
}

/// 觸發沿選取(D-r1c,§19.2 C-S1)——規則面 `extract_edge_process` 與破口面
/// `gaps.rs` 復位臂識別的**單一語義源**(C-S2 tb_plan 時鐘識別復用同一函數)。
/// trigger=敏感列表中第一個訊號 ≠ 復位候選 `reset_sig` 的 posedge/negedge;
/// 無此邊沿(或無復位候選)時回退首個 pos/neg 邊沿——回退保證既有形零漂移:
/// 單沿 `@(negedge rst_n) if (!rst_n)` 仍取 rst_n(is_trigger→無復位臂)。
/// 訊號比較鍵=`render(邊沿表達式)`(與 Trigger.signal 同一渲染,規則面判準;
/// 破口面舊用 lvalue_base,對裸標識符兩者恆同——in-git 語料 rg 普查非標識符
/// 邊沿表達式 0 例)。
pub(crate) fn select_trigger<'a>(
    edges: &[&'a ast::EventEdge],
    reset_sig: Option<&str>,
) -> Option<&'a ast::EventEdge> {
    let is_edge = |e: &ast::EventEdge| matches!(e.kind, EdgeKind::Posedge | EdgeKind::Negedge);
    let first = edges.iter().copied().find(|e| is_edge(e));
    match reset_sig {
        Some(rs) => edges
            .iter()
            .copied()
            .find(|e| is_edge(e) && render(&e.expr) != rs)
            .or(first),
        None => first,
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
