# spec: acc_pipe

- spec-format: 1
- module: __acc_pipe__acc_pipe
- port: clk input wire scalar
- port: x input wire [7:0]
- port: y input wire [7:0]
- port: out output wire [15:0]
- state: x1 [7:0] stage0 輸入捕獲暫存
- state: y1 [7:0] stage0 輸入捕獲暫存
- state: s1 [8:0] stage1 和(逐拍組合於 x1/y1)
- state: h1 [7:0] stage1 半值(s1 高 8 位切片)
- state: s2in [8:0] stage2 和輸入暫存(裝載 s1)
- state: h2in [7:0] stage2 半值輸入暫存(裝載 h1)
- state: s2 [9:0] stage2 和(逐拍組合於 h2in/s2in)
- state: scaled [15:0] stage2 縮放(逐拍組合;out 的背後暫存)

## behavior

三級流水:第 0 拍捕獲 x/y;第 1 級算 9 位和並切高 8 位;第 2 級將上一級暫存的和與半值相加得
10 位和,再拼接縮放成 16 位;第 3 拍 out 裝載縮放值。全部暫存無復位(上電 x 語義,期望以公共
TB 錄製為準)。

- B001 [timing] clk 上升沿後的下一拍,x1 裝載 x
  check(sequence): +1拍 → x1=x when -
- B002 [timing] clk 上升沿後的下一拍,y1 裝載 y
  check(sequence): +1拍 → y1=y when -
- B003 [arithmetic] s1 逐拍組合等於零擴展 x1 與 y1 的 9 位和
  check(relation): s1 ≡ {1'h0,x1} + {1'h0,y1} when -
- B004 [arithmetic] h1 逐拍組合等於 s1 的 [8:1] 位切片
  check(relation): h1 ≡ s1[8:1] when -
- B005 [timing] clk 上升沿後的下一拍,s2in 裝載 s1
  check(sequence): +1拍 → s2in=s1 when -
- B006 [timing] clk 上升沿後的下一拍,h2in 裝載 h1
  check(sequence): +1拍 → h2in=h1 when -
- B007 [arithmetic] s2 逐拍組合等於零擴展 h2in 與 s2in 的 10 位和
  check(relation): s2 ≡ {2'h0,h2in} + {1'h0,s2in} when -
- B008 [arithmetic] scaled 逐拍組合等於 {5'h00, s2, s2in[0]} 拼接
  check(relation): scaled ≡ {5'h00, s2, s2in[0]} when -
- B009 [timing] clk 上升沿後的下一拍,out 裝載 scaled 之值
  check(sequence): +1拍 → out=scaled when -

## scenarios

- SCENE1 power_on_x: 上電後僅驅動一組 x/y(無復位),觀察三拍流水後 out 的收斂值
- SCENE2 steady_5_7: x=5、y=7 穩定輸入下逐拍觀察 out(9 位和不溢出,縮放值可手算)
- SCENE3 carry_200_100: x=200、y=100 觸發 9 位和的進位路徑,切片與放大後逐位仍一致
- SCENE4 max_255_255: x=y=255 極值,9 位和與 10 位和的位寬邊界
- SCENE5 dynamic_stream: x/y 逐拍變化的數據流,驗證三級流水的逐拍錯位關係(相鄰拍輸出對應
  相差三拍的輸入組合)
- SCENE6 zero_converge: x/y 歸零後 out 逐拍收斂到 16'h0000
