//! SV 投影:vl-syntax AST → `view::ModuleInfo`(ANALYZER-SERVICE §4.2;
//! 既有欄位直讀,零新語義)。

use std::collections::{HashMap, HashSet};

use vl_syntax::ast;

use crate::view::{span_info, CellInfo, Dir, ModuleInfo, NetInfo, NetKindTag, ParamInfo, PortInfo};
use crate::{Analysis, InputKind, SvInput};

/// SV 管線:pp → lex → parse 全管線直呼既有函數,投影 + 診斷透傳。
pub(crate) fn analyze_sv(input: &SvInput) -> Analysis {
    let mut sm = rcs_diag::SourceMap::new();
    let mut diags = rcs_diag::DiagSink::new();
    let opts = vl_syntax::pp::PpOptions {
        include_dirs: input.include_dirs.clone(),
        predefines: input.defines.clone(),
    };
    let pp_out = vl_syntax::pp::preprocess_source(&input.name, &input.source, &mut sm, &mut diags, &opts);
    let ast_file = vl_syntax::parser::parse_source_file(pp_out.tokens, &mut diags);
    let modules = project(&ast_file, &sm);
    let mut text = String::new();
    rcs_diag::render_diags(&mut text, &sm, &diags.diags);
    Analysis {
        kind: InputKind::Sv,
        diagnostics: diags.diags,
        diag_text: text,
        modules,
        sv: Some(ast_file),
        rtlil: None,
        sm: Some(sm),
    }
}

pub(crate) fn project(file: &ast::SourceFile, sm: &rcs_diag::SourceMap) -> Vec<ModuleInfo> {
    file.modules.iter().map(|m| project_module(m, sm)).collect()
}

/// 模組體走查收集器:Decl/Instance/proc/assign 計數;generate 區
/// (GenIf/GenFor/GenCase/GenBlock/GenerateRegion)扁平遞入——generate 內
/// 實例必須進 cells/實例邊(S1 驗收「generate 實例」);作用域前綴=新語義,
/// 不發明(§S1 實作記錄)。Function/Task 局部聲明不進模組面。
pub(crate) struct Collected<'a> {
    pub decls: Vec<&'a ast::Decl>,
    pub instances: Vec<&'a ast::Instance>,
    pub proc_count: usize,
    pub assign_count: usize,
}

pub(crate) fn collect(items: &[ast::ModuleItem]) -> Collected<'_> {
    let mut out = Collected {
        decls: Vec::new(),
        instances: Vec::new(),
        proc_count: 0,
        assign_count: 0,
    };
    walk(items, &mut out);
    out
}

fn walk<'a>(items: &'a [ast::ModuleItem], out: &mut Collected<'a>) {
    for it in items {
        match it {
            ast::ModuleItem::Decl(d) => out.decls.push(d),
            ast::ModuleItem::ContAssign(pairs) => out.assign_count += pairs.len(),
            ast::ModuleItem::Always { .. } | ast::ModuleItem::Initial { .. } => out.proc_count += 1,
            ast::ModuleItem::Instance(i) => out.instances.push(i),
            ast::ModuleItem::GenerateRegion(items) => walk(items, out),
            ast::ModuleItem::GenIf { then, els, .. } => {
                walk(then, out);
                if let Some(e) = els {
                    walk(e, out);
                }
            }
            ast::ModuleItem::GenFor { items, .. } => walk(items, out),
            ast::ModuleItem::GenCase { items, default, .. } => {
                for (_, its) in items {
                    walk(its, out);
                }
                if let Some(d) = default {
                    walk(d, out);
                }
            }
            ast::ModuleItem::GenBlock { items, .. } => walk(items, out),
            // Function/Task 局部聲明、Defparam、Empty 不進模組查詢面。
            _ => {}
        }
    }
}

