`timescale 1ns/1ps
module tb;
  reg [7:0] r = 8'h00;
  initial begin
    force r[7:4] = 4'hF;
    #1 $display("r=%h", r);
    r = 8'h12;      // ignored while forced (whole-var writes blocked)
    #1 $display("after write r=%h", r);
    release r[7:4];
    #1 $display("released r=%h", r);
    $finish;
  end
endmodule
