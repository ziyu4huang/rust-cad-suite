//! tb_plan 單測(C-S2 步 b;§19.3.2):6 採樣設計 plan 結構斷言/fsm_counter
//! busy 寬 1(D-c2 依賴)/TB1 硬錯負樣/12 跑確定性。語料唯讀
//! (corpus/ 與 corpus-external/ 為 MIT/ISC;plugin fixtures=自著 MIT)。

use analyzer_core::{RtlilInput, SourceInput, SvInput};
use rtl_spec::{
    tb_plan, tb_plan_frame, tb_plan_json, tb_template_v, TbOpts, DEFAULT_CYCLES, DEFAULT_FLUSH,
    DEFAULT_SEED,
};

fn sv(name: &str, source: &str) -> SourceInput {
    SourceInput::Sv(SvInput {
        name: name.to_string(),
        source: source.to_string(),
        ..Default::default()
    })
}

fn repo_file(rel: &str) -> String {
    // crates/rtl-spec → repo 根=兩層(10.6 跨目錄引用先數層級)。
    std::fs::read_to_string(format!(
        "{}/../../{rel}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|e| panic!("語料檔在場 {rel}: {e}"))
}

fn plan_of_file(rel: &str, module: Option<&str>) -> rtl_spec::TbPlan {
    let src = repo_file(rel);
    tb_plan(&sv(rel, &src), module).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn tb1_err(input: &SourceInput, module: Option<&str>) -> String {
    match tb_plan(input, module) {
        Err(e) => e.to_string(),
        Ok(_) => panic!("expected TB1 error"),
    }
}

#[test]
fn c_s2_fsm_counter_fixture_busy_width_one() {
    // D-c2 依賴斷言(19.3.2 機械防線①):fixtures/fsm_counter.v 的 `busy`
    // 源碼 1 位(`output reg busy`)——修前 ANSI 組繼承誤報 4。contract 直讀。
    let src = repo_file("plugin/rtl2spec/fixtures/fsm_counter.v");
    let plan = tb_plan(&sv("fsm_counter.v", &src), None).expect("plan");
    assert_eq!(plan.module, "fsm_counter");
    let busy = plan.ports.iter().find(|p| p.name == "busy").unwrap();
    assert_eq!((busy.width, busy.dir.as_str(), busy.signed), (1, "output", false));
    assert_eq!(plan.clocks.len(), 1);
    assert_eq!(
        (plan.clocks[0].edge.as_str(), plan.clocks[0].signal.as_str()),
        ("posedge", "clk")
    );
    assert_eq!(plan.resets.len(), 1);
    let r = &plan.resets[0];
    assert_eq!(r.signal, "rst_n");
    assert!(r.active_low && r.r#async);
    assert_eq!(r.rule_ids.len(), 3, "guard×3(fsm_counter R001..R003)");
    // 模板硬面:SCENES 註釋/場景打印/DONE/WATCHDOG/xorshift(禁 $random)。
    let t = tb_template_v(&plan, &TbOpts::default());
    assert!(t.contains("// SCENES: 4"), "{}", &t[..200]);
    assert!(t.contains("$display(\"SCENE1 arst_rst_n\")"));
    assert!(t.contains("    $display(\"DONE\");"));
    assert!(t.contains("WATCHDOG"));
    assert!(t.contains("tb__xs32"));
    assert!(!t.contains("$random"));
    assert!(!t.contains("$urandom"));
    // 計劃 JSON:鍵序=寫出序首鍵 module。
    assert!(tb_plan_json(&plan).starts_with("{\"module\":\"fsm_counter\""));
}

#[test]
fn c_s2_sampling_six_designs_plan_structure() {
    // §19.7 採樣集 6 設計的 plan 結構斷言(結構面,非 golden)。
    // i2c_master:旗艦;guard 20 條→async 復位一組(rule_ids 20)。
    let p = plan_of_file("corpus/e2e/59_i2c_master/i2c_master.v", Some("i2c_master"));
    assert_eq!(p.module, "i2c_master");
    assert_eq!(p.obligations.len(), 152);
    assert_eq!(p.clocks.len(), 1);
    assert_eq!(p.clocks[0].signal, "clk");
    assert_eq!(p.resets.len(), 1);
    assert_eq!(p.resets[0].signal, "rst_n");
    assert!(p.resets[0].active_low && p.resets[0].r#async);
    assert_eq!(p.resets[0].rule_ids.len(), 20, "guard 20 條");
    assert!(
        p.sens_edges.contains(&("posedge".to_string(), "clk".to_string()))
            && p.sens_edges.contains(&("negedge".to_string(), "rst_n".to_string())),
        "sens_edges={:?}",
        p.sens_edges
    );
    assert_eq!(p.scenes[0].kind, "arst");
    assert_eq!(p.scenes[0].name, "arst_rst_n");

    // acc_pipe:無復位 3 級流水線→resets=∅、flush 場景打頭。
    let p = plan_of_file(
        "corpus/e2e/58_xls_acc_pipe/acc_pipe.v",
        Some("__acc_pipe__acc_pipe"),
    );
    assert_eq!(p.obligations.len(), 9);
    assert!(p.resets.is_empty(), "{:?}", p.resets);
    assert_eq!(p.scenes[0].kind, "flush");

    // fsm:同步復位不在頂層 If→零 guard→resets=∅(R1-11 口徑)。
    let p = plan_of_file("corpus-external/yosys-simple/fsm.v", Some("fsm_test"));
    assert_eq!(p.obligations.len(), 23);
    assert!(p.resets.is_empty());
    assert!(!p.scenes.iter().any(|s| s.kind == "rst" || s.kind == "arst"));

    // sincos:posedge 異步復位、高有效(reset 帶自有 posedge 沿)。
    let p = plan_of_file("corpus-external/yosys-simple/sincos.v", Some("d"));
    assert_eq!(p.clocks[0].signal, "clock");
    assert_eq!(p.resets.len(), 1);
    assert_eq!(p.resets[0].signal, "reset");
    assert!(!p.resets[0].active_low && p.resets[0].r#async);
    assert_eq!(p.scenes[0].name, "arst_reset");

    // signedexpr:純組合→零時鐘→exh(5 輸入位 ≤12)+small。
    let p = plan_of_file(
        "corpus-external/yosys-simple/signedexpr.v",
        Some("signed_test01"),
    );
    assert!(p.clocks.is_empty());
    assert_eq!(p.obligations.iter().filter(|o| o.form == "relation").count(), 6);
    let kinds: Vec<&str> = p.scenes.iter().map(|s| s.kind).collect();
    assert_eq!(kinds, vec!["exh", "small"], "{kinds:?}");

    // subbytes:negedge 異步復位、低有效;guard 3 條。
    let p = plan_of_file("corpus-external/yosys-simple/subbytes.v", Some("subbytes_00"));
    assert_eq!(p.obligations.len(), 16);
    assert_eq!(p.resets.len(), 1);
    assert_eq!(p.resets[0].signal, "reset");
    assert!(p.resets[0].active_low && p.resets[0].r#async);
    assert_eq!(p.resets[0].rule_ids.len(), 3);
}

#[test]
fn c_s2_tb1_hard_errors() {
    // ①解析錯誤(前端無 milestone → TB1)。
    let e = tb1_err(&sv("bad.v", "module m(input wire a;;\nendmodule\n"), None);
    assert!(e.starts_with("error: "), "{e}");
    assert!(e.contains("(milestone TB1)"), "{e}");
    // ②module 參數指定的模組不存在。
    let e = tb1_err(&sv("t.v", "module a(input wire c, output reg q);\nalways @(posedge c) q <= 1'b1;\nendmodule\n"), Some("zzz"));
    assert!(e.contains("module `zzz` not found") && e.contains("(milestone TB1)"), "{e}");
    // ③多模組未指定 module。
    let two = "module a(input wire c, output reg q);\nalways @(posedge c) q <= 1'b1;\nendmodule\nmodule b(input wire c, output reg r);\nalways @(posedge c) r <= 1'b0;\nendmodule\n";
    let e = tb1_err(&sv("two.v", two), None);
    assert!(e.contains("multi-module source") && e.contains("a, b"), "{e}");
    // ④parametric 端口(contract 無數值寬度)。
    let e = tb1_err(
        &sv("pm.v", "module m#(parameter W=8)(input [W-1:0] d, output reg q);\nassign q = d[0];\nendmodule\n"),
        None,
    );
    assert!(e.contains("parametric width"), "{e}");
    // ⑤inout 端口。
    let e = tb1_err(
        &sv("io.v", "module m(inout wire bus, input wire e, output reg y);\nassign bus = e;\nalways @* y = e;\nendmodule\n"),
        None,
    );
    assert!(e.contains("is inout"), "{e}");
    // ⑥零輸出端口。
    let e = tb1_err(&sv("noout.v", "module m(input wire a);\nendmodule\n"), None);
    assert!(e.contains("no output port"), "{e}");
    // ⑦多時鐘(兩個產規則沿行程的 trigger 訊號不同)。
    let e = tb1_err(
        &sv("2clk.v", "module m(input wire clk, clk2, input wire d, output reg q1, q2);\nalways @(posedge clk) q1 <= d;\nalways @(posedge clk2) q2 <= ~q1;\nendmodule\n"),
        None,
    );
    assert!(e.contains("multiple clock signals: clk, clk2"), "{e}");
    // ⑧時鐘不是輸入端口(內部生成)。
    let e = tb1_err(
        &sv("iclk.v", "module m(input wire d, output reg q);\nreg ck;\ninitial ck = 1'b0;\nalways #5 ck = ~ck;\nalways @(posedge ck) q <= d;\nendmodule\n"),
        None,
    );
    assert!(e.contains("clock signal `ck` is not an input port"), "{e}");
    // ⑪RTLIL 輸入(rtl_tb_plan_rtlil 面可達的路徑;核心恆 TB1)。
    let e = tb1_err(
        &SourceInput::Rtlil(RtlilInput {
            name: "x.il".to_string(),
            source: "module \\m \\n".to_string(),
        }),
        None,
    );
    assert!(e.contains("RTLIL input") && e.contains("(milestone TB1)"), "{e}");
    // ⑤孤兒端口(非 ANSI 無宣告:兩腿皆無方向證據→Inout+width 0 佔位)。
    let e = tb1_err(&sv("orphan.v", "module m(a, q);\noutput q;\nendmodule\n"), None);
    assert!(e.contains("port `a` is inout"), "{e}");
    // ⑩規則書截斷(>DEFAULT_MAX_RULES=256 條;計劃禁靜默殘缺)。
    let mut big = String::from(
        "module big(input wire clk, input wire [299:0] d, output reg [299:0] q);\nalways @(posedge clk) begin\n",
    );
    for k in 0..300 {
        big.push_str(&format!("  q[{k}] <= d[{k}];\n"));
    }
    big.push_str("end\nendmodule\n");
    let e = tb1_err(&sv("big.v", &big), None);
    assert!(e.contains("rulebook truncated for big") && e.contains("(milestone TB1)"), "{e}");
    // ⑨(同訊號兼時鐘與復位)=防禦性硬錯,公共路徑不可達:guard 構造上 trigger≠
    // 復位訊號(C-S1 select_trigger),另一行程以該訊號為時鐘則先觸發 ⑦(多時鐘)。
    // 不造假負樣;登記於 RTL2SPEC-PLUGIN §19.19。
}

#[test]
fn c_s2_frame_error_is_data_not_trap() {
    // wasm 面契約:錯誤走資料 {"ok":false,"error":"…"}。
    let frame = tb_plan_frame(
        &sv("t.v", "module m(input wire a);\nendmodule\n"),
        None,
        0,
        0,
        0,
    );
    assert!(frame.starts_with("{\"ok\":false,\"error\":\"error: "), "{frame}");
    // 0=核心默認(seed/cycles/flush 顯式 0 → TbOpts::resolve)。
    let src = repo_file("plugin/rtl2spec/fixtures/fsm_counter.v");
    let plan = tb_plan(&sv("fsm_counter.v", &src), None).unwrap();
    let a = tb_plan_frame(&sv("fsm_counter.v", &src), None, 0, 0, 0);
    let b = tb_plan_frame(
        &sv("fsm_counter.v", &src),
        None,
        DEFAULT_SEED,
        DEFAULT_CYCLES,
        DEFAULT_FLUSH,
    );
    let _ = plan;
    assert_eq!(a, b, "seed/cycles/flush=0 與顯式默認值幀逐位元組相等")
}

#[test]
fn c_s2_determinism_12_runs() {
    // 12 跑確定性(進程內;§16.1.5 契約①同形):plan JSON+模板逐位元組相等。
    for (rel, module) in [
        ("plugin/rtl2spec/fixtures/fsm_counter.v", None),
        ("corpus/e2e/59_i2c_master/i2c_master.v", Some("i2c_master")),
        ("corpus/e2e/58_xls_acc_pipe/acc_pipe.v", Some("__acc_pipe__acc_pipe")),
        ("corpus-external/yosys-simple/fsm.v", Some("fsm_test")),
        ("corpus-external/yosys-simple/sincos.v", Some("d")),
        ("corpus-external/yosys-simple/signedexpr.v", Some("signed_test01")),
        ("corpus-external/yosys-simple/subbytes.v", Some("subbytes_00")),
    ] {
        let src = repo_file(rel);
        let first = tb_plan_frame(&sv(rel, &src), module, 0, 0, 0);
        assert!(first.starts_with("{\"ok\":true,"), "{rel}: {}", &first[..first.len().min(160)]);
        for k in 0..12 {
            let again = tb_plan_frame(&sv(rel, &src), module, 0, 0, 0);
            assert_eq!(first, again, "{rel} 第 {k} 跑抖動");
        }
    }
}

#[test]
fn c_s2_opts_zero_means_default() {
    // TbOpts::resolve:0=核心默認(SSOT=tbplan.rs 常量)。
    let o = TbOpts::resolve(0, 0, 0);
    assert_eq!((o.seed, o.cycles, o.flush), (305_419_896, 32, 8));
    let o = TbOpts::resolve(7, 5, 3);
    assert_eq!((o.seed, o.cycles, o.flush), (7, 5, 3));
}

/// 模板文本探針:由內聯 SV 產模板(錯誤=測試 bug,不屬被測)。
fn template_of(src: &str) -> (rtl_spec::TbPlan, String) {
    let plan = tb_plan(&sv("t.v", src), None).unwrap_or_else(|e| panic!("{e}"));
    let t = tb_template_v(&plan, &TbOpts::default());
    (plan, t)
}

#[test]
fn c_s2_template_init_before_first_scene_and_timescale() {
    // 回歸錨(subbytes x=308 實錄):輸入未初始化=組合輸出 X→Q4 紅。
    // 復位置無效電平、數據輸入置 0,且都在 #1 之後(t=0 同刻=非確定序)。
    let (_, t) = template_of(
        "module m(input wire clk, input wire rst_n, input wire [7:0] d, output reg [7:0] q);\n\
         always @(posedge clk or negedge rst_n) if (!rst_n) q <= 8'd0; else q <= d;\nendmodule\n",
    );
    let at = |needle: &str| t.find(needle).unwrap_or_else(|| panic!("缺 {needle}\n{t}"));
    assert!(at("`timescale 1ns/1ps") < at("module tb_m;"));
    let delay = at("    tb__rnd = 32'd305419896;\n    #1;\n");
    let rst_init = at("    rst_n = 1'b1;\n");
    let data_init = at("    d = {8{1'b0}};\n");
    let first = at("$display(\"SCENE1 ");
    assert!(delay < rst_init && rst_init < data_init && data_init < first, "{t}");
}

#[test]
fn c_s2_template_clock_polarity_and_dual_edge() {
    // 正沿 DUT:驅動在負沿,無 #1。
    let (_, t) = template_of(
        "module m(input wire clk, input wire d, output reg q);\nalways @(posedge clk) q <= d;\nendmodule\n",
    );
    assert!(t.contains("@(negedge clk);\n"), "{t}");
    assert!(!t.contains("@(posedge clk)"), "{t}");
    // 負沿 DUT:驅動換到正沿(同沿=TB-R1x 違例)。
    let (p, t) = template_of(
        "module m(input wire clk, input wire d, output reg q);\nalways @(negedge clk) q <= d;\nendmodule\n",
    );
    assert_eq!(p.clocks[0].edge, "negedge");
    assert!(t.contains("@(posedge clk);\n"), "{t}");
    assert!(!t.contains("@(negedge clk)"), "{t}");
    // 兩極性都採樣:無安全沿→同行補 #1 關窗。
    let (p, t) = template_of(
        "module m(input wire clk, input wire d, output reg q, output reg r);\nalways @(posedge clk) q <= d;\nalways @(negedge clk) r <= d;\nendmodule\n",
    );
    assert_eq!(p.clocks.len(), 1, "同訊號不算多時鐘");
    assert!(t.contains("@(negedge clk); #1;\n"), "{t}");
}

#[test]
fn c_s2_template_drive_is_width_exact() {
    // Q7:tb.v 位置的 verilator 警告必為 0——禁隱式截斷(`(rnd >> s) & mask` 形)。
    let (_, t) = template_of(
        "module m(input wire clk, input wire a, input wire [7:0] b, input wire [39:0] c, input wire [31:0] d, output reg q);\n\
         always @(posedge clk) q <= a ^ b[0] ^ c[0] ^ d[0];\nendmodule\n",
    );
    assert!(!t.contains(">>  ") && !t.contains("& 32'h"), "{t}");
    assert!(t.contains("      a = tb__rnd[3];\n"), "{t}");
    assert!(t.contains("      b = tb__rnd[17:10];\n"), "{t}");
    // 40 位=1 份 32 位+低 8 位切片,拼接寬度恰 40。
    assert!(t.contains("      c = {tb__rnd[7:0], {1{tb__rnd}}};\n"), "{t}");
    // 32 位:切片起點鉗到 0。
    assert!(t.contains("      d = tb__rnd[31:0];\n"), "{t}");
    // small:高位補 0,寬度恰 8/40/32。
    assert!(t.contains("      b = {{5{1'b0}}, tb__rnd[12:10]};\n"), "{t}");
    assert!(t.contains("      c = {{37{1'b0}}, tb__rnd[19:17]};\n"), "{t}");
    assert!(t.contains("      d = {{29{1'b0}}, tb__rnd[2:0]};\n"), "{t}");
}

#[test]
fn c_s2_template_covers_grammar_and_internal_names() {
    // flush 非定向:COVERS 文法不收空表(協議 v1)。
    let (_, t) = template_of(
        "module m(input wire clk, input wire d, output reg q);\nalways @(posedge clk) q <= d;\nendmodule\n",
    );
    assert!(t.contains("// COVERS: 1: *\n"), "{t}");
    assert!(!t.lines().any(|l| l.starts_with("// COVERS:") && l.trim_end().ends_with(':')), "{t}");
    // 端口名與 TB 內部名撞名防線:內部一律 tb__ 前綴。
    let (_, t) = template_of(
        "module m(input wire clk, input wire hi, input wire rnd, input wire x, output reg dut);\nalways @(posedge clk) dut <= hi ^ rnd ^ x;\nendmodule\n",
    );
    assert!(t.contains("  reg hi;\n") && t.contains("  reg rnd;\n") && t.contains("  wire dut;\n"), "{t}");
    assert!(t.contains("  m tb__dut (\n"), "{t}");
    // 場景名文法 [a-z0-9_]+:大寫復位訊號小寫化。
    let (p, _) = template_of(
        "module m(input wire clk, input wire RST_N, input wire d, output reg q);\nalways @(posedge clk or negedge RST_N) if (!RST_N) q <= 1'b0; else q <= d;\nendmodule\n",
    );
    assert_eq!(p.scenes[0].name, "arst_rst_n");
    assert_eq!(p.resets[0].signal, "RST_N");
}

#[test]
fn c_s2_obligation_observe_and_x_waiver() {
    // 主體取基名:輸出端口部分選擇=port;內部訊號=indirect(R1-9)。
    let (p, t) = template_of(
        "module m(input wire clk, input wire d, output reg [3:0] q);\nreg r;\n\
         always @(posedge clk) begin\n  q[0] <= d;\n  r <= 1'bx;\nend\nendmodule\n",
    );
    let of = |subj: &str| {
        let rid = p
            .obligations
            .iter()
            .zip(tb_rule_subjects(&p))
            .find(|(_, s)| s == subj)
            .map(|(o, _)| o)
            .unwrap_or_else(|| panic!("無主體 {subj}: {:?}", p.obligations));
        rid.observe
    };
    assert_eq!(of("q['sd0]"), "port"); // 主體=渲染文本,部分選擇取基名 q
    assert_eq!(of("r"), "indirect");
    // x 位字面量值=x-valued 豁免,模板同步 WAIVE 註釋。
    assert_eq!(p.waivers.len(), 1, "{:?}", p.waivers);
    assert_eq!(p.waivers[0].reason, "x-valued");
    assert!(t.contains(&format!("// WAIVE: {} x-valued\n", p.waivers[0].rule_id)), "{t}");
}

/// 規則序主體表(obligations 與規則一一同序)。
fn tb_rule_subjects(p: &rtl_spec::TbPlan) -> Vec<String> {
    let src = "module m(input wire clk, input wire d, output reg [3:0] q);\nreg r;\n\
         always @(posedge clk) begin\n  q[0] <= d;\n  r <= 1'bx;\nend\nendmodule\n";
    let rb = rtl_spec::extract_rules(&sv("t.v", src), rtl_spec::DEFAULT_MAX_RULES);
    let subjects: Vec<String> = rb.modules[0].rules.iter().map(|r| r.check.subject()).collect();
    assert_eq!(subjects.len(), p.obligations.len());
    subjects
}
