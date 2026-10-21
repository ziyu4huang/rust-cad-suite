`timescale 1ns/1ps
module tb;
  genvar i;
  wire [3:0] in = 4'b0101;
  wire [3:0] inv;
  generate
    for (i = 0; i < 4; i = i + 1) begin : g
      assign inv[i] = ~in[i];
    end
  endgenerate
  initial begin
    #1 $display("inv=%b", inv);
    $finish;
  end
endmodule
