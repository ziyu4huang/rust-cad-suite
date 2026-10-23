//! Stepwise-scheduling gate (ADR-0007): `run_until` semantics — TB-free
//! host-driven simulation, timer pacing, init-once, EOS-once, and the
//! deadline contract. The probe lessons are encoded here: put_value wakes
//! DUT logic but never runs it (put+run_until pairing), and time advance
//! without `#` events requires the deadline (probe 2).

use std::cell::RefCell;
use std::rc::Rc;
use vl_syntax::parser;
use vl_syntax::pp::{self, PpOptions};
use vl_sim::interp::{RunEnd, Sim};
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

const COUNTER: &str = "\
module tb;
  reg clk = 0;
  reg rst_n = 0;
  reg [7:0] q;
  always @(posedge clk or negedge rst_n)
    if (!rst_n) q <= 0;
    else q <= q + 1;
endmodule
";

#[test]
fn tb_free_host_driven_counter() {
    // No `#` events anywhere in the design: run_until's deadline is the only
    // thing that paces time. The host toggles clk purely via put_value.
    let mut sim = build(&[("t.v", COUNTER)], "tb");
    let clk = sim.vpi_handle_by_name("clk").expect("clk");
    let rst_n = sim.vpi_handle_by_name("rst_n").expect("rst_n");
    let q = sim.vpi_handle_by_name("q").expect("q");

    // Reset release (x -> 0 is a negedge: async reset applies on init).
    sim.vpi_put_value(rst_n, VpiValue::Int(0)).unwrap();
    sim.run_until(10).unwrap();
    let vpi_put_value = |sim: &mut Sim, h, v: i64| {
        sim.vpi_put_value(h, VpiValue::Int(v)).unwrap();
    };
    vpi_put_value(&mut sim, rst_n, 1);
    sim.run_until(20).unwrap();
    match sim.vpi_get_as(q, Format::Int).unwrap() {
        VpiValue::Int(0) => {}
        other => panic!("reset released, want q=0, got {other:?}"),
    }

    // Four host-driven posedges.
    for i in 1..=4u64 {
        vpi_put_value(&mut sim, clk, 1);
        sim.run_until(20 + i * 20 - 5).unwrap(); // high half
        vpi_put_value(&mut sim, clk, 0);
        sim.run_until(20 + i * 20).unwrap(); // low half
    }
    match sim.vpi_get_as(q, Format::Int).unwrap() {
        VpiValue::Int(v) => assert_eq!(v, 4, "four posedges -> q=4"),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn timer_paces_between_delayed_events() {
    // rVPI AfterDelay timers fire at their deadline even when the design's
    // next `#` event is further out (or absent) — D17.
    let mut sim = build(
        &[(
            "t.v",
            "module tb;\n  reg clk = 0;\n  reg [7:0] q = 0;\n  always #50 q <= q + 1;\nendmodule\n",
        )],
        "tb",
    );
    let q = sim.vpi_handle_by_name("q").expect("q");
    let seen: Rc<RefCell<Vec<(u64, i64)>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    sim.register_cb(CbReason::AfterDelay(10), Box::new(move |_s, info| {
        sink.borrow_mut().push((info.time, 7));
    }));
    sim.run_until(100).unwrap();
    let seen = seen.borrow();
    // The timer fired at t=10 (its deadline), not at the design's next
    // delayed event (t=50) — and CbInfo carries the timer's own time.
    assert!(
        seen.iter().any(|(t, v)| *t == 10 && *v == 7),
        "timer must fire at its own deadline, got {seen:?}"
    );
    // The design event still fired normally.
    match sim.vpi_get_as(q, Format::Int).unwrap() {
        VpiValue::Int(2) => {}
        other => panic!("two #50 increments by t=100, got {other:?}"),
    }
}

#[test]
fn run_until_then_run_continues_and_eos_fires_once() {
    let seen: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
    let mut sim = build(
        &[(
            "t.v",
            "module tb;\n  initial begin #10 $display(\"mid\"); #10 $display(\"end\"); $finish; end\nendmodule\n",
        )],
        "tb",
    );
    let sink = seen.clone();
    sim.register_cb(
        CbReason::EndOfSimulation,
        Box::new(move |_s, _i| sink.borrow_mut().push("eos")),
    );
    let end = sim.run_until(15).unwrap();
    assert_eq!(end, RunEnd::Running);
    // Continuation runs to $finish.
    assert!(sim.run().is_ok());
    assert_eq!(seen.borrow().len(), 1, "EOS exactly once");
    // A second run() on the finished sim is an idempotent no-op (D13), and
    // does not re-fire EOS.
    assert!(sim.run().is_ok());
    assert_eq!(seen.borrow().len(), 1);
    // After Finished, further run_until is a harmless no-op advance.
    let end = sim.run_until(999).unwrap();
    assert_eq!(end, RunEnd::Finished);
    assert_eq!(seen.borrow().len(), 1);
}

#[test]
fn run_until_deadline_contract() {
    let mut sim = build(&[("t.v", COUNTER)], "tb");
    // t <= now is misuse = hard error.
    let e = sim.run_until(0).unwrap_err();
    assert!(e.message.contains("run_until deadline"));
    assert!(sim.run_until(100).is_ok());
    let e = sim.run_until(100).unwrap_err();
    assert!(e.message.contains("run_until deadline"));
}

#[test]
fn finish_inside_window_ends_stepwise() {
    let mut sim = build(
        &[(
            "t.v",
            "module tb;\n  initial begin #20 $finish; end\nendmodule\n",
        )],
        "tb",
    );
    // $finish at t=20 lands inside the requested window -> Finished.
    let end = sim.run_until(100).unwrap();
    assert_eq!(end, RunEnd::Finished);
}

#[test]
fn quiescent_design_runs_not_finishes() {
    // A synchronous design with zero `#` events between host edges must NOT
    // be reported Finished at the first deadline (P2-1 lesson).
    let mut sim = build(&[("t.v", COUNTER)], "tb");
    let end = sim.run_until(5).unwrap();
    assert_eq!(end, RunEnd::Running, "quiescent design: time flowed");
    assert_eq!(sim.vpi_time(TimeScale::Precision), 5);
}
