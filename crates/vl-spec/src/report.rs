//! Spec-verification result types: violations and the report consumed by
//! tests (text) and, in a later arc, the browser demo (structured).

#[derive(Debug, Clone, PartialEq)]
pub enum ViolationKind {
    /// A named per-cycle invariant tripped.
    Invariant(&'static str),
    /// The RTL projection disagreed with the golden model.
    Lockstep(&'static str),
}

#[derive(Debug, Clone)]
pub struct SpecViolation {
    /// Precision-tick time at the sampling point.
    pub time: u64,
    /// Lockstep cycle index (one clk period per cycle).
    pub cycle: u64,
    pub kind: ViolationKind,
    pub expected: String,
    pub actual: String,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct SpecReport {
    pub violations: Vec<SpecViolation>,
    pub cycles_run: u64,
}

impl SpecReport {
    /// Canonical text form (cargo-test facing; deterministic).
    pub fn render_text(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "spec report: {} cycles, {} violation(s)\n",
            self.cycles_run,
            self.violations.len()
        ));
        for v in &self.violations {
            let kind = match &v.kind {
                ViolationKind::Invariant(n) => format!("invariant `{n}`"),
                ViolationKind::Lockstep(n) => format!("lockstep[{n}]"),
            };
            s.push_str(&format!(
                "  VIOLATION cycle={} t={}: {}\n    expected: {}\n    actual:   {}\n    {}\n",
                v.cycle, v.time, kind, v.expected, v.actual, v.message
            ));
        }
        s
    }

    pub fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }

    /// Test helper: panic with the rendered report when not clean.
    pub fn assert_empty(&self) {
        assert!(
            self.is_clean(),
            "spec violations:\n{}",
            self.render_text()
        );
    }
}
