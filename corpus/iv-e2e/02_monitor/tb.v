`timescale 1ns/1ps
// riverilog R0 chase case: $monitor fires on changes (incl. the t0 arm).
module tb;
  reg [7:0] c = 0;
  initial begin
    $monitor("m: t=%0t c=%0d", $time, c);
    #5 c = 8'd10;
    #5 c = 8'd20;
    #5 $finish;
  end
endmodule
