`timescale 1ns/1ps
module tb;
  function [3:0] f(input [1:0] v);
    begin
      case (v)
        default: f = 4'd9;
        2'b01: f = 4'd1;
        2'b10: f = 4'd2;
      endcase
    end
  endfunction
  initial begin
    $display("%0d %0d %0d", f(2'b01), f(2'b10), f(2'b11));
    $finish;
  end
endmodule
