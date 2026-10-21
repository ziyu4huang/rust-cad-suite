//! Frontend integration tests: pp → lex → parse chain (M0-M2 acceptance).

use rcs_diag::{DiagSink, SourceMap};
use vl_syntax::ast::{self, CaseKind, CaseItem, DeclKind, EdgeKind, Expr, ModuleItem, Stmt};
use vl_syntax::parser;
use vl_syntax::pp::{self, PpOptions};
use vl_syntax::token::Tok;

fn frontend(src: &str) -> (ast::SourceFile, DiagSink, SourceMap) {
    let mut sm = SourceMap::new();
    let mut diags = DiagSink::new();
    let out = pp::preprocess_source("t.v", src, &mut sm, &mut diags, &PpOptions::default());
    let f = parser::parse_source_file(out.tokens, &mut diags);
    (f, diags, sm)
}

fn parse_ok(src: &str) -> ast::SourceFile {
    let (f, diags, _sm) = frontend(src);
    assert!(
        !diags.has_errors(),
        "unexpected errors: {:#?}",
        diags.diags
    );
    f
}

#[test]
fn counter8_ansi_params() {
    let f = parse_ok(
        "`timescale 1ns/1ps\nmodule counter #(parameter WIDTH = 8)(\n\
         input  wire             clk,\n\
         input  wire             rst,\n\
         output reg  [WIDTH-1:0] q\n\
         );\n\
         always @(posedge clk) begin\n\
         if (rst) q <= {WIDTH{1'b0}};\n\
         else     q <= q + 1'b1;\n\
         end\n\
         endmodule\n",
    );
    assert_eq!(f.modules.len(), 1);
    let m = &f.modules[0];
    assert_eq!(m.name, "counter");
    assert!(m.ansi);
    assert_eq!(m.params.len(), 1);
    assert_eq!(m.params[0].name, "WIDTH");
    assert_eq!(m.ports.len(), 3);
    let q = &m.ports[2];
    assert_eq!(q.name, "q");
    assert!(q.is_reg);
    assert!(q.range.is_some());
    assert_eq!(m.items.len(), 1);
    match &m.items[0] {
        ModuleItem::Always { ctrl, body, .. } => {
            let ctrl = ctrl.as_ref().unwrap();
            assert_eq!(ctrl.edges.len(), 1);
            assert_eq!(ctrl.edges[0].kind, EdgeKind::Posedge);
            match body {
                Stmt::Block { body, .. } => {
                    assert_eq!(body.len(), 1);
                    match &body[0] {
                        Stmt::If { cond, then, els, .. } => {
                            assert!(matches!(cond, Expr::Ident { name, .. } if name == "rst"));
                            match then.as_ref() {
                                Stmt::NbAssign { lhs, rhs, .. } => {
                                    assert!(matches!(lhs, Expr::Ident { name, .. } if name == "q"));
                                    assert!(matches!(rhs, Expr::Repeat { .. }));
                                }
                                s => panic!("expected NbAssign, got {s:?}"),
                            }
                            assert!(els.is_some());
                        }
                        s => panic!("expected If, got {s:?}"),
                    }
                }
                s => panic!("expected Block, got {s:?}"),
            }
        }
        it => panic!("expected Always, got {it:?}"),
    }
}

#[test]
fn non_ansi_ports() {
    let f = parse_ok(
        "module m(a, b, c);\n input a;\n output [7:0] b;\n inout c;\n\
         wire [7:0] w;\n assign b = w;\nendmodule\n",
    );
    let m = &f.modules[0];
    assert!(!m.ansi);
    assert_eq!(m.ports.len(), 3);
    assert!(m.ports.iter().all(|p| p.dir.is_none()));
    let dirs: Vec<_> = m
        .items
        .iter()
        .filter_map(|it| match it {
            ModuleItem::Decl(d) => d.dir,
            _ => None,
        })
        .collect();
    assert_eq!(dirs.len(), 3);
    assert_eq!(m.items.len(), 5); // 3 dir decls + wire decl + 1 assign
}

#[test]
fn nba_vs_le_disambiguation() {
    let f = parse_ok(
        "module m(input clk);\n reg a, b;\n\
         always @(posedge clk) a <= b;\n\
         always @(posedge clk) if (a <= b) a = 1'b1;\n\
         endmodule\n",
    );
    let m = &f.modules[0];
    for it in &m.items {
        if let ModuleItem::Always { body, .. } = it {
            match body {
                Stmt::NbAssign { .. } => {}
                Stmt::If { cond, then, .. } => {
                    assert!(matches!(
                        cond,
                        Expr::Binary { op: ast::BinOp::Le, .. }
                    ));
                    assert!(matches!(then.as_ref(), Stmt::BAssign { .. }));
                }
                s => panic!("unexpected stmt {s:?}"),
            }
        }
    }
}

