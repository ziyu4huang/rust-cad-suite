// tb_attrib09_bar — template tb_plan-v1(機械生成;T 腿基線;期望由原版錄製,禁手寫/禁加自檢)
// TB-LEG: template tb_plan-v1 seed=305419896 cycles=32 flush=8
// SCENES: 2
// COVERS: 1: *
// COVERS: 2: *

`timescale 1ns/1ps
module tb_attrib09_bar;

  reg clk;
  reg rst;
  reg [1:0] inp;
  wire [1:0] out;

  reg [31:0] tb__rnd;
  integer tb__i;

  attrib09_bar tb__dut (
    .clk(clk),
    .rst(rst),
    .inp(inp),
    .out(out)
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
    #1544;
    $display("WATCHDOG");
    $finish;
  end

  initial begin : tb__scenes
    tb__rnd = 32'd305419896;
    #1;
    clk = 1'b0;
    rst = 1'b0;
    inp = {2{1'b0}};
    // SCENE1 exh
    $display("SCENE1 exh");
    for (tb__i = 0; tb__i < 16; tb__i = tb__i + 1) begin
      {clk, rst, inp} = tb__i[3:0];
      #1;
      $display("  out=%h", out);
    end
    // SCENE2 small
    $display("SCENE2 small");
    repeat (256) begin
      tb__rnd = tb__xs32(tb__rnd);
      clk = tb__rnd[3];
      rst = tb__rnd[10];
      inp = tb__rnd[18:17];
      #1;
      $display("  out=%h", out);
    end
    $display("DONE");
    $finish;
  end
endmodule
