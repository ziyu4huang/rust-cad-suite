`timescale 1ns/1ps
module inner #(parameter W = 4)(input wire [W-1:0] a, output wire [W-1:0] y);
  assign y = a + 1'b1;
endmodule

module outer #(parameter IW = 6)(input wire [IW-1:0] a, output wire [IW-1:0] y);
  localparam DW = $clog2(IW) + 2;
  wire [DW-1:0] mid;
  inner #(.W(DW)) u (.a(a[DW-1:0]), .y(mid));
  assign y = {{(IW-DW){1'b0}}, mid};
endmodule

module tb;
  reg [5:0] a = 6'd40;
  wire [5:0] y;
  outer #(.IW(6)) o (.a(a), .y(y));
  initial begin
    #1 $display("y=%0d", y);
    $finish;
  end
endmodule
