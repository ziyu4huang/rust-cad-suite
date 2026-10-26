//! `rivvp` — the riverilog runtime entrypoint (vvp-shaped CLI).
//!
//! Measured vvp contract (riv-cli goldens): `-h` on stdout exits 0;
//! missing input file = "Unable to open input file." + exit 255; no input
//! = "vvp: no input file." shape + 255. Since C3 the runtime loads RIV-1
//! designs (riv-ir reader) and interprets them (riv-kernel), attaching the
//! vl-vcd writer through the kernel's DumpSink hook (ADR-0004).

use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

use riv_kernel::{DumpSink, Kernel};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "\
Usage: rivvp [-n] [-N] [-i] [-q] [-l logfile] [-M moduledir] [-m module]
             design-file [plusargs...]";

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut design: Option<String> = None;
    let mut plusargs: Vec<String> = Vec::new();
    // -n: $stop ≡ $finish (rivD2; measured: the stop line prints, then
    // the run ends). Default = banner + continue (EOF semantics).
    // -N: same halt but the process exits 1 (measured on vvp 13.0).
    let mut stop_as_finish = false;
    let mut stop_exit = 0i32;
    // -q: stdout fully silent (tool lines AND design $display — measured);
    // dump.vcd still written. outcome.output is always collected so -l
    // keeps mirroring under -q.
    let mut quiet = false;
    // -l <file>: mirror everything that would reach stdout into the file
    // (measured: log == direct stdout, byte-equal).
    let mut log_file: Option<String> = None;
    // -l - (rivD14, contract settled by U-3b probes): no file is created;
    // stderr receives a full copy of the stdout content. -q silences the
    // stdout side only — the stderr copy survives (measured vvp 13.0).
    let mut log_dash = false;
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
            "-n" => stop_as_finish = true,
            "-vcd" => {
                // VCD is the only dumper (ADR-0005); the flag is accepted
                // as the default selection.
            }
            "-fst" => {
                eprintln!("rivvp: FST dumper is not planned (VCD only; see docs/RIVERILOG-PLAN.md §5)");
                return ExitCode::from(1);
            }
            "-N" => {
                stop_as_finish = true;
                stop_exit = 1;
            }
            // -i: interactive (line-buffered) mode — accepted no-op; the
            // buffering difference is unobservable in batch corpus runs
            // (noted in REGISTRY riv-cli row).
            "-i" => {}
            "-q" => quiet = true,
            "-l" => match argv.get(i + 1) {
                // Measured oracle contract (vvp 13.0, U-3b probes): `-l -`
                // creates no file — the full copy reaches stdout AND stderr.
                Some(f) if f == "-" => {
                    log_dash = true;
                    i += 1;
                }
                Some(f) => {
                    log_file = Some(f.clone());
                    i += 1;
                }
                None => {
                    eprintln!("rivvp: option -l requires an argument");
                    return ExitCode::from(1);
                }
            },
            // VPI module face (-M/-m): R7 scope (docs/RIVERILOG-PLAN.md §8)
            // — hard error with the pointer instead of the old silent
            // arg-swallow (AGENTS §3).
            "-M" | "-m" => {
                eprintln!("rivvp: VPI module loading (-M/-m) lands at R7 (docs/RIVERILOG-PLAN.md §8)");
                return ExitCode::from(1);
            }
            _ if a.starts_with('+') => plusargs.push(a),
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

    // IVERILOG_DUMPER env (R5): vcd is the only supported dumper.
    if let Ok(dumper) = std::env::var("IVERILOG_DUMPER") {
        if !dumper.eq_ignore_ascii_case("vcd") && !dumper.is_empty() {
            eprintln!("rivvp: IVERILOG_DUMPER={dumper} is not supported (VCD only; see docs/RIVERILOG-PLAN.md §5)");
            return ExitCode::from(1);
        }
    }

    let Some(design) = design else {
        eprintln!("rivvp: no input file.");
        return ExitCode::from(255);
    };

    // An argument that does not exist reports the oracle's exact shape.
    if !Path::new(&design).exists() {
        eprintln!("{design}: Unable to open input file.");
        return ExitCode::from(255);
    }

    let text = match std::fs::read_to_string(&design) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("rivvp: {design}: cannot read: {e}");
            return ExitCode::from(1);
        }
    };
    let d = match riv_ir::read(&text) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("rivvp: {design}: not a riv design ({e})");
            return ExitCode::from(1);
        }
    };
    let mut kernel = match Kernel::new(d.clone()) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("rivvp: design load: {}", e.0);
            return ExitCode::from(1);
        }
    };
    // Plusargs feed $test$plusargs/$value$plusargs (C5/R3-A); -n feeds
    // the rivD2 $stop semantics.
    kernel.set_plusargs(plusargs);
    kernel.stop_as_finish = stop_as_finish;
    kernel.stop_exit = stop_exit;
    let mut sink = VcdSink::new(&d);
    let outcome = kernel.run(&mut sink);
    if let Some(lf) = &log_file {
        if let Err(e) = std::fs::write(lf, &outcome.output) {
            eprintln!("rivvp: cannot write log {lf}: {e}");
            return ExitCode::from(1);
        }
    }
    if !quiet {
        print!("{}", outcome.output);
    }
    if log_dash {
        eprint!("{}", outcome.output);
    }
    ExitCode::from(u8::try_from(outcome.exit.max(0)).unwrap_or(1))
}

