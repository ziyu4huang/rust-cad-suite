`timescale 1ns/1ps
module tb;
  reg [3:0] a = 4'd5, b = 4'd9;
  initial begin
    $display("lt=%b le=%b gt=%b ge=%b", a < b, a <= b, a > b, a >= b);
    $display("eq=%b ne=%b", a == b, a != b);
    $display("land=%b lor=%b lnot=%b", (a < b) && (b > 0), (a > b) || (b > 0), !(a < b));
    $finish;
  end
endmodule
