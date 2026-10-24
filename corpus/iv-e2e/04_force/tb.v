`timescale 1ns/1ps
// riverilog R0 chase case: force/procedural-assign + release (the reg keeps
// the forced value after release when nothing else drives it).
module tb;
  reg r = 0;
  wire w = r;
  initial begin
    #1 force r = 1'b1;
    #1 $display("forced r=%b w=%b", r, w);
    #1 release r;
    #1 $display("released r=%b w=%b", r, w);
    $finish;
  end
endmodule
