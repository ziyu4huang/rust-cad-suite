`timescale 1ns/1ps
module tb;
  reg [7:0] q, d;
  initial begin
    q = 0; d = 5;
    q <= #10 d;    // sampled at schedule (5), commits t10
    d = 9;         // later t0 write must NOT leak in
    #11 $display("q=%0d", q);
    $finish;
  end
endmodule
