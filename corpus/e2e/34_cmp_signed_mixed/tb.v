`timescale 1ns/1ps
module tb;
  reg [3:0] u = 4'b1111;
  reg signed [3:0] s = -4'sd1;
  initial begin
    $display("s=%0d u=%0d", s, u);
    $display("s_lt0=%b", s < 0);
    $display("mixed=%b", s > u);
    $display("slit=%b", 4'sd7 > 4'sd8);
    $finish;
  end
endmodule
