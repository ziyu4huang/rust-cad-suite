//! riv-difftest — the riverilog differential harness (design doc §6/§10
//! runner contract; vl-difftest is the shape donor, iverilog/vvp the
//! recording oracle).
//!
//! Contracts (all measured; see docs/riv-r2-design.md §0/§6/§10):
//!   riv-difftest --record [--corpus DIR]
//!     Recording — needs the real iverilog/vvp; writes expected.stdout/
//!     expected.exit per iv case (cwd = work dir with the case files).
//!   riv-difftest --e2e --corpus corpus/e2e
//!     Per e2e case (dir with tb.v + expected.stdout): riverilog → rivvp in
//!     a work dir with data-file copies, per-case watchdog, stdout
//!     normalized (whitelist drop lines only — see `normalize_e2e_stdout`)
//!     and byte-diffed. Compile/runtime hard-errors carrying a
//!     `milestone R` pointer classify as `pending` (DECL marker); a byte
//!     diff without a riv-pending.txt ledger is `UNLEDGERED`; a ledgered
//!     case that passes is `DRIFT`.
//!   riv-difftest [--e2e] --corpus corpus/iv-e2e
//!     iv-e2e cases honor case.json: enforced = raw stdout+exit byte-diff
//!     (incl. plusargs); pending = must hard-error with the milestone
//!     pointer, and running anyway is `DRIFT: ledger stale`. The arm is
//!     selected by the corpus path tail (§10's literal second invocation is
//!     `--e2e --corpus corpus/iv-e2e`), never by the `--e2e` flag alone.
//!   Output: one line per case `<corpus>\t<case>\t<status>\t<detail>`,
//!     final `SUMMARY e2e=<p>/<n> iv=<p>/<n>`; exit 0 regardless — the
//!     enforcement lives in scripts/regress.sh greps (exact baselines,
//!     UNLEDGERED==0, DRIFT==0).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Debug)]
struct Case {
    name: String,
    dir: PathBuf,
    files: Vec<String>,
    g: String,
    plusargs: Vec<String>,
    enforced: bool,
}

fn load_cases(corpus: &Path) -> Vec<Case> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(corpus)
        .expect("corpus dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs.into_iter()
        .map(|d| {
            let name = d.file_name().unwrap().to_string_lossy().into_owned();
            let manifest: serde_json_lenient::Manifest =
                parse_manifest(&d.join("case.json"));
            Case {
                name,
                enforced: manifest.enforced,
                files: manifest.files,
                g: manifest.g,
                plusargs: manifest.plusargs,
                dir: d,
            }
        })
        .collect()
}

/// Minimal manifest parser: flat JSON object with known keys (hand-rolled —
/// the crates stay zero-dependency per the workspace policy).
fn parse_manifest(p: &Path) -> serde_json_lenient::Manifest {
    let text = std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let get_str_list = |key: &str| -> Vec<String> {
        let pat = format!("\"{key}\"");
        let rest = text.split(pat.as_str()).nth(1).unwrap_or(":[");
        let start = rest.find('[').unwrap_or(0);
        let end = rest[start..].find(']').map(|e| start + e).unwrap_or(start);
        rest[start..end]
            .split('"')
            .skip(1)
            .step_by(2)
            .map(|s| s.to_string())
            .collect()
    };
    let g = text
        .split("\"g\"")
        .nth(1)
        .and_then(|r| r.split('"').nth(1))
        .unwrap_or("2005")
        .to_string();
    let enforced = text
        .split("\"enforced\"")
        .nth(1)
        .map(|r| r.trim_start_matches(|c: char| c != 't' && c != 'f').starts_with('t'))
        .unwrap_or(false);
    serde_json_lenient::Manifest {
        files: get_str_list("files"),
        plusargs: get_str_list("plusargs"),
        g,
        enforced,
    }
}

mod serde_json_lenient {
    pub struct Manifest {
        pub files: Vec<String>,
        pub plusargs: Vec<String>,
        pub g: String,
        pub enforced: bool,
    }
}

// ---------------------------------------------------------------------------
// Watchdog process runner
// ---------------------------------------------------------------------------

struct ProcOut {
    stdout: String,
    stderr: String,
    code: Option<i32>,
    watchdog: bool,
}

