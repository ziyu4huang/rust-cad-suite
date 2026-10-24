`timescale 1ns/1ps
module tb;
  reg [3:0] v;
  reg [3:0] r;
  initial begin
    v = 4'b1x01;
    casez (v)
      4'b1??1: r = 4'd1;
      4'b???? : r = 4'd2;
    endcase
    $display("casez r=%0d", r);
    casex (v)
      4'b1x01: r = 4'd3;
      default: r = 4'd4;
    endcase
    $display("casex r=%0d", r);
    $finish;
  end
endmodule
