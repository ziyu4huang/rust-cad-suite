// tb_fsm_r0_k1.v — D 腿候選(r0,K=1/1;腿 ID=fsm-r0-k1)
// 撰寫依據=prompt_r0_k1.md(規則書 23 條=transition 5+sequence 18,同步復位 `if (reset)
// state <= 100` 在規則書 R011);場景組織:按「單按鍵事件→狀態機走完一個計時臂→回到
// IDLE」的協議序列定向,覆蓋兩臂(red/green)與全部五個遷移臂。
`timescale 1ns/1ps
// SCENE1: reset_and_idle
// SCENE2: a_arm_full
// SCENE3: b_arm_full
// SCENE4: both_neither
// SCENE5: mid_arm_reset
// SCENE6: random_buttons
// SCENE LIST: reset_and_idle,a_arm_full,b_arm_full,both_neither,mid_arm_reset,random_buttons
// COVERS: 1: fsm_test.R007,fsm_test.R008,fsm_test.R009,fsm_test.R010
// COVERS: 2: fsm_test.R001,fsm_test.R003,fsm_test.R004,fsm_test.R006,fsm_test.R007,fsm_test.R008,fsm_test.R009,fsm_test.R010,fsm_test.R011,fsm_test.R012,fsm_test.R013,fsm_test.R014,fsm_test.R015,fsm_test.R016,fsm_test.R017
// COVERS: 3: fsm_test.R001,fsm_test.R002,fsm_test.R003,fsm_test.R004,fsm_test.R005,fsm_test.R006,fsm_test.R007,fsm_test.R008,fsm_test.R009,fsm_test.R010,fsm_test.R011,fsm_test.R014,fsm_test.R017,fsm_test.R018,fsm_test.R019,fsm_test.R020,fsm_test.R021,fsm_test.R022,fsm_test.R023
// COVERS: 4: fsm_test.R005,fsm_test.R006,fsm_test.R007,fsm_test.R008,fsm_test.R009,fsm_test.R010,fsm_test.R014,fsm_test.R017,fsm_test.R020,fsm_test.R023
// COVERS: 5: fsm_test.R001,fsm_test.R002,fsm_test.R003,fsm_test.R004,fsm_test.R005,fsm_test.R006,fsm_test.R007,fsm_test.R008,fsm_test.R009,fsm_test.R010,fsm_test.R011,fsm_test.R014,fsm_test.R017,fsm_test.R018,fsm_test.R019,fsm_test.R020,fsm_test.R023
// COVERS: 6: fsm_test.R002,fsm_test.R005,fsm_test.R006,fsm_test.R007,fsm_test.R008,fsm_test.R009,fsm_test.R010,fsm_test.R011,fsm_test.R018,fsm_test.R019,fsm_test.R020,fsm_test.R021,fsm_test.R022,fsm_test.R023
module tb_fsm_test;

  reg clk;
  reg reset;
  reg button_a;
  reg button_b;
  wire red_a;
  wire green_a;
  wire red_b;
  wire green_b;

  reg [31:0] tb__rnd;
  integer tb__i;

  fsm_test tb__dut (
    .clk(clk),
    .reset(reset),
    .button_a(button_a),
    .button_b(button_b),
    .red_a(red_a),
    .green_a(green_a),
    .red_b(red_b),
    .green_b(green_b)
  );

  function [31:0] tb__xs32;
    input [31:0] t;
    reg [31:0] w;
    begin
      w = t;
      w = w ^ (w << 13);
      w = w ^ (w >> 17);
      w = w ^ (w << 5);
      tb__xs32 = w;
    end
  endfunction

  initial begin
    #6000;
    $display("WATCHDOG");
    $finish;
  end

  initial tb__rnd = 32'd305419896;
  initial clk = 1'b0;
  always #5 clk = ~clk;

  // 同步釋放復位並觀測 N 拍(輸出 8 信號一行)
  task step_and_print;
    input integer n;
    integer k;
    begin
      for (k = 0; k < n; k = k + 1) begin
        @(negedge clk);
        $display("  ra=%b ga=%b rb=%b gb=%b", red_a, green_a, red_b, green_b);
      end
    end
  endtask

  initial begin : tb__scenes
    tb__rnd = 32'd777;
    #1;
    reset = 1'b1;
    button_a = 1'b0;
    button_b = 1'b0;
    // SCENE1 reset_and_idle:復位 3 拍(IDLE)→釋放後閒置 4 拍
    $display("SCENE1 reset_and_idle");
    @(negedge clk);
    @(negedge clk);
    @(negedge clk);
    reset = 1'b0;
    step_and_print(4);
    // SCENE2 a_arm_full:按 A→200(6 拍)→210(11 拍)→回 IDLE,單按鍵事件全鏈
    $display("SCENE2 a_arm_full");
    @(negedge clk);
    button_a = 1'b1;
    button_b = 1'b0;
    @(negedge clk);
    button_a = 1'b0;
    step_and_print(20);
    // SCENE3 b_arm_full:按 B→300(6 拍)→310(11 拍)→回 IDLE
    $display("SCENE3 b_arm_full");
    @(negedge clk);
    button_b = 1'b1;
    @(negedge clk);
    button_b = 1'b0;
    step_and_print(20);
    // SCENE4 both_neither:兩鍵同按(無遷移)與雙零(無遷移)交錯 8 拍
    $display("SCENE4 both_neither");
    for (tb__i = 0; tb__i < 8; tb__i = tb__i + 1) begin
      @(negedge clk);
      $display("  ra=%b ga=%b rb=%b gb=%b", red_a, green_a, red_b, green_b);
      tb__rnd = tb__xs32(tb__rnd);
      {button_a, button_b} = tb__i[0] ? 2'b11 : 2'b00;
    end
    // SCENE5 mid_arm_reset:計時臂中途復位兩拍再釋放(復位優先於遷移)
    $display("SCENE5 mid_arm_reset");
    @(negedge clk);
    button_a = 1'b1;
    @(negedge clk);
    button_a = 1'b0;
    step_and_print(3);
    reset = 1'b1;
    step_and_print(2);
    reset = 1'b0;
    step_and_print(6);
    // SCENE6 random_buttons:隨機按鍵 16 拍(遷移條件的無序組合)
    $display("SCENE6 random_buttons");
    for (tb__i = 0; tb__i < 16; tb__i = tb__i + 1) begin
      @(negedge clk);
      $display("  ra=%b ga=%b rb=%b gb=%b", red_a, green_a, red_b, green_b);
      tb__rnd = tb__xs32(tb__rnd);
      {button_a, button_b} = tb__rnd[1:0];
    end
    $display("DONE");
    $finish;
  end
endmodule
