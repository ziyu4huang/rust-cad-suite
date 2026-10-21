`timescale 1ns/1ps
module dut #(parameter MODE = 2)(input wire a, output wire y);
  generate
    case (MODE)
      0: begin : c0 assign y = 1'b0; end
      1: begin : c1 assign y = a; end
      default: begin : cd assign y = ~a; end
    endcase
  endgenerate
endmodule

module tb;
  wire y0, y1, y2;
  dut #(.MODE(0)) u0 (.a(1'b1), .y(y0));
  dut #(.MODE(1)) u1 (.a(1'b1), .y(y1));
  dut #(.MODE(7)) u2 (.a(1'b1), .y(y2));
  initial begin
    #1 $display("%b%b%b", y0, y1, y2);
    $finish;
  end
endmodule
