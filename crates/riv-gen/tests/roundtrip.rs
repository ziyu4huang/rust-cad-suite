//! M9 tier-B round-trip gate (ADR-0002 §5): for each in-subset corpus
//! case — gen → rustc → run, stdout must be byte-exact vs the oracle-
//! recorded expected.stdout (differential oracle = the interpreter).
//! Out-of-subset cases must hard-error with the milestone pointer.

use std::path::{Path, PathBuf};
use std::process::Command;

const CASES: &[&str] = &[
    "01_arith_widths",
    "02_div_mod_signed",
    "03_pow_shifts",
    "04_bitwise_reduce",
    "05_logical_cmp",
    "06_case_identity",
    "07_ternary",
    "08_concat_lvalue",
    "09_part_select",
    "10_literals",
    "11_display_formats",
    "12_signed_ops",
    "31_wide64",
    "45_display_esc",
    "46_signed_shift",
    "47_concat_nest",
    "60_wide_values",
    "32_proc_order",
    "49_multi_initial",
    "34_cmp_signed_mixed",
    "21_always_comb",
    "22_counter_nba",
    "23_nba_vs_blocking",
    "24_clock_time",
    "25_edge_or",
    "26_fsm_casez",
    "27_shift_reg",
    "42_nba_handshake",
    "43_counter_load",
    "44_lfsr",
    "48_frac_delay",
    "50_forever_finish",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn elaborate_case(case: &str) -> Result<riv_ir::Design, String> {
    let tb = root().join(format!("corpus/e2e/{case}/tb.v"));
    let src = std::fs::read_to_string(&tb).map_err(|e| e.to_string())?;
    let mut sm = rcs_diag::SourceMap::new();
    let mut diags = rcs_diag::DiagSink::new();
    let opts = vl_syntax::pp::PpOptions::default();
    let out = vl_syntax::pp::preprocess_source("tb.v", &src, &mut sm, &mut diags, &opts);
    let timescale = out.timescale.clone();
    let parsed = vl_syntax::parser::parse_source_file(out.tokens, &mut diags);
    if diags.has_errors() {
        return Err("parse errors".into());
    }
    let file_ts = vec![riv_elab::FileTs {
        path: "tb.v".into(),
        timescale,
    }];
    riv_elab::elaborate(riv_elab::ElabInput::strict(
        &parsed.modules,
        &sm,
        Some(parsed.modules[0].name.as_str()),
        &file_ts,
    ))
    .map_err(|e| e.message)
}

#[test]
fn gen_rust_roundtrip_on_subset() {
    let target = root().join("target/debug");
    let mut in_subset = Vec::new();
    let mut failures = Vec::new();
    for case in CASES {
        let design = match elaborate_case(case) {
            Ok(d) => d,
            Err(_) => continue, // case not parseable standalone (module structure) — skip
        };
        let src = match riv_gen::gen(&design) {
            Ok(s) => s,
            Err(e) => {
                assert!(e.contains("milestone M9"), "{case}: bad error: {e}");
                continue;
            }
        };
        in_subset.push(*case);
        let dir = std::env::temp_dir().join(format!("riv-gen-{}-{case}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let rs = dir.join("model.rs");
        std::fs::write(&rs, &src).unwrap();
        let bin = dir.join("model");
        let c = Command::new("rustc")
            .arg("--edition").arg("2021")
            .arg("--extern").arg(format!("vl_values={}", target.join("libvl_values.rlib").display()))
            .arg("--extern").arg(format!("riv_ir={}", target.join("libriv_ir.rlib").display()))
            .arg("--extern").arg(format!("riv_kernel={}", target.join("libriv_kernel.rlib").display()))
            .arg("--extern").arg(format!("vl_ops={}", target.join("libvl_ops.rlib").display()))
            .arg("-L").arg(format!("dependency={}", target.display()))
            .arg(&rs).arg("-o").arg(&bin)
            .output()
            .expect("rustc");
        if !c.status.success() {
            failures.push(format!("== {case}: rustc failed\n{}", String::from_utf8_lossy(&c.stderr)));
            continue;
        }
        // Per-case watchdog (iron rule 4): a generated model that loops
        // forever must fail the case, not hang the gate.
        let mut child = Command::new(&bin)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("run model");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut hung = false;
        let mut buf = Vec::new();
        loop {
            match child.try_wait().expect("wait") {
                Some(st) => {
                    let _ = st;
                    if let Some(mut p) = child.stdout.take() {
                        use std::io::Read;
                        let _ = p.read_to_end(&mut buf);
                    }
                    break;
                }
                None if std::time::Instant::now() > deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    hung = true;
                    break;
                }
                None => std::thread::sleep(std::time::Duration::from_millis(50)),
            }
        }
        if hung {
            failures.push(format!("== {case}: model hang (10s watchdog)"));
            let _ = std::fs::remove_dir_all(&dir);
            continue;
        }
        let got = String::from_utf8_lossy(&buf).to_string();
        let want = std::fs::read_to_string(root().join(format!("corpus/e2e/{case}/expected.stdout")))
            .unwrap_or_default();
        if got != want {
            failures.push(format!(
                "== {case}: output mismatch\n--- got ---\n{got}--- want ---\n{want}"
            ));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
    // The subset is pinned: slice-1 must cover at least the pure-main
    // cases; widen as slices land (regress-visible counter).
    eprintln!("in-subset cases: {in_subset:?}");
    // Pinned coverage = the ADR-0002 slice-2 subset (N initial threads,
    // straight-line segments, Delay suspension, delayed NBAs). Widen as
    // slices land; regress-visible.
    assert_eq!(
        in_subset,
        [
            "02_div_mod_signed",
            "03_pow_shifts",
            "04_bitwise_reduce",
            "05_logical_cmp",
            "06_case_identity",
            "07_ternary",
            "10_literals",
            "11_display_formats",
            "12_signed_ops",
            "31_wide64",
            "45_display_esc",
            "46_signed_shift",
            "47_concat_nest",
            "32_proc_order",
            "49_multi_initial",
            "34_cmp_signed_mixed",
            "23_nba_vs_blocking",
            "25_edge_or",
        ],
        "slice-3 subset coverage changed"
    );
    assert!(
        failures.is_empty(),
        "round-trip failures:\n{}",
        failures.join("\n")
    );
}
