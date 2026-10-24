//! `riverilog` — the riverilog compiler entrypoint (iverilog-shaped CLI).
//!
//! R0 contract (docs/RIVERILOG-PLAN.md §5, all behavioral claims measured on
//! Icarus Verilog 13.0 and pinned by crates/riv-cli/goldens):
//! exit 0 = `-V`; exit 1 = `-h`/usage/illegal option (stderr); exit 2 =
//! compile/elaboration class errors. The compile pipeline itself lands at
//! R1 (preprocess+parse over the shared vl-syntax) and R2 (event kernel) —
//! reaching it today is a milestone hard-error, never a silent no-op.

use std::process::ExitCode;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "\
Usage: riverilog [-EiRSuvV] [-B base] [-c cmdfile|-f cmdfile]
                 [-g1995|-g2001|-g2005|-g2005-sv|-g2009|-g2012] [-g<feature>]
                 [-D macro[=defn]] [-I includedir] [-L moduledir]
                 [-M [mode=]depfile] [-m module]
                 [-N file] [-o filename] [-p flag=value]
                 [-s topmodule] [-t target] [-T min|typ|max]
                 [-W class] [-y dir] [-Y suf] [-l file] source_file(s)

See docs/RIVERILOG-PLAN.md for the supported contract surface.";

fn banner() -> String {
    // Component-list shape mirrors the oracle's four-part `-V` banner while
    // self-identifying (no-impersonation law, PLAN §9.8-7).
    format!(
        "riverilog version {VERSION} (oss-cad-suite Rust rewrite)\n\
         \n\
         riverilog Preprocessor version {VERSION}\n\
         riverilog Parser/Elaborator version {VERSION}\n\
         \x20 FLAG vvp.rtg\n\
         vvp.rtg: riverilog riv design writer {VERSION}\n"
    )
}

/// Target registry: accepted (deferred), R-milestone-deferred, and the
/// explicit no-plan list (PLAN §5: synthesis-netlist targets belong to the
/// yosys-equivalent line, not this tool).
fn target_policy(t: &str) -> Result<&'static str, String> {
    match t {
        "vvp" => Ok("deferred to R2 (event kernel) + R6 (riv format)"),
        "null" => Ok("deferred to R1 (preprocess + parse)"),
        "blif" | "fpga" | "vhdl" | "vlog95" => Err(format!(
            "riverilog: target `{t}` is not planned (synthesis-netlist output belongs to the yosys-equivalent line; see docs/RIVERILOG-PLAN.md §5)"
        )),
        other => Err(format!(
            "riverilog: unknown target `{other}` (supported: vvp; null lands at R1)"
        )),
    }
}

fn std_level(g: &str) -> Result<&'static str, String> {
    match g {
        "1995" | "2001" | "2005" | "2005-sv" | "2009" | "2012" => Ok("known (dialect profiles land at R1/R8)"),
        // Oracle shape (measured): one message for any unknown -g argument,
        // generation or feature toggle alike.
        other => Err(format!("riverilog: Unknown/Unsupported Language generation {other}")),
    }
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut sources: Vec<String> = Vec::new();
    let mut out: Option<String> = None;
    let mut target: Option<String> = None;
    let mut i = 0usize;
    while i < argv.len() {
        let a = argv[i].clone();
        match a.as_str() {
            "-V" => {
                print!("{}", banner());
                return ExitCode::from(0);
            }
            "-h" | "-?" => {
                eprintln!("{USAGE}");
                return ExitCode::from(1);
            }
            "-o" => {
                out = argv.get(i + 1).cloned();
                i += 1;
            }
            "-t" => match argv.get(i + 1) {
                Some(t) => {
                    target = Some(t.clone());
                    i += 1;
                }
                None => {
                    eprintln!("riverilog: option -t requires an argument");
                    return ExitCode::from(1);
                }
            },
            "-g" => match argv.get(i + 1) {
                Some(g) => {
                    if let Err(e) = std_level(g) {
                        eprintln!("{e}");
                        return ExitCode::from(1);
                    }
                    i += 1;
                }
                None => {
                    eprintln!("riverilog: option -g requires an argument");
                    return ExitCode::from(1);
                }
            },
            _ if a.starts_with("-g") => {
                // Attached form (-g2005 / feature toggles). The oracle answers
                // any unknown -g argument with the same single message —
                // mirrored; the real feature registry lands at R1 from the
                // man page, not from guesses.
                if let Err(e) = std_level(&a[2..]) {
                    eprintln!("{e}");
                    return ExitCode::from(1);
                }
            }
            _ if a.starts_with('-') && a.len() > 1 => {
                let known = [
                    "-E", "-i", "-R", "-S", "-u", "-v", "-B", "-c", "-f", "-D", "-I", "-L", "-M",
                    "-m", "-N", "-p", "-s", "-T", "-W", "-y", "-Y", "-l",
                ];
                let base = a.split('=').next().unwrap_or(&a).to_string();
                if !known.contains(&base.as_str()) {
                    // Oracle shape: the byte right after the first dash
                    // ("--frobnicate" -> "illegal option -- -").
                    let c = a.split_at(1).1.chars().next().unwrap_or('-');
                    eprintln!("riverilog: illegal option -- {c}");
                    eprintln!("{USAGE}");
                    return ExitCode::from(1);
                }
                // Options with a separate argument consume it.
                if matches!(base.as_str(), "-B" | "-c" | "-f" | "-D" | "-I" | "-L" | "-M" | "-m" | "-N" | "-p" | "-s" | "-T" | "-W" | "-y" | "-Y" | "-l")
                    && !a.contains('=')
                {
                    i += 1;
                }
            }
            _ => sources.push(a),
        }
        i += 1;
    }

    if let Some(t) = &target {
        if let Err(e) = target_policy(t) {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    }
    let _ = out; // stored for R2 (default a.out per the oracle contract)

    if sources.is_empty() {
        eprintln!("riverilog: no source files.");
        eprintln!();
        eprintln!("{USAGE}");
        return ExitCode::from(1);
    }

    // The compile pipeline itself: milestone hard-error (AGENTS §3 —
    // unsupported never silently no-ops). Error class = compile (2).
    eprintln!(
        "riverilog: the compile pipeline lands at milestone R1 (preprocess+parse) and R2 (event kernel); nothing is compiled yet (docs/RIVERILOG-PLAN.md §8)"
    );
    ExitCode::from(2)
}
