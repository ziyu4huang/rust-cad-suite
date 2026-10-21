`timescale 1ns/1ps
module tb;
  reg [3:0] lo = 4'h1, hi = 4'h2;
  reg [7:0] w = 8'hAB;
  reg x = 1'b1, y = 1'b0;
  initial begin
    {x, y} = {y, x};
    {w[3:0], w[7:4]} = {w[7:4], w[3:0]};
    #1;
    $display("xy=%b%b w=%h", x, y, w);
    $display("cat=%b", {lo, hi});
    $display("repl=%b", {3{2'b10}});
    $finish;
  end
endmodule
