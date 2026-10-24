//! Golden model (spec layer 2): a per-clk master FSM written from the
//! protocol description, plus the open-drain bus and an ideal slave, so its
//! projections are computed from its own view of the bus. Compared
//! field-wise against the RTL every clk cycle by the lockstep harness.
//!
//! Same-author risk is mitigated by the stacked defenses: protocol
//! invariants watch the bus fed to the RTL independently, the corpus
//! oracle leg runs the same RTL under verilator, and the negative RTL
//! variants prove the harness detects violations.

use crate::CmdInput;
use vl_spec::GoldenModel;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Obs {
    pub scl_o: u8,
    pub sda_o: u8,
    pub scl_oe: u8,
    pub sda_oe: u8,
    pub busy: u8,
    pub tip: u8,
    pub done: u8,
    pub ack_rx: u8,
    pub dout: u8,
}

pub const ST_IDLE: u8 = 0;
pub const ST_START: u8 = 1;
pub const ST_SHIFT: u8 = 2;
pub const ST_ACK: u8 = 3;
pub const ST_HOLD: u8 = 4;
pub const ST_STOP: u8 = 5;

/// Ideal slave SET behind one bus position: acks a matched address
/// (byte[7:1] in the table), NACKs unknown addresses, and emits read data
/// from the addressed slave's word pattern. One instance models the whole
/// set honestly for this protocol subset (non-addressed slaves never
/// drive, so the wire-AND of the set equals the addressed slave). Optional
/// clock stretching: after each SCL fall the slave holds SCL low for
/// `stretch` calls (a slow slave; the master per this spec ignores scl_i,
/// so data levels survive the stretch and only the schedule shifts).
pub struct Slave {
    pub addr: u8,
    addrs: Vec<u8>,
    words: Vec<[u8; 4]>,
    active: usize,
    read_idx: usize,
    stretch_cfg: u32,
    scl_stretch_left: u32,
    /// This RTL's documented START phantom: the first byte after START
    /// shifts whatever `tx` holds. A slave modeling the documented spec
    /// skips that byte entirely (it is neither an address nor data); the
    /// phantom's ack slot stays released (NACK) because nothing decodes.
    skip_phantom: bool,
    skip_bits: u32,
    /// The addressed slave answered the current address byte: gates the
    /// read-payload drive (an unaddressed slave never drives — the bus
    /// reads released).
    addressed: bool,
    prev_scl: u8,
    prev_sda: u8,
    started: bool,
    bits: Vec<u8>,
    read_mode: bool,
    pub drives_sda_low: bool,
    /// Ack-slot bookkeeping: `in_ack` = the 8 data bits are decoded and the
    /// 9th (ack) clock is in flight; `ack_pull` = this byte's ack pulls low
    /// (write bytes; read payloads stay released for the master's ack);
    /// `ack_hi` = the ack slot's high half is in flight (the pull window —
    /// gating on it keeps the pull off the bit-7 data level).
    in_ack: bool,
    ack_pull: bool,
    ack_hi: bool,
    /// The drive level applied on the previous call: lets the STOP detector
    /// tell "the slave released its pull" from "the master released the bus"
    /// (a real STOP).
    prev_pull: bool,
}

impl Slave {
    pub fn new(addr: u8) -> Slave {
        Slave::with_slaves(&[(addr, [0xA5, 0x3C, 0x81, 0x5A])], 0, false)
    }

    /// A slave set: `(address, read word pattern)` pairs, optionally
    /// stretching SCL after every fall (`stretch_cycles` calls per fall;
    /// 0 = never stretches), optionally skipping the START phantom byte
    /// (true = model the documented spec where the address byte is the
    /// second byte after START).
    pub fn with_slaves(
        table: &[(u8, [u8; 4])],
        stretch_cycles: u32,
        skip_phantom: bool,
    ) -> Slave {
        Slave {
            addr: table[0].0,
            addrs: table.iter().map(|(a, _)| *a).collect(),
            words: table.iter().map(|(_, w)| *w).collect(),
            active: 0,
            read_idx: 0,
            stretch_cfg: stretch_cycles,
            scl_stretch_left: 0,
            skip_phantom,
            skip_bits: 0,
            addressed: false,
            prev_scl: 1,
            prev_sda: 1,
            started: false,
            bits: Vec::new(),
            read_mode: false,
            drives_sda_low: false,
            in_ack: false,
            ack_pull: false,
            ack_hi: false,
            prev_pull: false,
        }
    }
}

