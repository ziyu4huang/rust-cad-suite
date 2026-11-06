// m1_min_case.v — M1/M2/M3 一檔正樣(自著 MIT;S4 遷入 plugin/rtl2spec/fixtures/
// ——原住 build/mvp-exp/audit/(gitignored 實驗語料,弧後清理即丟);§5.2 S4)。
// 驗收斷言(M1 樣規則含 2'b01 觸發域;M2 樣產 default 臂規則):
//   M1: 2'b00,2'b01 多標簽臂逐標簽展開(s == 2'b00 / s == 2'b01 兩條);
//   M2: default 臂=未被覆蓋域(!(s == 2'b00) 且 !(s == 2'b01) 且 !(s == 2'b10));
//   M3: else 臂基條件 !(rst)。
module m1min (
  input  wire       clk,
  input  wire       rst,
  input  wire [1:0] din,
  output reg        y
);
  reg [1:0] s;

  always @(posedge clk)
    if (rst) begin
      y <= 1'b0;
      s <= 2'b00;
    end else
      case (s)
        2'b00, 2'b01: y <= 1'b1;
        2'b10:        y <= din[0];
        default:      y <= 1'b1;
      endcase
endmodule
