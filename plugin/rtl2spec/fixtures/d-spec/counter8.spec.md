# spec: counter8

- spec-format: 1
- module: counter
- param: WIDTH = 8
- port: clk input wire scalar
- port: rst input wire scalar
- port: q output reg [WIDTH-1:0]

## behavior

- B001 [reset] 同步復位:clk 上升沿且 rst 成立時,下一拍 q 裝載全 0(WIDTH 位)
  check(guard): trigger=clk posedge; when=rst; async=false; effects=q←{WIDTH{1'b0}}
- B002 [count] clk 上升沿且 !rst 時,下一拍 q 為原值加一(按 WIDTH 位自然環繞:WIDTH'hFF → WIDTH'h00)
  check(sequence): +1拍 → q=(q+1'b1) when !rst

## scenarios

- SCENE1 power_on_no_reset: 上電後未經復位即時鐘推進(初值不定,期望以公共 TB 錄製為準),觀察逐拍計數行為
- SCENE2 sync_reset: 計數中途 rst 拉高一個時鐘沿,該沿後 q=0(同步語義:復位發生在時鐘沿,非即時)
- SCENE3 count_after_release: 復位釋放後逐拍 +1
- SCENE4 mid_count_reset: 計到中間值時復位,之後再計數須從 0 重新遞增
- SCENE5 wrap: 計到 8'hFF 後下一拍環繞回 8'h00
- SCENE6 hold_reset_then_count: rst 持續期間 q 恒為 0,釋放後恢復 +1 計數
