//! `rverilator` driver: flag handling, per-file preprocess→parse pipeline,
//! diagnostic rendering. v0 modes: `--version`, `-E` (preprocess only),
//! `--dump-ast`, and the default parse/lint gate (M2 acceptance).
//! Simulation (`--binary`) lands at M4-M5 per PLAN §5.

use rcs_diag::{DiagSink, SourceMap};
use std::path::PathBuf;
use vl_syntax::parser;
use vl_syntax::pp::{self, PpOptions};
use vl_syntax::token::{DirKind, Punct, Tok};

pub const VERSION_LINE: &str =
    "rverilator 0.1.0 (rust-cad-suite); upstream-parity target: Verilator 5.x";

const USAGE: &str = "\
rverilator — Verilog simulator toolchain (rust-cad-suite Phase 1)

Usage: rverilator [options] <file.v>...

Options:
  --version            Print version and exit
  --lint-only          Parse + lint only, do not simulate (v0 default)
  -E                   Preprocess only, print result
  --dump-ast           Print the parsed AST
  -I <dir>, -I<dir>    Add include search directory
  -D <NAME[=VAL]>      Predefine a preprocessor macro (default 1)
  --top-module <mod>   Top module name (accepted; used from M3)
  --binary             Simulate now (not yet implemented — M4/M5)
  --timing, --no-timing, --trace, --Mdir <dir>, -o <name>,
  --build-jobs <n>, -Wall, -Wno-<CODE>
                       Accepted for compatibility; no-op in v0
  -h, --help           This help

Exit codes: 0 = no errors, 1 = diagnostics or usage error.
";

pub struct Options {
    pub lint_only: bool,
    pub preprocess_only: bool,
    pub dump_ast: bool,
    pub include_dirs: Vec<PathBuf>,
    pub predefines: Vec<String>,
    pub files: Vec<String>,
}

pub fn run(argv: &[String]) -> i32 {
    let mut o = Options {
        lint_only: false,
        preprocess_only: false,
        dump_ast: false,
        include_dirs: Vec::new(),
        predefines: Vec::new(),
        files: Vec::new(),
    };
    let mut want_binary = false;
    let mut i = 0;
    while i < argv.len() {
        let a = argv[i].as_str();
        match a {
            "--version" => {
                println!("{VERSION_LINE}");
                return 0;
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                return 0;
            }
            "--lint-only" => o.lint_only = true,
            "-E" => o.preprocess_only = true,
            "--dump-ast" => o.dump_ast = true,
            "--binary" => want_binary = true,
            "-Wall" | "--timing" | "--no-timing" | "--trace" => {}
            "-Wno-FATAL" | "-Wno-fatal" => {}
            s if s.starts_with("-Wno-") => {}
            "--top-module" | "--Mdir" | "-o" | "--build-jobs" => {
                i += 1;
                if i >= argv.len() {
                    eprintln!("error[VL-E030]: option `{a}` requires a value");
                    return 1;
                }
            }
            s if s.starts_with("--top-module=") || s.starts_with("--Mdir=") => {}
            s if s.starts_with("-I") && s.len() > 2 => o.include_dirs.push(PathBuf::from(&s[2..])),
            "-I" => {
                i += 1;
                if i >= argv.len() {
                    eprintln!("error[VL-E030]: `-I` requires a directory");
                    return 1;
                }
                o.include_dirs.push(PathBuf::from(&argv[i]));
            }
            s if s.starts_with("-D") && s.len() > 2 => o.predefines.push(s[2..].to_string()),
            "-D" => {
                i += 1;
                if i >= argv.len() {
                    eprintln!("error[VL-E030]: `-D` requires a name");
                    return 1;
                }
                o.predefines.push(argv[i].clone());
            }
            "--" => {
                for f in &argv[i + 1..] {
                    o.files.push(f.clone());
                }
                break;
            }
            s if s.starts_with('-') => {
                // Adversarial finding #8: unknown driver flags are an error,
                // not a warning — a typo'd flag must never silently change
                // semantics in CI scripts.
                eprintln!(
                    "error[VL-E030]: Invalid option: `{s}` (see --help; runtime plusargs are a separate surface)"
                );
                return 1;
            }
            f => o.files.push(f.to_string()),
        }
        i += 1;
    }
    if o.files.is_empty() {
        eprintln!("error[VL-E030]: no input files (try `rverilator --help`)");
        return 1;
    }
    if want_binary {
        eprintln!(
            "note: `--binary` simulation is not yet implemented — lands at milestones M4/M5 (PLAN §5); running parse/lint gate only"
        );
    }
    compile(&o)
}

