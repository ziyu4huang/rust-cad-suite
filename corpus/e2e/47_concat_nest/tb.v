`timescale 1ns/1ps
module tb;
  reg [1:0] a = 2'b10;
  reg [3:0] b = 4'b0011;
  initial begin
    $display("%b", {a, {2{b[1:0]}}, 3'b111});
    $display("w=%0d", $bits({a, b}));
    $finish;
  end
endmodule
