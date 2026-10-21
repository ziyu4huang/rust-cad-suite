module counter #(
    parameter WIDTH = 4
) (
    input  wire             clk,
    input  wire             rst_n,
    output wire [WIDTH-1:0] count
);
    reg [WIDTH-1:0] cnt;
    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) cnt <= {WIDTH{1'b0}};
        else        cnt <= cnt + 1'b1;
    end
    assign count = cnt;
endmodule
