`timescale 1ns/1ps
// R5 mining: dump controls — off parks changes (x section), on resumes
// with a current-value section, all snapshots the whole var set.
module tb;
  reg a = 0;
  reg [3:0] b = 4'h0;
  initial begin
    $dumpfile("dump.vcd");
    $dumpvars(0, tb);
    #5 a = 1;
    $dumpoff;
    #5 b = 4'h3;   // not recorded (dump off)
    $dumpon;
    #5 a = 0;
    $dumpall;
    #1 $finish;
  end
endmodule
