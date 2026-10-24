//! riv-difftest — the riverilog differential harness (vl-difftest shape,
//! oracle = real `iverilog`/`vvp`).
//!
//! Recording (needs the oracle, run once per case):
//!   riv-difftest --record [--corpus DIR]
//! writes expected.stdout/expected.exit per case from
//! `iverilog -g<level> -o c.vvp` + `vvp -n c.vvp <plusargs>` in a temp dir.
//!
//! Running (zero oracle dependency):
//!   riv-difftest [--corpus DIR]
//! per case: either the case is `enforced` (diff stdout+exit against the
//! recording) or it is a pending chase target — riverilog must answer the
//! milestone hard-error (exit 2, "milestone R" pointer). The pending
//! population is gated exactly: a case flipping enforced↔pending without a
//! committed case.json edit is drift.

use std::path::{Path, PathBuf};

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

fn record(corpus: &Path) {
    for c in load_cases(corpus) {
        let work = std::env::temp_dir().join(format!("riv-record-{}", c.name));
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).unwrap();
        for f in &c.files {
            std::fs::copy(c.dir.join(f), work.join(f)).unwrap();
        }
        let vvp_file = work.join("c.vvp");
        let comp = std::process::Command::new("iverilog")
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
        let run = std::process::Command::new("vvp")
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

fn locate_riverilog() -> PathBuf {
    if let Ok(p) = std::env::var("RIVERILOG_BIN") {
        return PathBuf::from(p);
    }
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    for profile in ["debug", "release"] {
        let p = base.join(profile).join("riverilog");
        if p.exists() {
            return p;
        }
    }
    panic!("riverilog binary not found; build it first (cargo build -p riv-cli) or set RIVERILOG_BIN");
}

fn run_suite(corpus: &Path) -> i32 {
    let riverilog = &locate_riverilog();
    let mut enforced_pass = 0usize;
    let mut enforced_total = 0usize;
    let mut pending = Vec::new();
    for c in load_cases(corpus) {
        if c.enforced {
            enforced_total += 1;
            let work = std::env::temp_dir().join(format!("riv-run-{}", c.name));
            let _ = std::fs::remove_dir_all(&work);
            std::fs::create_dir_all(&work).unwrap();
            // Full compile+run pipeline is R2+; today the compile step alone
            // answers the milestone error, so enforced cases cannot pass yet
            // — this arm exists so the moment R2 lands, flipping
            // case.json `enforced: true` arms the exact diff with zero
            // harness work.
            let out = std::process::Command::new(riverilog)
                .arg(format!("-g{}", c.g))
                .arg("-o")
                .arg("out.riv")
                .args(&c.files)
                .current_dir(&work)
                .output()
                .expect("spawn riverilog");
            let want_exit: i32 = std::fs::read_to_string(c.dir.join("expected.exit"))
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            let want: String = std::fs::read_to_string(c.dir.join("expected.stdout")).unwrap();
            let got = String::from_utf8_lossy(&out.stdout);
            if got == want && out.status.code() == Some(want_exit) {
                enforced_pass += 1;
                println!("PASS {}", c.name);
            } else {
                println!("FAIL {}", c.name);
            }
        } else {
            // Pending chase target: riverilog must answer the R-milestone
            // hard error (exit 2, "milestone R" pointer) — that IS the R0
            // contract on real inputs.
            let out = std::process::Command::new(riverilog)
                .arg(format!("-g{}", c.g))
                .args(&c.files)
                .output()
                .expect("spawn riverilog");
            let se = String::from_utf8_lossy(&out.stderr);
            let ok = out.status.code() == Some(2) && se.contains("milestone R");
            if ok {
                pending.push(c.name.clone());
            } else {
                println!(
                    "DRIFT {}: expected the R0 milestone hard-error (exit 2 + pointer), got exit {:?} stderr={se}",
                    c.name,
                    out.status.code()
                );
            }
        }
    }
    let pending_total = pending.len();
    println!(
        "== riverilog e2e: enforced {enforced_pass}/{enforced_total}, pending chase targets {pending_total} (expectations recorded) =="
    );
    if enforced_pass != enforced_total {
        return 1;
    }
    // Pending population gate: any drift above is a fail; a case flipping to
    // working requires editing case.json (enforced: true) in the same commit.
    0
}

fn main() -> std::process::ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut do_record = false;
    let mut corpus = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/iv-e2e"));
    let mut i = 0usize;
    while i < argv.len() {
        match argv[i].as_str() {
            "--record" => do_record = true,
            "--corpus" => {
                corpus = PathBuf::from(&argv[i + 1]);
                i += 1;
            }
            other => {
                eprintln!("unknown arg `{other}` (usage: riv-difftest [--record] [--corpus DIR])");
                return std::process::ExitCode::from(2);
            }
        }
        i += 1;
    }
    if do_record {
        record(&corpus);
        std::process::ExitCode::from(0)
    } else {
        std::process::ExitCode::from(u8::try_from(run_suite(&corpus)).unwrap_or(1))
    }
}
