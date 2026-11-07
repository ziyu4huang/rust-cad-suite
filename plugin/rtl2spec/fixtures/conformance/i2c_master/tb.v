// tb_i2c_r0_k1.v — D 腿候選(r0,K=1/1;腿 ID=i2c-r0-k1)
// 撰寫依據=prompt_r0_k1.md(規則書 152 條:guard 20+transition 9+sequence 123)。
// 場景組織:按「命令序列」定向——START/WR/STOP、START/RD(ACK/NACK 由 MST_ACK_IN)、
// 重複 START、復位中斷;prescale 全寬掃描(速控逐拍脈衝)與命令字解碼逐位;
// scl_i/sda_i 保持(規則書注:v1 未用)。
`timescale 1ns/1ps
// SCENE1: reset_and_idle
// SCENE2: wr_byte_seq
// SCENE3: rd_byte_seq
// SCENE4: repeated_start
// SCENE5: prescale_sweep
// SCENE6: mid_transfer_reset
// SCENE LIST: reset_and_idle,wr_byte_seq,rd_byte_seq,repeated_start,prescale_sweep,mid_transfer_reset
// COVERS: 1: i2c_master.R008,i2c_master.R019,i2c_master.R020,i2c_master.R030,i2c_master.R036,i2c_master.R037,i2c_master.R038,i2c_master.R039,i2c_master.R040,i2c_master.R041
// COVERS: 2: i2c_master.R001,i2c_master.R002,i2c_master.R003,i2c_master.R004,i2c_master.R005,i2c_master.R006,i2c_master.R007,i2c_master.R008,i2c_master.R009,i2c_master.R010,i2c_master.R011,i2c_master.R012,i2c_master.R013,i2c_master.R014,i2c_master.R015,i2c_master.R016,i2c_master.R017,i2c_master.R018,i2c_master.R019,i2c_master.R020,i2c_master.R021,i2c_master.R022,i2c_master.R023,i2c_master.R024,i2c_master.R027,i2c_master.R029,i2c_master.R030,i2c_master.R034,i2c_master.R035,i2c_master.R036,i2c_master.R037,i2c_master.R038,i2c_master.R039,i2c_master.R040,i2c_master.R041,i2c_master.R046,i2c_master.R047,i2c_master.R048,i2c_master.R049,i2c_master.R050,i2c_master.R051,i2c_master.R052,i2c_master.R053,i2c_master.R054,i2c_master.R055,i2c_master.R056,i2c_master.R057,i2c_master.R058,i2c_master.R059,i2c_master.R060,i2c_master.R061,i2c_master.R062,i2c_master.R063,i2c_master.R064,i2c_master.R065,i2c_master.R067,i2c_master.R068,i2c_master.R069,i2c_master.R070,i2c_master.R071,i2c_master.R072,i2c_master.R073,i2c_master.R076,i2c_master.R077,i2c_master.R078,i2c_master.R079,i2c_master.R080,i2c_master.R081,i2c_master.R082,i2c_master.R083,i2c_master.R084,i2c_master.R085,i2c_master.R088,i2c_master.R089,i2c_master.R090,i2c_master.R091,i2c_master.R094,i2c_master.R095,i2c_master.R096,i2c_master.R097,i2c_master.R098,i2c_master.R099,i2c_master.R102,i2c_master.R103,i2c_master.R107,i2c_master.R108,i2c_master.R109,i2c_master.R110,i2c_master.R111,i2c_master.R112,i2c_master.R113,i2c_master.R128,i2c_master.R129,i2c_master.R135,i2c_master.R136,i2c_master.R137,i2c_master.R138,i2c_master.R139,i2c_master.R140,i2c_master.R141,i2c_master.R142,i2c_master.R143,i2c_master.R144,i2c_master.R145,i2c_master.R146,i2c_master.R147,i2c_master.R148,i2c_master.R150,i2c_master.R151,i2c_master.R152
// COVERS: 3: i2c_master.R001,i2c_master.R002,i2c_master.R003,i2c_master.R004,i2c_master.R005,i2c_master.R006,i2c_master.R007,i2c_master.R008,i2c_master.R009,i2c_master.R010,i2c_master.R011,i2c_master.R012,i2c_master.R013,i2c_master.R014,i2c_master.R015,i2c_master.R016,i2c_master.R017,i2c_master.R018,i2c_master.R019,i2c_master.R020,i2c_master.R021,i2c_master.R022,i2c_master.R023,i2c_master.R024,i2c_master.R025,i2c_master.R026,i2c_master.R027,i2c_master.R028,i2c_master.R029,i2c_master.R030,i2c_master.R031,i2c_master.R032,i2c_master.R033,i2c_master.R034,i2c_master.R035,i2c_master.R036,i2c_master.R037,i2c_master.R038,i2c_master.R039,i2c_master.R041,i2c_master.R046,i2c_master.R047,i2c_master.R048,i2c_master.R049,i2c_master.R050,i2c_master.R051,i2c_master.R052,i2c_master.R053,i2c_master.R054,i2c_master.R055,i2c_master.R056,i2c_master.R057,i2c_master.R058,i2c_master.R059,i2c_master.R060,i2c_master.R061,i2c_master.R062,i2c_master.R063,i2c_master.R064,i2c_master.R065,i2c_master.R067,i2c_master.R068,i2c_master.R069,i2c_master.R070,i2c_master.R071,i2c_master.R072,i2c_master.R073,i2c_master.R074,i2c_master.R075,i2c_master.R076,i2c_master.R077,i2c_master.R078,i2c_master.R079,i2c_master.R080,i2c_master.R081,i2c_master.R082,i2c_master.R083,i2c_master.R084,i2c_master.R085,i2c_master.R086,i2c_master.R088,i2c_master.R089,i2c_master.R090,i2c_master.R091,i2c_master.R092,i2c_master.R093,i2c_master.R094,i2c_master.R095,i2c_master.R096,i2c_master.R097,i2c_master.R098,i2c_master.R099,i2c_master.R102,i2c_master.R103,i2c_master.R105,i2c_master.R106,i2c_master.R107,i2c_master.R108,i2c_master.R109,i2c_master.R110,i2c_master.R111,i2c_master.R112,i2c_master.R113,i2c_master.R114,i2c_master.R115,i2c_master.R116,i2c_master.R117,i2c_master.R118,i2c_master.R119,i2c_master.R121,i2c_master.R123,i2c_master.R124,i2c_master.R125,i2c_master.R126,i2c_master.R139,i2c_master.R141,i2c_master.R143,i2c_master.R145,i2c_master.R148,i2c_master.R150
// COVERS: 4: i2c_master.R001,i2c_master.R002,i2c_master.R003,i2c_master.R004,i2c_master.R005,i2c_master.R006,i2c_master.R007,i2c_master.R008,i2c_master.R009,i2c_master.R010,i2c_master.R011,i2c_master.R012,i2c_master.R013,i2c_master.R014,i2c_master.R015,i2c_master.R016,i2c_master.R017,i2c_master.R018,i2c_master.R019,i2c_master.R020,i2c_master.R021,i2c_master.R022,i2c_master.R023,i2c_master.R024,i2c_master.R025,i2c_master.R026,i2c_master.R027,i2c_master.R028,i2c_master.R029,i2c_master.R030,i2c_master.R031,i2c_master.R032,i2c_master.R033,i2c_master.R034,i2c_master.R035,i2c_master.R036,i2c_master.R037,i2c_master.R038,i2c_master.R039,i2c_master.R041,i2c_master.R046,i2c_master.R047,i2c_master.R048,i2c_master.R049,i2c_master.R050,i2c_master.R051,i2c_master.R052,i2c_master.R053,i2c_master.R054,i2c_master.R055,i2c_master.R056,i2c_master.R057,i2c_master.R058,i2c_master.R059,i2c_master.R060,i2c_master.R061,i2c_master.R062,i2c_master.R063,i2c_master.R064,i2c_master.R065,i2c_master.R067,i2c_master.R068,i2c_master.R069,i2c_master.R070,i2c_master.R071,i2c_master.R072,i2c_master.R073,i2c_master.R074,i2c_master.R075,i2c_master.R076,i2c_master.R077,i2c_master.R078,i2c_master.R079,i2c_master.R080,i2c_master.R081,i2c_master.R082,i2c_master.R083,i2c_master.R084,i2c_master.R085,i2c_master.R086,i2c_master.R088,i2c_master.R089,i2c_master.R090,i2c_master.R091,i2c_master.R092,i2c_master.R093,i2c_master.R094,i2c_master.R095,i2c_master.R096,i2c_master.R097,i2c_master.R098,i2c_master.R099,i2c_master.R102,i2c_master.R103,i2c_master.R105,i2c_master.R106,i2c_master.R107,i2c_master.R108,i2c_master.R109,i2c_master.R110,i2c_master.R111,i2c_master.R112,i2c_master.R113,i2c_master.R114,i2c_master.R115,i2c_master.R116,i2c_master.R117,i2c_master.R118,i2c_master.R119,i2c_master.R120,i2c_master.R121,i2c_master.R123,i2c_master.R124,i2c_master.R125,i2c_master.R126,i2c_master.R127,i2c_master.R128,i2c_master.R129,i2c_master.R130,i2c_master.R131,i2c_master.R132,i2c_master.R133,i2c_master.R134,i2c_master.R135,i2c_master.R136,i2c_master.R137,i2c_master.R138,i2c_master.R139,i2c_master.R140,i2c_master.R141,i2c_master.R142,i2c_master.R143,i2c_master.R144,i2c_master.R145,i2c_master.R146,i2c_master.R147,i2c_master.R148,i2c_master.R150,i2c_master.R151,i2c_master.R152
// COVERS: 5: i2c_master.R001,i2c_master.R002,i2c_master.R003,i2c_master.R004,i2c_master.R005,i2c_master.R006,i2c_master.R007,i2c_master.R008,i2c_master.R009,i2c_master.R010,i2c_master.R011,i2c_master.R012,i2c_master.R013,i2c_master.R014,i2c_master.R015,i2c_master.R016,i2c_master.R017,i2c_master.R018,i2c_master.R019,i2c_master.R020,i2c_master.R021,i2c_master.R022,i2c_master.R023,i2c_master.R024,i2c_master.R025,i2c_master.R026,i2c_master.R027,i2c_master.R028,i2c_master.R029,i2c_master.R030,i2c_master.R032,i2c_master.R033,i2c_master.R034,i2c_master.R035,i2c_master.R036,i2c_master.R037,i2c_master.R038,i2c_master.R039,i2c_master.R040,i2c_master.R041,i2c_master.R046,i2c_master.R047,i2c_master.R048,i2c_master.R049,i2c_master.R050,i2c_master.R051,i2c_master.R052,i2c_master.R053,i2c_master.R054,i2c_master.R055,i2c_master.R056,i2c_master.R057,i2c_master.R058,i2c_master.R059,i2c_master.R060,i2c_master.R061,i2c_master.R062,i2c_master.R063,i2c_master.R064,i2c_master.R065,i2c_master.R067,i2c_master.R068,i2c_master.R069,i2c_master.R070,i2c_master.R071,i2c_master.R072,i2c_master.R073,i2c_master.R074,i2c_master.R075,i2c_master.R076,i2c_master.R077,i2c_master.R078,i2c_master.R079,i2c_master.R080,i2c_master.R081,i2c_master.R082,i2c_master.R083,i2c_master.R084,i2c_master.R085,i2c_master.R086,i2c_master.R088,i2c_master.R089,i2c_master.R090,i2c_master.R091,i2c_master.R092,i2c_master.R093,i2c_master.R094,i2c_master.R095,i2c_master.R096,i2c_master.R097,i2c_master.R098,i2c_master.R099,i2c_master.R102,i2c_master.R103,i2c_master.R105,i2c_master.R106,i2c_master.R107,i2c_master.R108,i2c_master.R109,i2c_master.R110,i2c_master.R111,i2c_master.R112,i2c_master.R113,i2c_master.R114,i2c_master.R115,i2c_master.R120,i2c_master.R121,i2c_master.R123,i2c_master.R124,i2c_master.R125,i2c_master.R126,i2c_master.R127,i2c_master.R128,i2c_master.R129,i2c_master.R130,i2c_master.R131,i2c_master.R132,i2c_master.R133,i2c_master.R134,i2c_master.R135,i2c_master.R136,i2c_master.R137,i2c_master.R138,i2c_master.R139,i2c_master.R140,i2c_master.R141,i2c_master.R142,i2c_master.R143,i2c_master.R144,i2c_master.R145,i2c_master.R146,i2c_master.R147,i2c_master.R148,i2c_master.R150,i2c_master.R151,i2c_master.R152
// COVERS: 6: i2c_master.R001,i2c_master.R002,i2c_master.R003,i2c_master.R004,i2c_master.R005,i2c_master.R006,i2c_master.R007,i2c_master.R008,i2c_master.R009,i2c_master.R010,i2c_master.R011,i2c_master.R012,i2c_master.R013,i2c_master.R014,i2c_master.R015,i2c_master.R016,i2c_master.R017,i2c_master.R018,i2c_master.R019,i2c_master.R020,i2c_master.R021,i2c_master.R022,i2c_master.R023,i2c_master.R024,i2c_master.R025,i2c_master.R026,i2c_master.R027,i2c_master.R028,i2c_master.R029,i2c_master.R030,i2c_master.R033,i2c_master.R034,i2c_master.R036,i2c_master.R037,i2c_master.R038,i2c_master.R039,i2c_master.R040,i2c_master.R041,i2c_master.R046,i2c_master.R047,i2c_master.R049,i2c_master.R050,i2c_master.R054,i2c_master.R055,i2c_master.R056,i2c_master.R059,i2c_master.R060,i2c_master.R061,i2c_master.R067,i2c_master.R068,i2c_master.R071,i2c_master.R072,i2c_master.R073,i2c_master.R074,i2c_master.R075,i2c_master.R078,i2c_master.R079,i2c_master.R080,i2c_master.R081,i2c_master.R082,i2c_master.R083,i2c_master.R084,i2c_master.R085,i2c_master.R086,i2c_master.R088,i2c_master.R089,i2c_master.R096,i2c_master.R098,i2c_master.R099,i2c_master.R109,i2c_master.R114,i2c_master.R120,i2c_master.R121,i2c_master.R123,i2c_master.R124,i2c_master.R125,i2c_master.R126,i2c_master.R130,i2c_master.R139,i2c_master.R141,i2c_master.R143,i2c_master.R145,i2c_master.R148,i2c_master.R150
// WAIVE: i2c_master.R042 unadjudicated
// WAIVE: i2c_master.R043 unadjudicated
// WAIVE: i2c_master.R044 unadjudicated
// WAIVE: i2c_master.R045 unadjudicated
// WAIVE: i2c_master.R066 unadjudicated
// WAIVE: i2c_master.R087 unadjudicated
// WAIVE: i2c_master.R100 unadjudicated
// WAIVE: i2c_master.R101 unadjudicated
// WAIVE: i2c_master.R104 unadjudicated
// WAIVE: i2c_master.R122 unadjudicated
// WAIVE: i2c_master.R149 unadjudicated
module tb_i2c_master;

  reg clk;
  reg rst_n;
  reg we;
  reg scl_i;
  reg sda_i;
  reg [15:0] prescale;
  reg [3:0] cmd;
  reg [7:0] din;
  wire ack_rx;
  wire done;
  wire busy;
  wire tip;
  wire scl_o;
  wire sda_o;
  wire scl_oe;
  wire sda_oe;
  wire [7:0] dout;

  reg [31:0] tb__rnd;
  integer tb__i;
  integer tb__w;

  i2c_master tb__dut (
    .clk(clk),
    .rst_n(rst_n),
    .we(we),
    .scl_i(scl_i),
    .sda_i(sda_i),
    .prescale(prescale),
    .cmd(cmd),
    .din(din),
    .ack_rx(ack_rx),
    .done(done),
    .busy(busy),
    .tip(tip),
    .scl_o(scl_o),
    .sda_o(sda_o),
    .scl_oe(scl_oe),
    .sda_oe(sda_oe),
    .dout(dout)
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
    #60000;
    $display("WATCHDOG");
    $finish;
  end

  initial tb__rnd = 32'd1234567;
  initial clk = 1'b0;
  always #5 clk = ~clk;

  task wr_cmd;
    input [3:0] c;
    input [7:0] d;
    begin
      @(negedge clk);
      we = 1'b1;
      cmd = c;
      din = d;
      @(negedge clk);
      we = 1'b0;
    end
  endtask

  task wait_idle_or;
    input integer max;
    integer k;
    begin
      for (k = 0; k < max; k = k + 1) begin
        @(negedge clk);
        $display("  bus=%b%b oe=%b%b busy=%b tip=%b done=%b ack=%b dout=%02h",
                 scl_o, sda_o, scl_oe, sda_oe, busy, tip, done, ack_rx, dout);
        if (!busy && !tip) k = max;
      end
    end
  endtask

  initial begin : tb__scenes
    tb__rnd = 32'd987654;
    #1;
    rst_n = 1'b1;
    we = 1'b0;
    scl_i = 1'b1;
    sda_i = 1'b1;
    prescale = 16'd4;
    cmd = 4'b0000;
    din = 8'h00;
    // SCENE1 reset_and_idle:復位 2 拍→閒置 3 拍
    $display("SCENE1 reset_and_idle");
    @(negedge clk);
    rst_n = 1'b0;
    @(negedge clk);
    @(negedge clk);
    rst_n = 1'b1;
    @(negedge clk);
    $display("  bus=%b%b oe=%b%b busy=%b tip=%b done=%b ack=%b dout=%02h",
             scl_o, sda_o, scl_oe, sda_oe, busy, tip, done, ack_rx, dout);
    @(negedge clk);
    $display("  bus=%b%b oe=%b%b busy=%b tip=%b done=%b ack=%b dout=%02h",
             scl_o, sda_o, scl_oe, sda_oe, busy, tip, done, ack_rx, dout);
    @(negedge clk);
    $display("  bus=%b%b oe=%b%b busy=%b tip=%b done=%b ack=%b dout=%02h",
             scl_o, sda_o, scl_oe, sda_oe, busy, tip, done, ack_rx, dout);
    // SCENE2 wr_byte_seq:START→WR(0xA5)→ STOP(全時序觀測)
    $display("SCENE2 wr_byte_seq");
    wr_cmd(4'b0001, 8'h00); // START
    wait_idle_or(60);
    wr_cmd(4'b0010, 8'hA5); // WR
    wait_idle_or(120);
    wr_cmd(4'b1000, 8'h00); // STOP
    wait_idle_or(60);
    // SCENE3 rd_byte_seq:START→RD(讀回 din 設定的值)
    $display("SCENE3 rd_byte_seq");
    wr_cmd(4'b0001, 8'h00);
    wait_idle_or(60);
    wr_cmd(4'b0100, 8'h3C); // RD
    wait_idle_or(120);
    wr_cmd(4'b1000, 8'h00);
    wait_idle_or(60);
    // SCENE4 repeated_start:START→WR→START→RD→STOP(重複 START 不經 STOP)
    $display("SCENE4 repeated_start");
    wr_cmd(4'b0001, 8'h00);
    wait_idle_or(60);
    wr_cmd(4'b0010, 8'h77);
    wait_idle_or(120);
    wr_cmd(4'b0001, 8'h00);
    wait_idle_or(60);
    wr_cmd(4'b0100, 8'h00);
    wait_idle_or(120);
    wr_cmd(4'b1000, 8'h00);
    wait_idle_or(60);
    // SCENE5 prescale_sweep:同 WR 序在 prescale=2/8/32 下(速控逐拍脈衝)
    $display("SCENE5 prescale_sweep");
    for (tb__i = 0; tb__i < 3; tb__i = tb__i + 1) begin
      @(negedge clk);
      prescale = tb__i[1:0] == 2'b00 ? 16'd2 : tb__i[1:0] == 2'b01 ? 16'd8 : 16'd32;
      wr_cmd(4'b0001, 8'h00);
      wait_idle_or(200);
      wr_cmd(4'b0010, 8'h5A);
      wait_idle_or(400);
      wr_cmd(4'b1000, 8'h00);
      wait_idle_or(200);
    end
    // SCENE6 mid_transfer_reset:WR 傳輸中途異步復位一拍(恢復到 IDLE)
    $display("SCENE6 mid_transfer_reset");
    wr_cmd(4'b0001, 8'h00);
    wait_idle_or(60);
    wr_cmd(4'b0010, 8'h33);
    tb__w = 0;
    while (!tip && tb__w < 40) begin
      @(negedge clk);
      tb__w = tb__w + 1;
    end
    @(negedge clk);
    rst_n = 1'b0;
    @(negedge clk);
    rst_n = 1'b1;
    @(negedge clk);
    $display("  bus=%b%b oe=%b%b busy=%b tip=%b done=%b ack=%b dout=%02h",
             scl_o, sda_o, scl_oe, sda_oe, busy, tip, done, ack_rx, dout);
    @(negedge clk);
    $display("  bus=%b%b oe=%b%b busy=%b tip=%b done=%b ack=%b dout=%02h",
             scl_o, sda_o, scl_oe, sda_oe, busy, tip, done, ack_rx, dout);
    $display("DONE");
    $finish;
  end
endmodule
