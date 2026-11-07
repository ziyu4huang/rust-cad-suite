// tb1_parametric.v — tb_plan TB1 負樣(自著 MIT;§19.8 mcp-e2e ⑩;硬錯 ④):
//   端口寬度依參數(contract parametric=true,無數值寬度,§11.5 求值禁區)
//   → `rtl_tb_plan` 必 isError 且含 `milestone TB1`。
module tb1_parametric #(parameter WIDTH = 8) (
  input  wire             clk,
  input  wire [WIDTH-1:0] d,
  output reg  [WIDTH-1:0] q
);
  always @(posedge clk) q <= d;
endmodule
