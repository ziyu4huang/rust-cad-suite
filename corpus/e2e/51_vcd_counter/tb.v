`timescale 1ns/1ps
module counter #(parameter W = 8)(input wire clk, input wire rst, output reg [W-1:0] q);
  always @(posedge clk) begin
    if (rst) q <= {W{1'b0}};
    else     q <= q + 1'b1;
  end
endmodule

module tb;
  reg clk = 0, rst = 1;
  wire [7:0] q;
  counter #(.W(8)) dut (.clk(clk), .rst(rst), .q(q));
  always #5 clk = ~clk;
  initial begin
    $dumpfile("dump.vcd");
    $dumpvars(0, tb);
    repeat (3) @(posedge clk);
    rst <= 1'b0;
    repeat (5) @(posedge clk);
    #1;
    if (q !== 8'd5) $display("FAIL q=%0d", q);
    else            $display("PASS");
    $finish;
  end
endmodule
