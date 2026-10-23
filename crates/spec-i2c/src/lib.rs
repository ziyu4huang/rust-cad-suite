//! spec-i2c — spec-verification demonstration (ADR-0007).
//!
//! Layer 1 (interface contract) lives in the RTL header comment and is
//! asserted by [`invariants`]; layer 2 (executable golden model) is
//! [`model::I2cGolden`]; layer 3 (per-cycle protocol invariants) is
//! [`invariants`]. [`bus::I2cBus`] is the open-drain wire-AND + behavioral
//! slave that drives the RTL's `scl_i/sda_i` from the DUT's split outputs —
//! the same values the golden model computes internally.
//!
//! Everything here is written from the I2C-bus protocol description for this
//! demo; OpenCores' GPL core was read-only inspiration, zero code copied.

pub mod bus;
pub mod invariants;
pub mod model;

/// Command encoding shared by the RTL, the driver and the golden model:
/// `{STOP, START, RD, WR}` = bits 3..0.
pub const CMD_START: u8 = 0b0001;
pub const CMD_STOP: u8 = 0b0010;
pub const CMD_WR: u8 = 0b0100;
pub const CMD_RD: u8 = 0b1000;

/// One cycle's command-port stimulus (`we` strobe + payload).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CmdInput {
    pub we: bool,
    pub cmd: u8,
    pub din: u8,
}

impl CmdInput {
    pub fn idle() -> Self {
        CmdInput::default()
    }
    pub fn strobe(cmd: u8, din: u8) -> Self {
        CmdInput {
            we: true,
            cmd,
            din,
        }
    }
}

pub const RTL: &str = include_str!("../rtl/i2c_master.v");
