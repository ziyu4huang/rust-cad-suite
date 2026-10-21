//! Corpus policy lint (3-tiers+1 mechanical tier for AGENTS §2 rules).
//!
//! R1 same-edge race: a BLOCKING write (`x = …`) right after `@(posedge|negedge …)`
//!    with no intervening `#delay`, where `x` is read inside an edge-triggered
//!    `always` block in the same file. IEEE-nondeterministic → not a parity
//!    test. Fix pattern: `@(posedge clk); #1; x = …;` (corpus/e2e/26, 43).
//!    Suppress a confirmed-false-positive with `// race-ok` on the line.
//! R2 every case has expected.stdout (recorded, never hand-written).
//! R3 a TB that calls $dumpvars ships an oracle-ref.vcd (M6 parity evidence),
//!    and vice versa.

use std::path::Path;

fn idents(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
            cur.push(c);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn edge_read_vars(src: &str) -> std::collections::HashSet<String> {
    // Identifiers appearing in edge-triggered always blocks (approx: from an
    // `always @(posedge|negedge` line until the next top-level construct).
    let mut set = std::collections::HashSet::new();
    let mut in_block = false;
    for line in src.lines() {
        let t = line.trim_start();
        if t.starts_with("always") {
            in_block = t.contains("posedge") || t.contains("negedge");
        } else if t.starts_with("initial") || t.starts_with("endmodule") || t.starts_with("task")
            || t.starts_with("function")
        {
            in_block = false;
        }
        if in_block {
            for id in idents(line) {
                set.insert(id);
            }
        }
    }
    set
}

fn race_findings(src: &str) -> Vec<String> {
    let edge_vars = edge_read_vars(src);
    let lines: Vec<&str> = src.lines().collect();
    let mut findings = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim_start();
        if t.starts_with("always") || line.contains("race-ok") {
            continue;
        }
        let Some(at) = line.find("@(") else { continue };
        if !(line[at..].contains("posedge") || line[at..].contains("negedge")) {
            continue;
        }
        // Statements after the event control on this line, then following
        // lines until a delay (`#`) or another event control.
        let mut window = String::new();
        let rest = &line[at..];
        if let Some(close) = rest.find(')') {
            let tail = &rest[close + 1..];
            // Same-line remainder stops at the first delay/event control.
            let cut = tail
                .find('#')
                .into_iter()
                .chain(tail.find("@("))
                .min()
                .unwrap_or(tail.len());
            window.push_str(&tail[..cut]);
            if cut < tail.len() {
                window.push('\n');
                // A delay on this line ends the window entirely.
                for stmt in window.split(';') {
                    let _ = stmt;
                }
                let _ = &lines;
                // fallthrough with closed window
                let w = window.clone();
                window = w;
                // mark closed
                let closed = true;
                if closed {
                    // evaluate immediately below without extending lines
                }
            }
        }
        window.push('\n');
        let mut j = i + 1;
        while j < lines.len() {
            let lj = lines[j].trim();
            if lj.contains('#') || lj.contains("@(") || lj.starts_with("end") {
                break;
            }
            window.push_str(lj);
            window.push('\n');
            j += 1;
        }
        for stmt in window.split(';') {
            let s = stmt.trim();
            // blocking assignment only (`=` not `<=`, `==`, `!=`, `>=`)
            if let Some(eq) = s.find('=') {
                let before = &s[..eq];
                let after_ch = s[eq + 1..].chars().next().unwrap_or(' ');
                let prev_ch = before.chars().last().unwrap_or(' ');
                if after_ch == '=' || matches!(prev_ch, '<' | '>' | '!' | '=') {
                    continue;
                }
                let lhs = idents(before);
                if let Some(v) = lhs.last() {
                    if edge_vars.contains(v) {
                        findings.push(format!(
                            "line {}: blocking write to `{v}` right after an edge wait; `{v}` is read by an edge-triggered always → same-edge race (use `@(posedge ..); #1; {v} = …` or `// race-ok`)",
                            i + 1
                        ));
                    }
                }
            }
        }
    }
    findings
}

#[test]
fn corpus_policy_lint() {
    let corpus = vl_difftest::corpus_dir();
    let mut problems = Vec::new();
    let mut dirs: Vec<_> = std::fs::read_dir(&corpus)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for d in &dirs {
        let name = d.file_name().unwrap().to_string_lossy().to_string();
        if !d.join("expected.stdout").exists() {
            problems.push(format!("{name}: missing expected.stdout (R2)"));
        }
        let mut dumps = false;
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.extension().map(|x| x == "v").unwrap_or(false) {
                let src = std::fs::read_to_string(&p).unwrap();
                if src.contains("$dumpvars") {
                    dumps = true;
                }
                for f in race_findings(&src) {
                    problems.push(format!("{name}/{}: {f} (R1)", p.file_name().unwrap().to_string_lossy()));
                }
            }
        }
        let has_ref = d.join("oracle-ref.vcd").exists();
        if dumps && !has_ref {
            problems.push(format!("{name}: $dumpvars without oracle-ref.vcd (R3; run scripts/oracle_vcd.sh)"));
        }
        if has_ref && !dumps {
            problems.push(format!("{name}: oracle-ref.vcd but TB never $dumpvars (R3)"));
        }
    }
    assert!(problems.is_empty(), "corpus policy violations:\n{}", problems.join("\n"));
    let _ = Path::new("");
}

#[test]
fn race_lint_catches_the_documented_pattern() {
    let bad = "module tb;\nreg clk=0, ld=0; reg [7:0] q;\nalways @(posedge clk) if (ld) q <= 1;\ninitial begin\n  @(posedge clk); ld = 1;\n  @(posedge clk);\nend\nendmodule\n";
    assert_eq!(race_findings(bad).len(), 1, "{:?}", race_findings(bad));
    let good = "module tb;\nreg clk=0, ld=0; reg [7:0] q;\nalways @(posedge clk) if (ld) q <= 1;\ninitial begin\n  @(posedge clk); #1; ld = 1;\n  @(posedge clk);\nend\nendmodule\n";
    assert!(race_findings(good).is_empty(), "{:?}", race_findings(good));
    let nba = "module tb;\nreg clk=0, ld=0;\nalways @(posedge clk) if (ld) $display(1);\ninitial begin\n  @(posedge clk); ld <= 1;\nend\nendmodule\n";
    assert!(race_findings(nba).is_empty(), "NBA writes are region-safe");
}

#[test]
fn normalization_contract() {
    let raw = "- S i m u l a t i o n   R e p o r t: Verilator 5.050\n-Info: tb.v:9: $dumpvar ignored, as Verilated without --trace\n-7div2=-3 -7mod2=-1\n- /x/tb.v:24: Verilog $finish\nPASS\n- Verilator: $finish at 76ns; walltime 0.000 s\n";
    let got = vl_difftest::normalize_oracle_stdout(raw);
    assert_eq!(got, "-7div2=-3 -7mod2=-1\nPASS\n", "design lines starting with `-` survive; tool lines don't");
}
