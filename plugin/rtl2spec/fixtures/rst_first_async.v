// rst_first_async.v — D-r1c 正樣(自著 MIT;§19.2 C-S1):復位前置敏感列表。
//   `always @(negedge rst_n or posedge clk)` 首沿=復位沿;觸發應取「首個 ≠ 復位
//   候選訊號」的邊沿(clk posedge),復位臂=guard×1、async=true。
//   修前(首沿即觸發)=零 guard(復位臂被當普通分支,sequence×2)——bug 現形;
//   破口面修前報 G4 missing_reset certain(假陽性),修後零 G4。
//   孿生=rst_first_twin.v(時鐘前置序;check 載荷應相等,evidence 文本/行號不同)。
module rst_first (
  input  wire clk,
  input  wire rst_n,
  input  wire d,
  output reg  q
);
  always @(negedge rst_n or posedge clk)
    if (!rst_n) q <= 1'b0;
    else q <= d;
endmodule
