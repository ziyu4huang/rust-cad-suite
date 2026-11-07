// tb_aes — template tb_plan-v1(機械生成;T 腿基線;期望由原版錄製,禁手寫/禁加自檢)
// TB-LEG: template tb_plan-v1 seed=305419896 cycles=32 flush=8
// SCENES: 2
// COVERS: 1: *
// COVERS: 2: *

`timescale 1ns/1ps
module tb_aes;

  reg [3:0] addroundkey_data_i;
  reg [3:0] addroundkey_data_reg;
  reg [3:0] addroundkey_round;
  reg [3:0] key_i;
  reg [3:0] keysched_new_key_o;
  reg [3:0] round;
  reg addroundkey_start_i;
  reg keysched_ready_o;
  wire [3:0] keysched_last_key_i;
  wire [3:0] keysched_round_i;
  wire [3:0] next_addroundkey_data_reg;
  wire [3:0] next_addroundkey_round;
  wire [3:0] round_data_var;
  wire keysched_start_i;
  wire next_addroundkey_ready_o;

  reg [31:0] tb__rnd;
  integer tb__i;

  aes tb__dut (
    .addroundkey_data_i(addroundkey_data_i),
    .addroundkey_data_reg(addroundkey_data_reg),
    .addroundkey_round(addroundkey_round),
    .key_i(key_i),
    .keysched_new_key_o(keysched_new_key_o),
    .round(round),
    .addroundkey_start_i(addroundkey_start_i),
    .keysched_ready_o(keysched_ready_o),
    .keysched_last_key_i(keysched_last_key_i),
    .keysched_round_i(keysched_round_i),
    .next_addroundkey_data_reg(next_addroundkey_data_reg),
    .next_addroundkey_round(next_addroundkey_round),
    .round_data_var(round_data_var),
    .keysched_start_i(keysched_start_i),
    .next_addroundkey_ready_o(next_addroundkey_ready_o)
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
    addroundkey_data_i = {4{1'b0}};
    addroundkey_data_reg = {4{1'b0}};
    addroundkey_round = {4{1'b0}};
    key_i = {4{1'b0}};
    keysched_new_key_o = {4{1'b0}};
    round = {4{1'b0}};
    addroundkey_start_i = 1'b0;
    keysched_ready_o = 1'b0;
    // SCENE1 rand
    $display("SCENE1 rand");
    repeat (256) begin
      tb__rnd = tb__xs32(tb__rnd);
      addroundkey_data_i = tb__rnd[6:3];
      addroundkey_data_reg = tb__rnd[13:10];
      addroundkey_round = tb__rnd[20:17];
      key_i = tb__rnd[3:0];
      keysched_new_key_o = tb__rnd[10:7];
      round = tb__rnd[17:14];
      addroundkey_start_i = tb__rnd[21];
      keysched_ready_o = tb__rnd[4];
      #1;
      $display("  keysched_last_key_i=%h", keysched_last_key_i);
      $display("  keysched_round_i=%h", keysched_round_i);
      $display("  next_addroundkey_data_reg=%h", next_addroundkey_data_reg);
      $display("  next_addroundkey_round=%h", next_addroundkey_round);
      $display("  round_data_var=%h", round_data_var);
      $display("  keysched_start_i=%h", keysched_start_i);
      $display("  next_addroundkey_ready_o=%h", next_addroundkey_ready_o);
    end
    // SCENE2 small
    $display("SCENE2 small");
    repeat (256) begin
      tb__rnd = tb__xs32(tb__rnd);
      addroundkey_data_i = {{1{1'b0}}, tb__rnd[5:3]};
      addroundkey_data_reg = {{1{1'b0}}, tb__rnd[12:10]};
      addroundkey_round = {{1{1'b0}}, tb__rnd[19:17]};
      key_i = {{1{1'b0}}, tb__rnd[2:0]};
      keysched_new_key_o = {{1{1'b0}}, tb__rnd[9:7]};
      round = {{1{1'b0}}, tb__rnd[16:14]};
      addroundkey_start_i = tb__rnd[21];
      keysched_ready_o = tb__rnd[4];
      #1;
      $display("  keysched_last_key_i=%h", keysched_last_key_i);
      $display("  keysched_round_i=%h", keysched_round_i);
      $display("  next_addroundkey_data_reg=%h", next_addroundkey_data_reg);
      $display("  next_addroundkey_round=%h", next_addroundkey_round);
      $display("  round_data_var=%h", round_data_var);
      $display("  keysched_start_i=%h", keysched_start_i);
      $display("  next_addroundkey_ready_o=%h", next_addroundkey_ready_o);
    end
    $display("DONE");
    $finish;
  end
endmodule
