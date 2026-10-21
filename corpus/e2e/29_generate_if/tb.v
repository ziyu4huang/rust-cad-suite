`timescale 1ns/1ps
module dut #(parameter MODE = 1)(input wire a, output wire y);
  generate
    if (MODE == 1) begin : m1
      assign y = ~a;
    end else begin : m0
      assign y = a;
    end
  endgenerate
endmodule

module tb;
  wire y1, y0;
  dut #(.MODE(1)) u1 (.a(1'b0), .y(y1));
  dut #(.MODE(0)) u2 (.a(1'b0), .y(y0));
  initial begin
    #1 $display("y1=%b y0=%b", y1, y0);
    $finish;
  end
endmodule
