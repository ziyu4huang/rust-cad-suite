//! `riverilog` — the riverilog compiler entrypoint (iverilog-shaped CLI).
//!
//! Contract (iverilog-measured, pinned by crates/riv-cli goldens): exit 0
//! = `-V` and clean compile; exit 1 = `-h`/usage/illegal option (stderr)
//! and preprocessor-class errors (diag text is vl-syntax's, wrapper text
//! free per rivD5); exit 2 = parse/elaboration class errors. Since C2 the
//! pipeline compiles flat designs to RIV-1 (`out.riv` / `-o`):
//! preprocess+parse (shared vl-syntax) → flat elaboration (riv-elab) →
//! RIV-1 write (riv-ir); `-t null` = same minus the write (elaboration
//! still runs, oracle-measured), `-E` = preprocess only routed to
//! a.out/-o/stdout (rivD4: `-o -` refused off -E).

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
        // The RIV-1 writer IS the vvp target since C2. `null` = the full
        // pipeline minus the write (oracle-measured: elaboration still
        // runs; `-t null -o X` produces no file; error classes unchanged:
        // pp=1 parse=2 elab=2).
        "vvp" | "null" => Ok(None),
        "blif" | "fpga" | "vhdl" | "vlog95" => Err(format!(
            "riverilog: target `{t}` is not planned (synthesis-netlist output belongs to the yosys-equivalent line; see docs/RIVERILOG-PLAN.md §5)"
        )),
        other => Err(format!(
            "riverilog: unknown target `{other}` (supported: vvp, null)"
        )),
    }
}

/// The `-g` registry, measured verbatim from iverilog 13.0 (U-3b probe:
/// an unknown generation makes the oracle print its own supported list —
/// the registry text below is that measurement, not a man-page guess).
/// Generations gate keyword acceptance (R8 will consume `generation`);
/// feature toggles are accepted-as-measured, their semantic effects land
/// with the features themselves (assertions R8, specify R9, ...).
const GENERATIONS: [(&str, &str); 6] = [
    ("1995", "IEEE1364-1995"),
    ("2001", "IEEE1364-2001"),
    ("2005", "IEEE1364-2005"),
    ("2005-sv", "IEEE1800-2005"),
    ("2009", "IEEE1800-2009"),
    ("2012", "IEEE1800-2012"),
];

const FEATURE_FLAGS: [&str; 25] = [
    "assertions",
    "supported-assertions",
    "no-assertions",
    "specify",
    "no-specify",
    "interconnect",
    "no-interconnect",
    "verilog-ams",
    "no-verilog-ams",
    "std-include",
    "no-std-include",
    "relative-include",
    "no-relative-include",
    "xtypes",
    "no-xtypes",
    "icarus-misc",
    "no-icarus-misc",
    "io-range-error",
    "no-io-range-error",
    "strict-ca-eval",
    "no-strict-ca-eval",
    "strict-expr-width",
    "no-strict-expr-width",
    "shared-loop-index",
    "no-shared-loop-index",
];

/// Ok(generation) for a known generation; Ok("") for a feature toggle
/// (no generation change); Err(text) = the oracle's exact refusal shape.
/// Measured exit for the refusal is **255** (not 1 — re-measured U-3b,
/// supersedes the R0 receipt's single-message class).
fn std_level(g: &str) -> Result<&'static str, String> {
    if let Some((name, _)) = GENERATIONS.iter().find(|(n, _)| *n == g) {
        return Ok(name);
    }
    if FEATURE_FLAGS.contains(&g) {
        return Ok("");
    }
    let mut text = format!("riverilog: Unknown/Unsupported Language generation {g}\n\n");
    text.push_str("Supported generations are:\n");
    for (name, desc) in GENERATIONS {
        text.push_str(&format!("    {name:<8}-- {desc}\n"));
    }
    text.push_str("Other generation flags:\n");
    for group in [
        "assertions | supported-assertions | no-assertions",
        "specify | no-specify",
        "interconnect | no-interconnect",
        "verilog-ams | no-verilog-ams",
        "std-include | no-std-include",
        "relative-include | no-relative-include",
        "xtypes | no-xtypes",
        "icarus-misc | no-icarus-misc",
        "io-range-error | no-io-range-error",
        "strict-ca-eval | no-strict-ca-eval",
        "strict-expr-width | no-strict-expr-width",
        "shared-loop-index | no-shared-loop-index",
    ] {
        text.push_str(&format!("    {group}\n"));
    }
    Err(text)
}

