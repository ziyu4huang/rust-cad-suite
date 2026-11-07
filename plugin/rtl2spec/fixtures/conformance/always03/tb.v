// tb_uut_always03 — template tb_plan-v1(機械生成;T 腿基線;期望由原版錄製,禁手寫/禁加自檢)
// TB-LEG: template tb_plan-v1 seed=305419896 cycles=32 flush=8
// SCENES: 4
// COVERS: 1: *
// COVERS: 2: *
// COVERS: 3: *
// COVERS: 4: *

`timescale 1ns/1ps
module tb_uut_always03;

  reg clock;
  reg in1;
  reg in2;
  reg in3;
  reg in4;
  reg in5;
  reg in6;
  reg in7;
  wire out1;
  wire out2;
  wire out3;

  reg [31:0] tb__rnd;
  integer tb__i;

  uut_always03 tb__dut (
    .clock(clock),
    .in1(in1),
    .in2(in2),
    .in3(in3),
    .in4(in4),
    .in5(in5),
    .in6(in6),
    .in7(in7),
    .out1(out1),
    .out2(out2),
    .out3(out3)
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
    in1 = 1'b0;
    in2 = 1'b0;
    in3 = 1'b0;
    in4 = 1'b0;
    in5 = 1'b0;
    in6 = 1'b0;
    in7 = 1'b0;
    // SCENE1 flush
    $display("SCENE1 flush");
    repeat (8) begin
      @(negedge clock);
      tb__rnd = tb__xs32(tb__rnd);
      in1 = tb__rnd[3];
      in2 = tb__rnd[10];
      in3 = tb__rnd[17];
      in4 = tb__rnd[0];
      in5 = tb__rnd[7];
      in6 = tb__rnd[14];
      in7 = tb__rnd[21];
    end
    // SCENE2 full
    $display("SCENE2 full");
    repeat (33) begin
      @(negedge clock);
      $display("  out1=%h", out1);
      $display("  out2=%h", out2);
      $display("  out3=%h", out3);
      tb__rnd = tb__xs32(tb__rnd);
      in1 = tb__rnd[3];
      in2 = tb__rnd[10];
      in3 = tb__rnd[17];
      in4 = tb__rnd[0];
      in5 = tb__rnd[7];
      in6 = tb__rnd[14];
      in7 = tb__rnd[21];
    end
    // SCENE3 small
    $display("SCENE3 small");
    repeat (33) begin
      @(negedge clock);
      $display("  out1=%h", out1);
      $display("  out2=%h", out2);
      $display("  out3=%h", out3);
      tb__rnd = tb__xs32(tb__rnd);
      in1 = tb__rnd[3];
      in2 = tb__rnd[10];
      in3 = tb__rnd[17];
      in4 = tb__rnd[0];
      in5 = tb__rnd[7];
      in6 = tb__rnd[14];
      in7 = tb__rnd[21];
    end
    // SCENE4 hold
    $display("SCENE4 hold");
    for (tb__i = 0; tb__i < 32; tb__i = tb__i + 1) begin
      if (tb__i % 8 == 0) begin
        tb__rnd = tb__xs32(tb__rnd);
        in1 = tb__rnd[3];
        in2 = tb__rnd[10];
        in3 = tb__rnd[17];
        in4 = tb__rnd[0];
        in5 = tb__rnd[7];
        in6 = tb__rnd[14];
        in7 = tb__rnd[21];
      end
      @(negedge clock);
      $display("  out1=%h", out1);
      $display("  out2=%h", out2);
      $display("  out3=%h", out3);
    end
    $display("DONE");
    $finish;
  end
endmodule
