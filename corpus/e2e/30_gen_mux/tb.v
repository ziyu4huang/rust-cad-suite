`timescale 1ns/1ps
module tb;
  genvar k;
  reg [1:0] sel = 2'd2;
  reg [7:0] chans [0:3];
  wire [7:0] picked_one;
  generate
    if (1) begin : g
      assign picked_one = chans[sel];
    end
  endgenerate
  initial begin
    chans[0] = 8'd10; chans[1] = 8'd20; chans[2] = 8'd30; chans[3] = 8'd40;
    #1 $display("picked=%0d", picked_one);
    $finish;
  end
endmodule
