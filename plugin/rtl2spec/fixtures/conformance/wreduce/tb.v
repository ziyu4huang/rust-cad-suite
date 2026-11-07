// tb_wreduce_test0 — template tb_plan-v1(機械生成;T 腿基線;期望由原版錄製,禁手寫/禁加自檢)
// TB-LEG: template tb_plan-v1 seed=305419896 cycles=32 flush=8
// SCENES: 2
// COVERS: 1: *
// COVERS: 2: *

`timescale 1ns/1ps
module tb_wreduce_test0;

  reg [7:0] a;
  reg [7:0] b;
  wire [15:0] x;
  wire [15:0] y;
  wire [15:0] z;

  reg [31:0] tb__rnd;
  integer tb__i;

  wreduce_test0 tb__dut (
    .a(a),
    .b(b),
    .x(x),
    .y(y),
    .z(z)
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
    #2024;
    $display("WATCHDOG");
    $finish;
  end

  initial begin : tb__scenes
    tb__rnd = 32'd305419896;
    #1;
    a = {8{1'b0}};
    b = {8{1'b0}};
    // SCENE1 rand
    $display("SCENE1 rand");
    repeat (256) begin
      tb__rnd = tb__xs32(tb__rnd);
      a = tb__rnd[10:3];
      b = tb__rnd[17:10];
      #1;
      $display("  x=%h", x);
      $display("  y=%h", y);
      $display("  z=%h", z);
    end
    // SCENE2 small
    $display("SCENE2 small");
    repeat (256) begin
      tb__rnd = tb__xs32(tb__rnd);
      a = {{5{1'b0}}, tb__rnd[5:3]};
      b = {{5{1'b0}}, tb__rnd[12:10]};
      #1;
      $display("  x=%h", x);
      $display("  y=%h", y);
      $display("  z=%h", z);
    end
    $display("DONE");
    $finish;
  end
endmodule
