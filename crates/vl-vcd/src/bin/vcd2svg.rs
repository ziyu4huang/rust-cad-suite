//! vcd2svg — render a VCD dump to a deterministic SVG waveform.
//!
//! Companion tool (not an rverilator flag: a flag would flip the positional
//! argument from design files to a dump — exactly what the unknown-flag
//! hard-error contract exists to prevent). Input is ANY VCD, including
//! oracle-produced ones. Unknown flag = hard error exit 1 (same policy as
//! the rverilator driver, vl-cli/src/lib.rs).

use std::process::ExitCode;
use vl_vcd::{parse, render, RenderOptions};

const USAGE: &str = "\
vcd2svg — render a VCD dump to a deterministic SVG waveform (rust-cad-suite)

Usage: vcd2svg [options] <file.vcd>

Options:
  -o <out.svg>         Output file (default: stdout)
  --width <n>          Canvas width px (min 320, default 960)
  --height <n>         Canvas height px (default: auto = axis + rows)
  --from <t>           Visible window start tick (default 0)
  --to <t>             Visible window end tick (default t_max)
  --signals <a,b,c>    Row subset + order (full dotted names, comma-separated;
                       names containing commas are not addressable here)
  --version            Print version and exit
  -h, --help           This help

Exit codes: 0 = ok, 1 = usage/diagnostic error.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut opts = RenderOptions::default();
    let mut input: Option<String> = None;
    let mut out_path: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let val = |i: &mut usize, what: &str| -> Option<String> {
            *i += 1;
            args.get(*i).cloned().or_else(|| {
                eprintln!("error: {what} requires a value");
                None
            })
        };
        match a.as_str() {
            "-o" => match val(&mut i, "-o") {
                Some(v) => out_path = Some(v),
                None => return ExitCode::from(1),
            },
            "--width" => match val(&mut i, "--width").and_then(|v| v.parse().ok()) {
                Some(v) => opts.width = v,
                None => {
                    eprintln!("error: --width requires a number");
                    return ExitCode::from(1);
                }
            },
            "--height" => match val(&mut i, "--height").and_then(|v| v.parse().ok()) {
                Some(v) => opts.height = Some(v),
                None => {
                    eprintln!("error: --height requires a number");
                    return ExitCode::from(1);
                }
            },
            "--from" => match val(&mut i, "--from").and_then(|v| v.parse().ok()) {
                Some(v) => opts.t0 = Some(v),
                None => {
                    eprintln!("error: --from requires a number");
                    return ExitCode::from(1);
                }
            },
            "--to" => match val(&mut i, "--to").and_then(|v| v.parse().ok()) {
                Some(v) => opts.t1 = Some(v),
                None => {
                    eprintln!("error: --to requires a number");
                    return ExitCode::from(1);
                }
            },
            "--signals" => match val(&mut i, "--signals") {
                Some(v) => {
                    opts.signals = Some(v.split(',').map(|s| s.trim().to_string()).collect())
                }
                None => return ExitCode::from(1),
            },
            "--version" => {
                println!(
                    "vcd2svg {} (rust-cad-suite)",
                    env!("CARGO_PKG_VERSION")
                );
                return ExitCode::SUCCESS;
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            other if other.starts_with('-') && other != "-" => {
                eprintln!("error: unknown option `{other}` (see --help)");
                return ExitCode::from(1);
            }
            other => {
                if input.is_some() {
                    eprintln!("error: multiple input files (`{other}`)");
                    return ExitCode::from(1);
                }
                input = Some(other.to_string());
            }
        }
        i += 1;
    }
    let Some(input) = input else {
        eprintln!("error: no input file (see --help)");
        return ExitCode::from(1);
    };
    let text = match std::fs::read_to_string(&input) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: cannot open input file `{input}`: {e}");
            return ExitCode::from(1);
        }
    };
    let f = match parse(&text) {
        Ok(f) => f,
        Err(d) => {
            eprintln!("error: {}: {} @ byte {}", d.code, d.message, d.offset);
            return ExitCode::from(1);
        }
    };
    let svg = match render(&f, &opts) {
        Ok(svg) => svg,
        Err(d) => {
            eprintln!("error: {}: {}", d.code, d.message);
            return ExitCode::from(1);
        }
    };
    match out_path {
        None => {
            use std::io::Write;
            std::io::stdout().write_all(svg.as_bytes()).unwrap();
        }
        Some(p) => {
            if let Err(e) = std::fs::write(&p, svg) {
                eprintln!("error: cannot write `{p}`: {e}");
                return ExitCode::from(1);
            }
        }
    }
    ExitCode::SUCCESS
}
