//! The scripted I2C spec-check scenario: the exact composition the demo's
//! spec card runs — all four invariants (SdaStableWhileSclHigh,
//! AckRelease, BusyTipCoherence, MsbFirst), the canonical write script
//! (START; WR 0xA0; WR 0x96; STOP; settle), and the three RTL variants
//! (correct / LSB-first / no-ack-release). This composition is the union
//! of what the two test files exercise and is pinned by tests/scenario.rs
//! BEFORE the wasm gate depends on it (no pre-existing test ran it
//! end-to-end; the no-ack variant's STOP completion was unproven).
//!
//! VCD contract: `force_dump()` precedes every put (it starts the writer);
//! no puts happen after the last `run_until` (the slot-end flush is the
//! last recorder); `take_vcd()` runs before `finish()`, which consumes the
//! spec. Break any of the three and the waveform silently truncates.

use std::cell::RefCell;
use std::rc::Rc;

use vl_spec::{build_sim, LockstepSpec, SignalId, SpecError, SpecReport};

use crate::bus::BusDriver;
use crate::invariants::{AckRelease, BusyTipCoherence, MsbFirst, SdaStableWhileSclHigh};
use crate::model::{I2cGolden, Obs, Slave};
use crate::{CmdInput, RTL, CMD_STOP, CMD_WR};

const PRESCALE: u64 = 4;
const ADDR: u8 = 0x50;

/// Which RTL the scenario runs: the clean-room master or a surgically
/// broken variant (the detection-power demonstration).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Correct,
    LsbFirst,
    NoAckRelease,
    /// Honors scl_i at the read sample (`rd_mode && scl_i`) — deviates from
    /// the documented simplification "scl_i unused". Behaviorally EQUIVALENT
    /// against a slave that never stretches (scl_i mirrors the master's own
    /// schedule); only a stretching slave exposes it, which is why the
    /// negative test pairs this variant with `Config::stretch`.
    HonorsSclI,
}

const LSB_ANCHOR: &str = "tx[7 - bitcnt[2:0]]";
const LSB_BROKEN: &str = "tx[bitcnt[2:0]]";
const NOACK_ANCHOR: &str = "end else begin\n              sda_oe <= 1'b0;\n              sda_o <= 1'b1;\n            end";
const NOACK_BROKEN: &str = "end else begin\n              sda_oe <= 1'b1;\n              sda_o <= 1'b0;\n            end";

/// The (possibly surgically broken) RTL source for `v`. Returns Err (not a
/// panic — this crosses the wasm boundary, where a trap kills the page) if
/// a surgery anchor misses; the native test asserts the anchors hit.
pub fn rtl_for(v: Variant) -> Result<String, SpecError> {
    const SCL_ANCHOR: &str = "if (rd_mode) begin\n                tx <= {tx[6:0], sda_i};\n              end";
    const SCL_BROKEN: &str = "if (rd_mode && scl_i) begin\n                tx <= {tx[6:0], sda_i};\n              end";
    let (anchor, broken) = match v {
        Variant::Correct => return Ok(RTL.to_string()),
        Variant::LsbFirst => (LSB_ANCHOR, LSB_BROKEN),
        Variant::NoAckRelease => (NOACK_ANCHOR, NOACK_BROKEN),
        Variant::HonorsSclI => (SCL_ANCHOR, SCL_BROKEN),
    };
    let out = RTL.replace(anchor, broken);
    if out == RTL {
        return Err(SpecError(format!(
            "rtl surgery anchor missed: {anchor:?}"
        )));
    }
    Ok(out)
}

/// One spec run's artifacts: the structured report, the run's VCD
/// (rendered to SVG by the demo; the wasm gate byte-compares two runs),
/// and one `dout` sample per completed read byte (absolute payload checks
/// for multi-address scenarios).
pub struct ScenarioResult {
    pub report: SpecReport,
    pub vcd: Option<String>,
    pub dout_samples: Vec<u8>,
}

