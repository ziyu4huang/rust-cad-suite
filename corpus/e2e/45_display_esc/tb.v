`timescale 1ns/1ps
module tb;
  initial begin
    $display("A\tB");
    $display("65=%c", 8'd65);
    $display("100%% \\done/");
    $finish;
  end
endmodule
