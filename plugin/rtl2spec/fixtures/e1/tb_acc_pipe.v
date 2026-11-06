// E1 公共 TB — acc_pipe(原版 corpus/e2e/58_xls_acc_pipe/acc_pipe.v vs 盲写版
// reimpl/acc_pipe/top.v)。原版 x/y 8 位、out 16 位;盲写版参数化 WIDTH,
// wrapper 以 WIDTH=8 实例化使接口可比(接口契约本身是规格书缺口,见记录)。
// 输出协议:SCENE<n> 前缀 + 逐拍 out 值;两版逐行 diff。
`timescale 1ns/1ps
module tb_acc_pipe;
  reg clk = 0;
  reg [7:0] x, y;
  wire [15:0] out;
  integer i;

  // 两版经统一 wrapper dut_accpipe(原版直连 wrapper=dut_orig_accpipe.v;
  // 盲写版 wrapper=dut_reimpl_accpipe.v 以 WIDTH=8 实例化+out 零扩展)
  dut_accpipe dut_i (.clk(clk), .x(x), .y(y), .out(out));

  always #5 clk = ~clk;
  task tick; begin @(posedge clk); #1; end endtask

  initial begin
    x = 8'h00; y = 8'h00;

    // S1 上电 X 传播:3 拍内 out 全程 x(无复位无初值)
    $display("SCENE1 power_on_x");
    for (i = 0; i < 3; i = i + 1) begin tick; $display("  out=%h", out); end

    // S2 定值流:x=5,y=7 保持 8 拍,流水线收敛后的 out 稳态值现形
    $display("SCENE2 steady_5_7");
    x = 8'd5; y = 8'd7;
    for (i = 0; i < 8; i = i + 1) begin tick; $display("  t%0d out=%h", i, out); end

    // S3 进位路径:x=200,y=100(x+y 跨 256)
    $display("SCENE3 carry_200_100");
    x = 8'd200; y = 8'd100;
    for (i = 0; i < 8; i = i + 1) begin tick; $display("  t%0d out=%h", i, out); end

    // S4 全 1:x=255,y=255
    $display("SCENE4 max_255_255");
    x = 8'hFF; y = 8'hFF;
    for (i = 0; i < 8; i = i + 1) begin tick; $display("  t%0d out=%h", i, out); end

    // S5 动态流:x/y 交替变化 10 拍(观测逐拍流水线序列)
    $display("SCENE5 dynamic_stream");
    for (i = 0; i < 10; i = i + 1) begin
      x = 8'd1 + i[7:0];
      y = 8'd10 - i[7:0];
      tick; $display("  t%0d x=%0d y=%0d out=%h", i, x, y, out);
    end

    // S6 零输入收敛
    $display("SCENE6 zero_converge");
    x = 8'd0; y = 8'd0;
    for (i = 0; i < 6; i = i + 1) begin tick; $display("  t%0d out=%h", i, out); end

    $display("DONE");
    $finish;
  end
endmodule