#[test]
fn casez_multi_labels_and_default() {
    let f = parse_ok(
        "module m;\n reg [1:0] s; reg o;\n\
         always @* begin\n casez (s)\n 2'b1?: o = 1'b1;\n\
         2'b01, 2'b00: o = 1'b0;\n default: o = 1'bx;\n endcase\n end\nendmodule\n",
    );
    let m = &f.modules[0];
    let mut found = false;
    for it in &m.items {
        if let ModuleItem::Always { body, .. } = it {
            let Stmt::Block { body, .. } = body else { panic!("block") };
            let Stmt::Case { kind, items, .. } = &body[0] else { panic!("case") };
            assert_eq!(*kind, CaseKind::Casez);
            assert_eq!(items.len(), 3);
            match &items[0] {
                CaseItem::Items(labels, _) => {
                    assert_eq!(labels.len(), 1);
                    assert!(matches!(&labels[0], Expr::NumLit(n) if n.digits == "1?"));
                }
                _ => panic!("labels"),
            }
            match &items[1] {
                CaseItem::Items(labels, _) => assert_eq!(labels.len(), 2),
                _ => panic!("labels"),
            }
            assert!(matches!(&items[2], CaseItem::Default(_)));
            found = true;
        }
    }
    assert!(found);
}

#[test]
fn generate_for_and_if() {
    let f = parse_ok(
        "module m;\n genvar i;\n wire [3:0] w;\n generate\n\
         for (i = 0; i < 4; i = i + 1) begin : g\n assign w[i] = 1'b0;\n end\n\
         if (1) begin : h\n wire x;\n end else begin : k\n wire y;\n end\n\
         endgenerate\nendmodule\n",
    );
    let m = &f.modules[0];
    let mut gen = None;
    for it in &m.items {
        if let ModuleItem::GenerateRegion(items) = it {
            gen = Some(items);
        }
    }
    let items = gen.expect("generate region");
    assert_eq!(items.len(), 2);
    match &items[0] {
        ModuleItem::GenFor {
            var, block, items, ..
        } => {
            assert_eq!(var, "i");
            assert_eq!(block.as_deref(), Some("g"));
            assert_eq!(items.len(), 1);
        }
        it => panic!("expected GenFor, got {it:?}"),
    }
    match &items[1] {
        ModuleItem::GenIf { then, els, .. } => {
            assert_eq!(then.len(), 1);
            let els = els.as_ref().unwrap();
            assert_eq!(els.len(), 1);
        }
        it => panic!("expected GenIf, got {it:?}"),
    }
}

#[test]
fn old_style_function_and_call() {
    let f = parse_ok(
        "module m;\n function [7:0] double;\n input [3:0] v;\n\
         begin\n double = {v, 1'b0};\n end\n endfunction\n\
         reg [7:0] r;\n always @* r = double(4'd5);\nendmodule\n",
    );
    let m = &f.modules[0];
    let mut saw_fn = false;
    let mut saw_call = false;
    for it in &m.items {
        match it {
            ModuleItem::Function(func) => {
                assert_eq!(func.name, "double");
                assert!(func.ret_range.is_some());
                assert_eq!(func.decls.len(), 1);
                saw_fn = true;
            }
            ModuleItem::Always { body, .. } => {
                let Stmt::BAssign { rhs, .. } = body else { panic!("bassign") };
                assert!(matches!(rhs, Expr::Call { func, .. } if func == "double"));
                saw_call = true;
            }
            _ => {}
        }
    }
    assert!(saw_fn && saw_call);
}

#[test]
fn instances_named_positional_and_array() {
    let f = parse_ok(
        "module m;\n wire a, b, y;\n\
         submod #(.W(8), .H()) u1 (.x(a), .y(), z);\n\
         submod u2 [3:0] (a, b, y);\nendmodule\n",
    );
    let m = &f.modules[0];
    assert_eq!(m.items.len(), 3); // wire decl + 2 instances
    let insts: Vec<_> = m
        .items
        .iter()
        .filter_map(|it| match it {
            ModuleItem::Instance(i) => Some(i),
            _ => None,
        })
        .collect();
    assert_eq!(insts.len(), 2);
    let u1 = &insts[0];
    assert_eq!(u1.mod_name, "submod");
    assert_eq!(u1.params.len(), 2);
    assert_eq!(u1.params[0].name.as_deref(), Some("W"));
    assert!(u1.params[1].expr.is_none(), "`.H()` is an empty override");
    assert_eq!(u1.insts[0].conns.len(), 3);
    assert_eq!(u1.insts[0].conns[2].name, None, "`z` is positional");
    let u2 = &insts[1];
    assert!(u2.params.is_empty());
    assert!(u2.insts[0].array.is_some());
    assert_eq!(u2.insts[0].conns.len(), 3);
}

