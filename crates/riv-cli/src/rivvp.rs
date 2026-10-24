//! `rivvp` — the riverilog runtime entrypoint (vvp-shaped CLI).
//!
//! R0 contract (measured on vvp 13.0, pinned by riv-cli goldens): `-h` on
//! stdout exits 0 (asymmetric to riverilog's stderr/1); missing input file =
//! "Unable to open input file." + exit 255; no input = "vvp: no input file."
//! shape + 255. Design loading lands at R2 (interpreter) / R6 (riv format).

use std::path::Path;
use std::process::ExitCode;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "\
Usage: rivvp [-n] [-N] [-i] [-q] [-l logfile] [-M moduledir] [-m module]
             design-file [plusargs...]";

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut design: Option<String> = None;
    let mut i = 0usize;
    while i < argv.len() {
        let a = argv[i].clone();
        match a.as_str() {
            "-V" => {
                println!("rivvp (riverilog runtime) version {VERSION}");
                return ExitCode::from(0);
            }
            "-h" => {
                println!("{USAGE}");
                return ExitCode::from(0);
            }
            "-n" | "-N" | "-i" | "-q" => {}
            "-l" | "-M" | "-m" => i += 1,
            _ if a.starts_with('+') => {}
            _ if a.starts_with('-') => {
                eprintln!("rivvp: illegal option -- {}", &a[1..]);
                eprintln!("{USAGE}");
                return ExitCode::from(1);
            }
            _ => {
                design = Some(a);
            }
        }
        i += 1;
    }

    let Some(design) = design else {
        eprintln!("rivvp: no input file.");
        return ExitCode::from(255);
    };

    // An argument that does not exist reports the oracle's exact shape; one
    // that exists cannot be loaded yet — milestone hard-error, never silent.
    if !Path::new(&design).exists() {
        eprintln!("{design}: Unable to open input file.");
        return ExitCode::from(255);
    }
    eprintln!(
        "rivvp: design loading lands at milestone R2 (event kernel) and R6 (riv format); no design can be loaded yet (docs/RIVERILOG-PLAN.md §8)"
    );
    ExitCode::from(1)
}
