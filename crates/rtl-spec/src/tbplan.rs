//! tb_plan — 規則書→測試計劃與模板 TB(§19.3.2 C-S2;D-C2/D-C3)。
//!
//! 語義姿態同 crate 慣例:永不 panic、錯誤走 `TbError`(計劃期攔下,模板
//! 生成不可失敗);輸入=源文本標量(零 FS 觸碰);clocks 經
//! [`crate::sv::select_trigger`]=與規則面/破口面同一語義源(C-S1 單一源);
//! Vec 一律按規則序/行程序(禁 HashSet/HashMap 迭代面進輸出,§16.1.5 契約③);
//! JSON 手寫(鍵序=寫出序 byte-exact;編碼器復用 analyzer_core Json,只寫不讀)。
//!
//! 硬錯(AGENTS §3)一律 `error: <原因> (milestone TB1)`;前端診斷自帶
//! (milestone Mx) 者保留其 Mx(§19.3.2 ①)。

use analyzer_core::proto::json::Json;
use analyzer_core::{analyze, Analysis, SourceInput};
use rcs_diag::{FileId, Span};
use vl_syntax::ast::{self, EdgeKind, ModuleItem, Stmt};

use crate::expr::render;
use crate::schema::{CheckPayload, ContractPort, DEFAULT_MAX_RULES};
use crate::sv::{extract_parse_errors, reset_cond_signal, select_trigger, walk_items};

/// TbOpts 默認值 SSOT(§19.3.2;xorshift32 不收 0,seed 默認為非零常量)。
pub const DEFAULT_SEED: u32 = 305_419_896; // 0x12345678
/// 每個隨機場景的拍數默認。
pub const DEFAULT_CYCLES: u32 = 32;
/// 無復位時序設計首次觀測前驅動拍數默認。
pub const DEFAULT_FLUSH: u32 = 8;
/// 純組合窮舉上限(Σ輸入位;§19.3.2 S-EXH)。
const MAX_EXHAUSTIVE_BITS: u32 = 12;
/// 純組合隨機向量數(超窮舉上限時與 S-SMALL;§19.3.2)。
const COMB_RANDOM_VECTORS: u32 = 256;

/// 計劃期錯誤(RV1-11:Result API;Display 一律帶 milestone 指針)。
#[derive(Clone, Debug)]
pub struct TbError {
    reason: String,
}

impl TbError {
    fn tb1(reason: impl Into<String>) -> Self {
        TbError {
            reason: format!("{} (milestone TB1)", reason.into()),
        }
    }
}

impl std::fmt::Display for TbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error: {}", self.reason)
    }
}

/// 計劃條目:端口(TB 可用面;parametric/inout 已在計劃期攔下)。
#[derive(Clone, Debug)]
pub struct PortPlan {
    pub name: String,
    pub dir: String,
    pub width: u32,
    pub signed: bool,
}

#[derive(Clone, Debug)]
pub struct ClockPlan {
    pub edge: String,
    pub signal: String,
}

#[derive(Clone, Debug)]
pub struct ResetPlan {
    pub signal: String,
    /// true=`!rst_n` 形(低有效)。
    pub active_low: bool,
    /// 復位訊號帶自有沿(C-S1 select_trigger 分類;§5.2 S3 同源)。
    pub r#async: bool,
    pub rule_ids: Vec<String>,
}

/// 場景覆蓋聲明:規則 id,或 `*`(非定向場景)。
#[derive(Clone, Debug)]
pub enum Cover {
    Rule(String),
    Star,
}

#[derive(Clone, Debug)]
pub struct ScenePlan {
    pub name: String,
    /// rst|arst|full|small|hold|flush|exh|rand
    pub kind: &'static str,
    pub covers: Vec<Cover>,
}

#[derive(Clone, Debug)]
pub struct Obligation {
    pub rule_id: String,
    pub form: String,
    pub category: String,
    /// port=主體是輸出端口;indirect=內部訊號(R1-9:禁層次引用面)。
    pub observe: &'static str,
}

#[derive(Clone, Debug)]
pub struct Waiver {
    pub rule_id: String,
    /// x-valued(v1 唯一機械可判因;unobservable 由閘側 C-S3 補)。
    pub reason: &'static str,
}

#[derive(Clone, Debug)]
pub struct TbPlan {
    pub module: String,
    pub params: Vec<(String, String, bool)>,
    pub ports: Vec<PortPlan>,
    pub clocks: Vec<ClockPlan>,
    pub resets: Vec<ResetPlan>,
    /// DUT 全部沿行程敏感列表 (沿, 訊號) 有序去重(TB-R1x 的 E;RV1-5)。
    pub sens_edges: Vec<(String, String)>,
    pub data_inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub obligations: Vec<Obligation>,
    pub waivers: Vec<Waiver>,
    pub scenes: Vec<ScenePlan>,
}

