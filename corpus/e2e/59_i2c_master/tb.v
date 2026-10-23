`timescale 1ns/1ps
// Oracle leg for the spec-i2c I2C master: a behavioral open-drain slave,
// written independently of the Rust golden model in crates/spec-i2c. The
// expectations below are RECORDED from Verilator (expected.stdout), so the
// spec layer's lockstep agreement is cross-checked against a third author.
//
// Documented RTL characteristic this TB scripts around: the master enters
// ST_SHIFT directly when START completes and transmits whatever `tx` holds,
// so with no command queued the first byte after START is a phantom 0x00
// (the slave sees the address byte as the SECOND byte of a transaction).
//
// Driver discipline (mirrors the lockstep test in crates/spec-i2c): a
// strobe is issued only while tip==0, and after each strobe the driver
// waits for tip to rise (byte running) and then fall (byte done) before
// the next strobe. Waiting for the fall alone races the one-deep pend
// latch: HOLD can hand off to SHIFT on the very edge the driver notices
// the fall, and a mid-shift strobe rewrites tx under a live byte.
//
// Bus wiring: master drives split open-drain pins; the slave contributes a
// single pull-down bit. Data changes on the SCL falling edge (slave) / the
// SCL low half (master); both sides sample away from the driving edge.
// Single-writer discipline: every reg has exactly one driving process
// (A: frame/slave_sda on bus+falling edges; B: all counters on rising edges).
module tb;
  reg clk = 0;
  reg rst_n = 1;
  reg we = 0;
  reg [15:0] prescale = 16'd4;
  reg [3:0] cmd = 4'd0;
  reg [7:0] din = 8'd0;
  wire [7:0] dout;
  wire ack_rx, done, busy, tip;
  wire scl_o, sda_o, scl_oe, sda_oe;
  wire scl_i, sda_i;

  always #5 clk = ~clk;

  i2c_master dut(
    .clk(clk), .rst_n(rst_n), .prescale(prescale),
    .we(we), .cmd(cmd), .din(din),
    .dout(dout), .ack_rx(ack_rx), .done(done), .busy(busy), .tip(tip),
    .scl_i(scl_i), .sda_i(sda_i),
    .scl_o(scl_o), .sda_o(sda_o), .scl_oe(scl_oe), .sda_oe(sda_oe)
  );

  // Open-drain wire-AND (pull-up when nobody pulls low).
  assign scl_i = (scl_oe === 1'b1 && scl_o === 1'b0) ? 1'b0 : 1'b1;
  assign sda_i = ((sda_oe === 1'b1 && sda_o === 1'b0) || slave_sda === 1'b0)
                 ? 1'b0 : 1'b1;

  // ---------------- behavioral slave ----------------
  // One process owns every slave reg (single-writer; verilator-clean).
  // Edges are classified against the previous SCL/SDA levels, which also
  // keeps frame state correct across the idle gap between transactions —
  // sampling frame with an SCL edge cannot work there (SCL is parked high,
  // so the STOP level is never observed before the next START).
  reg slave_sda = 1'b1;    // 1 = released (pull-up wins)
  reg frame = 1'b0;        // inside START..STOP
  reg [7:0] stx = 8'h96;   // payload the slave hands out on a read
  reg scl_d = 1'b1, sda_d = 1'b1;  // previous levels (edge detection)
  reg [3:0] sbit = 4'd0;   // payload bits sampled in the current byte
  reg [1:0] byte_no = 2'd0;
  reg [3:0] btotal = 4'd0; // completed bytes across the whole run
  reg rw = 1'b0;
  reg rd_done = 1'b0;
  reg [7:0] srx = 8'd0;
  reg [7:0] cap0 = 8'd0, cap1 = 8'd0;

  always @(posedge scl_i or negedge scl_i or posedge sda_i or negedge sda_i) begin
    if (sda_i === 1'b0 && sda_d === 1'b1 && scl_i === 1'b1) begin
      // START: SDA fell while SCL was high. Fresh byte frame.
      frame <= 1'b1;
      slave_sda <= 1'b1;
      sbit <= 4'd0;
      byte_no <= 2'd0;
      rw <= 1'b0;
      rd_done <= 1'b0;
      srx <= 8'd0;
    end else if (sda_i === 1'b1 && sda_d === 1'b0 && scl_i === 1'b1) begin
      // STOP: SDA rose while SCL was high.
      frame <= 1'b0;
      slave_sda <= 1'b1;
    end else if (scl_i === 1'b1 && scl_d === 1'b0 && frame) begin
      // SCL rising edge: sample the data bit (sbit counts bits so far).
      srx <= {srx[6:0], sda_i};
      if (sbit == 4'd7 && byte_no == 2'd1) rw <= sda_i; // R/W bit (addr byte)
      if (sbit == 4'd8) begin
        sbit <= 4'd0;
        if (btotal == 4'd1) cap0 <= srx;   // tx1 address byte
        if (btotal == 4'd2) cap1 <= srx;   // tx1 data byte
        if (rw && byte_no != 2'd1) rd_done <= 1'b1;
        byte_no <= byte_no + 2'd1;
        btotal <= btotal + 4'd1;
      end else begin
        sbit <= sbit + 4'd1;
      end
    end else if (scl_i === 1'b0 && scl_d === 1'b1) begin
      // SCL falling edge: set up the next bit's drive level.
      if (!frame || rd_done)
        slave_sda <= 1'b1;               // idle or read serviced: release
      else if (sbit == 4'd8)
        slave_sda <= rw;                 // ACK slot: write -> pull low
      else if (rw)
        slave_sda <= stx[7 - sbit[2:0]]; // read: payload bit
      else
        slave_sda <= 1'b1;               // write: slave listens
    end
    scl_d <= scl_i;
    sda_d <= sda_i;
  end

  // ---------------- driver ----------------
  localparam CMD_START = 4'b0001;
  localparam CMD_STOP  = 4'b0010;
  localparam CMD_WR    = 4'b0100;
  localparam CMD_RD    = 4'b1000;

  // Strobe + handshake: issue while parked, wait for tip rise then fall.
  // (Bytes only; STOP never raises tip — strobe it inline + wait busy.)
  task strobe_byte;
    input [3:0] c;
    input [7:0] d;
    begin
      while (tip) @(posedge clk);
      @(posedge clk); #1; cmd = c; din = d; we = 1'b1;
      @(posedge clk); #1; we = 1'b0;
      while (!tip) @(posedge clk);
      while (tip) @(posedge clk);
    end
  endtask

  task strobe_stop;
    begin
      while (tip) @(posedge clk);
      @(posedge clk); #1; cmd = CMD_STOP; we = 1'b1;
      @(posedge clk); #1; we = 1'b0;
      while (busy) @(posedge clk);
    end
  endtask

  initial begin
    // Canonical reset under the 0-init policy: pre-charge high, then 1->0.
    @(negedge clk); rst_n = 1'b0;
    @(negedge clk); rst_n = 1'b1;

    // Transaction 1: START, write 0x20 (address) and 0x3C (data), STOP.
    @(posedge clk); #1; cmd = CMD_START; we = 1'b1;
    @(posedge clk); #1; we = 1'b0;
    while (!tip) @(posedge clk);
    #1 $display("start tip=%b busy=%b", tip, busy);
    while (tip) @(posedge clk);
    #1 $display("phantom ack=%b", ack_rx);

    strobe_byte(CMD_WR, 8'h20);
    #1 $display("addr ack=%b", ack_rx);
    strobe_byte(CMD_WR, 8'h3c);
    #1 $display("data ack=%b", ack_rx);

    while (tip) @(posedge clk);
    @(posedge clk); #1; cmd = CMD_STOP; we = 1'b1;
    @(posedge clk); #1; we = 1'b0;
    while (busy) @(posedge clk);
    #1 $display("idle1 scl=%b sda=%b", scl_i, sda_i);

    // Transaction 2: START, address-with-read 0x21, read one byte (the
    // slave sends 0x96), STOP.
    strobe_byte(CMD_START, 8'h00);
    strobe_byte(CMD_WR, 8'h21);
    #1 $display("addr2 ack=%b", ack_rx);
    strobe_byte(CMD_RD, 8'h00);
    #1 $display("rd dout=%h ack=%b", dout, ack_rx);
    strobe_stop;
    #1 $display("idle2 scl=%b sda=%b tip=%b", scl_i, sda_i, tip);
    $display("slave saw %h %h", cap0, cap1);
    $finish;
  end
endmodule
