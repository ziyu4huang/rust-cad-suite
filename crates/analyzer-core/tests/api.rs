//! 統一 API 測試(ANALYZER-SERVICE §8.1 api.rs 行):SV/RTLIL 正例投影
//! 逐項斷言(含 ANSI 繼承腿 + 非 ANSI 回填腿,R2-1)、SV 語法錯累積、
//! RTLIL 首錯硬斷、parametric 寬度標記、行列查詢(include 多文件,R2-3)、
//! 實例邊兩輸入形。測試源全部內嵌字符串(零新語料文件)。

use analyzer_core::view::Dir;
use analyzer_core::{
    analyze, SourceInput, RtlilInput, SvInput,
};

fn sv(name: &str, source: &str) -> SourceInput {
    SourceInput::Sv(SvInput {
        name: name.to_string(),
        source: source.to_string(),
        ..SvInput::default()
    })
}

fn il(name: &str, source: &str) -> SourceInput {
    SourceInput::Rtlil(RtlilInput {
        name: name.to_string(),
        source: source.to_string(),
    })
}

/// S1 正例 1:ANSI 全特徵(端口組繼承/參數/記憶體/閘原語/模組自例化/過程)。
#[test]
fn sv_ansi_full_feature() {
    let src = "\
module counter #(parameter WIDTH = 8'h08, parameter signed [3:0] BASE = 4'sd5) (
    input clk, input rst_n,
    output reg [7:0] q,
    inout io,
    input signed [3:0] s
);
    wire [3:0] a, b;
    reg [7:0] mem [0:255];
    assign a = b;
    always @(posedge clk) q <= q + 1'b1;
    initial $display(\"hi\");
    and g1 (o, i1, i2);
    counter u_next (.clk(clk), .rst_n(rst_n));
endmodule
";
    let a = analyze(&sv("counter.v", src));
    assert!(!a.has_errors(), "diag_text={}", a.diag_text);
    assert_eq!(a.kind, analyzer_core::InputKind::Sv);
    assert_eq!(a.modules.len(), 1);
    let m = a.module("counter").unwrap();

    // 參數:header 字面量形(§4.5)。
    assert_eq!(m.params.len(), 2);
    assert_eq!(m.params[0].name, "WIDTH");
    assert_eq!(m.params[0].value, "8'h08");
    assert_eq!(m.params[1].name, "BASE");
    assert_eq!(m.params[1].value, "4'sd5");

    // 端口:ANSI 腿(parser 新組重置 net/reg/signed/range、僅方向跨組繼承;
    // 投影端攜帶保底;D-c2 語義,oracle 見 RTL2SPEC-PLUGIN §19.3.2)。
    assert_eq!(m.ports.len(), 5);
    let p = |n: &str| m.ports.iter().find(|p| p.name == n).unwrap();
    assert_eq!(p("clk").dir, Dir::Input);
    assert_eq!(p("clk").width, 1);
    assert!(!p("clk").parametric);
    assert_eq!(p("rst_n").dir, Dir::Input);
    assert_eq!(p("q").dir, Dir::Output);
    assert_eq!(p("q").width, 8);
    assert_eq!(p("io").dir, Dir::Inout);
    let ps = p("s");
    assert_eq!(ps.dir, Dir::Input);
    assert_eq!(ps.width, 4);
    assert!(ps.signed);
    // 端口 span 有值(SV 側)。
    assert!(p("clk").span.is_some());

    // nets:SV 側=體內 Decl 面(§4.2 字面義)——ANSI 頭端口聲明不是體內
    // Decl,不進 nets(端口面已載 clk/rst_n/q/io/s);體內 a/b/mem 三條。
    assert_eq!(m.nets.len(), 3, "nets={:?}", m.nets);
    let n = |name: &str| m.nets.iter().find(|n| n.name == name).unwrap();
    assert_eq!(n("a").kind.as_str(), "wire");
    assert_eq!(n("a").width, 4);
    assert!(!n("a").is_port);
    assert_eq!(n("b").width, 4);
    assert_eq!(n("mem").kind.as_str(), "reg");
    assert_eq!(n("mem").width, 8);
    assert_eq!(n("mem").unpacked_dims, 1);
    assert!(m.nets.iter().all(|n| n.name != "q" && n.name != "io"));

    // cells:閘原語與模組例化同形;一 InstUnit 一條。
    assert_eq!(m.cells.len(), 2);
    let g = m.cells.iter().find(|c| c.name == "g1").unwrap();
    assert_eq!(g.ctype, "and");
    assert_eq!(g.port_count, 3);
    let u = m.cells.iter().find(|c| c.name == "u_next").unwrap();
    assert_eq!(u.ctype, "counter");
    assert_eq!(u.port_count, 2);

    // 過程/連續賦值計數 + 模組 span。
    assert_eq!(m.proc_count, 2);
    assert_eq!(m.assign_count, 1);
    let sp = m.span.as_ref().unwrap();
    assert_eq!(sp.start_line, 1);
    assert_eq!(sp.file, "counter.v");

    // 實例邊:counter→counter 命中(閘原語 and 不成邊)。
    assert_eq!(a.instance_edges(), vec![("counter".to_string(), "counter".to_string())]);
    // 原生逃逸口 + 行列查詢。
    assert!(a.sv.is_some());
    assert_eq!(a.line_col(analyzer_core::rcs_diag::FileId(0), 0).unwrap(), (1, 1));
}

