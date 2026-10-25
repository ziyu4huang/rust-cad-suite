//! riverilog/rivvp face on wasm (ADR-0009: one engine, two faces): the
//! host-target mirror of wasm-test/riv_run.mjs — riv_build must not panic
//! on the per-source timescale path (the empty-files slice once panicked in
//! riv-elab's min_precision.unwrap) and riv_run must reproduce the
//! iverilog-recorded golden byte-exactly modulo the source-path token.

use vl_wasm::{riv_build, riv_run};

#[test]
fn riv_face_hello_matches_iv_golden() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let src = std::fs::read_to_string(format!("{root}/corpus/iv-e2e/01_hello/tb.v")).unwrap();
    let expected = std::fs::read_to_string(format!("{root}/corpus/iv-e2e/01_hello/expected.stdout")).unwrap();
    let riv = riv_build("", vec![src]).unwrap();
    assert!(riv.starts_with("riv ") && riv.contains("\nvars\n"), "riv_build output is not a RIV-1 design");
    let r = riv_run(riv, vec![]).unwrap();
    let got = r.stdout().replace("source_0.v", "tb.v");
    assert_eq!(got, expected, "riv face stdout must match the iverilog golden");
}

// The error path builds a JsValue, which only exists on wasm32 — the
// browser-side rejection is covered by wasm-test/riv_run.mjs usage there.
#[cfg(target_arch = "wasm32")]
#[test]
fn riv_face_rejects_bad_design_text() {
    assert!(riv_run("not a riv design".into(), vec![]).is_err());
}
