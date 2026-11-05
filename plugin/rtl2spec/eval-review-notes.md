# R5 擴面複評筆記(§16.5;人工區,gen-eval-report.mjs 機械內聯進 RTL2SPEC-EVAL.md)

- 評審流程契約(§16.5):每檔探針題×3,先**只讀規則書**(build/eval verdict
  內嵌 rulebook JSON)作答、答案引用 rule id,後**粗讀 RTL 源檔**對照。
- provenance 注明:本複評=會話內評審(同會話讀規則書與源碼),非獨立第三人
  (S7 先例);R2 散文感抽查同批完成(見檔尾)。
- 抽樣(§16.5 分層 5 檔):頭部 1=operators(85 條 timing 密度面)/中部 2=
  fsm(18,fsm+counter)+multiplier(10,arithmetic+G3 誤報史)/尾部 2=
  subbytes(7,G1 對沖正樣+R007 when 適用域先例)+acc_pipe(7,G4+Apache 面)。

---

## operators.v(85 條,全 timing)

**探針 1:mode=='sd2 時 y 的行為?**
規則書作答:R004——`mode == 'sd2 成立後的下一拍,y 變為 s1 << s2`?否,
R004 原文=`mode == 'sd2 成立後的下一拍,y 變為 s1 << u2`(帶號左移無號量)。
粗讀對照:operators.v:16 `2: y <= s1 << u2`——**一致**(R004 與源行 16 對應)。

**探針 2:每拍 y 的「無條件基底值」是什麼,由哪條規則承載?**
規則書作答:R001——`clk 上升沿 成立後的下一拍,y 變為 8'h42`(無條件
statement,無 when 修飾);各 mode 分支規則(R002..)是**條件面**,基底
先行被分支覆蓋=case 優先語義在規則書中以「statement 有無條件前綴」可辨。
粗讀對照:operators.v:12 `y <= 8'h42;` 先於 case——**一致**。

**探針 3:mode 位寬與分支覆蓋率的關係?規則書是否如實暴露覆蓋缺口?**
規則書作答:mode 為 `input [6:0]`(R002..R0xx 條件面含 `mode == 'sd0..`),
規則數 85;mode 全域 0..127 共 128 編碼,規則僅覆蓋 case 顯式列出的分支
(8 個 shift 運算×8 有號組合=64+缺省)——規則書**未**宣告「未覆蓋編碼」
(那是 G2 incomplete_case 破口掃描的職責,不在 rules 模式)。結論=誠實
分工,非漏報。
粗讀對照:case 分支數與 R 條件數一致;mode [6:0]=128 編碼面,**一致**。

評審結論:頭部密度檔(85 條)的規則陳述逐條可對源行;timing 類 statement
格式統一(`條件 成立後的下一拍,<reg> 變為 <expr>`),散文感低、可機檢。

---

## fsm.v(18 條:fsm 5+timing 12+counter 1)

