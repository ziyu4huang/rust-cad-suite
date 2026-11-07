// tb_partsel_test001 — template tb_plan-v1(機械生成;T 腿基線;期望由原版錄製,禁手寫/禁加自檢)
// TB-LEG: template tb_plan-v1 seed=305419896 cycles=32 flush=8
// SCENES: 2
// COVERS: 1: *
// COVERS: 2: *

`timescale 1ns/1ps
module tb_partsel_test001;

  reg [2:0] idx;
  reg [31:0] data;
  wire [3:0] slice_up;
  wire [3:0] slice_down;

  reg [31:0] tb__rnd;
  integer tb__i;

  partsel_test001 tb__dut (
    .idx(idx),
    .data(data),
    .slice_up(slice_up),
    .slice_down(slice_down)
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
    idx = {3{1'b0}};
    data = {32{1'b0}};
    // SCENE1 rand
    $display("SCENE1 rand");
    repeat (256) begin
      tb__rnd = tb__xs32(tb__rnd);
      idx = tb__rnd[5:3];
      data = tb__rnd[31:0];
      #1;
      $display("  slice_up=%h", slice_up);
      $display("  slice_down=%h", slice_down);
    end
    // SCENE2 small
    $display("SCENE2 small");
    repeat (256) begin
      tb__rnd = tb__xs32(tb__rnd);
      idx = tb__rnd[5:3];
      data = {{29{1'b0}}, tb__rnd[12:10]};
      #1;
      $display("  slice_up=%h", slice_up);
      $display("  slice_down=%h", slice_down);
    end
    $display("DONE");
    $finish;
  end
endmodule
