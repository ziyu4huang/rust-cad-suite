//! spec — 高級 spec 檔(markdown 節頭逐字格式)解析/lint/JSON(D 弧 D-S1)。
//!
//! 文法 SSOT=docs/RTL2SPEC-D-PLAN §3(行級文法 R1-4 五條+check 行四收形,
//! sequence 無 anchor 欄;md.rs 規則書發射面/reimpl trim writer 與本解析
//! 不可互推、變動不溯及)。語義姿態同 crate 慣例:永不 panic、解析對任何
//! `&str` 全容錯(劣化面全走 [`spec_lint`] 七查;結構行欄位缺=None);
//! `Result` 形([`SpecError`])=計畫 §4 D-S1 簽名,供未來文法立檔
//! (invariant 形待語料)與 CLI 錯誤面,當前無產出點。Vec 一律按檔案行
//! 程序(禁 HashSet 迭代面進輸出,§16.1.5 契約③;⑥ 缺集=BTreeSet 字典
//! 序);JSON 手寫(鍵序=寫出序 byte-exact 契約,單測釘死;復用
//! analyzer_core Json,只寫不讀)。
//!
//! 七查(§3 機械檢查):
//! ①spec-format 在且=1;②module 非空;③port ≥1 且 dir/type 詞表;
//! ④behavior id 唯一(B\d+);⑤form∈四收形(guard/transition/sequence/
//! relation)且欄位非空、每 behavior 條目結構行恰 1;⑥decl_lint=結構行
//! 值側標識符 ⊆ ports∪params∪states∪derived(只掃值側:四收形 payload
//! 除欄位鍵外皆值側,排除表 [`EXCLUDE`]);⑦scenarios ≥1 且 SCENE<n> 唯一。

use std::collections::{BTreeSet, HashSet};

use analyzer_core::proto::json::Json;

use crate::lint::collect_idents;

/// spec 解析錯誤(沿 crate 姿態:永不 panic、錯誤進結果)。當前解析面對
/// 任何 &str 全容錯(Err 無產出點;劣化面全走 [`spec_lint`] 七查)。
#[derive(Clone, Debug)]
pub struct SpecError {
    pub message: String,
}

impl std::fmt::Display for SpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error: {}", self.message)
    }
}

/// `- param: <NAME> = <expr>`。
#[derive(Clone, Debug, PartialEq)]
pub struct SpecParam {
    pub name: String,
    pub expr: String,
}

/// `- port: <name> <dir> <kind> <width>`(width=scalar|[N:0]|[PARAM-1:0])。
#[derive(Clone, Debug, PartialEq)]
pub struct SpecPort {
    pub name: String,
    pub dir: String,
    pub kind: String,
    pub width: String,
}

/// `- state: <name> <width> <一句角色>`。
#[derive(Clone, Debug, PartialEq)]
pub struct SpecState {
    pub name: String,
    pub width: String,
    pub role: String,
}

/// `- derived: <name> <width> <一句角色>`(組合中間量聲明通道)。
#[derive(Clone, Debug, PartialEq)]
pub struct SpecDerived {
    pub name: String,
    pub width: String,
    pub role: String,
}

/// guard 的 `trigger=<sig> <edge>`。
#[derive(Clone, Debug, PartialEq)]
pub struct SpecTrigger {
    pub signal: String,
    pub edge: String,
}

/// guard effects 單項 `<sig>←<expr>`。
#[derive(Clone, Debug, PartialEq)]
pub struct SpecEffect {
    pub lhs: String,
    pub rhs: String,
}

/// 結構行解析結果(form 專屬欄位;None=該欄缺/劣化=⑤ finding 面;
/// Other=form ∉ 四收形,invariant 等 D 弧未立文法)。
#[derive(Clone, Debug, PartialEq)]
pub enum SpecCheck {
    Guard {
        trigger: Option<SpecTrigger>,
        when: Option<String>,
        /// 原文值(文法=true|false;非法值經 ⑥ 值側掃描成缺宣告 finding)。
        async_raw: Option<String>,
        effects: Vec<SpecEffect>,
    },
    Transition {
        reg: Option<String>,
        from: Option<String>,
        to: Option<String>,
        when: Option<String>,
    },
    Sequence {
        latency: Option<u32>,
        signal: Option<String>,
        becomes: Option<String>,
        when: Option<String>,
    },
    Relation {
        lhs: Option<String>,
        rhs: Option<String>,
        when: Option<String>,
    },
    Other {
        form: String,
    },
}

