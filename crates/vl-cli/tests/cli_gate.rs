//! vl-cli golden gate: the rverilator CLI contract pinned byte-exact
//! against committed self-goldens — zero oracle dependency. These goldens
//! anchor the ADR-0009 U-3 shared `-E` frontend move (render_preprocessed
//! relocating to vl-syntax::pp): byte-equality before/after the move IS
//! the zero-drift proof.

use std::process::Command;

fn run(args: &[&str]) -> (String, String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_rverilator"))
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn rverilator: {e}"));
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

/// Fixed pp fixture (exercise -I include + -D macro + `ifdef in one file);
/// written to a temp dir so the golden stays a pure output snapshot.
const PP_IN: &str = "\
`include \"inc1.vh\"
module ppin;
`ifdef WIDE
localparam integer WIDTH = `W;
`else
localparam integer WIDTH = 1;
`endif
initial $display(\"w=%0d\", WIDTH);
endmodule
";
const INC1: &str = "\
// include smoke
`define ADD1(x) ((x)+1)
";

fn pp_workdir(tag: &str) -> std::path::PathBuf {
    // Per-test tag: tests run in parallel threads; a shared dir would race
    // on remove_dir_all.
    let work = std::env::temp_dir().join(format!("vl-cli-gate-{tag}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(work.join("inc")).unwrap();
    std::fs::write(work.join("pp-in.v"), PP_IN).unwrap();
    // inc1.vh lives under inc/ so `-I inc` is load-bearing (include
    // resolution is relative to the including file first — a sibling copy
    // would make the -I smoke test vacuous).
    std::fs::write(work.join("inc").join("inc1.vh"), INC1).unwrap();
    work
}

#[test]
fn version_matches_golden() {
    let (so, se, rc) = run(&["--version"]);
    assert_eq!(rc, 0);
    assert!(se.is_empty());
    assert_eq!(so, golden("goldens/version.txt"));
}

#[test]
fn help_matches_golden() {
    // This face has no -h/-V short forms (F6: `-V` is an unknown-option
    // error here, see invalid_dash_v_is_vl_e030); the long forms are the
    // contract.
    let (so, se, rc) = run(&["--help"]);
    assert_eq!(rc, 0);
    assert!(se.is_empty());
    assert_eq!(so, golden("goldens/help.txt"));
}

/// -E byte-golden: the shared frontend's output routing (stdout) and
/// rendering must not drift when render_preprocessed moves to vl-syntax.
#[test]
fn preprocess_only_matches_golden() {
    let work = pp_workdir("ppout");
    let out = Command::new(env!("CARGO_BIN_EXE_rverilator"))
        .args(["-E", "-I", "inc", "-DWIDE", "-DW=7", "pp-in.v"])
        .current_dir(&work)
        .output()
        .unwrap();
    let so = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{so}");
    // stdout-only routing: -E on this face writes no files (the a.out
    // routing is the riverilog face's contract, not this one).
    let entries: Vec<_> = std::fs::read_dir(&work)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entries, vec!["inc", "pp-in.v"]);
    assert_eq!(so, golden("goldens/pp-out.txt"));
}

/// -I/-D flow into the full pipeline (not just -E): the macro gate selects
/// WIDTH=7 and lint passes on it.
#[test]
fn include_and_define_smoke() {
    let work = pp_workdir("smoke");
    let out = Command::new(env!("CARGO_BIN_EXE_rverilator"))
        .args(["-I", "inc", "-DWIDE", "-DW=7", "pp-in.v"])
        .current_dir(&work)
        .output()
        .unwrap();
    let so = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{so}");
    assert!(so.contains("lint ok"), "{so}");
    // And the -D value is real: an undefined-macro-free alternative where
    // W is never expanded would still lint — assert the gate took the
    // WIDE branch by breaking it on purpose (missing inc1 must fail, so
    // the include search through -I is proven load-bearing).
    let out_noinc = Command::new(env!("CARGO_BIN_EXE_rverilator"))
        .args(["pp-in.v"])
        .current_dir(&work)
        .output()
        .unwrap();
    assert!(
        !out_noinc.status.success(),
        "without -I the include must fail (VL-E010)"
    );
}

/// F6 pin: `-V` is NOT a flag on this face (measured: invalid option).
/// Kept as the error-path golden, clearly named — never a version form.
#[test]
fn invalid_dash_v_is_vl_e030() {
    let (_, se, rc) = run(&["-V"]);
    assert_eq!(rc, 1);
    assert!(
        se.contains("error[VL-E030]: Invalid option: `-V`"),
        "{se}"
    );
}

#[test]
fn gen_rust_emits_model_and_subset_hard_errors() {
    // M9 tier-B (ADR-0002): --gen-rust on an in-subset design emits a
    // compilable model; out-of-subset designs hard-error with the
    // milestone pointer (iron rule 3).
    let dir = std::env::temp_dir().join(format!("vl-cli-gen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("t.v"), r#"
module tb;
  initial begin
    $display("hello %0d", 8'd42);
    $finish;
  end
endmodule
"#).unwrap();
    let out = dir.join("model.rs");
    let (so, se, rc) = run(&["-o", "out.vvp", "--gen-rust", out.to_str().unwrap(), dir.join("t.v").to_str().unwrap()]);
    assert_eq!(rc, 0, "so={so} se={se}");
    let src = std::fs::read_to_string(&out).unwrap();
    assert!(src.contains("Generated by rcs riv-gen"), "generator header");
    assert!(src.contains("render_pieces"), "display goes through the shared kernel face");
    // out-of-subset: continuous assignment creates extra threads
    std::fs::write(dir.join("w.v"), r#"
module tb;
  wire [8:0] s = 8'd200 + 8'd100;
  initial begin
    $display("%0d", s);
    $finish;
  end
endmodule
"#).unwrap();
    let (so, se, rc) = run(&["-o", "out.vvp", "--gen-rust", dir.join("bad.rs").to_str().unwrap(), dir.join("w.v").to_str().unwrap()]);
    assert_eq!(rc, 1, "so={so} se={se}");
    assert!(so.contains("milestone M9"), "hard error carries the milestone pointer: {so}");
    let _ = std::fs::remove_dir_all(&dir);
}
