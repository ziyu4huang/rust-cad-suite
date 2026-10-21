`timescale 1ns/1ps
module tb;
  reg c1 = 1'b0, c2 = 1'b1;
  reg [3:0] a = 4'd3, b = 4'd7, d = 4'd12;
  initial begin
    #1;
    $display("t=%0d", c1 ? a : (c2 ? b : d));
    $display("t2=%0d", (a < b) ? a + b : a - b);
    $finish;
  end
endmodule
