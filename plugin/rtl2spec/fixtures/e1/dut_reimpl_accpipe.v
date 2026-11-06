// 盲写版 acc_pipe wrapper:以 WIDTH=8 实例化(匹配原版 8 位端口;WIDTH 默认 32
// 是盲写版假设 A3,接口位宽差异=规格书缺口证据)。out:盲写版 WIDTH+2=10 位,
// 原版 16 位——高 6 位零扩展连接(低位对齐),高段值差异保留为对拍维度。
`timescale 1ns/1ps
module dut_accpipe (
    input  wire        clk,
    input  wire [7:0]  x,
    input  wire [7:0]  y,
    output wire [15:0] out
);
  wire [9:0] out_r;
  __acc_pipe__acc_pipe #(.WIDTH(8)) u (
    .clk(clk), .x(x), .y(y), .out(out_r)
  );
  assign out = {6'h00, out_r};
endmodule
