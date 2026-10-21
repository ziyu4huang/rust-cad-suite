`timescale 1ns/1ps
module tb;
  function [7:0] max8(input [7:0] a, input [7:0] b);
    max8 = (a > b) ? a : b;
  endfunction
  initial begin
    $display("max=%0d", max8(8'd42, 8'd17));
    $display("max2=%0d", max8(8'd3, 8'd9));
    $finish;
  end
endmodule
