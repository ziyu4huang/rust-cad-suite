`timescale 1ns/1ps
module leaf(input wire a, output wire y);
  assign y = ~a;
endmodule

module mid(input wire a, output wire y);
  wire l;
  leaf l0 (.a(a), .y(l));
  leaf l1 (.a(l), .y(y));
endmodule

module tb;
  reg a = 1'b0;
  wire y;
  mid dut (.a(a), .y(y));
  initial begin
    #1 $display("y=%b inner=%b", y, dut.l1.y);
    a = 1'b1;
    #1 $display("y=%b inner=%b", y, dut.l1.y);
    $finish;
  end
endmodule
