//! Minimal repro (found by the read-transaction lockstep): a serial
//! `{tx[6:0], sda}` shift under put_int-driven clocking. The golden master
//! shifts the same bus correctly (0xA5); the RTL produced 0x01 — every
//! shift evaluated as if `tx[6:0]` were 0.

use vl_spec::{build_sim, Signals};

const RTL: &str = "module tb(\n  input clk,\n  input sda,\n  output reg [7:0] tx\n);\n  always @(posedge clk) tx <= {tx[6:0], sda};\nendmodule\n";

#[test]
fn serial_shift_put_clocking() {
    let sim = build_sim(&[("t.v", RTL)], "tb").expect("build");
    let mut sim = sim;
    let mut sigs = Signals::default();
    let clk = sigs.bind(&sim, "clk").expect("clk");
    let sda = sigs.bind(&sim, "sda").expect("sda");
    let tx = sigs.bind(&sim, "tx").expect("tx");

    // Mirror LockstepSpec::cycle: driver put at clk-low, posedge half,
    // negedge half.
    // Deliberately NO priming run_until: the very first put-driven edge
    // used to be lost because it fired before the scheduler's init_once
    // had run the always-block prologues (no process was registered on its
    // sensitivity list yet). The put path now seeds the scheduler itself.
    let mut t = sim.now_ticks();
    let pattern = [1u8, 0, 1, 0, 0, 1, 0, 1];
    for &b in &pattern {
        sigs.put_int(&mut sim, sda, b as i64).unwrap(); // setup before the edge
        sigs.put_int(&mut sim, clk, 1).unwrap();
        t += 5;
        sim.run_until(t).expect("posedge half");
        sigs.put_int(&mut sim, clk, 0).unwrap();
        t += 5;
        sim.run_until(t).expect("negedge half");
        if std::env::var("SHIFT_DEBUG").is_ok() {
            eprintln!("after bit {b}: tx={:02x}", sigs.u64(&sim, tx).unwrap());
        }
    }
    let got = sigs.u64(&sim, tx).unwrap();
    assert_eq!(got, 0xA5, "shifted-in byte wrong");
}

/// Layer 2: the same shift under the I2C master's exact always shape —
/// mixed-edge sensitivity (posedge clk or negedge rst_n) with a reset
/// branch. If this fails while layer 1 passes, the NBA RHS part-select
/// breaks under async-reset arming.
const RTL_MIXED: &str = "module tb(\n  input clk,\n  input rst_n,\n  input sda,\n  output reg [7:0] tx\n);\n  always @(posedge clk or negedge rst_n) begin\n    if (!rst_n) tx <= 8'h00;\n    else tx <= {tx[6:0], sda};\n  end\nendmodule\n";

#[test]
fn serial_shift_mixed_edge_reset() {
    let sim = build_sim(&[("t.v", RTL_MIXED)], "tb").expect("build");
    let mut sim = sim;
    let mut sigs = Signals::default();
    let clk = sigs.bind(&sim, "clk").expect("clk");
    let rst = sigs.bind(&sim, "rst_n").expect("rst_n");
    let sda = sigs.bind(&sim, "sda").expect("sda");
    let tx = sigs.bind(&sim, "tx").expect("tx");

    // Canonical reset under the 0-init policy: pre-charge high, 1->0, release.
    let mut t = sim.now_ticks();
    sigs.put_int(&mut sim, clk, 1).unwrap();
    t += 5;
    sim.run_until(t).expect("prime posedge");
    sigs.put_int(&mut sim, rst, 1).unwrap();
    t += 5;
    sim.run_until(t).expect("pre-charge");
    sigs.put_int(&mut sim, rst, 0).unwrap();
    t += 5;
    sim.run_until(t).expect("reset assert");
    sigs.put_int(&mut sim, rst, 1).unwrap();
    t += 5;
    sim.run_until(t).expect("reset release");
    // Park the clock LOW before the data loop: the reset sequence left it
    // high, and a put clk=1 on an already-high clk is no edge at all.
    sigs.put_int(&mut sim, clk, 0).unwrap();
    t += 5;
    sim.run_until(t).expect("park clk low");

    let pattern = [1u8, 0, 1, 0, 0, 1, 0, 1];
    for &b in &pattern {
        sigs.put_int(&mut sim, sda, b as i64).unwrap();
        sigs.put_int(&mut sim, clk, 1).unwrap();
        t += 5;
        sim.run_until(t).expect("posedge half");
        sigs.put_int(&mut sim, clk, 0).unwrap();
        t += 5;
        sim.run_until(t).expect("negedge half");
    }
    let got = sigs.u64(&sim, tx).unwrap();
    assert_eq!(got, 0xA5, "shifted-in byte wrong (mixed-edge shape)");
}

/// Layer 3: the REAL i2c_master RTL, minimal harness (no slave, no golden —
/// the bus is released-high throughout, so a working shift must read
/// 0xff on the RD byte; the observed failure signature was dout=0x01).
const RTL_I2C: &str = include_str!("../rtl/i2c_master.v");

