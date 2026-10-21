`timescale 1ns/1ps
module tb;
  reg clk = 0;
  reg [3:0] qb = 0, qn = 0;
  always #5 clk = ~clk;
  // Blocking: sampled value is the OLD qb in the same edge (process order).
  // Nonblocking: qn updates commit after active region.
  always @(posedge clk) qb = qb + 1;
  always @(posedge clk) qn <= qn + 1;
  initial begin
    @(posedge clk); #1;
    $display("after1 b=%0d n=%0d", qb, qn);
    @(posedge clk); #1;
    $display("after2 b=%0d n=%0d", qb, qn);
    $finish;
  end
endmodule
