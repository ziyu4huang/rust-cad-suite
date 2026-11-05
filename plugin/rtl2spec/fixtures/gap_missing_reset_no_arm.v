// rtl2spec 自著測試語料(repo MIT):G4 無復位臂正樣(§16.2 矩陣新增檔)。
// 期望 findings 全集=恰 1 條 missing_reset(certain,message 含
// 「2 個寄存器上電值未定義」,suggested_test 逐字含字典序首寄存器 ra——
// F1a 有序取樣釘死,gaps.rs 無復位臂分支 regs.sort() 後取首)。
// 零共報設計:單行程單邊沿驅動=無 multi_driver(G6);時序行程=無 latch(G1);
// 無阻塞寫=無 blocking_in_seq(G7);無 case/組合賦值=無 G2/G3/G5;
// 無 if=無 G8 → 整份 gaps JSON 可入 golden(§17.2 步 5 同款)。
module gap_missing_reset_no_arm(input wire clk, input wire d, output reg ra, output reg rb);
  always @(posedge clk) begin
    ra <= d;
    rb <= ~ra;
  end
endmodule