/// Scenario configuration: the RTL variant, the slave set behind the bus
/// (address, per-address read word pattern), and whether the slave set
/// stretches SCL after every fall.
pub struct Config {
    pub variant: Variant,
    pub slaves: Vec<(u8, [u8; 4])>,
    pub stretch: bool,
    /// Read-leg addresses; defaults to each configured slave's address.
    /// Point it at an address OUTSIDE the table to exercise the
    /// unknown-address NACK path (released bus reads 0xff).
    pub read_addrs: Option<Vec<u8>>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            variant: Variant::Correct,
            slaves: vec![(ADDR, [0xA5, 0x3C, 0x81, 0x5A])],
            stretch: false,
            read_addrs: None,
        }
    }
}

struct Ids {
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
    ack_rx: SignalId,
    dout: SignalId,
    state: SignalId,
    rd_mode: SignalId,
    cnt: SignalId,
    half: SignalId,
}

/// Parking+handshake driver discipline as a Result (the test-local issue()
/// panics; this crosses the wasm boundary where a panic kills the page):
/// `we` strobes only while `tip==0`, then wait tip rise + fall (or busy
/// fall for STOP), 300 cycles each, else Err.
fn issue(
    spec: &mut LockstepSpec<I2cGolden>,
    ids: &Ids,
    c: (u8, u8),
) -> Result<(), SpecError> {
    if spec.read_u64(ids.tip) != 0 {
        return Err(SpecError("issue we only while parked".into()));
    }
    spec.cycle(&CmdInput::strobe(c.0, c.1))?;
    spec.cycle(&CmdInput::idle())?; // latch cycle
    if c.0 == CMD_STOP {
        for _ in 0..300 {
            if spec.read_u64(ids.busy) == 0 {
                return Ok(());
            }
            spec.cycle(&CmdInput::idle())?;
        }
        return Err(SpecError("STOP never completed".into()));
    }
    let mut saw_tip = false;
    for _ in 0..300 {
        if spec.read_u64(ids.tip) == 1 {
            saw_tip = true;
            break;
        }
        spec.cycle(&CmdInput::idle())?;
    }
    if !saw_tip {
        return Err(SpecError("transfer never started".into()));
    }
    for _ in 0..300 {
        if spec.read_u64(ids.tip) == 0 {
            return Ok(());
        }
        spec.cycle(&CmdInput::idle())?;
    }
    Err(SpecError("transfer never finished".into()))
}

