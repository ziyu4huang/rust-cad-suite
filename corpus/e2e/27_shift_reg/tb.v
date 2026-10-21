`timescale 1ns/1ps
module tb;
  reg clk = 0;
  reg [7:0] sr = 8'b0;
  always #5 clk = ~clk;
  always @(posedge clk) sr <= {sr[6:0], ~sr[7]};
  initial begin
    $dumpfile("dump.vcd"); $dumpvars(0, tb);
    sr = 8'b10000001;
    repeat (3) @(posedge clk);
    #1;
    $display("sr=%b", sr);
    $finish;
  end
endmodule
