// counter8 — M5 acceptance fixture (PLAN §5): 8-bit counter, sync reset.
// Oracle-verified against Verilator 5.050 (see corpus/oracle-fixtures/README.md).
`timescale 1ns/1ps

module counter #(
    parameter WIDTH = 8
) (
    input  wire             clk,
    input  wire             rst,
    output reg  [WIDTH-1:0] q
);

    always @(posedge clk) begin
        if (rst) q <= {WIDTH{1'b0}};
        else     q <= q + 1'b1;
    end

endmodule
