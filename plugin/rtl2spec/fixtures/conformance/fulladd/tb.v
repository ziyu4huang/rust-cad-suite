// tb_halfadd — template tb_plan-v1(機械生成;T 腿基線;期望由原版錄製,禁手寫/禁加自檢)
// TB-LEG: template tb_plan-v1 seed=305419896 cycles=32 flush=8
// SCENES: 2
// COVERS: 1: *
// COVERS: 2: *

`timescale 1ns/1ps
module tb_halfadd;

  reg a;
  reg b;
  wire s;
  wire c;

  reg [31:0] tb__rnd;
  integer tb__i;

  halfadd tb__dut (
    .a(a),
    .b(b),
    .s(s),
    .c(c)
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
    #1520;
    $display("WATCHDOG");
    $finish;
  end

  initial begin : tb__scenes
    tb__rnd = 32'd305419896;
    #1;
    a = 1'b0;
    b = 1'b0;
    // SCENE1 exh
    $display("SCENE1 exh");
    for (tb__i = 0; tb__i < 4; tb__i = tb__i + 1) begin
      {a, b} = tb__i[1:0];
      #1;
      $display("  s=%h", s);
      $display("  c=%h", c);
    end
    // SCENE2 small
    $display("SCENE2 small");
    repeat (256) begin
      tb__rnd = tb__xs32(tb__rnd);
      a = tb__rnd[3];
      b = tb__rnd[10];
      #1;
      $display("  s=%h", s);
      $display("  c=%h", c);
    end
    $display("DONE");
    $finish;
  end
endmodule