/// `## behavior` 條目(check_lines[0]=canonical 結構行;len≠1=⑤ finding)。
#[derive(Clone, Debug, PartialEq)]
pub struct SpecBehavior {
    pub id: String,
    pub category: Option<String>,
    pub statement: String,
    pub check_lines: Vec<String>,
    pub check: Option<SpecCheck>,
}

/// `## scenarios` 條目(0 結構行;縮進續行併入 statement)。
#[derive(Clone, Debug, PartialEq)]
pub struct SpecScenario {
    pub id: String,
    pub name: String,
    pub statement: String,
}

/// spec 檔整檔(`# spec: <design>` 標題入 name;§3 格式塊八欄全收)。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpecDoc {
    pub name: Option<String>,
    pub spec_format: Option<String>,
    pub module: Option<String>,
    pub params: Vec<SpecParam>,
    pub ports: Vec<SpecPort>,
    pub states: Vec<SpecState>,
    pub derived: Vec<SpecDerived>,
    pub assumes: Vec<String>,
    pub behavior: Vec<SpecBehavior>,
    pub scenarios: Vec<SpecScenario>,
}

/// ⑥ 值側掃描排除表(§3 R1-1;lint.rs KEYWORDS 無 true/false,故顯式列;
/// `-` 佔位與數值字面量在 collect_idents 詞法面天然不收/由 `'` 前綴跳過)。
const EXCLUDE: &[&str] = &[
    // 欄位鍵
    "trigger",
    "when",
    "async",
    "effects",
    // 結構行關鍵詞
    "check",
    // 五形名
    "guard",
    "transition",
    "sequence",
    "relation",
    "invariant",
    // edge 詞
    "posedge",
    "negedge",
    // async 值
    "true",
    "false",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Section {
    Preamble,
    Behavior,
    Scenarios,
}

enum OpenEntry {
    Behavior(SpecBehavior),
    Scenario(SpecScenario),
}

fn opt_non_empty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// `… when <expr>` 以最後一個 " when " 切(when=文法保留詞,表達式內不可再現)。
fn split_when(s: &str) -> (&str, Option<&str>) {
    match s.rsplit_once(" when ") {
        Some((a, b)) => (a.trim(), Some(b.trim())),
        None => (s.trim(), None),
    }
}

fn flush_entry(open: &mut Option<OpenEntry>, doc: &mut SpecDoc) {
    if let Some(e) = open.take() {
        match e {
            OpenEntry::Behavior(b) => doc.behavior.push(b),
            OpenEntry::Scenario(s) => doc.scenarios.push(s),
        }
    }
}

/// 前言節指令(七個已知鍵;未知鍵=零語義跳過,擴充鍵走 ADR-0014 修訂)。
fn preamble_directive(payload: &str, doc: &mut SpecDoc) {
    if let Some(v) = payload.strip_prefix("spec-format:") {
        doc.spec_format = opt_non_empty(v.trim());
    } else if let Some(v) = payload.strip_prefix("module:") {
        doc.module = opt_non_empty(v.trim());
    } else if let Some(v) = payload.strip_prefix("param:") {
        let v = v.trim();
        let (name, expr) = match v.split_once('=') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (v, ""),
        };
        doc.params.push(SpecParam {
            name: name.to_string(),
            expr: expr.to_string(),
        });
    } else if let Some(v) = payload.strip_prefix("port:") {
        let toks: Vec<&str> = v.split_whitespace().collect();
        let tok = |i: usize| toks.get(i).copied().unwrap_or("").to_string();
        doc.ports.push(SpecPort {
            name: tok(0),
            dir: tok(1),
            kind: tok(2),
            width: tok(3),
        });
    } else if let Some(v) = payload.strip_prefix("state:") {
        let (name, rest) = first_rest(v.trim());
        let (width, role) = first_rest(rest);
        doc.states.push(SpecState {
            name: name.to_string(),
            width: width.to_string(),
            role: role.to_string(),
        });
    } else if let Some(v) = payload.strip_prefix("derived:") {
        let (name, rest) = first_rest(v.trim());
        let (width, role) = first_rest(rest);
        doc.derived.push(SpecDerived {
            name: name.to_string(),
            width: width.to_string(),
            role: role.to_string(),
        });
    } else if let Some(v) = payload.strip_prefix("assumes:") {
        if let Some(a) = opt_non_empty(v.trim()) {
            doc.assumes.push(a);
        }
    }
}

