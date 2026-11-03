// rtl2spec 自著 fixture(repo MIT):G1 觸發形,**@* 形**(R1-1 行程分類律
// 的端到端回歸錨——@* 解析為 Some(EventCtrl{edges:[]}),照「ctrl 是否
// None」二分會兩頭不靠;e2e 斷言:gaps 含 latch 類)。
module latch_trap(input wire d, input wire en, output reg q);
  always @* begin
    if (en) q = d;
  end
endmodule
