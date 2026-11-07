// rst_first_loadsig.v — D-r1c 零漂移釘(自著 MIT;§19.2 C-S1):頂層 If 條件為單訊號
//   `load`,但 load 不在敏感列表 → 復位候選=load、select_trigger 取首個 ≠load 的邊沿
//   =首沿 rst_n negedge(與修前同)→ 照產 guard×1 when=load、async=false、
//   trigger=rst_n negedge(修前修後同;本修法不改此類)。破口面零(修前修後同)。
module rst_first_loadsig (
  input  wire clk,
  input  wire rst_n,
  input  wire load,
  input  wire d,
  output reg  q
);
  always @(negedge rst_n or posedge clk)
    if (load) q <= 1'b0;
    else q <= d;
endmodule
