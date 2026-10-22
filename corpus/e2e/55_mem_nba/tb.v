`timescale 1ns/1ps
module tb;
  reg clk = 0;
  reg [7:0] mem [0:3];
  reg [1:0] addr;
  reg [7:0] rdata;
  integer i;
  always #5 clk = ~clk;
  always @(posedge clk) mem[addr] <= mem[addr] + 8'h11;
  initial begin
    for (i = 0; i < 4; i = i + 1) mem[i] = 8'h00;
    addr = 2'd0;
    repeat (6) begin
      @(posedge clk);
      #1 addr = addr + 2'd1;
    end
    #1 $display("m=%h %h %h %h", mem[0], mem[1], mem[2], mem[3]);
    $finish;
  end
endmodule
