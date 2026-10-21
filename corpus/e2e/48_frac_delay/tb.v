`timescale 1ns/1ps
module tb;
  initial begin
    #1.5;
    $display("t=%0t", $time);
    #0.25;
    $display("t=%0t", $time);
    $finish;
  end
endmodule
