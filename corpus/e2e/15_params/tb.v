`timescale 1ns/1ps
module adder #(parameter W = 8, parameter V = 1)(
  input  wire [W-1:0] a,
  output wire [W-1:0] y
);
  localparam [W-1:0] BIAS = V;
  assign y = a + BIAS;
endmodule

module tb;
  wire [3:0] y4;
  wire [7:0] y8;
  adder #(.W(4), .V(3)) u1 (.a(4'd5), .y(y4));
  adder u2 (.a(8'd10), .y(y8));
  localparam CW = $clog2(300);
  initial begin
    #1;
    $display("y4=%0d y8=%0d cw=%0d", y4, y8, CW);
    $finish;
  end
endmodule
