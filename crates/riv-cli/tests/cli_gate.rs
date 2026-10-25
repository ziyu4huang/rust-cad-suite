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

/// pp-class errors (missing source file included) are exit 1 — the oracle
/// answers a missing input with "No such file or directory" + exit 1
/// (measured iverilog 13.0); our pre-R1-shape exit 2 folded pp into parse
/// and was reclassified with the shared frontend (U-3). Diag text is
/// vl-syntax's; the wrapper line is free (rivD5).
#[test]
fn pp_class_errors_are_exit_1() {
    let (_, se, rc) = run("riverilog", &["-g2005", "tb.v"]);
    assert_eq!(rc, 1, "pp-class error");
    assert!(se.contains("cannot open input file"), "{se}");
    assert!(se.contains("preprocessor failed with 1 error(s)"), "{se}");
}

/// parse-class errors stay exit 2, independent of the pp class.
#[test]
fn parse_class_errors_are_exit_2() {
    let work = std::env::temp_dir().join("riv-cli-gate-parse");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("bad.v"), "module bad;\nendmodule m typos;)\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_riverilog"))
        .args(["-t", "null", "bad.v"])
        .current_dir(&work)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "parse-class error");
    let se = String::from_utf8_lossy(&out.stderr);
    assert!(se.contains("error(s) during parsing"), "{se}");
}

/// -t null real behavior (replaces the retired R1 deferral): full pipeline
/// minus the write — clean compile exits 0 with NO output file even when
/// -o names one (oracle-measured); elaboration errors still surface
/// (exit 2, same as without -t).
#[test]
fn null_target_compiles_without_output() {
    let work = std::env::temp_dir().join("riv-cli-gate-null");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("tb.v"), "module tb; initial $display(\"hi\"); endmodule\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_riverilog"))
        .args(["-t", "null", "-o", "shouldnotexist.riv", "tb.v"])
        .current_dir(&work)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(out.stdout.is_empty());
    assert!(
        !work.join("shouldnotexist.riv").exists(),
        "-t null must not write the -o file"
    );
    // Elaboration still runs under -t null (oracle: unknown module errors
    // identically with and without -t).
    std::fs::write(work.join("ymiss.v"), "module ymiss; nosuch u(); endmodule\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_riverilog"))
        .args(["-t", "null", "ymiss.v"])
        .current_dir(&work)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let se = String::from_utf8_lossy(&out.stderr);
    assert!(se.contains("nosuch"), "{se}");
}

/// -E: preprocess only (a syntax-error file passes), routed to a.out by
/// default, `-o file` to that file, `-o -` to stdout.
#[test]
fn e_mode_routes_preprocessed_output() {
    let work = std::env::temp_dir().join("riv-cli-gate-e");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("tb.v"), "module tb; initial $display(1); endmodule\n").unwrap();
    // Default: a.out (oracle-measured), stdout silent.
    let out = Command::new(env!("CARGO_BIN_EXE_riverilog"))
        .args(["-E", "tb.v"])
        .current_dir(&work)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(out.stdout.is_empty(), "-E without -o goes to a.out");
    let aout = std::fs::read_to_string(work.join("a.out")).unwrap();
    assert!(aout.contains("module tb;"), "{aout}");
    // -o file.
    let out = Command::new(env!("CARGO_BIN_EXE_riverilog"))
        .args(["-E", "-o", "pp.txt", "tb.v"])
        .current_dir(&work)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(work.join("pp.txt").exists());
    // -o -: stdout (the one legitimate stdout route, rivD4).
    let out = Command::new(env!("CARGO_BIN_EXE_riverilog"))
        .args(["-E", "-o", "-", "tb.v"])
        .current_dir(&work)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("module tb;"));
    assert!(!work.join("-").exists(), "no `-` file from -E -o -");
    // Syntax errors do not stop -E (oracle-measured: exit 0).
    std::fs::write(work.join("bad.v"), "module bad; endmodule nonsense)\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_riverilog"))
        .args(["-E", "bad.v"])
        .current_dir(&work)
        .output()
        .unwrap();
    assert!(out.status.success(), "syntax errors pass -E");
}

/// -D both shapes (P3): attached -DW=8 and separate -D W=8 both reach the
/// design — the old binary reported illegal option / silently dropped the
/// value (measured); oracle prints W=8 for both.
#[test]
fn define_both_shapes_reach_the_design() {
    for (tag, args) in [
        ("d-attached", vec!["-DW=8"]),
        ("d-separate", vec!["-D", "W=8"]),
    ] {
        let work = std::env::temp_dir().join(format!("riv-cli-gate-{tag}"));
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).unwrap();
        std::fs::write(
            work.join("dprobe.v"),
            "module dprobe;\n  localparam W = `W;\n  initial $display(\"W=%0d\", W);\nendmodule\n",
        )
        .unwrap();
        let mut full = args.clone();
        full.extend(["-o", "d.riv", "dprobe.v"]);
        let c = Command::new(env!("CARGO_BIN_EXE_riverilog"))
            .args(&full)
            .current_dir(&work)
            .output()
            .unwrap();
        assert!(c.status.success(), "{tag}: {}", String::from_utf8_lossy(&c.stderr));
        let r = Command::new(env!("CARGO_BIN_EXE_rivvp"))
            .arg("d.riv")
            .current_dir(&work)
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&r.stdout), "W=8\n", "{tag}");
    }
}

