// tb_dff0_test — template tb_plan-v1(機械生成;T 腿基線;期望由原版錄製,禁手寫/禁加自檢)
// TB-LEG: template tb_plan-v1 seed=305419896 cycles=32 flush=8
// SCENES: 4
// COVERS: 1: *
// COVERS: 2: *
// COVERS: 3: *
// COVERS: 4: *

`timescale 1ns/1ps
module tb_dff0_test;

  wire n1;
  wire n1_inv;
  reg clk;

  reg [31:0] tb__rnd;
  integer tb__i;

  dff0_test tb__dut (
    .n1(n1),
    .n1_inv(n1_inv),
    .clk(clk)
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
    // SCENE1 flush
    $display("SCENE1 flush");
    repeat (8) begin
      @(negedge clk);
      tb__rnd = tb__xs32(tb__rnd);
    end
    // SCENE2 full
    $display("SCENE2 full");
    repeat (33) begin
      @(negedge clk);
      $display("  n1=%h", n1);
      $display("  n1_inv=%h", n1_inv);
      tb__rnd = tb__xs32(tb__rnd);
    end
    // SCENE3 small
    $display("SCENE3 small");
    repeat (33) begin
      @(negedge clk);
      $display("  n1=%h", n1);
      $display("  n1_inv=%h", n1_inv);
      tb__rnd = tb__xs32(tb__rnd);
    end
    // SCENE4 hold
    $display("SCENE4 hold");
    for (tb__i = 0; tb__i < 32; tb__i = tb__i + 1) begin
      if (tb__i % 8 == 0) begin
        tb__rnd = tb__xs32(tb__rnd);
      end
      @(negedge clk);
      $display("  n1=%h", n1);
      $display("  n1_inv=%h", n1_inv);
    end
    $display("DONE");
    $finish;
  end
endmodule
