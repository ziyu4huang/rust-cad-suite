//! vl-spec gate: the lockstep harness on a tiny counter DUT — projection
//! match (correct golden), mismatch detection (deliberately wrong golden),
//! stateful invariants, and report rendering.

use vl_spec::{
    build_sim, GoldenModel, Invariant, LockstepSpec, SignalId, SpecReport, SpecViolation,
    ViolationKind,
};

const COUNTER: &str = "\
module tb;
  reg clk = 0;
  reg rst_n = 0;
  reg [7:0] q;
  always @(posedge clk or negedge rst_n)
    if (!rst_n) q <= 0;
    else q <= q + 1;
endmodule
";

/// Obs = the counter value the model believes the RTL holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Obs(u64);

struct CountModel {
    q: u64,
    /// false = a deliberately wrong golden (never counts) — must produce
    /// Lockstep violations from cycle 1. The same-author defense is
    /// exercised end-to-end in spec-i2c; here it just proves the harness
    /// fires.
    correct: bool,
}
impl GoldenModel for CountModel {
    type Input = ();
    type Obs = Obs;
    fn step(&mut self, _input: &()) {
        if self.correct {
            self.q = (self.q + 1) & 0xff;
        }
    }
    fn observe(&self) -> Obs {
        Obs(self.q)
    }
}

/// Stateful invariant: q must increment by exactly one per cycle (after the
/// first observed value).
struct IncrementsByOne {
    q: SignalId,
    prev: Option<u64>,
}
impl Invariant for IncrementsByOne {
    fn name(&self) -> &'static str {
        "q_increments_by_one"
    }
    fn check(&mut self, ctx: &vl_spec::CycleCtx, out: &mut Vec<SpecViolation>) {
        let q = ctx.sigs.u64(ctx.sim, self.q).unwrap_or(u64::MAX);
        if let Some(p) = self.prev {
            if q != ((p + 1) & 0xff) {
                out.push(SpecViolation {
                    time: ctx.time,
                    cycle: ctx.cycle,
                    kind: ViolationKind::Invariant(self.name()),
                    expected: ((p + 1) & 0xff).to_string(),
                    actual: q.to_string(),
                    message: "counter must count by one".into(),
                });
            }
        }
        self.prev = Some(q);
    }
}

/// Deliberately wrong invariant (expects q to DECREMENT) — trips every
/// cycle; proves stateful invariants render with their name.
struct Decrements {
    q: SignalId,
    prev: Option<u64>,
}
impl Invariant for Decrements {
    fn name(&self) -> &'static str {
        "q_decrements"
    }
    fn check(&mut self, ctx: &vl_spec::CycleCtx, out: &mut Vec<SpecViolation>) {
        let q = ctx.sigs.u64(ctx.sim, self.q).unwrap_or(u64::MAX);
        if let Some(p) = self.prev {
            if q != (p.wrapping_sub(1) & 0xff) {
                out.push(SpecViolation {
                    time: ctx.time,
                    cycle: ctx.cycle,
                    kind: ViolationKind::Invariant(self.name()),
                    expected: "prev-1".into(),
                    actual: q.to_string(),
                    message: "deliberately wrong invariant".into(),
                });
            }
        }
        self.prev = Some(q);
    }
}

/// Build the harness and run the init sequence: assert async reset (x->0 is
/// a negedge on rst_n), release it before the first clocked cycle.
fn build_spec(correct: bool) -> LockstepSpec<CountModel> {
    let sim = build_sim(&[("t.v", COUNTER)], "tb").expect("build");
    let mut spec =
        LockstepSpec::new(sim, "clk", 5, CountModel { q: 0, correct }).expect("clk binds");
    let rst = spec.bind_signal("rst_n").expect("rst_n");
    spec.put_int(rst, 0).expect("put rst");
    spec.run_until(10).expect("reset slot");
    spec.put_int(rst, 1).expect("release rst");
    spec.run_until(20).expect("release slot");
    spec
}

#[test]
fn lockstep_match_clean() {
    let mut spec = build_spec(true);
    let q = spec.bind_signal("q").expect("q");
    let mut spec = spec
        .probe(move |sigs, sim| Ok(Obs(sigs.u64(sim, q)?)))
        .invariant(IncrementsByOne { q, prev: None });
    for _ in 0..16 {
        spec.cycle(&()).unwrap();
    }
    let report: SpecReport = spec.finish();
    assert_eq!(report.cycles_run, 16);
    report.assert_empty();
}

#[test]
fn wrong_golden_is_caught_every_cycle() {
    let mut spec = build_spec(false);
    let q = spec.bind_signal("q").expect("q");
    let mut spec = spec.probe(move |sigs, sim| Ok(Obs(sigs.u64(sim, q)?)));
    for _ in 0..8 {
        spec.cycle(&()).unwrap();
    }
    let report = spec.finish();
    assert_eq!(report.violations.len(), 8, "every cycle must mismatch");
    assert!(matches!(
        report.violations[0].kind,
        ViolationKind::Lockstep("projection")
    ));
    let text = report.render_text();
    assert!(text.contains("RTL projection != golden model"));
    assert!(text.contains("VIOLATION cycle=0"));
}

#[test]
fn invariant_violations_render_with_name() {
    let mut spec = build_spec(true);
    let q = spec.bind_signal("q").expect("q");
    let mut spec = spec
        .probe(move |sigs, sim| Ok(Obs(sigs.u64(sim, q)?)))
        .invariant(Decrements { q, prev: None });
    for _ in 0..4 {
        spec.cycle(&()).unwrap();
    }
    let report = spec.finish();
    // Stateful invariant: the first cycle only seeds the prev value, so 4 cycles
    // produce 3 checks.
    assert_eq!(report.violations.len(), 3);
    assert_eq!(
        report.violations[0].kind,
        ViolationKind::Invariant("q_decrements")
    );
    assert!(report.render_text().contains("invariant `q_decrements`"));
}

#[test]
fn stateful_invariant_stays_clean_when_correct() {
    let mut spec = build_spec(true);
    let q = spec.bind_signal("q").expect("q");
    let mut spec = spec
        .probe(move |sigs, sim| Ok(Obs(sigs.u64(sim, q)?)))
        .invariant(IncrementsByOne { q, prev: None });
    for _ in 0..32 {
        spec.cycle(&()).unwrap();
    }
    let report = spec.finish();
    report.assert_empty();
}
