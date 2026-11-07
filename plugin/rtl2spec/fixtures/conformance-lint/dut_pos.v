// dut_pos.v — TB-R1x 向量 DUT(自著 MIT):正沿採樣 d。
module cl_dut (
  input  wire clk,
  input  wire d,
  output reg  q
);
  always @(posedge clk) q <= d;
endmodule