/// 位字面量含 x/z/?(`'b…x`/`'h…z`/`1'bx`/`'sd?` 等形;值/becomes/rhs 掃描)。
fn has_xz_bit_literal(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut k = 0usize;
    while k < bytes.len() {
        if bytes[k] == b'\'' {
            let mut j = k + 1;
            if j < bytes.len() && matches!(bytes[j], b's' | b'S') {
                j += 1; // signed 前綴
            }
            if j < bytes.len() {
                let base = bytes[j] | 0x20; // 小寫化
                let digit_ok = |c: u8| match base {
                    b'b' => c.is_ascii_hexdigit(),
                    b'o' => (b'0'..=b'7').contains(&c),
                    b'h' => c.is_ascii_hexdigit(),
                    b'd' => c.is_ascii_digit(),
                    _ => false,
                };
                if matches!(base, b'b' | b'o' | b'h' | b'd') {
                    let mut m = j + 1;
                    let mut xz = false;
                    while m < bytes.len() {
                        let c = bytes[m] | 0x20;
                        if c == b'x' || c == b'z' || c == b'?' {
                            xz = true;
                            m += 1;
                        } else if digit_ok(c) || bytes[m] == b'_' {
                            m += 1;
                        } else {
                            break;
                        }
                    }
                    if xz && m > j + 1 {
                        return true;
                    }
                }
            }
        }
        k += 1;
    }
    false
}

fn rule_x_valued(rule: &crate::schema::Rule) -> bool {
    match &rule.check.payload {
        CheckPayload::Guard { effects, .. } => effects.iter().any(|e| has_xz_bit_literal(&e.value)),
        CheckPayload::Sequence { becomes, .. } => has_xz_bit_literal(becomes),
        CheckPayload::Relation { rhs, .. } => has_xz_bit_literal(rhs),
        CheckPayload::Transition { to, .. } => has_xz_bit_literal(to),
        CheckPayload::Invariant { .. } => false,
    }
}

/// 規則主體是否輸出端口(下標/部分選擇取基名;拼接左值任一元素命中即可見)。
fn subject_is_output(subject: &str, outputs: &[String]) -> bool {
    subject
        .trim_matches(|c| c == '{' || c == '}')
        .split(',')
        .map(|part| part.split('[').next().unwrap_or("").trim())
        .any(|base| outputs.iter().any(|o| o == base))
}

/// span → 主檔行範圍(閉區間;鏡像 Ctx::evidence 的行規則:end 恰在行首
/// → 上一行)。
fn span_line_range(analysis: &Analysis, sp: &Span) -> (u32, u32) {
    let Ok((sl, _)) = analysis.line_col(FileId(0), sp.start.0) else {
        return (0, 0);
    };
    let mut el = sl;
    if sp.end.0 >= sp.start.0 {
        if let Ok((l, c)) = analysis.line_col(FileId(0), sp.end.0) {
            el = l;
            if c == 1 && el > sl {
                el -= 1;
            }
        }
    }
    (sl, el.max(sl))
}

/// 邊沿行程記錄(計劃面私有)。
struct EdgeProc {
    /// 整條 always 語句 span(行程歸屬行域)。
    span: Span,
    /// select_trigger(edges, reset_cand) → (edge, signal)。
    trigger: Option<(String, String)>,
    /// 敏感列表全部 pos/neg 邊 (edge, signal)(行程序)。
    edges: Vec<(String, String)>,
}

fn edge_str(k: EdgeKind) -> &'static str {
    match k {
        EdgeKind::Posedge => "posedge",
        EdgeKind::Negedge => "negedge",
        _ => "anyedge",
    }
}

/// 行程枚舉:全部沿行程(clocks 用=產規則者過濾;sens_edges 用=全集)。
fn collect_edge_procs(m: &ast::Module) -> Vec<EdgeProc> {
    let mut procs = Vec::new();
    walk_items(&m.items, false, &mut |item, _| {
        if let ModuleItem::Always { span, ctrl, body } = item {
            let Some(c) = ctrl else { return };
            let edges: Vec<&ast::EventEdge> = c.edges.iter().collect();
            let is_edge =
                |e: &ast::EventEdge| matches!(e.kind, EdgeKind::Posedge | EdgeKind::Negedge);
            if !edges.iter().any(|e| is_edge(e)) {
                return; // @*/@(a or b)=組合行程
            }
            let top: Vec<&Stmt> = match body {
                Stmt::Block { body, .. } => body.iter().collect(),
                s => vec![s],
            };
            let reset_cand: Option<&str> = match top.first() {
                Some(Stmt::If { cond, .. }) => reset_cond_signal(cond),
                _ => None,
            };
            let trigger = select_trigger(&edges, reset_cand)
                .map(|e| (edge_str(e.kind).to_string(), render(&e.expr)));
            let sens = edges
                .iter()
                .filter(|e| is_edge(e))
                .map(|e| (edge_str(e.kind).to_string(), render(&e.expr)))
                .collect();
            procs.push(EdgeProc {
                span: *span,
                trigger,
                edges: sens,
            });
        }
    });
    procs
}

fn port_width_of<'a>(ports: &'a [PortPlan], name: &str) -> Option<&'a PortPlan> {
    ports.iter().find(|p| p.name == name)
}

/// 純組合設計的 Σdata_input 位寬。
fn comb_input_bits(ports: &[PortPlan], data_inputs: &[String]) -> u32 {
    data_inputs
        .iter()
        .map(|n| port_width_of(ports, n).map(|p| p.width).unwrap_or(1))
        .sum()
}

