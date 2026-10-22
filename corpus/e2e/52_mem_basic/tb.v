`timescale 1ns/1ps
module tb;
  reg [7:0] mem [0:3];
  reg [7:0] mem_hi [8:11];
  reg [1:0] ix;
  initial begin
    mem[0] = 8'hA0; mem[1] = 8'hB1; mem[2] = 8'hC2; mem[3] = 8'hD3;
    mem_hi[9] = 8'h99;
    ix = 2'd2;
    #1 $display("m=%h %h %h %h ix=%h hi9=%h", mem[0], mem[1], mem[2], mem[3], mem[ix], mem_hi[9]);
    ix = 2'd3;
    #1 $display("ix3=%h", mem[ix]);
    $finish;
  end
endmodule
