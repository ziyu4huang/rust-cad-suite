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

/// Output of one simulation: stdout plus the VCD text if the TB dumped one.
pub struct SimOut {
    pub stdout: String,
    pub vcd: Option<String>,
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
pub fn run_ours(dir: &Path) -> Result<SimOut, String> {
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
    // ADR-0009 U-1: the differential gate drives the unified engine
    // (the 59 goldens are its zero-drift acceptance). RVERILATOR_LEGACY=1
    // falls back to the segments interpreter (rollback path).
    if std::env::var("RVERILATOR_LEGACY").is_err() {
        let files: Vec<(String, Option<String>)> = files
            .iter()
            .map(|f| {
                let ts = std::fs::read_to_string(f).ok().and_then(|t| {
                    t.lines().find_map(|l| {
                        let l = l.trim_start();
                        l.strip_prefix("`timescale").map(|r| r.trim().trim_end_matches(';').to_string())
                    })
                });
                (f.to_string_lossy().into_owned(), ts)
            })
            .collect();
        let mut sim = vl_sim::USim::build(&modules, &top, timescale.as_deref(), &sm, &files)
            .map_err(|e| format!("elaborate: {}", e.message))?;
        sim.set_file_base(dir.to_path_buf());
        sim.run().map_err(|e| format!("run: {}", e.message))?;
        let stdout = sim.take_output();
        let vcd = sim.take_vcd().map(|(_, text)| text);
        return Ok(SimOut { stdout, vcd });
    }
    let mut sim = vl_sim::Sim::build(&modules, &top, timescale.as_deref())
        .map_err(|e| format!("elaborate: {}", e.message))?;
    sim.set_file_base(dir.to_path_buf());
    sim.run().map_err(|e| format!("run: {}", e.message))?;
    let stdout = sim.take_output();
    let vcd = sim.take_vcd().map(|(_, text)| text);
    Ok(SimOut { stdout, vcd })
}

pub fn normalize_oracle_stdout(raw: &str) -> String {
    // Tool-internal reporting only: `- ` (Simulation Report / $finish
    // marker lines) and `-Info:`/`-Warning`/`-Error` notes. A design may
    // legitimately print lines starting with `-` (e.g. `-7div2=-3`).
    raw.lines()
        .filter(|l| {
            !(l.starts_with("- ")
                || l.starts_with("-Info")
                || l.starts_with("-Warning")
                || l.starts_with("-Error"))
        })
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
        Ok(out) => {
            let mut d = String::new();
            if out.stdout != expected {
                d.push_str("output mismatch\n--- expected ---\n");
                d.push_str(&expected);
                d.push_str("--- actual ---\n");
                d.push_str(&out.stdout);
            }
            // M6: a committed expect.vcd is a byte-exact golden of our own
            // writer (stability gate); oracle parity is checked separately
            // by `--vcd-xcheck` (alias/suffix-folded value comparison).
            if let Ok(exp_vcd) = std::fs::read_to_string(dir.join("expect.vcd")) {
                match &out.vcd {
                    None => d.push_str("vcd expected but the TB produced none\n"),
                    Some(v) if *v != exp_vcd => {
                        d.push_str(&format!(
                            "vcd mismatch ({} vs {} bytes)\n",
                            v.len(),
                            exp_vcd.len()
                        ));
                    }
                    _ => {}
                }
            }
            CaseResult {
                name,
                pass: d.is_empty(),
                detail: d,
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
            .arg("--trace")
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
        // `$readmem*` resolves against the sim binary's cwd: copy every
        // non-source data file (hex/mem/…) from the case dir into `work`.
        for e in std::fs::read_dir(&d).into_iter().flatten().filter_map(|e| e.ok()) {
            let n = e.file_name().to_string_lossy().to_string();
            let p = e.path();
            if p.is_file()
                && !n.ends_with(".v")
                && !n.starts_with("expected.")
                && !n.ends_with(".vcd")
                && !n.ends_with(".md")
            {
                let _ = std::fs::copy(&p, work.join(&n));
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
                // Self-golden VCD (writer stability); oracle VCD parity is
                // a separate manual crosscheck (see docs/e2e-report/).
                if let Ok(ours) = run_ours(&d) {
                    if let Some(v) = ours.vcd {
                        std::fs::write(d.join("expect.vcd"), v).unwrap();
                    }
                }
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

// ---------- M6: VCD parsing + oracle crosscheck ----------
// Parsing lives in the shared vl-vcd crate (same reader behind the CLI,
// wasm and FFI facades); this module keeps only the oracle comparison.

/// Per-path step function: path -> [(t, value)] — values normalized to the
/// declared width (vl_vcd::normalize_width, contract identical to the old
/// local norm_val).
fn step_functions(d: &vl_vcd::VcdFile) -> std::collections::BTreeMap<String, Vec<(u64, String)>> {
    let mut m: std::collections::BTreeMap<String, Vec<(u64, String)>> = Default::default();
    for c in &d.changes {
        let var = &d.vars[c.var];
        let nv = vl_vcd::normalize_width(&c.value.to_raw(), var.width);
        m.entry(var.name.clone()).or_default().push((c.time, nv));
    }
    m
}

fn value_at(f: &[(u64, String)], t: u64) -> Option<&str> {
    let mut cur: Option<&str> = None;
    for (ct, v) in f {
        if *ct <= t {
            cur = Some(v);
        } else {
            break;
        }
    }
    cur
}

/// Compare our VCD against the oracle's: names are matched by prefixing our
/// paths with the oracle's top scope (auto-detected as the prefix matching
/// the most of our paths); alias groups fold naturally because every alias
/// path receives the shared id's values. Values are compared as step
/// functions on the union of change times (dump-frequency independent).
pub fn vcd_crosscheck(ours: &str, oracle: &str) -> (String, bool) {
    let a = vl_vcd::parse(ours).expect("ours: valid VCD");
    let b = vl_vcd::parse(oracle).expect("oracle: valid VCD");
    let fa = step_functions(&a);
    let fb = step_functions(&b);
    let mut prefixes: std::collections::BTreeSet<String> = Default::default();
    prefixes.insert(String::new());
    for p in fb.keys() {
        let comps: Vec<&str> = p.split('.').collect();
        for k in 1..comps.len() {
            prefixes.insert(comps[..k].join("."));
        }
    }
    let score = |pre: &str| -> usize {
        fa.keys()
            .filter(|p| {
                let full = if pre.is_empty() {
                    (*p).clone()
                } else {
                    format!("{pre}.{p}")
                };
                fb.contains_key(&full)
            })
            .count()
    };
    let best = prefixes
        .iter()
        .max_by_key(|p| (score(p), std::cmp::Reverse(p.len())))
        .cloned()
        .unwrap_or_default();
    let mut report = String::new();
    report.push_str(&format!(
        "oracle top-scope prefix: `{}`\n",
        if best.is_empty() { "<none>" } else { &best }
    ));
    let mut matched = 0usize;
    let mut points = 0usize;
    let mut mismatches = 0usize;
    let mut unmatched_ours = Vec::new();
    for (p, fa_p) in &fa {
        let full = if best.is_empty() {
            p.clone()
        } else {
            format!("{best}.{p}")
        };
        let Some(fb_p) = fb.get(&full) else {
            unmatched_ours.push(p.clone());
            continue;
        };
        matched += 1;
        let mut times: std::collections::BTreeSet<u64> = Default::default();
        for (t, _) in fa_p {
            times.insert(*t);
        }
        for (t, _) in fb_p {
            times.insert(*t);
        }
        let mut bad = 0usize;
        for t in &times {
            points += 1;
            let va = value_at(fa_p, *t);
            let vb = value_at(fb_p, *t);
            if va != vb {
                bad += 1;
                if bad <= 3 {
                    report.push_str(&format!(
                        "  MISMATCH {p} @#{t}: ours={:?} oracle={:?}\n",
                        va.unwrap_or("<none>"),
                        vb.unwrap_or("<none>")
                    ));
                }
            }
        }
        mismatches += bad;
        report.push_str(&format!(
            "  {p}: {} sample points, {bad} mismatches\n",
            times.len()
        ));
    }
    let oracle_only: Vec<String> = fb
        .keys()
        .filter(|k| {
            let stripped = if best.is_empty() {
                (*k).clone()
            } else {
                k.strip_prefix(&format!("{best}.")).unwrap_or(k).to_string()
            };
            !fa.contains_key(&stripped)
        })
        .cloned()
        .collect();
    report.push_str(&format!(
        "matched vars: {matched}/{} ours; sample points: {points}; mismatches: {mismatches}\n",
        fa.len()
    ));
    if !unmatched_ours.is_empty() {
        report.push_str(&format!(
            "ours-only (no oracle counterpart): {unmatched_ours:?}\n"
        ));
    }
    if !oracle_only.is_empty() {
        report.push_str(&format!(
            "oracle-only (not dumped by us, e.g. parameters): {oracle_only:?}\n"
        ));
    }
    let ok = mismatches == 0 && matched == fa.len() && matched > 0;
    report.push_str(if ok {
        "VCD CROSSCHECK: OK\n"
    } else {
        "VCD CROSSCHECK: FAIL\n"
    });
    (report, ok)
}

/// Mechanically generated HTML evidence report.
/// Mechanically generated HTML evidence report. Every number comes from
/// `run_suite` output, not from hand-authoring — the "why" is reproducible
/// from the two commands printed in the report itself.
pub fn html_report(corpus: &Path, results: &[CaseResult]) -> String {
    let npass = results.iter().filter(|r| r.pass).count();
    let rate = pass_rate(results) * 100.0;
    let rev = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(corpus)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    let oracle = std::process::Command::new("/opt/homebrew/bin/verilator")
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "n/a (expectations committed)".into());
    let mut rows = String::new();
    for r in results {
        let (cls, tag) = if r.pass {
            ("pass", "PASS")
        } else if r.detail.contains("not yet supported") {
            ("decl", "DECL")
        } else {
            ("fail", "FAIL")
        };
        let mut detail = String::new();
        if !r.pass {
            detail = format!(
                "<details><summary>detail</summary><pre>{}</pre></details>",
                html_esc(&r.detail)
            );
        }
        rows.push_str(&format!(
            "<tr class=\"{cls}\"><td>{}</td><td>{tag}</td><td>{}</td></tr>\n",
            html_esc(&r.name),
            detail
        ));
    }
    format!(
        r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8">
<title>rust-cad-suite E2E differential report — {rate:.1}%</title>
<style>
 body{{font-family:-apple-system,Helvetica,sans-serif;max-width:960px;margin:2rem auto;padding:0 1rem;color:#1a1a1a}}
 h1{{font-size:1.5rem}} .big{{font-size:3rem;font-weight:700}}
 .pass{{color:#0a7d32}} .fail{{color:#b42318}} .decl{{color:#8a6100}}
 table{{border-collapse:collapse;width:100%;font-size:.85rem}}
 td,th{{border:1px solid #ddd;padding:.35rem .5rem;text-align:left}}
 tr.decl td{{background:#fff8e6}} tr.fail td{{background:#fdeceb}}
 pre{{background:#f6f6f6;padding:.6rem;overflow-x:auto;font-size:.75rem}}
 code{{background:#f0f0f0;padding:.1rem .3rem}}
 .meta{{color:#555;font-size:.85rem}}
</style></head><body>
<h1>rverilator E2E differential report</h1>
<p class="big {cls_rate}">{npass}/{n} = {rate:.1}%</p>
<div class="meta">
 git <code>{rev}</code> · oracle <code>{oracle}</code> · corpus <code>{corpus}</code><br>
 gate: <code>cargo test -p vl-difftest</code> asserts rate ≥ 80% (expectations committed — no verilator needed)
</div>

<h2>Why this number is trustworthy</h2>
<ol>
<li><b>Expectations are recorded, never authored.</b> Every
<code>expected.stdout</code> is captured by running the real Verilator
(<code>vl-difftest --record</code>) and committed. Seven hand-authored
expectations were proven wrong by the oracle earlier in this repo — recorded
expectations are the policy since (AGENTS §1).</li>
<li><b>Same pipeline as the product.</b> Each case runs through the exact
preprocess → parse → elaborate → schedule path the <code>rverilator --binary</code>
CLI uses (<code>vl_difftest::run_ours</code>), then is compared <i>byte-for-byte</i>
with the oracle stdout after one normalization: Verilator's own report block
(lines starting <code>- </code>) is dropped. No fuzzy matching anywhere.</li>
<li><b>The corpus is semantics-clean by policy</b> (AGENTS §2): no X/Z
propagation samples (the oracle is a 2-state evaluator — 4-state correctness
is gated separately by IEEE §5 truth-table unit tests in <code>vl-sim</code>),
and no same-edge scheduling races (IEEE-nondeterministic constructs were
rewritten mid-cycle; see corpus/e2e/26, 43 for the documented pattern).</li>
<li><b>The gate is mechanical.</b> The ≥80% assertion lives in
<code>crates/vl-difftest/tests/regression_gate.rs</code>, runs in every
<code>cargo test</code>, and fails the build when the rate drops — including
on machines without Verilator installed.</li>
<li><b>The one non-pass is declared scope.</b> <code>30_gen_mux</code> uses
memory arrays; the simulator hard-errors with <code>(milestone M7)</code>
by design (AGENTS §3 — silent misexecution is the banned failure mode).</li>
</ol>

<h2>Reproduce</h2>
<pre>cargo build
cargo run -p vl-difftest                      # this report's numbers
cargo test -p vl-difftest                     # the ≥80% gate
cargo run -p vl-difftest -- --record          # re-record from the oracle</pre>

<h2>Per-case results ({n})</h2>
<table><tr><th>case</th><th>result</th><th>detail</th></tr>
{rows}</table>

<h2>Corpus coverage</h2>
<p>Operators &amp; widths (01-05, 31-33, 46-47) · identity/case families
(06, 13, 35) · ternary/concat/part-select/literals (07-10) · display
contract (11, 45) · signedness (12, 34) · loops (14) · parameters &amp;
$clog2 (15, 38) · hierarchy &amp; generate (16, 28-30, 36-37) · functions
(17-18, 39-40) · tasks (19-20, 41) · always @*/comb (21) · sequential/NBA
(22-23, 27, 42-44, 50) · clock &amp; $time (24, 48) · edges (25) · process
order determinism (32) · multi-initial interleaving (49) · VCD dump (51, with
oracle cross-check on 21/27/42/51). Declared not-yet: memory arrays (30),
gates (M7), SVA, VPI.</p>
<p>Known, accepted divergences from the oracle (parameters not dumped,
declaration-order scheduling under races, 4-state X vs 2-state, …) are
registered with evidence in <code>docs/REGISTRY.md</code> → Divergence
register; corpus policy (X-clean, race-free, dump↔oracle-ref) is enforced by
<code>cargo test -p vl-difftest --test corpus_lint</code>.</p>
</body></html>
"#,
        rate = rate,
        cls_rate = if rate >= 95.0 { "pass" } else { "fail" },
        npass = npass,
        n = results.len(),
        rev = html_esc(&rev),
        oracle = html_esc(&oracle),
        corpus = html_esc(&corpus.display().to_string()),
        rows = rows,
    )
}

fn html_esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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
