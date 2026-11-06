//! rtl-spec — RTL 行為規則書提取與破口掃描核心(ADR-0012)。
//!
//! 語義姿態對齊 analyzer-core:永不 panic、錯誤進結果;輸入=源文本字串
//! (零 FS 觸碰→wasm 安全);走 analyzer-core 逃逸口(Analysis.sv /
//! Analysis.rtlil),**不擴 analyzer-core**(D1:其立檔禁區「求值/解析」
//! 不承載規則歸納)。JSON 編碼復用 analyzer_core::proto::json::Json,
//! **只寫不讀**(D12 opts 標量化→庫內零 JSON parser 依賴;鍵序=寫出序
//! byte-exact 契約,單測釘死)。深提取的規則歸納是行為語義層(非求值):
//! 質量契約=反 doc-dump 三鎖(RTL2SPEC-PLUGIN §2.3)+V1..V6 機械判據
//! (§8.3),無 oracle,禁虛構 golden。

pub mod expr;
pub mod json;
mod lint;
pub mod md;
pub mod rtlil;
pub mod schema;
pub mod sv;
pub mod summary;

pub use analyzer_core::{RtlilInput, SourceInput, SvInput};
pub use schema::{
    AnalysisSummary, Category, Check, CheckForm, CheckPayload, Confidence, Contract, ContractParam,
    ContractPort, ContractSignal, DeclLint, Effect, Evidence, EvidenceClass, GapClass, GapItem,
    GapReport, InvKind, ModuleRules, ModuleSummary, Qualifier, Rule, Rulebook, SkippedNote,
    SkipReason, Trigger, DEFAULT_MAX_RULES,
};

/// 提取規則書(永不 panic;parse 錯誤進 Rulebook.parse_errors;TB 檔整檔
/// 跳過記 skipped;超 max_rules_per_module 截斷記 truncated)。
pub fn extract_rules(input: &SourceInput, max_rules_per_module: usize) -> Rulebook {
    match input {
        SourceInput::Sv(i) => sv::extract_rules_sv(i, max_rules_per_module),
        SourceInput::Rtlil(i) => rtlil::extract_rules_rtlil(i, max_rules_per_module),
    }
}

/// 破口掃描(同姿態;v1 全類開,過濾在呼叫層)。
pub fn scan_gaps(input: &SourceInput) -> GapReport {
    match input {
        SourceInput::Sv(i) => sv::scan_gaps_sv(i),
        SourceInput::Rtlil(i) => rtlil::scan_gaps_rtlil(i),
    }
}

/// 結構投影摘要(rtl_analyze 的源;零新語義直讀組合)。
pub fn analyze_summary(input: &SourceInput) -> AnalysisSummary {
    summary::analyze_summary(input)
}

/// JSON 寫出(手寫,鍵序=寫出序 byte-exact 契約)。
pub fn rulebook_json(rb: &Rulebook) -> String {
    json::rulebook_json(rb)
}

pub fn gap_report_json(gr: &GapReport) -> String {
    json::gap_report_json(gr)
}

pub fn analysis_json(s: &AnalysisSummary) -> String {
    json::analysis_json(s)
}
