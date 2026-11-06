// 原版直连 wrapper:签名天然匹配 TB
`timescale 1ns/1ps
module dut_i2c (
    input  wire       clk,
    input  wire       rst_n,
    input  wire       we,
    input  wire       scl_i,
    input  wire       sda_i,
    output wire       ack_rx,
    output wire       done,
    output wire       busy,
    output wire       tip,
    output wire       scl_o,
    output wire       sda_o,
    output wire       scl_oe,
    output wire       sda_oe,
    input  wire [15:0] prescale,
    input  wire [3:0]  cmd,
    input  wire [7:0]  din,
    output wire [7:0]  dout
);
  i2c_master u (
    .clk(clk), .rst_n(rst_n), .we(we), .scl_i(scl_i), .sda_i(sda_i),
    .ack_rx(ack_rx), .done(done), .busy(busy), .tip(tip),
    .scl_o(scl_o), .sda_o(sda_o), .scl_oe(scl_oe), .sda_oe(sda_oe),
    .prescale(prescale), .cmd(cmd), .din(din), .dout(dout)
  );
endmodule
