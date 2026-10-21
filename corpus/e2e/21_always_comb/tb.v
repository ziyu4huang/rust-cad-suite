`timescale 1ns/1ps
module tb;
  reg [3:0] a = 4'd3, b = 4'd5;
  reg [3:0] s, p;
  wire [3:0] w = a + b;
  always @* s = a + b;
  always @(a or b) p = a * b;
  initial begin
    $dumpfile("dump.vcd"); $dumpvars(0, tb);
    #1 $display("w=%0d s=%0d p=%0d", w, s, p);
    a = 4'd6;
    #1 $display("w=%0d s=%0d p=%0d", w, s, p);
    $finish;
  end
endmodule
