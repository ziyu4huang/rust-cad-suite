`timescale 1ns/1ps
module tb;
  reg [3:0] u = 4'b1010;
  initial begin
    $display("signed_view=%0d", $signed(u));
    $display("sgt=%b ugt=%b", $signed(u) > 0, u > 0);
    $display("neg=%0d", -$signed(u));
    $display("mix_cmp=%b", $signed(u) > -1);
    $finish;
  end
endmodule
