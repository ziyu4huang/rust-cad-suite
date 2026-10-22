//! rVPI gate: handles/properties, callback firing order, NBA visibility,
//! put_value blocking semantics, iteration, memory words, write-burst
//! collapse. Slice-1 acceptance (ADR-0003). The oracle for observable
//! behavior is the corpus (recorded from Verilator); the VPI surface is
//! asserted for structural semantics.

use std::cell::RefCell;
use std::rc::Rc;
use vl_syntax::parser;
use vl_syntax::pp::{self, PpOptions};
use vl_sim::interp::Sim;
use vl_sim::vpi::*;

fn build(files: &[(&str, &str)], top: &str) -> Sim {
    let mut sm = rcs_diag::SourceMap::new();
    let mut diags = rcs_diag::DiagSink::new();
    let opts = PpOptions::default();
    let mut modules = Vec::new();
    let mut timescale: Option<String> = None;
    for (name, src) in files {
        let out = pp::preprocess_source(name, src, &mut sm, &mut diags, &opts);
        if timescale.is_none() {
            timescale = out.timescale.clone();
        }
        let parsed = parser::parse_source_file(out.tokens, &mut diags);
        modules.extend(parsed.modules);
    }
    assert!(!diags.has_errors(), "{:#?}", diags.diags);
    Sim::build(&modules, top, timescale.as_deref()).expect("elaborate")
}

#[test]
fn handles_and_properties() {
    let mut sim = build(
        &[
            ("t.v", "module tb;\n  reg [7:0] mem [0:3];\n  wire clk;\n  parameter P = 5;\nendmodule\n"),
        ],
        "tb",
    );
    let top = sim.vpi_top();
    assert_eq!(sim.vpi_name(top), "tb");
    assert_eq!(sim.vpi_kind(top), CellKind::Module);
    // bare and top-prefixed paths both resolve
    let mem = sim.vpi_handle_by_name("mem").expect("mem");
    assert_eq!(sim.vpi_handle_by_name("tb.mem"), Some(mem));
    assert_eq!(sim.vpi_kind(mem), CellKind::Reg);
    assert_eq!(sim.vpi_size(mem), 8);
    assert_eq!(sim.vpi_range(mem), Some((7, 0)));
    let clk = sim.vpi_handle_by_name("clk").unwrap();
    assert_eq!(sim.vpi_kind(clk), CellKind::Net);
    let p = sim.vpi_handle_by_name("P").unwrap();
    assert_eq!(sim.vpi_kind(p), CellKind::Param);
    // children: regs = mem; nets = clk; params = P; modules = none
    let regs: Vec<_> = sim
        .vpi_children(top, Relation::Regs)
        .iter()
        .map(|h| sim.vpi_name(*h).to_string())
        .collect();
    assert_eq!(regs, vec!["mem[0]", "mem[1]", "mem[2]", "mem[3]"]);
    let nets: Vec<_> = sim
        .vpi_children(top, Relation::Nets)
        .iter()
        .map(|h| sim.vpi_name(*h).to_string())
        .collect();
    assert_eq!(nets, vec!["clk"]);
    assert!(sim.vpi_children(top, Relation::Modules).is_empty());
}