/// `-W` classes that the oracle accepts silently (U-3b probe: all, none,
/// infloop, timescale, portbind, implicit-dimensions, select-range pass
/// without output; anything else prints "Ignoring unknown warning class"
/// on stderr and continues with exit 0).
const WARNING_CLASSES: [&str; 7] = [
    "all",
    "none",
    "infloop",
    "timescale",
    "portbind",
    "implicit-dimensions",
    "select-range",
];

/// `-T` selections accepted by the oracle (min/typ/max validated; the
/// triple-selection *semantics* stay ledgered pending a conclusive oracle
/// probe — accepted-and-validated at the CLI, no guessed behavior).
const TIMESCALE_ARGS: [&str; 3] = ["min", "typ", "max"];

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut sources: Vec<String> = Vec::new();
    let mut out: Option<String> = None;
    let mut target: Option<String> = None;
    let mut top: Option<String> = None;
    let mut include_dirs: Vec<PathBuf> = Vec::new();
    let mut lib_dirs: Vec<PathBuf> = Vec::new();
    let mut lib_suffixes: Vec<String> = Vec::new();
    let mut predefines: Vec<String> = Vec::new();
    let mut preprocess_only = false;
    // U-3b additions (all oracle-measured on iverilog 13.0).
    let mut ignore_missing = false; // -i: skip undefined instances (exit 0)
    let mut depfile: Option<String> = None; // -M <file>
    let mut ptop: Option<String> = None; // -P <name>: top hint, non-fatal if absent
    let mut timescale_arg: Option<String> = None; // -T min|typ|max
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
            // Shared frontend shapes (ADR-0009 U-3): -D/-I mirror the
            // rverilator face (vl-cli/src/lib.rs) — attached + separate.
            "-D" => match argv.get(i + 1) {
                Some(d) => {
                    predefines.push(d.clone());
                    i += 1;
                }
                None => {
                    eprintln!("riverilog: option -D requires an argument");
                    return ExitCode::from(1);
                }
            },
            s if s.starts_with("-D") && s.len() > 2 => predefines.push(s[2..].to_string()),
            "-I" => match argv.get(i + 1) {
                Some(d) => {
                    include_dirs.push(PathBuf::from(d));
                    i += 1;
                }
                None => {
                    eprintln!("riverilog: option -I requires an argument");
                    return ExitCode::from(1);
                }
            },
            s if s.starts_with("-I") && s.len() > 2 => include_dirs.push(PathBuf::from(&s[2..])),
            // Library search (R1): -y dirs; -Y suffixes APPEND to the
            // default [".v"] (measured on iverilog 13.0: -Y is additive,
            // never replaces, and the dot travels inside the suffix).
            "-y" => match argv.get(i + 1) {
                Some(d) => {
                    lib_dirs.push(PathBuf::from(d));
                    i += 1;
                }
                None => {
                    eprintln!("riverilog: option -y requires an argument");
                    return ExitCode::from(1);
                }
            },
            "-Y" => match argv.get(i + 1) {
                Some(sfx) => {
                    lib_suffixes.push(sfx.clone());
                    i += 1;
                }
                None => {
                    eprintln!("riverilog: option -Y requires an argument");
                    return ExitCode::from(1);
                }
            },
            "-E" => preprocess_only = true,
            // U-3b: real -i (oracle-measured iverilog 13.0: a missing
            // module is skipped silently, elaboration exits 0).
            "-i" => ignore_missing = true,
            // U-3b: -f/-c command file (oracle-measured grammar: each
            // non-blank line is ONE path token; `#` lines are comments;
            // option-looking lines are a parse failure, exit 1).
            "-f" | "-c" => match argv.get(i + 1) {
                Some(f) => {
                    if let Err(e) = read_cmd_file(f, &mut sources) {
                        eprint!("{e}");
                        return ExitCode::from(1);
                    }
                    i += 1;
                }
                None => {
                    eprintln!("riverilog: option requires an argument -- {}", &a[1..]);
                    return ExitCode::from(1);
                }
            },
            // U-3b: -M <depfile> — dependency list written after a
            // successful compile (sources once, then each -y-resolved
            // library file exactly twice; measured rule, n=3 probes).
            "-M" => match argv.get(i + 1) {
                Some(f) => {
                    depfile = Some(f.clone());
                    i += 1;
                }
                None => {
                    eprintln!("riverilog: option requires an argument -- M");
                    return ExitCode::from(1);
                }
            },
            // U-3b: -P <name> selects the top like -s, but an absent name
            // is NON-FATAL (oracle: one stderr line, default top, exit 0).
            "-P" => match argv.get(i + 1) {
                Some(n) => {
                    ptop = Some(n.clone());
                    i += 1;
                }
                None => {
                    eprintln!("riverilog: option requires an argument -- P");
                    return ExitCode::from(1);
                }
            },
            // U-3b: -S = synthesis path — unplanned target, hard error with
            // the pointer (R0 policy; the oracle synthesizes, we do not).
            "-S" => {
                eprintln!("riverilog: -S (synthesis) is not supported (R0 policy; docs/RIVERILOG-PLAN.md)");
                return ExitCode::from(1);
            }
            // U-3b: -T min|typ|max — validated; triple-selection semantics
            // ledgered pending a conclusive oracle probe.
            "-T" => match argv.get(i + 1) {
                Some(t) => {
                    if !TIMESCALE_ARGS.contains(&t.as_str()) {
                        eprintln!("riverilog: invalid -T{t} argument");
                        return ExitCode::from(1);
                    }
                    timescale_arg = Some(t.clone());
                    i += 1;
                }
                None => {
                    eprintln!("riverilog: option requires an argument -- T");
                    return ExitCode::from(1);
                }
            },
            // U-3b: -p NAME=VALUE accepted and ignored (oracle: silent,
            // exit 0, even without '=').
            "-p" => {
                i += 1;
            }
            // U-3b: -W <class> — known classes silent; unknown classes get
            // the oracle's stderr line and compilation continues.
            "-W" => match argv.get(i + 1) {
                Some(w) => {
                    if !WARNING_CLASSES.contains(&w.as_str()) {
                        eprintln!("Ignoring unknown warning class {w}");
                    }
                    i += 1;
                }
                None => {
                    eprintln!("riverilog: option requires an argument -- W");
                    return ExitCode::from(1);
                }
            },
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
                        // Measured exit is 255 (U-3b re-probe; the R0
                        // single-message receipt's exit 1 is superseded).
                        eprint!("{e}");
                        return ExitCode::from(255);
                    }
                    i += 1;
                }
                None => {
                    eprintln!("riverilog: option -g requires an argument");
                    return ExitCode::from(1);
                }
            },
            _ if a.starts_with("-g") => {
                // Attached form (-g2005 / feature toggles).
                if let Err(e) = std_level(&a[2..]) {
                    eprint!("{e}");
                    return ExitCode::from(255);
                }
            }
            s if s.starts_with("-W") && s.len() > 2 => {
                let w = &s[2..];
                if !WARNING_CLASSES.contains(&w) {
                    eprintln!("Ignoring unknown warning class {w}");
                }
            }
            s if s.starts_with("-P") && s.len() > 2 => {
                ptop = Some(s[2..].to_string());
            }
            s if s.starts_with("-T") && s.len() > 2 => {
                let t = &s[2..];
                if !TIMESCALE_ARGS.contains(&t) {
                    eprintln!("riverilog: invalid -T{t} argument");
                    return ExitCode::from(1);
                }
                timescale_arg = Some(t.to_string());
            }
            s if s.starts_with("-M") && s.len() > 2 => {
                depfile = Some(s[2..].to_string());
            }
            s if s.starts_with("-p") && s.len() > 2 => {}
            _ if a.starts_with('-') && a.len() > 1 => {
                let known = [
                    "-R", "-S", "-u", "-v", "-B", "-c", "-f", "-L", "-M", "-m", "-N", "-p", "-s",
                    "-T", "-W", "-l",
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
                if matches!(base.as_str(), "-B" | "-c" | "-f" | "-L" | "-M" | "-m" | "-N" | "-p" | "-s" | "-T" | "-W" | "-l")
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
    // rivD4 (fulfilled): `-o -` on a compile target is refused — the
    // oracle writes a file literally named `-` (measured, iverilog 13.0),
    // a footgun this face does not inherit. `-E -o -` is the legitimate
    // stdout route.
    if out.as_deref() == Some("-") && !preprocess_only {
        eprintln!("riverilog: -o - (stdout) is only valid together with -E (rivD4; docs/REGISTRY.md divergence register)");
        return ExitCode::from(1);
    }
    let out_path = out.unwrap_or_else(|| "a.out".to_string());

    if sources.is_empty() {
        eprintln!("riverilog: no source files.");
        eprintln!();
        eprintln!("{USAGE}");
        return ExitCode::from(1);
    }

    let mode = if preprocess_only {
        Mode::E
    } else if target.as_deref() == Some("null") {
        Mode::NullTarget
    } else {
        Mode::Normal
    };
    // -T selection (U-3b, oracle-measured semantics on `a:b:c` delays);
    // None = the compiler default (typ) with the per-site warning.
    let delay_selection = timescale_arg.as_deref().map(|t| match t {
        "min" => riv_elab::DelaySelection::Min,
        "max" => riv_elab::DelaySelection::Max,
        _ => riv_elab::DelaySelection::Typ,
    });
    let warnings = std::cell::RefCell::new(Vec::new());
    match compile(
        &sources,
        top.as_deref(),
        ptop.as_deref(),
        &include_dirs,
        &predefines,
        &lib_dirs,
        &lib_suffixes,
        mode,
        Path::new(&out_path),
        ignore_missing,
        delay_selection,
        &warnings,
    ) {
        Ok(resolved) => {
            // Oracle-measured warning flush (compile-time stderr, after
            // elaboration, before any -M work).
            for w in warnings.borrow().iter() {
                eprintln!("{w}");
            }
            // -M (U-3b): dependency list after a successful compile.
            // Measured rule: explicit sources once, then each -y-resolved
            // library file exactly twice (n=3 oracle probes all show the
            // doubled entry).
            if let Some(df) = &depfile {
                let mut text = String::new();
                for s in &sources {
                    text.push_str(s);
                    text.push('\n');
                }
                for r in &resolved {
                    text.push_str(r);
                    text.push('\n');
                    text.push_str(r);
                    text.push('\n');
                }
                if let Err(e) = std::fs::write(df, text) {
                    eprintln!("riverilog: cannot write {df}: {e}");
                    return ExitCode::from(2);
                }
            }
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(u32::try_from(e.exit).unwrap_or(2) as u8)
        }
    }
}

/// Pipeline mode (ADR-0009 U-3): E = preprocess only (oracle-measured: a
/// syntax-error file still passes -E with exit 0; pp errors exit 1);
/// NullTarget = pp+parse+elaborate with no output file (oracle-measured:
/// elaboration still runs, `-t null -o X` produces no file); Normal = the
/// full RIV-1 compile. E wins over null.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Normal,
    NullTarget,
    E,
}

