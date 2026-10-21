`timescale 1ns/1ps
module tb;
  reg signed [7:0] v = -8'sd100;
  initial begin
    $display("a=%0d b=%0d c=%0d", v >>> 1, v >>> 2, v >>> 3);
    $display("u=%0d", $unsigned(v) >>> 1);
    $finish;
  end
endmodule
