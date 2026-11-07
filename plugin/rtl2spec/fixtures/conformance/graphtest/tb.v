// tb_graphtest — template tb_plan-v1(機械生成;T 腿基線;期望由原版錄製,禁手寫/禁加自檢)
// TB-LEG: template tb_plan-v1 seed=305419896 cycles=32 flush=8
// SCENES: 2
// COVERS: 1: *
// COVERS: 2: *

`timescale 1ns/1ps
module tb_graphtest;

  reg [3:0] A;
  reg [3:0] B;
  wire [3:0] X;
  wire [9:0] Y;
  wire [7:0] Z;

  reg [31:0] tb__rnd;
  integer tb__i;

  graphtest tb__dut (
    .A(A),
    .B(B),
    .X(X),
    .Y(Y),
    .Z(Z)
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
    A = {4{1'b0}};
    B = {4{1'b0}};
    // SCENE1 exh
    $display("SCENE1 exh");
    for (tb__i = 0; tb__i < 256; tb__i = tb__i + 1) begin
      {A, B} = tb__i[7:0];
      #1;
      $display("  X=%h", X);
      $display("  Y=%h", Y);
      $display("  Z=%h", Z);
    end
    // SCENE2 small
    $display("SCENE2 small");
    repeat (256) begin
      tb__rnd = tb__xs32(tb__rnd);
      A = {{1{1'b0}}, tb__rnd[5:3]};
      B = {{1{1'b0}}, tb__rnd[12:10]};
      #1;
      $display("  X=%h", X);
      $display("  Y=%h", Y);
      $display("  Z=%h", Z);
    end
    $display("DONE");
    $finish;
  end
endmodule
