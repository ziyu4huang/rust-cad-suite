// dut_neg.v — TB-R1x 向量 DUT(自著 MIT):負沿採樣 d。
module cl_dut (
  input  wire clk,
  input  wire d,
  output reg  q
);
  always @(negedge clk) q <= d;
endmodule
