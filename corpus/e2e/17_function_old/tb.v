`timescale 1ns/1ps
module tb;
  function [7:0] double;
    input [3:0] v;
    begin
      double = {3'b000, v, 1'b0};
    end
  endfunction
  function [7:0] poly;
    input [3:0] x;
    reg [7:0] x2;
    begin
      x2 = double(x);
      poly = x2 + {4'b0000, x};
    end
  endfunction
  initial begin
    $display("double(5)=%0d poly(5)=%0d", double(4'd5), poly(4'd5));
    $finish;
  end
endmodule
