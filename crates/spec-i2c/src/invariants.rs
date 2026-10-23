//! Protocol invariants (spec layer 3) — per-cycle checks written from the
//! I2C-bus rules, watching the bus the RTL actually sees plus DUT-internal
//! state. These are independent of the golden model (one of the stacked
//! same-author defenses).

use std::cell::RefCell;
use std::rc::Rc;

use vl_spec::{Invariant, SignalId, SpecViolation, ViolationKind};

use crate::model::{ST_ACK, ST_IDLE, ST_SHIFT, ST_START, ST_STOP};

/// Master SDA-drive discipline: the DUT must not change its SDA drive
/// level while it is NOT actively driving SCL low (SCL high = released),
/// outside the START/STOP sequencer windows. Checked over the DUT-internal
/// drive registers (sda_o/sda_oe/scl_oe) — per-cycle bus views are sampled
/// once per SCL period and would quantize edge timing; the drive registers
/// are the master's actual intent. Slave-side changes are out of scope for
/// this invariant (the slave model is exercised by lockstep).
pub struct SdaStableWhileSclHigh {
    pub sda_o: SignalId,
    pub sda_oe: SignalId,
    pub scl_oe: SignalId,
    pub state: SignalId,
    prev: Option<(u8, u8)>, // (oe, level)
}
impl SdaStableWhileSclHigh {
    pub fn new(sda_o: SignalId, sda_oe: SignalId, scl_oe: SignalId, state: SignalId) -> Self {
        SdaStableWhileSclHigh {
            sda_o,
            sda_oe,
            scl_oe,
            state,
            prev: None,
        }
    }
}
impl Invariant for SdaStableWhileSclHigh {
    fn name(&self) -> &'static str {
        "sda_stable_while_scl_high"
    }
    fn check(&mut self, ctx: &vl_spec::CycleCtx, out: &mut Vec<SpecViolation>) {
        let sda_o = ctx.sigs.u64(ctx.sim, self.sda_o).unwrap_or(1) as u8;
        let sda_oe = ctx.sigs.u64(ctx.sim, self.sda_oe).unwrap_or(0) as u8;
        let scl_oe = ctx.sigs.u64(ctx.sim, self.scl_oe).unwrap_or(0) as u8;
        let state = ctx.sigs.u64(ctx.sim, self.state).unwrap_or(ST_IDLE as u64) as u8;
        let level = if sda_oe != 0 { sda_o } else { 1 };
        let scl_high = scl_oe == 0; // released = pulled high
        let in_seq = state == ST_START || state == ST_STOP;
        if let Some((poe, plevel)) = self.prev {
            let prev_level = if poe != 0 { plevel } else { 1 };
            if scl_high && !in_seq && prev_level != level {
                out.push(SpecViolation {
                    time: ctx.time,
                    cycle: ctx.cycle,
                    kind: ViolationKind::Invariant(self.name()),
                    expected: format!("master SDA drive stable at {prev_level}"),
                    actual: format!("master SDA drive changed to {level} while SCL high"),
                    message: "SDA must not change while SCL is high outside START/STOP".into(),
                });
            }
        }
        self.prev = Some((sda_oe, sda_o));
    }
}

/// Ack discipline: in the write-ack slot the master must RELEASE SDA — a
/// master that holds the line low can never see a slave nak. Reads the
/// DUT-internal `state`/`rd_mode`/`sda_oe`.
pub struct AckRelease {
    pub state: SignalId,
    pub rd_mode: SignalId,
    pub sda_oe: SignalId,
    pub half: SignalId,
}
impl AckRelease {
    pub fn new(
        state: SignalId,
        rd_mode: SignalId,
        sda_oe: SignalId,
        half: SignalId,
    ) -> Self {
        AckRelease {
            state,
            rd_mode,
            sda_oe,
            half,
        }
    }
}
impl Invariant for AckRelease {
    fn name(&self) -> &'static str {
        "ack_release"
    }
    fn check(&mut self, ctx: &vl_spec::CycleCtx, out: &mut Vec<SpecViolation>) {
        let state = ctx.sigs.u64(ctx.sim, self.state).unwrap_or(ST_IDLE as u64) as u8;
        if state != ST_ACK {
            return;
        }
        let rd_mode = ctx.sigs.u64(ctx.sim, self.rd_mode).unwrap_or(0) as u8;
        let sda_oe = ctx.sigs.u64(ctx.sim, self.sda_oe).unwrap_or(0) as u8;
        // Check during the SCL-high half of the 9th clock: by then the
        // master must have released SDA so the slave can drive its ack (the
        // first slot clk still carries the pre-release drive — NBA timing).
        let half = ctx.sigs.u64(ctx.sim, self.half).unwrap_or(0) as u8;
        if half != 1 {
            return;
        }
        if rd_mode == 0 && sda_oe != 0 {
            out.push(SpecViolation {
                time: ctx.time,
                cycle: ctx.cycle,
                kind: ViolationKind::Invariant(self.name()),
                expected: "sda_oe=0 (released) during write-ack".into(),
                actual: format!("sda_oe={sda_oe}"),
                message: "master must release SDA in the write-ack slot".into(),
            });
        }
    }
}

