`timescale 1ns/1ps
// riverilog R0 chase case: gate primitives with delays (nand #6 not settled
// at t3 reads x; buf #2 settles) — pins gate-delay semantics.
module tb;
  reg x;
  wire mid;
  wire y;
  nand #6 g(y, x, x);
  buf #2 b1(mid, x);
  initial begin
    x = 0;
    #3 $display("t3 y=%b mid=%b", y, mid);
    x = 1;
    #10 $display("t13 y=%b mid=%b", y, mid);
    $finish;
  end
endmodule