fn project_module(m: &ast::Module, sm: &rcs_diag::SourceMap) -> ModuleInfo {
    let collected = collect(&m.items);
    let port_names: HashSet<&str> = m.ports.iter().map(|p| p.name.as_str()).collect();

    // 非 ANSI 回填映射(§4.2 雙腿規則):Port.dir/net/signed/range 恆 None
    // (parser/module.rs 非 ANSI 分支),方向與寬度在模組體 Decl 承載;
    // 按 DeclEntry.name 回填,首個帶 dir 的 Decl 勝。ANSI 模組不建此映射
    //(兩腿分立,互不污染)。
    let mut decl_dir_map: HashMap<&str, (ast::Direction, bool, Option<&ast::Range>)> = HashMap::new();
    if !m.ansi {
        for d in &collected.decls {
            if let Some(dir) = d.dir {
                for e in &d.entries {
                    decl_dir_map
                        .entry(e.name.as_str())
                        .or_insert((dir, d.signed, d.range.as_ref()));
                }
            }
        }
    }

    let mut params: Vec<ParamInfo> = m
        .params
        .iter()
        .map(|pe| ParamInfo { name: pe.name.clone(), value: expr_value(&pe.init) })
        .collect();
    // 體內 `parameter` 宣告(本弧裁定收錄;localparam 不收,見 view.rs 注)。
    for d in &collected.decls {
        if d.kind == ast::DeclKind::Parameter {
            for e in &d.entries {
                params.push(ParamInfo {
                    name: e.name.clone(),
                    value: e.init.as_ref().map(expr_value).unwrap_or_default(),
                });
            }
        }
    }

    let mut ports = Vec::with_capacity(m.ports.len());
    let mut last_dir: Option<ast::Direction> = None;
    for p in &m.ports {
        // ANSI 腿:parser 已按 LRM §12.3.3 繼承(ast.rs:94-96 doc);投影端
        // 再攜帶 last_dir 保底。非 ANSI 腿:回填映射。孤兒(兩腿皆無證據)
        // → Inout + width 0 佔位(§4.2 佔位語義收窄版)。
        let bf = decl_dir_map.get(p.name.as_str());
        let dir = p
            .dir
            .or(last_dir)
            .map(to_dir)
            .or_else(|| bf.map(|(d, _, _)| to_dir(*d)))
            .unwrap_or(Dir::Inout);
        if p.dir.is_some() {
            last_dir = p.dir;
        }
        let (width, parametric) = match p.range.as_ref().or_else(|| bf.and_then(|(_, _, r)| *r)) {
            Some(r) => range_width(r),
            // 孤兒:寬度未知 → 0;其餘無範圍 = 標量默認 1。
            None if p.dir.is_none() && bf.is_none() => (0, false),
            None => (1, false),
        };
        let signed = p.signed || bf.map(|(_, s, _)| *s).unwrap_or(false);
        ports.push(PortInfo {
            name: p.name.clone(),
            dir,
            width,
            parametric,
            signed,
            span: span_info(sm, p.span),
        });
    }

    let mut nets = Vec::new();
    for d in &collected.decls {
        // parameter/localparam 不是 net(§13 詞表不含)。
        if matches!(d.kind, ast::DeclKind::Parameter | ast::DeclKind::Localparam) {
            continue;
        }
        let (width, _parametric) = d.range.as_ref().map(range_width).unwrap_or((1, false));
        for e in &d.entries {
            nets.push(NetInfo {
                name: e.name.clone(),
                kind: NetKindTag(kind_str(d.kind).to_string()),
                width,
                is_port: port_names.contains(e.name.as_str()),
                unpacked_dims: e.unpacked.len().min(u32::MAX as usize) as u32,
                span: span_info(sm, e.span),
            });
        }
    }

    let cells: Vec<CellInfo> = collected
        .instances
        .iter()
        .flat_map(|inst| inst.insts.iter().map(move |u| (inst, u)))
        .map(|(inst, u)| CellInfo {
            name: u.name.clone(),
            ctype: inst.mod_name.clone(),
            port_count: u.conns.len(),
            span: span_info(sm, u.span),
        })
        .collect();

    ModuleInfo {
        name: m.name.clone(),
        params,
        ports,
        nets,
        cells,
        proc_count: collected.proc_count,
        assign_count: collected.assign_count,
        span: span_info(sm, m.span),
    }
}

fn to_dir(d: ast::Direction) -> Dir {
    match d {
        ast::Direction::Input => Dir::Input,
        ast::Direction::Output => Dir::Output,
        ast::Direction::Inout => Dir::Inout,
    }
}

/// §13 詞表:DeclKind 既有變體名;Net(x) 用內層 net 名(wire/tri/…)。
fn kind_str(k: ast::DeclKind) -> &'static str {
    match k {
        ast::DeclKind::Net(n) => n.as_str(),
        ast::DeclKind::Reg => "reg",
        ast::DeclKind::Integer => "integer",
        ast::DeclKind::Real => "real",
        ast::DeclKind::Realtime => "realtime",
        ast::DeclKind::Time => "time",
        ast::DeclKind::Genvar => "genvar",
        ast::DeclKind::Event => "event",
        // 上游已過濾;保守給名不 panic(投影永不 panic 契約)。
        ast::DeclKind::Parameter => "parameter",
        ast::DeclKind::Localparam => "localparam",
    }
}

/// 範圍寬度派生(§4.2):字面量 msb/lsb 直取 → |msb-lsb|+1;非字面量 →
/// (0, true) parametric 標記,不求值參數。
fn range_width(r: &ast::Range) -> (u32, bool) {
    match (lit_u64(&r.msb), lit_u64(&r.lsb)) {
        (Some(msb), Some(lsb)) => (
            (msb.abs_diff(lsb).saturating_add(1)).min(u32::MAX as u64) as u32,
            false,
        ),
        _ => (0, true),
    }
}

/// 字面量求值(僅 NumLit 純數字位;x/z/? 與溢出 → None=parametric)。
/// based 字面量的 digits 保留源碼下劃線(lexer.rs U-3b parity:「every value
/// parser filters '_' itself」)——本層即 value parser,求值前剝除;剝後
/// x/z/? 等非數值字符仍 `from_str_radix` Err → None 走 parametric,零誤傷。
fn lit_u64(e: &ast::Expr) -> Option<u64> {
    let n = match e {
        ast::Expr::NumLit(n) => n,
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

/// §4.5 參數值文本形:字面量渲染,非字面量 → `<expr>` 佔位(不求值)。
pub(crate) fn expr_value(e: &ast::Expr) -> String {
    match e {
        ast::Expr::NumLit(n) => render_num_lit(n),
        ast::Expr::StrLit { value, .. } => {
            format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
        }
        ast::Expr::RealLit { text, .. } => text.clone(),
        _ => "<expr>".to_string(),
    }
}

/// NumLit → `W'sdDIGITS` 既有欄位拼回(width None → 無寬前綴)。
fn render_num_lit(n: &ast::NumLit) -> String {
    let mut s = String::new();
    if let Some(w) = n.width {
        s.push_str(&w.to_string());
    }
    s.push('\'');
    if n.signed {
        s.push('s');
    }
    s.push(match n.base {
        ast::Base::Dec => 'd',
        ast::Base::Bin => 'b',
        ast::Base::Oct => 'o',
        ast::Base::Hex => 'h',
    });
    s.push_str(&n.digits);
    s
}
