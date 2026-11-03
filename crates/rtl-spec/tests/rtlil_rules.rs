//! S4 驗收:RTLIL 子集(reset/fsm/timing 提取+結構路徑 evidence;G1/G5 掃描)。
//! fixture=oracle proc 後 -readable 正典形子集,自著(repo MIT)。

use analyzer_core::RtlilInput;
use rtl_spec::{extract_rules, scan_gaps, Category, CheckPayload, SourceInput, DEFAULT_MAX_RULES};

fn il(name: &str, source: &str) -> SourceInput {
    SourceInput::Rtlil(RtlilInput {
        name: name.to_string(),
        source: source.to_string(),
    })
}

#[test]
fn rtlil_sample_extracts_reset_timing_and_flags_g1_g5() {
    let src = include_str!("fixtures/sample.il");
    let rb = extract_rules(&il("sample.il", src), DEFAULT_MAX_RULES);
    assert!(rb.parse_errors.is_empty(), "{:?}", rb.parse_errors);
    let names: Vec<&str> = rb.modules.iter().map(|m| m.module.as_str()).collect();
    // ry-ir 名字逐字保留(`\` 前綴在場;analyzer-core RTLIL 同款)。
    assert_eq!(names, vec!["\\il_counter", "\\il_latch", "\\il_loop"]);

    // il_counter:reset(常量臂 1 條)+timing(counter update 1 條;fsm 面無)。
    let counter = rb
        .modules
        .iter()
        .find(|m| m.module == "\\il_counter")
        .expect("il_counter 在場");
    let counts = rb.category_counts();
    let reset = counts
        .iter()
        .find(|(c, _)| *c == "reset")
        .map(|(_, n)| *n)
        .unwrap_or(0);
    let timing = counts
        .iter()
        .find(|(c, _)| *c == "timing")
        .map(|(_, n)| *n)
        .unwrap_or(0);
    assert_eq!(reset, 1, "{counts:?}");
    assert_eq!(timing, 1, "{counts:?}");
    let r0 = &counter.rules[0];
    assert_eq!(r0.category, Category::Reset);
    match &r0.check.payload {
        CheckPayload::Guard { when: Some(w), effects, .. } => {
            assert_eq!(w, "\\reset == 1'1");
            assert_eq!(effects[0].lhs, "$0\\count[7:0]");
            assert_eq!(effects[0].value, "8'00000000");
        }
        other => panic!("{other:?}"),
    }
    // 結構路徑證形(D2):line=0,snippet="",path 帶 process/case 層級。
    assert_eq!(r0.evidence[0].line, 0);
    assert_eq!(r0.evidence[0].snippet, "");
    let path = r0.evidence[0].path.as_deref().expect("RTLIL path 證形");
    assert!(path.contains("process[0]") && path.contains("switch[0]"), "{path}");

    let t = counter
        .rules
        .iter()
        .find(|r| r.category == Category::Timing)
        .expect("timing 規則");
    match &t.check.payload {
        CheckPayload::Sequence { signal, becomes, anchor, latency, .. } => {
            assert_eq!(signal, "\\count");
            assert!(becomes.contains("update") || becomes.contains("$0\\count"), "{becomes}");
            assert_eq!(anchor, "\\clk 上升沿");
            assert_eq!(*latency, 1);
        }
        other => panic!("{other:?}"),
    }
    assert!(t.evidence[0].path.as_deref().unwrap().contains("sync.posedge"));

    // G1:il_latch 的 switch \sel 無 default 臂(保持值=鎖存,certain)。
    // G5:il_loop 的 connect \a \a 自環(certain)。
    let gaps = scan_gaps(&il("sample.il", src));
    let latch = gaps
        .items
        .iter()
        .find(|g| g.module == "\\il_latch")
        .expect("G1 latch 在場");
    assert_eq!(latch.class.as_str(), "latch");
    assert_eq!(latch.confidence.as_str(), "certain");
    assert!(latch.message.contains("\\sel") && latch.message.contains("default"));
    let loop_g = gaps
        .items
        .iter()
        .find(|g| g.module == "\\il_loop")
        .expect("G5 自環在場");
    assert_eq!(loop_g.class.as_str(), "comb_loop");
    assert!(loop_g.message.contains("\\a"));

    // markdown 渲染含結構路徑。
    let md = rb.render_markdown();
    assert!(md.contains("## 模組 \\il_counter"));
}

#[test]
fn rtlil_bad_parse_errors_surface_without_panic() {
    // 未宣告 wire 引用=ry-ir 語義驗證錯(AN-E001)→ parse_errors 照收。
    let bad = "module \\bad\n  wire width 1 input 0 \\x\n  connect \\y \\x\nend\n";
    let rb = extract_rules(&il("bad.il", bad), DEFAULT_MAX_RULES);
    assert!(!rb.parse_errors.is_empty(), "解析錯誤進 parse_errors:{:?}", rb.parse_errors);
    let gaps = scan_gaps(&il("bad.il", bad));
    assert!(!gaps.parse_errors.is_empty());
}
