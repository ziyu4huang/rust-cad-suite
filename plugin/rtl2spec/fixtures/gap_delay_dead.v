// gap_delay_dead.v — M4 正樣(自著 MIT;§5.2 S4):G8 遍歷器展開 DelayCtrl/
// Wait 後,容器內死分支命中(觸發形 `always ... #1; if(1'b0)` 即 canon 正樣形)。
module delay_dead (
  input wire a,
  output reg x
);
  always begin
    #1;
    if (1'b0) x <= a; // 恆假:then 臂不可達(G8 certain)
    else x <= 1'b1; // else 臂在場:無 latch 尾巴
  end
endmodule

module wait_dead (
  input wire a,
  output reg y
);
  always begin
    wait (a);
    if (1'b1) y <= 1'b0; // 恆真:else 臂不可達(G8 certain)
    else y <= 1'b1; // else 臂在場:無 latch 尾巴
  end
endmodule
