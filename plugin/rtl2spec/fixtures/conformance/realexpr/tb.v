// tb_demo_001 — template tb_plan-v1(機械生成;T 腿基線;期望由原版錄製,禁手寫/禁加自檢)
// TB-LEG: template tb_plan-v1 seed=305419896 cycles=32 flush=8
// SCENES: 2
// COVERS: 1: *
// COVERS: 2: *

`timescale 1ns/1ps
module tb_demo_001;

  wire [7:0] y1;
  wire [7:0] y2;
  wire [7:0] y3;
  wire [7:0] y4;

  reg [31:0] tb__rnd;
  integer tb__i;

  demo_001 tb__dut (
    .y1(y1),
    .y2(y2),
    .y3(y3),
    .y4(y4)
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
    // SCENE1 rand
    $display("SCENE1 rand");
    repeat (256) begin
      tb__rnd = tb__xs32(tb__rnd);
      #1;
      $display("  y1=%h", y1);
      $display("  y2=%h", y2);
      $display("  y3=%h", y3);
      $display("  y4=%h", y4);
    end
    // SCENE2 small
    $display("SCENE2 small");
    repeat (256) begin
      tb__rnd = tb__xs32(tb__rnd);
      #1;
      $display("  y1=%h", y1);
      $display("  y2=%h", y2);
      $display("  y3=%h", y3);
      $display("  y4=%h", y4);
    end
    $display("DONE");
    $finish;
  end
endmodule
