`timescale 1ns/1ps
module tb;
  reg [63:0] a = 64'hDEAD_BEEF_CAFE_0001;
  reg [63:0] b = 64'h0000_0000_0000_FFFF;
  initial begin
    $display("and=%h or=%h xor=%h", a & b, a | b, a ^ b);
    $display("shl=%h shr=%h", a << 16, a >> 16);
    $display("add=%h", a + b);
    $display("hi=%h lo=%h", a[63:32], a[31:0]);
    $finish;
  end
endmodule
