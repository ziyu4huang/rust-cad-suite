// Blind reimplementation from inline rulebook spec (corpus/counter8/counter.v).
// Rules covered:
//   counter.R001 [reset]    : posedge clk & rst -> q <= {WIDTH{1'b0}}   (synchronous reset)
//   counter.R002 [counter]  : posedge clk & !rst -> q <= q + 1'b1       (count / wrap)
// Everything outside these two rules (port list shape, WIDTH default,
// power-on value of q) is unspecified by the rulebook; see experiment
// assumptions accompanying this file.

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
