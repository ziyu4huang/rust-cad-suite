// rst_only_edge.v — D-r1c 負樣(自著 MIT;§19.2 C-S1):單沿「純復位行程」。
//   `always @(negedge rst_n) if (!rst_n) ...`:敏感列表無 ≠復位候選的邊沿 → 回退首沿
//   → 觸發=rst_n 自身 → is_trigger → 零 guard(修前修後同,舊行為釘死)。
//   破口面 G4 missing_reset certain(修前修後同;零漂移釘≠正確性背書,§19.13-7 待決)。
module rst_only_edge (
  input  wire rst_n,
  output reg  q
);
  always @(negedge rst_n)
    if (!rst_n) q <= 1'b0;
endmodule
