// tb_sincos_r0_k1.v — D 腿候選(r0,K=1/1;腿 ID=sincos-r0-k1)
// 撰寫依據=prompt_r0_k1.md(規則書 46 條:guard 8+sequence 38;start/done 握手、
// posedge reset 高有效、帶號算術)。場景組織:start/done 握手協議定向——
// 角度取象限掃描(0/±1/±最大/隨機),每個 start 後按 done 握手(帶上限)逐拍
// 觀測;單拍觀測窗口抓 R 系列的 19 步迭代序列。
`timescale 1ns/1ps
// SCENE1: reset_and_idle
// SCENE2: quadrant_scan
// SCENE3: back_to_back
// SCENE4: random_stream
// SCENE5: mid_compute_reset
// SCENE LIST: reset_and_idle,quadrant_scan,back_to_back,random_stream,mid_compute_reset,async_reset_observe
// COVERS: 1: d.R002,d.R003,d.R004
// COVERS: 2: d.R001,d.R002,d.R003,d.R009,d.R010,d.R011,d.R012,d.R013,d.R014,d.R015,d.R016,d.R017,d.R018,d.R019,d.R020,d.R021,d.R022,d.R023,d.R024,d.R025,d.R026,d.R027,d.R028,d.R029,d.R030,d.R031,d.R032,d.R033,d.R036,d.R037,d.R038,d.R039,d.R040,d.R041,d.R042,d.R043,d.R044,d.R045,d.R046
// COVERS: 3: d.R009,d.R010,d.R011,d.R012,d.R013,d.R014,d.R015,d.R016,d.R017,d.R018,d.R019,d.R020,d.R021,d.R022,d.R023,d.R024,d.R025,d.R026,d.R027,d.R028,d.R029,d.R030,d.R031,d.R032,d.R033,d.R034,d.R036,d.R037,d.R038,d.R039,d.R040,d.R041,d.R042,d.R043,d.R044,d.R045,d.R046
// COVERS: 4: d.R009,d.R010,d.R011,d.R012,d.R013,d.R014,d.R015,d.R016,d.R017,d.R018,d.R019,d.R020,d.R021,d.R022,d.R023,d.R024,d.R025,d.R026,d.R027,d.R028,d.R029,d.R030,d.R031,d.R032,d.R033,d.R034,d.R036,d.R037,d.R038,d.R039,d.R040,d.R041,d.R042,d.R043,d.R044,d.R045,d.R046
// COVERS: 5: d.R002,d.R003,d.R004,d.R014,d.R045,d.R046
// COVERS: 6: d.R001,d.R002,d.R003,d.R004,d.R005,d.R006,d.R007,d.R008,d.R009,d.R010,d.R011,d.R012,d.R013,d.R014,d.R015,d.R016,d.R017,d.R018,d.R019,d.R020,d.R021,d.R022,d.R023,d.R024,d.R025,d.R026,d.R027,d.R028,d.R029,d.R030,d.R031,d.R032,d.R036,d.R037,d.R038,d.R039,d.R040,d.R041,d.R042,d.R043,d.R045,d.R046
// WAIVE: d.R035 unadjudicated
module tb_d;

  reg clock;
  reg reset;
  reg start;
  reg signed [19:0] z0;
  wire signed [19:0] cos_z0;
  wire signed [19:0] sin_z0;
  wire done;

  reg [31:0] tb__rnd;
  integer tb__i;
  integer tb__w;

  d tb__dut (
    .clock(clock),
    .reset(reset),
    .start(start),
    .z0(z0),
    .cos_z0(cos_z0),
    .sin_z0(sin_z0),
    .done(done)
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
    #4000;
    $display("WATCHDOG");
    $finish;
  end

  initial tb__rnd = 32'd90210;
  initial clock = 1'b0;
  always #5 clock = ~clock;

  task run_angle;
    input signed [19:0] ang;
    integer k;
    begin
      @(negedge clock);
      z0 = ang;
      start = 1'b1;
      @(negedge clock);
      start = 1'b0;
      for (k = 0; k < 24; k = k + 1) begin
        @(negedge clock);
        $display("  cos=%0d sin=%0d done=%b", cos_z0, sin_z0, done);
        if (done) begin
          // done 後再觀測 2 拍(state 回 IDLE 的握手尾巴)
          @(negedge clock);
          $display("  cos=%0d sin=%0d done=%b", cos_z0, sin_z0, done);
          @(negedge clock);
          $display("  cos=%0d sin=%0d done=%b", cos_z0, sin_z0, done);
          k = 24;
        end
      end
    end
  endtask

  initial begin : tb__scenes
    tb__rnd = 32'd424242;
    #1;
    reset = 1'b1;
    start = 1'b0;
    z0 = 20'sd0;
    // SCENE1 reset_and_idle:復位 2 拍→釋放閒置 3 拍(done=0)
    $display("SCENE1 reset_and_idle");
    @(negedge clock);
    @(negedge clock);
    reset = 1'b0;
    @(negedge clock);
    $display("  cos=%0d sin=%0d done=%b", cos_z0, sin_z0, done);
    @(negedge clock);
    $display("  cos=%0d sin=%0d done=%b", cos_z0, sin_z0, done);
    @(negedge clock);
    $display("  cos=%0d sin=%0d done=%b", cos_z0, sin_z0, done);
    // SCENE2 quadrant_scan:0/1/-1 最大/最小/隨機 六角(start→done 握手全鏈)
    $display("SCENE2 quadrant_scan");
    run_angle(20'sd0);
    run_angle(20'sd1);
    run_angle(-20'sd1);
    run_angle(20'sd524287);
    run_angle(-20'sd524288);
    tb__rnd = tb__xs32(tb__rnd);
    run_angle(tb__rnd[19:0]);
    // SCENE3 back_to_back:start 拉高跨 done(連續運算;start 於 IDLE 每拍生效)
    $display("SCENE3 back_to_back");
    for (tb__i = 0; tb__i < 3; tb__i = tb__i + 1) begin
      @(negedge clock);
      z0 = 20'sd100 + tb__i[19:0];
      start = 1'b1;
      @(negedge clock);
      start = 1'b0;
      tb__w = 0;
      while (!done && tb__w < 24) begin
        @(negedge clock);
        $display("  cos=%0d sin=%0d done=%b", cos_z0, sin_z0, done);
        tb__w = tb__w + 1;
      end
    end
    // SCENE4 random_stream:連續 6 個隨機角(值域掃描)
    $display("SCENE4 random_stream");
    for (tb__i = 0; tb__i < 6; tb__i = tb__i + 1) begin
      tb__rnd = tb__xs32(tb__rnd);
      run_angle(tb__rnd[19:0]);
    end
    // SCENE5 mid_compute_reset:運算中途 posedge reset(異步復位優先)
    $display("SCENE5 mid_compute_reset");
    @(negedge clock);
    z0 = 20'sd300000;
    start = 1'b1;
    @(negedge clock);
    start = 1'b0;
    @(negedge clock);
    reset = 1'b1;
    @(negedge clock);
    reset = 1'b0;
    @(negedge clock);
    $display("  cos=%0d sin=%0d done=%b", cos_z0, sin_z0, done);
    @(negedge clock);
    $display("  cos=%0d sin=%0d done=%b", cos_z0, sin_z0, done);
    // SCENE6 async_reset_observe:兩沿之間 negedge 後 #2 拉起 posedge reset、#1 後立即觀測
    // (異步復位即時生效;async→sync 變異要到下一 posedge 才動作,觀測點值不同)
    $display("SCENE6 async_reset_observe");
    run_angle(20'sd12345);
    @(negedge clock);
    #2 reset = 1'b1;
    #1;
    $display("  cos=%0d sin=%0d done=%b", cos_z0, sin_z0, done);
    @(negedge clock);
    reset = 1'b0;
    @(negedge clock);
    $display("  cos=%0d sin=%0d done=%b", cos_z0, sin_z0, done);
    @(negedge clock);
    $display("  cos=%0d sin=%0d done=%b", cos_z0, sin_z0, done);
    $display("DONE");
    $finish;
  end
endmodule
