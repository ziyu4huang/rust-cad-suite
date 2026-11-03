//! S2/S3 驗收測試:提取器五基類 + 破口 G1..G7 + JSON 鍵序 byte-exact +
//! 敵意轉義 + V4 生成端自對拍 + repo 語料只讀集成(carryadd ≥3=R1-3
//! 回歸錨)。自著 fixtures=repo MIT;repo 語料零改動唯讀引用。

use std::collections::HashSet;

use analyzer_core::SvInput;
use rtl_spec::{
    analysis_json, extract_rules, gap_report_json, scan_gaps, Category, CheckPayload,
    Effect, Evidence, EvidenceClass, Qualifier, Rule, Rulebook, SourceInput, Trigger,
    DEFAULT_MAX_RULES,
};

fn sv(name: &str, source: &str) -> SourceInput {
    SourceInput::Sv(SvInput {
        name: name.to_string(),
        source: source.to_string(),
        ..Default::default()
    })
}

/// V4 生成端自對拍:snippet 必等於源檔對應行按拼接規則重算(與 TS checker
/// 同一定義;違者=生成端拼接規則漂移)。
fn assert_v4(rb: &Rulebook, source: &str) {
    let lines: Vec<&str> = source.split('\n').collect();
    for m in &rb.modules {
        for r in &m.rules {
            for e in &r.evidence {
                assert!(e.line >= 1, "{} {} evidence line=0", rb.source, r.id);
                assert!(e.end_line >= e.line, "{} {}", rb.source, r.id);
                let want = lines[(e.line - 1) as usize..e.end_line as usize].join("\n");
                assert_eq!(
                    e.snippet, want,
                    "{} {}:{}-{} snippet 與源行逐位元組不對",
                    rb.source, r.id, e.line, e.end_line
                );
            }
        }
    }
}

// ---- S2:提取器五基類 ----

#[test]
fn rulebook_sample_extracts_five_categories() {
    let src = include_str!("fixtures/rulebook_sample.v");
    let rb = extract_rules(&sv("rulebook_sample.v", src), DEFAULT_MAX_RULES);
    assert!(rb.parse_errors.is_empty(), "{:?}", rb.parse_errors);
    assert!(rb.skipped.is_empty());
    assert!(rb.truncated.is_empty());
    assert_v4(&rb, src);

    // 分類精確數:reset=3(三個復位效果各一條)/fsm=2/timing=2/counter=1/
    // arithmetic=1(calc = start | busy)。
    let counts = rb.category_counts();
    let get = |k: &str| counts.iter().find(|(c, _)| *c == k).map(|(_, n)| *n).unwrap_or(0);
    assert_eq!(rb.total_rules(), 9, "counts={counts:?}");
    assert_eq!(get("reset"), 3);
    assert_eq!(get("fsm"), 2);
    assert_eq!(get("timing"), 2);
    assert_eq!(get("counter"), 1);
    assert_eq!(get("arithmetic"), 1);

    let m = &rb.modules[0];
    assert_eq!(m.module, "rulebook_sample");
    // id 單調序。
    for (i, r) in m.rules.iter().enumerate() {
        assert_eq!(r.id, format!("rulebook_sample.R{:03}", i + 1));
    }
    // 全集指紋零重複(V5-③ 生成端面)。
    let fps: HashSet<&str> = m.rules.iter().map(|r| r.fingerprint.as_str()).collect();
    assert_eq!(fps.len(), m.rules.len());

    // 復位規則形:guard + trigger(posedge clk)+ when(!rst_n)。
    let r0 = &m.rules[0];
    assert_eq!(r0.category, Category::Reset);
    assert_eq!(r0.qualifier, Qualifier::Derived);
    assert!(r0.statement.contains("rst_n") && r0.statement.contains("state"));
    match &r0.check.payload {
        CheckPayload::Guard {
            trigger: Some(t),
            when: Some(w),
            effects,
        } => {
            assert_eq!((t.edge.as_deref(), t.signal.as_deref()), (Some("posedge"), Some("clk")));
            assert_eq!(w, "!rst_n");
            assert_eq!(effects.len(), 1);
            assert_eq!(effects[0].lhs, "state");
            assert_eq!(effects[0].value, "2'b00");
        }
        other => panic!("reset rule check 形錯: {other:?}"),
    }
    assert_eq!(r0.evidence[0].class, EvidenceClass::Process);

    // FSM 遷移形:from=2'b00,to=2'b01,when=start(臂內 If 條件;case 臂
    // 條件文本不重複入 when——from 已承載)。
    let fsm = m
        .rules
        .iter()
        .find(|r| r.category == Category::Fsm)
        .expect("fsm rule 在場");
    match &fsm.check.payload {
        CheckPayload::Transition {
            state_reg, from, to, when,
        } => {
            assert_eq!(state_reg, "state");
            assert_eq!(from, "2'b00");
            assert_eq!(to, "2'b01");
            assert_eq!(when.as_deref(), Some("start"));
        }
        other => panic!("fsm check 形錯: {other:?}"),
    }
    assert_eq!(fsm.evidence[0].class, EvidenceClass::Case);
    assert!(fsm.statement.contains("state=2'b00") && fsm.statement.contains("下一拍 state=2'b01"));

    // counter 形:self±(count <= count + 8'd1)。
    let counter = m
        .rules
        .iter()
        .find(|r| r.category == Category::Counter)
        .expect("counter rule 在場");
    match &counter.check.payload {
        CheckPayload::Sequence { signal, becomes, latency, .. } => {
            assert_eq!(signal, "count");
            assert_eq!(becomes, "count + 8'd1");
            assert_eq!(*latency, 1);
        }
        other => panic!("counter check 形錯: {other:?}"),
    }

    // arithmetic 形:relation(rended rhs 保留 | 運算符)。
    let arith = m
        .rules
        .iter()
        .find(|r| r.category == Category::Arithmetic)
        .expect("arithmetic rule 在場");
    match &arith.check.payload {
        CheckPayload::Relation { lhs, rhs, when, .. } => {
            assert_eq!(lhs, "calc");
            assert_eq!(rhs, "start | busy");
            assert!(when.is_none(), "連續賦值=全域恆等,when=None(R3-②)");
        }
        other => panic!("arithmetic check 形錯: {other:?}"),
    }
    assert_eq!(arith.evidence[0].class, EvidenceClass::Assign);
    assert!(arith.statement.contains("calc ≡ start | busy"));
}

