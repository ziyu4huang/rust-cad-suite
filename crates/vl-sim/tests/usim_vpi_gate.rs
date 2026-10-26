//! U-4 slice-2 gate (ADR-0009 / ADR-0003 amendment): the unified-engine
//! face (USim) shares the rVPI registry with the legacy segments face —
//! registration surface identical (vpi_gate is the other side), firing
//! order asserted verbatim here through the kernel's four hooks.

use std::cell::RefCell;
use std::rc::Rc;
use vl_syntax::parser;
use vl_syntax::pp::{self, PpOptions};
use vl_sim::vpi::{CbReason, TimeScale};
use vl_sim::USim;
use vl_values::Bits;

fn build_usim(src: &str) -> USim {
    let mut sm = rcs_diag::SourceMap::new();
    let mut diags = rcs_diag::DiagSink::new();
    let opts = PpOptions::default();
    let out = pp::preprocess_source("t.v", src, &mut sm, &mut diags, &opts);
    let timescale = out.timescale.clone();
    let parsed = parser::parse_source_file(out.tokens, &mut diags);
    assert!(!diags.has_errors(), "{:#?}", diags.diags);
    let files = vec![("t.v".to_string(), timescale.clone())];
    USim::build(&parsed.modules, "tb", timescale.as_deref(), &sm, &files).expect("elaborate")
}

#[test]
fn usim_firing_order_parity() {
    let tb = r#"
`timescale 1ns/1ps
module tb;
  reg q = 0;
  initial begin
    #2 q = 1;
    #2 q = 0;
    #2 $finish;
  end
endmodule
"#;
    let mut sim = build_usim(tb);
    let q = sim.vpi_handle_by_name("q").expect("q handle");
    let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    sim.register_cb(
        CbReason::StartOfSimulation,
        {
            let log = log.clone();
            Box::new(move |_h, _i| log.borrow_mut().push("start".to_string()))
        },
    );
    sim.register_cb(
        CbReason::ValueChange(q),
        {
            let log = log.clone();
            Box::new(move |_h, i| {
                if let Some(b) = &i.value {
                    log.borrow_mut().push(format!("vc q={}@{}", b.fmt_dec(), i.time));
                }
            })
        },
    );
    sim.register_cb(
        CbReason::AfterDelay(4_000),
        {
            let log = log.clone();
            Box::new(move |h, i| {
                log.borrow_mut().push(format!("timer@{}", i.time));
                h.finish(); // vpi_control(vpiFinish) analog on the unified engine
            })
        },
    );
    sim.register_cb(
        CbReason::EndOfSimulation,
        {
            let log = log.clone();
            Box::new(move |_h, i| log.borrow_mut().push(format!("end@{}", i.time)))
        },
    );
    sim.run().expect("run");
    // 1ns/1ps → precision ticks are ps. The kernel fires vc post-settle per
    // commit (t=2000); at t=4000 the after_delay point fires the timer
    // BEFORE the slots execute, and its finish short-circuits the whole
    // time step — the q=0 commit never happens (legacy fire_due_timers →
    // finish parity; the stop_as_finish check at the hook is the slice-2
    // kernel amendment). The end-point drain is a no-op.
    assert_eq!(
        *log.borrow(),
        vec![
            "start".to_string(),
            "vc q=1@2000".to_string(),
            "timer@4000".to_string(),
            "end@4000".to_string(),
        ]
    );
}

#[test]
fn usim_host_read_write_from_callback() {
    let tb = r#"
`timescale 1ns/1ps
module tb;
  reg a = 0;
  initial begin
    #1 a = 1;
    #1 $finish;
  end
endmodule
"#;
    let mut sim = build_usim(tb);
    let a = sim.vpi_handle_by_name("a").expect("a");
    let seen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    // On the first change of a: read through the host face, then write a
    // back to 0 — the put goes through the same funnel as design writes
    // (kernel vpi_write_var). The end-of-sim callback reads back through
    // the host to prove the put landed in kernel storage.
    sim.register_cb(
        CbReason::ValueChange(a),
        {
            let seen = seen.clone();
            let mut done = false;
            Box::new(move |h, _i| {
                if done {
                    return;
                }
                done = true;
                let bits = h.bits(a).expect("host bits");
                seen.borrow_mut().push(format!("read a={}", bits.fmt_dec()));
                seen.borrow_mut().push(format!("name={}", h.name(a)));
                seen.borrow_mut()
                    .push(format!("now={}", h.time(TimeScale::Precision)));
                h.put_bits(a, &Bits::from_u64(1, 0).expect("bits")).expect("host put");
            })
        },
    );
    sim.register_cb(
        CbReason::EndOfSimulation,
        {
            let seen = seen.clone();
            Box::new(move |h, _i| {
                let bits = h.bits(a).expect("readback");
                seen.borrow_mut().push(format!("final a={}", bits.fmt_dec()));
            })
        },
    );
    sim.run().expect("run");
    // 1ns/1ps → ticks are ps.
    assert_eq!(
        *seen.borrow(),
        vec![
            "read a=1".to_string(),
            "name=a".to_string(),
            "now=1000".to_string(),
            "final a=0".to_string(),
        ]
    );
}

#[test]
fn usim_remove_cb_and_no_cb_zero_cost_path() {
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
    let mut sim = build_usim(tb);
    let r = sim.vpi_handle_by_name("r").expect("r");
    let n: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
    let id = sim.register_cb(
        CbReason::ValueChange(r),
        {
            let n = n.clone();
            Box::new(move |_h, _i| *n.borrow_mut() += 1)
        },
    );
    sim.remove_cb(id);
    sim.run().expect("run");
    assert_eq!(*n.borrow(), 0);
}
