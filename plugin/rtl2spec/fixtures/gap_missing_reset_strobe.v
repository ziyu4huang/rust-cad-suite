// gap_missing_reset_strobe.v — M5 正負同檔(自著 MIT;§5.2 S4):
//   mr_data  =數據承載/條件賦值寄存器不在復位臂 effect 集 → missing_reset certain(真缺口);
//   mr_strobe=無條件純常量逐拍清除(strobe,無狀態保持)→ M5 約束不報(誤報形紅路面)。
module mr_data (
  input  wire clk,
  input  wire rst_n,
  input  wire d,
  output reg  hold
);
  reg acc;
  always @(posedge clk or negedge rst_n)
    if (!rst_n) acc <= 1'b0;
    else if (d) hold <= d; // hold 不在復位臂 → 報(certain)
endmodule

module mr_strobe (
  input  wire clk,
  input  wire rst_n,
  input  wire d,
  output reg  pulse
);
  always @(posedge clk or negedge rst_n)
    if (!rst_n) pulse <= 1'b0;
    else begin
      pulse <= 1'b0; // 無條件純常量逐拍清除=無狀態保持 → 不報(M5 約束)
      if (d) pulse <= 1'b1;
    end
endmodule
