// tb_aes_key_expand_128 — template tb_plan-v1(機械生成;T 腿基線;期望由原版錄製,禁手寫/禁加自檢)
// TB-LEG: template tb_plan-v1 seed=305419896 cycles=32 flush=8
// SCENES: 4
// COVERS: 1: *
// COVERS: 2: *
// COVERS: 3: *
// COVERS: 4: *

`timescale 1ns/1ps
module tb_aes_key_expand_128;

  reg clk;
  reg kld;
  reg [15:0] key;
  wire [3:0] wo_0;
  wire [3:0] wo_1;
  wire [3:0] wo_2;
  wire [3:0] wo_3;

  reg [31:0] tb__rnd;
  integer tb__i;

  aes_key_expand_128 tb__dut (
    .clk(clk),
    .kld(kld),
    .key(key),
    .wo_0(wo_0),
    .wo_1(wo_1),
    .wo_2(wo_2),
    .wo_3(wo_3)
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
    kld = 1'b0;
    key = {16{1'b0}};
    // SCENE1 flush
    $display("SCENE1 flush");
    repeat (8) begin
      @(negedge clk);
      tb__rnd = tb__xs32(tb__rnd);
      kld = tb__rnd[3];
      key = tb__rnd[25:10];
    end
    // SCENE2 full
    $display("SCENE2 full");
    repeat (33) begin
      @(negedge clk);
      $display("  wo_0=%h", wo_0);
      $display("  wo_1=%h", wo_1);
      $display("  wo_2=%h", wo_2);
      $display("  wo_3=%h", wo_3);
      tb__rnd = tb__xs32(tb__rnd);
      kld = tb__rnd[3];
      key = tb__rnd[25:10];
    end
    // SCENE3 small
    $display("SCENE3 small");
    repeat (33) begin
      @(negedge clk);
      $display("  wo_0=%h", wo_0);
      $display("  wo_1=%h", wo_1);
      $display("  wo_2=%h", wo_2);
      $display("  wo_3=%h", wo_3);
      tb__rnd = tb__xs32(tb__rnd);
      kld = tb__rnd[3];
      key = {{13{1'b0}}, tb__rnd[12:10]};
    end
    // SCENE4 hold
    $display("SCENE4 hold");
    for (tb__i = 0; tb__i < 32; tb__i = tb__i + 1) begin
      if (tb__i % 8 == 0) begin
        tb__rnd = tb__xs32(tb__rnd);
        kld = tb__rnd[3];
        key = tb__rnd[25:10];
      end
      @(negedge clk);
      $display("  wo_0=%h", wo_0);
      $display("  wo_1=%h", wo_1);
      $display("  wo_2=%h", wo_2);
      $display("  wo_3=%h", wo_3);
    end
    $display("DONE");
    $finish;
  end
endmodule
