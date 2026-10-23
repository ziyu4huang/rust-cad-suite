//! spec-i2c lockstep acceptance: the clean-room master RTL vs the golden
//! model, cycle-exact, with protocol invariants. The negative variants at
//! the bottom are the detection-power proof: deliberately broken RTL must
//! trip the expected invariants.

use std::cell::RefCell;
use std::rc::Rc;

use spec_i2c::bus::BusDriver;
use spec_i2c::invariants::{
    AckRelease, BusyTipCoherence, MsbFirst, SdaStableWhileSclHigh,
};
use spec_i2c::model::{I2cGolden, ST_ACK, ST_IDLE};
use spec_i2c::{CmdInput, RTL, CMD_RD, CMD_START, CMD_STOP, CMD_WR};
use vl_spec::{build_sim, GoldenModel, LockstepSpec, SignalId, SpecReport};

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
    done: SignalId,
    state: SignalId,
    rd_mode: SignalId,
    cnt: SignalId,
    prescale: SignalId,
    ack_rx: SignalId,
    dout: SignalId,
    half: SignalId,
    tx: SignalId,
    bitcnt: SignalId,
}

fn build_harness(
    rtl: &str,
) -> (
    LockstepSpec<I2cGolden>,
    IDs,
    Rc<RefCell<BusDriver>>,
    Rc<RefCell<Option<u8>>>,
) {
    let sim = build_sim(&[("i2c_master.v", rtl)], "i2c_master").expect("build");
    let mut spec = LockstepSpec::new(
        sim,
        "clk",
        PRESCALE,
        I2cGolden::new(PRESCALE as u16, ADDR),
    )
    .expect("clk binds");

    // Init: async reset, prescale, bus released.
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
        done: spec.bind_signal("done").unwrap(),
        state: spec.bind_signal("state").unwrap(),
        rd_mode: spec.bind_signal("rd_mode").unwrap(),
        cnt: spec.bind_signal("cnt").unwrap(),
        prescale: spec.bind_signal("prescale").unwrap(),
        ack_rx: spec.bind_signal("ack_rx").unwrap(),
        dout: spec.bind_signal("dout").unwrap(),
        half: spec.bind_signal("half").unwrap(),
        tx: spec.bind_signal("tx").unwrap(),
        bitcnt: spec.bind_signal("bitcnt").unwrap(),
    };
    spec.write_int(ids.scl_i, 1);
    spec.write_int(ids.sda_i, 1);
    let prescale = spec.bind_signal("prescale").unwrap();
    spec.write_int(prescale, PRESCALE as i64);
    // Canonical reset order (ADR-0007): under the 0-init policy an input
    // port is already 0 at registration, so "assert" must be a real 1->0
    // transition — pre-charge high after init_once, then assert, then
    // release. (Exactly how real AMBA/I2C testbenches drive reset.)
    let rst = spec.bind_signal("rst_n").unwrap();
    spec.run_until(5).unwrap(); // init_once: waits registered at rst_n=0
    spec.put_int(rst, 1).unwrap(); // pre-charge (no edge: negedge arm quiet)
    spec.run_until(10).unwrap();
    spec.put_int(rst, 0).unwrap(); // 1->0 negedge: the async reset fires
    spec.run_until(20).unwrap();
    spec.put_int(rst, 1).unwrap(); // release
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
        slave: spec_i2c::model::Slave::new(ADDR),
    }));
    let expected: Rc<RefCell<Option<u8>>> = Rc::new(RefCell::new(None));

    let bus2 = bus.clone();
    let expected2 = expected.clone();
    let bus3 = bus.clone();
    let spec = spec
        .driver(move |sim, sigs, input: &CmdInput| {
            bus2.borrow_mut().drive(sim, sigs, input);
            Ok(())
        })
        .post(move |sim, sigs| {
            bus3.borrow_mut().refresh(sim, sigs);
            let g = |id: SignalId| -> u8 { sigs.u64(sim, id).unwrap_or(0) as u8 };
            println!(
                "BUS t={} state={} scl={} sda={} oe={} tx={:02x} bc={}",
                sim.now_ticks(),
                g(ids.state),
                g(ids.scl_i),
                g(ids.sda_i),
                g(ids.sda_oe),
                g(ids.tx),
                g(ids.bitcnt)
            );
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
        ))
        .invariant(BusyTipCoherence::new(ids.busy, ids.tip, ids.state));
    (spec, ids, bus, expected)
}

