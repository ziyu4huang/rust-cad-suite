`timescale 1ns/1ps
module tb;
  reg [7:0] mem = 8'hE7;
  reg [7:0] rev = 8'hA5;
  reg [2:0] idx = 3'd2;
  initial begin
    $display("hi=%h lo=%h", mem[7:4], mem[3:0]);
    $display("bit2=%b bit6=%b", mem[2], mem[6]);
    $display("var_bit=%b", mem[idx]);
    $display("rev0=%b rev7=%b", rev[0], rev[7]);
    mem[5:4] = 2'b00;
    #1 $display("mod=%h", mem);
    $finish;
  end
endmodule