#[test]
fn i2c_rd_shift_minimal_harness() {
    let sim = build_sim(&[("i2c_master.v", RTL_I2C)], "i2c_master").expect("build");
    let mut sim = sim;
    let mut sigs = Signals::default();
    let clk = sigs.bind(&sim, "clk").expect("clk");
    let rst = sigs.bind(&sim, "rst_n").expect("rst_n");
    let we = sigs.bind(&sim, "we").expect("we");
    let cmd = sigs.bind(&sim, "cmd").expect("cmd");
    let din = sigs.bind(&sim, "din").expect("din");
    let dout = sigs.bind(&sim, "dout").expect("dout");
    let tip = sigs.bind(&sim, "tip").expect("tip");
    let scl_i = sigs.bind(&sim, "scl_i").expect("scl_i");
    let sda_i = sigs.bind(&sim, "sda_i").expect("sda_i");
    let _ = scl_i;
    let dbg = std::env::var("SL_DEBUG").is_ok();
    let d_tx = sigs.bind(&sim, "tx").ok();
    let d_rd = sigs.bind(&sim, "rd_mode").ok();
    let d_st = sigs.bind(&sim, "state").ok();
    let mut cycle_no = 0u32;

    let mut t = sim.now_ticks();
    let mut cyc = |sigs: &mut Signals,
                   sim: &mut vl_sim::interp::Sim,
                   t: &mut u64,
                   we_v: u8,
                   cmd_v: u8,
                   din_v: u8| {
        sigs.put_int(sim, we, we_v as i64).unwrap();
        sigs.put_int(sim, cmd, cmd_v as i64).unwrap();
        sigs.put_int(sim, din, din_v as i64).unwrap();
        sigs.put_int(sim, clk, 1).unwrap();
        *t += 5;
        sim.run_until(*t).expect("posedge");
        sigs.put_int(sim, we, 0).unwrap();
        sigs.put_int(sim, clk, 0).unwrap();
        *t += 5;
        sim.run_until(*t).expect("negedge");
        if dbg {
            cycle_no += 1;
            let g = |sigs: &Signals, sim: &vl_sim::interp::Sim, id: &Option<vl_spec::SignalId>| {
                id.and_then(|i| sigs.u64(sim, i).ok()).unwrap_or(u64::MAX)
            };
            eprintln!(
                "cyc={} st={} rd={} tx={:02x}",
                cycle_no,
                g(sigs, sim, &d_st),
                g(sigs, sim, &d_rd),
                g(sigs, sim, &d_tx)
            );
        }
    };

    // Reset (canonical 1->0), bus released high, prescale 4.
    cyc(&mut sigs, &mut sim, &mut t, 0, 0, 0);
    sigs.put_int(&mut sim, rst, 1).unwrap();
    cyc(&mut sigs, &mut sim, &mut t, 0, 0, 0);
    sigs.put_int(&mut sim, rst, 0).unwrap();
    cyc(&mut sigs, &mut sim, &mut t, 0, 0, 0);
    sigs.put_int(&mut sim, rst, 1).unwrap();
    for _ in 0..4 {
        cyc(&mut sigs, &mut sim, &mut t, 0, 0, 0);
    }
    // prescale = 4
    // (prescale is an input; drive it once)
    let prescale = sigs.bind(&sim, "prescale").expect("prescale");
    sigs.put_int(&mut sim, prescale, 4).unwrap();
    // Release the bus high (a driven harness, unlike the 0-init defaults).
    sigs.put_int(&mut sim, scl_i, 1).unwrap();
    sigs.put_int(&mut sim, sda_i, 1).unwrap();

    // START
    cyc(&mut sigs, &mut sim, &mut t, 1, 0b0001, 0);
    cyc(&mut sigs, &mut sim, &mut t, 0, 0, 0);
    // wait out the phantom byte (tip rises then falls)
    let mut guard = 0;
    while sigs.u64(&sim, tip).unwrap() == 0 {
        cyc(&mut sigs, &mut sim, &mut t, 0, 0, 0);
        guard += 1;
        assert!(guard < 200, "tip never rose");
    }
    while sigs.u64(&sim, tip).unwrap() == 1 {
        cyc(&mut sigs, &mut sim, &mut t, 0, 0, 0);
        guard += 1;
        assert!(guard < 400, "phantom never finished");
    }
    // WR address-with-read 0xA1
    cyc(&mut sigs, &mut sim, &mut t, 1, 0b0100, 0xA1);
    cyc(&mut sigs, &mut sim, &mut t, 0, 0, 0);
    guard = 0;
    while sigs.u64(&sim, tip).unwrap() == 0 {
        cyc(&mut sigs, &mut sim, &mut t, 0, 0, 0);
        guard += 1;
        assert!(guard < 200, "addr tip never rose");
    }
    // (simpler: poll tip down)
    while sigs.u64(&sim, tip).unwrap() == 1 {
        cyc(&mut sigs, &mut sim, &mut t, 0, 0, 0);
        guard += 1;
        assert!(guard < 600, "addr never finished");
    }
    // RD byte: the bus is released-high, so every sampled bit is 1.
    cyc(&mut sigs, &mut sim, &mut t, 1, 0b1000, 0);
    cyc(&mut sigs, &mut sim, &mut t, 0, 0, 0);
    guard = 0;
    while sigs.u64(&sim, tip).unwrap() == 0 {
        cyc(&mut sigs, &mut sim, &mut t, 0, 0, 0);
        guard += 1;
        assert!(guard < 200, "rd tip never rose");
    }
    while sigs.u64(&sim, tip).unwrap() == 1 {
        cyc(&mut sigs, &mut sim, &mut t, 0, 0, 0);
        guard += 1;
        assert!(guard < 600, "rd never finished");
    }
    let got = sigs.u64(&sim, dout).unwrap();
    // The bus is released: a correct shift reads 0xff. The failing
    // signature was 0x01 (only the last sample landed).
    assert_eq!(got, 0xff, "RD shift with a released bus");
}

