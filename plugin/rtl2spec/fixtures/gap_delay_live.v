// gap_delay_live.v — M4 負樣(自著 MIT;§5.2 S4):DelayCtrl/Wait 容器內
// 條件為可變表達式(非純字面量)=零死分支(誤報紅路面)。
module delay_live (
  input wire a,
  output reg x
);
  initial begin
    x = 1'b0;
    #1;
    if (a) x = 1'b1;
  end
endmodule

module wait_live (
  input wire a,
  output reg y
);
  initial begin
    y = 1'b1;
    wait (a) y = 1'b0;
  end
endmodule
