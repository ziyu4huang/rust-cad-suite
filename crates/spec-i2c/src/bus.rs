//! The RTL-side bus: open-drain wire-AND computed from the DUT's split
//! outputs plus a behavioral slave, driving `scl_i/sda_i` back into the DUT
//! each clk (before the posedge — inputs settle combinationally). The same
//! wire-AND lives inside the golden model's own bus view.

use crate::model::{bus_levels, MasterView, Slave};
use crate::CmdInput;
use vl_spec::SignalId;

pub struct BusDriver {
    pub scl_i: SignalId,
    pub sda_i: SignalId,
    pub we: SignalId,
    pub cmd: SignalId,
    pub din: SignalId,
    pub scl_o: SignalId,
    pub sda_o: SignalId,
    pub scl_oe: SignalId,
    pub sda_oe: SignalId,
    pub slave: Slave,
}

impl BusDriver {
    /// One clk-cycle bus update: read the DUT's drive outputs, run the
    /// slave against the bus edges, drive the bus levels and the command
    /// port back into the DUT. Takes (sim, sigs) directly so the harness
    /// driver closure can pass them through.
    pub fn drive(&mut self, sim: &mut vl_sim::USim, sigs: &vl_spec::Signals, input: &CmdInput) {
        let g = |id: SignalId| -> u8 { sigs.u64(sim, id).unwrap_or(0) as u8 };
        let view = MasterView {
            scl_o: g(self.scl_o),
            sda_o: g(self.sda_o),
            scl_oe: g(self.scl_oe),
            sda_oe: g(self.sda_oe),
        };
        let (scl_i, sda_i) = bus_levels(&view, &mut self.slave);
        // The ONE bus: publish the levels for the golden master's view too
        // (it adopts exactly what the DUT samples this cycle).
        input.bus.set((scl_i, sda_i));
        sigs.put_int(sim, self.scl_i, scl_i as i64).ok();
        sigs.put_int(sim, self.sda_i, sda_i as i64).ok();
        sigs.put_int(sim, self.we, input.we as i64).ok();
        sigs.put_int(sim, self.cmd, input.cmd as i64).ok();
        sigs.put_int(sim, self.din, input.din as i64).ok();
    }

    /// Recompute the bus levels from the CURRENT DUT outputs without
    /// stepping the slave (used post-half so per-cycle invariants see the
    /// bus as of the real SCL/SDA edges, not one clk stale).
    pub fn refresh(&mut self, sim: &mut vl_sim::USim, sigs: &vl_spec::Signals) {
        let g = |id: SignalId| -> u8 { sigs.u64(sim, id).unwrap_or(0) as u8 };
        let view = MasterView {
            scl_o: g(self.scl_o),
            sda_o: g(self.sda_o),
            scl_oe: g(self.scl_oe),
            sda_oe: g(self.sda_oe),
        };
        let (scl_i, sda_i) = bus_levels_no_edge(&view, &mut self.slave);
        sigs.put_int(sim, self.scl_i, scl_i as i64).ok();
        sigs.put_int(sim, self.sda_i, sda_i as i64).ok();
    }
}

/// Levels-only recompute (slave edge state untouched).
fn bus_levels_no_edge(m: &MasterView, slave: &mut Slave) -> (u8, u8) {
    let scl_i = if m.scl_oe != 0 && m.scl_o == 0 { 0 } else { 1 };
    let mut sda_i = if m.sda_oe != 0 && m.sda_o == 0 { 0 } else { 1 };
    if slave.drives_sda_low {
        sda_i = 0;
    }
    (scl_i, sda_i)
}