/// busy/tip coherence: tip implies busy; busy=0 only in IDLE.
pub struct BusyTipCoherence {
    pub busy: SignalId,
    pub tip: SignalId,
    pub state: SignalId,
}
impl BusyTipCoherence {
    pub fn new(busy: SignalId, tip: SignalId, state: SignalId) -> Self {
        BusyTipCoherence {
            busy,
            tip,
            state,
        }
    }
}
impl Invariant for BusyTipCoherence {
    fn name(&self) -> &'static str {
        "busy_tip_coherence"
    }
    fn check(&mut self, ctx: &vl_spec::CycleCtx, out: &mut Vec<SpecViolation>) {
        let busy = ctx.sigs.u64(ctx.sim, self.busy).unwrap_or(0) as u8;
        let tip = ctx.sigs.u64(ctx.sim, self.tip).unwrap_or(0) as u8;
        let state = ctx.sigs.u64(ctx.sim, self.state).unwrap_or(ST_IDLE as u64) as u8;
        if tip == 1 && busy != 1 {
            out.push(SpecViolation {
                time: ctx.time,
                cycle: ctx.cycle,
                kind: ViolationKind::Invariant(self.name()),
                expected: "busy=1 whenever tip=1".into(),
                actual: "tip=1 with busy=0".into(),
                message: "tip must imply busy".into(),
            });
        }
        if busy == 0 && state != ST_IDLE {
            out.push(SpecViolation {
                time: ctx.time,
                cycle: ctx.cycle,
                kind: ViolationKind::Invariant(self.name()),
                expected: format!("state=IDLE ({ST_IDLE}) when busy=0"),
                actual: format!("state={state}"),
                message: "busy=0 only in IDLE".into(),
            });
        }
    }
}

/// MSB-first bit order for write bytes: capture the SDA level at each SCL
/// falling edge during SHIFT and compare the assembled byte against the
/// byte the driver issued. The expected byte is shared via an
/// `Rc<RefCell>` slot the test updates when it issues a command.
/// OPEN ITEM (1023): the bit sampler collects 0x92 instead of 0x96 for a
/// 0x96 write byte — one position reads the next-bit level. Sampling-phase
/// question under investigation; see the #[ignore] reproduction test.
#[allow(dead_code)]
pub struct MsbFirst {
    pub sda_i: SignalId,
    pub state: SignalId,
    pub rd_mode: SignalId,
    pub cnt: SignalId,
    pub half: SignalId,
    pub prescale: SignalId,
    pub expected: Rc<RefCell<Option<u8>>>,
    bits: Vec<u8>,
}
#[allow(dead_code)]
impl MsbFirst {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        sda_i: SignalId,
        state: SignalId,
        rd_mode: SignalId,
        cnt: SignalId,
        half: SignalId,
        prescale: SignalId,
        expected: Rc<RefCell<Option<u8>>>,
    ) -> Self {
        MsbFirst {
            sda_i,
            state,
            rd_mode,
            cnt,
            half,
            prescale,
            expected,
            bits: Vec::new(),
        }
    }
}
#[allow(dead_code)]
impl Invariant for MsbFirst {
    fn name(&self) -> &'static str {
        "msb_first_bit_order"
    }
    fn check(&mut self, ctx: &vl_spec::CycleCtx, out: &mut Vec<SpecViolation>) {
        let sda = ctx.sigs.u64(ctx.sim, self.sda_i).unwrap_or(1) as u8;
        let state = ctx.sigs.u64(ctx.sim, self.state).unwrap_or(ST_IDLE as u64) as u8;
        let rd_mode = ctx.sigs.u64(ctx.sim, self.rd_mode).unwrap_or(0) as u8;
        let cnt = ctx.sigs.u64(ctx.sim, self.cnt).unwrap_or(0);
        let half = ctx.sigs.u64(ctx.sim, self.half).unwrap_or(0) as u8;
        // Sampling phase has one known-wrong position — open item, see the
        // #[ignore] test in spec-i2c.
        if state == ST_SHIFT && rd_mode == 0 && half == 0 && cnt == 1 {
            // First clk after the low-half entry edge: the new bit has just
            // been driven (post-edge view) and is stable for the half.
            self.bits.push(sda);
        }
        if state == ST_ACK && self.bits.len() == 8 {
            let byte = self.bits.iter().fold(0u8, |a, &b| (a << 1) | b);
            let exp = *self.expected.borrow();
            if let Some(exp) = exp {
                if byte != exp {
                    out.push(SpecViolation {
                        time: ctx.time,
                        cycle: ctx.cycle,
                        kind: ViolationKind::Invariant(self.name()),
                        expected: format!("byte 0x{exp:02x} MSB-first"),
                        actual: format!("byte 0x{byte:02x}"),
                        message: "write bytes must go out MSB-first".into(),
                    });
                }
            }
            self.bits.clear();
        }
        if state != ST_SHIFT && state != ST_ACK && !self.bits.is_empty() {
            self.bits.clear();
        }
    }
}
