//! The committed-expectations regression gate: runs every corpus/e2e case
//! through the simulator and requires ≥80% byte-exact matches against the
//! Verilator-5.050-recorded expectations. Deterministic — needs no verilator.

#[test]
fn e2e_regression_gate() {
    let corpus = vl_difftest::corpus_dir();
    let results = vl_difftest::run_suite(&corpus);
    assert!(!results.is_empty(), "corpus/e2e has no cases");
    for r in results.iter().filter(|r| !r.pass) {
        eprintln!("FAIL {}:\n{}", r.name, r.detail);
    }
    let rate = vl_difftest::pass_rate(&results);
    eprintln!(
        "E2E gate: {}/{} = {:.1}%",
        results.iter().filter(|r| r.pass).count(),
        results.len(),
        rate * 100.0
    );
    assert!(rate >= 0.80, "E2E pass rate {:.1}% < 80%", rate * 100.0);
}
