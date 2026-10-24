`timescale 1ns/1ps
// riverilog R0 chase case: stratified event queue — active reads the old
// value across an NBA, #0 (inactive) still reads it, settled at #1.
module tb;
  reg a = 0;
  initial a <= 1;
  initial begin
    $display("t0 active sees a=%b", a);
    #0 $display("post-#0 sees a=%b", a);
    #1 $display("t1 sees a=%b", a);
    $finish;
  end
endmodule
