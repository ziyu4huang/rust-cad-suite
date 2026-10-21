`timescale 1ns/1ps
module tb;
  initial begin
    $display("%5d|%05d|", 42, 42);
    $display("%c%c %s 100%%", 8'd72, 8'd73, "ok");
    $display("tab[\t]nl-ends-line");
    $write("no");
    $write("newline");
    $display("");
    $finish;
  end
endmodule
