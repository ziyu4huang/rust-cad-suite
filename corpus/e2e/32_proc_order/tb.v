`timescale 1ns/1ps
module tb;
  reg a = 0;
  initial begin
    a = 1;
    $display("first? a=%b", a);
  end
  initial begin
    $display("second sees a=%b", a);
  end
endmodule
