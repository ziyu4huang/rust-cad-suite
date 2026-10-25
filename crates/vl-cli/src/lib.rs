//! `rverilator` driver: flag handling, per-file preprocess→parse pipeline,
//! diagnostic rendering. Modes: `--version`, `-E` (preprocess only),
//! `--dump-ast`, the default parse/lint gate, and `--binary` — simulate now
//! (v0 slice: combinational + `initial` blocks; the event scheduler lands at
//! M5 per PLAN §5).

use rcs_diag::{DiagSink, SourceMap};
use std::collections::HashSet;
use std::path::PathBuf;
use vl_syntax::ast::ModuleItem;
use vl_syntax::parser;
use vl_syntax::pp::{self, render_preprocessed, PpOptions};

pub const VERSION_LINE: &str =
    "rverilator 0.1.0 (rust-cad-suite); upstream-parity target: Verilator 5.x";

const USAGE: &str = "\
rverilator — Verilog simulator toolchain (rust-cad-suite Phase 1)

Usage: rverilator [options] <file.v>...

Options:
  --version            Print version and exit
  --lint-only          Parse + lint only, do not simulate
  -E                   Preprocess only, print result
  --binary             Simulate now (v0 slice: combinational + initial blocks;
                       always/#delay scheduling lands at M5)
  --dump-ast           Print the parsed AST
  -I <dir>, -I<dir>    Add include search directory
  -D <NAME[=VAL]>      Predefine a preprocessor macro (default 1)
  --top-module <mod>   Top module (default: auto-detect the un-instantiated one)
  --timing, --no-timing, --trace, --Mdir <dir>, -o <name>,
  --build-jobs <n>, -Wall, -Wno-<CODE>
                       Accepted for compatibility; no-op in v0
  -h, --help           This help

Companion: `vcd2svg <file.vcd>` renders a dump to SVG (crates/vl-vcd).
Exit codes: 0 = no errors, 1 = diagnostics or usage error.
";

pub struct Options {
    pub lint_only: bool,
    pub preprocess_only: bool,
    pub dump_ast: bool,
    pub top_module: Option<String>,
    pub include_dirs: Vec<PathBuf>,
    pub predefines: Vec<String>,
    pub files: Vec<String>,
}

pub fn run(argv: &[String]) -> i32 {
    let mut o = Options {
        lint_only: false,
        preprocess_only: false,
        dump_ast: false,
        top_module: None,
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
            "--top-module" => {
                i += 1;
                if i >= argv.len() {
                    eprintln!("error[VL-E030]: option `--top-module` requires a value");
                    return 1;
                }
                o.top_module = Some(argv[i].clone());
            }
            "--Mdir" | "-o" | "--build-jobs" => {
                i += 1;
                if i >= argv.len() {
                    eprintln!("error[VL-E030]: option `{a}` requires a value");
                    return 1;
                }
            }
            s if s.starts_with("--top-module=") => {
                o.top_module = Some(s["--top-module=".len()..].to_string());
            }
            s if s.starts_with("--Mdir=") => {}
            "-Wall" | "--timing" | "--no-timing" | "--trace" => {}
            s if s.starts_with("-Wno-") => {}
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
    let mut sm = SourceMap::new();
    let mut diags = DiagSink::new();
    if want_binary && !o.preprocess_only {
        simulate(&o, &mut sm, &mut diags)
    } else {
        compile(&o, &mut sm, &mut diags)
    }
}

fn pp_options(o: &Options) -> PpOptions {
    PpOptions {
        include_dirs: o.include_dirs.clone(),
        predefines: o.predefines.clone(),
    }
}

thread_local! {
    /// (path, per-file timescale) collected during preprocess — consumed
    /// by the unified-engine simulate path (ADR-0009 U-1).
    static SOURCE_FILES: std::cell::RefCell<Vec<(String, Option<String>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn preprocess_and_parse(
    o: &Options,
    sm: &mut SourceMap,
    diags: &mut DiagSink,
) -> (Vec<vl_syntax::ast::Module>, Option<String>) {
    let opts = pp_options(o);
    let mut modules = Vec::new();
    let mut timescale: Option<String> = None;
    // (path, per-file timescale) — the unified engine's per-file sticky
    // timescale derivation needs the list (ADR-0009 U-1).
    SOURCE_FILES.with(|c| c.borrow_mut().clear());
    for f in &o.files {
        let path = PathBuf::from(f);
        let Some(out) = pp::preprocess_file(&path, sm, diags, &opts) else {
            continue;
        };
        SOURCE_FILES.with(|c| {
            c.borrow_mut().push((f.clone(), out.timescale.clone()))
        });
        if timescale.is_none() {
            timescale = out.timescale.clone();
        }
        if o.preprocess_only {
            println!("{}", render_preprocessed(&out.tokens));
            continue;
        }
        let file = parser::parse_source_file(out.tokens, diags);
        if o.dump_ast {
            println!("{:#?}", file);
        }
        modules.extend(file.modules);
    }
    (modules, timescale)
}

fn compile(o: &Options, sm: &mut SourceMap, diags: &mut DiagSink) -> i32 {
    let (modules, timescale) = preprocess_and_parse(o, sm, diags);
    if o.preprocess_only {
        return if diags.has_errors() { 1 } else { 0 };
    }
    let mut out = String::new();
    rcs_diag::render_diags(&mut out, sm, &diags.diags);
    // Verilator writes diagnostics to stdout; match that for diffability.
    print!("{out}");
    let n_err = diags.error_count();
    if n_err > 0 {
        println!("%Error: {n_err} error(s) during parsing");
        return 1;
    }
    // Lint = parse + elaborate (upstream `--lint-only` elaborates too);
    // simulation only runs for --binary.
    if let Some(explicit) = &o.top_module {
        return match vl_sim::Sim::build(&modules, explicit, timescale.as_deref()) {
            Ok(_) => {
                println!("%Info: lint ok ({explicit})");
                0
            }
            Err(e) => {
                println!("%Error: {}", e.message);
                1
            }
        };
    }
    // Multi-top files lint like upstream: every un-instantiated module is a
    // root and all of them must elaborate (simulation still demands --top-module).
    match detect_top(&modules) {
        Ok(top) => match vl_sim::Sim::build(&modules, &top, timescale.as_deref()) {
            Ok(_) => {
                println!("%Info: lint ok ({top})");
                0
            }
            Err(e) => {
                println!("%Error: {}", e.message);
                1
            }
        },
        Err(cands) if cands.len() > 1 => {
            let mut first_err = None;
            for c in &cands {
                if let Err(e) = vl_sim::Sim::build(&modules, c, timescale.as_deref()) {
                    first_err.get_or_insert((c.clone(), e.message.clone()));
                }
            }
            match first_err {
                None => {
                    println!("%Info: lint ok (multi-top: {})", cands.join(", "));
                    0
                }
                Some((c, m)) => {
                    println!("%Error: top `{c}`: {m}");
                    1
                }
            }
        }
        Err(cands) => {
            println!(
                "%Error: cannot auto-detect top module (candidates: {}); pass --top-module",
                if cands.is_empty() { "none".to_string() } else { cands.join(", ") }
            );
            1
        }
    }
}

/// Top-module auto-detection: the module no other module instantiates
/// (mirrors upstream's behavior when `--top-module` is omitted).
pub fn detect_top(modules: &[vl_syntax::ast::Module]) -> Result<String, Vec<String>> {
    let instantiated: HashSet<String> = modules
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
    match cands.as_slice() {
        [only] => Ok(only.clone()),
        _ => Err(cands),
    }
}

fn simulate(o: &Options, sm: &mut SourceMap, diags: &mut DiagSink) -> i32 {
    let (modules, timescale) = preprocess_and_parse(o, sm, diags);
    if diags.has_errors() {
        let mut out = String::new();
        rcs_diag::render_diags(&mut out, sm, &diags.diags);
        print!("{out}");
        println!("%Error: cannot simulate while parsing has errors");
        return 1;
    }
    let top = match &o.top_module {
        Some(t) => t.clone(),
        None => match detect_top(&modules) {
            Ok(t) => t,
            Err(cands) => {
                println!(
                    "%Error: cannot auto-detect top module (candidates: {}); pass --top-module",
                    if cands.is_empty() { "none".to_string() } else { cands.join(", ") }
                );
                return 1;
            }
        },
    };
    // ADR-0009 U-1: the unified engine (riv-elab → riv-kernel) drives
    // --binary; the legacy segments interpreter stays reachable via
    // RVERILATOR_LEGACY=1 until U-5 retires it.
    let files: Vec<(String, Option<String>)> =
        SOURCE_FILES.with(|c| c.borrow().clone());
    let use_legacy = std::env::var("RVERILATOR_LEGACY").is_ok();
    if !use_legacy {
        let mut sim = match vl_sim::USim::build(&modules, &top, timescale.as_deref(), sm, &files) {
            Ok(s) => s,
            Err(e) => {
                println!("%Error: {}", e.message);
                return 1;
            }
        };
        sim.set_file_base(std::env::current_dir().unwrap_or_default());
        let r = sim.run();
        print!("{}", sim.take_output());
        if let Some((path, text)) = sim.take_vcd() {
            let path = path.unwrap_or_else(|| "dump.vcd".to_string());
            if let Err(e) = std::fs::write(&path, text) {
                println!("%Error: cannot write VCD `{path}`: {e}");
                return 1;
            }
        }
        return match r {
            Ok(()) => 0,
            Err(e) => {
                println!("%Error: {}", e.message);
                1
            }
        };
    }
    let mut sim = match vl_sim::Sim::build(&modules, &top, timescale.as_deref()) {
        Ok(s) => s,
        Err(e) => {
            println!("%Error: {}", e.message);
            return 1;
        }
    };
    let r = sim.run();
    print!("{}", sim.take_output());
    if let Some((path, text)) = sim.take_vcd() {
        let path = path.unwrap_or_else(|| "dump.vcd".to_string());
        if let Err(e) = std::fs::write(&path, text) {
            println!("%Error: cannot write VCD `{path}`: {e}");
            return 1;
        }
    }
    match r {
        Ok(()) => 0,
        Err(e) => {
            println!("%Error: {}", e.message);
            1
        }
    }
}

