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
    // Plusargs feed $test$plusargs/$value$plusargs (C5/R3-A).
    kernel.set_plusargs(plusargs);
    let mut sink = VcdSink::new(&d);
    let outcome = kernel.run(&mut sink);
    print!("{}", outcome.output);
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
        let var_list: Vec<(String, Vec<String>, u32)> = d
            .vars
            .iter()
            .map(|v| {
                (
                    format!("{}.{}", d.meta.top, v.name),
                    vec![d.meta.top.clone(), v.name.clone()],
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
