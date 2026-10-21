`timescale 1ns/1ps
module tb;
  reg clk = 0, rst = 1, ld = 0, en = 1;
  reg [7:0] din = 8'd50;
  reg [7:0] q = 0;
  always #5 clk = ~clk;
  always @(posedge clk) begin
    if (rst)      q <= 8'd0;
    else if (ld)  q <= din;
    else if (en)  q <= q + 1'b1;
  end
  // Control changes happen mid-cycle (#1 after an edge) — no same-edge
  // write/read race (IEEE nondeterministic; see corpus README policy).
  initial begin
    @(posedge clk); rst = 0; #1;
    @(posedge clk); #1; ld = 1;
    @(posedge clk); #1; ld = 0;
    repeat (3) @(posedge clk);
    #1 $display("q=%0d", q);
    $finish;
  end
endmodule
