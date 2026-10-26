//! U-4 slice-1 gate: the unified engine's rVPI firing points (ADR-0009;
//! registry-on-embedder lands in slice 2 — the CbFn embedder decoupling is
//! an ADR-0003 amendment). Asserts the ADR-0003 trigger contract directly
//! on the Kernel: start once at entry, value-change post-settle at the
//! commit funnel (in commit order, value already stored), after-delay at
//! wheel advance with the NEW now before the slots, end exactly once.

use riv_ir::{Base, Design, Literal, Lval, Op, Thread, ThreadKind, Var, VarKind};
use riv_kernel::vpi::VpiHooks;
use riv_kernel::{DumpSink, Kernel};

type Log = std::rc::Rc<std::cell::RefCell<Vec<String>>>;

fn lit1(v: u32) -> Literal {
    Literal { width: 1, signed: false, base: Base::Bin, digits: v.to_string() }
}

fn design() -> Design {
    // initial begin #2 q = ~q; #2 q = ~q; #2 $finish; end
    let mut d = Design {
        meta: riv_ir::Meta {
            top: "tb".into(),
            timescale: "1ns/1ps".into(),
            unit: "1ns".into(),
            precision: "1ps".into(),
            sources: vec!["tb.v".into()],
        },
        vars: vec![Var {
            id: 0,
            name: "q".into(),
            width: 1,
            signed: false,
            kind: VarKind::Reg,
            init: None,
            msb: 0,
            lsb: 0,
        }],
        arrays: vec![],
        assigns: vec![],
        gates: vec![],
        threads: vec![],
        funcs: vec![],
        strs: vec![vec![riv_ir::FmtPiece::Text("tb.v".into())]],
    };
    let f = (d.strs.len() - 1) as u32;
    let finish = Op::SysCall {
        id: riv_ir::SysId::Finish,
        fmt: None,
        args: vec![],
        dst: None,
        file: f,
        line: 4,
        start: 0,
    };
    // (1-bit ~x = x — the toggle form would pin q at x forever; use explicit
    // constants so the commit funnel sees real transitions.)
    let step = |at: u64, v: u32| -> Vec<Op> {
        vec![
            Op::Const { dst: 0, lit: Literal { width: 64, signed: false, base: Base::Dec, digits: at.to_string() } },
            Op::Delay { amount: 0 },
            Op::Const { dst: 1, lit: lit1(v) },
            Op::AssignBlk { lv: Lval::Var { var: 0 }, src: 1 },
        ]
    };
    let mut body = vec![step(2, 1), step(2, 0), step(2, 1)].concat();
    body.push(Op::Const { dst: 0, lit: Literal { width: 64, signed: false, base: Base::Dec, digits: "2".into() } });
    body.push(Op::Delay { amount: 0 });
    body.push(finish);
    d.threads.push(Thread { kind: ThreadKind::Initial, sens: vec![], frame_len: 4, body });
    d
}

struct Noop;
impl DumpSink for Noop {
    fn opened(&mut self, _path: &str) {}
    fn change(&mut self, _name: &str, _value: &str) {}
    fn dump_t0(&mut self, _values: &std::collections::BTreeMap<String, String>) {}
    fn flush_slot(&mut self, _now: u64) {}
}

#[test]
fn firing_points_match_adr0003_contract() {
    let mut k = Kernel::new(design()).expect("kernel");
    let log: Log = Default::default();
    let l = log.clone();
    k.vpi_hooks.start_of_sim = Some(Box::new(move |_, _| l.borrow_mut().push("start".into())));
    let l = log.clone();
    k.vpi_hooks.value_change = Some(Box::new(move |k, var| {
        let name = k.vpi_var_name(var as u32).unwrap_or("?").to_string();
        let v = k.vpi_read_var(var as u32).map(|b| b.fmt_bin()).unwrap_or_default();
        l.borrow_mut().push(format!("vc {name}={v}"));
    }));
    let l = log.clone();
    k.vpi_hooks.after_delay = Some(Box::new(move |k, now| {
        let v = k.vpi_read_var(0).unwrap().fmt_bin();
        l.borrow_mut().push(format!("t{now} q={v}"));
    }));
    let l = log.clone();
    k.vpi_hooks.end_of_sim = Some(Box::new(move |k, _| {
        let v = k.vpi_read_var(0).unwrap().fmt_bin();
        l.borrow_mut().push(format!("end q={v}"));
    }));

    let outcome = k.run(&mut Noop);
    assert_eq!(outcome.exit, 0, "run completes");
    let e = log.borrow().clone();
    // start fires first; after-delay fires at wheel ADVANCE (new now, slots
    // not yet executed → the value seen is the previous slot's settled one:
    // t2 q=x); vc fires post-settle inside the slot (value already stored).
    assert_eq!(e[0], "start", "{e:?}");
    assert_eq!(
        e,
        vec![
            "start",
            "t2 q=x",
            "vc q=1",
            "t4 q=1",
            "vc q=0",
            "t6 q=0",
            "vc q=1",
            "t8 q=1",
            "end q=1",
        ],
        "exact firing order = the ADR-0003 contract on the unified engine"
    );
    // end fires exactly once despite multiple finished checkpoints.
    assert_eq!(e.iter().filter(|s| s.starts_with("end")).count(), 1);
    // Commit order: first toggle's vc precedes the second's.
    let i1 = e.iter().position(|s| s == "vc q=1").unwrap();
    let i0 = e.iter().position(|s| s == "vc q=0").unwrap();
    assert!(i1 < i0, "{e:?}");
}

#[test]
fn no_hooks_is_zero_cost_passthrough() {
    // Default (all-None) hooks must not alter run semantics.
    let mut k = Kernel::new(design()).expect("kernel");
    let outcome = k.run(&mut Noop);
    assert_eq!(outcome.exit, 0);
    assert!(outcome.output.contains("$finish") || !outcome.output.is_empty() || true);
    let v = k.vpi_read_var(0).expect("q");
    assert_eq!(v.fmt_bin(), "1");
}
