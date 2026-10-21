`timescale 1ns/1ps
module tb;
  reg [1:0] s = 2'b10;
  reg [1:0] z = 2'b1?;
  function [3:0] cls(input [1:0] v);
    begin
      casez (v)
        2'b1?: cls = 4'd1;
        2'b01: cls = 4'd2;
        default: cls = 4'd0;
      endcase
    end
  endfunction
  initial begin
    $display("cz=%0d", cls(s));
    $display("cz1=%0d", cls(2'b11));
    $display("cz0=%0d", cls(2'b00));
    case (s)
      2'b10: $display("plain-hit");
      default: $display("plain-miss");
    endcase
    $finish;
  end
endmodule
