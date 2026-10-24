`timescale 1ns/1ps
module tb;
  reg [3:0] x;
  initial begin
    x = 0;
    $display("A x=%0d", x);
    #0 $display("Z x=%0d", x);
    x <= 4'd7;
    #1 $display("B x=%0d", x);
    $finish;
  end
endmodule
