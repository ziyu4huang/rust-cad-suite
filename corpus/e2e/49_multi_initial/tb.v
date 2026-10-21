`timescale 1ns/1ps
module tb;
  initial begin
    #2 $display("A@2");
    #2 $display("A@4");
  end
  initial begin
    #1 $display("B@1");
    #4 $display("B@5");
  end
  initial begin
    #6 $display("C@6");
    $finish;
  end
endmodule