/// 首個空白前 token 與其餘(trim)。
fn first_rest(s: &str) -> (&str, &str) {
    match s.split_once(char::is_whitespace) {
        Some((a, b)) => (a, b.trim()),
        None => (s, ""),
    }
}

fn open_behavior(payload: &str) -> SpecBehavior {
    let (id, rest) = first_rest(payload);
    let (category, statement) = if let Some(r) = rest.strip_prefix('[') {
        match r.split_once(']') {
            Some((cat, st)) => (opt_non_empty(cat.trim()), st.trim().to_string()),
            None => (None, rest.to_string()),
        }
    } else {
        (None, rest.to_string())
    };
    SpecBehavior {
        id: id.to_string(),
        category,
        statement,
        check_lines: Vec::new(),
        check: None,
    }
}

fn open_scenario(payload: &str) -> SpecScenario {
    let (id, rest) = first_rest(payload);
    let (name, statement) = match rest.split_once(':') {
        Some((a, b)) => (a.trim().to_string(), b.trim().to_string()),
        None => (String::new(), rest.to_string()),
    };
    SpecScenario {
        id: id.to_string(),
        name,
        statement,
    }
}

/// 結構行 `check(<form>): <payload>` → form 專屬欄位(全容錯;括號缺=
/// Other{form:""})。
fn parse_check_line(line: &str) -> SpecCheck {
    let (form, payload) = match (line.find('('), line.find(')')) {
        (Some(o), Some(c)) if c > o => {
            let form = line[o + 1..c].trim().to_string();
            let rest = &line[c + 1..];
            let payload = rest.strip_prefix(':').unwrap_or(rest).trim().to_string();
            (form, payload)
        }
        _ => (String::new(), String::new()),
    };
    match form.as_str() {
        "guard" => parse_guard(&payload),
        "transition" => parse_transition(&payload),
        "sequence" => parse_sequence(&payload),
        "relation" => parse_relation(&payload),
        _ => SpecCheck::Other { form },
    }
}

