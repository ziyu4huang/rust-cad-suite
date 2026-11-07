// tb_up3down5 — template tb_plan-v1(機械生成;T 腿基線;期望由原版錄製,禁手寫/禁加自檢)
// TB-LEG: template tb_plan-v1 seed=305419896 cycles=32 flush=8
// SCENES: 4
// COVERS: 1: *
// COVERS: 2: *
// COVERS: 3: *
// COVERS: 4: *
// WAIVE: up3down5.R007 x-valued

`timescale 1ns/1ps
module tb_up3down5;

  reg clock;
  reg [8:0] data_in;
  reg up;
  reg down;
  wire carry_out;
  wire borrow_out;
  wire [8:0] count_out;
  wire parity_out;

  reg [31:0] tb__rnd;
  integer tb__i;

  up3down5 tb__dut (
    .clock(clock),
    .data_in(data_in),
    .up(up),
    .down(down),
    .carry_out(carry_out),
    .borrow_out(borrow_out),
    .count_out(count_out),
    .parity_out(parity_out)
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

  initial clock = 1'b0;
  always #5 clock = ~clock;

  initial begin : tb__scenes
    tb__rnd = 32'd305419896;
    #1;
    data_in = {9{1'b0}};
    up = 1'b0;
    down = 1'b0;
    // SCENE1 flush
    $display("SCENE1 flush");
    repeat (8) begin
      @(negedge clock);
      tb__rnd = tb__xs32(tb__rnd);
      data_in = tb__rnd[11:3];
      up = tb__rnd[10];
      down = tb__rnd[17];
    end
    // SCENE2 full
    $display("SCENE2 full");
    repeat (33) begin
      @(negedge clock);
      $display("  carry_out=%h", carry_out);
      $display("  borrow_out=%h", borrow_out);
      $display("  count_out=%h", count_out);
      $display("  parity_out=%h", parity_out);
      tb__rnd = tb__xs32(tb__rnd);
      data_in = tb__rnd[11:3];
      up = tb__rnd[10];
      down = tb__rnd[17];
    end
    // SCENE3 small
    $display("SCENE3 small");
    repeat (33) begin
      @(negedge clock);
      $display("  carry_out=%h", carry_out);
      $display("  borrow_out=%h", borrow_out);
      $display("  count_out=%h", count_out);
      $display("  parity_out=%h", parity_out);
      tb__rnd = tb__xs32(tb__rnd);
      data_in = {{6{1'b0}}, tb__rnd[5:3]};
      up = tb__rnd[10];
      down = tb__rnd[17];
    end
    // SCENE4 hold
    $display("SCENE4 hold");
    for (tb__i = 0; tb__i < 32; tb__i = tb__i + 1) begin
      if (tb__i % 8 == 0) begin
        tb__rnd = tb__xs32(tb__rnd);
        data_in = tb__rnd[11:3];
        up = tb__rnd[10];
        down = tb__rnd[17];
      end
      @(negedge clock);
      $display("  carry_out=%h", carry_out);
      $display("  borrow_out=%h", borrow_out);
      $display("  count_out=%h", count_out);
      $display("  parity_out=%h", parity_out);
    end
    $display("DONE");
    $finish;
  end
endmodule
