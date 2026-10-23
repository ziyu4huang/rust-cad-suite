//! rVPI × VCD internal crosscheck: two independent observation paths over
//! ONE simulation run — (a) rVPI ValueChange callbacks recorded at the write
//! sites, (b) the VCD dump flushed at slot boundaries. Their change sets for
//! the same signal must agree (time, value) after write-burst collapse.
//! Complements vpi_gate's two-paths-vs-corpus evidence (ADR-0003 §7).

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use vl_vcd::{parse as parse_vcd, VcdValue};
use vl_syntax::parser;
use vl_syntax::pp::{self, PpOptions};
use vl_sim::interp::Sim;
use vl_sim::vpi::CbReason;

#[test]
fn vc_trace_matches_vcd_dump() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/e2e/51_vcd_counter");
    let tb = std::fs::read_to_string(dir.join("tb.v")).unwrap();
    let mut sm = rcs_diag::SourceMap::new();
    let mut diags = rcs_diag::DiagSink::new();
    let out = pp::preprocess_source(
        &dir.join("tb.v").display().to_string(),
        &tb,
        &mut sm,
        &mut diags,
        &PpOptions::default(),
    );
    let parsed = parser::parse_source_file(out.tokens, &mut diags);
    assert!(!diags.has_errors());
    let mut sim = Sim::build(&parsed.modules, "tb", out.timescale.as_deref()).unwrap();

    // (a) rVPI observation path: ValueChange deliveries at write sites.
    let q = sim.vpi_handle_by_name("q").expect("q");
    let vc: Rc<RefCell<Vec<(u64, String)>>> = Rc::new(RefCell::new(Vec::new()));
    sim.register_cb(
        CbReason::ValueChange(q),
        {
            let vc = vc.clone();
            Box::new(move |_s, i| {
                if let Some(b) = &i.value {
                    vc.borrow_mut().push((i.time, b.fmt_dec()));
                }
            })
        },
    );
    sim.run().unwrap();

    // (b) VCD observation path: the in-sim dump via $dumpfile/$dumpvars.
    let (_, vcd_text) = sim.take_vcd().expect("TB carries a dump");
    let vcd = parse_vcd(&vcd_text).expect("our own VCD parses");
    let qvar = vcd
        .vars
        .iter()
        .position(|v| v.leaf == "q")
        .expect("q in vcd");
    let vcd_changes: Vec<(u64, String)> = vcd
        .changes
        .iter()
        .filter(|c| c.var == qvar)
        .map(|c| {
            let raw = c.value.to_raw();
            let dec = match &c.value {
                VcdValue::Vector(v) if v.chars().all(|ch| ch == '0' || ch == '1') => {
                    u64::from_str_radix(v, 2).unwrap_or(0).to_string()
                }
                _ => raw,
            };
            (c.time, dec)
        })
        .collect();

    let vc_ref = vc.borrow();
    assert!(
        !vc_ref.is_empty() && !vcd_changes.is_empty(),
        "empty trace: rVPI={vc_ref:?} VCD={vcd_changes:?}"
    );
    // Agreement: every VCD change (time, value) must appear in the rVPI
    // trace — the rVPI path may collapse identically but never disagree.
    // The t=0 VCD rows are the $dumpvars initial snapshot, not change
    // events (C-VPI cbValueChange likewise only fires post-registration).
    for (t, v) in vcd_changes.iter().filter(|(t, _)| *t > 0) {
        assert!(
            vc_ref.iter().any(|(vt, vv)| vt == t && vv == v),
            "VCD change ({t},{v}) missing from rVPI trace {vc_ref:?}"
        );
    }
}
