`timescale 1ns/1ps
module tb;
  reg a;
  wire y;
  assign y = ~a;
  initial begin
    a = 0;
    #1 $display("y=%b", y);
    force y = 1'b0;
    #1 $display("forced y=%b", y);
    a = 1;          // driver flips, force still wins
    #1 $display("a=1 forced y=%b", y);
    release y;
    #1 $display("released y=%b", y);
    $finish;
  end
endmodule
