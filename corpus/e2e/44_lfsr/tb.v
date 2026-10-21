`timescale 1ns/1ps
module tb;
  reg clk = 0;
  reg [7:0] lfsr = 8'hAC;
  always #5 clk = ~clk;
  always @(posedge clk) lfsr <= {lfsr[6:0], lfsr[7] ^ lfsr[5] ^ lfsr[4] ^ lfsr[3]};
  initial begin
    repeat (8) @(posedge clk);
    #1 $display("lfsr=%h", lfsr);
    $finish;
  end
endmodule