/// 計劃主入口(RV1-11:內部以 DEFAULT_MAX_RULES 提取,少一個可漂移旋鈕;
/// truncated 非空=TbError 防計劃靜默殘缺)。
pub fn tb_plan(input: &SourceInput, module: Option<&str>) -> Result<TbPlan, TbError> {
    if matches!(input, SourceInput::Rtlil(_)) {
        return Err(TbError::tb1("RTLIL input is not supported by tb_plan (SV front only)"));
    }
    let SourceInput::Sv(sv_in) = input else {
        return Err(TbError::tb1("unsupported input"));
    };
    let analysis = analyze(&SourceInput::Sv(sv_in.clone()));
    let parse_errors = extract_parse_errors(&analysis);
    if !parse_errors.is_empty() {
        let joined = parse_errors.join("; ");
        // 前端診斷自帶 (milestone Mx) 者保留其 Mx(§19.3.2 ①)。
        if joined.contains("(milestone ") {
            return Err(TbError {
                reason: format!("parse errors: {joined}"),
            });
        }
        return Err(TbError::tb1(format!("parse errors: {joined}")));
    }
    let rb = crate::extract_rules(&SourceInput::Sv(sv_in.clone()), DEFAULT_MAX_RULES);
    if !rb.truncated.is_empty() {
        return Err(TbError::tb1(format!(
            "rulebook truncated for {} (raise budget or shrink source)",
            rb.truncated.join(",")
        )));
    }
    if rb.modules.is_empty() {
        return Err(TbError::tb1("no module in source"));
    }
    let mrs = match module {
        Some(name) => match rb.modules.iter().find(|m| m.module == name) {
            Some(m) => m,
            None => return Err(TbError::tb1(format!("module `{name}` not found"))),
        },
        None => {
            if rb.modules.len() > 1 {
                let names: Vec<&str> = rb.modules.iter().map(|m| m.module.as_str()).collect();
                return Err(TbError::tb1(format!(
                    "multi-module source, `module` required: {}",
                    names.join(", ")
                )));
            }
            &rb.modules[0]
        }
    };
    let contract = &mrs.contract;

    // ---- 端口面硬錯(④⑤⑥) ----
    if let Some(p) = contract.ports.iter().find(|p| p.parametric) {
        return Err(TbError::tb1(format!(
            "port `{}` has parametric width (no numeric width in contract)",
            p.name
        )));
    }
    for p in &contract.ports {
        if p.dir == "inout" {
            return Err(TbError::tb1(format!("port `{}` is inout", p.name)));
        }
        if p.width == 0 {
            return Err(TbError::tb1(format!("port `{}` has width 0", p.name)));
        }
    }
    let input_ports: Vec<&ContractPort> =
        contract.ports.iter().filter(|p| p.dir == "input").collect();
    let outputs: Vec<String> = contract
        .ports
        .iter()
        .filter(|p| p.dir == "output")
        .map(|p| p.name.clone())
        .collect();
    if outputs.is_empty() {
        return Err(TbError::tb1("no output port"));
    }

    // ---- 行程面:clocks(產規則的沿行程經 select_trigger)/sens_edges(全集) ----
    let Some(file_ast) = analysis.sv.clone() else {
        return Err(TbError::tb1("no SV AST (internal)"));
    };
    let Some(ast_mod) = file_ast
        .modules
        .iter()
        .find(|m| m.name == mrs.module)
    else {
        return Err(TbError::tb1(format!(
            "module `{}` not found in AST",
            mrs.module
        )));
    };
    let procs = collect_edge_procs(ast_mod);
    let mut sens_edges: Vec<(String, String)> = Vec::new();
    for p in &procs {
        for e in &p.edges {
            if !sens_edges.contains(e) {
                sens_edges.push(e.clone());
            }
        }
    }
    let mut clocks: Vec<ClockPlan> = Vec::new();
    for p in &procs {
        let (ps, pe) = span_line_range(&analysis, &p.span);
        let produces = mrs.rules.iter().any(|r| {
            r.evidence
                .first()
                .map(|e| e.line >= ps && e.line <= pe)
                .unwrap_or(false)
        });
        if !produces {
            continue;
        }
        if let Some((edge, sig)) = &p.trigger {
            if !clocks.iter().any(|c| &c.signal == sig) {
                clocks.push(ClockPlan {
                    edge: edge.clone(),
                    signal: sig.clone(),
                });
            }
        }
    }
    let clock_sigs: Vec<&str> = clocks.iter().map(|c| c.signal.as_str()).collect();
    if clock_sigs.len() > 1 {
        return Err(TbError::tb1(format!(
            "multiple clock signals: {}",
            clock_sigs.join(", ")
        )));
    }

    // ---- resets:只由 guard 規則導出(R1-11;禁啟發式猜測) ----
    let mut resets: Vec<ResetPlan> = Vec::new();
    for r in &mrs.rules {
        let CheckPayload::Guard { when, r#async, .. } = &r.check.payload else {
            continue;
        };
        let Some(w) = when else { continue };
        let (sig, active_low) = match w.strip_prefix('!') {
            Some(s) => (s.to_string(), true),
            None => (w.clone(), false),
        };
        if let Some(rp) = resets
            .iter_mut()
            .find(|rp| rp.signal == sig && rp.active_low == active_low && rp.r#async == *r#async)
        {
            rp.rule_ids.push(r.id.clone());
        } else {
            resets.push(ResetPlan {
                signal: sig,
                active_low,
                r#async: *r#async,
                rule_ids: vec![r.id.clone()],
            });
        }
    }

    // ---- 時鐘/復位角色硬錯(⑧⑨) ----
    for c in &clocks {
        if !input_ports.iter().any(|p| p.name == c.signal) {
            return Err(TbError::tb1(format!(
                "clock signal `{}` is not an input port",
                c.signal
            )));
        }
    }
    for r in &resets {
        if !input_ports.iter().any(|p| p.name == r.signal) {
            return Err(TbError::tb1(format!(
                "reset signal `{}` is not an input port",
                r.signal
            )));
        }
    }
    for c in &clocks {
        if resets.iter().any(|r| r.signal == c.signal) {
            return Err(TbError::tb1(format!(
                "signal `{}` is both clock and reset",
                c.signal
            )));
        }
    }

    let clock_sig = clocks.first().map(|c| c.signal.clone());
    let data_inputs: Vec<String> = input_ports
        .iter()
        .map(|p| p.name.clone())
        .filter(|n| Some(n.as_str()) != clock_sig.as_deref())
        .filter(|n| !resets.iter().any(|r| &r.signal == n))
        .collect();

    // ---- obligations/waivers ----
    let obligations: Vec<Obligation> = mrs
        .rules
        .iter()
        .map(|r| Obligation {
            rule_id: r.id.clone(),
            form: r.check.form.as_str().to_string(),
            category: r.category.as_str().to_string(),
            observe: if subject_is_output(&r.check.subject(), &outputs) {
                "port"
            } else {
                "indirect"
            },
        })
        .collect();
    let waivers: Vec<Waiver> = mrs
        .rules
        .iter()
        .filter(|r| rule_x_valued(r))
        .map(|r| Waiver {
            rule_id: r.id.clone(),
            reason: "x-valued",
        })
        .collect();

    // ---- 端口計劃面(先於場景;場景的 comb 位寬判定要用) ----
    let ports: Vec<PortPlan> = contract
        .ports
        .iter()
        .map(|p| PortPlan {
            name: p.name.clone(),
            dir: p.dir.clone(),
            width: p.width,
            signed: p.signed,
        })
        .collect();
    let comb_bits = comb_input_bits(&ports, &data_inputs);
    let scenes = build_scenes(&clocks, &resets, comb_bits);

    Ok(TbPlan {
        module: mrs.module.clone(),
        params: contract
            .params
            .iter()
            .map(|p| (p.name.clone(), p.value.clone(), p.local))
            .collect(),
        ports,
        clocks,
        resets,
        sens_edges,
        data_inputs,
        outputs,
        obligations,
        waivers,
        scenes,
    })
}

// (片段檔:build_scenes 全量替換)
/// 場景名=`[a-z0-9_]+`(協議 v1 文法):小寫化+非法字元改 `_`;重名補序號。
fn scene_name(prefix: &str, signal: &str, used: &[ScenePlan]) -> String {
    let body: String = signal
        .chars()
        .map(|c| {
            let l = c.to_ascii_lowercase();
            if l.is_ascii_lowercase() || l.is_ascii_digit() || l == '_' {
                l
            } else {
                '_'
            }
        })
        .collect();
    let base = format!("{prefix}_{body}");
    if !used.iter().any(|s| s.name == base) {
        return base;
    }
    let mut k = 2usize;
    loop {
        let cand = format!("{base}_{k}");
        if !used.iter().any(|s| s.name == cand) {
            return cand;
        }
        k += 1;
    }
}

fn build_scenes(clocks: &[ClockPlan], resets: &[ResetPlan], comb_bits: u32) -> Vec<ScenePlan> {
    let star = |name: &str, kind: &'static str| ScenePlan {
        name: name.to_string(),
        kind,
        covers: vec![Cover::Star],
    };
    let mut scenes = Vec::new();
    if clocks.is_empty() {
        // 純組合:窮舉(Σ輸入位 ≤12)否則 LFSR 256 向量;+S-SMALL。
        if comb_bits > 0 && comb_bits <= MAX_EXHAUSTIVE_BITS {
            scenes.push(star("exh", "exh"));
        } else {
            scenes.push(star("rand", "rand"));
        }
        scenes.push(star("small", "small"));
        return scenes;
    }
    for r in resets {
        let (prefix, kind) = if r.r#async { ("arst", "arst") } else { ("rst", "rst") };
        let name = scene_name(prefix, &r.signal, &scenes);
        scenes.push(ScenePlan {
            name,
            kind,
            covers: r.rule_ids.iter().map(|id| Cover::Rule(id.clone())).collect(),
        });
    }
    if resets.is_empty() {
        // flush=非定向場景(COVERS 文法不收空表,標 `*`)。
        scenes.push(star("flush", "flush"));
    }
    scenes.push(star("full", "full"));
    scenes.push(star("small", "small"));
    scenes.push(star("hold", "hold"));
    scenes
}