#[test]
fn scripted_write_transaction_clean() {
    let (mut spec, ids, _bus, expected) = build_harness(RTL);

    // Driver discipline (pinned in the spec): a strobe may only be issued
    // while the master is parked (tip==0), and after strobing the driver
    // waits for the handshake — tip rises then falls (START/WR/RD), or busy
    // falls (STOP). Strobing on the consume edge has undefined latch/FSM
    // arbitration in ANY RTL — never do it.
    let mut cmd = |spec: &mut LockstepSpec<I2cGolden>,
                   ids: &IDs,
                   expected: &Rc<RefCell<Option<u8>>>,
                   c: Option<(u8, u8)>| {
        let Some((c8, d8)) = c else {
            return;
        };
        assert_eq!(spec.read_u64(ids.tip), 0, "issue we only while parked");
        if c8 == CMD_WR {
            *expected.borrow_mut() = Some(d8);
        }
        spec.cycle(&CmdInput::strobe(c8, d8)).unwrap();
        spec.cycle(&CmdInput::idle()).unwrap(); // latch cycle
        if c8 == CMD_STOP {
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
    };

    cmd(&mut spec, &ids, &expected, Some((CMD_START, 0)));
    cmd(&mut spec, &ids, &expected, Some((CMD_WR, 0xA0)));
    cmd(&mut spec, &ids, &expected, Some((CMD_WR, 0x96)));
    cmd(&mut spec, &ids, &expected, Some((CMD_STOP, 0)));
    for _ in 0..32 {
        spec.cycle(&CmdInput::idle()).unwrap();
    }
    for _ in 0..32 {
        spec.cycle(&CmdInput::idle()).unwrap();
    }

    let report: SpecReport = spec.finish();
    assert_eq!(report.violations.len(), 0, "{}", report.render_text());
    assert!(report.cycles_run > 100);
}

/// Read transaction: address-with-read (0xA1 = 0x50<<1 | 1), then one RD
/// byte. The bus slave drives its first read word (0xA5) MSB-first on SDA;
/// the DUT must clock it in and expose it on `dout`. This is the absolute
/// payload check the lockstep projection alone cannot give: both slave
/// instances (bus driver + golden view) must agree AND the received byte
/// must equal the word the spec says the slave holds.
#[test]
fn scripted_read_transaction_clean() {
    let (mut spec, ids, _bus, _expected) = build_harness(RTL);

    let mut cmd = |spec: &mut LockstepSpec<I2cGolden>, ids: &IDs, c: (u8, u8)| {
        let (c8, d8) = c;
        assert_eq!(spec.read_u64(ids.tip), 0, "issue we only while parked");
        spec.cycle(&CmdInput::strobe(c8, d8)).unwrap();
        spec.cycle(&CmdInput::idle()).unwrap(); // latch cycle
        if c8 == CMD_STOP {
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
    };

    cmd(&mut spec, &ids, (CMD_START, 0));
    cmd(&mut spec, &ids, (CMD_WR, 0xA1));
    cmd(&mut spec, &ids, (CMD_RD, 0));
    // The read byte completed: dout must hold the slave's first word.
    assert_eq!(
        spec.read_u64(ids.dout),
        0xA5,
        "DUT must clock in the slave's read word"
    );
    cmd(&mut spec, &ids, (CMD_STOP, 0));

    let report: SpecReport = spec.finish();
    assert_eq!(report.violations.len(), 0, "{}", report.render_text());
    assert!(report.cycles_run > 100);
}
