// rel_positive.v — S2(N3)relation 產出放寬正樣(自著 MIT;RTL2SPEC-MVP-NEXT §5.2 S2)。
// 斷言規格(手寫合法,與 sv_rules.rs s2_rel_boundary_* 單測/gap-fixture-check.mjs
// S2 規則面行同源;golden=運行輸出快照禁手寫,兩者劃界):
//   結構形 RHS(PartSelect/Concat)分支域外一律產;純常量僅分支域(case/if 臂)內產。
module rel_positive (
    input  wire [7:0] a,
    input  wire [7:0] b,
    input  wire       sel,
    output wire [4:0] slice,
    output wire [4:0] scaled,
    output reg  [3:0] arm_c
);
  // 結構形:PartSelect(位切片;acc_pipe p1_bit_slice_103_comb=[8:1] 右移同款)。
  assign slice = a[7:3];
  // 結構形:Concat(拼接;acc_pipe p2_scaled_comb 同款)。
  assign scaled = {1'b0, b[7:3]};
  always @* begin
    case (sel)
      1'b0: arm_c = 4'd1; // 純常量@分支域:產(when=sel == 1'b0)
      default: arm_c = 4'd2; // default 臂 conds 空:不產(S2;default 臂條件合成=S4/M2)
    endcase
  end
endmodule
