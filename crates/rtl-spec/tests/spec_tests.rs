//! spec 模組單測(D-S1;RTL2SPEC-D-PLAN §4 D-S1/§3):三正樣
//! fixtures/d-spec/*.spec.md 各 parse 出 2/9/94 條+零 findings+欄位逐項
//! 對拍;行級文法三形(散文/###/續行)斷言;七查負樣各觸發恰好對應
//! finding+零誤報形(async=true/parametric width/derived 引用);spec_json
//! 鍵序 byte-exact 釘死。語料唯讀(plugin fixtures=自著 MIT)。

use rtl_spec::spec::{
    parse_spec_md, spec_json, spec_lint, SpecCheck, SpecDoc,
};

fn repo_file(rel: &str) -> String {
    // crates/rtl-spec → repo 根=兩層(10.6 跨目錄引用先數層級)。
    std::fs::read_to_string(format!(
        "{}/../../{rel}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|e| panic!("語料檔在場 {rel}: {e}"))
}

fn parse_file(rel: &str) -> SpecDoc {
    let src = repo_file(rel);
    parse_spec_md(&src).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// 全欄位合法的最小 spec(七查負樣的公共基底;先自證零 findings)。
/// raw string 逐字縮進(普通字串 `\` 續行會吞下一行前導空白=結構行失縮進)。
const BASE: &str = r#"# spec: t
- spec-format: 1
- module: tmod
- port: clk input wire scalar
- port: rst_n input wire scalar
- port: q output reg scalar

## behavior

- B001 [reset] 復位裝載
  check(guard): trigger=clk posedge; when=rst_n; async=true; effects=q←1'b0

## scenarios

- SCENE1 a: 契約
"#;

/// 七欄全形+零誤報正樣(async=true/parametric width/derived 引用/sized 字面量)。
const FULL: &str = r#"# spec: t
- spec-format: 1
- module: tmod
- param: W = 2'd3
- param: A = 2'd0
- param: B = 2'd1
- port: clk input wire scalar
- port: rst_n input wire scalar
- port: q output reg [W-1:0]
- state: s scalar 狀態
- derived: d [2:0] 組合
- assumes: 無

## behavior

- B001 [reset] 復位裝載
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=q←{W{1'b0}}
- B002 [count] 下一拍自增
  check(sequence): +1拍 → q=(q+1'b1) when !rst_n
- B003 [fsm] 轉換
  check(transition): s: A → B when d == 3'd7
- B004 [rel] 恆等
  check(relation): d ≡ {1'b0,q} when -

## scenarios

- SCENE1 one: 契約
"#;

fn mutate(anchor: &str, replacement: &str) -> String {
    assert_eq!(BASE.matches(anchor).count(), 1, "mutation anchor drift: {anchor}");
    BASE.replace(anchor, replacement)
}

fn one_finding(src: &str, prefix: &str) -> String {
    let doc = parse_spec_md(src).expect("parse");
    let mut f = spec_lint(&doc);
    assert_eq!(f.len(), 1, "恰 1 條 {prefix} finding,得 {f:?}");
    assert!(f[0].starts_with(prefix), "finding `{}` 應以 {prefix} 起首", f[0]);
    f.remove(0)
}

#[test]
fn base_spec_is_clean() {
    let doc = parse_spec_md(BASE).expect("parse");
    assert!(spec_lint(&doc).is_empty(), "{:?}", spec_lint(&doc));
}

#[test]
fn counter8_positive_fields() {
    let doc = parse_file("plugin/rtl2spec/fixtures/d-spec/counter8.spec.md");
    assert_eq!(doc.name.as_deref(), Some("counter8"));
    assert_eq!(doc.spec_format.as_deref(), Some("1"));
    assert_eq!(doc.module.as_deref(), Some("counter"));
    assert_eq!(doc.params.len(), 1);
    assert_eq!(doc.params[0].name, "WIDTH");
    assert_eq!(doc.params[0].expr, "8");
    assert_eq!(doc.ports.len(), 3);
    assert_eq!(doc.ports[0].name, "clk");
    assert_eq!(doc.ports[2].name, "q");
    assert_eq!(doc.ports[2].dir, "output");
    assert_eq!(doc.ports[2].kind, "reg");
    assert_eq!(doc.ports[2].width, "[WIDTH-1:0]");
    assert!(doc.states.is_empty());
    assert!(doc.derived.is_empty());
    assert!(doc.assumes.is_empty());
    assert_eq!(doc.behavior.len(), 2);
    assert_eq!(doc.scenarios.len(), 6);
    assert_eq!(doc.behavior[0].id, "B001");
    assert_eq!(doc.behavior[0].category.as_deref(), Some("reset"));
    assert_eq!(
        doc.behavior[0].statement,
        "同步復位:clk 上升沿且 rst 成立時,下一拍 q 裝載全 0(WIDTH 位)"
    );
    assert_eq!(doc.behavior[0].check_lines.len(), 1);
    match &doc.behavior[0].check {
        Some(SpecCheck::Guard {
            trigger,
            when,
            async_raw,
            effects,
        }) => {
            let t = trigger.as_ref().expect("trigger 在場");
            assert_eq!((t.signal.as_str(), t.edge.as_str()), ("clk", "posedge"));
            assert_eq!(when.as_deref(), Some("rst"));
            assert_eq!(async_raw.as_deref(), Some("false"));
            assert_eq!(effects.len(), 1);
            assert_eq!(effects[0].lhs, "q");
            assert_eq!(effects[0].rhs, "{WIDTH{1'b0}}");
        }
        other => panic!("B001 應為 guard,得 {other:?}"),
    }
    match &doc.behavior[1].check {
        Some(SpecCheck::Sequence {
            latency,
            signal,
            becomes,
            when,
        }) => {
            assert_eq!(*latency, Some(1));
            assert_eq!(signal.as_deref(), Some("q"));
            assert_eq!(becomes.as_deref(), Some("(q+1'b1)"));
            assert_eq!(when.as_deref(), Some("!rst"));
        }
        other => panic!("B002 應為 sequence,得 {other:?}"),
    }
    assert!(
        spec_lint(&doc).is_empty(),
        "counter8 零 findings,得 {:?}",
        spec_lint(&doc)
    );
}

#[test]
fn acc_pipe_positive_fields() {
    let doc = parse_file("plugin/rtl2spec/fixtures/d-spec/acc_pipe.spec.md");
    assert_eq!(doc.name.as_deref(), Some("acc_pipe"));
    assert_eq!(doc.module.as_deref(), Some("__acc_pipe__acc_pipe"));
    assert!(doc.params.is_empty());
    assert_eq!(doc.ports.len(), 4);
    assert_eq!(doc.states.len(), 8);
    assert_eq!(doc.states[0].name, "x1");
    assert_eq!(doc.states[0].width, "[7:0]");
    assert_eq!(doc.states[3].name, "h1");
    assert!(doc.derived.is_empty());
    assert_eq!(doc.behavior.len(), 9);
    assert_eq!(doc.scenarios.len(), 6);
    // 散文段不成條目(R1-4 ③):三行散文被跳過,且不併入任何陳述。
    assert!(
        !doc
            .behavior
            .iter()
            .any(|b| b.statement.contains("三級流水")),
        "散文不得成條目或併入陳述"
    );
    // 續行併入陳述(R1-4 ④):SCENE5 第二行。
    assert_eq!(doc.scenarios[4].id, "SCENE5");
    assert_eq!(doc.scenarios[4].name, "dynamic_stream");
    assert!(
        doc.scenarios[4]
            .statement
            .contains('\n')
        && doc.scenarios[4].statement.contains("相差三拍的輸入組合"),
        "SCENE5 續行應併入陳述,得 {:?}",
        doc.scenarios[4].statement
    );
    // relation 拆解:≡ 兩側+when。
    match &doc.behavior[2].check {
        Some(SpecCheck::Relation { lhs, rhs, when }) => {
            assert_eq!(lhs.as_deref(), Some("s1"));
            assert_eq!(rhs.as_deref(), Some("{1'h0,x1} + {1'h0,y1}"));
            assert_eq!(when.as_deref(), Some("-"));
        }
        other => panic!("B003 應為 relation,得 {other:?}"),
    }
    // sequence 無條件形:when=-。
    match &doc.behavior[0].check {
        Some(SpecCheck::Sequence { signal, when, .. }) => {
            assert_eq!(signal.as_deref(), Some("x1"));
            assert_eq!(when.as_deref(), Some("-"));
        }
        other => panic!("B001 應為 sequence,得 {other:?}"),
    }
    assert!(spec_lint(&doc).is_empty(), "{:?}", spec_lint(&doc));
}

#[test]
fn i2c_positive_fields() {
    let doc = parse_file("plugin/rtl2spec/fixtures/d-spec/i2c_master.spec.md");
    assert_eq!(doc.name.as_deref(), Some("i2c_master"));
    assert_eq!(doc.module.as_deref(), Some("i2c_master"));
    assert_eq!(doc.params.len(), 10);
    assert_eq!(doc.params[0].name, "ST_IDLE");
    assert_eq!(doc.params[0].expr, "4'd0");
    assert_eq!(doc.ports.len(), 17);
    assert_eq!(doc.states.len(), 11);
    assert_eq!(doc.derived.len(), 1);
    assert_eq!(doc.derived[0].name, "cnt_end");
    assert_eq!(doc.derived[0].width, "scalar");
    assert_eq!(doc.behavior.len(), 94);
    assert_eq!(doc.scenarios.len(), 11);
    // ### 子節頭跳過(R1-4 ②):條目跨 9 個子節連續編號不中斷。
    assert_eq!(doc.behavior[20].id, "B021");
    assert_eq!(doc.behavior[93].id, "B094");
    // 全部 id 唯一(B\d+)由零 findings 蓋住;逐類 spot-check。
    match &doc.behavior[0].check {
        Some(SpecCheck::Guard {
            trigger,
            when,
            async_raw,
            effects,
        }) => {
            assert_eq!(async_raw.as_deref(), Some("true"));
            assert_eq!(when.as_deref(), Some("!rst_n"));
            assert_eq!(
                trigger.as_ref().map(|t| t.signal.as_str()),
                Some("clk")
            );
            assert_eq!(effects.len(), 1);
            assert_eq!(effects[0].lhs, "state");
            assert_eq!(effects[0].rhs, "ST_IDLE");
        }
        other => panic!("B001 應為 guard,得 {other:?}"),
    }
    // derived 引用形(R1-1 核心樣本):B021 relation 引用 cnt_end。
    match &doc.behavior[20].check {
        Some(SpecCheck::Relation { lhs, rhs, when }) => {
            assert_eq!(lhs.as_deref(), Some("cnt_end"));
            assert_eq!(rhs.as_deref(), Some("(cnt >= prescale - 16'd1)"));
            assert_eq!(when.as_deref(), Some("-"));
        }
        other => panic!("B021 應為 relation,得 {other:?}"),
    }
    match &doc.behavior[35].check {
        Some(SpecCheck::Transition { reg, from, to, when }) => {
            assert_eq!(reg.as_deref(), Some("state"));
            assert_eq!(from.as_deref(), Some("ST_IDLE"));
            assert_eq!(to.as_deref(), Some("ST_START"));
            assert_eq!(when.as_deref(), Some("pend_start"));
        }
        other => panic!("B036 應為 transition,得 {other:?}"),
    }
    // 條件含「且」與位選/拼接表示式的 sequence(B054);續行併入陳述(B035)。
    match &doc.behavior[53].check {
        Some(SpecCheck::Sequence {
            signal, becomes, when, ..
        }) => {
            assert_eq!(signal.as_deref(), Some("sda_oe"));
            assert_eq!(becomes.as_deref(), Some("~tx[7 - bitcnt[2:0]]"));
            assert_eq!(
                when.as_deref(),
                Some("state == ST_SHIFT 且 half == 1'b0 且 !rd_mode")
            );
        }
        other => panic!("B054 應為 sequence,得 {other:?}"),
    }
    assert!(
        doc.behavior[34].statement.contains('\n'),
        "B035 續行應併入陳述,得 {:?}",
        doc.behavior[34].statement
    );
    assert!(
        doc.scenarios[9].statement.contains('\n'),
        "SCENE10 續行應併入陳述"
    );
    let f = spec_lint(&doc);
    assert!(f.is_empty(), "i2c 零 findings(含 ⑥ derived cnt_end),得 {f:?}");
}

#[test]
fn line_grammar_forms() {
    // 散文(前/中)/### 子節頭/無空行條目界,三形直測。
    let src = r#"# spec: g
- spec-format: 1
- module: gm
- port: a input wire scalar

## behavior

開頭散文行(不成條目)
### 分組標題(跳過)
- B001 [c] 一
  check(relation): a ≡ a when -
中間散文行(不成條目、不關條目)
- B002 [c] 二
  check(sequence): +1拍 → a=1'b1 when -

## scenarios

- SCENE1 s: c
"#;
    let doc = parse_spec_md(src).expect("parse");
    assert_eq!(doc.behavior.len(), 2, "散文不得成條目: {:?}", doc.behavior);
    assert_eq!(doc.behavior[0].statement, "一");
    assert_eq!(doc.behavior[1].statement, "二");
    assert!(spec_lint(&doc).is_empty(), "{:?}", spec_lint(&doc));
}

#[test]
fn lint1_spec_format() {
    one_finding(&mutate("- spec-format: 1\n", ""), "①");
    one_finding(&mutate("- spec-format: 1", "- spec-format: 2"), "①");
}

#[test]
fn lint2_module() {
    one_finding(&mutate("- module: tmod\n", ""), "②");
}

#[test]
fn lint3_port() {
    one_finding(&mutate("- port: clk input wire scalar", "- port: clk in wire scalar"), "③");
    one_finding(&mutate("- port: clk input wire scalar", "- port: clk input banana scalar"), "③");
    // port 全缺(behavior 只引用 state,避開 ⑥ 連帶)。
    let src = r#"# spec: p3
- spec-format: 1
- module: m
- state: s scalar 狀態

## behavior

- B001 [c] 一
  check(relation): s ≡ s when -

## scenarios

- SCENE1 x: y
"#;
    one_finding(src, "③");
}

#[test]
fn lint4_behavior_id() {
    one_finding(&mutate("- B001 [reset]", "- X001 [reset]"), "④");
    one_finding(
        &mutate(
            "effects=q←1'b0\n",
            "effects=q←1'b0\n- B001 [dup] 另條\n  check(sequence): +1拍 → q=1'b1 when -\n",
        ),
        "④",
    );
}

#[test]
fn lint5_check_structure() {
    // (a) 零結構行;(b) 兩條結構行;(c) guard 缺 when;(d) invariant 形。
    let check_line = "  check(guard): trigger=clk posedge; when=rst_n; async=true; effects=q←1'b0\n";
    one_finding(&mutate(check_line, ""), "⑤");
    one_finding(&mutate(check_line, &format!("{check_line}{check_line}")), "⑤");
    one_finding(&mutate("; when=rst_n;", ";"), "⑤");
    one_finding(&mutate("check(guard)", "check(invariant)"), "⑤");
}

#[test]
fn lint6_decl_lint() {
    let f = one_finding(&mutate("when=rst_n", "when=rst_bad"), "⑥");
    assert!(f.contains("rst_bad"), "finding 應指名未宣告標識符: {f}");
    // 零誤報形:async=true guard+derived 引用+parametric width+sized 字面量
    // (§3 R1-1;FULL 即該形)。
    let doc = parse_spec_md(FULL).expect("parse");
    let all = spec_lint(&doc);
    assert!(all.is_empty(), "FULL 零誤報,得 {all:?}");
}

#[test]
fn lint7_scenarios() {
    one_finding(&mutate("\n## scenarios\n\n- SCENE1 a: 契約\n", "\n"), "⑦");
    one_finding(
        &mutate(
            "- SCENE1 a: 契約\n",
            "- SCENE1 a: 契約\n- SCENE1 a: 另契約\n",
        ),
        "⑦",
    );
    one_finding(&mutate("SCENE1 a:", "SCENE_A a:"), "⑦");
}

#[test]
fn spec_json_byte_exact() {
    // 鍵序=寫出序 byte-exact 契約(緊湊形;單測釘死)。
    let doc = parse_spec_md(FULL).expect("parse");
    let expected = "{\"name\":\"t\",\"spec_format\":\"1\",\"module\":\"tmod\",\
\"params\":[{\"name\":\"W\",\"expr\":\"2'd3\"},{\"name\":\"A\",\"expr\":\"2'd0\"},{\"name\":\"B\",\"expr\":\"2'd1\"}],\
\"ports\":[{\"name\":\"clk\",\"dir\":\"input\",\"kind\":\"wire\",\"width\":\"scalar\"},\
{\"name\":\"rst_n\",\"dir\":\"input\",\"kind\":\"wire\",\"width\":\"scalar\"},\
{\"name\":\"q\",\"dir\":\"output\",\"kind\":\"reg\",\"width\":\"[W-1:0]\"}],\
\"states\":[{\"name\":\"s\",\"width\":\"scalar\",\"role\":\"狀態\"}],\
\"derived\":[{\"name\":\"d\",\"width\":\"[2:0]\",\"role\":\"組合\"}],\
\"assumes\":[\"無\"],\
\"behavior\":[{\"id\":\"B001\",\"category\":\"reset\",\"statement\":\"復位裝載\",\
\"check\":{\"form\":\"guard\",\"trigger\":{\"signal\":\"clk\",\"edge\":\"posedge\"},\"when\":\"!rst_n\",\"async\":\"true\",\"effects\":[{\"lhs\":\"q\",\"rhs\":\"{W{1'b0}}\"}]}},\
{\"id\":\"B002\",\"category\":\"count\",\"statement\":\"下一拍自增\",\
\"check\":{\"form\":\"sequence\",\"latency\":1,\"signal\":\"q\",\"becomes\":\"(q+1'b1)\",\"when\":\"!rst_n\"}},\
{\"id\":\"B003\",\"category\":\"fsm\",\"statement\":\"轉換\",\
\"check\":{\"form\":\"transition\",\"reg\":\"s\",\"from\":\"A\",\"to\":\"B\",\"when\":\"d == 3'd7\"}},\
{\"id\":\"B004\",\"category\":\"rel\",\"statement\":\"恆等\",\
\"check\":{\"form\":\"relation\",\"lhs\":\"d\",\"rhs\":\"{1'b0,q}\",\"when\":\"-\"}}],\
\"scenarios\":[{\"id\":\"SCENE1\",\"name\":\"one\",\"statement\":\"契約\"}]}";
    assert_eq!(spec_json(&doc), expected);
}
