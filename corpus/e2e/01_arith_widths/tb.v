`timescale 1ns/1ps
module tb;
  reg [7:0] a = 8'd200, b = 8'd100;
  wire [8:0] sum9 = a + b;      // context width keeps the carry
  wire [7:0] sum8 = a + b;      // truncated
  wire [15:0] prod = a * b;
  initial begin
    #1;
    $display("sum9=%0d sum8=%0d prod=%0d", sum9, sum8, prod);
    $display("diff=%0d", a - b);
    $finish;
  end
endmodule