#[test]
fn memory_decl_and_defparam() {
    let f = parse_ok(
        "module m;\n reg [7:0] mem [0:255];\n defparam u1.W = 4;\n\
         submod u1();\nendmodule\n",
    );
    let m = &f.modules[0];
    match &m.items[0] {
        ModuleItem::Decl(d) => {
            assert_eq!(d.kind, DeclKind::Reg);
            assert_eq!(d.entries.len(), 1);
            assert_eq!(d.entries[0].unpacked.len(), 1);
        }
        it => panic!("expected Decl, got {it:?}"),
    }
    assert!(matches!(&m.items[1], ModuleItem::Defparam(_)));
}

#[test]
fn error_recovery_reports_multiple() {
    let src = "module m;\n reg ;\n wire b;\n assign = 5;\n wire c;\nendmodule\n";
    let (f, diags, _sm) = frontend(src);
    assert!(diags.error_count() >= 2, "expected >=2 errors: {:#?}", diags.diags);
    // Recovery preserved the good declarations around the broken ones.
    let names: Vec<String> = f.modules[0]
        .items
        .iter()
        .filter_map(|it| match it {
            ModuleItem::Decl(d) => Some(d.entries[0].name.clone()),
            _ => None,
        })
        .collect();
    assert!(names.contains(&"b".to_string()));
    assert!(names.contains(&"c".to_string()));
}

#[test]
fn pp_object_and_function_macros() {
    let f = parse_ok(
        "`define WIDTH 8\n`define ADD(a, b) ((a) + (b))\n\
         module m;\n wire [`WIDTH-1:0] x;\n\
         assign y = `ADD(1, 2);\nendmodule\n",
    );
    let m = &f.modules[0];
    // `wire [(8)-1:0] x` — the parameter reference was expanded to `8`.
    match &m.items[0] {
        ModuleItem::Decl(d) => {
            let r = d.range.as_ref().unwrap();
            match &r.msb {
                Expr::Binary { op: ast::BinOp::Sub, lhs, .. } => {
                    assert!(matches!(&**lhs, Expr::NumLit(n) if n.digits == "8"));
                }
                e => panic!("expected (WIDTH)-1 expansion, got {e:?}"),
            }
        }
        it => panic!("expected Decl, got {it:?}"),
    }
    // ADD(1,2) expanded to ((1) + (2)).
    match &m.items[1] {
        ModuleItem::ContAssign(pairs) => match &pairs[0].rhs {
            Expr::Binary { op: ast::BinOp::Add, lhs, rhs, .. } => {
                assert!(matches!(&**lhs, Expr::NumLit(n) if n.digits == "1"));
                assert!(matches!(&**rhs, Expr::NumLit(n) if n.digits == "2"));
            }
            e => panic!("expected addition, got {e:?}"),
        },
        it => panic!("expected ContAssign, got {it:?}"),
    }
}

#[test]
fn pp_token_paste() {
    let f = parse_ok(
        "`define CAT(a, b) a``b\n module m;\n wire foobar;\n\
         assign foobar = 1'b0;\n assign CAT(foo, bar) = 1'b0;\nendmodule\n",
    );
    // If paste failed, the last assign would reference two idents and fail
    // to parse (or produce an error diag).
    assert_eq!(f.modules[0].name, "m");
}

#[test]
fn pp_ifdef_chain() {
    let f = parse_ok(
        "`define A\n module m;\n\
         `ifdef B\n wire missing;\n`elsif A\n wire present;\n`else\n wire absent;\n`endif\n\
         endmodule\n",
    );
    let text = format!("{:#?}", f);
    assert!(text.contains("present"));
    assert!(!text.contains("missing"));
    assert!(!text.contains("absent"));
}