#[test]
fn firing_order_and_nba_visibility() {
    // `q` updates via NBA at posedge; a ValueChange callback must observe
    // the post-NBA value at the NBA drain (counter8 `#1` subtlety).
    let tb = r#"
`timescale 1ns/1ps
module tb;
  reg clk = 0;
  reg [7:0] q = 0;
  always #5 clk = ~clk;
  always @(posedge clk) q <= q + 1;
  initial begin
    repeat (3) @(posedge clk);
    #1 $finish;
  end
endmodule
"#;
    let mut sim = build(&[("t.v", tb)], "tb");
    let q = sim.vpi_handle_by_name("q").unwrap();
    let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let log2: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let log3: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let log4: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    sim.register_cb(
        CbReason::StartOfSimulation,
        {
            let log = log.clone();
            Box::new(move |_s, _i| log.borrow_mut().push("start".to_string()))
        },
    );
    sim.register_cb(
        CbReason::ValueChange(q),
        {
            let log2 = log2.clone();
            Box::new(move |_s, i| {
                if let Some(b) = &i.value {
                    log2.borrow_mut().push(format!("q={}@{}", b.fmt_dec(), i.time));
                }
            })
        },
    );
    sim.register_cb(
        CbReason::AfterDelay(16_000),
        {
            let log3 = log3.clone();
            Box::new(move |s, i| {
                log3.borrow_mut().push(format!("timer@{}", i.time));
                s.finish(); // vpi_control(vpiFinish) analog
            })
        },
    );
    sim.register_cb(
        CbReason::EndOfSimulation,
        {
            let log4 = log4.clone();
            Box::new(move |_s, i| log4.borrow_mut().push(format!("end@{}", i.time)))
        },
    );
    sim.run().unwrap();
    assert_eq!(*log.borrow(), vec!["start"]);
    // Each posedge's NBA increment delivers one collapsed callback with the
    // post-NBA value (1, 2, 3); the write at t=0 fires before.
    // 1ns/1ps → `now` is precision (ps) ticks: posedges at 5/15/25ns.
    assert_eq!(
        *log2.borrow(),
        vec!["q=1@5000".to_string(), "q=2@15000".to_string()]
    );
    // Timer at 16k fired; finish() from inside a callback ended the run and
    // EndOfSimulation still delivered (third posedge never ran).
    // Timers fire at the start of the next slot (next delayed key >= t).
    assert_eq!(*log3.borrow(), vec!["timer@20000"]);
    assert_eq!(*log4.borrow(), vec!["end@20000"]);
}

#[test]
fn two_paths_one_truth_against_corpus() {
    // The 44_lfsr corpus case samples `lfsr` at t=81 (8 posedges @10ns + #1).
    // The Verilog path's recorded expectation (oracle) must equal what an
    // rVPI monitor renders at the same offsets.
    let expected = include_str!("../../../corpus/e2e/44_lfsr/expected.stdout");
    let tb = include_str!("../../../corpus/e2e/44_lfsr/tb.v");
    let mut sim = build(&[("tb.v", tb)], "tb");
    let lfsr = sim.vpi_handle_by_name("lfsr").expect("lfsr");
    let lines: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    // 1ns/1ps → ticks are ps: 8 posedges (5..75ns) + #1 = t=76_000.
    sim.register_cb(
        CbReason::AfterDelay(76_000),
        {
            let lines = lines.clone();
            Box::new(move |s, _i| {
                if let Ok(VpiValue::Vector(b)) = s.vpi_get_as(lfsr, Format::Vector) {
                    lines.borrow_mut().push(format!("lfsr={}", b.fmt_hex()));
                }
            })
        },
    );
    sim.run().unwrap();
    let vpi_line = format!("{}\n", lines.borrow()[0]);
    assert!(
        expected.contains(&vpi_line),
        "VPI sample {vpi_line:?} not in recorded corpus output {expected:?}"
    );
}

#[test]
fn put_value_blocks_and_propagates() {
    let tb = r#"
module tb;
  reg a = 0;
  wire b = ~a;
  initial begin
    #1;
    $finish;
  end
endmodule
"#;
    let mut sim = build(&[("t.v", tb)], "tb");
    let a = sim.vpi_handle_by_name("a").unwrap();
    let b = sim.vpi_handle_by_name("b").unwrap();
    sim.run().unwrap();
    // Post-run steady state: b = ~a = 1.
    assert!(matches!(sim.vpi_get_as(b, Format::Int), Ok(VpiValue::Int(1))));
    sim.vpi_put_value(a, VpiValue::Int(1)).unwrap();
    // NoDelay put = blocking assign: combinational settle already ran.
    assert!(matches!(sim.vpi_get_as(b, Format::Int), Ok(VpiValue::Int(0))));
    // x-carrying values refuse Int format (deterministic, X-policy).
    sim.vpi_put_value(a, VpiValue::Scalar(Scalar::X)).unwrap();
    assert!(sim.vpi_get_as(a, Format::Int).is_err());
    assert!(matches!(
        sim.vpi_get_as(a, Format::Scalar),
        Ok(VpiValue::Scalar(Scalar::X))
    ));
}

#[test]
fn memory_words_and_whole_memory_guard() {
    let tb = r#"
`timescale 1ns/1ps
module tb;
  reg [7:0] mem [0:3];
  initial begin
    mem[0] = 8'h11;
    #1 $finish;
  end
endmodule
"#;
    let mut sim = build(&[("t.v", tb)], "tb");
    let w2 = sim.vpi_handle_by_name("mem[2]").unwrap();
    assert_eq!(sim.vpi_kind(w2), CellKind::MemoryWord);
    assert!(matches!(
        sim.vpi_get_as(w2, Format::Int),
        Ok(VpiValue::Int(0))
    ));
    sim.vpi_put_value(w2, VpiValue::Int(0x42)).unwrap();
    assert!(matches!(
        sim.vpi_get_as(w2, Format::Int),
        Ok(VpiValue::Int(0x42))
    ));
    // The array itself is an object (vpiMemory analog) but not writable as
    // a whole; word handles are the write path.
    let mem = sim.vpi_handle_by_name("mem").unwrap();
    assert_eq!(sim.vpi_kind(mem), CellKind::Reg);
    assert!(sim.vpi_put_value(mem, VpiValue::Int(1)).is_err());
}

