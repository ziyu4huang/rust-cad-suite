`timescale 1ns/1ps
module tb;
  event go;
  integer n = 0;
  initial begin
    -> go;              // lost: waiter not yet at @
    #1 -> go;           // received
    #1 $display("n=%0d", n);
    $finish;
  end
  initial begin
    @(go); n = n + 1;   // waits for the SECOND trigger only
  end
endmodule