fn parse_guard(payload: &str) -> SpecCheck {
    let mut trigger = None;
    let mut when = None;
    let mut async_raw = None;
    let mut effects = Vec::new();
    for field in payload.split(';') {
        let f = field.trim();
        let Some(eq) = f.find('=') else {
            continue;
        };
        let key = f[..eq].trim();
        let val = f[eq + 1..].trim();
        if val.is_empty() {
            continue;
        }
        match key {
            "trigger" => {
                let mut it = val.split_whitespace();
                let sig = it.next().unwrap_or("");
                let edge = it.next().unwrap_or("");
                trigger = Some(SpecTrigger {
                    signal: sig.to_string(),
                    edge: edge.to_string(),
                });
            }
            "when" => when = Some(val.to_string()),
            "async" => async_raw = Some(val.to_string()),
            "effects" => {
                // 多項以 `、` 並列(`,` 為 Verilog 拼接/實參字符,不可作分隔)。
                for item in val.split('、') {
                    let it = item.trim();
                    if let Some((lhs, rhs)) = it.split_once('←') {
                        effects.push(SpecEffect {
                            lhs: lhs.trim().to_string(),
                            rhs: rhs.trim().to_string(),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    SpecCheck::Guard {
        trigger,
        when,
        async_raw,
        effects,
    }
}

fn parse_transition(payload: &str) -> SpecCheck {
    let (reg, rest) = match payload.split_once(':') {
        Some((a, b)) => (opt_non_empty(a.trim()), b.trim()),
        None => (None, payload.trim()),
    };
    let (ft, when) = split_when(rest);
    let (from, to) = match ft.split_once('→') {
        Some((a, b)) => (opt_non_empty(a.trim()), opt_non_empty(b.trim())),
        None => (None, None),
    };
    SpecCheck::Transition {
        reg,
        from,
        to,
        when: when.map(str::to_string),
    }
}

fn parse_sequence(payload: &str) -> SpecCheck {
    let mut latency = None;
    let mut rest = payload.trim();
    // `+<N>拍 → ` 前綴(N=十進制;形不全=latency None,⑤ finding 面)。
    let b = rest.as_bytes();
    if b.first() == Some(&b'+') {
        let mut i = 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i > 1 {
            let after = rest[i..].trim_start();
            if let Some(a2) = after.strip_prefix('拍') {
                let a3 = a2.trim_start();
                if let Some(a4) = a3.strip_prefix('→') {
                    latency = rest[1..i].parse::<u32>().ok();
                    rest = a4.trim();
                }
            }
        }
    }
    let (se, when) = split_when(rest);
    let (signal, becomes) = match se.find('=') {
        Some(eq) => (opt_non_empty(se[..eq].trim()), opt_non_empty(se[eq + 1..].trim())),
        None => (None, None),
    };
    SpecCheck::Sequence {
        latency,
        signal,
        becomes,
        when: when.map(str::to_string),
    }
}

fn parse_relation(payload: &str) -> SpecCheck {
    let (lr, when) = split_when(payload.trim());
    let (lhs, rhs) = match lr.split_once('≡') {
        Some((a, b)) => (opt_non_empty(a.trim()), opt_non_empty(b.trim())),
        None => (None, None),
    };
    SpecCheck::Relation {
        lhs,
        rhs,
        when: when.map(str::to_string),
    }
}

/// spec 檔解析(行級文法 R1-4 五條;全容錯,永不 panic、永不 Err)。
pub fn parse_spec_md(src: &str) -> Result<SpecDoc, SpecError> {
    let mut doc = SpecDoc::default();
    let mut section = Section::Preamble;
    let mut open: Option<OpenEntry> = None;
    for raw in src.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            // 空行=條目界(R1-4 ⑤)。
            flush_entry(&mut open, &mut doc);
            continue;
        }
        if trimmed.starts_with('#') {
            if trimmed.starts_with("###") {
                // 子節頭=人讀分組,跳過零語義(R1-4 ②;不關條目)。
                continue;
            }
            flush_entry(&mut open, &mut doc);
            let content = trimmed.trim_start_matches('#').trim();
            match content {
                "behavior" => section = Section::Behavior,
                "scenarios" => section = Section::Scenarios,
                _ => {
                    section = Section::Preamble;
                    if let Some(name) = content.strip_prefix("spec:") {
                        doc.name = opt_non_empty(name.trim());
                    }
                }
            }
            continue;
        }
        let indented = raw.starts_with(' ') || raw.starts_with('\t');
        if !indented && trimmed.starts_with("- ") {
            // `- ` 起新條目(R1-4 ④;行首列)。
            flush_entry(&mut open, &mut doc);
            let payload = trimmed[2..].trim();
            match section {
                Section::Preamble => preamble_directive(payload, &mut doc),
                Section::Behavior => open = Some(OpenEntry::Behavior(open_behavior(payload))),
                Section::Scenarios => open = Some(OpenEntry::Scenario(open_scenario(payload))),
            }
            continue;
        }
        if indented {
            // 縮進連續行屬開放條目;無開放條目=散文跳過。
            match open.as_mut() {
                Some(OpenEntry::Behavior(b)) => {
                    if trimmed.starts_with("check(") {
                        // 結構行:首行=canonical(parse 面),全數計數(⑤ 恰 1 查)。
                        if b.check_lines.is_empty() {
                            b.check = Some(parse_check_line(trimmed));
                        }
                        b.check_lines.push(trimmed.to_string());
                    } else {
                        b.statement.push('\n');
                        b.statement.push_str(raw);
                    }
                }
                Some(OpenEntry::Scenario(s)) => {
                    s.statement.push('\n');
                    s.statement.push_str(raw);
                }
                None => {}
            }
            continue;
        }
        // 節內散文段:跳過零語義(R1-4 ③)。
    }
    flush_entry(&mut open, &mut doc);
    Ok(doc)
}

fn is_b_id(id: &str) -> bool {
    let mut ch = id.chars();
    ch.next() == Some('B') && id.len() > 1 && ch.all(|c| c.is_ascii_digit())
}

fn is_scene_id(id: &str) -> bool {
    id.strip_prefix("SCENE")
        .map_or(false, |r| !r.is_empty() && r.chars().all(|c| c.is_ascii_digit()))
}

fn missing_fields(form: &str, names: &[&str]) -> String {
    format!("{form} 缺欄位: {}", names.join(", "))
}

/// spec lint 七查(§3;findings 序=①→⑦,同查內=檔案行序;⑥ 缺集字典序)。
pub fn spec_lint(doc: &SpecDoc) -> Vec<String> {
    let mut f = Vec::new();
    // ① spec-format 在且=1。
    match &doc.spec_format {
        None => f.push("①spec-format: 行缺失(需 `- spec-format: 1`)".to_string()),
        Some(v) => {
            if v.trim() != "1" {
                f.push(format!("①spec-format: 值=`{v}` ≠ 1"));
            }
        }
    }
    // ② module 非空。
    if doc.module.as_deref().map_or(true, |m| m.trim().is_empty()) {
        f.push("②module: 行缺失或空(需 `- module: <DUT 側模組名>`)".to_string());
    }
    // ③ port ≥1 且 dir/type 詞表。
    if doc.ports.is_empty() {
        f.push(
            "③port: 行缺失(≥1 行 `- port: <name> <input|output> <wire|reg> <scalar|[N:0]>`)"
                .to_string(),
        );
    }
    for p in &doc.ports {
        if p.dir != "input" && p.dir != "output" {
            f.push(format!("③port: `{}` dir=`{}` ∉ input|output", p.name, p.dir));
        }
        if p.kind != "wire" && p.kind != "reg" {
            f.push(format!("③port: `{}` type=`{}` ∉ wire|reg", p.name, p.kind));
        }
        if p.width.is_empty() {
            f.push(format!("③port: `{}` 缺 width 欄(scalar|[N:0])", p.name));
        }
    }
    // ④ behavior id 唯一(B\d+)。
    let mut seen_b = HashSet::new();
    for b in &doc.behavior {
        if !is_b_id(&b.id) {
            f.push(format!("④behavior: id `{}` 不合 B\\d+", b.id));
        } else if !seen_b.insert(b.id.clone()) {
            f.push(format!("④behavior: id `{}` 重複", b.id));
        }
    }
    // ⑤ form∈四收形且欄位非空、每 behavior 條目結構行恰 1。
    for b in &doc.behavior {
        if b.check_lines.len() != 1 {
            f.push(format!(
                "⑤{}: 結構行數={}≠1(behavior 條目須恰 1 條 check 行)",
                b.id,
                b.check_lines.len()
            ));
            continue;
        }
        match &b.check {
            Some(SpecCheck::Guard {
                trigger,
                when,
                async_raw,
                effects,
            }) => {
                let mut missing = Vec::new();
                let trig_ok = trigger
                    .as_ref()
                    .map_or(false, |t| !t.signal.is_empty() && !t.edge.is_empty());
                if !trig_ok {
                    missing.push("trigger");
                }
                if when.as_deref().map_or(true, str::is_empty) {
                    missing.push("when");
                }
                if async_raw.as_deref().map_or(true, str::is_empty) {
                    missing.push("async");
                }
                if effects.is_empty() {
                    missing.push("effects");
                }
                if !missing.is_empty() {
                    f.push(format!("⑤{}: {}", b.id, missing_fields("guard", &missing)));
                }
            }
            Some(SpecCheck::Transition { reg, from, to, when }) => {
                let mut missing = Vec::new();
                if reg.as_deref().map_or(true, str::is_empty) {
                    missing.push("reg");
                }
                if from.is_none() {
                    missing.push("from");
                }
                if to.is_none() {
                    missing.push("to");
                }
                if when.as_deref().map_or(true, str::is_empty) {
                    missing.push("when");
                }
                if !missing.is_empty() {
                    f.push(format!("⑤{}: {}", b.id, missing_fields("transition", &missing)));
                }
            }
            Some(SpecCheck::Sequence {
                latency,
                signal,
                becomes,
                when,
            }) => {
                let mut missing = Vec::new();
                if latency.is_none() {
                    missing.push("latency");
                }
                if signal.as_deref().map_or(true, str::is_empty) {
                    missing.push("signal");
                }
                if becomes.as_deref().map_or(true, str::is_empty) {
                    missing.push("becomes");
                }
                if when.as_deref().map_or(true, str::is_empty) {
                    missing.push("when");
                }
                if !missing.is_empty() {
                    f.push(format!("⑤{}: {}", b.id, missing_fields("sequence", &missing)));
                }
            }
            Some(SpecCheck::Relation { lhs, rhs, when }) => {
                let mut missing = Vec::new();
                if lhs.as_deref().map_or(true, str::is_empty) {
                    missing.push("lhs");
                }
                if rhs.as_deref().map_or(true, str::is_empty) {
                    missing.push("rhs");
                }
                if when.as_deref().map_or(true, str::is_empty) {
                    missing.push("when");
                }
                if !missing.is_empty() {
                    f.push(format!("⑤{}: {}", b.id, missing_fields("relation", &missing)));
                }
            }
            Some(SpecCheck::Other { form }) => {
                f.push(format!(
                    "⑤{}: form=`{form}` ∉ 四收形(guard/transition/sequence/relation)",
                    b.id
                ));
            }
            None => {} // 結構行恰 1 時 parse 必產 Some;此臂不可達,防禦保留。
        }
    }
    // ⑥ decl_lint:結構行值側標識符 ⊆ ports∪params∪states∪derived(字典序)。
    let mut declared = HashSet::new();
    for p in &doc.ports {
        declared.insert(p.name.clone());
    }
    for p in &doc.params {
        declared.insert(p.name.clone());
    }
    for s in &doc.states {
        declared.insert(s.name.clone());
    }
    for d in &doc.derived {
        declared.insert(d.name.clone());
    }
    for b in &doc.behavior {
        if b.check_lines.len() != 1 {
            continue; // 結構行劣化已由 ⑤ 報;無 canonical 值側可掃。
        }
        if !matches!(
            &b.check,
            Some(
                SpecCheck::Guard { .. }
                    | SpecCheck::Transition { .. }
                    | SpecCheck::Sequence { .. }
                    | SpecCheck::Relation { .. }
            )
        ) {
            continue; // 未立文法形(invariant 等)無值側定義,由 ⑤ 報。
        }
        let mut refs = BTreeSet::new();
        collect_idents(&b.check_lines[0], &mut refs);
        for ex in EXCLUDE {
            refs.remove(*ex);
        }
        let missing: Vec<String> = refs
            .iter()
            .filter(|x| !declared.contains(*x))
            .cloned()
            .collect();
        if !missing.is_empty() {
            f.push(format!(
                "⑥{}: 未宣告標識符 [{}] — 應宣告於 port/param/state/derived 或修正引用",
                b.id,
                missing.join(", ")
            ));
        }
    }
    // ⑦ scenarios ≥1 且 SCENE<n> 唯一。
    if doc.scenarios.is_empty() {
        f.push("⑦scenarios: 缺失(≥1 條 `- SCENE<n> <name>: <一句行為契約>`)".to_string());
    }
    let mut seen_s = HashSet::new();
    for s in &doc.scenarios {
        if !is_scene_id(&s.id) {
            f.push(format!("⑦scenario: id `{}` 不合 SCENE<n>", s.id));
        } else if !seen_s.insert(s.id.clone()) {
            f.push(format!("⑦scenario: id `{}` 重複", s.id));
        }
    }
    f
}

/// findings JSON(緊湊字串陣列;CLI/wasm/MCP 三面 byte-equal 的單一源,
/// D-S3 禁各面自拼)。
pub fn spec_lint_json(doc: &SpecDoc) -> String {
    Json::Arr(spec_lint(doc).into_iter().map(Json::Str).collect()).render()
}

fn j_str(v: impl Into<String>) -> Json {
    Json::Str(v.into())
}

fn j_opt(v: &Option<String>) -> Json {
    match v {
        Some(x) => Json::Str(x.clone()),
        None => Json::Null,
    }
}

fn j_opt_u32(v: &Option<u32>) -> Json {
    match v {
        Some(x) => Json::Int(i64::from(*x)),
        None => Json::Null,
    }
}

fn check_json(c: &SpecCheck) -> Json {
    match c {
        SpecCheck::Guard {
            trigger,
            when,
            async_raw,
            effects,
        } => Json::Obj(vec![
            ("form".to_string(), j_str("guard")),
            (
                "trigger".to_string(),
                match trigger {
                    Some(t) => Json::Obj(vec![
                        ("signal".to_string(), j_str(t.signal.clone())),
                        ("edge".to_string(), j_str(t.edge.clone())),
                    ]),
                    None => Json::Null,
                },
            ),
            ("when".to_string(), j_opt(when)),
            ("async".to_string(), j_opt(async_raw)),
            (
                "effects".to_string(),
                Json::Arr(
                    effects
                        .iter()
                        .map(|e| {
                            Json::Obj(vec![
                                ("lhs".to_string(), j_str(e.lhs.clone())),
                                ("rhs".to_string(), j_str(e.rhs.clone())),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]),
        SpecCheck::Transition { reg, from, to, when } => Json::Obj(vec![
            ("form".to_string(), j_str("transition")),
            ("reg".to_string(), j_opt(reg)),
            ("from".to_string(), j_opt(from)),
            ("to".to_string(), j_opt(to)),
            ("when".to_string(), j_opt(when)),
        ]),
        SpecCheck::Sequence {
            latency,
            signal,
            becomes,
            when,
        } => Json::Obj(vec![
            ("form".to_string(), j_str("sequence")),
            ("latency".to_string(), j_opt_u32(latency)),
            ("signal".to_string(), j_opt(signal)),
            ("becomes".to_string(), j_opt(becomes)),
            ("when".to_string(), j_opt(when)),
        ]),
        SpecCheck::Relation { lhs, rhs, when } => Json::Obj(vec![
            ("form".to_string(), j_str("relation")),
            ("lhs".to_string(), j_opt(lhs)),
            ("rhs".to_string(), j_opt(rhs)),
            ("when".to_string(), j_opt(when)),
        ]),
        SpecCheck::Other { form } => Json::Obj(vec![("form".to_string(), j_str(form.clone()))]),
    }
}

/// spec 檔 JSON(緊湊形;鍵序=寫出序 byte-exact 契約,單測釘死)。
pub fn spec_json(doc: &SpecDoc) -> String {
    let arr = |items: Vec<Json>| Json::Arr(items);
    Json::Obj(vec![
        ("name".to_string(), j_opt(&doc.name)),
        ("spec_format".to_string(), j_opt(&doc.spec_format)),
        ("module".to_string(), j_opt(&doc.module)),
        (
            "params".to_string(),
            arr(doc
                .params
                .iter()
                .map(|p| {
                    Json::Obj(vec![
                        ("name".to_string(), j_str(p.name.clone())),
                        ("expr".to_string(), j_str(p.expr.clone())),
                    ])
                })
                .collect()),
        ),
        (
            "ports".to_string(),
            arr(doc
                .ports
                .iter()
                .map(|p| {
                    Json::Obj(vec![
                        ("name".to_string(), j_str(p.name.clone())),
                        ("dir".to_string(), j_str(p.dir.clone())),
                        ("kind".to_string(), j_str(p.kind.clone())),
                        ("width".to_string(), j_str(p.width.clone())),
                    ])
                })
                .collect()),
        ),
        (
            "states".to_string(),
            arr(doc
                .states
                .iter()
                .map(|s| {
                    Json::Obj(vec![
                        ("name".to_string(), j_str(s.name.clone())),
                        ("width".to_string(), j_str(s.width.clone())),
                        ("role".to_string(), j_str(s.role.clone())),
                    ])
                })
                .collect()),
        ),
        (
            "derived".to_string(),
            arr(doc
                .derived
                .iter()
                .map(|d| {
                    Json::Obj(vec![
                        ("name".to_string(), j_str(d.name.clone())),
                        ("width".to_string(), j_str(d.width.clone())),
                        ("role".to_string(), j_str(d.role.clone())),
                    ])
                })
                .collect()),
        ),
        (
            "assumes".to_string(),
            arr(doc.assumes.iter().map(|a| j_str(a.clone())).collect()),
        ),
        (
            "behavior".to_string(),
            arr(doc
                .behavior
                .iter()
                .map(|b| {
                    Json::Obj(vec![
                        ("id".to_string(), j_str(b.id.clone())),
                        ("category".to_string(), j_opt(&b.category)),
                        ("statement".to_string(), j_str(b.statement.clone())),
                        (
                            "check".to_string(),
                            b.check.as_ref().map(check_json).unwrap_or(Json::Null),
                        ),
                    ])
                })
                .collect()),
        ),
        (
            "scenarios".to_string(),
            arr(doc
                .scenarios
                .iter()
                .map(|s| {
                    Json::Obj(vec![
                        ("id".to_string(), j_str(s.id.clone())),
                        ("name".to_string(), j_str(s.name.clone())),
                        ("statement".to_string(), j_str(s.statement.clone())),
                    ])
                })
                .collect()),
        ),
    ])
    .render()
}