/// Assemble the full harness for variant `v` (force_dump included — the
/// VCD must be capturing before the init puts).
fn build(
    cfg: &Config,
) -> Result<(LockstepSpec<I2cGolden>, Ids, Rc<RefCell<Option<u8>>>), SpecError> {
    let rtl = rtl_for(cfg.variant)?;
    let sim = build_sim(&[("i2c_master.v", rtl.as_str())], "i2c_master")?;
    let mut spec =
        LockstepSpec::new(sim, "clk", PRESCALE, I2cGolden::new(PRESCALE as u16, ADDR))?;
    let ids = Ids {
        scl_i: spec.bind_signal("scl_i")?,
        sda_i: spec.bind_signal("sda_i")?,
        we: spec.bind_signal("we")?,
        cmd: spec.bind_signal("cmd")?,
        din: spec.bind_signal("din")?,
        scl_o: spec.bind_signal("scl_o")?,
        sda_o: spec.bind_signal("sda_o")?,
        scl_oe: spec.bind_signal("scl_oe")?,
        sda_oe: spec.bind_signal("sda_oe")?,
        busy: spec.bind_signal("busy")?,
        tip: spec.bind_signal("tip")?,
        done: spec.bind_signal("done")?,
        ack_rx: spec.bind_signal("ack_rx")?,
        dout: spec.bind_signal("dout")?,
        state: spec.bind_signal("state")?,
        rd_mode: spec.bind_signal("rd_mode")?,
        cnt: spec.bind_signal("cnt")?,
        half: spec.bind_signal("half")?,
    };

    // VCD contract: start the writer before any put.
    spec.sim().force_dump();

    // Canonical reset under the 0-init policy: pre-charge high, 1->0, release.
    spec.write_int(ids.scl_i, 1);
    spec.write_int(ids.sda_i, 1);
    let prescale = spec.bind_signal("prescale")?;
    spec.write_int(prescale, PRESCALE as i64);
    let rst = spec.bind_signal("rst_n")?;
    spec.run_until(5)?;
    spec.put_int(rst, 1)?;
    spec.run_until(10)?;
    spec.put_int(rst, 0)?; // negedge: the async reset fires
    spec.run_until(20)?;
    spec.put_int(rst, 1)?; // release
    spec.run_until(30)?;

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
        slave: Slave::with_slaves(
            &cfg.slaves,
            if cfg.stretch { 12 } else { 0 },
            true,
        ),
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
            Ok(Obs {
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
        .invariant(BusyTipCoherence::new(ids.busy, ids.tip, ids.state))
        .invariant(MsbFirst::new(
            ids.sda_oe,
            ids.sda_o,
            ids.state,
            ids.rd_mode,
            ids.cnt,
            ids.half,
            expected_inv,
        ));
    Ok((spec, ids, expected))
}

/// Run the canonical write script (START; WR 0xA0; WR 0x96; STOP; settle)
/// end-to-end inside this process (wasm-safe: no fs/threads; error paths
/// return Err, never panic).
pub fn run(v: Variant) -> Result<ScenarioResult, SpecError> {
    run_cfg(&Config {
        variant: v,
        ..Config::default()
    })
}

/// Run `cfg` with the canonical write script.
pub fn run_cfg(cfg: &Config) -> Result<ScenarioResult, SpecError> {
    let (mut spec, ids, expected) = build(cfg)?;
    issue(&mut spec, &ids, (crate::CMD_START, 0))?;
    *expected.borrow_mut() = Some(0xA0);
    issue(&mut spec, &ids, (CMD_WR, 0xA0))?;
    *expected.borrow_mut() = Some(0x96);
    issue(&mut spec, &ids, (CMD_WR, 0x96))?;
    issue(&mut spec, &ids, (CMD_STOP, 0))?;
    for _ in 0..32 {
        spec.cycle(&CmdInput::idle())?;
    }
    // VCD contract: take before finish (finish consumes the spec).
    let vcd = spec.sim().take_vcd().map(|(_, text)| text);
    let report = spec.finish();
    Ok(ScenarioResult {
        report,
        vcd,
        dout_samples: Vec::new(),
    })
}

/// Run one read transaction per configured slave (START; WR addr|1 — the
/// address byte with R/W=1; RD — the slave drives its first word; STOP)
/// and return the `dout` sample of each read byte. Addressed slaves answer
/// with their own word; an unknown address is NACKed and the released bus
/// reads 0xff.
pub fn run_reads(cfg: &Config) -> Result<ScenarioResult, SpecError> {
    let (mut spec, ids, _expected) = build(cfg)?;
    let mut dout_samples = Vec::new();
    let read_addrs: Vec<u8> = match &cfg.read_addrs {
        Some(a) => a.clone(),
        None => cfg.slaves.iter().map(|(a, _)| *a).collect(),
    };
    for addr in &read_addrs {
        issue(&mut spec, &ids, (crate::CMD_START, 0))?;
        issue(&mut spec, &ids, (CMD_WR, (addr << 1) | 1))?;
        issue(&mut spec, &ids, (crate::CMD_RD, 0))?;
        dout_samples.push(spec.read_u64(ids.dout) as u8);
        issue(&mut spec, &ids, (CMD_STOP, 0))?;
        for _ in 0..16 {
            spec.cycle(&CmdInput::idle())?;
        }
    }
    // VCD contract: take before finish (finish consumes the spec).
    let vcd = spec.sim().take_vcd().map(|(_, text)| text);
    let report = spec.finish();
    Ok(ScenarioResult {
        report,
        vcd,
        dout_samples,
    })
}
