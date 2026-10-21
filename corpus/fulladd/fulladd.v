// M2 vertical-slice fixture: combinational hierarchy (two half adders).
`timescale 1ns/1ps

module halfadd(
    input  wire a,
    input  wire b,
    output wire s,
    output wire c
);
    assign s = a ^ b;
    assign c = a & b;
endmodule

module fulladd(
    input  wire a,
    input  wire b,
    input  wire cin,
    output wire s,
    output wire cout
);
    wire s1, c1, c2;
    halfadd h1 (.a(a),  .b(b),   .s(s1), .c(c1));
    halfadd h2 (.a(s1), .b(cin), .s(s),  .c(c2));
    assign cout = c1 | c2;
endmodule