/// The open-drain wire-AND + slave, shared by the golden model's bus view.
/// Given the master's drive outputs, returns the bus levels.
pub fn bus_levels(m: &MasterView, slave: &mut Slave) -> (u8, u8) {
    let mut scl_i = if m.scl_oe != 0 && m.scl_o == 0 { 0 } else { 1 };
    let mut sda_i = if m.sda_oe != 0 && m.sda_o == 0 { 0 } else { 1 };
    // Clock stretching: while the stretch counter runs, the slave holds SCL
    // low no matter what the master does with its own scl_o. The master per
    // this spec ignores scl_i, so its schedule is unaffected and data
    // levels (stable through both halves) survive; only the bus schedule
    // shifts. A variant RTL that honors scl_i diverges exactly here.
    if slave.scl_stretch_left > 0 {
        scl_i = 0;
        slave.scl_stretch_left -= 1;
    }
    // Slave reacts to SDA/SCL on the falling edge of SCL (change) and the
    // rising edge (sample); START/STOP detected on SDA falls/rises while
    // SCL is high.
    if slave.prev_scl == 1 && scl_i == 1 && slave.prev_sda == 1 && sda_i == 0 && !slave.drives_sda_low {
        slave.started = true;
        slave.bits.clear();
        slave.drives_sda_low = false;
        slave.in_ack = false;
        slave.ack_pull = false;
        slave.ack_hi = false;
        slave.read_mode = false;
        slave.addressed = false;
        if slave.skip_phantom {
            slave.skip_bits = 9; // the phantom's 8 bits + its ack level
        }
    }
    if slave.prev_scl == 1 && scl_i == 1 && slave.prev_sda == 0 && sda_i == 1 && !slave.prev_pull {
        slave.started = false;
        slave.read_mode = false;
        slave.drives_sda_low = false;
        slave.in_ack = false;
        slave.ack_pull = false;
        slave.ack_hi = false;
        slave.addressed = false;
        slave.bits.clear();
    }
    // Rising edge: a slave holding ACK keeps it through the whole 9th
    // clock (the master samples at the end of the high half) and releases
    // at the falling edge instead.
    // Level-derived drive FIRST, from the state as of the previous edge,
    // and applied BEFORE the boundary detectors run: the slave's own ack
    // pull must be visible in the SDA level the START/STOP detectors
    // examine, or the pull's own release (prev_sda=0 pulled, fresh wire-AND
    // =1) reads as a false STOP while SCL is high and kills `started`.
    // Level-derived drive, recomputed every call: idempotent, and immune
    // to missed edges where SCL parks low through HOLD straight into the
    // next byte's first low half (an edge-scheduled drive missed that
    // first bit entirely). Ack high: pull for write / release for read.
    // Read payload: drive the current word's bit for the byte position.
    slave.drives_sda_low = if slave.ack_hi && slave.ack_pull {
        true
    } else if !slave.in_ack
        && !slave.ack_hi
        && slave.started
        && slave.addressed
        && slave.read_mode
        && slave.bits.len() <= 8
        && (scl_i == 0 && slave.bits.len() < 8
            || (scl_i == 1 && slave.bits.len() >= 1))
    {
        // Bit k's window (low half + high half) must carry word bit 7-k.
        // The buffer is pushed at each SCL rise, so len is k in the low
        // half and k+1 in the high half — the phase-based bitpos keeps the
        // level continuous across the SCL edge the master samples at.
        let len = slave.bits.len();
        let bitpos = if scl_i == 1 { 8 - len } else { 7 - len };
        let word = slave.words[slave.active][slave.read_idx % slave.words[slave.active].len()];
        (word >> bitpos) & 1 == 0
    } else {
        false
    };
    if slave.drives_sda_low {
        sda_i = 0;
    }
    // Rising edge: sample the data bit. Levels are stable through the whole
    // high half, so the one-clk-stale driver view still reads the right bit.
    // The ack-slot level is never data: while `in_ack` the rise SWALLOWS it.
    if slave.started && slave.prev_scl == 0 && scl_i == 1 {
        if slave.in_ack {
            slave.in_ack = false;
            slave.ack_hi = true;
        } else if slave.skip_bits > 0 {
            // The START phantom's bits: not an address, not data — the
            // slave neither decodes nor acks them (released ack slot).
            slave.skip_bits -= 1;
        } else {
            slave.bits.push(sda_i);
            if slave.bits.len() == 8 {
                let byte = slave
                    .bits
                    .iter()
                    .fold(0u8, |a, &b| (a << 1) | b);
                let completed_payload = slave.read_mode; // mode this byte ran in
                slave.read_mode = byte & 1 == 1;
                let addr = byte >> 1;
                let matched = slave.addrs.contains(&addr);
                if matched {
                    slave.addressed = true;
                    slave.active = slave.addrs.iter().position(|a| *a == addr).unwrap();
                    slave.read_idx = 0; // each read transaction restarts the word
                }
                // Addressed write bytes: the slave pulls the ack low.
                // Unknown addresses: NACK (released — nothing at that
                // address pulls). Read payloads: the master acks; the
                // slave stays released.
                slave.in_ack = true;
                slave.ack_pull = matched || !slave.read_mode;
                slave.ack_hi = false;
                if slave.read_mode && completed_payload {
                    // Payload read byte serviced: the next payload serves
                    // the next word. The address byte advances nothing.
                    slave.read_idx = (slave.read_idx + 1)
                        % slave.words[slave.active].len();
                }
                slave.bits.clear();
            }
        }
    }
    // Ack-slot end: the fall after the ack slot's high half closes the
    // pull window. A +-1 clk skew on this edge touches no sample point.
    if slave.started && slave.prev_scl == 1 && scl_i == 0 && slave.ack_hi {
        slave.ack_hi = false;
        slave.ack_pull = false;
    }
    // Clock stretching: arm at every real SCL fall (bit boundary). The
    // stretch pins scl low through the following window; data levels are
    // identical in both halves, so the master's (schedule-bound) samples
    // still read the right bit. The stretch-induced rise at release is the
    // bit's rise, delayed — the slave's sample bookkeeping stays 1:1.
    if slave.started
        && slave.prev_scl == 1
        && scl_i == 0
        && slave.scl_stretch_left == 0
        && slave.stretch_cfg > 0
    {
        slave.scl_stretch_left = slave.stretch_cfg;
    }
    slave.prev_pull = slave.drives_sda_low;
    slave.prev_scl = scl_i;
    slave.prev_sda = sda_i;
    (scl_i, sda_i)
}

