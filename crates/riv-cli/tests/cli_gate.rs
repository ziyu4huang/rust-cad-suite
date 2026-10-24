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
fn compile_pipeline_is_a_milestone_hard_error_exit_2() {
    let (_, se, rc) = run("riverilog", &["-g2005", "tb.v"]);
    assert_eq!(rc, 2, "compile-class error");
    assert!(se.contains("milestone R1"), "{se}");
    // null target defers to R1 — same error today, different pointer.
    let (_, se, rc) = run("riverilog", &["-t", "null", "tb.v"]);
    assert_eq!(rc, 2);
    assert!(se.contains("milestone R1"), "{se}");
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
    // Existing file, nothing loadable yet: milestone hard-error.
    let (_, se, rc) = run_vp(&["Cargo.toml"]);
    assert_eq!(rc, 1);
    assert!(se.contains("milestone R2"), "{se}");
}
