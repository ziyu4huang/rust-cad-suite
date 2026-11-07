// tb_acc_r0_k1.v — D 腿候選(r0,K=1/1;CONFORMANCE-PROTOCOL 腿 ID=acc-r0-k1)
// 撰寫依據=prompt_r0_k1.md(規則書+tb_plan+骨架);場景組織與模板腿不同:按流水線
// 級聯特性分三段——同值注入(x=y)後只變一側(x≠y)再只變另一側,讓「每一級的
// 數據相關」逐級現形;x/y 經全寬隨機與邊界值(0x00/0xFF)交替。
`timescale 1ns/1ps
// SCENE1: pipe_stages_sync_ramp
// SCENE2: only_x_changes
// SCENE3: only_y_changes
// SCENE4: boundary_walk
// SCENE LIST: pipe_stages_sync_ramp,only_x_changes,only_y_changes,boundary_walk
// COVERS: 1: __acc_pipe__acc_pipe.R001,__acc_pipe__acc_pipe.R002,__acc_pipe__acc_pipe.R003,__acc_pipe__acc_pipe.R004,__acc_pipe__acc_pipe.R005,__acc_pipe__acc_pipe.R006,__acc_pipe__acc_pipe.R007,__acc_pipe__acc_pipe.R008,__acc_pipe__acc_pipe.R009
// COVERS: 2: __acc_pipe__acc_pipe.R001,__acc_pipe__acc_pipe.R002,__acc_pipe__acc_pipe.R003,__acc_pipe__acc_pipe.R004,__acc_pipe__acc_pipe.R005,__acc_pipe__acc_pipe.R006,__acc_pipe__acc_pipe.R007,__acc_pipe__acc_pipe.R008,__acc_pipe__acc_pipe.R009
// COVERS: 3: __acc_pipe__acc_pipe.R001,__acc_pipe__acc_pipe.R002,__acc_pipe__acc_pipe.R003,__acc_pipe__acc_pipe.R004,__acc_pipe__acc_pipe.R005,__acc_pipe__acc_pipe.R006,__acc_pipe__acc_pipe.R007,__acc_pipe__acc_pipe.R008,__acc_pipe__acc_pipe.R009
// COVERS: 4: __acc_pipe__acc_pipe.R001,__acc_pipe__acc_pipe.R002,__acc_pipe__acc_pipe.R003,__acc_pipe__acc_pipe.R004,__acc_pipe__acc_pipe.R005,__acc_pipe__acc_pipe.R006,__acc_pipe__acc_pipe.R007,__acc_pipe__acc_pipe.R008,__acc_pipe__acc_pipe.R009
module tb___acc_pipe__acc_pipe;

  reg clk;
  reg [7:0] x;
  reg [7:0] y;
  wire [15:0] out;

  reg [31:0] tb__rnd;
  integer tb__i;

  __acc_pipe__acc_pipe tb__dut (
    .clk(clk),
    .x(x),
    .y(y),
    .out(out)
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

  initial tb__rnd = 32'd305419896;
  initial clk = 1'b0;
  always #5 clk = ~clk;

  initial begin : tb__scenes
    tb__rnd = 32'd21849;
    #1;
    x = 8'h00;
    y = 8'h00;
    // SCENE1 pipe_stages_sync_ramp:x=y 同步注入,三級流水逐級現形(R001/R002/R005/R006/R009 的
    // 級聯延遲=3 拍;每拍打印一次,流水線各級值可區分)
    $display("SCENE1 pipe_stages_sync_ramp");
    // 前 3 個 negedge 只驅動不打印:三級流水線灌滿(上電 out 未定義,打印會出 X)
    for (tb__i = 0; tb__i < 3; tb__i = tb__i + 1) begin
      @(negedge clk);
      tb__rnd = tb__xs32(tb__rnd);
      x = tb__rnd[15:8];
      y = x;
    end
    for (tb__i = 0; tb__i < 20; tb__i = tb__i + 1) begin
      @(negedge clk);
      $display("  out=%h", out);
      tb__rnd = tb__xs32(tb__rnd);
      x = tb__rnd[15:8];
      y = x;
    end
    // SCENE2 only_x_changes:只變 x(R001/R003/R005/R007 的 x 通路單側敏感)
    $display("SCENE2 only_x_changes");
    for (tb__i = 0; tb__i < 12; tb__i = tb__i + 1) begin
      @(negedge clk);
      $display("  out=%h", out);
      tb__rnd = tb__xs32(tb__rnd);
      x = tb__rnd[15:8];
    end
    // SCENE3 only_y_changes:只變 y(R002/R003/R006/R007 的 y 通路單側敏感)
    $display("SCENE3 only_y_changes");
    for (tb__i = 0; tb__i < 12; tb__i = tb__i + 1) begin
      @(negedge clk);
      $display("  out=%h", out);
      tb__rnd = tb__xs32(tb__rnd);
      y = tb__rnd[15:8];
    end
    // SCENE4 boundary_walk:0x00/0xFF/隨機交替(R003 加法邊界/R008 位拼接的最低位)
    $display("SCENE4 boundary_walk");
    for (tb__i = 0; tb__i < 16; tb__i = tb__i + 1) begin
      @(negedge clk);
      $display("  out=%h", out);
      tb__rnd = tb__xs32(tb__rnd);
      case (tb__i[1:0])
        2'b00: begin
          x = 8'h00;
          y = 8'h00;
        end
        2'b01: begin
          x = 8'hFF;
          y = 8'hFF;
        end
        2'b10: begin
          x = 8'hFF;
          y = 8'h01;
        end
        default: begin
          x = tb__rnd[15:8];
          y = tb__rnd[23:16];
        end
      endcase
    end
    @(negedge clk);
    $display("  out=%h", out);
    $display("DONE");
    $finish;
  end
endmodule
