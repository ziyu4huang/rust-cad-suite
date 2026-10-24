//! Native pin for the demo scenario composition (all four invariants +
//! full script + all three RTL variants). Nothing ran this union before;
//! the no-ack variant's STOP completion and the LSB-first variant's second
//! WR were unproven until this test. The wasm gate (wasm-test/spec.mjs)
//! depends on these behaviors — green here FIRST, wired there second.

use spec_i2c::scenario::{self, Variant};

#[test]
fn correct_variant_clean_with_vcd() {
    let r = scenario::run(Variant::Correct).expect("scenario runs");
    assert_eq!(r.report.violations.len(), 0, "{}", r.report.render_text());
    assert!(r.report.cycles_run > 100, "canonical script is longer");
    let vcd = r.vcd.expect("force_dump before init = vcd captured");
    assert!(vcd.contains("$var"), "vcd carries signal declarations");
    assert!(vcd.contains("$enddefinitions $end"));
}

#[test]
fn correct_variant_deterministic() {
    let a = scenario::run(Variant::Correct).expect("run a");
    let b = scenario::run(Variant::Correct).expect("run b");
    assert_eq!(a.vcd, b.vcd, "vcd must be byte-deterministic");
    assert_eq!(
        a.report.render_text(),
        b.report.render_text(),
        "report must be deterministic"
    );
}

#[test]
fn lsb_first_variant_caught() {
    let r = scenario::run(Variant::LsbFirst).expect("scenario runs");
    assert!(
        r.report.violations.len() >= 40,
        "wholesale divergence expected: {}",
        r.report.render_text()
    );
    let names: Vec<String> = r
        .report
        .violations
        .iter()
        .map(|v| match &v.kind {
            vl_spec::ViolationKind::Invariant(n) => n.to_string(),
            vl_spec::ViolationKind::Lockstep(n) => format!("lockstep[{n}]"),
        })
        .collect();
    assert!(names.iter().any(|n| n.starts_with("lockstep")));
    assert!(names.iter().any(|n| n == "msb_first_bit_order"));
}

#[test]
fn no_ack_release_variant_caught() {
    let r = scenario::run(Variant::NoAckRelease).expect("scenario runs (STOP completes)");
    let names: Vec<String> = r
        .report
        .violations
        .iter()
        .map(|v| match &v.kind {
            vl_spec::ViolationKind::Invariant(n) => n.to_string(),
            vl_spec::ViolationKind::Lockstep(n) => format!("lockstep[{n}]"),
        })
        .collect();
    assert!(
        names.iter().any(|n| n == "ack_release"),
        "must trip ack_release; got {names:?}"
    );
}

#[test]
fn rtl_for_anchors_hit() {
    let base = spec_i2c::RTL;
    assert_ne!(scenario::rtl_for(Variant::LsbFirst).unwrap(), base);
    assert_ne!(scenario::rtl_for(Variant::NoAckRelease).unwrap(), base);
    assert_eq!(scenario::rtl_for(Variant::Correct).unwrap(), base);
    // Anchor-drift guard: a miss returns Err, never panics (wasm path).
    assert!(scenario::rtl_for(Variant::Correct).is_ok());
}
