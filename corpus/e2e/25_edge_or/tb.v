`timescale 1ns/1ps
module tb;
  reg a = 0, b = 1;
  reg [3:0] hits = 0;
  always @(posedge a or negedge b) hits = hits + 1;
  initial begin
    #1 a = 1;      // posedge a
    #1 b = 0;      // negedge b
    #1 a = 0;      // negedge a: no hit
    #1 a = 1;      // posedge a
    #1;
    $display("hits=%0d", hits);
    $finish;
  end
endmodule
