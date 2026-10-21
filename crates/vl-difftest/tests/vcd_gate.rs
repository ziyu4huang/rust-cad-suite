//! M6 gate: for every e2e case that ships an `oracle-ref.vcd` (recorded via
//! scripts/oracle_vcd.sh), our simulator's VCD must agree with the oracle's
//! on every matched signal at every change time (alias/suffix-folded
//! comparison, dump-frequency independent). No verilator needed at test time.

#[test]
fn vcd_oracle_crosscheck_gate() {
    let corpus = vl_difftest::corpus_dir();
    let mut checked = 0;
    for entry in std::fs::read_dir(&corpus).unwrap().flatten() {
        let dir = entry.path();
        let ref_path = dir.join("oracle-ref.vcd");
        if !ref_path.exists() {
            continue;
        }
        let oracle = std::fs::read_to_string(&ref_path).unwrap();
        let out = vl_difftest::run_ours(&dir).expect("simulate");
        let ours = out.vcd.expect("case with oracle-ref.vcd must $dumpvars");
        let (report, ok) = vl_difftest::vcd_crosscheck(&ours, &oracle);
        eprintln!("== {} ==\n{report}", dir.display());
        assert!(ok, "VCD crosscheck failed for {}", dir.display());
        checked += 1;
    }
    assert!(checked >= 1, "no oracle-ref.vcd cases found");
}
