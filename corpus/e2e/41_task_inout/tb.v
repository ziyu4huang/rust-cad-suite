`timescale 1ns/1ps
module tb;
  reg [7:0] v = 8'd10;
  task twice(inout [7:0] x);
    x = x + x;
  endtask
  initial begin
    twice(v);
    #1 $display("v=%0d", v);
    twice(v);
    #1 $display("v=%0d", v);
    $finish;
  end
endmodule
