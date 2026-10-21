`timescale 1ns/1ps
module tb;
  reg clk = 0;
  reg a = 0;
  reg [3:0] tick = 0;
  always #5 clk = ~clk;
  always @(posedge clk) a <= ~a;
  always @(posedge clk) if (a) tick <= tick + 1;
  initial begin
    $dumpfile("dump.vcd"); $dumpvars(0, tb);
    repeat (6) @(posedge clk);
    #1 $display("tick=%0d", tick);
    $finish;
  end
endmodule
