//! `riverilog` — the riverilog compiler entrypoint (iverilog-shaped CLI).
//!
//! Contract (iverilog-measured, pinned by crates/riv-cli/goldens): exit 0
//! = `-V`; exit 1 = `-h`/usage/illegal option (stderr); exit 2 =
//! compile/elaboration class errors. Since C2 the pipeline compiles flat
//! designs to RIV-1 (`out.riv` / `-o`): preprocess+parse (shared
//! vl-syntax) → flat elaboration (riv-elab) → RIV-1 write (riv-ir).

use std::path::{Path, PathBuf};
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

/// Target registry: accepted, R-milestone-deferred, and the explicit
/// no-plan list (PLAN §5: synthesis-netlist targets belong to the
/// yosys-equivalent line, not this tool).
fn target_policy(t: &str) -> Result<Option<&'static str>, String> {
    match t {
        // The RIV-1 writer IS the vvp target since C2.
        "vvp" => Ok(None),
        "null" => Ok(Some("the null target (preprocess + parse only) lands at milestone R1 (docs/RIVERILOG-PLAN.md §8)")),
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
    let mut top: Option<String> = None;
    let mut include_dirs: Vec<PathBuf> = Vec::new();
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
            "-s" => {
                top = argv.get(i + 1).cloned();
                i += 1;
            }
            "-I" | "-y" => {
                if let Some(d) = argv.get(i + 1) {
                    include_dirs.push(PathBuf::from(d));
                }
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
        match target_policy(t) {
            Ok(None) => {}
            Ok(Some(defer)) => {
                eprintln!("riverilog: {defer}");
                return ExitCode::from(2);
            }
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(1);
            }
        }
    }
    let out_path = out.unwrap_or_else(|| "a.out".to_string());

    if sources.is_empty() {
        eprintln!("riverilog: no source files.");
        eprintln!();
        eprintln!("{USAGE}");
        return ExitCode::from(1);
    }

    match compile(
        &sources,
        top.as_deref(),
        &include_dirs,
        Path::new(&out_path),
    ) {
        Ok(()) => ExitCode::from(0),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(u32::try_from(e.exit).unwrap_or(2) as u8)
        }
    }
}

/// Compile pipeline: preprocess + parse (shared vl-syntax) → flat
/// elaboration (riv-elab) → RIV-1 text write.
fn compile(
    sources: &[String],
    top: Option<&str>,
    include_dirs: &[PathBuf],
    out_path: &Path,
) -> Result<(), riv_elab::ElabError> {
    use rcs_diag::{DiagSink, SourceMap};
    let mut sm = SourceMap::new();
    let mut diags = DiagSink::new();
    let opts = vl_syntax::pp::PpOptions {
        include_dirs: include_dirs.to_vec(),
        predefines: Vec::new(),
    };
    let mut modules = Vec::new();
    let mut files: Vec<riv_elab::FileTs> = Vec::new();
    for f in sources {
        let path = PathBuf::from(f);
        let Some(pp_out) = vl_syntax::pp::preprocess_file(&path, &mut sm, &mut diags, &opts) else {
            continue;
        };
        files.push(riv_elab::FileTs {
            path: f.clone(),
            timescale: pp_out.timescale,
        });
        let parsed = vl_syntax::parser::parse_source_file(pp_out.tokens, &mut diags);
        modules.extend(parsed.modules);
    }
    if diags.has_errors() {
        let mut text = String::new();
        rcs_diag::render_diags(&mut text, &sm, &diags.diags);
        eprint!("{text}");
        return Err(riv_elab::ElabError {
            message: format!("riverilog: {} error(s) during parsing", diags.error_count()),
            exit: 2,
        });
    }
    let design = riv_elab::elaborate(riv_elab::ElabInput {
        modules: &modules,
        sm: &sm,
        top,
        files: &files,
    })?;
    std::fs::write(out_path, design.write()).map_err(|e| riv_elab::ElabError {
        message: format!("riverilog: cannot write {}: {e}", out_path.display()),
        exit: 2,
    })?;
    Ok(())
}