/// -o - off -E: hard error (rivD4 fulfilled) — the old binary created a
/// file literally named `-` (measured footgun); the oracle does too, this
/// is a registered policy divergence, not a parity claim.
#[test]
fn output_dash_on_compile_target_is_refused() {
    let work = std::env::temp_dir().join("riv-cli-gate-odash");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("tb.v"), "module tb; initial $display(1); endmodule\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_riverilog"))
        .args(["-o", "-", "tb.v"])
        .current_dir(&work)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let se = String::from_utf8_lossy(&out.stderr);
    assert!(se.contains("rivD4"), "{se}");
    assert!(!work.join("-").exists(), "no `-` file created");
}

/// -i: pointer hard error until U-3b implements missing-instance skips.
#[test]
fn ignore_missing_modules_names_the_arc() {
    let (_, se, rc) = run("riverilog", &["-i", "tb.v"]);
    assert_eq!(rc, 1);
    assert!(se.contains("U-3b"), "{se}");
}

/// -y library search, transitive: tb→mid→leaf with mid+leaf only in the
/// -y dir. Expected stdout recorded from the oracle (vvp), never
/// hand-written.
#[test]
fn y_search_transitive_byte_matches_oracle() {
    let case = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("goldens/y-flow");
    let work = std::env::temp_dir().join("riv-cli-gate-yflow");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    for f in ["tb.v", "libdir/mid.v", "libdir/leaf.v"] {
        let dst = work.join(f);
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        std::fs::copy(case.join(f), dst).unwrap();
    }
    let c = Command::new(env!("CARGO_BIN_EXE_riverilog"))
        .args(["-y", "libdir", "-o", "out.riv", "tb.v"])
        .current_dir(&work)
        .output()
        .unwrap();
    assert!(
        c.status.success(),
        "-y transitive compile: {}",
        String::from_utf8_lossy(&c.stderr)
    );
    let r = Command::new(env!("CARGO_BIN_EXE_rivvp"))
        .arg("out.riv")
        .current_dir(&work)
        .output()
        .unwrap();
    let want = std::fs::read_to_string(case.join("expected.stdout")).unwrap();
    assert_eq!(String::from_utf8_lossy(&r.stdout), want);
}

/// -Y suffix: <mod><suffix> literal concatenation (dot travels inside the
/// suffix — measured); -Y appends to the default [".v"], never replaces.
#[test]
fn y_cap_y_suffix_form_works() {
    let case = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("goldens/y-flow");
    let work = std::env::temp_dir().join("riv-cli-gate-yflowva");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    std::fs::copy(case.join("tb.v"), work.join("tb.v")).unwrap();
    for f in ["mid.va", "leaf.va"] {
        std::fs::copy(case.join("libdirva").join(f), work.join(f)).unwrap();
    }
    let c = Command::new(env!("CARGO_BIN_EXE_riverilog"))
        .args(["-y", ".", "-Y", ".va", "-o", "out.riv", "tb.v"])
        .current_dir(&work)
        .output()
        .unwrap();
    assert!(
        c.status.success(),
        "-Y .va compile: {}",
        String::from_utf8_lossy(&c.stderr)
    );
    let r = Command::new(env!("CARGO_BIN_EXE_rivvp"))
        .arg("out.riv")
        .current_dir(&work)
        .output()
        .unwrap();
    let want = std::fs::read_to_string(case.join("expected-va.stdout")).unwrap();
    assert_eq!(String::from_utf8_lossy(&r.stdout), want);
}

