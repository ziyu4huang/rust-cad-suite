`timescale 1ns/1ps
module tb;
  reg clk = 0;
  integer n = 0;
  always #5 clk = ~clk;
  initial begin
    forever begin
      @(posedge clk);
      n = n + 1;
      if (n == 4) begin
        $display("n=%0d", n);
        $finish;
      end
    end
  end
endmodule