/// Shared-frontend loader: per file pp → parse with per-stage error
/// accounting. Classification (oracle-measured exits; wrapper text free
/// per rivD5): pp errors (missing file included) → exit 1, parse errors →
/// exit 2, elaboration errors → exit 2 (riv-elab).
struct Loader {
    opts: vl_syntax::pp::PpOptions,
    sm: rcs_diag::SourceMap,
    diags: rcs_diag::DiagSink,
    modules: Vec<vl_syntax::ast::Module>,
    files: Vec<riv_elab::FileTs>,
    pp_errors: usize,
    parse_errors: usize,
}

impl Loader {
    fn new(include_dirs: &[PathBuf], predefines: &[String]) -> Loader {
        Loader {
            opts: vl_syntax::pp::PpOptions {
                include_dirs: include_dirs.to_vec(),
                predefines: predefines.to_vec(),
            },
            sm: rcs_diag::SourceMap::new(),
            diags: rcs_diag::DiagSink::new(),
            modules: Vec::new(),
            files: Vec::new(),
            pp_errors: 0,
            parse_errors: 0,
        }
    }

    /// pp (+parse unless `parse` is false) one file — explicit source or
    /// -y library hit. Stage buckets come from error-count deltas; returns
    /// the rendered -E text when parsing is skipped.
    fn load_file(&mut self, f: &str, parse: bool) -> Option<String> {
        let path = PathBuf::from(f);
        let before = self.diags.error_count();
        let Some(pp_out) =
            vl_syntax::pp::preprocess_file(&path, &mut self.sm, &mut self.diags, &self.opts)
        else {
            self.pp_errors += self.diags.error_count() - before;
            return None;
        };
        self.pp_errors += self.diags.error_count() - before;
        self.files.push(riv_elab::FileTs {
            path: f.to_string(),
            timescale: pp_out.timescale,
        });
        if !parse {
            return Some(vl_syntax::pp::render_preprocessed(&pp_out.tokens));
        }
        let before = self.diags.error_count();
        let parsed = vl_syntax::parser::parse_source_file(pp_out.tokens, &mut self.diags);
        self.parse_errors += self.diags.error_count() - before;
        self.modules.extend(parsed.modules);
        None
    }