/// S1 正例 2:ANSI 繼承腿(D-c2 訂正 2026-11-07:方向唯一跨組繼承,新組
/// net/reg/signed/range 重置——oracle=iverilog 13.0(-g2005/-g2012)與
/// verilator 5.052 對 `module m(input [3:0] a, b, c, output d)` 三方皆
/// d=1(§23.2.2.3 同向;RTL2SPEC-PLUGIN §19.3.2 七向量 v1)。舊期望
/// d=4 是把缺陷語義釘成正確,由 ADR-0013 D-C13 顯式修訂 ADR-0012 D1。
#[test]
fn sv_ansi_inheritance_leg() {
    let src = "module m(input [3:0] a, b, c, output d);\nendmodule\n";
    let a = analyze(&sv("inherit.v", src));
    assert!(!a.has_errors(), "diag_text={}", a.diag_text);
    let m = a.module("m").unwrap();
    let port = |n: &str| m.ports.iter().find(|p| p.name == n).unwrap();
    for n in ["a", "b", "c"] {
        assert_eq!(port(n).dir, Dir::Input, "port {n}");
        assert_eq!(port(n).width, 4, "port {n}");
        assert!(!port(n).parametric, "port {n}");
    }
    assert_eq!(port("d").dir, Dir::Output);
    assert_eq!(port("d").width, 1);
}

/// S1 正例 3:非 ANSI 回填腿(探針實證源;R2-1)。Port.dir 恆 None,
/// 方向在體內 Decl 回填。
#[test]
fn sv_non_ansi_backfill_leg() {
    let src = "module top(a, b, c);\ninput a, b;\noutput reg [3:0] c;\nwire [7:0] w;\nassign w = 8'h00;\nendmodule\n";
    let a = analyze(&sv("nonansi.v", src));
    assert!(!a.has_errors(), "diag_text={}", a.diag_text);
    let m = a.module("top").unwrap();
    let port = |n: &str| m.ports.iter().find(|p| p.name == n).unwrap();
    assert_eq!(port("a").dir, Dir::Input);
    assert_eq!(port("a").width, 1);
    assert_eq!(port("b").dir, Dir::Input);
    assert_eq!(port("c").dir, Dir::Output);
    assert_eq!(port("c").width, 4);
    // 體內 `output reg` 的 reg 面進 nets(kind="reg")。
    let nc = m.nets.iter().find(|n| n.name == "c").unwrap();
    assert_eq!(nc.kind.as_str(), "reg");
    assert!(nc.is_port);
    let na = m.nets.iter().find(|n| n.name == "a").unwrap();
    assert_eq!(na.kind.as_str(), "wire");
    assert!(na.is_port);
    let nw = m.nets.iter().find(|n| n.name == "w").unwrap();
    assert_eq!(nw.width, 8);
    assert!(!nw.is_port);
    assert!(a.instance_edges().is_empty());
}

