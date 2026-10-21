`timescale 1ns/1ps
module tb;
  reg [7:0] counter = 8'd41;
  function [7:0] bump;
    bump = counter + 1'b1;
  endfunction
  initial begin
    #1 $display("%0d", bump());
    counter = 8'd100;
    #1 $display("%0d", bump());
    $finish;
  end
endmodule
