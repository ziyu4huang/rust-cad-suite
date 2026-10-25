# sv-elab 調查與 SV 統一支援採用計畫

> 2026-10-25 立案(使用者指示:學習開源 CAD suite 資源,以 sv-elab 為參照,
> 統一 rust-cad-suite 的 SystemVerilog 支援;先落計畫)。
> 證據=本機淺 clone 實測(github.com/povik/sv-elab)+ 專案 README/LICENSE。

## 0. 事實塊(全部實測)

- **授權=ISC License**(Martin Povišer)——與 yosys-simple 同級,最寬鬆:
  **測試語料可入 git**(GPL 防疫不適用;third_party/slang 另攜自身授權,不收)。
- **定位**:SystemVerilog elaborator,把 SV 設計展開到 word-level netlist;
  前身 yosys-slang。**建構在 slang(MikePopoloski/slang,MIT)之上**。
- **下游=產業正典**:Yosys v0.67 起內建;OpenROAD 整合合成工具;
  **OSS CAD Suite 預 built 外掛**——我們對標的 oss-cad-suite 的 SV 前端正是它。
- **相容面**(README 實錄):Black Parrot、BSC Core Tile、CV32E40P、Ibex、
  OpenTitan、RSD——大型真實 SV IP 的解析證據鏈(compat-suite repo 另存)。
- 語料:`tests/various/`(12 SV 檔:interface/modport 陣列、concurrent
  assert、delays、timescale、pragmas…)+ `tests/unit/`(27 SV 檔:bitsel/
  break_return/case/countones…)+ croc_boot;`tests/*.ys`=Yosys 腳本
  (外掛呼叫形,不收)。SV 語言面=IEEE 1800-2017/2023 可綜合子集(非正式定義),
  Feature-support wiki 為官方支援矩陣。

## 1. 採用裁決(三路取值,代碼零依賴)

sv-elab 本體=C++/slang 下游 —— 依 Rust-only 與「禁 fork 第二 parser」鐵則
(RIVERILOG-PLAN §3.1),**不搬任何代碼**。價值三路:

| # | 取值 | 形式 | 依據 |
|---|---|---|---|
| A | **回歸語料** | tests/{various,unit} 的 .sv 收編 corpus-external/sv-elab/(ISC 隨附 LICENSE);雙引擎 SV 接受性同位閘 | ZipCPU EXT/ZIPLINT 語意已驗證,但 ISC 可入 git=無 oracle 依賴、無 vendor 樹依賴 |
| B | **支援矩陣正典** | Feature-support wiki 的分類法=rverilator M8-M9 廣度表與 riverilog R8 階梯的共通分類軸;SV 構造收錄順序按其「synthesizable subset」劃界 | 我們兩線的 SV 廣度表目前各自為政 |
| C | **slang 架構參照** | 層次化 elaboration/AST 設計研究(§9.5 既列合法);未來「共享前端不承載 SV 時新前端 vs 擴展」ADR 的比較基準 | slang 一個前端多下游=我們「一個 SV 前端雙語義引擎」的產業同構證據 |

## 2. 「統一 SV 支援」的架構立場(本 repo 落法)

使用者方向=「unified all good thing into one」。本 repo 的對應裁決:

**單一共用 SV 前端(vl-syntax dialect profile 擴充),雙語義引擎各自消費**:
- 前端唯一:vl-syntax 已是雙線共享(R4 實踐:Trigger/IntraAssign 同輪落
  雙引擎,vl-sim 錯誤搬遷保位元組)——SV 構造續走此路,**禁第二 parser**。
- 方言階梯:-g2012 全階梯=RIVERILOG-PLAN §8 R8 本就認領的面;rverilator
  側 SV=fill literals/always_ff 族已部分在案(M8-M9 候補清單)。
- 語義分岔仍在引擎層(排程/2-vs-4-state),與 SV 語法面正交。
- sv-elab=此架構的外部印證:slang 一個前端服務 Yosys/OpenROAD/仿真下游。

## 3. SV-UNIFY 弧規劃(語料驅動,納入 PLAN 路線)

排程掛鉤:riverilog R8 提級為雙線聯合弧;rverilator M8-M9 的 SV 候補
(fill literals 等)同弧收口。逐簇順序=sv-elab 語料覆蓋 × 既有排除表:

1. **SV-1 語料腿**:收編 tests/{various,unit}.sv → `--sv-parity` 同位閘
   (iverilog -g2012 vs riverilog;verilator --lint-only vs rverilator;
   排除表登記制)。基線先量測再逐簇收。
2. **SV-2 即時斷言** `assert/assume/cover(expr)` 語句(vl-syntax 解析+
   雙引擎語義:仿真=求值失敗報錯;lint=可解析)——收復 ZipCPU fwb×3 排除。
3. **SV-3 類型族**:logic/typedef/enum/packed struct(riverilog R8 既列;
   rverilator 側 M8-M9 對齊)。
4. **SV-4 interface/modport**(sv-elab various 的主構造;語義=展平成
   埠束——兩引擎共用一種 lowering)。
5. **SV-5 always_ff/always_comb/always_latch + fill literals '0/'1**
   (rverilator M8-M9 候補既列,≈11 檔)。
6. **SV-6 長字串>8 字元**(vl-sim M7 硬錯解除;wbarbiter 排除收復)。

每步同輪:vl-syntax 擴充+雙引擎語義+sv-parity 閘基線推進+帳務。

## 4. 風險與非目標

- **非目標**:不引入 C++/slang 依賴;不做 SVA sequence/property 深水區
  (`|=>`/`##1`——div.v 排除在案,屬 formal 面 P5+);不追求 sv-elab 的
  netlist 後端(那是 yosys 線)。
- 風險:SV 面大 → 以語料同位閘逐簇收(EXT 模式已驗證);interface 語義
  展平的雙引擎一致性 → 共用 lowering 契約測試釘死。