**探針 1:state='sd100 時兩個輸入條件各導向何態?**
規則書作答:R001(button_a && !button_b → 'sd200)、R002
(!button_a && button_b → 'sd300)。
粗讀對照:fsm.v case 100 分支兩個 if——**一致**。

**探針 2:'sd200 → 'sd210 的轉移條件?附屬效果?
規則書作答:R003(state='sd200 且 cnt == 'sd5 → 下一拍 state='sd210);
同態另有 R012 條件時序效果(red_a/green_a 在 state=='sd200 下的賦值)。
粗讀對照:fsm.v case 200:`cnt<=cnt+1; if (cnt==5) state<=210; red_a<=0;
green_a<=1`——**一致**(R003 只述轉移,同態效果由 R012 系分承載=規則
粒度單一職責,非漏)。

**探針 3:reset 是同步還是異步?規則書答得出嗎?**
規則書作答:R011=`reset 成立後的下一拍,state 變為 'sd100`——「下一拍」
措辭隱含同步;但**單看規則書無法確證** sensitivities(異步 posedge reset
在 evidence 行為類 span 上,不在 statement 文字)。
粗讀對照:`ifdef ASYNC_RESET` 默認未定義→`always @(posedge clk)`=同步;
**規則書作答(隱含同步)與源一致,但確證需讀 evidence 或源碼**——規則書
陳述粒度的已知邊界,評審按 certain/heuristic 差異化採信的適用例。

評審結論:fsm 類轉移規則忠實;counter(cnt)效果分佈在 timing 類;同步/
異步覆位面=陳述文字不承載、evidence 承載——下游消費者須連 evidence 讀,
文檔可注記(非本輪阻塞)。

---

## multiplier.v(10 條,全 arithmetic;G3 誤報史檔)

**探針 1:全加器的輸出關係式?**
規則書作答:FullAdder.R001——`{Cout, S} ≡ A + B + Cin`(拼接 LHS 表達
雙輸出)。
粗讀對照:multiplier.v FullAdder `assign {Cout,S} = A+B+Cin`——**一致**
(R6 拼接 LHS 驅動圖誤報修復後,該形規則化正常、G3 不誤報)。

**探針 2:最終積 P 取自哪個部分積行(2D 版)?**
規則書作答:Multiplier_2D.R002——`P ≡ PP[N - 'sd1]`。
粗讀對照:2D 生成器的末行部分積即 P——**一致**。

**探針 3:gA/gB 的位形中移位量與選擇子關係?**
規則書作答:R003=`gA ≡ {A & M{B['sd0]}, i{1'b0}}`、R004=
`gB ≡ PP[(i - 'sd1) * (i + 'sd2 * M) / 'sd2 +:: M + i]`——部分積門控
`A&M{B[i]}`(符號擴展門控)+移位拼接;R004 的 +:: 部分選擇寬=M+i。
粗讀對照:生成器參數化拼接與規則書組合表達式同形;**表達式為機械轉寫
(範圍索引轉寫為算式)**,可讀性受 generate 參數化影響=已知換算面,
判定忠實但散文感高於行為類。

評審結論:arithmetic 類對 generate 生成器的組合關係轉寫正確;G3 誤報史
檔零破口誤報(eval per-file 破口列=0);拼接 LHS 規則化=誤報修復的回歸
錨在場。

---

## subbytes.v(7 條:reset 3+timing 3+arithmetic 1;G1 對沖正樣+when 先例)

**探針 1:復位時哪些寄存器裝載什麼值?**
規則書作答:R001(data_reg←'sd0)、R002(state←'sd0)、R003(ready_o←
'sd0),三條均 `當 !reset 且 clk 上升沿`。
粗讀對照:always @(posedge clk) if (!reset) begin 三個賦值——**一致**。

**探針 2:next_state 何時等於 state+1?適用域?**
規則書作答:R007——`當 !(state) 時,next_state ≡ state + 'sd1`(逐拍
組合);when 適用域=`!(state)`(分支域轉寫,非全域恆等)。
粗讀對照:subbytes.v:72-77 `end else begin ... next_state = state+1;`
=僅 state==0 臂——**一致**(when 適用域把分支限定轉寫進規則,避免
「全域誤讀」=R3-② 先例成立)。

**探針 3:本檔為何無 G1 latch 破口?規則書+破口兩模式合併可答嗎?**
規則書作答:時序行程(posedge)+組合行程(next_* 右值鏈)分立;破口模式
(gaps)對本檔報告=0——組合行程的 data_reg_var 等中間變量有塊首頂層
賦值(data_reg_var[0..3]=data_reg 切片),對沖成立。
粗讀對照:組合 always 頂層無條件賦值在前、分支在後——**一致**(G1 塊首
默認賦值對沖判準的實檔應用;subbytes 正是 R3-① 收平輪誤報形來源檔)。

評審結論:when 適用域與 reset 類格式均正確;規則+破口兩模式合併讀=
「為何零破口」可從規則書部分推出(塊首賦值),破口掃描確認——兩模式
分工閉合。

---

## acc_pipe.v(7 條:timing 5+arithmetic 2;G4+Apache 面)

**探針 1:第一級流水寄存了什麼?**
規則書作答:R001/R002——`x__input_flop ← x`、`y__input_flop ← y`(clk
上升沿下一拍)。
粗讀對照:acc_pipe.v stage0 兩個非阻塞賦值——**一致**。

**探針 2:p2_add_98_comb 的兩個加數與位寫法?**
規則書作答:R006——`p2_add_98_comb ≡ {2'h0, p0_bit_slice_48} +
{1'h0, p0_add_20}`(逐拍組合)。
粗讀對照:stage2 `assign p2_add_98_comb = {2'h0, p0_bit_slice_48} +
{1'h0, p0_add_20};`——**逐字一致**(拼接零擴展位寫原樣保留)。

**探針 3:輸出寄存器的驅動鏈(三級)?**
規則書作答:R007(out__output_flop←p2_scaled_comb)+R003/R005/R006 組合
鏈(p1_add_96_comb 由 x/y__input_flop;p0 兩寄存器←p1 組合;p2_scaled_comb
含 p2_add_98_comb)——鏈可從 7 條規則重建。
粗讀對照:stage0→stage1→stage2 三級,**一致**;G4 破口面=本檔 3 條
missing_reset(eval per-file 破口列 G4×3,寄存器均無復位)——與 xls
生成器無復位風格一致,破口報告如實。

評審結論:行為類鏈路規則忠實且零改寫;G4×3 破口=設計風格而非提取缺陷
(xls 生成器無復位),按 confidence=certain 差異化採信後仍屬真實測試缺口。

---

## R2 散文感抽查(§16.5;同批完成)

- i2c_master 深層 else-if 條件鏈(corpus/e2e/59):陳述含忠實噪音
  (`i2c_master_top.R0xx` 系列 when 修飾鏈),接受基準(S7 先例)——
  本輪 5 檔同基準:組合表達式轉寫(generate 參數化)散文感最高但判定
  忠實;行為類 timing/guard 格式最乾淨。
- 整體:陳述全部引用源碼真標識符(V5-④ 機檢在場),無「描述性散文」
  空轉規則;九類類別分布見報告類別分布表(R4 素材)。
