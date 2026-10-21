`timescale 1ns/1ps
module tb;
  reg [7:0] acc;
  task addv(input [7:0] v, output [7:0] o);
    o = acc + v;
  endtask
  initial begin
    acc = 8'd10;
    addv(8'd5, acc);
    #1;
    $display("acc=%0d", acc);
    $finish;
  end
endmodule
