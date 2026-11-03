//! 規則書/破口類型詞表(RTL2SPEC-PLUGIN §2/§3;ADR-0012)。
//!
//! 欄位契約見正典 §2.1:id=模組內單調序;statement=中文模板(標識符保持源碼
//! 原文名,D7);qualifier=derived(結構直讀可證)|inferred(模式推斷);
//! evidence 必含 ≥1 行為類(反 doc-dump 鎖①,decl 只許輔助);check.form
//! ∈五形(§2.4)且 form 專屬欄位非空(鎖②);fingerprint=拼接鍵串零哈希
//! (D13,鍵=source|category|module|subject|when|effects)。

/// extract_rules 的模組內規則上限(D12 opts 標量化;超限截斷並記入
/// Rulebook.truncated,驗證閘 V1 對 truncated 非空→該檔 FAIL)。
pub const DEFAULT_MAX_RULES: usize = 128;

/// §2.2 類別分類學 v1(9 類;詞表=機械閘 V2/V3 的域)。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Reset,
    Fsm,
    Handshake,
    Timing,
    Arithmetic,
    Counter,
    Memory,
    Protocol,
    Param,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Reset => "reset",
            Category::Fsm => "fsm",
            Category::Handshake => "handshake",
            Category::Timing => "timing",
            Category::Arithmetic => "arithmetic",
            Category::Counter => "counter",
            Category::Memory => "memory",
            Category::Protocol => "protocol",
            Category::Param => "param",
        }
    }

    /// 行為類(參與 V2 占比分子);param 與 structure(本弧不產)記非行為類。
    pub fn is_behavioural(self) -> bool {
        !matches!(self, Category::Param)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Qualifier {
    /// 語法結構直接證明(如復位分支的賦值對)。
    Derived,
    /// 模式匹配推斷(如握手模式)。
    Inferred,
}

impl Qualifier {
    pub fn as_str(self) -> &'static str {
        match self {
            Qualifier::Derived => "derived",
            Qualifier::Inferred => "inferred",
        }
    }
}

/// §2.1 evidence.class:行為類=前五種(process|assign|case|generate|expr);
/// decl 只允許作輔助附加,不許單獨成證(反 doc-dump 鎖①)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceClass {
    Process,
    Assign,
    Case,
    Generate,
    Expr,
    Decl,
}

