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

/// Ideal slave: acks address 0x50 (+R bit), acks every data byte, and
/// emits read data from a fixed pattern.
pub struct Slave {
    pub addr: u8,
    read_data: [u8; 4],
    read_idx: usize,
    prev_scl: u8,
    prev_sda: u8,
    started: bool,
    bits: Vec<u8>,
    acking: bool,
    read_mode: bool,
    pub drives_sda_low: bool,
    pub prev_ack_done: bool,
}

impl Slave {
    pub fn new(addr: u8) -> Slave {
        Slave {
            addr,
            read_data: [0xA5, 0x3C, 0x81, 0x5A],
            read_idx: 0,
            prev_scl: 1,
            prev_sda: 1,
            started: false,
            bits: Vec::new(),
            acking: false,
            read_mode: false,
            drives_sda_low: false,
            prev_ack_done: false,
        }
    }
}

/// The open-drain wire-AND + slave, shared by the golden model's bus view.
/// Given the master's drive outputs, returns the bus levels.
pub fn bus_levels(m: &MasterView, slave: &mut Slave) -> (u8, u8) {
    let scl_i = if m.scl_oe != 0 && m.scl_o == 0 { 0 } else { 1 };
    let mut sda_i = if m.sda_oe != 0 && m.sda_o == 0 { 0 } else { 1 };
    // Slave reacts to SDA/SCL on the falling edge of SCL (change) and the
    // rising edge (sample); START/STOP detected on SDA falls/rises while
    // SCL is high.
    if slave.prev_scl == 1 && scl_i == 1 && slave.prev_sda == 1 && sda_i == 0 {
        slave.started = true;
        slave.bits.clear();
    }
    if slave.prev_scl == 1 && scl_i == 1 && slave.prev_sda == 0 && sda_i == 1 {
        slave.started = false;
        slave.acking = false;
        slave.read_mode = false;
        slave.drives_sda_low = false;
        slave.bits.clear();
    }
    // Rising edge: a slave holding ACK keeps it through the whole 9th
    // clock (the master samples at the end of the high half) and releases
    // at the falling edge instead.
    if slave.started && slave.prev_scl == 1 && scl_i == 0 {
        // Falling edge: the ACK slot (if any) ends here — release the drive
        // while SCL is low so the next phase starts clean.
        if slave.acking {
            slave.drives_sda_low = false;
            slave.acking = false;
            slave.bits.clear();
        }
        // Data-bit falling edge: capture; the ack level is NOT data and
        // would misalign the next byte's bit counting.
        if !slave.acking {
            slave.bits.push(sda_i);
        }
        if slave.acking {
            // ack released this fall; nothing else to do this edge
        } else if slave.bits.len() == 8 {
            let byte = slave
                .bits
                .iter()
                .fold(0u8, |a, &b| (a << 1) | b);
            slave.read_mode = byte & 1 == 1;
            let matched = (byte >> 1) == slave.addr;
            slave.acking = matched || !slave.read_mode;
            slave.drives_sda_low = slave.acking && !slave.read_mode;
            if slave.read_mode {
                slave.drives_sda_low = false;
            }
            slave.bits.clear();
        } else if slave.read_mode && slave.bits.len() < 8 {
            // drive the next read-data bit during this low half
            let bitpos = 7 - slave.bits.len();
            let word = slave.read_data[slave.read_idx % slave.read_data.len()];
            slave.drives_sda_low = (word >> bitpos) & 1 == 0;
        }
        if slave.read_mode && slave.bits.is_empty() && slave.prev_ack_done {
            // read byte + ack finished: advance to the next word
            slave.read_idx += 1;
            slave.prev_ack_done = false;
        }
    }
    if slave.drives_sda_low {
        sda_i = 0;
    }
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
    // own bus view
    pub slave: Slave,
    pub scl_i: u8,
    pub sda_i: u8,
}

impl I2cGolden {
    pub fn new(prescale: u16, slave_addr: u8) -> I2cGolden {
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
            slave: Slave::new(slave_addr),
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

        // Own bus view (same wire-AND + slave).
        let view = MasterView {
            scl_o: self.scl_o,
            sda_o: self.sda_o,
            scl_oe: self.scl_oe,
            sda_oe: self.sda_oe,
        };
        let (scl_i, sda_i) = bus_levels(&view, &mut self.slave);
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
