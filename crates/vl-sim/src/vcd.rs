//! VCD value-change dump writer (IEEE 1364-2005 §18) — M6.
//!
//! Dump policy: variables are all elaborated net/reg/integer cells (params,
//! genvars and function/task statics are excluded at registration time);
//! ids are assigned in sorted-path order so output is deterministic. Change
//! emission is per-time-slot: the scheduler flushes everything that changed
//! while processing time T under a single `#T` header (combinational settling
//! inside a slot collapses to the final value — that is also what the oracle's
//! dump does, and what the alias-folding crosscheck compares).

use crate::value::Bits;
use std::collections::BTreeMap;

pub struct VcdWriter {
    /// (flat name, scope path incl. leaf, width) — sorted by flat name.
    vars: Vec<(String, Vec<String>, u32)>,
    ids: BTreeMap<String, String>,
    body: String,
    last: BTreeMap<String, Bits>,
    pending: BTreeMap<String, Bits>,
    now_emitted: Option<u64>,
    started: bool,
    header_done: bool,
    timescale: String,
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
    pub fn new(mut vars: Vec<(String, Vec<String>, u32)>, timescale: &str) -> VcdWriter {
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
        }
    }

    pub fn start(&mut self) {
        self.started = true;
    }

    pub fn started(&self) -> bool {
        self.started
    }

    pub fn record_change(&mut self, name: &str, bits: Bits) {
        if self.started {
            self.pending.insert(name.to_string(), bits);
        }
    }

    fn header(&self) -> String {
        let mut h = String::from("$date\n  (recorded)\n$end\n");
        h.push_str("$version\n  rverilator (rust-cad-suite)\n$end\n");
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

    fn value_line(&self, name: &str, bits: &Bits) -> Option<String> {
        let id = self.ids.get(name)?;
        if bits.width == 1 {
            let c = if bits.mask & 1 != 0 {
                if bits.data & 1 != 0 {
                    'z'
                } else {
                    'x'
                }
            } else if bits.data & 1 != 0 {
                '1'
            } else {
                '0'
            };
            Some(format!("{c}{id}\n"))
        } else {
            Some(format!("b{} {id}\n", bits.fmt_bin()))
        }
    }

    /// `#0` initialization dump (`$dumpvars` block).
    pub fn dump_t0(&mut self, values: &BTreeMap<String, Bits>) {
        if !self.started || self.header_done {
            return;
        }
        self.header_done = true;
        let header = self.header();
        self.body.push_str(&header);
        self.body.push_str("#0\n$dumpvars\n");
        let names: Vec<String> = self.vars.iter().map(|v| v.0.clone()).collect();
        for name in names {
            if let Some(bits) = values.get(&name) {
                if let Some(line) = self.value_line(&name, bits) {
                    self.body.push_str(&line);
                }
                self.last.insert(name, *bits);
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
        for (name, bits) in &pending {
            if self.last.get(name) == Some(bits) {
                continue;
            }
            if let Some(line) = self.value_line(name, bits) {
                lines.push_str(&line);
            }
            self.last.insert(name.clone(), *bits);
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
        vals.insert("clk".to_string(), Bits::from_u64(1, 0).unwrap());
        vals.insert("dut.q".to_string(), Bits::from_u64(8, 3).unwrap());
        w.dump_t0(&vals);
        w.record_change("clk", Bits::from_u64(1, 1).unwrap());
        w.flush_at(5000);
        w.record_change("clk", Bits::from_u64(1, 1).unwrap()); // no change
        w.flush_at(6000);
        let out = w.finish();
        assert!(out.contains("$scope module dut $end\n$var wire 8 \" q $end\n$upscope $end"));
        assert!(out.contains("$var wire 1 ! clk $end"));
        assert!(out.contains("#0\n$dumpvars\n0!\nb00000011 \"\n$end\n"));
        assert!(out.contains("#5000\n1!\n"));
        assert!(!out.contains("#6000"), "unchanged values emit nothing");
    }
}
