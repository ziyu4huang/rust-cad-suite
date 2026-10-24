`timescale 1ns/1ps
module tb;
  reg [7:0] a, v;
  initial begin
    a = 8'd10; v = 8'd20;
    a = #5 v;      // samples v NOW (20), commits at t5
    v = 8'd30;     // ...before this t0 change
    #1 $display("t6 a=%0d v=%0d", a, v);
    $finish;
  end
endmodule
