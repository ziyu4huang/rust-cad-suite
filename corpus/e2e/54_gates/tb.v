`timescale 1ns/1ps
module tb;
  reg a, b, en;
  wire w_and, w_or, w_xor, w_xnor, w_nand, w_nor, w_not, w_buf, w_bufif;
  and g1(w_and, a, b);
  or  g2(w_or, a, b);
  xor g3(w_xor, a, b);
  xnor g4(w_xnor, a, b);
  nand g5(w_nand, a, b);
  nor g6(w_nor, a, b);
  not g7(w_not, a);
  buf g8(w_buf, a);
  bufif1 g9(w_bufif, a, en);
  initial begin
    a = 1'b1; b = 1'b0; en = 1'b1;
    #1 $display("and=%b or=%b xor=%b xnor=%b nand=%b nor=%b not=%b buf=%b bufif=%b",
                w_and, w_or, w_xor, w_xnor, w_nand, w_nor, w_not, w_buf, w_bufif);
    a = 1'b1; b = 1'b1;
    #1 $display("and=%b xor=%b", w_and, w_xor);
    en = 1'b0;
    #1;
    $finish;
  end
endmodule
