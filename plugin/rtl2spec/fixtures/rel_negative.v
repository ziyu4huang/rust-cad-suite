// rel_negative.v — S2(N3)負樣(RTL2SPEC-MVP-NEXT §5.2 S2;A12/Daikon 教訓釘死形)。
// 無條件直賦兩形不產 relation(緊湊性保留,防「什麼都報」):
//   ①純常量直賦(assign y = 8'h00)②直通別名(assign z = x)
//   ③行程內無條件純常量(conds 空=非分支域)。
module rel_negative (
    input  wire [7:0] x,
    output wire [7:0] y,
    output wire [7:0] z,
    output reg  [3:0] w
);
  assign y = 8'h00;
  assign z = x;
  always @* w = 4'd0;
endmodule
