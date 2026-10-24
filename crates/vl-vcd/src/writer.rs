//! VCD value-change dump writer (IEEE 1364-2005 §18).
//!
//! Dump policy: variables are all elaborated net/reg/integer cells (params,
//! genvars and function/task statics are excluded at registration time by the
//! simulator); ids are assigned in sorted-path order so output is
//! deterministic. Change emission is per-time-slot: the scheduler flushes
//! everything that changed while processing time T under a single `#T` header
//! (combinational settling inside a slot collapses to the final value — that
//! is also what the oracle's dump does, and what the alias-folding crosscheck
//! compares).
//!
//! Values are 4-state bit strings MSB-first (`"0101xz"`; scalars are one
//! char). The simulator's `Bits` maps 1:1 onto these strings per width, so
//! byte output is unchanged from when this module lived inside vl-sim.

use std::collections::BTreeMap;

pub struct VcdWriter {
    /// (flat name, scope path incl. leaf, width) — sorted by flat name.
    vars: Vec<(String, Vec<String>, u32)>,
    ids: BTreeMap<String, String>,
    body: String,
    last: BTreeMap<String, String>,
    pending: BTreeMap<String, String>,
    now_emitted: Option<u64>,
    started: bool,
    header_done: bool,
    timescale: String,
    /// `$version` identity (rverilator by default; riverilog passes its
    /// own name via `new_named` — ivD1 self-identification).
    program_name: String,
}

/// Printable ASCII 33..=126 (94 codes); multi-char beyond that.
fn id_code(n: usize) -> String {
    const BASE: u32 = 33;
    const LEN: u32 = 94;
    let mut v = n as u32;
    let mut s = Vec::new();
    loop {
        s.push((BASE + v % LEN) as u8 as char);
        v /= LEN;
        if v == 0 {
            break;
        }
        v -= 1;
    }
    s.into_iter().collect()
}

impl VcdWriter {
    pub fn new(vars: Vec<(String, Vec<String>, u32)>, timescale: &str) -> VcdWriter {
        Self::new_named(vars, "rverilator (rust-cad-suite)", timescale)
    }

    /// `program_name` lands in the `$version` header; the default keeps
    /// the historical `rverilator` string so e2e expect.vcd goldens stay
    /// byte-identical (ADR-0004 zero-drift anchor).
    pub fn new_named(
        mut vars: Vec<(String, Vec<String>, u32)>,
        program_name: &str,
        timescale: &str,
    ) -> VcdWriter {
        vars.sort_by(|a, b| a.0.cmp(&b.0));
        let ids = vars
            .iter()
            .enumerate()
            .map(|(i, (name, _, _))| (name.clone(), id_code(i)))
            .collect();
        VcdWriter {
            vars,
            ids,
            body: String::new(),
            last: BTreeMap::new(),
            pending: BTreeMap::new(),
            now_emitted: None,
            started: false,
            header_done: false,
            timescale: timescale.to_string(),
            program_name: program_name.to_string(),
        }
    }

    pub fn start(&mut self) {
        self.started = true;
    }

    pub fn started(&self) -> bool {
        self.started
    }

    pub fn record_change(&mut self, name: &str, value: &str) {
        if self.started {
            self.pending.insert(name.to_string(), value.to_string());
        }
    }

    fn header(&self) -> String {
        let mut h = String::from("$date\n  (recorded)\n$end\n");
        h.push_str(&format!("$version\n  {}\n$end\n", self.program_name));
        h.push_str(&format!("$timescale {} $end\n", self.timescale));
        let mut tree: BTreeMap<Vec<String>, Vec<(String, u32, String)>> = BTreeMap::new();
        for (name, path, w) in &self.vars {
            let id = self.ids[name].clone();
            let leaf = path.last().cloned().unwrap_or_else(|| name.clone());
            let scope = path[..path.len().saturating_sub(1)].to_vec();
            tree.entry(scope).or_default().push((leaf, *w, id));
        }
        let mut depth = 0usize;
        let mut open: Vec<String> = Vec::new();
        for (path, vs) in &tree {
            // Pop scopes that are not a prefix of `path`.
            while !path.starts_with(&open) {
                h.push_str("$upscope $end\n");
                open.pop();
                depth -= 1;
            }
            while depth < path.len() {
                h.push_str(&format!("$scope module {} $end\n", path[depth]));
                open.push(path[depth].clone());
                depth += 1;
            }
            for (leaf, w, id) in vs {
                h.push_str(&format!("$var wire {w} {id} {leaf} $end\n"));
            }
        }
        while depth > 0 {
            h.push_str("$upscope $end\n");
            depth -= 1;
        }
        h.push_str("$enddefinitions $end\n");
        h
    }

