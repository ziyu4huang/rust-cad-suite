`timescale 1ns/1ps
// TB for XLS-generated 2-stage acc_pipe (module __acc_pipe__acc_pipe;
// pipeline stages + flops = 3-cycle latency). Same race-safe sampling.
module tb;
  reg clk = 0;
  reg [7:0] x = 0, y = 0;
  wire [15:0] out;
  always #5 clk = ~clk;
  __acc_pipe__acc_pipe dut(.clk(clk), .x(x), .y(y), .out(out));
  initial begin
    @(negedge clk);
    x = 8'd3; y = 8'd5;
    repeat (5) @(posedge clk);
    #1 $display("pipe=%0d", out);
    @(negedge clk);
    x = 8'd250; y = 8'd10;
    repeat (5) @(posedge clk);
    #1 $display("pipe=%0d", out);
    $finish;
  end
endmodule
