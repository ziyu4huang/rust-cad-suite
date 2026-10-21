`timescale 1ns/1ps
module tb;
  integer i;
  initial begin
    i = -7;
    $display("-7div2=%0d -7mod2=%0d", i / 2, i % 2);
    $display("7div-2=%0d 7mod-2=%0d", i / -2, 7 % -2);
    $display("neg-div-zero-is-x=%b", 8'h05 / 8'h00 === 8'hxx);
    $finish;
  end
endmodule
