//! End-to-end simulation gate: run the corpus fixture through the same
//! pipeline the CLI uses and require a byte-exact match with the
//! Verilator-5.050-recorded expected output. This test is the M2 vertical
//! slice's acceptance (PLAN §5); when it fails, do not loosen it — fix the
//! semantics (oracle is the spec).

use vl_syntax::parser;
use vl_syntax::pp::{self, PpOptions};
use vl_sim::Sim;

fn run_case(files: &[(&str, &str)], top: &str) -> (String, Result<(), vl_sim::SimError>) {
    let mut sm = rcs_diag::SourceMap::new();
    let mut diags = rcs_diag::DiagSink::new();
    let opts = PpOptions::default();
    let mut modules = Vec::new();
    let mut timescale: Option<String> = None;
    for (name, src) in files {
        let out = pp::preprocess_source(name, src, &mut sm, &mut diags, &opts);
        if timescale.is_none() {
            timescale = out.timescale.clone();
        }
        let parsed = parser::parse_source_file(out.tokens, &mut diags);
        modules.extend(parsed.modules);
    }
    assert!(!diags.has_errors(), "{:#?}", diags.diags);
    let mut sim = match Sim::build(&modules, top, timescale.as_deref()) {
        Ok(s) => s,
        Err(e) => panic!("elaborate: {}", e.message),
    };
    let r = sim.run();
    (sim.take_output(), r)
}

#[test]
fn fulladd_matches_oracle_bytes() {
    let design = include_str!("../../../corpus/fulladd/fulladd.v");
    let tb = include_str!("../../../corpus/fulladd/tb_fulladd.v");
    let expected = include_str!("../../../corpus/fulladd/expected.stdout");
    let (out, r) = run_case(
        &[("fulladd.v", design), ("tb_fulladd.v", tb)],
        "tb_fulladd",
    );
    r.expect("simulation errors");
    assert_eq!(out, expected, "byte-diff vs Verilator 5.050 oracle failed");
}

/// M5 acceptance (PLAN §5): the counter8 fixture, including the NBA-region
/// read subtlety documented in corpus/counter8/README.md.
#[test]
fn counter8_matches_oracle_bytes() {
    let design = include_str!("../../../corpus/counter8/counter.v");
    let tb = include_str!("../../../corpus/counter8/tb_counter.v");
    let expected = include_str!("../../../corpus/counter8/expected.stdout");
    let (out, r) = run_case(&[("counter.v", design), ("tb_counter.v", tb)], "tb_counter");
    if let Err(e) = r {
        panic!("simulation error: {} (output so far: {out:?})", e.message);
    }
    assert_eq!(out, expected, "byte-diff vs Verilator 5.050 oracle failed");
}
