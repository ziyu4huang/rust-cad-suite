`timescale 1ns/1ps
module tb;
  reg [7:0] hmem [0:7];
  reg [3:0] bmem [0:5];
  integer k;
  initial begin
    for (k = 0; k < 8; k = k + 1) hmem[k] = 8'h00;
    $readmemh("data.hex", hmem);
    $readmemb("data.bin", bmem);
    $display("h=%h %h %h %h %h %h %h %h", hmem[0], hmem[1], hmem[2], hmem[3], hmem[4], hmem[5], hmem[6], hmem[7]);
    $display("b=%b %b %b %b %b %b", bmem[0], bmem[1], bmem[2], bmem[3], bmem[4], bmem[5]);
  end
  initial begin
    $readmemh("data2.hex", hmem, 5, 6);
    #1 $display("h2=%h %h", hmem[5], hmem[6]);
    $finish;
  end
endmodule
