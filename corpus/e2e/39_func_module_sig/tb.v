`timescale 1ns/1ps
module tb;
  reg [3:0] base = 4'd2;
  reg en = 1'b1;
  function [7:0] scaled(input [3:0] v);
    scaled = en ? {4'd0, v} << base[1:0] : {4'd0, v};
  endfunction
  initial begin
    #1 $display("r=%0d", scaled(4'd3));
    base = 4'd1;
    #1 $display("r=%0d", scaled(4'd3));
    $finish;
  end
endmodule
