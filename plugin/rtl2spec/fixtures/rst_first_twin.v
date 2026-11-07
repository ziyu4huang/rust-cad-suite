// rst_first_twin.v — D-r1c 孿生(自著 MIT;§19.2 C-S1):rst_first_async.v 的時鐘前置序。
//   `always @(posedge clk or negedge rst_n)`:修前修後皆 guard×1、async=true、
//   trigger=clk posedge;修後與正樣 check 載荷相等——evidence[1](敏感列表原文)
//   與行號必然不同(順序不變性的比較面=check,不是 evidence)。破口面修前修後皆零。
module rst_first (
  input  wire clk,
  input  wire rst_n,
  input  wire d,
  output reg  q
);
  always @(posedge clk or negedge rst_n)
    if (!rst_n) q <= 1'b0;
    else q <= d;
endmodule
