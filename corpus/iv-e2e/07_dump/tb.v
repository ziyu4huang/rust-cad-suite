`timescale 1ns/1ps
// riverilog R0 chase case: $dumpfile/$dumpvars — stdout carries the VCD info
// line; VCD content cross-check lands at R5 via vl-vcd.
module tb;
  reg [7:0] c = 0;
  initial begin
    $dumpfile("dump.vcd");
    $dumpvars(0, tb);
    #5 c = 8'd255;
    #5 $finish;
  end
endmodule