fn compile(o: &Options) -> i32 {
    let mut sm = SourceMap::new();
    let mut diags = DiagSink::new();
    let opts = PpOptions {
        include_dirs: o.include_dirs.clone(),
        predefines: o.predefines.clone(),
    };
    for f in &o.files {
        let path = PathBuf::from(f);
        let Some(out) = pp::preprocess_file(&path, &mut sm, &mut diags, &opts) else {
            continue;
        };
        if o.preprocess_only {
            println!("{}", render_preprocessed(&out.tokens));
            continue;
        }
        let file = parser::parse_source_file(out.tokens, &mut diags);
        if o.dump_ast {
            println!("{:#?}", file);
        }
    }
    if o.preprocess_only {
        return if diags.has_errors() { 1 } else { 0 };
    }
    let mut out = String::new();
    rcs_diag::render_diags(&mut out, &sm, &diags.diags);
    // Verilator writes diagnostics to stdout; match that for diffability.
    print!("{out}");
    let n_err = diags.error_count();
    if n_err > 0 {
        println!("%Error: {n_err} error(s) during parsing");
        1
    } else {
        println!("%Info: parse ok (lint gate; elaboration lands at M3)");
        0
    }
}

/// `-E` output: token reconstruction (formatting is approximate in v0 —
/// byte-exact `-E` parity against `verilator -E` is an M1 acceptance item).
fn render_preprocessed(toks: &[vl_syntax::token::Token]) -> String {
    let mut s = String::new();
    let mut need_space = false;
    for t in toks {
        if t.kind == Tok::Eof {
            break;
        }
        let text = match t.kind {
            Tok::Ident | Tok::SysIdent | Tok::IntLit | Tok::RealLit | Tok::BasedLit
            | Tok::StrLit => t.text.clone(),
            Tok::Keyword(k) => k.as_str().to_string(),
            Tok::Punct(p) => p.as_str().to_string(),
            Tok::Directive(d) => {
                let name = directive_name(d);
                if t.text.is_empty() {
                    format!("`{name}\n")
                } else {
                    format!("`{name} {}\n", t.text)
                }
            }
            _ => continue,
        };
        let no_space = matches!(
            t.kind,
            Tok::Punct(Punct::LParen)
                | Tok::Punct(Punct::LBracket)
                | Tok::Punct(Punct::Semi)
                | Tok::Punct(Punct::Comma)
        ) || s.ends_with('(')
            || s.ends_with('[');
        if need_space && !no_space {
            s.push(' ');
        }
        s.push_str(&text);
        need_space = !matches!(
            t.kind,
            Tok::Punct(Punct::LParen) | Tok::Punct(Punct::LBracket)
        );
    }
    s
}

fn directive_name(d: DirKind) -> &'static str {
    match d {
        DirKind::Define => "define",
        DirKind::Undef => "undef",
        DirKind::Ifdef => "ifdef",
        DirKind::Ifndef => "ifndef",
        DirKind::Elsif => "elsif",
        DirKind::Else => "else",
        DirKind::Endif => "endif",
        DirKind::Include => "include",
        DirKind::Timescale => "timescale",
        DirKind::Resetall => "resetall",
        DirKind::DefaultNettype => "default_nettype",
        DirKind::Line => "line",
        DirKind::Celldefine => "celldefine",
        DirKind::Endcelldefine => "endcelldefine",
        DirKind::UnconnectedDrive => "unconnected_drive",
        DirKind::NounconnectedDrive => "nounconnected_drive",
        DirKind::Pragma => "pragma",
        DirKind::BeginKeywords => "begin_keywords",
        DirKind::EndKeywords => "end_keywords",
    }
}
