`timescale 1ns/1ps
module tb;
  reg [7:0] v = 8'b11100010;
  initial begin
    $display("pow=%0d", 3 ** 4);
    $display("shl=%b shr=%b", v << 2, v >> 2);
    $display("ashr=%b ashl=%b", $signed(v) >>> 2, $signed(v) <<< 1);
    $display("shl_amt_var=%b", v << (2 + 1));
    $finish;
  end
endmodule
