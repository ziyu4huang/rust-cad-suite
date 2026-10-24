`timescale 1ns/1ps
// riverilog R0 chase case: fork/join concurrency with deterministic delays.
module tb;
  initial begin
    fork
      begin #5 $display("A@5"); end
      begin #2 $display("B@2"); end
      $display("C@0");
    join
    $display("joined");
    #1 $finish;
  end
endmodule
