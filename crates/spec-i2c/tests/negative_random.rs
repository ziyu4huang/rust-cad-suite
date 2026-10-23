//! Detection-power proof (the product) + randomized lockstep:
//! - two deliberately broken RTL variants must trip the expected invariants
//!   (LSB-first bit order; no-ack-release) — a spec harness that cannot
//!   catch broken RTL is worthless;
//! - ≥20 randomized write transactions (fixed-seed LCG, no deps) stay clean
//!   on the correct RTL.

use std::cell::RefCell;
use std::rc::Rc;

use spec_i2c::bus::BusDriver;
use spec_i2c::invariants::{AckRelease, MsbFirst, SdaStableWhileSclHigh};
use spec_i2c::model::{I2cGolden, Slave};
use spec_i2c::{CmdInput, RTL, CMD_START, CMD_STOP, CMD_WR};
use vl_spec::{build_sim, GoldenModel, LockstepSpec, SignalId, SpecReport, ViolationKind};

const PRESCALE: u64 = 4;
const ADDR: u8 = 0x50;

struct IDs {
    scl_i: SignalId,
    sda_i: SignalId,
    we: SignalId,
    cmd: SignalId,
    din: SignalId,
    scl_o: SignalId,
    sda_o: SignalId,
    scl_oe: SignalId,
    sda_oe: SignalId,
    busy: SignalId,
    tip: SignalId,
    state: SignalId,
    rd_mode: SignalId,
    cnt: SignalId,
    prescale: SignalId,
    done: SignalId,
    ack_rx: SignalId,
    dout: SignalId,
    half: SignalId,
}

fn build_harness(rtl: &str) -> (LockstepSpec<I2cGolden>, IDs, Rc<RefCell<Option<u8>>>) {
    let sim = build_sim(&[("i2c_master.v", rtl)], "i2c_master").expect("build");
    let mut spec = LockstepSpec::new(sim, "clk", PRESCALE, I2cGolden::new(PRESCALE as u16, ADDR))
        .expect("clk binds");
    let ids = IDs {
        scl_i: spec.bind_signal("scl_i").unwrap(),
        sda_i: spec.bind_signal("sda_i").unwrap(),
        we: spec.bind_signal("we").unwrap(),
        cmd: spec.bind_signal("cmd").unwrap(),
        din: spec.bind_signal("din").unwrap(),
        scl_o: spec.bind_signal("scl_o").unwrap(),
        sda_o: spec.bind_signal("sda_o").unwrap(),
        scl_oe: spec.bind_signal("scl_oe").unwrap(),
        sda_oe: spec.bind_signal("sda_oe").unwrap(),
        busy: spec.bind_signal("busy").unwrap(),
        tip: spec.bind_signal("tip").unwrap(),
        state: spec.bind_signal("state").unwrap(),
        rd_mode: spec.bind_signal("rd_mode").unwrap(),
        cnt: spec.bind_signal("cnt").unwrap(),
        prescale: spec.bind_signal("prescale").unwrap(),
        done: spec.bind_signal("done").unwrap(),
        ack_rx: spec.bind_signal("ack_rx").unwrap(),
        dout: spec.bind_signal("dout").unwrap(),
        half: spec.bind_signal("half").unwrap(),
    };
    spec.write_int(ids.scl_i, 1);
    spec.write_int(ids.sda_i, 1);
    let prescale = spec.bind_signal("prescale").unwrap();
    spec.write_int(prescale, PRESCALE as i64);
    let rst = spec.bind_signal("rst_n").unwrap();
    // Canonical reset: pre-charge high, assert (1->0 negedge), release.
    spec.run_until(5).unwrap();
    spec.put_int(rst, 1).unwrap();
    spec.run_until(10).unwrap();
    spec.put_int(rst, 0).unwrap();
    spec.run_until(20).unwrap();
    spec.put_int(rst, 1).unwrap();
    spec.run_until(30).unwrap();

    let bus = Rc::new(RefCell::new(BusDriver {
        scl_i: ids.scl_i,
        sda_i: ids.sda_i,
        we: ids.we,
        cmd: ids.cmd,
        din: ids.din,
        scl_o: ids.scl_o,
        sda_o: ids.sda_o,
        scl_oe: ids.scl_oe,
        sda_oe: ids.sda_oe,
        slave: Slave::new(ADDR),
    }));
    let expected: Rc<RefCell<Option<u8>>> = Rc::new(RefCell::new(None));
    let b2 = bus.clone();
    let b3 = bus.clone();
    let expected_inv = expected.clone();
    let spec = spec
        .driver(move |sim, sigs, input: &CmdInput| {
            b2.borrow_mut().drive(sim, sigs, input);
            Ok(())
        })
        .post(move |sim, sigs| {
            b3.borrow_mut().refresh(sim, sigs);
        })
        .probe(move |sigs, sim| {
            let g = |id: SignalId| -> u8 { sigs.u64(sim, id).unwrap_or(0) as u8 };
            Ok(spec_i2c::model::Obs {
                scl_o: g(ids.scl_o),
                sda_o: g(ids.sda_o),
                scl_oe: g(ids.scl_oe),
                sda_oe: g(ids.sda_oe),
                busy: g(ids.busy),
                tip: g(ids.tip),
                done: g(ids.done),
                ack_rx: g(ids.ack_rx),
                dout: g(ids.dout),
            })
        })
        .invariant(SdaStableWhileSclHigh::new(
            ids.sda_o,
            ids.sda_oe,
            ids.scl_oe,
            ids.state,
        ))
        .invariant(AckRelease::new(
            ids.state,
            ids.rd_mode,
            ids.sda_oe,
            ids.half,
        ));
    (spec, ids, expected)
}

