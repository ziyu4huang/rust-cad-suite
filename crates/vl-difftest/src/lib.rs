//! E2E differential-test library.
//!
//! Contract (mirrors `corpus/counter8/README.md`):
//! - Each case = one directory under `corpus/e2e/` with one or more `.v`
//!   files; top module auto-detected (the un-instantiated one).
//! - `expected.stdout` = ORACLE stdout after normalization (drop lines
//!   starting `- ` — Verilator's Simulation Report block). Committed, so the
//!   regression gate never needs verilator installed.
//! - `--record` re-runs the real Verilator to (re)generate expectations.
//!   Byte-diff corpus stays X-clean by policy (ADR-0001); 4-state operator
//!   correctness is covered by value.rs unit tests instead.

use rcs_diag::{DiagSink, SourceMap};
use std::path::{Path, PathBuf};
use vl_syntax::ast::ModuleItem;
use vl_syntax::parser;
use vl_syntax::pp::{self, PpOptions};

pub struct CaseResult {
    pub name: String,
    pub pass: bool,
    pub detail: String,
}

pub fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/e2e")
}

fn collect_v_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "v" || x == "sv").unwrap_or(false))
        .collect();
    v.sort();
    v
}

/// Run one case through the same pipeline as `rverilator --binary`.
/// Returns the normalized stdout (or an error message).
pub fn run_ours(dir: &Path) -> Result<String, String> {
    let files = collect_v_files(dir);
    if files.is_empty() {
        return Err(format!("no .v files in {}", dir.display()));
    }
    let mut sm = SourceMap::new();
    let mut diags = DiagSink::new();
    let opts = PpOptions::default();
    let mut modules = Vec::new();
    let mut timescale: Option<String> = None;
    for f in &files {
        let Some(out) = pp::preprocess_file(f, &mut sm, &mut diags, &opts) else {
            return Err(format!("preprocess failed: {}", f.display()));
        };
        if timescale.is_none() {
            timescale = out.timescale.clone();
        }
        let parsed = parser::parse_source_file(out.tokens, &mut diags);
        modules.extend(parsed.modules);
    }
    if diags.has_errors() {
        let mut out = String::new();
        rcs_diag::render_diags(&mut out, &sm, &diags.diags);
        return Err(format!("parse errors:\n{out}"));
    }
    let instantiated: std::collections::HashSet<String> = modules
        .iter()
        .flat_map(|m| m.items.iter())
        .filter_map(|it| match it {
            ModuleItem::Instance(i) => Some(i.mod_name.clone()),
            _ => None,
        })
        .collect();
    let cands: Vec<String> = modules
        .iter()
        .map(|m| m.name.clone())
        .filter(|n| !instantiated.contains(n))
        .collect();
    let top = match cands.as_slice() {
        [only] => only.clone(),
        _ => return Err(format!("top auto-detect failed: {cands:?}")),
    };
    let mut sim = vl_sim::Sim::build(&modules, &top, timescale.as_deref())
        .map_err(|e| format!("elaborate: {}", e.message))?;
    sim.run().map_err(|e| format!("run: {}", e.message))?;
    Ok(sim.take_output())
}

pub fn normalize_oracle_stdout(raw: &str) -> String {
    raw.lines()
        .filter(|l| !l.starts_with("- "))
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end_matches('\n')
        .to_string()
        + "\n"
}

pub fn check_case(dir: &Path) -> CaseResult {
    let name = dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let expected_path = dir.join("expected.stdout");
    let Ok(expected) = std::fs::read_to_string(&expected_path) else {
        return CaseResult {
            name,
            pass: false,
            detail: "missing expected.stdout (record first)".into(),
        };
    };
    match run_ours(dir) {
        Err(e) => CaseResult {
            name,
            pass: false,
            detail: e,
        },
        Ok(actual) => {
            if actual == expected {
                CaseResult {
                    name,
                    pass: true,
                    detail: String::new(),
                }
            } else {
                let mut d = String::from("output mismatch\n--- expected ---\n");
                d.push_str(&expected);
                d.push_str("--- actual ---\n");
                d.push_str(&actual);
                CaseResult {
                    name,
                    pass: false,
                    detail: d,
                }
            }
        }
    }
}

pub fn run_suite(corpus: &Path) -> Vec<CaseResult> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(corpus)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs.iter().map(|d| check_case(d)).collect()
}

pub fn pass_rate(results: &[CaseResult]) -> f64 {
    if results.is_empty() {
        return 0.0;
    }
    results.iter().filter(|r| r.pass).count() as f64 / results.len() as f64
}

/// Record mode: regenerate `expected.stdout` for every case using the real
/// Verilator (--binary --timing). Fails a case whose oracle run errors.
pub fn record_with_oracle(corpus: &Path, verilator: &Path) -> anyhow_none::Result {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(corpus)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    let mut failures = Vec::new();
    for d in dirs {
        let name = d.file_name().unwrap().to_string_lossy().to_string();
        let files = collect_v_files(&d);
        let work = std::env::temp_dir().join(format!("vl-record-{name}"));
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).unwrap();
        let out_path = work.join("obj").join("sim");
        let status = std::process::Command::new(verilator)
            .arg("--binary")
            .arg("--timing")
            .arg("--Mdir")
            .arg(work.join("obj"))
            .arg("-o")
            .arg("sim")
            .args(&files)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .status();
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                failures.push(format!("{name}: verilator build exit {s}"));
                continue;
            }
            Err(e) => {
                failures.push(format!("{name}: {e}"));
                continue;
            }
        }
        let run = std::process::Command::new(&out_path)
            .current_dir(&work)
            .output();
        match run {
            Ok(o) if o.status.success() => {
                let raw = String::from_utf8_lossy(&o.stdout).to_string();
                std::fs::write(d.join("expected.oracle-stdout.txt"), &raw).unwrap();
                std::fs::write(d.join("expected.stdout"), normalize_oracle_stdout(&raw)).unwrap();
            }
            Ok(o) => {
                failures.push(format!(
                    "{name}: sim exit {:?} stdout={:?}",
                    o.status.code(),
                    String::from_utf8_lossy(&o.stdout)
                ));
            }
            Err(e) => failures.push(format!("{name}: {e}")),
        }
        let _ = std::fs::remove_dir_all(&work);
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(anyhow_none::ErrList(failures))
    }
}

/// Tiny local error type (zero deps).
pub mod anyhow_none {
    pub struct ErrList(pub Vec<String>);
    impl std::fmt::Debug for ErrList {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    pub type Result = std::result::Result<(), ErrList>;
}
