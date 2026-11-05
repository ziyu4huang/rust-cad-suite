// rtl2spec 拓展性實證 fixture(§17/S13;repo MIT):G8 dead_branch 觸發形,
// 零共報設計(塊首默認賦值防 G1 latch/無 case/單驅動/無環/組合行程無 G4)。
// expected findings(§16.2 集合相等斷言形):
//   dead_branch      → [G8-dead_branch-001]      certain「恆假,then 臂不可達」
//   dead_branch_else → [G8-dead_branch_else-001] certain「恆真,else 臂不可達」
module dead_branch(input wire a, output reg y);
  always @* begin
    y = a;
    if (1'b0) begin
      y = ~a;
    end
  end
endmodule

module dead_branch_else(input wire b, input wire c, output reg z);
  always @* begin
    z = b;
    if (1'b1) begin
      z = c;
    end else begin
      z = b;
    end
  end
endmodule
