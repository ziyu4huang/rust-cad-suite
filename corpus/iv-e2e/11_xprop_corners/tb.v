`timescale 1ns/1ps
// Clean-room R4 mining case: x/z propagation corners (families from
// RIVERILOG-PLAN §4-1; written fresh, expectations recorded from iverilog).
module tb;
  reg [3:0] a;
  reg [3:0] b;
  wire [3:0] sum = a + 1;
  wire [3:0] bw   = a & 4'b001z;
  wire [3:0] orr  = a | b;
  initial begin
    a = 4'b10x1; b = 4'b01z0;
    #1 $display("sum=%b bw=%b or=%b", sum, bw, orr);
    if (a[2]) $display("taken");
    else $display("else-taken");
    $display("eq=%b eqcase=%b", (a == 4'b10x1), (a === 4'b10x1));
    $finish;
  end
endmodule
