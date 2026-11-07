// tb_subbytes_00 — template tb_plan-v1(機械生成;T 腿基線;期望由原版錄製,禁手寫/禁加自檢)
// TB-LEG: template tb_plan-v1 seed=305419896 cycles=32 flush=8
// SCENES: 4
// COVERS: 1: subbytes_00.R001,subbytes_00.R002,subbytes_00.R003
// COVERS: 2: *
// COVERS: 3: *
// COVERS: 4: *

`timescale 1ns/1ps
module tb_subbytes_00;

  reg clk;
  reg reset;
  reg start_i;
  reg decrypt_i;
  reg [31:0] data_i;
  wire ready_o;
  wire [31:0] data_o;
  wire [7:0] sbox_data_o;
  reg [7:0] sbox_data_i;
  wire sbox_decrypt_o;

  reg [31:0] tb__rnd;
  integer tb__i;

  subbytes_00 tb__dut (
    .clk(clk),
    .reset(reset),
    .start_i(start_i),
    .decrypt_i(decrypt_i),
    .data_i(data_i),
    .ready_o(ready_o),
    .data_o(data_o),
    .sbox_data_o(sbox_data_o),
    .sbox_data_i(sbox_data_i),
    .sbox_decrypt_o(sbox_decrypt_o)
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
    reset = 1'b1;
    start_i = 1'b0;
    decrypt_i = 1'b0;
    data_i = {32{1'b0}};
    sbox_data_i = {8{1'b0}};
    // SCENE1 arst_reset
    $display("SCENE1 arst_reset");
    @(negedge clk);
    #2;
    reset = 1'b0;
    #1;
    $display("  ready_o=%h", ready_o);
    $display("  data_o=%h", data_o);
    $display("  sbox_data_o=%h", sbox_data_o);
    $display("  sbox_decrypt_o=%h", sbox_decrypt_o);
    @(negedge clk);
    reset = 1'b1;
    repeat (4) begin
      @(negedge clk);
      $display("  ready_o=%h", ready_o);
      $display("  data_o=%h", data_o);
      $display("  sbox_data_o=%h", sbox_data_o);
      $display("  sbox_decrypt_o=%h", sbox_decrypt_o);
      tb__rnd = tb__xs32(tb__rnd);
      start_i = tb__rnd[3];
      decrypt_i = tb__rnd[10];
      data_i = tb__rnd[31:0];
      sbox_data_i = tb__rnd[7:0];
    end
    // SCENE2 full
    $display("SCENE2 full");
    repeat (33) begin
      @(negedge clk);
      $display("  ready_o=%h", ready_o);
      $display("  data_o=%h", data_o);
      $display("  sbox_data_o=%h", sbox_data_o);
      $display("  sbox_decrypt_o=%h", sbox_decrypt_o);
      tb__rnd = tb__xs32(tb__rnd);
      start_i = tb__rnd[3];
      decrypt_i = tb__rnd[10];
      data_i = tb__rnd[31:0];
      sbox_data_i = tb__rnd[7:0];
    end
    // SCENE3 small
    $display("SCENE3 small");
    repeat (33) begin
      @(negedge clk);
      $display("  ready_o=%h", ready_o);
      $display("  data_o=%h", data_o);
      $display("  sbox_data_o=%h", sbox_data_o);
      $display("  sbox_decrypt_o=%h", sbox_decrypt_o);
      tb__rnd = tb__xs32(tb__rnd);
      start_i = tb__rnd[3];
      decrypt_i = tb__rnd[10];
      data_i = {{29{1'b0}}, tb__rnd[19:17]};
      sbox_data_i = {{5{1'b0}}, tb__rnd[2:0]};
    end
    // SCENE4 hold
    $display("SCENE4 hold");
    for (tb__i = 0; tb__i < 32; tb__i = tb__i + 1) begin
      if (tb__i % 8 == 0) begin
        tb__rnd = tb__xs32(tb__rnd);
        start_i = tb__rnd[3];
        decrypt_i = tb__rnd[10];
        data_i = tb__rnd[31:0];
        sbox_data_i = tb__rnd[7:0];
      end
      @(negedge clk);
      $display("  ready_o=%h", ready_o);
      $display("  data_o=%h", data_o);
      $display("  sbox_data_o=%h", sbox_data_o);
      $display("  sbox_decrypt_o=%h", sbox_decrypt_o);
    end
    $display("DONE");
    $finish;
  end
endmodule
