// tb_omsp_dbg_uart — template tb_plan-v1(機械生成;T 腿基線;期望由原版錄製,禁手寫/禁加自檢)
// TB-LEG: template tb_plan-v1 seed=305419896 cycles=32 flush=8
// SCENES: 4
// COVERS: 1: omsp_dbg_uart.R001
// COVERS: 2: *
// COVERS: 3: *
// COVERS: 4: *

`timescale 1ns/1ps
module tb_omsp_dbg_uart;

  reg dbg_clk;
  reg dbg_rst;
  reg mem_burst;
  wire cmd_valid;

  reg [31:0] tb__rnd;
  integer tb__i;

  omsp_dbg_uart tb__dut (
    .dbg_clk(dbg_clk),
    .dbg_rst(dbg_rst),
    .mem_burst(mem_burst),
    .cmd_valid(cmd_valid)
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

  initial dbg_clk = 1'b0;
  always #5 dbg_clk = ~dbg_clk;

  initial begin : tb__scenes
    tb__rnd = 32'd305419896;
    #1;
    dbg_rst = 1'b0;
    mem_burst = 1'b0;
    // SCENE1 arst_dbg_rst
    $display("SCENE1 arst_dbg_rst");
    @(negedge dbg_clk);
    #2;
    dbg_rst = 1'b1;
    #1;
    $display("  cmd_valid=%h", cmd_valid);
    @(negedge dbg_clk);
    dbg_rst = 1'b0;
    repeat (4) begin
      @(negedge dbg_clk);
      $display("  cmd_valid=%h", cmd_valid);
      tb__rnd = tb__xs32(tb__rnd);
      mem_burst = tb__rnd[3];
    end
    // SCENE2 full
    $display("SCENE2 full");
    repeat (33) begin
      @(negedge dbg_clk);
      $display("  cmd_valid=%h", cmd_valid);
      tb__rnd = tb__xs32(tb__rnd);
      mem_burst = tb__rnd[3];
    end
    // SCENE3 small
    $display("SCENE3 small");
    repeat (33) begin
      @(negedge dbg_clk);
      $display("  cmd_valid=%h", cmd_valid);
      tb__rnd = tb__xs32(tb__rnd);
      mem_burst = tb__rnd[3];
    end
    // SCENE4 hold
    $display("SCENE4 hold");
    for (tb__i = 0; tb__i < 32; tb__i = tb__i + 1) begin
      if (tb__i % 8 == 0) begin
        tb__rnd = tb__xs32(tb__rnd);
        mem_burst = tb__rnd[3];
      end
      @(negedge dbg_clk);
      $display("  cmd_valid=%h", cmd_valid);
    end
    $display("DONE");
    $finish;
  end
endmodule
