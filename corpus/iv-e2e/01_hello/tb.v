`timescale 1ns/1ps
// riverilog R0 chase case: $display + $finish contract (file:line + unit).
module tb;
  initial begin
    $display("Hello 42");
    $finish;
  end
endmodule