/// Missing module after the -y scan: elaboration class, exit 2 (oracle
/// measured: "Unknown module type" + missing list, exit 2).
#[test]
fn missing_module_after_y_scan_is_exit_2() {
    let case = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("goldens/y-flow");
    let work = std::env::temp_dir().join("riv-cli-gate-ymiss");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    for f in ["tb.v", "libdir/mid.v", "libdir/leaf.v"] {
        let dst = work.join(f);
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        std::fs::copy(case.join(f), dst).unwrap();
    }
    // The search itself must succeed for the libdir case (covered by
    // y_search_transitive_byte_matches_oracle); here point -y at libdir
    // while instantiating a module no library defines.
    std::fs::write(
        work.join("tbmiss.v"),
        "module tbmiss;\n  nosuch u();\nendmodule\n",
    )
    .unwrap();
    let c = Command::new(env!("CARGO_BIN_EXE_riverilog"))
        .args(["-y", "libdir", "-o", "out.riv", "tbmiss.v"])
        .current_dir(&work)
        .output()
        .unwrap();
    assert_eq!(c.status.code(), Some(2), "elaboration class");
    let se = String::from_utf8_lossy(&c.stderr);
    assert!(se.contains("nosuch"), "{se}");
    assert!(!se.contains("CLI driver"), "old hook sentence retired");
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

/// Compile a source string in a fresh work dir; returns (work, exit).
fn compile_src(tag: &str, src: &str) -> (std::path::PathBuf, i32) {
    let work = std::env::temp_dir().join(format!("riv-cli-gate-{tag}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("tb.v"), src).unwrap();
    let c = Command::new(env!("CARGO_BIN_EXE_riverilog"))
        .args(["-o", "out.riv", "tb.v"])
        .current_dir(&work)
        .output()
        .unwrap();
    (work, c.status.code().unwrap_or(-1))
}

fn vp_run(work: &std::path::Path, args: &[&str]) -> (String, String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_rivvp"))
        .args(args)
        .current_dir(work)
        .output()
        .unwrap();
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

const STOP_TB: &str = "\
module st;
  initial begin
    $display(\"before stop\");
    #1 $stop;
    $display(\"after stop\");
  end
endmodule
";
const DUMP_TB: &str = "\
module qd;
  reg a;
  initial begin
    $dumpvars(0, qd);
    a = 0;
    #1 a = 1;
    #1 $display(\"qd ran\");
    $finish;
  end
endmodule
";

/// -N: stop halts like -n but the process exits 1 (measured vvp 13.0).
#[test]
fn rivvp_dash_n_caps_stop_at_exit_1() {
    let (work, rc) = compile_src("vp-n", STOP_TB);
    assert_eq!(rc, 0);
    let (so, _, rc) = vp_run(&work, &["-N", "out.riv"]);
    assert_eq!(rc, 1, "-N exits 1");
    assert!(so.contains("$stop called at"), "{so}");
    assert!(so.contains("before stop"), "{so}");
    // Halt semantics (-n family): no `after stop`, no continue banner.
    assert!(!so.contains("after stop"), "{so}");
    assert!(!so.contains("** Continue **"), "{so}");
}

/// -q: stdout fully silent (VCD info line, design $display, finish line);
/// dump.vcd still written; exit 0.
#[test]
fn rivvp_dash_q_silences_stdout_keeps_vcd() {
    let (work, rc) = compile_src("vp-q", DUMP_TB);
    assert_eq!(rc, 0);
    let (so, _, rc) = vp_run(&work, &["-q", "out.riv"]);
    assert_eq!(rc, 0);
    assert_eq!(so, "", "-q stdout fully silent, got {so}");
    assert!(work.join("dump.vcd").exists(), "dump.vcd still written");
}

/// -l <file>: everything that reaches stdout is mirrored byte-equal into
/// the log (measured: log == direct stdout); stdout still printed.
#[test]
fn rivvp_dash_l_mirrors_stdout() {
    let (work, rc) = compile_src("vp-l", DUMP_TB);
    assert_eq!(rc, 0);
    let (direct, _, _) = vp_run(&work, &["out.riv"]);
    let (so, _, rc) = vp_run(&work, &["-l", "log.txt", "out.riv"]);
    assert_eq!(rc, 0);
    assert_eq!(so, direct, "stdout unchanged with -l");
    let logged = std::fs::read_to_string(work.join("log.txt")).unwrap();
    assert_eq!(logged, direct, "log mirrors stdout byte-equal");
}

/// -l -: oracle contract unsettled (measured dual-stream full-copy, not
/// "to stderr" as the man page claims) — hard error with pointer, rivD14.
#[test]
fn rivvp_dash_l_dash_is_hard_error() {
    let (work, _) = compile_src("vp-ldash", DUMP_TB);
    let (_, se, rc) = vp_run(&work, &["-l", "-", "out.riv"]);
    assert_eq!(rc, 1);
    assert!(se.contains("rivD14"), "{se}");
    assert!(!work.join("-").exists(), "no `-` file created");
    assert!(!work.join("dump.vcd").exists(), "erred before running");
}

/// -M/-m (VPI module face): R7 pointer hard error, no silent swallow.
#[test]
fn rivvp_vpi_flags_name_r7() {
    let (work, _) = compile_src("vp-m", DUMP_TB);
    for args in [["-m", "x.so", "out.riv"], ["-M", "dir", "out.riv"]] {
        let (_, se, rc) = vp_run(&work, &args);
        assert_eq!(rc, 1, "{args:?}");
        assert!(se.contains("R7"), "{args:?}: {se}");
    }
}
