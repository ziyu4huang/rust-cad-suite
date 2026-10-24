`timescale 1ns/1ps
module tb;
  reg a, b;
  wire w_and, w_or, w_nand, w_buf;
  and g1(w_and, a, b);
  or  g2(w_or, a, b);
  nand g3(w_nand, a, b);
  buf g4(w_buf, a);
  initial begin
    a = 1'b0; b = 1'bx;
    #1 $display("and=%b or=%b nand=%b buf=%b", w_and, w_or, w_nand, w_buf);
    a = 1'bx; b = 1'b1;
    #1 $display("and=%b or=%b nand=%b", w_and, w_or, w_nand);
    $finish;
  end
endmodule
