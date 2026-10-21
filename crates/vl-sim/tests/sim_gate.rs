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
    for (name, src) in files {
        let out = pp::preprocess_source(name, src, &mut sm, &mut diags, &opts);
        let parsed = parser::parse_source_file(out.tokens, &mut diags);
        modules.extend(parsed.modules);
    }
    assert!(!diags.has_errors(), "{:#?}", diags.diags);
    let mut sim = Sim::build(&modules, top).expect("elaborate");
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

#[test]
fn always_block_errors_with_milestone_pointer() {
    let design = include_str!("../../../corpus/counter8/counter.v");
    let mut sm = rcs_diag::SourceMap::new();
    let mut diags = rcs_diag::DiagSink::new();
    let opts = PpOptions::default();
    let parsed = parser::parse_source_file(
        pp::preprocess_source("counter.v", design, &mut sm, &mut diags, &opts).tokens,
        &mut diags,
    );
    let e = match Sim::build(&parsed.modules, "counter") {
        Ok(_) => panic!("always must be rejected at elab time in the slice"),
        Err(e) => e,
    };
    assert!(
        e.message.contains("M5"),
        "error must point at the milestone: {}",
        e.message
    );
}
