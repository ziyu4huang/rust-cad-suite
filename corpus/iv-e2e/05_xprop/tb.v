`timescale 1ns/1ps
// riverilog R0 chase case: 4-state propagation — arithmetic all-x, bitwise
// per-bit tables, condition x takes the else branch.
module tb;
  reg [3:0] a = 4'b10x1;
  reg [3:0] z = 4'b001z;
  initial begin
    $display("add=%b", a + 4'b0001);
    $display("and=%b", a & z);
    $display("or=%b", a | 4'b0011);
    if (a[2]) $display("taken"); else $display("else");
    $finish;
  end
endmodule
