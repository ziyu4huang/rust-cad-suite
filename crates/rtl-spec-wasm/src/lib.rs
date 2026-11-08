//! WASM facade over the rtl-spec rulebook/gap core (ADR-0012, D3).
//!
//! Facade pattern (10.7 core-first): all semantics live in [`rtl_spec`];
//! this crate only translates the boundary — JSON strings in, JSON strings
//! out, **no js_sys, no callbacks, no state, no BigInt edge** (line numbers
//! are u32). Built with `wasm-pack build crates/rtl-spec-wasm --target web
//! --out-dir pkg`; loaded from JS/Bun with explicit bytes:
//! `init({ module_or_path: new Uint8Array(readFileSync(...)) })`.
//!
//! Export face (11 fns; D-S3 adds spec parse/lint on top of §6.1 five MCP
//! tools closed on both langs + tb_plan): analyze/extract/scan × {sv, rtlil} +
//! tb_plan × {sv, rtlil} + spec parse/lint + version. Errors are embedded as
//! `{"error"/"ok":false, ...}` JSON, never thrown across the boundary.

use wasm_bindgen::prelude::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Package identity (matches plugin manifest `version`).
#[wasm_bindgen]
pub fn rtl_spec_version() -> String {
    VERSION.to_string()
}

fn sv_input(name: &str, source: &str) -> rtl_spec::SourceInput {
    rtl_spec::SourceInput::Sv(rtl_spec::SvInput {
        name: name.to_string(),
        source: source.to_string(),
        ..Default::default()
    })
}

fn rtlil_input(name: &str, source: &str) -> rtl_spec::SourceInput {
    rtl_spec::SourceInput::Rtlil(rtl_spec::RtlilInput {
        name: name.to_string(),
        source: source.to_string(),
    })
}

/// default export parity with native callers (rtl_spec::DEFAULT_MAX_RULES;
/// u32=0 視作默認——wasm 面標量入參契約)。
const DEFAULT_MAX_RULES: u32 = rtl_spec::DEFAULT_MAX_RULES as u32;

#[wasm_bindgen]
pub fn rtl_analyze_sv(name: &str, source: &str) -> String {
    rtl_spec::analysis_json(&rtl_spec::analyze_summary(&sv_input(name, source)))
}

#[wasm_bindgen]
pub fn rtl_extract_rules_sv(name: &str, source: &str, max_rules: u32) -> String {
    let max = if max_rules == 0 {
        DEFAULT_MAX_RULES as usize
    } else {
        max_rules as usize
    };
    rtl_spec::rulebook_json(&rtl_spec::extract_rules(&sv_input(name, source), max))
}

#[wasm_bindgen]
pub fn rtl_scan_gaps_sv(name: &str, source: &str) -> String {
    rtl_spec::gap_report_json(&rtl_spec::scan_gaps(&sv_input(name, source)))
}

#[wasm_bindgen]
pub fn rtl_analyze_rtlil(name: &str, source: &str) -> String {
    rtl_spec::analysis_json(&rtl_spec::analyze_summary(&rtlil_input(name, source)))
}

#[wasm_bindgen]
pub fn rtl_extract_rules_rtlil(name: &str, source: &str) -> String {
    rtl_spec::rulebook_json(&rtl_spec::extract_rules(
        &rtlil_input(name, source),
        DEFAULT_MAX_RULES as usize,
    ))
}

#[wasm_bindgen]
pub fn rtl_scan_gaps_rtlil(name: &str, source: &str) -> String {
    rtl_spec::gap_report_json(&rtl_spec::scan_gaps(&rtlil_input(name, source)))
}

/// tb_plan sv 面(RV1-11/RV2-8):`module==""`=未指定(單模組自動選取);
/// seed/cycles/flush==0=核心默認(TbOpts::resolve,與 max_rules==0 同約定);
/// 回傳幀=`{"ok":true,"plan":{…},"template_v":"…"}` 或
/// `{"ok":false,"error":"…(milestone TB1)"}`(錯誤走資料,不 trap)。
#[wasm_bindgen]
pub fn rtl_tb_plan_sv(name: &str, source: &str, module: &str, seed: u32, cycles: u32, flush: u32) -> String {
    let m = if module.is_empty() { None } else { Some(module) };
    rtl_spec::tb_plan_frame(&sv_input(name, source), m, seed, cycles, flush)
}

/// tb_plan rtlil 面(RV2-8:恆 TB1 錯誤幀;硬錯文案由核心產生,本面零語義)。
#[wasm_bindgen]
pub fn rtl_tb_plan_rtlil(name: &str, source: &str, module: &str, seed: u32, cycles: u32, flush: u32) -> String {
    let m = if module.is_empty() { None } else { Some(module) };
    rtl_spec::tb_plan_frame(&rtlil_input(name, source), m, seed, cycles, flush)
}

