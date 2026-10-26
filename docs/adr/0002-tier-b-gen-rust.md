# ADR-0002:tier-B `--gen-rust` — 統一引擎 IR → 獨立 Rust 模型

> 2026-10-26。狀態:接受(PLAN §3/§8-1 既定方向;M9 動工案)。slice-1 已落地(同日):riv-gen crate+vl-cli --gen-rust+4 案例 round-trip byte-exact。
> U-6(ADR-0009)的落地步:tier-B 對象 = 統一引擎的 riv-ir IR。

## 1. 決策

`rverilator --gen-rust <out.rs>`:把 elab 後的 riv-ir `Design` 生成一個
**獨立可編譯的 Rust 原始碼**(對應上游 Verilator 的 Verilog→C++ 編譯哲學;
async-fn 語法糖形態按 PLAN §3)。生成碼依賴 runtime 面等同 verilated runtime:

- `vl-values::Bits`(4-state 值單一核心,ADR-0004 同源精神);
- `riv_ir`(FmtPiece/字面量資料);
- `riv_kernel` 的 **顯示面**(`display::render_pieces` — $display/% 格式化
  是契約,gen 不得第二實作;M9 slice-1 從 kernel 抽出為 pub 葉函數,kernel
  本體同一條路徑零漂移)。

## 2. 排程契約(鐵則繼承)

**async 只作宿主語法糖,禁真並發**(PLAN §3):生成碼=單線程、固定 process
序(宣告序,ADR-0001)。slice-1 形態:

- 恰一個 `initial` thread、無 sens、無 fork/task/gate/VCD 的 design;
- NBA = 延後佇列,於下一 await(#delay)前與 finish 前套用(同一時間步內
  $display 看到 NBA 前值的語義保持);
- #delay = `rt.advance(ticks)`:時間推進 + NBA 套用。

## 2.5 slice-2(2026-10-26 同日落地)

多進程=嵌入式排程器(ADR-0001 縮影):線程=狀態機(match 段),掛起僅
Delay(#0→inactive 區,#n→wheel);run_all=active FIFO(宣告序)→inactive
區→NBA 批次→wheel 推進(新時間的延遲 NBA 先於被喚醒線程提交;kernel
run-loop 序)。線程函數回 `Result<Susp,_>`(運行錯誤=exit 1 對齊 kernel)。
**實錄兩刀**:①Delay 被切進下一段開頭→上一段誤回 Done、線程死在第一個 #
(修=Delay 終結所屬段);②賦值未按宣告寬截斷→1-bit reg 被 64-bit const 蓋寬、
%b 打出原始寬(修=rt.store/nba 攜寬,Limbs::padded+pack,即 kernel 提交
同款)。覆蓋 4→16 案例(含 32_proc_order=process 序釘子、49_multi_initial、
34_cmp_signed_mixed),全部 byte-exact。

## 2.7 slice-3(2026-10-26 同日落地)

事件驅動:邊沿觸發 always(靜態敏感表,建構即武裝、整體重臂)+ initial 的
`@(edge)` 掛起;喚醒=riv_kernel::edge_fire_pub 單源(第二張邊沿表=契約分叉)。
**實錄三刀**:①Rearm 未重置 st——kernel 重臂=pc=0,留 st 會在下次觸發跑空段
回 Done、線程永久死亡(25_edge_or hits=1 而非 3);②WaitEdge 掛起未推進
st——每次觸發整段重跑、停在同一 @(posedge) 永久循環(23 掛死);③排程器
佇列放 Rt 欄位(commit 的喚醒要進 active 尾)。watchdog 進閘:生成模型死循環
=單例失敗(10s),不再吊死閘線(鐵則 4)。覆蓋 16→18(23_nba_vs_blocking=
NBA 對 blocking 語義釘子、25_edge_or=雙邊沿 or 敏感)。剩餘阻擋普查:Branch
(迴圈)×5、$time×2、LoadBit×2、level 線程(21)——slice-4 候選序。

## 3. 子集邊界與硬錯

子集外一律 `error: --gen-rust: <構造> not supported yet (milestone M9)`
(exit≠0,鐵則 §3)。slice-1 op 白名單:Const/Mov/Bin/Bit/BitXnor/BitNot/
Reduce/LogicAnd/LogicOr/LogicNot/Cmp/CaseCmp/Shift/Sel/Concat/Replicate/
LoadVar/LoadPart/LoadBit/AssignBlk/AssignNba/SysCall{Display,Write,Finish,
Time,FinishLine 形}。多線程/連續賦值非常數 RHS/always/函數呼叫 = 後續切片
(切片序=語料按解釋器差分通過率排序)。

## 4. delay 語意 → 生成碼對照表(PLAN §3 附錄義務)

| IR 原語 | 生成碼 |
|---|---|
| `Op::Delay`(#n) | `rt.advance(n)`(推進 now+NBA 套用) |
| `Op::AssignBlk` | `vars[id] = v`(立 即) |
| `Op::AssignNba` | `rt.nba(id, v)`(佇列) |
| intra-assignment delay | elab 已拆「取樣+延遲+寫回」節點( PLAN §3),取樣值走暫存 slot |
| NBA 區兩段式 | 同時間步內 $display 見舊值(佇列於 advance/finish 前套用) |

## 5. 驗收(差分 oracle=解釋器,PLAN §8-1)

- 逐 case:gen → rustc 編譯 → 執行,stdout 與解釋器(`--binary`)錄製期望
  **byte-exact**;
- 閘=`gen_rust` round-trip 測試(regress 之 cargo test 面),子集外案例=
  硬錯訊息逐字釘死;
- 效能目標(tier-B ≤10× 上游)為後續切片 bench 項,slice-1 不宣稱。

## 6. 非目標

真並發/unsafe;VCD 傾卸(與 --binary 的 trace 差異留 slice-3+);SV 構造
(M8+ 候補同族);把 gen 面接到 riverilog CLI(iverilog 形無此旗標)。
