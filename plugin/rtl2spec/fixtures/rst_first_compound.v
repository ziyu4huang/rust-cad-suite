// rst_first_compound.v — D-r1c 負樣(自著 MIT;§19.2 C-S1):雙沿敏感列表,但頂層
//   If 條件為非單訊號形的複合式 `load && en` → reset_cond_signal=None → 無復位候選、
//   零 guard(修前修後同)。破口面:無單訊號復位臂=結構可證 → G4 certain(修前修後同)。
module rst_first_compound (
  input  wire clk,
  input  wire rst_n,
  input  wire load,
  input  wire en,
  input  wire d,
  output reg  q
);
  always @(negedge rst_n or posedge clk)
    if (load && en) q <= 1'b0;
    else q <= d;
endmodule
