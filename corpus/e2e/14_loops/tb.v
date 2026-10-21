`timescale 1ns/1ps
module tb;
  integer i, j;
  reg [3:0] acc;
  initial begin
    acc = 0;
    for (i = 0; i < 4; i = i + 1) acc = acc + i[3:0];
    $display("for acc=%0d", acc);
    i = 0; j = 0;
    while (i < 5) begin
      j = j + i;
      i = i + 1;
    end
    $display("while j=%0d", j);
    acc = 0;
    repeat (3) acc = acc + 4'd2;
    $display("repeat acc=%0d", acc);
    $finish;
  end
endmodule