    fn render_errors(&self) -> String {
        let mut text = String::new();
        rcs_diag::render_diags(&mut text, &self.sm, &self.diags.diags);
        text
    }

    fn pp_failure(&self) -> riv_elab::ElabError {
        riv_elab::ElabError {
            message: format!(
                "riverilog: preprocessor failed with {} error(s)",
                self.pp_errors
            ),
            exit: 1,
        }
    }
}

/// Compile pipeline: preprocess + parse (shared vl-syntax) → -y library
/// fixpoint → flat elaboration (riv-elab) → RIV-1 text write (skipped for
/// NullTarget; routed per -o for E). Returns the -y-resolved library paths
/// (deduped, sorted) for `-M` (U-3b).
fn compile(
    sources: &[String],
    top: Option<&str>,
    ptop: Option<&str>,
    include_dirs: &[PathBuf],
    predefines: &[String],
    lib_dirs: &[PathBuf],
    lib_suffixes: &[String],
    mode: Mode,
    out_path: &Path,
    ignore_missing: bool,
    delay_selection: Option<riv_elab::DelaySelection>,
    warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<Vec<String>, riv_elab::ElabError> {
    let mut ld = Loader::new(include_dirs, predefines);
    let mut rendered: Vec<String> = Vec::new();
    let mut resolved: Vec<String> = Vec::new();
    for f in sources {
        if let Some(text) = ld.load_file(f, mode != Mode::E) {
            rendered.push(text);
        }
    }

    // -E routing: no parse happened (oracle-measured: syntax errors pass
    // -E); default output = a.out, `-o -` = stdout.
    if mode == Mode::E {
        if ld.pp_errors > 0 {
            eprint!("{}", ld.render_errors());
            return Err(ld.pp_failure());
        }
        let text = rendered.concat();
        if out_path == Path::new("-") {
            print!("{text}");
        } else if let Err(e) = std::fs::write(out_path, &text) {
            return Err(write_failure(out_path, e));
        }
        return Ok(resolved);
    }

    // -y library search (R1): fixpoint over instantiated-but-undefined
    // module names — each is looked up as <dir>/<name><suffix>, pp+parsed
    // into the pool, and the scan repeats until no progress (transitive
    // loads, oracle-measured). Deterministic order via BTreeSet.
    if !lib_dirs.is_empty() {
        let suffixes: Vec<String> = if lib_suffixes.is_empty() {
            vec![".v".to_string()]
        } else {
            lib_suffixes.to_vec()
        };
        let mut parsed_paths: std::collections::HashSet<PathBuf> =
            std::collections::HashSet::new();
        loop {
            let mut inst: std::collections::BTreeSet<String> =
                std::collections::BTreeSet::new();
            for m in &ld.modules {
                collect_inst_names(&m.items, &mut inst);
            }
            let defined: std::collections::BTreeSet<String> =
                ld.modules.iter().map(|m| m.name.clone()).collect();
            let mut progress = false;
            'scan: for name in inst.difference(&defined) {
                for dir in lib_dirs {
                    for suf in &suffixes {
                        let p = dir.join(format!("{name}{suf}"));
                        if !p.is_file() || parsed_paths.contains(&p) {
                            continue;
                        }
                        parsed_paths.insert(p.clone());
                        ld.load_file(&p.to_string_lossy(), true);
                        resolved.push(p.to_string_lossy().to_string());
                        progress = true;
                        continue 'scan;
                    }
                }
            }
            if !progress {
                break;
            }
        }
    }

    // Stage classification (oracle-measured exits; text free per rivD5):
    // pp class (missing file included) = 1, parse class = 2. Elaboration
    // errors stay exit 2 via riv-elab.
    if ld.pp_errors > 0 {
        eprint!("{}", ld.render_errors());
        return Err(ld.pp_failure());
    }
    if ld.parse_errors > 0 {
        eprint!("{}", ld.render_errors());
        return Err(riv_elab::ElabError {
            message: format!("riverilog: {} error(s) during parsing", ld.parse_errors),
            exit: 2,
        });
    }

    // -P (U-3b, oracle-measured): the named top is used only when defined;
    // an absent name prints the oracle's stderr line and compilation falls
    // back to auto-detect (exit follows the compile, measured 0).
    let top = match ptop {
        Some(p) => {
            if ld.modules.iter().any(|m| &m.name == p) {
                Some(p)
            } else {
                eprintln!("<command line>: error: invalid value specified for defparam: {p}");
                None
            }
        }
        None => top,
    };

    let design = riv_elab::elaborate(riv_elab::ElabInput {
        modules: &ld.modules,
        sm: &ld.sm,
        top,
        files: &ld.files,
        allow_missing_instances: ignore_missing,
        delay_selection,
        warnings: Some(warnings),
    })?;
    if mode == Mode::NullTarget {
        // Oracle-measured: -t null writes nothing, even with -o.
        return Ok(resolved);
    }
    std::fs::write(out_path, design.write()).map_err(|e| write_failure(out_path, e))?;
    Ok(resolved)
}