/// Master view passed into the bus: the golden's own outputs (or the RTL's
/// when the driver uses this for the RTL side).
pub struct MasterView {
    pub scl_o: u8,
    pub sda_o: u8,
    pub scl_oe: u8,
    pub sda_oe: u8,
}

/// Golden master FSM (per-clk, mirrors the spec semantics of the RTL
/// header: prescaled halves, START/STOP sequencers, MSB-first shift, ack
/// slot, hold-between-bytes).
pub struct I2cGolden {
    pub state: u8,
    pub cnt: u16,
    pub half: bool,
    pub bitcnt: u8,
    pub step: u8,
    pub tx: u8,
    pub rd_mode: bool,
    pub pend_start: bool,
    pub pend_stop: bool,
    pub pend_wr: bool,
    pub pend_rd: bool,
    pub scl_o: u8,
    pub sda_o: u8,
    pub scl_oe: u8,
    pub sda_oe: u8,
    pub busy: u8,
    pub tip: u8,
    pub done: u8,
    pub ack_rx: u8,
    pub dout: u8,
    pub prescale: u16,
    // Bus view: adopted from the cycle's CmdInput (the ONE testbench bus —
    // see the CmdInput docs; an own-slave view diverges from the RTL's bus
    // on 1-clk edge races).
    pub scl_i: u8,
    pub sda_i: u8,
}

