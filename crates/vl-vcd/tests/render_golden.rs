//! Render golden: the tracked corpus artifact corpus/e2e/51_vcd_counter/expect.vcd
//! (in git since ADR-0005 — `!corpus/**/*.vcd` negation) must render
//! byte-identically to tests/goldens/counter.svg. Re-generating the golden
//! is a conscious act: rerun vcd2svg, eyeball the SVG, commit both together
//! (same discipline as expected.stdout).

use std::process::Command;

fn repo(path: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
        .to_string_lossy()
        .into()
}

#[test]
fn counter_vcd_renders_golden_byte_exact() {
    let vcd = repo("corpus/e2e/51_vcd_counter/expect.vcd");
    let text = std::fs::read_to_string(&vcd)
        .unwrap_or_else(|e| panic!("missing tracked fixture {}: {e}", vcd));
    let f = vl_vcd::parse(&text).expect("expect.vcd parses");
    let svg = vl_vcd::render(&f, &vl_vcd::RenderOptions::default()).expect("renders");
    let golden = std::fs::read_to_string(repo("crates/vl-vcd/tests/goldens/counter.svg"))
        .expect("golden present");
    assert_eq!(svg, golden, "render output drifted from golden");
}

#[test]
fn cli_stdout_matches_golden() {
    let out = Command::new(env!("CARGO_BIN_EXE_vcd2svg"))
        .arg(repo("corpus/e2e/51_vcd_counter/expect.vcd"))
        .output()
        .expect("bin runs");
    assert!(out.status.success(), "exit: {:?}", out.status);
    let golden = std::fs::read_to_string(repo("crates/vl-vcd/tests/goldens/counter.svg"))
        .expect("golden present");
    assert_eq!(String::from_utf8_lossy(&out.stdout), golden);
}

#[test]
fn cli_unknown_flag_is_hard_error() {
    let out = Command::new(env!("CARGO_BIN_EXE_vcd2svg"))
        .arg("--frobnicate")
        .arg("x.vcd")
        .output()
        .expect("bin runs");
    assert_eq!(out.status.code(), Some(1), "unknown flag must exit 1");
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown option"));
}

#[test]
fn cli_missing_file_and_bad_time_exit_1() {
    let out = Command::new(env!("CARGO_BIN_EXE_vcd2svg"))
        .arg("no/such/file.vcd")
        .output()
        .expect("bin runs");
    assert_eq!(out.status.code(), Some(1));
    // VL-E101 through the CLI on a malformed #time.
    let dir = std::env::temp_dir().join("vcd2svg-e101.vcd");
    std::fs::write(&dir, "$enddefinitions $end\n#abc\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_vcd2svg"))
        .arg(&dir)
        .output()
        .expect("bin runs");
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("VL-E101"));
}