impl EvidenceClass {
    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceClass::Process => "process",
            EvidenceClass::Assign => "assign",
            EvidenceClass::Case => "case",
            EvidenceClass::Generate => "generate",
            EvidenceClass::Expr => "expr",
            EvidenceClass::Decl => "decl",
        }
    }

    pub fn is_behavioural(self) -> bool {
        !matches!(self, EvidenceClass::Decl)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Evidence {
    /// SV 側=輸入檔名;RTLIL 側=`<name>.il`(D2)。
    pub file: String,
    /// SV 側=1-based 行號(主檔 span);RTLIL 側=0。
    pub line: u32,
    pub end_line: u32,
    pub class: EvidenceClass,
    /// 與源行逐位元組對拍(V4);拼接規則=多行各行原文按序 `\n` 連接
    /// (保留縮進,末行不帶終結換行),單行=該行原文去行尾換行(R1-10)。
    pub snippet: String,
    /// RTLIL 結構路徑證形(D2),SV 側 None。
    pub path: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckForm {
    Guard,
    Transition,
    Sequence,
    Relation,
    Invariant,
}

impl CheckForm {
    pub fn as_str(self) -> &'static str {
        match self {
            CheckForm::Guard => "guard",
            CheckForm::Transition => "transition",
            CheckForm::Sequence => "sequence",
            CheckForm::Relation => "relation",
            CheckForm::Invariant => "invariant",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    pub lhs: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Trigger {
    pub edge: Option<String>,
    pub signal: Option<String>,
}

/// check.form 五形詞表(§2.4);表達式欄位一律存源碼渲染文本(AST print,
/// 不改寫、不求值)。payload 變體與 form 一一綁定=鎖②的類型級形。
#[derive(Clone, Debug, PartialEq)]
pub enum CheckPayload {
    Guard {
        trigger: Option<Trigger>,
        when: Option<String>,
        effects: Vec<Effect>,
    },
    Transition {
        state_reg: String,
        from: String,
        to: String,
        when: Option<String>,
    },
    Sequence {
        signal: String,
        becomes: String,
        anchor: String,
        latency: u32,
        when: Option<String>,
    },
    Relation {
        lhs: String,
        rhs: String,
        /// 適用域(收平輪 R3-②):條件鏈非空=該恆等式只在分支域成立
        /// (分支轉寫不冒充全域);None=全域。
        when: Option<String>,
        width_note: Option<String>,
    },
    Invariant {
        subject: String,
        kind: InvKind,
        values: Vec<String>,
        expr: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvKind {
    Domain,
    Identity,
}

impl InvKind {
    pub fn as_str(self) -> &'static str {
        match self {
            InvKind::Domain => "domain",
            InvKind::Identity => "identity",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    pub form: CheckForm,
    pub payload: CheckPayload,
}

impl Check {
    /// fingerprint 的 when 維(guard/transition/sequence/relation 的 when;
    /// invariant 空)。relation 入 when 維=R3-②:同 lhs 同 rhs 不同分支
    /// 域不再互併,指紋可區分適用域。
    pub fn when_text(&self) -> &str {
        match &self.payload {
            CheckPayload::Guard { when, .. } => when.as_deref().unwrap_or(""),
            CheckPayload::Transition { when, .. } => when.as_deref().unwrap_or(""),
            CheckPayload::Sequence { when, .. } => when.as_deref().unwrap_or(""),
            CheckPayload::Relation { when, .. } => when.as_deref().unwrap_or(""),
            _ => "",
        }
    }

    /// fingerprint 的 subject 維。
    pub fn subject(&self) -> String {
        match &self.payload {
            CheckPayload::Guard { effects, .. } => {
                effects.first().map(|e| e.lhs.clone()).unwrap_or_default()
            }
            CheckPayload::Transition { state_reg, .. } => state_reg.clone(),
            CheckPayload::Sequence { signal, .. } => signal.clone(),
            CheckPayload::Relation { lhs, .. } => lhs.clone(),
            CheckPayload::Invariant { subject, .. } => subject.clone(),
        }
    }

    /// fingerprint 的 effects 維(`lhs<=value` 以 `;` 連接;他形=becomes/rhs
    /// 同形渲染,保證同 subject 同 when 的不同賦值不去重)。
    pub fn effects_text(&self) -> String {
        match &self.payload {
            CheckPayload::Guard { effects, .. } => effects
                .iter()
                .map(|e| format!("{}<={}", e.lhs, e.value))
                .collect::<Vec<_>>()
                .join(";"),
            CheckPayload::Transition { to, .. } => to.clone(),
            CheckPayload::Sequence { becomes, .. } => becomes.clone(),
            CheckPayload::Relation { rhs, .. } => rhs.clone(),
            CheckPayload::Invariant { values, expr, .. } => {
                let mut parts: Vec<String> = values.clone();
                if let Some(e) = expr {
                    parts.push(e.clone());
                }
                parts.join(";")
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Rule {
    /// `<module>.R<nn>`(3 位零填充,模組內單調序)。
    pub id: String,
    pub category: Category,
    pub module: String,
    /// 中文模板句(標識符=源碼原文名,D7;模板機械生成禁自由散文)。
    pub statement: String,
    pub qualifier: Qualifier,
    pub evidence: Vec<Evidence>,
    pub check: Check,
    /// `source|category|module|subject|when|effects` 拼接鍵串(D13;同指紋
    /// =生成器 bug,全集零重複=V5-③)。
    pub fingerprint: String,
}

#[derive(Clone, Debug)]
pub struct ModuleRules {
    pub module: String,
    pub rules: Vec<Rule>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    /// 檔含 `$finish`(TB 啟發判定;TB 不是被提取對象,R2-6/R7b)。
    TbFile,
    /// 該輸入形本弧未支援(帶里程碑;硬錯契約以注記形呈現)。
    Unsupported,
}

impl SkipReason {
    pub fn as_str(self) -> &'static str {
        match self {
            SkipReason::TbFile => "tb_file",
            SkipReason::Unsupported => "unsupported",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SkippedNote {
    pub file: String,
    pub reason: SkipReason,
}

#[derive(Clone, Debug)]
pub struct Rulebook {
    pub source: String,
    /// 僅解析錯誤(R2-6:TB 跳過/未支援注記不入此欄)。
    pub parse_errors: Vec<String>,
    pub skipped: Vec<SkippedNote>,
    /// 一模組一組(含零規則模組)。
    pub modules: Vec<ModuleRules>,
    /// 超 max_rules 被截斷的模組名(V1 執法:非空→該檔 FAIL)。
    pub truncated: Vec<String>,
}

impl Rulebook {
    /// 規則總數(全集;V1 的 Σrules)。
    pub fn total_rules(&self) -> usize {
        self.modules.iter().map(|m| m.rules.len()).sum()
    }

    /// 類別分布(category → 數量,全集;V3 的 distinct 計數來源)。
    pub fn category_counts(&self) -> Vec<(&'static str, usize)> {
        let mut counts: Vec<(&'static str, usize)> = Vec::new();
        for m in &self.modules {
            for r in &m.rules {
                let key = r.category.as_str();
                if let Some(c) = counts.iter_mut().find(|(k, _)| *k == key) {
                    c.1 += 1;
                } else {
                    counts.push((key, 1));
                }
            }
        }
        counts
    }
}

// ---- 結構投影摘要(R2-1;rtl_analyze 的資料源) ----

/// 結構投影摘要:analyzer-core Analysis 公開查詢直讀組合,零新語義
/// (欄位=ModuleInfo view.rs:8-28 直投;兩輸入形同一查詢形)。
#[derive(Clone, Debug)]
pub struct AnalysisSummary {
    pub source: String,
    pub has_errors: bool,
    pub diagnostics_count: usize,
    pub modules: Vec<ModuleSummary>,
    /// analyzer-core 同名直通 `(from_module, to_module)`。
    pub instance_edges: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub struct ModuleSummary {
    pub name: String,
    /// (name, value 文本)。
    pub params: Vec<(String, String)>,
    /// (name, dir, width, signed)。
    pub ports: Vec<(String, String, u32, bool)>,
    /// (name, kind, width, is_port)。
    pub nets: Vec<(String, String, u32, bool)>,
    /// (name, ctype, port_count)。
    pub cells: Vec<(String, String, usize)>,
    pub proc_count: usize,
    pub assign_count: usize,
}

// ---- 破口(§3;S3 填充掃描器) ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GapClass {
    Latch,
    IncompleteCase,
    UndrivenRead,
    MissingReset,
    CombLoop,
    MultiDriver,
    BlockingInSeq,
}

impl GapClass {
    pub fn as_str(self) -> &'static str {
        match self {
            GapClass::Latch => "latch",
            GapClass::IncompleteCase => "incomplete_case",
            GapClass::UndrivenRead => "undriven_read",
            GapClass::MissingReset => "missing_reset",
            GapClass::CombLoop => "comb_loop",
            GapClass::MultiDriver => "multi_driver",
            GapClass::BlockingInSeq => "blocking_in_seq",
        }
    }

    /// §3.1 id 前綴(G1..G7)。
    pub fn prefix(self) -> &'static str {
        match self {
            GapClass::Latch => "G1",
            GapClass::IncompleteCase => "G2",
            GapClass::UndrivenRead => "G3",
            GapClass::MissingReset => "G4",
            GapClass::CombLoop => "G5",
            GapClass::MultiDriver => "G6",
            GapClass::BlockingInSeq => "G7",
        }
    }
}

/// G1..G7 結構可證=certain;G2 值域覆蓋與 G3 驅動圖受限場景=heuristic(§3.2)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confidence {
    Certain,
    Heuristic,
}

impl Confidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Confidence::Certain => "certain",
            Confidence::Heuristic => "heuristic",
        }
    }
}

#[derive(Clone, Debug)]
pub struct GapItem {
    /// `G<n>-<module>-<nn>`(模組內序號,3 位零填充)。
    pub id: String,
    pub class: GapClass,
    pub module: String,
    pub confidence: Confidence,
    /// 中文;機械模板生成。
    pub message: String,
    pub evidence: Vec<Evidence>,
    /// §3.1 測試缺口模板(=破口報告即測試缺口清單的載體,§3.2)。
    pub suggested_test: String,
}

#[derive(Clone, Debug)]
pub struct GapReport {
    pub source: String,
    pub parse_errors: Vec<String>,
    pub skipped: Vec<SkippedNote>,
    pub items: Vec<GapItem>,
}