/// Output-file write failure — rivD13 flipped to ALIGNED (U-3b): the
/// oracle's shape is `<path>: <errno text>\nerror: Code generator failure: -1`
/// with **exit 255** (measured: ENOENT / EACCES / EISDIR all identical).
fn write_failure(path: &Path, e: std::io::Error) -> riv_elab::ElabError {
    let msg = match e.raw_os_error() {
        Some(2) => "No such file or directory",
        Some(13) => "Permission denied",
        Some(21) => "Is a directory",
        _ => {
            return riv_elab::ElabError {
                message: format!("riverilog: cannot write {}: {e}", path.display()),
                exit: 2,
            };
        }
    };
    riv_elab::ElabError {
        message: format!("{}: {msg}\nerror: Code generator failure: -1", path.display()),
        exit: 255,
    }
}

/// `-f`/`-c` command file (U-3b, oracle-measured grammar): each non-blank
/// line is ONE path token (no whitespace splitting — "a b" is a single
/// weird filename); `#`-initial lines are comments; a line that looks like
/// an option is a parse failure reported at the 1-based non-comment line
/// number, exit 1.
fn read_cmd_file(f: &str, sources: &mut Vec<String>) -> Result<(), String> {
    let text = std::fs::read_to_string(f).map_err(|_| {
        format!("riverilog: cannot open command file {f} for reading.\n")
    })?;
    let mut n = 0usize;
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        n += 1;
        if t.starts_with('-') {
            return Err(format!(
                "Error: unable to parse line {n} in cmd file {f}.\n\
                 riverilog: parsing failed in base command file {f}.\n"
            ));
        }
        sources.push(t.to_string());
    }
    Ok(())
}

/// Instantiated module names, static lexical walk: generate bodies are
/// traversed structurally (GenIf/GenFor/GenCase/GenBlock) so nested
/// instances count; genvar values are NOT expanded (name collection is
/// pre-elaboration, the missing-set only needs the lexically present
/// names).
fn collect_inst_names(
    items: &[vl_syntax::ast::ModuleItem],
    out: &mut std::collections::BTreeSet<String>,
) {
    use vl_syntax::ast::ModuleItem;
    for it in items {
        match it {
            ModuleItem::Instance(i) => {
                out.insert(i.mod_name.clone());
            }
            ModuleItem::GenerateRegion(items) => collect_inst_names(items, out),
            ModuleItem::GenIf { then, els, .. } => {
                collect_inst_names(then, out);
                if let Some(els) = els {
                    collect_inst_names(els, out);
                }
            }
            ModuleItem::GenFor { items, .. } => collect_inst_names(items, out),
            ModuleItem::GenCase { items, default, .. } => {
                for (_, body) in items {
                    collect_inst_names(body, out);
                }
                if let Some(d) = default {
                    collect_inst_names(d, out);
                }
            }
            ModuleItem::GenBlock { items, .. } => collect_inst_names(items, out),
            _ => {}
        }
    }
}