/// S1 正例 4:generate 內實例與聲明扁平遞入(實例邊命中模組名集合)。
#[test]
fn sv_generate_instances_and_edges() {
    let src = "\
module g;\n\
genvar i;\n\
generate\n\
  for (i = 0; i < 2; i = i + 1) begin : blk\n\
    wire gw;\n\
    sub u_sub (.x());\n\
  end\n\
endgenerate\n\
endmodule\n\
module sub(x);\ninput x;\nendmodule\n";
    let a = analyze(&sv("gen.v", src));
    assert!(!a.has_errors(), "diag_text={}", a.diag_text);
    assert_eq!(a.modules.len(), 2);
    let g = a.module("g").unwrap();
    assert_eq!(g.cells.len(), 1);
    assert_eq!(g.cells[0].name, "u_sub");
    assert_eq!(g.cells[0].ctype, "sub");
    assert_eq!(g.cells[0].port_count, 1);
    assert!(g.nets.iter().any(|n| n.name == "gw"), "generate 內聲明進 nets");
    // sub → g(sub 不例化任何模組):僅一條邊 (g, sub)。
    assert_eq!(
        a.instance_edges(),
        vec![("g".to_string(), "sub".to_string())]
    );
}

/// S1 正例 5:參數字面量形 + 非字面量佔位 + parametric 寬度標記;
/// localparam 不進 params(本弧裁定,對齊 RTLIL 介面參數語義)。
#[test]
fn sv_params_parametric_and_localparam() {
    let src = "\
module p #(parameter W = 4'd12) (input [W-1:0] x, input [7:0] y);\n\
localparam L = 2;\n\
parameter P2 = \"strv\";\n\
parameter P3 = W + 1;\n\
endmodule\n";
    let a = analyze(&sv("params.v", src));
    assert!(!a.has_errors(), "diag_text={}", a.diag_text);
    let m = a.module("p").unwrap();
    assert_eq!(m.params.len(), 3, "W + P2 + P3;localparam L 不收");
    assert_eq!(m.params[0].name, "W");
    assert_eq!(m.params[0].value, "4'd12");
    assert_eq!(m.params[1].name, "P2");
    assert_eq!(m.params[1].value, "\"strv\"");
    assert_eq!(m.params[2].name, "P3");
    assert_eq!(m.params[2].value, "<expr>", "非字面量 init → 佔位");
    let px = m.ports.iter().find(|p| p.name == "x").unwrap();
    assert_eq!(px.width, 0);
    assert!(px.parametric, "[W-1:0] 參數化寬度 → 標記");
    let py = m.ports.iter().find(|p| p.name == "y").unwrap();
    assert_eq!(py.width, 8);
    assert!(!py.parametric);
}

/// SV 語法錯 → 累積式多診斷 + 部分模組(§4.3 不對稱的 SV 側)。
#[test]
fn sv_syntax_error_accumulates() {
    let src = "\
module ok(input a);\nassign a = 1'b0;\nendmodule\n\
module bad(input a);\nwire ;\nassign = 1'b1;\nendmodule\n";
    let a = analyze(&sv("broken.v", src));
    assert!(a.has_errors());
    assert!(a.diagnostics().len() >= 2, "expected multiple diags, got {}", a.diagnostics().len());
    assert!(a.diag_text.contains("error"), "diag_text={}", a.diag_text);
    // 部分模組仍在:ok 完整投影。
    assert!(a.module("ok").is_some());
    let ok = a.module("ok").unwrap();
    assert_eq!(ok.ports.len(), 1);
    assert_eq!(ok.assign_count, 1);
}

