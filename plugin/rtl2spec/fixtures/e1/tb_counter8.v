// E1 公共 TB — counter8(原版 corpus/counter8/counter.v 与盲写版 reimpl/counter8/top.v)
// 模块签名两版一致:counter #(.WIDTH(8))(clk, rst, q)。
// 输出协议:每个场景打印 "SCENE<n> <name>" 前缀,块内逐拍打印 q;
// 两版输出逐行 diff,块内任何差异=该场景失配。
`timescale 1ns/1ps
module tb_counter8;
  reg clk = 0, rst = 0;
  wire [7:0] q;
  integer i;

  counter #(.WIDTH(8)) dut (.clk(clk), .rst(rst), .q(q));

  always #5 clk = ~clk;
  task tick; begin @(posedge clk); #1; end endtask

  initial begin
    // S1 上电无复位:4 拍,q 保持 x(x+1=x)
    $display("SCENE1 power_on_no_reset");
    for (i = 0; i < 4; i = i + 1) begin tick; $display("  q=%h", q); end

    // S2 同步复位生效:rst=1 3 拍,q=0
    $display("SCENE2 sync_reset");
    rst = 1;
    for (i = 0; i < 3; i = i + 1) begin tick; $display("  q=%h", q); end

    // S3 释放后计数 5 拍:1,2,3,4,5
    $display("SCENE3 count_after_release");
    rst = 0;
    for (i = 0; i < 5; i = i + 1) begin tick; $display("  q=%h", q); end

    // S4 计数中途拉复位:计到中间值后 rst=1 → 下一拍回 0
    $display("SCENE4 mid_count_reset");
    for (i = 0; i < 10; i = i + 1) tick;
    $display("  pre=%h", q);
    rst = 1; tick; $display("  after_reset_edge=%h", q);
    rst = 0; tick; $display("  after_release_edge=%h", q);

    // S5 环绕:rst 清零后连打 257 拍,打印 253..257 拍的 q(全1→0 现形)
    $display("SCENE5 wrap");
    rst = 1; tick; rst = 0;
    for (i = 0; i < 257; i = i + 1) begin
      tick;
      if (i >= 252) $display("  tick%0d q=%h", i + 1, q);
    end

    // S6 rst 保持多拍再释放:仍从 0 起计
    $display("SCENE6 hold_reset_then_count");
    rst = 1; tick; tick;
    $display("  held=%h", q);
    rst = 0;
    for (i = 0; i < 3; i = i + 1) begin tick; $display("  q=%h", q); end

    $display("DONE");
    $finish;
  end
endmodule
