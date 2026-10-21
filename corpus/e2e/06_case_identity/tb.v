`timescale 1ns/1ps
module tb;
  reg [3:0] v = 4'b0101;
  initial begin
    // === is bit-exact identity (corpus is X-clean by policy: no z literals)
    $display("ce=%b cne=%b", v === 4'b0101, v !== 4'b0101);
    $display("ce0=%b", v === 4'b0011);
    $finish;
  end
endmodule
