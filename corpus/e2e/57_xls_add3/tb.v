`timescale 1ns/1ps
// TB for XLS-generated add3 (module __add3__add3; flop_inputs+outputs =
// 2-cycle latency). Sampling waits 4 posedges + #1 (race-safe, corpus R1).
module tb;
  reg clk = 0;
  reg [7:0] x = 0, y = 0, z = 0;
  wire [8:0] out;
  always #5 clk = ~clk;
  __add3__add3 dut(.clk(clk), .x(x), .y(y), .z(z), .out(out));
  initial begin
    @(negedge clk);
    x = 8'd1; y = 8'd2; z = 8'd3;
    repeat (4) @(posedge clk);
    #1 $display("add3=%0d", out);
    @(negedge clk);
    x = 8'd200; y = 8'd100; z = 8'd55;
    repeat (4) @(posedge clk);
    #1 $display("add3=%0d", out);
    $finish;
  end
endmodule
