`timescale 1ns/1ps
module tb;
  initial begin
    #23;
    $display("t=%0t ns=%0d", $time, $time);
    #7;
    $display("t=%0t ns=%0d", $time, $time);
    $finish;
  end
endmodule