/// DumpSink over the vl-vcd writer (single VCD core, ADR-0004). The VCD
/// text is written when the sink finishes; content correctness is the R5
/// cross-check's business (C3 = minimal, must not crash).
struct VcdSink {
    timescale: String,
    path: Option<String>,
    writer: Option<vl_vcd::VcdWriter>,
    var_list: Vec<(String, Vec<String>, u32)>,
}

impl VcdSink {
    fn new(d: &riv_ir::Design) -> VcdSink {
        // Var registry: flat name `top.var`, scope path [top, var],
        // declared width — deterministic order comes from the writer's
        // sort.
        // Scope tree (RIVERILOG-PLAN §8.1 hygiene ①): dotted var paths
        // split into real VCD scopes (tb.dut.q → $scope tb $scope dut $var
        // q). Event vars carry no value and arrays dump per-element at R5 —
        // both stay out of the registry. Collapsed-port dead vars keep
        // their row (constant x); the cross-compare gate intersects.
        // Raw var paths ("a", "dut.q") — the legacy VCD shape; the top
        // module carries no prefix (ADR-0009 U-1 byte-compat).
        let var_list: Vec<(String, Vec<String>, u32)> = d
            .vars
            .iter()
            .filter(|v| {
                v.kind != riv_ir::VarKind::Event && v.kind != riv_ir::VarKind::Array
            })
            .map(|v| {
                (
                    v.name.clone(),
                    v.name.split('.').map(|c| c.to_string()).collect(),
                    v.width,
                )
            })
            .collect();
        VcdSink {
            timescale: d.meta.precision.clone(),
            path: None,
            writer: None,
            var_list,
        }
    }
}

impl DumpSink for VcdSink {
    fn opened(&mut self, path: &str) {
        self.path = Some(path.to_string());
    }

    fn dump_section(&mut self, now: u64, name: &str, values: &BTreeMap<String, String>) {
        if let Some(w) = self.writer.as_mut() {
            w.dump_section(now, name, values);
        }
    }

    fn dump_t0(&mut self, values: &BTreeMap<String, String>) {
        let vars = self.var_list.clone();
        let mut w = vl_vcd::VcdWriter::new_named(
            vars,
            &format!("rivvp (riverilog runtime) version {VERSION}"),
            &self.timescale,
        );
        w.start();
        w.dump_t0(values);
        self.writer = Some(w);
    }

    fn change(&mut self, name: &str, value: &str) {
        if let Some(w) = &mut self.writer {
            w.record_change(name, value);
        }
    }

    fn flush_slot(&mut self, now: u64) {
        if let Some(w) = &mut self.writer {
            w.flush_at(now);
        }
    }

    fn finish(&mut self) {
        if let (Some(w), Some(path)) = (self.writer.take(), &self.path) {
            let text = w.finish();
            let _ = std::fs::write(path, text);
        }
    }
}