/// RTLIL 正例 1:端口序(port_id)/nets 全量/cells/實例邊/原生逃逸口。
#[test]
fn rtlil_basic_projection() {
    let src = "\
autoidx 2\n\
module \\leaf\n\
  wire input 1 width 4 \\li\n\
  wire output 2 width 4 \\lo\n\
  connect \\lo \\li\n\
end\n\
module \\top\n\
  parameter \\W 4'1000\n\
  wire input 1 width 8 \\x\n\
  wire output 2 width 4 \\y\n\
  wire width 8 \\t\n\
  connect \\t \\x\n\
  cell \\leaf \\u1\n\
    connect \\A \\x\n\
  end\n\
  cell $mux \\m1\n\
    connect \\A \\x\n\
  end\n\
end\n";
    let a = analyze(&il("top.il", src));
    assert!(!a.has_errors(), "diag_text={}", a.diag_text);
    assert_eq!(a.kind, analyzer_core::InputKind::Rtlil);
    assert_eq!(a.modules.len(), 2);
    let top = a.module("\\top").unwrap();

    // 端口按 port_id 升序;dir 由兩 bool 派生。
    assert_eq!(
        top.ports.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        vec!["\\x", "\\y"]
    );
    assert_eq!(top.ports[0].dir, Dir::Input);
    assert_eq!(top.ports[0].width, 8);
    assert_eq!(top.ports[1].dir, Dir::Output);
    assert_eq!(top.ports[1].width, 4);
    assert!(top.ports.iter().all(|p| p.span.is_none()));

    // nets:wires 全量含端口。
    assert_eq!(top.nets.len(), 3);
    assert!(top.nets.iter().all(|n| n.kind.as_str() == "wire"));
    assert_eq!(
        top.nets.iter().find(|n| n.name == "\\t").unwrap().is_port,
        false
    );
    assert_eq!(
        top.nets.iter().find(|n| n.name == "\\x").unwrap().is_port,
        true
    );

    // cells + 參數(4'1000 → 4'h8)。
    assert_eq!(top.cells.len(), 2);
    let u1 = top.cells.iter().find(|c| c.name == "\\u1").unwrap();
    assert_eq!(u1.ctype, "\\leaf");
    assert_eq!(u1.port_count, 1);
    let w = top.params.iter().find(|p| p.name == "\\W").unwrap();
    assert_eq!(w.value, "4'h8");

    assert_eq!(top.proc_count, 0);
    assert_eq!(top.assign_count, 1);
    assert!(top.span.is_none());

    // 實例邊:\\leaf 命中,\\$mux 不命中。名字逐字保留(帶 \\ 前綴)。
    assert_eq!(
        a.instance_edges(),
        vec![("\\top".to_string(), "\\leaf".to_string())]
    );
    // RTLIL 側無行列查詢(§4.2)。
    assert!(a.line_col(analyzer_core::rcs_diag::FileId(0), 0).is_err());
    assert!(a.sv.is_none());
    assert!(a.rtlil.is_some());
}

/// RTLIL 正例 2:memory 單列 nets(kind="memory")+ 無預設值參數空串 +
/// 零診斷時 diag_text 空串。
#[test]
fn rtlil_memory_and_params() {
    let src = "\
module \\pr\n\
  parameter \\W 16'0011111111111111\n\
  parameter \\NODEF\n\
  wire width 1 \\z\n\
  memory width 8 size 16 \\mem\n\
  connect \\z \\z\n\
end\n";
    let a = analyze(&il("pr.il", src));
    assert!(!a.has_errors());
    assert!(a.diagnostics().is_empty());
    assert_eq!(a.diag_text, "");
    let m = a.module("\\pr").unwrap();
    let w = m.params.iter().find(|p| p.name == "\\W").unwrap();
    assert_eq!(w.value, "16'h3fff");
    let nd = m.params.iter().find(|p| p.name == "\\NODEF").unwrap();
    assert_eq!(nd.value, "", "無預設值參數 → 空串");
    let mem = m.nets.iter().find(|n| n.name == "\\mem").unwrap();
    assert_eq!(mem.kind.as_str(), "memory");
    assert_eq!(mem.width, 8);
    assert!(!mem.is_port);
    assert_eq!(mem.unpacked_dims, 0);
}

/// RTLIL 壞 .il → 首錯硬斷:單條 AN-E001 + 零模組(§4.3 不對稱 RTLIL 側)。
#[test]
fn rtlil_bad_source_single_error() {
    let src = "module \\bad\n  frobnicate \\w\nend\n";
    let a = analyze(&il("bad.il", src));
    assert!(a.has_errors());
    assert_eq!(a.diagnostics().len(), 1, "首錯硬斷=單診斷");
    let d = &a.diagnostics()[0];
    assert_eq!(d.code, "AN-E001");
    assert_eq!(d.severity, analyzer_core::rcs_diag::Severity::Error);
    // message 帶 ry-ir 原文。
    assert!(d.message.contains("Parser error in line 2"), "msg={}", d.message);
    assert!(a.modules.is_empty());
    assert!(a.rtlil.is_none());
    assert!(!a.diag_text.is_empty());
    // span 指到錯誤行(第 2 行)。
    assert_eq!(d.span.start.0, "module \\bad\n".len() as u32);
}

