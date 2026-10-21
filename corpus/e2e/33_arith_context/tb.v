`timescale 1ns/1ps
module tb;
  reg [7:0] a = 8'd200, b = 8'd100, c = 8'd6, d = 8'd3;
  reg [15:0] wa = 16'd200, wb = 16'd100, wc = 16'd6, wd = 16'd3;
  wire [15:0] wide = ((wa + wb) * wc) / wd;  // full-precision chain
  wire [7:0] narrow = (a & b) | (c ^ d);
  wire [8:0] carry = a + b;               // context keeps bit 8
  initial begin
    #1 $display("wide=%0d narrow=%0d carry=%0d", wide, narrow, carry);
    $finish;
  end
endmodule