/// spec 檔解析面(D-S3):name 沿導出形(解析零名消費;SpecDoc.name 自檔
/// 頭 `# spec:`);回傳=spec_json;Err(現無產出點)→ `{"error":…}` 幀。
#[wasm_bindgen]
pub fn rtl_spec_spec_parse(name: &str, source: &str) -> String {
    let _ = name;
    match rtl_spec::spec::parse_spec_md(source) {
        Ok(doc) => rtl_spec::spec::spec_json(&doc),
        Err(e) => rtl_spec::json::error_json(e.to_string()),
    }
}

/// spec 七查 lint 面(D-S3):findings 緊湊字串陣列(與 CLI `spec lint`
/// stdout byte-equal——run-spec-cli-parity spec 腿判據)。
#[wasm_bindgen]
pub fn rtl_spec_spec_lint(name: &str, source: &str) -> String {
    let _ = name;
    match rtl_spec::spec::parse_spec_md(source) {
        Ok(doc) => rtl_spec::spec::spec_lint_json(&doc),
        Err(e) => rtl_spec::json::error_json(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    // host 面:導出函數在非 wasm 目標直呼(wasm-bindgen 宏對宿主編譯為
    // 普通函數);11 導出回合法 JSON(sv 面 7 導出無 {"error"};tb_plan 恒
    // {"ok":…} 幀;spec 面見 spec_exports 測)。
    const SV: &str = "module m(input wire clk, input wire d, output reg q);\n  always @(posedge clk) q <= d;\nendmodule\n";
    const IL: &str = "module \\m\n  wire width 1 input 0 \\clk\n  wire width 1 output 1 \\q\n  wire width 1 \\n0\n  process $p\n    assign \\n0 \\q\n    sync posedge \\clk\n      update \\q \\n0\n  end\nend\n";

    fn assert_no_error(s: &str) {
        assert!(s.starts_with('{'), "not JSON object: {s}");
        assert!(!s.contains("\"error\""), "embedded error: {s}");
    }

    #[test]
    fn nine_exports_return_valid_json_on_host() {
        assert!(!super::rtl_spec_version().is_empty());
        for s in [
            super::rtl_analyze_sv("m.v", SV),
            super::rtl_extract_rules_sv("m.v", SV, 128),
            super::rtl_scan_gaps_sv("m.v", SV),
            super::rtl_analyze_rtlil("m.il", IL),
            super::rtl_extract_rules_rtlil("m.il", IL),
            super::rtl_scan_gaps_rtlil("m.il", IL),
        ] {
            assert_no_error(&s);
        }
        // tb_plan sv 正樣:{"ok":true,...};rtlil 面:恆 {"ok":false,...}(RV2-8)。
        let tb_ok = super::rtl_tb_plan_sv("m.v", SV, "", 0, 0, 0);
        assert!(tb_ok.starts_with("{\"ok\":true,"), "{tb_ok}");
        assert!(tb_ok.contains("\"template_v\":"), "{tb_ok}");
        let tb_il = super::rtl_tb_plan_rtlil("m.il", IL, "", 0, 0, 0);
        assert!(tb_il.starts_with("{\"ok\":false,") && tb_il.contains("milestone TB1"), "{tb_il}");
    }

    #[test]
    fn spec_exports_parse_and_lint() {
        const GOOD: &str = "# spec: t\n- spec-format: 1\n- module: m\n- port: a input wire scalar\n\n## behavior\n\n- B001 [c] 一\n  check(relation): a ≡ a when -\n\n## scenarios\n\n- SCENE1 s: c\n";
        let parsed = super::rtl_spec_spec_parse("t.spec.md", GOOD);
        assert!(parsed.contains("\"module\":\"m\"") && parsed.contains("\"behavior\""), "{parsed}");
        assert_eq!(super::rtl_spec_spec_lint("t.spec.md", GOOD), "[]");
        const BAD: &str = "- module: m\n"; // 缺 spec-format ①
        let findings = super::rtl_spec_spec_lint("b.spec.md", BAD);
        assert!(findings.starts_with("[\"") && findings.contains("①"), "{findings}");
        // parse 面對劣化輸入仍出 JSON(全容錯;Err 幀現無產出點)。
        assert!(super::rtl_spec_spec_parse("b.spec.md", BAD).starts_with('{'));
    }

    #[test]
    fn wasm_face_deterministic_same_input_same_bytes() {
        let a = super::rtl_extract_rules_sv("m.v", SV, 128);
        let b = super::rtl_extract_rules_sv("m.v", SV, 128);
        assert_eq!(a, b);
    }
}