#[test]
fn write_burst_collapses_to_one_callback() {
    let tb = r#"
`timescale 1ns/1ps
module tb;
  reg [7:0] r = 0;
  initial begin
    r = 1;
    r = 2;
    r = 3;
    #1 $finish;
  end
endmodule
"#;
    let mut sim = build(&[("t.v", tb)], "tb");
    let r = sim.vpi_handle_by_name("r").unwrap();
    let n: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
    let seen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    sim.register_cb(
        CbReason::ValueChange(r),
        {
            let (n, seen) = (n.clone(), seen.clone());
            Box::new(move |_s, i| {
                *n.borrow_mut() += 1;
                if let Some(b) = &i.value {
                    seen.borrow_mut().push(b.fmt_dec());
                }
            })
        },
    );
    sim.run().unwrap();
    // Three writes in the same active burst → one delivery with the final
    // value (same collapse the VCD pending map applies).
    assert_eq!(*n.borrow(), 1);
    assert_eq!(*seen.borrow(), vec!["3"]);
}

#[test]
fn hierarchical_iteration() {
    let tb = r#"
module tb;
  wire w;
  dut u0(.o(w));
endmodule
module dut(input i, output o);
  wire inner;
  reg [3:0] r;
  assign o = i | inner | r[0];
endmodule
"#;
    let sim = build(&[("t.v", tb)], "tb");
    let top = sim.vpi_top();
    let mods = sim.vpi_children(top, Relation::Modules);
    assert_eq!(mods.len(), 1);
    assert_eq!(sim.vpi_name(mods[0]), "u0");
    let inner = sim.vpi_handle_by_name("u0.inner").unwrap();
    assert_eq!(sim.vpi_kind(inner), CellKind::Net);
    let regs: Vec<_> = sim
        .vpi_children(mods[0], Relation::Regs)
        .iter()
        .map(|h| sim.vpi_name(*h).to_string())
        .collect();
    // Names are canonical dotted paths.
    assert_eq!(regs, vec!["u0.r"]);
}

#[test]
fn remove_cb_takes_effect() {
    let tb = r#"
`timescale 1ns/1ps
module tb;
  reg r = 0;
  initial begin
    r = 1;
    #1 $finish;
  end
endmodule
"#;
    let mut sim = build(&[("t.v", tb)], "tb");
    let r = sim.vpi_handle_by_name("r").unwrap();
    let n: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
    let id = sim.register_cb(
        CbReason::ValueChange(r),
        {
            let n = n.clone();
            Box::new(move |_s, _i| *n.borrow_mut() += 1)
        },
    );
    sim.remove_cb(id);
    sim.run().unwrap();
    assert_eq!(*n.borrow(), 0);
}
