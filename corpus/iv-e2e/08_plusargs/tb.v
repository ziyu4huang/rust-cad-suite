`timescale 1ns/1ps
// riverilog R0 chase case: plusargs (runner passes +N=42 deterministically).
module tb;
  reg [31:0] v;
  initial begin
    if ($value$plusargs("N=%d", v)) $display("N=%0d", v);
    else $display("no N");
    if ($test$plusargs("N")) $display("N arg present");
    $finish;
  end
endmodule