// ---- 模板生成(不可失敗;全部失敗模式在 tb_plan 期攔下) ----
//
// TB 內部名一律 `tb__` 前綴(防與 DUT 端口撞名=Q1 編譯紅);輸入驅動寬度精確
// (切片/拼接,不走隱式截斷——Q7:tb.v 位置的 verilator 警告必為 0)。

const CLOCK_HALF_PERIOD: u32 = 5;

/// 寬度 w 的全 0/全 1 字面量。
fn level_lit(width: u32, high: bool) -> String {
    let bit = if high { "1'b1" } else { "1'b0" };
    if width <= 1 {
        bit.to_string()
    } else {
        format!("{{{width}{{{bit}}}}}")
    }
}

fn port_decl(kind: &str, p: &PortPlan) -> String {
    let signed = if p.signed { " signed" } else { "" };
    if p.width == 1 {
        format!("  {}{} {};", kind, signed, p.name)
    } else {
        format!("  {}{} [{}:0] {};", kind, signed, p.width - 1, p.name)
    }
}

/// `tb__rnd` 上寬度 e(1..=32)的切片,起點鉗到不越界。
fn rnd_slice(e: u32, shift: u32) -> String {
    let s = shift.min(32 - e);
    if e == 1 {
        format!("tb__rnd[{s}]")
    } else {
        format!("tb__rnd[{}:{}]", s + e - 1, s)
    }
}

