// counter8 — 盲寫凍結產物(D-S5;REIMPL-PROTOCOL §5)。
// 來源=counter8.spec.md 全文(spec-format:1):module counter;param WIDTH=8;
// B001 同步復位(rst→q←{WIDTH{1'b0}});B002 !rst 時逐拍 +1(WIDTH 位自然環繞)。
// 上電初值=不定(spec SCENE1:初值不定,以公共 TB 錄製為準;禁自加初始塊)。
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
