`timescale 1ns/1ps
// riverilog R0 chase case: combinational UDP primitive.
primitive my_and(out, a, b);
  output out;
  input a, b;
  table
    0 ? : 0;
    ? 0 : 0;
    1 1 : 1;
  endtable
endprimitive

module tb;
  reg a, b;
  wire o;
  my_and g(o, a, b);
  initial begin
    a = 0; b = 0; #1 $display("%b", o);
    a = 1; b = 0; #1 $display("%b", o);
    a = 1; b = 1; #1 $display("%b", o);
    $finish;
  end
endmodule