/// 輸入驅動右值(寬度恰==width;small=多位遮罩到低 3 位,高位補 0)。
fn drive_rhs(width: u32, shift: u32, small: bool) -> String {
    let e = if small { width.min(3) } else { width };
    if e <= 32 {
        let slice = rnd_slice(e, shift);
        if e == width {
            slice
        } else {
            format!("{{{{{}{{1'b0}}}}, {}}}", width - e, slice)
        }
    } else {
        // 寬於 32 位:以 tb__rnd 重複拼接到恰好 e 位。
        let (k, r) = (e / 32, e % 32);
        if r == 0 {
            format!("{{{k}{{tb__rnd}}}}")
        } else {
            format!("{{{}, {{{k}{{tb__rnd}}}}}}", rnd_slice(r, 0))
        }
    }
}

fn drive_inputs(plan: &TbPlan, small: bool, indent: &str) -> String {
    let mut s = format!("{indent}tb__rnd = tb__xs32(tb__rnd);\n");
    for (j, name) in plan.data_inputs.iter().enumerate() {
        let w = port_width_of(&plan.ports, name).map(|p| p.width).unwrap_or(1);
        let shift = (7 * j as u32 + 3) % 24;
        s.push_str(&format!("{indent}{name} = {};\n", drive_rhs(w, shift, small)));
    }
    s
}

/// 驅動沿等待:時鐘有效沿的反極性;DUT 兩極性都採樣時補 `#1;`(同行關窗,
/// TB-R1x)。極性感知=負沿時鐘 DUT 不與採樣沿競爭(§19.3.3)。
fn wait_drive(plan: &TbPlan) -> String {
    let Some(c) = plan.clocks.first() else {
        return String::new();
    };
    let opp = if c.edge == "posedge" { "negedge" } else { "posedge" };
    let dual = plan
        .sens_edges
        .iter()
        .any(|(e, s)| e == opp && s == &c.signal);
    if dual {
        format!("@({opp} {}); #1;", c.signal)
    } else {
        format!("@({opp} {});", c.signal)
    }
}

fn print_outputs(outputs: &[String], indent: &str) -> String {
    let mut s = String::new();
    for o in outputs {
        s.push_str(&format!("{indent}$display(\"  {o}=%h\", {o});\n"));
    }
    s
}

/// 觀測拍(`indent` 層):等驅動沿→打印輸出→驅動隨機(打印在前=捕捉上一
/// 有效沿後的穩定態)。
fn observe_body(plan: &TbPlan, small: bool, indent: &str) -> String {
    let mut s = format!("{indent}{}\n", wait_drive(plan));
    s.push_str(&print_outputs(&plan.outputs, indent));
    s.push_str(&drive_inputs(plan, small, indent));
    s
}

/// 時序設計總拍數上界(看門狗用;flush 取本次 opts,非默認常量)。
fn clocked_cycles(plan: &TbPlan, opts: &TbOpts) -> u32 {
    let mut n = 0u32;
    for sc in &plan.scenes {
        n = n.saturating_add(match sc.kind {
            "rst" | "arst" => 8,
            "flush" => opts.flush,
            "full" | "small" | "hold" => opts.cycles.saturating_add(1),
            _ => 0,
        });
    }
    n
}

fn comb_vectors(plan: &TbPlan) -> u32 {
    let bits = comb_input_bits(&plan.ports, &plan.data_inputs);
    if bits > 0 && bits <= MAX_EXHAUSTIVE_BITS {
        1u32 << bits
    } else {
        COMB_RANDOM_VECTORS
    }
}

