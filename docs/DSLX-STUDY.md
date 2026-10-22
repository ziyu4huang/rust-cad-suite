# DSLX 研究 → tier-B `--gen-rust` API 設計參考(MVP 筆記)

日期:2026-10-23 · 狀態:MVP(源碼/文件取證 + 兩次工具實跑)· SSOT=本檔
父計畫:PLAN §5 tier B(M9)· 工具鏈:google/xls(Apache-2.0,1.9k⭐)

## 0. XLS 是什麼、與本專案的關係

XLS = Accelerated HW Synthesis:DSLX(Rust 風格 DSL)/xlscc(C++ 子集)
→ XLS IR → 優化 → **生成 Verilog**。它在 Verilog 的**上游**,不與
rverilator 競爭——它的 codegen 輸出正是 rverilator 的輸入語料
(corpus-external/xls/,收割管線=.github/workflows/xls-harvest.yml)。

## 1. 哲學印證(記錄在案)

| 工具 | 模擬加速路線 |
|---|---|
| XLS | LLVM **JIT 編譯 IR**(`ir_jit`) |
| Verilator | **C++ codegen** + 編譯模型 |
| 本專案 tier B(M9) | **`--gen-rust`** 生成 Rust 模型 |

三家同一答案:模擬要快,編譯設計而非逐事件直譯。M9 方向被背書。

## 2. DSLX 型別系統 → tier-B API 對映(MVP 觀察)

| DSLX 形狀 | tier-B `--gen-rust` 對應(建議) |
|---|---|
| `uN[N]` / `sN[N]` 參數化位寬 | `Bits<N>`/`Signed<N>` const-generic 新型別或運行期 `Bits{width}`(與 vl-sim::value 一致);位寬即型別=抓 WIDTH 類錯誤於編譯期 |
| `u32/s32` 等別名 | 生成碼直接用具體寬度型別 |
| 參數化函式 `fn f<N: u32>(..)` | Rust const generics(單調映射,無須發明) |
| enum(無繼承)/ struct / tuple | Rust enum/struct/tuple 直譯 |
| `let` 綁定+型別 ascription、不可變為主 | SSA 風格中間碼 → 生成碼天然貼合 |
| `match` + 窮盡性分析(exhaustiveness) | Rust match 原生(窮盡性檢查免費獲得) |
| `for i in range(..)` | Rust for + 迭代器;含 `state_element` 的行為轉換 |
| 隱式寬度推導(type system 目錄=推斷引擎) | tier-B 需自帶寬度推斷或沿用 vl-sim 的上下文寬度規則(§5.4) |

**直接可偷的設計**:①「位寬即型別」讓 WIDTH 類錯誤編譯期曝露——
tier-B 生成碼應優先 const-generic 位寬;②match 窮盡性免費;
③DSLX 的不可變 let 主導 = 生成 Rust 代碼的天然形狀(可 mut 限於狀態元)。

## 3. 工具實錄(2026-10-23,xls-harvest 四輪迭代,證據=CI log)

- 最新 release 的 `codegen_main` 旗標改版:IR 走**位置參數**、Verilog 走
  stdout、`--delay_model` 預設空(**release 二進位未帶模型資料檔,
  `--delay_model=unit` 必須顯式給**);pipeline scheduling 必帶
  `--pipeline_stages` 或 `--clock_period_ps`。
- DSLX 已移除 `#[pipeline]` 屬性(工具 parse error 實證)——級數由
  codegen 旗標驅動;`s1`/`s2` 式短名已成保留字(同樣工具實證)。
- codegen 輸出用 `always_ff`(SV 關鍵字):收割管線正規化
  always_ff→always(單時鐘 flop 語義等價,raw 副本保留)。
- 方法論點:旗標衝突**以工具 `--helpfull`+stderr 為準**,禁盲猜
  (與 corpus oracle-first 同族)。

## 4. 後續(非 MVP)
- 收割更多樣本(stdlib/例題),加 `--module_name` 正規化;
- `ir_jit` vs 我們直譯器的性能對照 bench(M9 前 baseline);
- `state_element`/proc 語法的現行形狀考證。
