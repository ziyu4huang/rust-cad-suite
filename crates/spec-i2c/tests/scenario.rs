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

// ---------- clock stretching + multi-address (ADR-0007 open items) ----------

use spec_i2c::scenario::{run_cfg, run_reads, Config};

/// A stretching slave leaves the spec RTL clean: the master per this spec
/// ignores scl_i, data levels are identical in both halves, and the golden
/// agrees cycle-for-cycle. This pins the documented simplification under
/// the exact condition it could break.
#[test]
fn stretch_stays_clean_on_spec_rtl() {
    let r = run_cfg(&Config {
        stretch: true,
        ..Config::default()
    })
    .expect("stretched scenario runs");
    assert_eq!(r.report.violations.len(), 0, "{}", r.report.render_text());
}

/// The HonorsSclI variant (rd_mode && scl_i at the read sample) deviates
/// from the documented simplification. Against a NON-stretching slave it
/// is behaviorally equivalent (scl_i mirrors the master's own schedule) —
/// and must NOT be reported. Under a stretching slave the stretched
/// windows cover sample points, samples are skipped, and the lockstep
/// names the divergence.
#[test]
fn honors_scl_i_equivalent_without_stretch() {
    let r = run_cfg(&Config {
        variant: Variant::HonorsSclI,
        ..Config::default()
    })
    .expect("runs");
    assert_eq!(r.report.violations.len(), 0, "{}", r.report.render_text());
}

#[test]
fn stretch_exposes_honors_scl_i() {
    // The mutation gates the READ sample path (rd_mode && scl_i), so the
    // leg must carry a read byte — the write-only script never executes it.
    let r = run_reads(&Config {
        variant: Variant::HonorsSclI,
        stretch: true,
        ..Config::default()
    })
    .expect("runs");
    assert!(
        !r.report.violations.is_empty(),
        "stretching must expose an scl_i-honoring RTL"
    );
    assert!(r
        .report
        .violations
        .iter()
        .any(|v| matches!(&v.kind, vl_spec::ViolationKind::Lockstep(_))));
}

/// Two slaves behind one bus position: each read transaction returns the
/// addressed slave's own word, and the run stays clean (the golden's bus
/// view and the driver's slave agree on addressing).
#[test]
fn multi_address_reads() {
    let r = run_reads(&Config {
        slaves: vec![(0x50, [0xA5, 0x3C, 0x81, 0x5A]), (0x23, [0x11, 0x22, 0x33, 0x44])],
        ..Config::default()
    })
    .expect("multi-address reads run");
    r.report.assert_empty();
    assert_eq!(
        r.dout_samples,
        vec![0xA5, 0x11],
        "each addressed slave must serve its own word"
    );
}

/// Unknown-address read: the requested address is outside the slave
/// table, so nothing pulls the ack (NACK) and the released bus reads
/// 0xff — with the golden agreeing (clean lockstep). Exercises the
/// strict-addressing decode (matched gates ack_pull) end-to-end.
#[test]
fn unknown_address_read_returns_ff() {
    let r = run_reads(&Config {
        slaves: vec![(0x50, [0xA5, 0x3C, 0x81, 0x5A])],
        read_addrs: Some(vec![0x77]),
        ..Config::default()
    })
    .expect("unknown-address read runs");
    r.report.assert_empty();
    assert_eq!(r.dout_samples, vec![0xff], "released bus reads 0xff");
}

/// The START phantom's bits must not reach the slave's decode: with the
/// documented skip, the slave's mode/addressing state after the phantom is
/// exactly what the ADDRESS byte (second byte) sets — proven by running
/// two transactions back-to-back where the phantom carries a read-direction
/// stale tx (LSB=1) that would flip the slave into read mode and contend
/// with the master's address drive if it were decoded.
#[test]
fn phantom_byte_never_decodes() {
    let r = run_reads(&Config {
        slaves: vec![(0x50, [0xA5, 0x3C, 0x81, 0x5A]), (0x23, [0x11, 0x22, 0x33, 0x44])],
        ..Config::default()
    })
    .expect("two-transaction read run");
    r.report.assert_empty();
    assert_eq!(r.dout_samples, vec![0xA5, 0x11]);
}