#[test]
fn pp_include_search_dir() {
    let dir = std::env::temp_dir().join(format!("vl-pp-include-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("inc.v"), "wire w_inc;\n").unwrap();
    let mut sm = SourceMap::new();
    let mut diags = DiagSink::new();
    let opts = PpOptions {
        include_dirs: vec![dir.clone()],
        ..Default::default()
    };
    let out = pp::preprocess_source(
        "t.v",
        "module m;\n`include \"inc.v\"\nendmodule\n",
        &mut sm,
        &mut diags,
        &opts,
    );
    let f = parser::parse_source_file(out.tokens, &mut diags);
    assert!(!diags.has_errors(), "{:#?}", diags.diags);
    let found = f.modules[0]
        .items
        .iter()
        .any(|it| matches!(it, ModuleItem::Decl(d) if d.entries[0].name == "w_inc"));
    assert!(found);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pp_timescale_captured() {
    let mut sm = SourceMap::new();
    let mut diags = DiagSink::new();
    let out = pp::preprocess_source(
        "t.v",
        "`timescale 1ns/1ps\nmodule m;\nendmodule\n",
        &mut sm,
        &mut diags,
        &PpOptions::default(),
    );
    assert_eq!(out.timescale.as_deref(), Some("1ns/1ps"));
    assert!(out.tokens.iter().all(|t| !matches!(
        t.kind,
        Tok::Directive(_) | Tok::MacroId | Tok::MacroQuote | Tok::MacroPaste
    )));
}

#[test]
fn pp_line_builtin() {
    let f = parse_ok("module m;\ninitial $display(__LINE__);\nendmodule\n");
    let m = &f.modules[0];
    match &m.items[0] {
        ModuleItem::Initial { body, .. } => {
            let Stmt::SysCall { args, .. } = body else { panic!("syscall") };
            assert!(matches!(&args[0], Expr::NumLit(n) if n.digits == "2"));
        }
        it => panic!("expected Initial, got {it:?}"),
    }
}

#[test]
fn expr_precedence() {
    // a | b & c ^ d  ==  a | (b & (c ^ d))
    let f = parse_ok("module m;\n assign y = a | b & c ^ d;\nendmodule\n");
    let ModuleItem::ContAssign(pairs) = &f.modules[0].items[0] else { panic!("assign") };
    let Expr::Binary { op: ast::BinOp::BitOr, rhs, .. } = &pairs[0].rhs else { panic!("top |") };
    // `&` binds tighter than `^`: (b & c) ^ d
    let Expr::Binary { op: ast::BinOp::BitXor, lhs, .. } = &**rhs else { panic!("^ level") };
    assert!(matches!(&**lhs, Expr::Binary { op: ast::BinOp::BitAnd, .. }));

    // a + b << 2 == (a + b) << 2
    let f = parse_ok("module m;\n assign y = a + b << 2;\nendmodule\n");
    let ModuleItem::ContAssign(pairs) = &f.modules[0].items[0] else { panic!("assign") };
    let Expr::Binary { op: ast::BinOp::Shl, lhs, .. } = &pairs[0].rhs else { panic!("shl") };
    assert!(matches!(&**lhs, Expr::Binary { op: ast::BinOp::Add, .. }));

    // ternary: c ? a : b | d  -> els is (b | d)
    let f = parse_ok("module m;\n assign y = c ? a : b | d;\nendmodule\n");
    let ModuleItem::ContAssign(pairs) = &f.modules[0].items[0] else { panic!("assign") };
    let Expr::Ternary { els, .. } = &pairs[0].rhs else { panic!("ternary") };
    assert!(matches!(&**els, Expr::Binary { op: ast::BinOp::BitOr, .. }));

    // replication {2{a, b}}
    let f = parse_ok("module m;\n assign y = {2{a, b}};\nendmodule\n");
    let ModuleItem::ContAssign(pairs) = &f.modules[0].items[0] else { panic!("assign") };
    assert!(matches!(&pairs[0].rhs, Expr::Repeat { .. }));
}

#[test]
fn unsized_literal_parse() {
    let f = parse_ok("module m;\n assign y = 4'sd15 + 'h3x + 42;\nendmodule\n");
    let ModuleItem::ContAssign(pairs) = &f.modules[0].items[0] else { panic!("assign") };
    let Expr::Binary { lhs, .. } = &pairs[0].rhs else { panic!("add") };
    let Expr::Binary { lhs, rhs, .. } = &**lhs else { panic!("add inner") };
    assert!(matches!(&**lhs, Expr::NumLit(n) if n.width == Some(4) && n.signed && n.digits == "15"));
    assert!(matches!(&**rhs, Expr::NumLit(n) if n.width.is_none() && n.digits == "3x"));
}