impl I2cGolden {
    pub fn new(prescale: u16, _slave_addr: u8) -> I2cGolden {
        I2cGolden {
            state: ST_IDLE,
            cnt: 0,
            half: false,
            bitcnt: 0,
            step: 0,
            tx: 0,
            rd_mode: false,
            pend_start: false,
            pend_stop: false,
            pend_wr: false,
            pend_rd: false,
            scl_o: 1,
            sda_o: 1,
            scl_oe: 0,
            sda_oe: 0,
            busy: 0,
            tip: 0,
            done: 0,
            ack_rx: 1,
            dout: 0,
            prescale,
            scl_i: 1,
            sda_i: 1,
        }
    }

    fn cnt_end(&self) -> bool {
        self.cnt >= self.prescale - 1
    }
}

impl GoldenModel for I2cGolden {
    type Input = CmdInput;
    type Obs = Obs;

    fn step(&mut self, input: &CmdInput) {
        self.done = 0;
        // NBA discipline: the RTL latches `we` strobes in nonblocking
        // assignments, visible to the FSM on the NEXT cycle — so the golden
        // latches at the END of the step (after the FSM case), not before.
        match self.state {
            ST_IDLE => {
                self.scl_oe = 0;
                self.sda_oe = 0;
                self.scl_o = 1;
                self.sda_o = 1;
                self.busy = 0;
                self.tip = 0;
                self.cnt = 0;
                self.half = false;
                self.bitcnt = 0;
                self.step = 0;
                if self.pend_start {
                    self.pend_start = false;
                    self.busy = 1;
                    self.rd_mode = false;
                    self.step = 0;
                    self.cnt = 0;
                    self.state = ST_START;
                }
            }
            ST_START => {
                if self.step == 0 {
                    self.scl_oe = 0;
                    self.sda_oe = 0;
                    self.sda_o = 1;
                    if self.cnt_end() {
                        self.step = 1;
                        self.cnt = 0;
                    } else {
                        self.cnt += 1;
                    }
                } else if self.step == 1 {
                    self.sda_o = 0;
                    self.sda_oe = 1;
                    if self.cnt_end() {
                        self.step = 2;
                        self.cnt = 0;
                    } else {
                        self.cnt += 1;
                    }
                } else {
                    self.scl_oe = 1;
                    self.scl_o = 0;
                    self.sda_oe = 1;
                    self.sda_o = 0;
                    if self.cnt_end() {
                        self.step = 0;
                        self.cnt = 0;
                        self.half = false;
                        self.bitcnt = 0;
                        self.tip = 1;
                        self.state = ST_SHIFT;
                    } else {
                        self.cnt += 1;
                    }
                }
            }
            ST_SHIFT => {
                if !self.half {
                    self.scl_oe = 1;
                    self.scl_o = 0;
                    if self.rd_mode {
                        self.sda_oe = 0;
                        self.sda_o = 1;
                    } else {
                        self.sda_oe = u8::from((self.tx >> (7 - (self.bitcnt & 7))) & 1 == 0);
                        self.sda_o = 0;
                    }
                    if self.cnt_end() {
                        self.half = true;
                        self.scl_oe = 0;
                        self.cnt = 0;
                    } else {
                        self.cnt += 1;
                    }
                } else {
                    if self.cnt_end() {
                        self.cnt = 0;
                        self.half = false;
                        self.scl_oe = 1;
                        self.scl_o = 0;
                        if self.rd_mode {
                            self.tx = (self.tx << 1) | (self.sda_i & 1);
                        }
                        if self.bitcnt == 7 {
                            self.bitcnt = 0;
                            self.state = ST_ACK;
                        } else {
                            self.bitcnt += 1;
                        }
                    } else {
                        self.cnt += 1;
                    }
                }
            }
            ST_ACK => {
                if !self.half {
                    self.scl_oe = 1;
                    self.scl_o = 0;
                    if self.rd_mode {
                        self.sda_oe = 1;
                        self.sda_o = 0;
                    } else {
                        self.sda_oe = 0;
                        self.sda_o = 1;
                    }
                    if self.cnt_end() {
                        self.half = true;
                        self.scl_oe = 0;
                        self.cnt = 0;
                    } else {
                        self.cnt += 1;
                    }
                } else {
                    if self.cnt_end() {
                        self.cnt = 0;
                        self.half = false;
                        self.scl_oe = 1;
                        self.scl_o = 0;
                        if !self.rd_mode {
                            self.ack_rx = self.sda_i;
                            self.dout = self.tx;
                        } else {
                            self.dout = self.tx;
                        }
                        self.done = 1;
                        self.tip = 0;
                        self.state = ST_HOLD;
                    } else {
                        self.cnt += 1;
                    }
                }
            }
            ST_HOLD => {
                self.scl_oe = 1;
                self.scl_o = 0;
                self.sda_oe = 0;
                self.sda_o = 1;
                if self.pend_wr {
                    self.pend_wr = false;
                    self.rd_mode = false;
                    self.bitcnt = 0;
                    self.half = false;
                    self.cnt = 0;
                    self.tip = 1;
                    self.state = ST_SHIFT;
                } else if self.pend_rd {
                    self.pend_rd = false;
                    self.rd_mode = true;
                    self.tx = 0;
                    self.bitcnt = 0;
                    self.half = false;
                    self.cnt = 0;
                    self.tip = 1;
                    self.state = ST_SHIFT;
                } else if self.pend_stop {
                    self.pend_stop = false;
                    self.step = 0;
                    self.cnt = 0;
                    self.state = ST_STOP;
                } else if self.pend_start {
                    self.pend_start = false;
                    self.step = 0;
                    self.cnt = 0;
                    self.sda_oe = 1;
                    self.sda_o = 1;
                    self.state = ST_START;
                }
            }
            ST_STOP => {
                if self.step == 0 {
                    self.sda_oe = 1;
                    self.sda_o = 0;
                    self.scl_oe = 1;
                    self.scl_o = 0;
                    if self.cnt_end() {
                        self.step = 1;
                        self.cnt = 0;
                    } else {
                        self.cnt += 1;
                    }
                } else if self.step == 1 {
                    self.scl_oe = 0;
                    if self.cnt_end() {
                        self.step = 2;
                        self.cnt = 0;
                    } else {
                        self.cnt += 1;
                    }
                } else if self.step == 2 {
                    self.sda_oe = 0;
                    self.sda_o = 1;
                    if self.cnt_end() {
                        self.step = 3;
                        self.cnt = 0;
                    } else {
                        self.cnt += 1;
                    }
                } else {
                    self.busy = 0;
                    self.tip = 0;
                    self.state = ST_IDLE;
                }
            }
            _ => self.state = ST_IDLE,
        }

        // Command latch (end of step = NBA visibility next cycle).
        if input.we {
            if input.cmd == crate::CMD_WR {
                self.pend_wr = true;
                self.tx = input.din;
            }
            if input.cmd == crate::CMD_RD {
                self.pend_rd = true;
            }
            if input.cmd == crate::CMD_START {
                self.pend_start = true;
            }
            if input.cmd == crate::CMD_STOP {
                self.pend_stop = true;
            }
        }

        // Adopt the cycle's bus (computed by the ONE testbench slave from
        // the RTL's outputs — exactly what the DUT samples this cycle).
        let (scl_i, sda_i) = input.bus.get();
        self.scl_i = scl_i;
        self.sda_i = sda_i;
    }

    fn observe(&self) -> Obs {
        Obs {
            scl_o: self.scl_o,
            sda_o: self.sda_o,
            scl_oe: self.scl_oe,
            sda_oe: self.sda_oe,
            busy: self.busy,
            tip: self.tip,
            done: self.done,
            ack_rx: self.ack_rx,
            dout: self.dout,
        }
    }
}
