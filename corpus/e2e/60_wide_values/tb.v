`timescale 1ns/1ps
// M8 multi-limb (ADR-0008): wide arithmetic/formatting across the 128-bit
// limb boundary. Race-free pattern: drive regs, #1, then display.
module tb;
  reg [255:0] a, b;
  reg signed [199:0] s;
  reg [511:0] p;
  initial begin
    a = 256'h00000000000000000000000000000000_ffffffffffffffff_ffffffffffffffff;
    b = 256'h00000000000000000000000000000001_0000000000000000_0000000000000001;
    s = -200'sd5;
    #1;
    $display("sum=%h", a + b);
    $display("sumd=%0d", a + b);
    $display("prod=%h", a * b);
    $display("quot=%h", a / 256'hffff);
    $display("remo=%h", a % 256'hffff);
    $display("shl64=%h", a << 64);
    $display("shr64=%h", a >> 64);
    $display("part=%h", a[195:130]);
    $display("sgn=%0d", s);
    $display("sgndiv=%0d", s / 200'sd3);
    $display("sgnrem=%0d", s % 200'sd3);
    $display("sgt=%b", s > 200'sd0);
    $display("wide_pow=%h", 256'h2 ** 200'sd7);
    p[255:0] = a;
    p[511:256] = 256'h0000_00ad;
    $display("p512=%h", p);
    $display("clog2=%0d", $clog2(a));
    $display("hex=%h dec=%0d", a, a);
    $finish;
  end
endmodule