/// 不支持構造(RyError::Unsupported)也走 AN-E001 單診斷(milestone 指針保留)。
#[test]
fn rtlil_unsupported_keeps_milestone_pointer() {
    let src = "module \\u\n  wire \\w\nend\n";
    // twines 區塊 = milestone Y1(ry-ir M0 邊界)。
    let src = format!("{src}twines \\t\nend\n");
    let a = analyze(&il("u.il", &src));
    assert!(a.has_errors());
    assert_eq!(a.diagnostics().len(), 1);
    assert!(a.diagnostics()[0].message.contains("milestone"), "msg={}", a.diagnostics()[0].message);
}

/// 行列查詢 include 多文件語義(R2-3):pp 向同一 SourceMap add 多文件,
/// (FileId, pos) 雙參直通既有 pub 面。
#[test]
fn line_col_include_multi_file() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let dir = std::env::temp_dir().join(format!("analyzer-core-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let hdr = dir.join("hdr.vh");
    std::fs::write(&hdr, "// header line one\nwire hh;\n").unwrap();

    let src = "module inc();\n`include \"hdr.vh\"\nassign hh = 1'b0;\nendmodule\n";
    let input = SourceInput::Sv(SvInput {
        name: "main.v".into(),
        source: src.into(),
        include_dirs: vec![dir.clone()],
        defines: vec![],
    });
    let a = analyze(&input);
    assert!(!a.has_errors(), "diag_text={}", a.diag_text);

    // 主文件 = FileId(0);pp include 向同一 sm add 標頭 → FileId(1)。
    assert_eq!(a.line_col(analyzer_core::rcs_diag::FileId(0), 0).unwrap(), (1, 1));
    let line2 = "module inc();\n".len() as u32;
    assert_eq!(a.line_col(analyzer_core::rcs_diag::FileId(0), line2).unwrap(), (2, 1));
    // 標頭自己的行表(證明 file 維度生效,不是拿主文件行表)。
    assert_eq!(a.line_col(analyzer_core::rcs_diag::FileId(1), 0).unwrap(), (1, 1));
    let hdr_line2 = "// header line one\n".len() as u32;
    assert_eq!(a.line_col(analyzer_core::rcs_diag::FileId(1), hdr_line2).unwrap(), (2, 1));
    // 未註冊文件 → Err。
    assert!(a.line_col(analyzer_core::rcs_diag::FileId(9), 0).is_err());

    let _ = std::fs::remove_dir_all(&dir);
}

/// 等價字面量投影一致(收平輪 ① 回歸):based 字面量 digits 保留源碼
/// 下劃線(lexer.rs U-3b parity:「every value parser filters '_' itself」),
/// lit_u64 求值前剝除——`8'h0_F` 與 `8'h0F` 必須同投影;修復前前者
/// from_str_radix Err → width=0+parametric=true。x/z/? 剝後仍非數值
/// → parametric 路徑零誤傷。
#[test]
fn based_literal_underscore_equivalence() {
    let port_width = |rng: &str| {
        let src = format!("module t(input [{rng}:0] x);\nendmodule\n");
        let a = analyze(&sv("u.v", &src));
        assert!(!a.has_errors(), "diag_text={}", a.diag_text);
        let m = a.module("t").unwrap();
        (m.ports[0].width, m.ports[0].parametric)
    };
    // 等價形逐對:帶下劃線 vs 無下劃線,四基全覆蓋。
    for (us, plain) in [
        ("8'h0_F", "8'h0F"),
        ("8'd1_2", "8'd12"),
        ("4'o1_7", "4'o17"),
        ("16'b1010_1010_1010_1010", "16'b1010101010101010"),
    ] {
        assert_eq!(port_width(us), port_width(plain), "{us} vs {plain}");
        let (w, par) = port_width(us);
        assert!(w > 0 && !par, "{us}: width={w} parametric={par}");
    }
    // x/z/? 非數值字符:剝下劃線不誤傷 → 仍 parametric。
    assert_eq!(port_width("8'hx_X"), (0, true));
    assert_eq!(port_width("4'bz_Z?"), (0, true));
    // 全下劃線(剝後空)→ parametric,不 panic。
    assert_eq!(port_width("8'h_"), (0, true));
}

/// module() 查找 + 空 RTLIL 輸入(零模組、零診斷)。
#[test]
fn empty_and_lookup() {
    let a = analyze(&il("empty.il", ""));
    assert!(!a.has_errors());
    assert!(a.modules.is_empty());
    assert!(a.module("x").is_none());
    let b = analyze(&sv("empty.v", ""));
    assert_eq!(b.modules.len(), 0);
}