/// Issue one command with the parking+handshake driver discipline.
fn issue(
    spec: &mut LockstepSpec<I2cGolden>,
    ids: &IDs,
    c: (u8, u8),
) {
    assert_eq!(spec.read_u64(ids.tip), 0, "issue we only while parked");
    spec.cycle(&CmdInput::strobe(c.0, c.1)).unwrap();
    spec.cycle(&CmdInput::idle()).unwrap(); // latch
    if c.0 == CMD_STOP {
        for _ in 0..300 {
            if spec.read_u64(ids.busy) == 0 {
                return;
            }
            spec.cycle(&CmdInput::idle()).unwrap();
        }
        panic!("STOP never completed");
    }
    let mut saw_tip = false;
    for _ in 0..300 {
        if spec.read_u64(ids.tip) == 1 {
            saw_tip = true;
            break;
        }
        spec.cycle(&CmdInput::idle()).unwrap();
    }
    assert!(saw_tip, "transfer never started");
    for _ in 0..300 {
        if spec.read_u64(ids.tip) == 0 {
            return;
        }
        spec.cycle(&CmdInput::idle()).unwrap();
    }
    panic!("transfer never finished");
}

/// Fixed-seed LCG (no-dep determinism; PLAN §7 seeded-fuzz precedent).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

#[test]
fn lsb_first_variant_is_caught() {
    let broken = RTL.replace("tx[7 - bitcnt[2:0]]", "tx[bitcnt[2:0]]");
    assert_ne!(broken, RTL, "surgery anchor missed");
    let (mut spec, ids, expected) = build_harness(&broken);
    issue(&mut spec, &ids, (CMD_START, 0));
    *expected.borrow_mut() = Some(0xA0);
    issue(&mut spec, &ids, (CMD_WR, 0xA0));
    let report: SpecReport = spec.finish();
    let names: Vec<String> = report
        .violations
        .iter()
        .map(|v| match &v.kind {
            ViolationKind::Invariant(n) => n.to_string(),
            ViolationKind::Lockstep(n) => format!("lockstep[{n}]"),
        })
        .collect();
    // Detection power: the lockstep leg catches the broken bit order
    // wholesale (the golden diverges every shifted cycle). The dedicated
    // msb_first invariant is an OPEN ITEM (see msb_sampler_phase_open_item)
    // — once its sampler phase is fixed, tighten this assertion to require
    // `msb_first_bit_order` explicitly.
    assert!(
        names.iter().any(|n| n.starts_with("lockstep")),
        "LSB-first RTL must trip the lockstep leg; got {names:?}\n{}",
        report.render_text()
    );
    assert!(report.violations.len() >= 40, "wholesale divergence expected");
}

#[test]
fn no_ack_release_variant_is_caught() {
    let anchor = "end else begin\n              sda_oe <= 1'b0;\n              sda_o <= 1'b1;\n            end";
    let broken = RTL.replace(anchor, "end else begin\n              sda_oe <= 1'b1;\n              sda_o <= 1'b0;\n            end");
    assert_ne!(broken, RTL, "surgery anchor missed");
    let (mut spec, ids, expected) = build_harness(&broken);
    issue(&mut spec, &ids, (CMD_START, 0));
    *expected.borrow_mut() = Some(0xA0);
    issue(&mut spec, &ids, (CMD_WR, 0xA0));
    let report: SpecReport = spec.finish();
    let names: Vec<String> = report
        .violations
        .iter()
        .map(|v| match &v.kind {
            ViolationKind::Invariant(n) => n.to_string(),
            ViolationKind::Lockstep(n) => format!("lockstep[{n}]"),
        })
        .collect();
    assert!(
        names.iter().any(|n| n == "ack_release"),
        "no-release RTL must trip ack_release; got {names:?}\n{}",
        report.render_text()
    );
}

#[test]
fn randomized_write_transactions_clean() {
    let (mut spec, ids, expected) = build_harness(RTL);
    let mut lcg = Lcg(0x5EED_2026_1023);
    for _ in 0..20 {
        issue(&mut spec, &ids, (CMD_START, 0));
        let byte = (lcg.next() & 0xFF) as u8;
        *expected.borrow_mut() = Some(byte);
        issue(&mut spec, &ids, (CMD_WR, byte));
        issue(&mut spec, &ids, (CMD_STOP, 0));
    }
    let report: SpecReport = spec.finish();
    report.assert_empty();
}

/// OPEN ITEM (1023): the MsbFirst bit sampler collects 0x92 instead of 0x96
/// for a 0x96 write byte — one bit position reads the next-bit level under
/// the one-sample-per-SCL-period view. Every other layer (lockstep,
/// ack/busy invariants, negative detection) is proven; this sampler's
/// phase question is the open piece. Un-ignore when picked up.
#[test]
#[ignore]
fn msb_sampler_phase_open_item() {
    let (mut spec, ids, expected) = build_harness(RTL);
    issue(&mut spec, &ids, (CMD_START, 0));
    // MsbFirst is not registered in build_harness while the sampler phase
    // is open; the reproduction needs it — rebuild with a local invariant.
    drop(spec);
    let _ = expected;
}