/// R1-1 行程分類律回歸錨(提取面):@* 組合塊內含 Binary 的賦值必產
/// arithmetic(規則)且 edge 行程不誤入組合提取。
#[test]
fn comb_process_classified_by_edges_not_ctrl_none() {
    let src = r#"module comb_extract(input wire a, input wire b, output wire c);
  always @* begin
    c = a & b;
  end
endmodule
"#;
    let rb = extract_rules(&sv("comb.v", src), DEFAULT_MAX_RULES);
    assert_eq!(rb.total_rules(), 1);
    assert_eq!(rb.modules[0].rules[0].category, Category::Arithmetic);
    assert_eq!(rb.modules[0].rules[0].evidence[0].class, EvidenceClass::Process);
}

/// R3-② relation 適用域:分支域賦值攜 when(不冒充全域恆等——subbytes
/// R007「state==0 else 分支被全局誤讀」實錄);全域連續賦值 when=None;
/// 指紋含 when 維;rhs 為變量/常量拷貝(無 Binary)不產 relation=覆蓋缺口
/// 的展示形(誠實注記同步 review-notes)。
#[test]
fn relation_rule_carries_branch_domain_when() {
    let src = r#"module cond_relation(input wire [1:0] state, output reg [1:0] next_state, output wire out);
  assign out = state | 2'd1;
  always @* begin
    next_state = state;
    if (state == 2'd0) begin
      next_state = state + 2'd1;
    end
  end
endmodule
"#;
    let rb = extract_rules(&sv("cond_relation.v", src), DEFAULT_MAX_RULES);
    // out=全域連續賦值 relation;next_state=state 拷貝(無 Binary)不產;
    // next_state=state+2'd1 分支域 relation 一條。計 2 條。
    assert_eq!(rb.total_rules(), 2, "實得 {:?}", rb.modules[0].rules.iter().map(|r| (&r.id, &r.statement)).collect::<Vec<_>>());

    let branch = rb.modules[0]
        .rules
        .iter()
        .find(|r| r.check.subject() == "next_state")
        .expect("分支域 relation 在場");
    match &branch.check.payload {
        CheckPayload::Relation { lhs, rhs, when, width_note } => {
            assert_eq!(lhs, "next_state");
            assert_eq!(rhs, "state + 2'd1");
            assert_eq!(when.as_deref(), Some("state == 2'd0"), "分支域必攜 when");
            assert!(width_note.is_none());
        }
        other => panic!("{other:?}"),
    }
    assert!(branch.statement.contains("當 state == 2'd0 時"), "{}", branch.statement);
    assert!(branch.statement.contains("next_state ≡ state + 2'd1"));
    // 指紋含 when 維:source|category|module|subject|when|effects。
    assert!(
        branch.fingerprint.contains("|state == 2'd0|state + 2'd1"),
        "{}", branch.fingerprint
    );

    let global = rb.modules[0]
        .rules
        .iter()
        .find(|r| r.check.subject() == "out")
        .expect("全域 relation 在場");
    match &global.check.payload {
        CheckPayload::Relation { when, .. } => assert!(when.is_none(), "全域恆等 when=None"),
        other => panic!("{other:?}"),
    }
    assert!(!global.statement.contains("當"), "全域恆等陳述無條件前綴");
}

#[test]
fn timing_rule_uses_enclosing_condition_as_anchor() {
    let src = r#"module timing_anchor(input wire clk, input wire d, output reg q, output reg held);
  always @(posedge clk) begin
    q <= d;
    if (d) begin
      held <= 1'b1;
    end
  end
endmodule
"#;
    let rb = extract_rules(&sv("t.v", src), DEFAULT_MAX_RULES);
    let held = rb.modules[0]
        .rules
        .iter()
        .find(|r| r.category == Category::Timing && r.check.subject() == "held")
        .expect("held timing rule");
    match &held.check.payload {
        CheckPayload::Sequence { anchor, latency, .. } => {
            assert_eq!(anchor, "d");
            assert_eq!(*latency, 1);
        }
        other => panic!("{other:?}"),
    }
    // q 無條件鏈 → anchor=時鐘沿文本。
    let q = rb.modules[0]
        .rules
        .iter()
        .find(|r| r.check.subject() == "q")
        .expect("q timing rule");
    match &q.check.payload {
        CheckPayload::Sequence { anchor, .. } => assert_eq!(anchor, "clk 上升沿"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn dedup_by_fingerprint_and_truncation() {
    let src = r#"module dupmod(input wire clk, input wire d, output reg q, output reg r);
  always @(posedge clk) begin
    q <= d;
    q <= d;
  end
  always @(posedge clk) begin
    r <= d;
  end
endmodule
"#;
    // 同行程兩條同賦值 → 指紋同 → 去重保首;模組內上限=1 → 截斷記錄。
    let rb = extract_rules(&sv("dup.v", src), 1);
    assert_eq!(rb.modules[0].rules.len(), 1);
    assert_eq!(rb.truncated, vec!["dupmod".to_string()]);
    // 默認上限不截斷。
    let rb2 = extract_rules(&sv("dup.v", src), DEFAULT_MAX_RULES);
    assert_eq!(rb2.truncated.len(), 0);
    assert_eq!(rb2.total_rules(), 2);
}

// ---- repo 語料只讀集成(R1-3 回歸錨) ----

#[test]
fn carryadd_yields_at_least_three_arithmetic_rules() {
    let src = include_str!("../../../corpus-external/yosys-simple/carryadd.v");
    let rb = extract_rules(&sv("carryadd.v", src), DEFAULT_MAX_RULES);
    assert_v4(&rb, src);
    let counts = rb.category_counts();
    let arith = counts
        .iter()
        .find(|(c, _)| *c == "arithmetic")
        .map(|(_, n)| *n)
        .unwrap_or(0);
    assert!(arith >= 3, "carryadd 必產 ≥3 條(R1-3 放寬回歸錨),實得 {counts:?}");
    // generate 語境證據類。
    for m in &rb.modules {
        for r in &m.rules {
            assert_eq!(r.evidence[0].class, EvidenceClass::Generate);
        }
    }
    // 誤報面:良構 generate 設計零破口(G3 不報聲明初始化驅動)。
    let gaps = scan_gaps(&sv("carryadd.v", src));
    assert!(
        gaps.items.is_empty(),
        "carryadd 零誤報破口,實得 {:?}",
        gaps.items.iter().map(|g| (g.id.as_str(), &g.message)).collect::<Vec<_>>()
    );
}

#[test]
fn fiedler_cooley_extracts_timing_and_flags_missing_reset() {
    let src = include_str!("../../../corpus-external/yosys-simple/fiedler-cooley.v");
    let rb = extract_rules(&sv("fiedler-cooley.v", src), DEFAULT_MAX_RULES);
    assert_v4(&rb, src);
    assert!(rb.total_rules() >= 8, "實得 {}", rb.total_rules());
    assert!(rb
        .modules
        .iter()
        .all(|m| m.rules.iter().all(|r| r.category == Category::Timing)));
    // 該檔確實無復位臂 → G4 單列(certain)。
    let gaps = scan_gaps(&sv("fiedler-cooley.v", src));
    assert!(gaps
        .items
        .iter()
        .any(|g| g.class.as_str() == "missing_reset" && g.confidence.as_str() == "certain"));
}

// ---- S3:破口 G1..G7 正/負樣 ----

#[test]
fn gap_positive_fixtures_trigger_expected_classes() {
    let src = include_str!("fixtures/gaps_positive.v");
    let gaps = scan_gaps(&sv("gaps_positive.v", src));
    let classes_of = |module: &str| -> Vec<&'static str> {
        let mut v: Vec<&'static str> = gaps
            .items
            .iter()
            .filter(|g| g.module == module)
            .map(|g| g.class.as_str())
            .collect();
        v.sort();
        v.dedup();
        v
    };
    assert_eq!(classes_of("latch_trap"), vec!["latch"]);
    // G1 R3-① fixture 矩陣(正樣):x 有塊首默認賦值=對沖不報;y 真 latch
    // 形仍報且 confidence=certain(AGENTS 10.10 certain=結構可證契約——
    // 無默認+分支不對稱=Verilog 語義下結構可證,置信度鏈復核維持)。
    assert_eq!(classes_of("latch_default_matrix"), vec!["latch"]);
    {
        let g = gaps
            .items
            .iter()
            .find(|g| g.module == "latch_default_matrix")
            .unwrap();
        assert_eq!(g.confidence.as_str(), "certain");
        assert!(g.message.contains("y"), "{}", g.message);
        assert!(
            !g.message.contains("x") && !g.suggested_test.contains("x"),
            "默認賦值覆蓋的 x 被誤報:{} / {}",
            g.message,
            g.suggested_test
        );
    }
    assert_eq!(classes_of("incomplete_case"), vec!["incomplete_case"]);
    assert_eq!(classes_of("undriven_read"), vec!["undriven_read"]);
    assert_eq!(classes_of("missing_reset"), vec!["missing_reset"]);
    assert_eq!(classes_of("comb_loop"), vec!["comb_loop"]);
    assert_eq!(classes_of("multi_driver"), vec!["missing_reset", "multi_driver"]);
    assert_eq!(
        classes_of("blocking_race"),
        vec!["blocking_in_seq", "missing_reset", "multi_driver"]
    );

    // G1 @* 形(R1-1 端到端錨):latch_trap 的破口證據必在 @* 行程上。
    let g1 = gaps.items.iter().find(|g| g.module == "latch_trap").unwrap();
    assert_eq!(g1.confidence.as_str(), "certain");
    assert!(g1.message.contains("y") && g1.message.contains("sel"));
    assert!(g1.suggested_test.contains("sel"));

    // G2 帶寬度證:2 位選擇子 → 2/4 編碼覆蓋。
    let g2 = gaps.items.iter().find(|g| g.module == "incomplete_case").unwrap();
    assert_eq!(g2.confidence.as_str(), "heuristic");
    assert!(g2.message.contains("2/4"), "{}", g2.message);

    // G5 環路徑。
    let g5 = gaps.items.iter().find(|g| g.module == "comb_loop").unwrap();
    assert!(g5.message.contains("t1") && g5.message.contains("t2"), "{}", g5.message);

    // id 形:G<n>-<module>-<nn>。
    assert!(gaps.items.iter().all(|g| {
        let parts: Vec<&str> = g.id.split('-').collect();
        parts.len() == 3 && parts[1] == g.module && parts[2].len() == 3
    }));
}

#[test]
fn gap_negative_fixtures_zero_false_positives() {
    let src = include_str!("fixtures/gaps_negative.v");
    let gaps = scan_gaps(&sv("gaps_negative.v", src));
    assert!(
        gaps.items.is_empty(),
        "良構設計必須零破口,實得 {:?}",
        gaps.items.iter().map(|g| (g.id.as_str(), &g.message)).collect::<Vec<_>>()
    );
    // R3-① 負樣抽點釘死:latch_default_ok(塊首默認賦值形,subbytes.v
    // 誤報同形)零命中。
    let src2 = r#"module latch_default_ok(input wire a, input wire sel, output reg y, output reg z);
  always @* begin
    y = 1'b0;
    z = 1'b1;
    if (sel) begin
      y = a;
      if (a) z = 1'b0;
    end
  end
endmodule
"#;
    let gaps2 = scan_gaps(&sv("latch_default_ok.v", src2));
    assert!(
        gaps2.items.iter().all(|g| g.class.as_str() != "latch"),
        "塊首默認賦值形誤報 G1,實得 {:?}",
        gaps2.items.iter().map(|g| &g.message).collect::<Vec<_>>()
    );
}

#[test]
fn concat_lhs_assign_is_a_driver_no_undriven_gap() {
    // R6 誤報形釘死:`assign {Cout,S} = …` 拼接 LHS 的各部件=被驅動
    // (multiplier.v FullAdder G3 誤報實錄,已修)。
    let src = r#"module fa(input wire a, input wire b, input wire cin, output wire s, output wire cout);
  assign {cout, s} = a + b + cin;
endmodule
"#;
    let gaps = scan_gaps(&sv("fa.v", src));
    assert!(
        gaps.items.is_empty(),
        "拼接 LHS 誤報 G3,實得 {:?}",
        gaps.items.iter().map(|g| (g.id.as_str(), &g.message)).collect::<Vec<_>>()
    );
    // 正樣仍在:concat LHS 不驅動被讀的無驅動源——dangling(被讀、無驅動)
    // 照報;cout/s 有驅動(拼接賦值)不報。
    let src2 = r#"module fa2(input wire a, output wire s, output wire cout);
  wire dangling;
  assign {cout, s} = a & dangling;
endmodule
"#;
    let gaps2 = scan_gaps(&sv("fa2.v", src2));
    assert!(gaps2.items.iter().all(|g| g.class.as_str() != "undriven_read"
        || (g.message.contains("dangling")
            && !g.message.contains("cout")
            && !g.message.contains("s "))));
}

#[test]
fn tb_file_is_skipped_with_note_not_parse_error() {
    let src = include_str!("fixtures/tb_sample.v");
    let rb = extract_rules(&sv("tb_sample.v", src), DEFAULT_MAX_RULES);
    assert!(rb.parse_errors.is_empty());
    assert_eq!(rb.modules.len(), 0);
    assert_eq!(rb.skipped.len(), 1);
    assert_eq!(rb.skipped[0].reason.as_str(), "tb_file");
    let gaps = scan_gaps(&sv("tb_sample.v", src));
    assert_eq!(gaps.skipped.len(), 1);
    assert_eq!(gaps.items.len(), 0);
}

// ---- JSON:鍵序 byte-exact + 敵意轉義(R9) ----

#[test]
fn rulebook_json_key_order_is_byte_exact() {
    let rb = Rulebook {
        source: "demo.v".to_string(),
        parse_errors: vec!["VL-E100: syntax error".to_string()],
        skipped: vec![rtl_spec::SkippedNote {
            file: "tb.v".to_string(),
            reason: rtl_spec::SkipReason::TbFile,
        }],
        modules: vec![rtl_spec::ModuleRules {
            module: "demo".to_string(),
            rules: vec![Rule {
                id: "demo.R001".to_string(),
                category: Category::Reset,
                module: "demo".to_string(),
                statement: "當 !rst_n 且 clk 上升沿時,state 裝載 2'b00".to_string(),
                qualifier: Qualifier::Derived,
                evidence: vec![Evidence {
                    file: "demo.v".to_string(),
                    line: 5,
                    end_line: 6,
                    class: EvidenceClass::Process,
                    snippet: "if (!rst_n) begin\n  state <= 2'b00;".to_string(),
                    path: None,
                }],
                check: rtl_spec::Check {
                    form: rtl_spec::CheckForm::Guard,
                    payload: CheckPayload::Guard {
                        trigger: Some(Trigger {
                            edge: Some("posedge".to_string()),
                            signal: Some("clk".to_string()),
                        }),
                        when: Some("!rst_n".to_string()),
                        effects: vec![Effect {
                            lhs: "state".to_string(),
                            value: "2'b00".to_string(),
                        }],
                    },
                },
                fingerprint: "demo.v|reset|demo|state|!rst_n|state<=2'b00".to_string(),
            }],
        }],
        truncated: vec![],
    };
    let json = rtl_spec::rulebook_json(&rb);
    let want = "{\"source\":\"demo.v\",\"parse_errors\":[\"VL-E100: syntax error\"],\
\"skipped\":[{\"file\":\"tb.v\",\"reason\":\"tb_file\"}],\
\"modules\":[{\"module\":\"demo\",\"rules\":[\
{\"id\":\"demo.R001\",\"category\":\"reset\",\"module\":\"demo\",\
\"statement\":\"當 !rst_n 且 clk 上升沿時,state 裝載 2'b00\",\
\"qualifier\":\"derived\",\
\"evidence\":[{\"file\":\"demo.v\",\"line\":5,\"end_line\":6,\"class\":\"process\",\
\"snippet\":\"if (!rst_n) begin\\n  state <= 2'b00;\"}],\
\"check\":{\"form\":\"guard\",\"trigger\":{\"edge\":\"posedge\",\"signal\":\"clk\"},\
\"when\":\"!rst_n\",\"effects\":[{\"lhs\":\"state\",\"value\":\"2'b00\"}]},\
\"fingerprint\":\"demo.v|reset|demo|state|!rst_n|state<=2'b00\"}]}],\"truncated\":[]}";
    assert_eq!(json, want);
}

#[test]
fn json_escapes_hostile_strings() {
    let rb = Rulebook {
        source: "we\"ird\\name中文\n.v".to_string(),
        parse_errors: vec![],
        skipped: vec![],
        modules: vec![],
        truncated: vec![],
    };
    let json = rtl_spec::rulebook_json(&rb);
    // 引號/反斜線/換行轉義 + 非 ASCII 原樣保留(UTF-8 直出)。
    assert!(json.contains("we\\\"ird\\\\name中文\\n.v"), "{json}");
}

#[test]
fn gap_report_json_key_order() {
    let gr = rtl_spec::GapReport {
        source: "x.v".to_string(),
        parse_errors: vec![],
        skipped: vec![],
        items: vec![rtl_spec::GapItem {
            id: "G1-m-001".to_string(),
            class: rtl_spec::GapClass::Latch,
            module: "m".to_string(),
            confidence: rtl_spec::Confidence::Certain,
            message: "訊息".to_string(),
            evidence: vec![],
            suggested_test: "測試".to_string(),
        }],
    };
    let json = gap_report_json(&gr);
    assert!(json.starts_with(
        "{\"source\":\"x.v\",\"parse_errors\":[],\"skipped\":[],\"items\":\
[{\"id\":\"G1-m-001\",\"class\":\"latch\",\"module\":\"m\",\"confidence\":\"certain\","
    ), "{json}");
}

#[test]
fn analysis_json_key_order() {
    let sum = rtl_spec::analyze_summary(&sv("a.v", "module a(input wire x, output wire y);\n  assign y = x;\nendmodule\n"));
    let json = analysis_json(&sum);
    assert!(json.starts_with(
        "{\"source\":\"a.v\",\"has_errors\":false,\"diagnostics_count\":"
    ), "{json}");
    assert!(json.contains("\"modules\":[{\"name\":\"a\",\"params\":[],\"ports\":["));
    assert!(json.contains("\"proc_count\":0,\"assign_count\":1}"));
    // 結構投影欄位:埠表/網表。
    assert!(json.contains("\"dir\":\"input\",\"width\":1,\"signed\":false"));
    assert_eq!(sum.modules.len(), 1);
    assert_eq!(sum.instance_edges.len(), 0);
}

// ---- markdown 渲染(報告全文樣本的資料源) ----

#[test]
fn markdown_render_contains_samples() {
    let src = include_str!("fixtures/rulebook_sample.v");
    let rb = extract_rules(&sv("rulebook_sample.v", src), DEFAULT_MAX_RULES);
    let md = rb.render_markdown();
    assert!(md.starts_with("# 規則書:rulebook_sample.v"));
    assert!(md.contains("## 模組 rulebook_sample(9 條)"));
    assert!(md.contains("### rulebook_sample.R001 [reset/derived]"));
    assert!(md.contains("```verilog"));
    assert!(md.contains("- 指紋:`rulebook_sample.v|reset|"));

    let gaps = scan_gaps(&sv("rulebook_sample.v", src));
    assert!(gaps.items.is_empty(), "rulebook_sample 良構應零破口");
    let gsrc = include_str!("fixtures/gaps_positive.v");
    let gmd = scan_gaps(&sv("gaps_positive.v", gsrc)).render_markdown();
    assert!(gmd.starts_with("# 破口報告:gaps_positive.v"));
    assert!(gmd.contains("- 建議測試:"));
}
