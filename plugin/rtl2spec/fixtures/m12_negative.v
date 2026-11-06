// m12_negative.v — M1/M2 負樣(自著 MIT;§5.2 S4):單標簽臂不增殖(M1 負形)
// +無 default 不產 default 臂規則(M2 負形:條件不可導,保持無條件行走)。
module m1neg (
  input  wire       clk,
  input  wire [1:0] sel,
  output reg        y
);
  // M1 負形:單標簽臂=每臂恰 1 條(無乘法增殖)。
  always @(posedge clk)
    case (sel)
      2'b00: y <= 1'b1;
      2'b01: y <= 1'b0;
      2'b10: y <= 1'b1;
      2'b11: y <= 1'b0;
    endcase
endmodule

module m2neg (
  input  wire       clk,
  input  wire [1:0] sel,
  output reg        z
);
  // M2 負形:無 default(臂 2/4 覆蓋)——無 default 臂規則可產。
  always @(posedge clk)
    case (sel)
      2'b00: z <= 1'b1;
      2'b01: z <= 1'b0;
    endcase
endmodule
