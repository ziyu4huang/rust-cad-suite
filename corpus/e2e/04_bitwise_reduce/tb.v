`timescale 1ns/1ps
module tb;
  reg [3:0] a = 4'b1011;
  initial begin
    $display("and=%b or=%b xor=%b", a & 4'b0110, a | 4'b0100, a ^ 4'b1111);
    $display("nand=%b nor=%b xnor=%b", ~(a & 4'b1111), ~(a | 4'b0000), a ~^ 4'b1011);
    $display("red_and=%b red_or=%b red_xor=%b", &a, |a, ^a);
    $display("not=%b", ~a);
    $finish;
  end
endmodule