    fn value_line(&self, name: &str, value: &str) -> Option<String> {
        let id = self.ids.get(name)?;
        let w = self.vars.iter().find(|(n, _, _)| n == name)?.2;
        if w == 1 {
            let c = value.chars().next()?;
            Some(format!("{c}{id}\n"))
        } else {
            Some(format!("b{value} {id}\n"))
        }
    }

    /// `#0` initialization dump (`$dumpvars` block).
    pub fn dump_t0(&mut self, values: &BTreeMap<String, String>) {
        if !self.started || self.header_done {
            return;
        }
        self.header_done = true;
        let header = self.header();
        self.body.push_str(&header);
        self.body.push_str("#0\n$dumpvars\n");
        let names: Vec<String> = self.vars.iter().map(|v| v.0.clone()).collect();
        for name in names {
            if let Some(value) = values.get(&name) {
                if let Some(line) = self.value_line(&name, value) {
                    self.body.push_str(&line);
                }
                self.last.insert(name, value.clone());
            }
        }
        self.body.push_str("$end\n");
        self.now_emitted = Some(0);
        self.pending.clear();
    }

    /// Flush changes accumulated while processing time `now`.
    pub fn flush_at(&mut self, now: u64) {
        if !self.started || !self.header_done {
            self.pending.clear();
            return;
        }
        let pending = std::mem::take(&mut self.pending);
        let mut lines = String::new();
        for (name, value) in &pending {
            if self.last.get(name) == Some(value) {
                continue;
            }
            if let Some(line) = self.value_line(name, value) {
                lines.push_str(&line);
            }
            self.last.insert(name.clone(), value.clone());
        }
        if lines.is_empty() {
            return;
        }
        if self.now_emitted != Some(now) {
            self.body.push_str(&format!("#{now}\n"));
            self.now_emitted = Some(now);
        }
        self.body.push_str(&lines);
    }

    pub fn finish(self) -> String {
        if !self.header_done {
            return String::new();
        }
        self.body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_printable_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for i in 0..300 {
            let id = id_code(i);
            assert!(id.chars().all(|c| ('!'..='~').contains(&c)), "{id}");
            assert!(seen.insert(id));
        }
        assert_eq!(id_code(0), "!");
        assert_eq!(id_code(93), "~");
        assert_eq!(id_code(94), "!!");
    }

    #[test]
    fn header_and_changes_shape() {
        let vars = vec![
            ("dut.q".to_string(), vec!["dut".into(), "q".into()], 8u32),
            ("clk".to_string(), vec!["clk".into()], 1u32),
        ];
        let mut w = VcdWriter::new(vars, "1ps");
        w.start();
        let mut vals = BTreeMap::new();
        vals.insert("clk".to_string(), "0".to_string());
        vals.insert("dut.q".to_string(), "00000011".to_string());
        w.dump_t0(&vals);
        w.record_change("clk", "1");
        w.flush_at(5000);
        w.record_change("clk", "1"); // no change
        w.flush_at(6000);
        let out = w.finish();
        assert!(out.contains("$scope module dut $end\n$var wire 8 \" q $end\n$upscope $end"));
        assert!(out.contains("$var wire 1 ! clk $end"));
        assert!(out.contains("#0\n$dumpvars\n0!\nb00000011 \"\n$end\n"));
        assert!(out.contains("#5000\n1!\n"));
        assert!(!out.contains("#6000"), "unchanged values emit nothing");
    }

    #[test]
    fn program_name_is_parametric_with_rverilator_default() {
        let vars = vec![("x".to_string(), vec!["x".into()], 1u32)];
        let mut w = VcdWriter::new(vars.clone(), "1ps");
        w.start();
        let mut vals = BTreeMap::new();
        vals.insert("x".to_string(), "0".to_string());
        w.dump_t0(&vals);
        assert!(w.finish().contains("$version\n  rverilator (rust-cad-suite)\n$end"));

        let mut w = VcdWriter::new_named(vars, "riverilog 0.1.0 (oss-cad-suite Rust rewrite)", "1ps");
        w.start();
        let mut vals = BTreeMap::new();
        vals.insert("x".to_string(), "0".to_string());
        w.dump_t0(&vals);
        assert!(w.finish().contains("$version\n  riverilog 0.1.0 (oss-cad-suite Rust rewrite)\n$end"));
    }

    #[test]
    fn four_state_values_round_trip() {
        let vars = vec![("dut.z".to_string(), vec!["dut".into(), "z".into()], 4u32)];
        let mut w = VcdWriter::new(vars, "1ps");
        w.start();
        let mut vals = BTreeMap::new();
        vals.insert("dut.z".to_string(), "xz01".to_string());
        w.dump_t0(&vals);
        let out = w.finish();
        assert!(out.contains("bxz01 !"));
    }
}
