//! `vl-difftest` CLI: `--record` regenerates oracle expectations; default
//! checks rverilator output against committed expectations and gates on the
//! pass rate.

use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut record = false;
    let mut min_pass = 80.0f64;
    let mut verilator = String::from("/opt/homebrew/bin/verilator");
    let mut only: Option<String> = None;
    let mut html_path: Option<String> = None;
    let mut xcheck: Option<(String, String)> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--record" => record = true,
            "--min-pass" => {
                i += 1;
                min_pass = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(80.0);
            }
            "--verilator" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    verilator = v.clone();
                }
            }
            "--only" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    only = Some(v.clone());
                }
            }
            "--vcd-xcheck" => {
                let a = args.get(i + 1).cloned().unwrap_or_default();
                let b = args.get(i + 2).cloned().unwrap_or_default();
                xcheck = Some((a, b));
                i += 2;
            }
            "--html" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    html_path = Some(v.clone());
                }
            }
            other => {
                eprintln!("unknown arg `{other}` (usage: vl-difftest [--record] [--min-pass N] [--verilator PATH])");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    if let Some((a, b)) = xcheck {
        let ours = std::fs::read_to_string(&a).unwrap_or_else(|e| panic!("read {a}: {e}"));
        let oracle = std::fs::read_to_string(&b).unwrap_or_else(|e| panic!("read {b}: {e}"));
        let (rep, ok) = vl_difftest::vcd_crosscheck(&ours, &oracle);
        print!("{rep}");
        std::process::exit(if ok { 0 } else { 1 });
    }
    let corpus = vl_difftest::corpus_dir();
    if record {
        if let Err(e) = vl_difftest::record_with_oracle(&corpus, Path::new(&verilator)) {
            eprintln!("record failures: {:?}", e.0);
            std::process::exit(1);
        }
        println!("recorded all cases under {}", corpus.display());
        return;
    }
    let results = vl_difftest::run_suite(&corpus)
        .into_iter()
        .filter(|r| only.as_ref().map(|o| r.name.contains(o)).unwrap_or(true))
        .collect::<Vec<_>>();
    let rate = vl_difftest::pass_rate(&results);
    for r in &results {
        if r.pass {
            println!("PASS {}", r.name);
        } else {
            println!("FAIL {}", r.name);
            for line in r.detail.lines().take(12) {
                println!("     {line}");
            }
        }
    }
    let npass = results.iter().filter(|r| r.pass).count();
    println!(
        "== {npass}/{} passed ({:.1}%) — gate ≥{min_pass:.0}% ==",
        results.len(),
        rate * 100.0
    );
    if let Some(hp) = html_path {
        let full = vl_difftest::run_suite(&corpus);
        std::fs::write(hp.clone(), vl_difftest::html_report(&corpus, &full))
            .expect("write html report");
        println!("html report → {hp}");
    }
    if rate * 100.0 < min_pass {
        std::process::exit(1);
    }
}
