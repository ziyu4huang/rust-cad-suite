`timescale 1ns/1ps
module tb;
  reg clk = 0, done = 0;
  reg [3:0] cnt = 0;
  always #5 clk = ~clk;
  task pulse(input integer n);
    integer k;
    begin
      for (k = 0; k < n; k = k + 1) begin
        @(posedge clk);
        cnt = cnt + 1;
      end
    end
  endtask
  initial begin
    pulse(3);
    done = 1;
    #1;
    $display("cnt=%0d done=%b", cnt, done);
    $finish;
  end
endmodule
