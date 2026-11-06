// gap_case_covered.v — M6 正負同檔(自著 MIT;§5.2 S4):
//   case_covered = 4 標簽覆蓋 2-bit 全集(4/4)無 default → M6 守衛不報(誤報形);
//   case_sparse  = 1 標簽覆蓋 1/4 無 default → 仍報 incomplete_case heuristic(真缺口)。
module case_covered (
  input  wire [1:0] sel,
  output reg        y
);
  always @* begin
    case (sel)
      2'b00: y = 1'b1;
      2'b01: y = 1'b0;
      2'b10: y = 1'b1;
      2'b11: y = 1'b0;
    endcase
  end
endmodule

module case_sparse (
  input  wire [1:0] sel,
  output reg        z
);
  always @* begin
    case (sel)
      2'b00: z = 1'b1;
    endcase
  end
endmodule
