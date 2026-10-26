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