/// 模板 v1(§19.3.3 協議;不可失敗)。場景內容由 plan.scenes 驅動;未知
/// kind 跳過(永不 panic 姿態;build_scenes 只產已知 kind)。
pub fn tb_template_v(plan: &TbPlan, opts: &TbOpts) -> String {
    let covers_text = |covers: &[Cover]| -> String {
        covers
            .iter()
            .map(|c| match c {
                Cover::Rule(id) => id.clone(),
                Cover::Star => "*".to_string(),
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    let mut t = String::new();
    t.push_str(&format!(
        "// tb_{} — template tb_plan-v1(機械生成;T 腿基線;期望由原版錄製,禁手寫/禁加自檢)\n",
        plan.module
    ));
    t.push_str(&format!(
        "// TB-LEG: template tb_plan-v1 seed={} cycles={} flush={}\n",
        opts.seed, opts.cycles, opts.flush
    ));
    t.push_str(&format!("// SCENES: {}\n", plan.scenes.len()));
    for (i, sc) in plan.scenes.iter().enumerate() {
        t.push_str(&format!("// COVERS: {}: {}\n", i + 1, covers_text(&sc.covers)));
    }
    for w in &plan.waivers {
        t.push_str(&format!("// WAIVE: {} {}\n", w.rule_id, w.reason));
    }
    let clk = plan.clocks.first().map(|c| c.signal.clone());
    // 統一 timescale:DUT 帶 `timescale 而 TB 不帶=tb.v 位置 TIMESCALEMOD(Q7)。
    t.push_str("\n`timescale 1ns/1ps\n");
    t.push_str(&format!("module tb_{};\n\n", plan.module));
    for p in &plan.ports {
        let kind = if p.dir == "input" { "reg" } else { "wire" };
        t.push_str(&port_decl(kind, p));
        t.push('\n');
    }
    t.push_str("\n  reg [31:0] tb__rnd;\n  integer tb__i;\n\n");
    // 實例化(具名連接覆蓋全部 contract 端口;§19.3.3 靜態約束)。
    t.push_str(&format!("  {} tb__dut (\n", plan.module));
    for (k, p) in plan.ports.iter().enumerate() {
        t.push_str(&format!("    .{}({})", p.name, p.name));
        t.push_str(if k + 1 == plan.ports.len() { "\n" } else { ",\n" });
    }
    t.push_str("  );\n\n");
    // xorshift32(禁 $random:跨模擬器序列不保證一致)。
    t.push_str("  function [31:0] tb__xs32;\n");
    t.push_str("    input [31:0] x;\n");
    t.push_str("    reg [31:0] y;\n");
    t.push_str("    begin\n");
    t.push_str("      y = x;\n");
    t.push_str("      y = y ^ (y << 13);\n");
    t.push_str("      y = y ^ (y >> 17);\n");
    t.push_str("      y = y ^ (y << 5);\n");
    t.push_str("      tb__xs32 = y;\n");
    t.push_str("    end\n");
    t.push_str("  endfunction\n\n");
    // 看門狗(W 兜底;場景內等待一律 repeat/for 有界)。
    let watchdog = if clk.is_none() {
        comb_vectors(plan)
            .saturating_add(COMB_RANDOM_VECTORS)
            .saturating_mul(2)
            .saturating_add(1000)
    } else {
        clocked_cycles(plan, opts)
            .saturating_add(32)
            .saturating_mul(2 * CLOCK_HALF_PERIOD)
            .saturating_add(1000)
    };
    t.push_str("  initial begin\n");
    t.push_str(&format!("    #{watchdog};\n"));
    t.push_str("    $display(\"WATCHDOG\");\n");
    t.push_str("    $finish;\n");
    t.push_str("  end\n\n");
    if let Some(c) = &clk {
        let w = port_width_of(&plan.ports, c).map(|p| p.width).unwrap_or(1);
        t.push_str(&format!("  initial {c} = {};\n", level_lit(w, false)));
        t.push_str(&format!("  always #{CLOCK_HALF_PERIOD} {c} = ~{c};\n\n"));
    }
    // 場景序列(單 initial;順序=plan.scenes)。#1 後才初始化輸入:DUT 組合行程
    // 全部掛上事件等待之後再給定值(X→定值 的跳變必被看到;t=0 同刻=IEEE 非確定
    // 序,AGENTS §2)。復位一律先置無效電平,數據輸入置 0(Q4 X-clean)。
    t.push_str("  initial begin : tb__scenes\n");
    t.push_str(&format!("    tb__rnd = 32'd{};\n", opts.seed));
    t.push_str("    #1;\n");
    for r in &plan.resets {
        let w = port_width_of(&plan.ports, &r.signal).map(|p| p.width).unwrap_or(1);
        t.push_str(&format!("    {} = {};\n", r.signal, level_lit(w, r.active_low)));
    }
    for name in &plan.data_inputs {
        let w = port_width_of(&plan.ports, name).map(|p| p.width).unwrap_or(1);
        t.push_str(&format!("    {name} = {};\n", level_lit(w, false)));
    }
    let mut reset_ix = 0usize;
    for (i, sc) in plan.scenes.iter().enumerate() {
        t.push_str(&format!("    // SCENE{} {}\n", i + 1, sc.name));
        t.push_str(&format!("    $display(\"SCENE{} {}\");\n", i + 1, sc.name));
        match sc.kind {
            "rst" | "arst" => {
                // 復位場景與 plan.resets 同序一一對應(build_scenes)。
                let Some(rp) = plan.resets.get(reset_ix) else {
                    continue;
                };
                reset_ix += 1;
                let w = port_width_of(&plan.ports, &rp.signal)
                    .map(|p| p.width)
                    .unwrap_or(1);
                let act = level_lit(w, !rp.active_low);
                let rel = level_lit(w, rp.active_low);
                let sig = &rp.signal;
                let wait = wait_drive(plan);
                if sc.kind == "rst" {
                    // S-RST:驅動沿處置有效 2 拍→釋放→觀測 4 拍。
                    t.push_str(&format!("    {wait}\n"));
                    t.push_str(&format!("    {sig} = {act};\n"));
                    t.push_str(&format!("    {wait}\n"));
                    t.push_str(&format!("    {wait}\n"));
                    t.push_str(&format!("    {sig} = {rel};\n"));
                } else {
                    // S-ARST:兩沿之間驅動沿後 #2 拉起、#1 後立即觀測、再釋放
                    // (專打「只在有效沿後採樣」盲區,§19.0-5 counter8 async)。
                    t.push_str(&format!("    {wait}\n"));
                    t.push_str("    #2;\n");
                    t.push_str(&format!("    {sig} = {act};\n"));
                    t.push_str("    #1;\n");
                    t.push_str(&print_outputs(&plan.outputs, "    "));
                    t.push_str(&format!("    {wait}\n"));
                    t.push_str(&format!("    {sig} = {rel};\n"));
                }
                t.push_str("    repeat (4) begin\n");
                t.push_str(&observe_body(plan, false, "      "));
                t.push_str("    end\n");
            }
            "flush" => {
                // 無復位時序設計:首次觀測前驅動 flush 拍(X-clean 由閘 Q4 兜底)。
                t.push_str(&format!("    repeat ({}) begin\n", opts.flush));
                t.push_str(&format!("      {}\n", wait_drive(plan)));
                t.push_str(&drive_inputs(plan, false, "      "));
                t.push_str("    end\n");
            }
            "full" | "small" if clk.is_some() => {
                t.push_str(&format!("    repeat ({}) begin\n", opts.cycles.saturating_add(1)));
                t.push_str(&observe_body(plan, sc.kind == "small", "      "));
                t.push_str("    end\n");
            }
            "small" | "rand" => {
                // 純組合 small/rand:LFSR 向量+#1 穩定後觀測(small 多位遮罩)。
                t.push_str(&format!("    repeat ({COMB_RANDOM_VECTORS}) begin\n"));
                t.push_str(&drive_inputs(plan, sc.kind == "small", "      "));
                t.push_str("      #1;\n");
                t.push_str(&print_outputs(&plan.outputs, "      "));
                t.push_str("    end\n");
            }
            "hold" => {
                // S-HOLD:每 8 拍換值(計數/握手類行為走完)。
                t.push_str(&format!(
                    "    for (tb__i = 0; tb__i < {}; tb__i = tb__i + 1) begin\n",
                    opts.cycles
                ));
                t.push_str("      if (tb__i % 8 == 0) begin\n");
                t.push_str(&drive_inputs(plan, false, "        "));
                t.push_str("      end\n");
                t.push_str(&format!("      {}\n", wait_drive(plan)));
                t.push_str(&print_outputs(&plan.outputs, "      "));
                t.push_str("    end\n");
            }
            "exh" => {
                // S-EXH:Σ輸入位 ≤12 窮舉;#1 穩定後觀測。
                let bits = comb_input_bits(&plan.ports, &plan.data_inputs);
                t.push_str(&format!(
                    "    for (tb__i = 0; tb__i < {}; tb__i = tb__i + 1) begin\n",
                    1u32 << bits.min(MAX_EXHAUSTIVE_BITS)
                ));
                let cat = plan.data_inputs.join(", ");
                let sel = if bits == 1 {
                    "tb__i[0]".to_string()
                } else {
                    format!("tb__i[{}:0]", bits - 1)
                };
                t.push_str(&format!("      {{{cat}}} = {sel};\n"));
                t.push_str("      #1;\n");
                t.push_str(&print_outputs(&plan.outputs, "      "));
                t.push_str("    end\n");
            }
            _ => {}
        }
    }
    t.push_str("    $display(\"DONE\");\n");
    t.push_str("    $finish;\n");
    t.push_str("  end\n");
    t.push_str("endmodule\n");
    t
}

/// TbOpts(seed/cycles/flush;0=核心默認,wasm 面標量入參契約)。
#[derive(Clone, Debug)]
pub struct TbOpts {
    pub seed: u32,
    pub cycles: u32,
    pub flush: u32,
}

impl Default for TbOpts {
    fn default() -> Self {
        TbOpts {
            seed: DEFAULT_SEED,
            cycles: DEFAULT_CYCLES,
            flush: DEFAULT_FLUSH,
        }
    }
}

impl TbOpts {
    /// 0=核心默認(與 rtl_extract_rules_sv 的 max_rules==0 同約定)。
    pub fn resolve(seed: u32, cycles: u32, flush: u32) -> TbOpts {
        let d = TbOpts::default();
        TbOpts {
            seed: if seed == 0 { d.seed } else { seed },
            cycles: if cycles == 0 { d.cycles } else { cycles },
            flush: if flush == 0 { d.flush } else { flush },
        }
    }
}

fn jstr(v: impl Into<String>) -> Json {
    Json::Str(v.into())
}

fn covers_json(covers: &[Cover]) -> Json {
    Json::Arr(
        covers
            .iter()
            .map(|c| match c {
                Cover::Rule(id) => jstr(id.clone()),
                Cover::Star => jstr("*"),
            })
            .collect(),
    )
}

fn tb_plan_obj(plan: &TbPlan) -> Json {
    Json::Obj(vec![
        ("module".to_string(), jstr(plan.module.clone())),
        (
            "params".to_string(),
            Json::Arr(
                plan.params
                    .iter()
                    .map(|(n, v, l)| {
                        Json::Obj(vec![
                            ("name".to_string(), jstr(n.clone())),
                            ("value".to_string(), jstr(v.clone())),
                            ("local".to_string(), Json::Bool(*l)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "ports".to_string(),
            Json::Arr(
                plan.ports
                    .iter()
                    .map(|p| {
                        Json::Obj(vec![
                            ("name".to_string(), jstr(p.name.clone())),
                            ("dir".to_string(), jstr(p.dir.clone())),
                            ("width".to_string(), Json::Int(i64::from(p.width))),
                            ("signed".to_string(), Json::Bool(p.signed)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "clocks".to_string(),
            Json::Arr(
                plan.clocks
                    .iter()
                    .map(|c| {
                        Json::Obj(vec![
                            ("edge".to_string(), jstr(c.edge.clone())),
                            ("signal".to_string(), jstr(c.signal.clone())),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "resets".to_string(),
            Json::Arr(
                plan.resets
                    .iter()
                    .map(|r| {
                        Json::Obj(vec![
                            ("signal".to_string(), jstr(r.signal.clone())),
                            ("active_low".to_string(), Json::Bool(r.active_low)),
                            ("async".to_string(), Json::Bool(r.r#async)),
                            (
                                "rule_ids".to_string(),
                                Json::Arr(r.rule_ids.iter().map(|id| jstr(id.clone())).collect()),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "sens_edges".to_string(),
            Json::Arr(
                plan.sens_edges
                    .iter()
                    .map(|(e, s)| Json::Arr(vec![jstr(e.clone()), jstr(s.clone())]))
                    .collect(),
            ),
        ),
        (
            "data_inputs".to_string(),
            Json::Arr(plan.data_inputs.iter().map(|n| jstr(n.clone())).collect()),
        ),
        (
            "outputs".to_string(),
            Json::Arr(plan.outputs.iter().map(|n| jstr(n.clone())).collect()),
        ),
        (
            "obligations".to_string(),
            Json::Arr(
                plan.obligations
                    .iter()
                    .map(|o| {
                        Json::Obj(vec![
                            ("rule_id".to_string(), jstr(o.rule_id.clone())),
                            ("form".to_string(), jstr(o.form.clone())),
                            ("category".to_string(), jstr(o.category.clone())),
                            ("observe".to_string(), jstr(o.observe)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "waivers".to_string(),
            Json::Arr(
                plan.waivers
                    .iter()
                    .map(|w| {
                        Json::Obj(vec![
                            ("rule_id".to_string(), jstr(w.rule_id.clone())),
                            ("reason".to_string(), jstr(w.reason)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "scenes".to_string(),
            Json::Arr(
                plan.scenes
                    .iter()
                    .map(|sc| {
                        Json::Obj(vec![
                            ("name".to_string(), jstr(sc.name.clone())),
                            ("kind".to_string(), jstr(sc.kind)),
                            ("covers".to_string(), covers_json(&sc.covers)),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

/// 計劃 JSON(裸 plan 物件;`{"ok":…}` 全幀由 [`tb_plan_frame`] 面承)。
pub fn tb_plan_json(plan: &TbPlan) -> String {
    tb_plan_obj(plan).render()
}

/// wasm/emit 對拍基準的全幀:`{"ok":true,"plan":{…},"template_v":"…"}`
/// 或 `{"ok":false,"error":"…"}`(錯誤走資料,不 trap)。
pub fn tb_plan_frame(
    input: &SourceInput,
    module: Option<&str>,
    seed: u32,
    cycles: u32,
    flush: u32,
) -> String {
    match tb_plan(input, module) {
        Ok(plan) => {
            let opts = TbOpts::resolve(seed, cycles, flush);
            Json::Obj(vec![
                ("ok".to_string(), Json::Bool(true)),
                ("plan".to_string(), tb_plan_obj(&plan)),
                ("template_v".to_string(), jstr(tb_template_v(&plan, &opts))),
            ])
            .render()
        }
        Err(e) => Json::Obj(vec![
            ("ok".to_string(), Json::Bool(false)),
            ("error".to_string(), jstr(e.to_string())),
        ])
        .render(),
    }
}