/// Per-case wall-clock watchdog (design §6-4): default 60 s, overridable
/// via RIV_CASE_TIMEOUT_MS; expiry kills the child → `watchdog`.
fn run_with_watchdog(mut cmd: Command, timeout: Duration) -> ProcOut {
    use std::io::Read;
    use std::process::Stdio;
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return ProcOut {
                stdout: String::new(),
                stderr: format!("spawn failed: {e}"),
                code: None,
                watchdog: false,
            }
        }
    };
    let start = Instant::now();
    loop {
        match child.try_wait().expect("try_wait") {
            Some(status) => {
                let mut out = String::new();
                let mut err = String::new();
                if let Some(mut s) = child.stdout.take() {
                    let _ = s.read_to_string(&mut out);
                }
                if let Some(mut s) = child.stderr.take() {
                    let _ = s.read_to_string(&mut err);
                }
                return ProcOut {
                    stdout: out,
                    stderr: err,
                    code: status.code(),
                    watchdog: false,
                };
            }
            None => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return ProcOut {
                        stdout: String::new(),
                        stderr: String::new(),
                        code: None,
                        watchdog: true,
                    };
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

fn watchdog_timeout() -> Duration {
    Duration::from_millis(
        std::env::var("RIV_CASE_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60_000),
    )
}

// ---------------------------------------------------------------------------
// Work-dir contract (design §6-1): copy sources + data files; expectation
// and manifest files stay behind.
// ---------------------------------------------------------------------------

fn stage_work_dir(case_dir: &Path, tag: &str, name: &str) -> PathBuf {
    let work = std::env::temp_dir().join(format!("{tag}-{}", name));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(case_dir)
        .expect("case dir")
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    for p in entries {
        if !p.is_file() {
            continue;
        }
        let fname = p.file_name().unwrap().to_string_lossy().into_owned();
        let is_expectation = fname.starts_with("expected.")
            || fname == "case.json"
            || fname == "riv-pending.txt"
            || fname.ends_with(".vcd");
        if is_expectation {
            continue;
        }
        std::fs::copy(&p, work.join(&fname)).unwrap();
    }
    work
}

/// Source glob: every .v/.sv staged in the work dir, sorted for
/// determinism (design §6-2; 57/58/59's dual sources covered).
fn source_files(work: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(work)
        .expect("work dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .map(|x| x == "v" || x == "sv")
                    .unwrap_or(false)
        })
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

// ---------------------------------------------------------------------------
// Normalization (design §6-5 / §10 reconciliation note 1): drop-line
// whitelist only, NO content rewriting. vl-difftest's four exact prefixes
// ∪ the riv tool-report shapes. Hostile fixtures in the test module below.
// ---------------------------------------------------------------------------

fn is_riv_tool_line(l: &str) -> bool {
    // `<file>:<line>: $finish called at ...` — the iverilog-shaped finish
    // banner rivvp prints; e2e goldens (recorded through vl-difftest) have
    // no finish line, so the runner drops ours. A design line that merely
    // starts with `<file>:<line>: ` survives — only the exact finish
    // banner matches.
    if let Some(rest) = strip_finish_prefix(l) {
        // The banner shape is `<file>:<line>: $finish called at ` — one
        // space between the second colon and the banner.
        if rest.trim_start_matches(' ').starts_with("$finish called at") {
            return true;
        }
    }
    l.starts_with("VCD info: ")
}

fn strip_finish_prefix(l: &str) -> Option<&str> {
    // `^[^: ]+:[0-9]+: ` — file:line: prefix, no spaces in the file part.
    let colon = l.find(':')?;
    let file = &l[..colon];
    if file.is_empty() || file.contains(' ') {
        return None;
    }
    let rest = &l[colon + 1..];
    let colon2 = rest.find(':')?;
    let line_no = &rest[..colon2];
    if line_no.is_empty() || !line_no.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(&rest[colon2 + 1..])
}

fn normalize_e2e_stdout(raw: &str) -> String {
    raw.lines()
        .filter(|l| {
            !(l.starts_with("- ")
                || l.starts_with("-Info")
                || l.starts_with("-Warning")
                || l.starts_with("-Error")
                || is_riv_tool_line(l))
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

// ---------------------------------------------------------------------------
// Arms
// ---------------------------------------------------------------------------

struct CaseLine {
    name: String,
    status: &'static str,
    detail: String,
}

fn one_line(s: &str) -> String {
    // Prefer the line carrying the milestone pointer (multi-line diags end
    // with a summary that loses it); else the last non-empty line.
    let t: String = s
        .lines()
        .filter(|l| l.contains("milestone"))
        .next_back()
        .or_else(|| s.lines().rev().find(|l| !l.trim().is_empty()))
        .unwrap_or("")
        .to_string();
    let t = t.replace('\t', " ");
    if t.len() > 160 {
        format!("{}…", &t[..160])
    } else {
        t
    }
}

fn is_decl(code: Option<i32>, stderr: &str) -> bool {
    // Expected-fail (DECL) classifier: hard error carrying the milestone
    // pointer. Both message shapes match — the parenless R0 form
    // (`lands at milestone R1`) and the §5-5 paren form
    // (`(milestone R4)`) — the substring is the contract (design §6).
    code == Some(2) && stderr.contains("milestone R")
}

fn ledger_text(case_dir: &Path) -> Option<String> {
    std::fs::read_to_string(case_dir.join("riv-pending.txt"))
        .ok()
        .map(|t| t.lines().next().unwrap_or("").to_string())
}

/// e2e arm (design §6-1..6).
fn run_e2e(corpus: &Path, riverilog: &Path, rivvp: &Path) -> Vec<CaseLine> {
    let timeout = watchdog_timeout();
    let mut lines = Vec::new();
    for d in std::fs::read_dir(corpus).expect("corpus dir").flatten() {
        let case_dir = d.path();
        if !case_dir.is_dir() {
            continue;
        }
        let name = case_dir.file_name().unwrap().to_string_lossy().into_owned();
        let work = stage_work_dir(&case_dir, "riv-e2e", &name);
        let sources = source_files(&work);
        if sources.is_empty() {
            lines.push(CaseLine { name, status: "error", detail: "no source files".into() });
            continue;
        }
        let compile = run_with_watchdog(
            {
                let mut c = Command::new(riverilog);
                c.arg("-o").arg("out.riv").args(&sources).current_dir(&work);
                c
            },
            timeout,
        );
        if compile.watchdog {
            lines.push(CaseLine { name, status: "error", detail: "watchdog: compile".into() });
            continue;
        }
        if compile.code != Some(0) {
            if is_decl(compile.code, &compile.stderr) {
                lines.push(CaseLine {
                    name,
                    status: "pending",
                    detail: one_line(&compile.stderr),
                });
            } else {
                lines.push(CaseLine {
                    name,
                    status: "error",
                    detail: format!("compile: {}", one_line(&compile.stderr)),
                });
            }
            continue;
        }
        let run = run_with_watchdog(
            {
                let mut c = Command::new(rivvp);
                c.arg("out.riv").current_dir(&work);
                c
            },
            timeout,
        );
        if run.watchdog {
            lines.push(CaseLine { name, status: "error", detail: "watchdog: run".into() });
            continue;
        }
        if run.code != Some(0) {
            lines.push(CaseLine {
                name,
                status: "error",
                detail: format!("run exit {:?}: {}", run.code, one_line(&run.stderr)),
            });
            continue;
        }
        let expected = std::fs::read_to_string(case_dir.join("expected.stdout")).ok();
        let Some(expected) = expected else {
            lines.push(CaseLine { name, status: "skip", detail: "no expected.stdout".into() });
            continue;
        };
        let got = normalize_e2e_stdout(&run.stdout);
        if got == expected {
            if let Some(ledger) = ledger_text(&case_dir) {
                lines.push(CaseLine {
                    name,
                    status: "pass",
                    detail: format!("DRIFT: ledger stale — case now byte-matches ({ledger})"),
                });
            } else {
                lines.push(CaseLine { name, status: "pass", detail: String::new() });
            }
        } else if let Some(ledger) = ledger_text(&case_dir) {
            lines.push(CaseLine { name, status: "pending", detail: ledger });
        } else {
            lines.push(CaseLine {
                name,
                status: "diff",
                detail: "UNLEDGERED: byte-diff without a riv-pending.txt ledger".into(),
            });
        }
    }
    lines
}

/// iv-e2e arm (design §6 iv section): case.json honored; enforced diffs raw
/// stdout+exit (no normalization — the iverilog tool lines ARE the goldens);
/// pending must hard-error with the milestone pointer.
fn run_iv(corpus: &Path, riverilog: &Path, rivvp: &Path) -> Vec<CaseLine> {
    let timeout = watchdog_timeout();
    let mut lines = Vec::new();
    for c in load_cases(corpus) {
        if c.enforced {
            let work = stage_work_dir(&c.dir, "riv-iv", &c.name);
            let compile = run_with_watchdog(
                {
                    let mut cmd = Command::new(riverilog);
                    cmd.arg(format!("-g{}", c.g))
                        .arg("-o")
                        .arg("out.riv")
                        .args(&c.files)
                        .current_dir(&work);
                    cmd
                },
                timeout,
            );
            if compile.watchdog {
                lines.push(CaseLine { name: c.name, status: "error", detail: "watchdog: compile".into() });
                continue;
            }
            if compile.code != Some(0) {
                lines.push(CaseLine {
                    name: c.name,
                    status: "error",
                    detail: format!("compile: {}", one_line(&compile.stderr)),
                });
                continue;
            }
            let run = run_with_watchdog(
                {
                    let mut cmd = Command::new(rivvp);
                    cmd.arg("out.riv").args(&c.plusargs).current_dir(&work);
                    cmd
                },
                timeout,
            );
            if run.watchdog {
                lines.push(CaseLine { name: c.name, status: "error", detail: "watchdog: run".into() });
                continue;
            }
            let want_exit: i32 = std::fs::read_to_string(c.dir.join("expected.exit"))
                .unwrap()
                .trim()
                .parse()
                .unwrap_or(-1);
            let want = std::fs::read_to_string(c.dir.join("expected.stdout")).unwrap();
            if run.stdout == want && run.code == Some(want_exit) {
                lines.push(CaseLine { name: c.name, status: "pass", detail: String::new() });
            } else {
                lines.push(CaseLine {
                    name: c.name,
                    status: "diff",
                    detail: format!(
                        "exit got {:?} want {want_exit}; stdout {} bytes vs {}",
                        run.code,
                        run.stdout.len(),
                        want.len()
                    ),
                });
            }
        } else {
            // Pending chase target: must answer the milestone hard-error on
            // the real case inputs (files staged so the frontend sees them).
            let work = stage_work_dir(&c.dir, "riv-iv-pending", &c.name);
            let out = run_with_watchdog(
                {
                    let mut cmd = Command::new(riverilog);
                    cmd.arg(format!("-g{}", c.g)).args(&c.files).current_dir(&work);
                    cmd
                },
                timeout,
            );
            if out.watchdog {
                lines.push(CaseLine { name: c.name, status: "error", detail: "watchdog".into() });
            } else if is_decl(out.code, &out.stderr) {
                lines.push(CaseLine {
                    name: c.name,
                    status: "pending",
                    detail: one_line(&out.stderr),
                });
            } else if out.code == Some(0) {
                // Ran clean while the ledger still says pending — the ledger
                // is stale; flip case.json enforced=true in the same round.
                lines.push(CaseLine {
                    name: c.name,
                    status: "pass",
                    detail: "DRIFT: ledger stale (pending case ran to completion)".into(),
                });
            } else {
                lines.push(CaseLine {
                    name: c.name,
                    status: "error",
                    detail: one_line(&out.stderr),
                });
            }
        }
    }
    lines
}

fn locate_bin(name: &str) -> PathBuf {
    let env_key = format!("{}_BIN", name.to_uppercase());
    if let Ok(p) = std::env::var(&env_key) {
        return PathBuf::from(p);
    }
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    for profile in ["debug", "release"] {
        let p = base.join(profile).join(name);
        if p.exists() {
            return p;
        }
    }
    panic!(
        "{name} binary not found; build it first (cargo build -p riv-cli) or set {env_key}"
    );
}

fn print_report(label: &str, lines: &[CaseLine], e2e_pass: usize, e2e_n: usize, iv_pass: usize, iv_n: usize) {
    for l in lines {
        println!("{label}\t{}\t{}\t{}", l.name, l.status, l.detail);
    }
    match label {
        "e2e" => println!("SUMMARY e2e={e2e_pass}/{e2e_n} iv=0/0"),
        _ => println!("SUMMARY e2e=0/0 iv={iv_pass}/{iv_n}"),
    }
}

fn record(corpus: &Path) {
    let riverilog = locate_bin("iverilog");
    let _ = &riverilog;
    for c in load_cases(corpus) {
        let work = std::env::temp_dir().join(format!("riv-record-{}", c.name));
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).unwrap();
        for f in &c.files {
            std::fs::copy(c.dir.join(f), work.join(f)).unwrap();
        }
        let vvp_file = work.join("c.vvp");
        let comp = Command::new("iverilog")
            .arg(format!("-g{}", c.g))
            .arg("-o")
            .arg(&vvp_file)
            .args(&c.files)
            .current_dir(&work)
            .output()
            .expect("spawn iverilog (recording needs the oracle)");
        if !comp.status.success() {
            panic!("{}: iverilog build failed: {}", c.name, String::from_utf8_lossy(&comp.stderr));
        }
        let run = Command::new("vvp")
            .arg("-n")
            .arg("c.vvp")
            .args(&c.plusargs)
            .current_dir(&work)
            .output()
            .expect("spawn vvp");
        std::fs::write(c.dir.join("expected.stdout"), &run.stdout).unwrap();
        let code = run.status.code().unwrap_or(-1);
        std::fs::write(c.dir.join("expected.exit"), format!("{code}\n")).unwrap();
        println!(
            "recorded {} (rc {code}, {} bytes)",
            c.name,
            run.stdout.len()
        );
    }
}

// ---------------------------------------------------------------------------
// Step 0 of the R4 arc (RIVERILOG-PLAN §8.1): the iverilog leg over the e2e
// corpus + the mechanical three-way triage. The e2e goldens are
// verilator-recorded while riverilog's spec is "match iverilog" — without
// this leg, case flips cannot distinguish riv bugs from two-oracle
// disagreements (rivD7 withdrawal precedent).
// ---------------------------------------------------------------------------

/// `--record-iv-leg --corpus corpus/e2e`: per e2e-style case dir (tb.v +
/// expected.stdout, data files staged), record the iverilog/vvp answer as
/// `expected.iv.stdout` + `expected.iv.exit` (raw; committed so the triage
/// replays without a local oracle). A case iverilog cannot compile gets
/// exit 2 + empty stdout — tri skips those legs.
fn record_iv_leg(corpus: &Path) {
    let timeout = watchdog_timeout();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(corpus)
        .expect("corpus dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    let mut n_ok = 0usize;
    let mut n_fail = 0usize;
    for case_dir in dirs {
        let name = case_dir.file_name().unwrap().to_string_lossy().into_owned();
        let work = stage_work_dir(&case_dir, "riv-iv-leg", &name);
        let sources = source_files(&work);
        if sources.is_empty() {
            eprintln!("iv-leg {name}: no source files, skipped");
            continue;
        }
        let comp = run_with_watchdog(
            {
                let mut c = Command::new("iverilog");
                c.arg("-o").arg("c.vvp").args(&sources).current_dir(&work);
                c
            },
            timeout,
        );
        if comp.watchdog || comp.code != Some(0) {
            n_fail += 1;
            eprintln!(
                "iv-leg {name}: iverilog compile failed ({}), leg recorded as exit 2",
                comp.stderr.lines().last().unwrap_or("")
            );
            std::fs::write(case_dir.join("expected.iv.exit"), "2\n").unwrap();
            std::fs::write(case_dir.join("expected.iv.stdout"), "").unwrap();
            continue;
        }
        let run = run_with_watchdog(
            {
                let mut c = Command::new("vvp");
                c.arg("-n").arg("c.vvp").current_dir(&work);
                c
            },
            timeout,
        );
        if run.watchdog {
            eprintln!("iv-leg {name}: vvp watchdog, leg recorded as exit 2");
            std::fs::write(case_dir.join("expected.iv.exit"), "2\n").unwrap();
            std::fs::write(case_dir.join("expected.iv.stdout"), "").unwrap();
            n_fail += 1;
            continue;
        }
        let code = run.code.unwrap_or(-1);
        std::fs::write(case_dir.join("expected.iv.stdout"), &run.stdout).unwrap();
        std::fs::write(case_dir.join("expected.iv.exit"), format!("{code}\n")).unwrap();
        n_ok += 1;
        println!("iv-leg {name}: recorded (rc {code}, {} bytes)", run.stdout.len());
    }
    println!("iv-leg done: {n_ok} recorded, {n_fail} compile/run failures");
}

/// One e2e-arm riv run reused by the triage (compile+run+normalize).
struct RivRun {
    /// Normalized stdout; None when compile/run failed (pending counts as a
    /// distinct status, not a diff).
    got: Option<String>,
    status: String,
    detail: String,
}

fn run_riv_e2e_case(
    case_dir: &Path,
    name: &str,
    riverilog: &Path,
    rivvp: &Path,
    timeout: Duration,
) -> RivRun {
    let work = stage_work_dir(case_dir, "riv-tri", name);
    let sources = source_files(&work);
    if sources.is_empty() {
        return RivRun { got: None, status: "error".into(), detail: "no source files".into() };
    }
    let compile = run_with_watchdog(
        {
            let mut c = Command::new(riverilog);
            c.arg("-o").arg("out.riv").args(&sources).current_dir(&work);
            c
        },
        timeout,
    );
    if compile.watchdog {
        return RivRun { got: None, status: "error".into(), detail: "watchdog: compile".into() };
    }
    if compile.code != Some(0) {
        let d = one_line(&compile.stderr);
        return RivRun {
            got: None,
            status: if is_decl(compile.code, &compile.stderr) { "pending".into() } else { "error".into() },
            detail: d,
        };
    }
    let run = run_with_watchdog(
        {
            let mut c = Command::new(rivvp);
            c.arg("out.riv").current_dir(&work);
            c
        },
        timeout,
    );
    if run.watchdog {
        return RivRun { got: None, status: "error".into(), detail: "watchdog: run".into() };
    }
    if run.code != Some(0) {
        return RivRun {
            got: None,
            status: "error".into(),
            detail: format!("run exit {:?}: {}", run.code, one_line(&run.stderr)),
        };
    }
    RivRun {
        got: Some(normalize_e2e_stdout(&run.stdout)),
        status: "pass".into(),
        detail: String::new(),
    }
}

fn first_diff(a: &str, b: &str) -> String {
    let la: Vec<&str> = a.lines().collect();
    let lb: Vec<&str> = b.lines().collect();
    for i in 0..la.len().max(lb.len()) {
        let (x, y) = (la.get(i).copied().unwrap_or("<eof>"), lb.get(i).copied().unwrap_or("<eof>"));
        if x != y {
            return format!("line {}: iv `{}` vs vl `{}`", i + 1, x, y);
        }
    }
    "identical".into()
}

/// `--tri --corpus corpus/e2e`: mechanical three-way triage per case
/// (iv leg = expected.iv.stdout normalized like the e2e arm; vl =
/// expected.stdout; riv = live run). Classification per plan §8.1:
///   iv==vl && riv==both   → tri-pass   (both oracles agree; hard currency)
///   iv==vl && riv differs → tri-bug    (riv bug — block the flip)
///   iv!=vl && riv==iv     → tri-div-iv (rivD candidate vs the verilator golden)
///   iv!=vl && riv==vl     → tri-div-vl (rivD candidate vs iverilog)
///   otherwise             → tri-own   (riv differs from both — likely bug)
/// Legs with expected.iv.exit != 0 (iverilog could not build the case) are
/// skipped: no second oracle, the e2e arm alone governs.
fn tri_gate(corpus: &Path, riverilog: &Path, rivvp: &Path) {
    let timeout = watchdog_timeout();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(corpus)
        .expect("corpus dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    let (mut n_pass, mut n_bug, mut n_div, mut n_skip) = (0usize, 0usize, 0usize, 0usize);
    for case_dir in dirs {
        let name = case_dir.file_name().unwrap().to_string_lossy().into_owned();
        let Some(vl) = std::fs::read_to_string(case_dir.join("expected.stdout")).ok() else {
            println!("tri\t{name}\ttri-skip\tno expected.stdout");
            n_skip += 1;
            continue;
        };
        let iv_exit: i32 = std::fs::read_to_string(case_dir.join("expected.iv.exit"))
            .ok()
            .and_then(|t| t.trim().parse().ok())
            .unwrap_or(-1);
        if iv_exit != 0 {
            println!(
                "tri\t{name}\ttri-skip\tno iverilog leg (exit {iv_exit}; record with --record-iv-leg)"
            );
            n_skip += 1;
            continue;
        }
        let iv_raw = std::fs::read_to_string(case_dir.join("expected.iv.stdout")).unwrap_or_default();
        let iv = normalize_e2e_stdout(&iv_raw);
        let riv = run_riv_e2e_case(&case_dir, &name, riverilog, rivvp, timeout);
        let Some(got) = riv.got else {
            println!("tri\t{name}\ttri-{}\t{}", riv.status, riv.detail);
            if riv.status == "pass" {
                unreachable!()
            }
            // pending/error cases: not a three-way question yet.
            n_skip += 1;
            continue;
        };
        if iv == vl {
            if got == vl {
                println!("tri\t{name}\ttri-pass\toracles agree, riv matches");
                n_pass += 1;
            } else {
                println!("tri\t{name}\ttri-bug\toracles agree, riv differs: {}", first_diff(&iv, &got));
                n_bug += 1;
            }
        } else if got == iv {
            println!("tri\t{name}\ttri-div-iv\toracles differ ({}), riv matches iverilog — rivD candidate", first_diff(&iv, &vl));
            n_div += 1;
        } else if got == vl {
            println!("tri\t{name}\ttri-div-vl\toracles differ ({}), riv matches verilator — rivD candidate", first_diff(&iv, &vl));
            n_div += 1;
        } else {
            println!("tri\t{name}\ttri-own\toracles differ ({}), riv matches neither: {}", first_diff(&iv, &vl), first_diff(&iv, &got));
            n_div += 1;
        }
    }
    println!("TRI agree={n_pass} bug={n_bug} div={n_div} skip={n_skip}");
}

fn main() -> std::process::ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut do_record = false;
    let mut do_e2e = false;
    let mut do_iv_leg = false;
    let mut do_tri = false;
    let mut corpus: Option<PathBuf> = None;
    let mut i = 0usize;
    while i < argv.len() {
        match argv[i].as_str() {
            "--record" => do_record = true,
            "--e2e" => do_e2e = true,
            "--record-iv-leg" => do_iv_leg = true,
            "--tri" => do_tri = true,
            "--corpus" => {
                corpus = Some(PathBuf::from(&argv[i + 1]));
                i += 1;
            }
            other => {
                eprintln!(
                    "unknown arg `{other}` (usage: riv-difftest [--record] [--record-iv-leg] [--tri] [--e2e] [--corpus DIR])"
                );
                return std::process::ExitCode::from(2);
            }
        }
        i += 1;
    }
    if do_iv_leg {
        let corpus = corpus.unwrap_or_else(default_e2e_corpus);
        record_iv_leg(&corpus);
        return std::process::ExitCode::from(0);
    }
    if do_tri {
        let corpus = corpus.unwrap_or_else(default_e2e_corpus);
        let riverilog = locate_bin("riverilog");
        let rivvp = locate_bin("rivvp");
        tri_gate(&corpus, &riverilog, &rivvp);
        return std::process::ExitCode::from(0);
    }
    if do_record {
        let corpus = corpus.unwrap_or_else(default_iv_corpus);
        record(&corpus);
        return std::process::ExitCode::from(0);
    }
    let riverilog = locate_bin("riverilog");
    let rivvp = locate_bin("rivvp");
    // Arm selection is by CORPUS, not by the `--e2e` flag alone (design §6:
    // the corpus label comes from the path's tail segment, §10's second
    // literal invocation is `--e2e --corpus corpus/iv-e2e`). Routing
    // `--e2e --corpus corpus/iv-e2e` into the e2e arm drops the iverilog
    // finish/VCD-info lines that ARE the iv goldens (normalization is
    // e2e-only) and skips -g/plusargs/case.json — every enforced iv case
    // then diffs as UNLEDGERED.
    let corpus =
        corpus.unwrap_or_else(|| if do_e2e { default_e2e_corpus() } else { default_iv_corpus() });
    let label = arm_label(&corpus);
    let lines = if label == "iv-e2e" {
        run_iv(&corpus, &riverilog, &rivvp)
    } else {
        run_e2e(&corpus, &riverilog, &rivvp)
    };
    let pass = lines.iter().filter(|l| l.status == "pass").count();
    match label {
        "iv-e2e" => print_report(label, &lines, 0, 0, pass, lines.len()),
        _ => print_report(label, &lines, pass, lines.len(), 0, 0),
    }
    // Exit 0 regardless (design §10) — enforcement lives in the regress
    // greps over the SUMMARY/status text.
    std::process::ExitCode::from(0)
}

fn default_iv_corpus() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/iv-e2e"))
}

/// Arm label from the corpus path's tail segment (design §6: "`--e2e` 的
/// corpus 標籤取自路徑尾段(`e2e`/`iv-e2e`)"). `iv-e2e` selects the
/// case.json-honoring raw-byte-diff arm regardless of how the corpus was
/// requested (`--e2e --corpus corpus/iv-e2e` — §10's literal second
/// invocation — or the flagless `--corpus corpus/iv-e2e` regress.sh uses).
fn arm_label(corpus: &Path) -> &'static str {
    match corpus.file_name().and_then(|s| s.to_str()) {
        Some("iv-e2e") => "iv-e2e",
        _ => "e2e",
    }
}

fn default_e2e_corpus() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/e2e"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The normalization contract (design §6-5 / §10 note 1): drop-line
    /// whitelist only. Hostile fixtures — design output that LOOKS like
    /// tool reporting must survive.
    #[test]
    fn tool_lines_are_dropped() {
        let raw = "- S i m u l a t i o n   R e p o r t: x\n\
                   -Info: ignored\n\
                   -Warning: meh\n\
                   -Error: bad\n\
                   tb.v:6: $finish called at 1000 (1ps)\n\
                   VCD info: dumpfile dump.vcd opened for output.\n\
                   keepme\n";
        assert_eq!(normalize_e2e_stdout(raw), "keepme\n");
    }

    #[test]
    fn design_output_survives_hostile_shapes() {
        // Bare `-` is design output (02_div_mod golden: `-7div2=-3`).
        assert_eq!(normalize_e2e_stdout("-7div2=-3\n"), "-7div2=-3\n");
        // A design line with a file:line: prefix but no finish banner
        // survives (the §10 hostile fixture).
        assert_eq!(
            normalize_e2e_stdout("tb.v:6: $display wrote this\n"),
            "tb.v:6: $display wrote this\n"
        );
        // A file:line: prefix with a non-numeric "line" is not a tool line.
        assert_eq!(
            normalize_e2e_stdout("tb.v:line: $finish called at 5 (1ps)\n"),
            "tb.v:line: $finish called at 5 (1ps)\n"
        );
        // Spaces in the file part disqualify the finish-banner shape.
        assert_eq!(
            normalize_e2e_stdout("my file:6: $finish called at 5 (1ps)\n"),
            "my file:6: $finish called at 5 (1ps)\n"
        );
        // `-Infox` IS dropped: the four prefixes are string-prefix
        // matches, verified identical to vl-difftest's normalize
        // (lib.rs normalize_oracle_stdout) — a design line starting with
        // the literal characters "-Info" is indistinguishable from tool
        // reporting under the mirrored contract.
        assert_eq!(normalize_e2e_stdout("-Infox\n"), "\n");
    }

    #[test]
    fn normalization_matches_vl_difftest_prefixes() {
        // The four vl-difftest prefixes must behave identically.
        let raw = "- Simulation Report\n-Info: a\n-Warning: b\n-Error: c\n- x\nplain\n";
        assert_eq!(normalize_e2e_stdout(raw), "plain\n");
    }

    #[test]
    fn watchdog_kills_hung_child() {
        // The design §6-4 watchdog contract as a mechanical anchor: a
        // child still running past the deadline is killed → watchdog=true,
        // no exit code.
        let out = run_with_watchdog(
            {
                let mut c = Command::new("sleep");
                c.arg("5");
                c
            },
            Duration::from_millis(100),
        );
        assert!(out.watchdog, "hung child must be flagged watchdog");
        assert_eq!(out.code, None);
    }

    #[test]
    fn watchdog_passes_through_fast_child() {
        let out = run_with_watchdog(
            {
                let mut c = Command::new("sh");
                c.arg("-c").arg("echo ok");
                c
            },
            Duration::from_secs(5),
        );
        assert!(!out.watchdog);
        assert_eq!(out.code, Some(0));
        assert_eq!(out.stdout.trim(), "ok");
    }

    #[test]
    fn decl_classifier_accepts_both_message_shapes() {
        // Parenless R0 form.
        assert!(is_decl(
            Some(2),
            "riverilog: the compile pipeline lands at milestone R1 (preprocess+parse)"
        ));
        // §5-5 paren form.
        assert!(is_decl(
            Some(2),
            "tb.v:5: module instantiation needs the structural batch (milestone R4)"
        ));
        // Wrong exit class or M-family pointers do not count.
        assert!(!is_decl(Some(1), "sorry: net delays (milestone R4)"));
        assert!(!is_decl(Some(2), "force/release are not yet supported (milestone M7)"));
        assert!(!is_decl(Some(0), "fine (milestone R4)"));
    }

    /// The arm must follow the CORPUS tail, not the `--e2e` flag: §10's
    /// literal `--e2e --corpus corpus/iv-e2e` invocation runs the iv arm
    /// (raw byte-diff, case.json honored). Routing it into the e2e arm
    /// dropped the iverilog finish/VCD-info goldens via the e2e-only
    /// normalization and reported every enforced iv case as an UNLEDGERED
    /// diff (harvest round 1, 2026-10-25).
    #[test]
    fn arm_label_comes_from_corpus_tail() {
        assert_eq!(arm_label(Path::new("corpus/e2e")), "e2e");
        assert_eq!(arm_label(Path::new("corpus/iv-e2e")), "iv-e2e");
        assert_eq!(arm_label(Path::new("/abs/path/corpus/iv-e2e")), "iv-e2e");
        assert_eq!(arm_label(Path::new("/abs/path/corpus/e2e")), "e2e");
    }
}
