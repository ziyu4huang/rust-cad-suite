// 原版 acc_pipe 直连 wrapper(TB 统一接口 dut_accpipe)
`timescale 1ns/1ps
module dut_accpipe (
    input  wire        clk,
    input  wire [7:0]  x,
    input  wire [7:0]  y,
    output wire [15:0] out
);
  __acc_pipe__acc_pipe u (.clk(clk), .x(x), .y(y), .out(out));
endmodule
