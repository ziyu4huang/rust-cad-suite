`timescale 1ns/1ps
module tb;
  genvar i, j;
  wire [3:0] grid;
  wire [1:0] sel = 2'b10;
  generate
    for (i = 0; i < 4; i = i + 1) begin : row
      for (j = 0; j < 1; j = j + 1) begin : col
        assign grid[i] = (i == sel) | (i == 3);
      end
    end
  endgenerate
  initial begin
    #1 $display("grid=%b", grid);
    $finish;
  end
endmodule
