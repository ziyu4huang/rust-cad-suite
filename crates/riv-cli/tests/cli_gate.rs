//! riv-cli golden gate: the riverilog/rivvp CLI contract pinned byte-exact
//! against committed goldens — zero oracle dependency (the gate runs in any
//! environment; recording needed the oracle once, running never does).

use std::process::Command;

fn run(bin: &str, args: &[&str]) -> (String, String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_riverilog"))
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn {bin}: {e}"));
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

fn run_vp(args: &[&str]) -> (String, String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_rivvp"))
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn rivvp: {e}"));
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

fn golden(name: &str) -> String {
    std::fs::read_to_string(format!("{}/{}", env!("CARGO_MANIFEST_DIR"), name))
        .unwrap_or_else(|e| panic!("golden {name}: {e}"))
}

/// riverilog -V: banner shape with version self-identification (golden is
/// templated on VERSION — the {VERSION} marker is substituted before diff).
#[test]
fn v_banner_matches_golden() {
    let (so, _, rc) = run("riverilog", &["-V"]);
    assert_eq!(rc, 0);
    let want = golden("goldens/riverilog-V.txt").replace("{VERSION}", env!("CARGO_PKG_VERSION"));
    assert_eq!(so, want);
}

#[test]
fn h_goes_to_stderr_with_exit_1() {
    let (so, se, rc) = run("riverilog", &["-h"]);
    assert_eq!(rc, 1, "iverilog -h exits 1 (measured, asymmetric to vvp)");
    assert!(so.is_empty(), "usage goes to stderr only");
    assert_eq!(se, golden("goldens/riverilog-h.txt"));
}

#[test]
fn no_source_files_matches_oracle_shape() {
    let (_, se, rc) = run("riverilog", &[]);
    assert_eq!(rc, 1);
    assert!(se.starts_with("riverilog: no source files."), "{se}");
}

#[test]
fn illegal_option_matches_oracle_shape() {
    let (_, se, rc) = run("riverilog", &["--frobnicate"]);
    assert_eq!(rc, 1);
    assert!(
        se.contains("riverilog: illegal option -- -"),
        "oracle shape: the byte after the first dash, got {se}"
    );
}

#[test]
fn unknown_generation_is_hard_error() {
    let (_, se, rc) = run("riverilog", &["-g", "2007", "x.v"]);
    assert_eq!(rc, 1);
    assert!(
        se.contains("riverilog: Unknown/Unsupported Language generation 2007"),
        "{se}"
    );
}

#[test]
fn unplanned_targets_name_the_line() {
    for t in ["blif", "fpga", "vhdl", "vlog95"] {
        let (_, se, rc) = run("riverilog", &["-t", t, "x.v"]);
        assert_eq!(rc, 1);
        assert!(se.contains("not planned"), "{t}: {se}");
    }
}

#[test]
fn compile_class_errors_are_exit_2() {
    // Missing source file: a compile-class error, exit 2 (the R0 pipeline
    // milestone stub retired at C2 — the pipeline now compiles flat
    // designs; only the error CLASS is pinned here, the diag text is
    // vl-syntax's).
    let (_, se, rc) = run("riverilog", &["-g2005", "tb.v"]);
    assert_eq!(rc, 2, "compile-class error");
    assert!(se.contains("cannot open input file"), "{se}");
    // null target still defers to R1 (preprocess+parse only).
    let (_, se, rc) = run("riverilog", &["-t", "null", "tb.v"]);
    assert_eq!(rc, 2);
    assert!(se.contains("milestone R1"), "{se}");
}

/// C2/C3 acceptance as a mechanical gate: riverilog compiles the iv-e2e
/// flat cases to RIV-1 and rivvp runs them byte-identical to the recorded
/// iverilog goldens (run in a temp dir, cwd = case dir per the recording
/// contract).
fn run_iv_case(case: &str) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let case_dir = root.join("corpus/iv-e2e").join(case);
    let work = std::env::temp_dir().join(format!("riv-cli-gate-{case}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    std::fs::copy(case_dir.join("tb.v"), work.join("tb.v")).unwrap();
    let compile = Command::new(env!("CARGO_BIN_EXE_riverilog"))
        .args(["-g2005", "-o", "out.riv", "tb.v"])
        .current_dir(&work)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{case}: riverilog failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let run = Command::new(env!("CARGO_BIN_EXE_rivvp"))
        .arg("out.riv")
        .current_dir(&work)
        .output()
        .unwrap();
    let want = std::fs::read_to_string(case_dir.join("expected.stdout")).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        want,
        "{case}: stdout byte-diff"
    );
    let want_exit: i32 = std::fs::read_to_string(case_dir.join("expected.exit"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(run.status.code(), Some(want_exit), "{case}: exit code");
}

#[test]
fn iv_e2e_hello_and_nba_are_byte_exact() {
    run_iv_case("01_hello");
    // 03_nba pins the stratified slot: active reads the old value, #0
    // (inactive) still the old one, NBA commits after, settled at #1.
    run_iv_case("03_nba");
}

#[test]
fn vvp_contracts() {
    let (so, _, rc) = run_vp(&["-V"]);
    assert_eq!(rc, 0);
    assert_eq!(so, format!("rivvp (riverilog runtime) version {}\n", env!("CARGO_PKG_VERSION")));
    // vvp -h exits 0 on stdout (measured asymmetry vs iverilog -h).
    let (so, se, rc) = run_vp(&["-h"]);
    assert_eq!(rc, 0);
    assert!(so.starts_with("Usage: rivvp"), "{so}");
    assert!(se.is_empty());
    // Missing input file: oracle shape + 255.
    let (_, se, rc) = run_vp(&["nonexistent.vvp"]);
    assert_eq!(rc, 255);
    assert_eq!(se, "nonexistent.vvp: Unable to open input file.\n");
    // No input at all.
    let (_, se, rc) = run_vp(&[]);
    assert_eq!(rc, 255);
    assert_eq!(se, "rivvp: no input file.\n");
    // Existing file that is not a riv design: load refusal, exit 1 (the
    // R0 "nothing loadable" milestone stub retired at C3 — rivvp loads
    // RIV-1 now).
    let (_, se, rc) = run_vp(&["Cargo.toml"]);
    assert_eq!(rc, 1);
    assert!(se.contains("not a riv design"), "{se}");
}
