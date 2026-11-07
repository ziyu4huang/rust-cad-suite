// tb_i2c_master — template tb_plan-v1(機械生成;T 腿基線;期望由原版錄製,禁手寫/禁加自檢)
// TB-LEG: template tb_plan-v1 seed=305419896 cycles=32 flush=8
// SCENES: 4
// COVERS: 1: i2c_master.R001,i2c_master.R002,i2c_master.R003,i2c_master.R004,i2c_master.R005,i2c_master.R006,i2c_master.R007,i2c_master.R008,i2c_master.R009,i2c_master.R010,i2c_master.R011,i2c_master.R012,i2c_master.R013,i2c_master.R014,i2c_master.R015,i2c_master.R016,i2c_master.R017,i2c_master.R018,i2c_master.R019,i2c_master.R020
// COVERS: 2: *
// COVERS: 3: *
// COVERS: 4: *

`timescale 1ns/1ps
module tb_i2c_master;

  reg clk;
  reg rst_n;
  reg we;
  reg scl_i;
  reg sda_i;
  wire ack_rx;
  wire done;
  wire busy;
  wire tip;
  wire scl_o;
  wire sda_o;
  wire scl_oe;
  wire sda_oe;
  reg [15:0] prescale;
  reg [3:0] cmd;
  reg [7:0] din;
  wire [7:0] dout;

  reg [31:0] tb__rnd;
  integer tb__i;

  i2c_master tb__dut (
    .clk(clk),
    .rst_n(rst_n),
    .we(we),
    .scl_i(scl_i),
    .sda_i(sda_i),
    .ack_rx(ack_rx),
    .done(done),
    .busy(busy),
    .tip(tip),
    .scl_o(scl_o),
    .sda_o(sda_o),
    .scl_oe(scl_oe),
    .sda_oe(sda_oe),
    .prescale(prescale),
    .cmd(cmd),
    .din(din),
    .dout(dout)
  );

  function [31:0] tb__xs32;
    input [31:0] x;
    reg [31:0] y;
    begin
      y = x;
      y = y ^ (y << 13);
      y = y ^ (y >> 17);
      y = y ^ (y << 5);
      tb__xs32 = y;
    end
  endfunction

  initial begin
    #2390;
    $display("WATCHDOG");
    $finish;
  end

  initial clk = 1'b0;
  always #5 clk = ~clk;

  initial begin : tb__scenes
    tb__rnd = 32'd305419896;
    #1;
    rst_n = 1'b1;
    we = 1'b0;
    scl_i = 1'b0;
    sda_i = 1'b0;
    prescale = {16{1'b0}};
    cmd = {4{1'b0}};
    din = {8{1'b0}};
    // SCENE1 arst_rst_n
    $display("SCENE1 arst_rst_n");
    @(negedge clk);
    #2;
    rst_n = 1'b0;
    #1;
    $display("  ack_rx=%h", ack_rx);
    $display("  done=%h", done);
    $display("  busy=%h", busy);
    $display("  tip=%h", tip);
    $display("  scl_o=%h", scl_o);
    $display("  sda_o=%h", sda_o);
    $display("  scl_oe=%h", scl_oe);
    $display("  sda_oe=%h", sda_oe);
    $display("  dout=%h", dout);
    @(negedge clk);
    rst_n = 1'b1;
    repeat (4) begin
      @(negedge clk);
      $display("  ack_rx=%h", ack_rx);
      $display("  done=%h", done);
      $display("  busy=%h", busy);
      $display("  tip=%h", tip);
      $display("  scl_o=%h", scl_o);
      $display("  sda_o=%h", sda_o);
      $display("  scl_oe=%h", scl_oe);
      $display("  sda_oe=%h", sda_oe);
      $display("  dout=%h", dout);
      tb__rnd = tb__xs32(tb__rnd);
      we = tb__rnd[3];
      scl_i = tb__rnd[10];
      sda_i = tb__rnd[17];
      prescale = tb__rnd[15:0];
      cmd = tb__rnd[10:7];
      din = tb__rnd[21:14];
    end
    // SCENE2 full
    $display("SCENE2 full");
    repeat (33) begin
      @(negedge clk);
      $display("  ack_rx=%h", ack_rx);
      $display("  done=%h", done);
      $display("  busy=%h", busy);
      $display("  tip=%h", tip);
      $display("  scl_o=%h", scl_o);
      $display("  sda_o=%h", sda_o);
      $display("  scl_oe=%h", scl_oe);
      $display("  sda_oe=%h", sda_oe);
      $display("  dout=%h", dout);
      tb__rnd = tb__xs32(tb__rnd);
      we = tb__rnd[3];
      scl_i = tb__rnd[10];
      sda_i = tb__rnd[17];
      prescale = tb__rnd[15:0];
      cmd = tb__rnd[10:7];
      din = tb__rnd[21:14];
    end
    // SCENE3 small
    $display("SCENE3 small");
    repeat (33) begin
      @(negedge clk);
      $display("  ack_rx=%h", ack_rx);
      $display("  done=%h", done);
      $display("  busy=%h", busy);
      $display("  tip=%h", tip);
      $display("  scl_o=%h", scl_o);
      $display("  sda_o=%h", sda_o);
      $display("  scl_oe=%h", scl_oe);
      $display("  sda_oe=%h", sda_oe);
      $display("  dout=%h", dout);
      tb__rnd = tb__xs32(tb__rnd);
      we = tb__rnd[3];
      scl_i = tb__rnd[10];
      sda_i = tb__rnd[17];
      prescale = {{13{1'b0}}, tb__rnd[2:0]};
      cmd = {{1{1'b0}}, tb__rnd[9:7]};
      din = {{5{1'b0}}, tb__rnd[16:14]};
    end
    // SCENE4 hold
    $display("SCENE4 hold");
    for (tb__i = 0; tb__i < 32; tb__i = tb__i + 1) begin
      if (tb__i % 8 == 0) begin
        tb__rnd = tb__xs32(tb__rnd);
        we = tb__rnd[3];
        scl_i = tb__rnd[10];
        sda_i = tb__rnd[17];
        prescale = tb__rnd[15:0];
        cmd = tb__rnd[10:7];
        din = tb__rnd[21:14];
      end
      @(negedge clk);
      $display("  ack_rx=%h", ack_rx);
      $display("  done=%h", done);
      $display("  busy=%h", busy);
      $display("  tip=%h", tip);
      $display("  scl_o=%h", scl_o);
      $display("  sda_o=%h", sda_o);
      $display("  scl_oe=%h", scl_oe);
      $display("  sda_oe=%h", sda_oe);
      $display("  dout=%h", dout);
    end
    $display("DONE");
    $finish;
  end
endmodule
