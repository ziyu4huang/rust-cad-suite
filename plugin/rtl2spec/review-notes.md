# 評審批注(S7 定性評審;gen-report.mjs 機械內聯進 validation-report.md)

> 方法:R5 探針題按「**先只讀規則書作答,後粗讀 RTL 對照**」順序執行,答案
> 引用 rule id;R2 散文感抽查跨檔抽樣。評審 provenance=本實作會話內評審
> (非獨立第三人;獨立複審可重跑兩閘後在本檔追加,gen-report 重新內聯)。

## R5 探針問題×3/代表檔

### acc_pipe(corpus/e2e/58,Apache-2.0)

- Q1:out 在第幾拍後反映 x 與 y 的組合結果?流程是什麼?
  A:三級流水,3 拍後有效:x/y 先進輸入級(R001/R002)→ p1_add_96_comb 組合
  相加(R003)→ 下一拍裝載 p0_add_20 與 p0_bit_slice_48(R004/R005)→
  p2_add_98_comb 組合相加(R006)→ 下一拍裝載 out(R007)。
  對照粗讀:與 stage0/1/2 註釋結構逐段一致。✓
- Q2:p0_bit_slice_48 裝載的是什麼?
  A:依 R005,是 p1_bit_slice_103_comb 的下一拍值;結合 R003(9-bit 和)可推
  切片語義,但**切片關係本身沒有 relation 規則**(rhs=純 PartSelect,無
  Binary,三鎖下不產)——覆蓋注記:讀 R005+R003 可重建,規則書未直答。
  對照粗讀:`p1_bit_slice_103_comb = p1_add_96_comb[8:1]` 一致。✓(帶覆蓋注記)
- Q3:輸出縮放({5'h00, add, p0_add_20[0]})在哪條規則?
  A:**沒有**(rhs=純 Concat,同上取捨面)。out__output_flop 的裝載由 R007 釘住
  來源訊號名,縮放恆等式需回 RTL。對照粗讀一致;覆蓋注記=誠實。✓

### carryadd(corpus-external/yosys-simple,ISC)

- Q1:最低位的進位與和如何構成?
  A:R001(C=IN1&IN2)、R002(Y=IN1^IN2)。對照粗讀:i==0 臘一致。✓
- Q2:非最低位的進位呢?
  A:R003(C=IN1&IN2 | (IN1|IN2)&STAGE[i-1].C)、R004(Y=IN1^IN2^STAGE[i-1].C)
  ——行波進位鏈。對照粗讀:else 臘一致。✓
- Q3:y[i] 從何而來?
  A:規則書**沒有**直接規則(assign y[i]=Y 純線連,不含 Binary);由 R004 的
  Y 與模組結構可推。對照粗讀:`assign y[i]=Y` 一致;規則書答不全需回 RTL
  一眼——「規則書不是文件轉寫」的取捨面,非錯誤。✓

### i2c_master(corpus/e2e/59,repo MIT)

- Q1:一次位元組傳送的 FSM 環流?
  A:R022(START→SHIFT)、R023(SHIFT→ACK,當 bitcnt==7 且非半位低位)→
  R024(ACK→HOLD)→ R025-28(HOLD 按 pend_wr/pend_rd/pend_stop/pend_start
  分派回 SHIFT/STOP/START)→ R029(STOP→IDLE)。
  對照粗讀:與 case(state) 主環一致。✓
- Q2:復位裝載哪些寄存器、成什麼值?
  A:R001-R020(20 條 guard,when=!rst_n:state→ST_IDLE、cnt→16'd0、
  scl_o→1'b1、sda_o→1'b1 等,每寄存器一條)。對照粗讀:復位臂 20 個賦值
  一一對應。✓
- Q3:計數面?
  A:R030+ 的 counter 類(cnt/step 環繞與遞增)+ timing 類的半位計數。
  對照粗讀:cnt 16-bit、step 2-bit 計數結構一致。✓

## R2 散文感抽查

- 抽樣:跨 3 代表檔各抽 ~10 條規則讀陳述句。
- 結論:陳述全部為模板機械生成(「當…且…時,裝載/變為/下一拍…」句式),
  標識符=源碼原文名,無自由散文、無虛構語義。
- 噪音(接受):i2c_master 的深層 else-if 條件鏈渲染為
  「!(step == 2'd0) 且 !(step == 2'd1) 且 cnt_end」形(R022/R029 等)——
  忠實但冗長,屬機械生成契約內;不影響逐條可檢驗性。

## 覆蓋面誠實注記(評審發現,閘外資訊)

1. 純 Concat/PartSelect rhs(切片、縮放、線連)不產 relation 規則——
   R1-3 放寬只涵蓋 Binary 類;R5-Q2/Q3 兩例即此取捨面。
2. i2c_master 的 fsm when 條件鏈含上層否定鏈(機械忠實),人讀需展开。
3. **rhs 為變量/常量拷貝的賦值不產 relation 規則**(收平輪 R3-② 補記):
   relation 提取源=rhs 含 Binary;`next_state = state`、`assign y = Y`
   純線連/拷貝形不產——讀者需回 RTL 對讀。同輪已落的是另一面:分支域
   relation 攜 `when` 適用域(subbytes R007 轉寫為「當 !(state) 時,
   next_state ≡ state + 'sd1」,不再冒充全域恆等);本條缺口若未來要收,
   屬提取器迭代輪,按正典 §8.3「閾值不可降、加強提取器」路徑走。
4. 以上各點不改閘判定(V1..V6 全過);閘外資訊性質不變。
