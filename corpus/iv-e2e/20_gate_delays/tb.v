`timescale 1ns/1ps
module tb;
  reg a;
  wire fast, slow;
  buf b1(fast, a);
  buf #4 b2(slow, a);
  initial begin
    a = 1'b0;
    #2 a = 1'b1;
    #1 $display("t3 fast=%b slow=%b", fast, slow);
    #3 $display("t6 fast=%b slow=%b", fast, slow);
    $finish;
  end
endmodule
