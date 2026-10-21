# rust-cad-suite — PLAN(正典)

> 独立 repo,2026-10-21 立项。目標:YosysHQ oss-cad-suite 工具集的**纯 Rust 重写**
> (no upstream C/C++ compiled or linked for core function;每個交付工具都是 Rust 二進制)。
> 本檔 = planner 出案 + 對抗審 10 條發現合成後的正典;PLAN-DRAFT.md 保留原案供對帳。
> Phase 1 = **Verilator 等价物**(clean-room:復刻角色與語義,不抄 C++ 代碼)。

## 0. 已驗證事實(2026-10-21 本機)
- rustc/cargo 1.95.0;macOS arm64。
- 真 Verilator 5.050 在 `/opt/homebrew/bin/verilator` = dev-time 差分 oracle(永不 ship);
  iverilog 未裝(difftest 自動跳過)。
- oss-cad-suite 二進制包 darwin-arm64-20261020(526MB)已下載至
  `vendor-reference/suite-dl/`(gitignored),Phase 2+ 各工具的 oracle 候選。
- upstream 63 個工具腳本清單見 `vendor-reference/oss-cad-suite-build/default/scripts/`。

## 1. Suite 路線圖(Phase 1 之後的排序)
1. **P2 yosys-equiv 綜合核**——最高槓桿,復用 vl-syntax/vl-elab。
2. **P3 iverilog-equiv**——事件驅動調度器 + VPI 子集,~80% 復用 Phase 1 crates。
3. **P4 nextpnr-generic + FPGA 家族**——prjoxide(Rust, ISC)直接收編;其餘長尾。
4. **P5 formal**——⚠ 風險最高:上游 SMT/SAT 全是 C++,純 Rust solver 成熟度是全 suite 最大未知;屆時再裁。
5. **P6 波形查看器**——surfer(Rust, EUPL-1.2,僅可獨立二進制)或自寫 TUI。
6. **P7 cocotb 等价**——Rust-only 與 Python 生態衝突,open question。

## 2. Phase 1 架構(workspace)
```
crates/
├── rcs-diag/     spans/source map/診斷/caret 渲染            [0 deps]
├── vl-syntax/    pp + lexer + parser + AST(手寫前端)        [rcs-diag]
├── vl-ir/        展開後 IR(sim ↔ tier-B codegen 共用)      [rcs-diag]   (M3 建)
├── vl-elab/      鏈接/參數/generate/寬度 → 降到 vl-ir        (M3)
├── vl-sim/       調度器 + 解釋器 + 4-state 值 + VCD          (M4-M6)
├── vl-cli/       bin `rverilator`                            (現用,0 外部依賴)
└── vl-difftest/  dev-only oracle 差分 harness               (M4)
corpus/           自寫樣例 + oracle 期望輸出(入 git)
corpus-external/  picorv32/serv/sv-tests 快照(ISC,入 git,帶 LICENSE 聲明;gitignored 直到首次收編)
fuzz/             cargo-fuzz(M2 起)
```
依賴政策:核心 crates 零外部依賴;vl-cli 未來可 clap(現手寫 flags)。

## 3. 執行策略(ADR-0001 摘要,全文 docs/adr/0001)
v0 = **A. 樹遍歷解釋器**(reference semantics;`--binary` 語義 = 即刻模擬,零編譯延遲——比上游 16-92s C++ build 是真 UX 優勢)。進程降到 **segments**(直線語句段 + 顯式掛起點)= VM-shaped IR,tier B(`--gen-rust`:生成 async-fn Rust 模型 → rustc,對應上游 C++20 coroutine 調度)在 M9 ADR-0002 決策。C(bytecode)否決。
**delay 語意 → IR 原語對照表(ADR 附錄,動工 M4 前補齊)**:intra-assignment delay 顯式拆「取樣節點+延遲+寫回節點」;`@(a or b)` 多源喚醒;wait 邊沿重入;#0 與 NBA 區兩段式更新。async 只作宿主語法糖,**禁止真並發**——調度契約=單線程固定 process 序(宣告序;oracle 實證 tb_race 輸出確定)。

## 4. 語言範圍
IN(v0, Verilog-2005 核):見 PLAN-DRAFT §4(模組/ANSI+非 ANSI 埠/wire-reg/assign/always/阻塞非阻塞/case 族/for-while-repeat/parameter/generate/實例化/function-task/$display 族/`timescale-`define-`include 族/#delay/字面量含 x-z/拼接複製/全運算符)。
OUT(顯式 "not yet supported, Mx" 硬錯):specify/UDP/VPI/force-release(M7)/wait-fork(M8)/實數角落(M7)/FST(M9)/SVA。
SV 階梯:M8 = logic/always_ff-comb-latch/typedef-enum-struct/packages;interfaces = M10。

## 5. 里程碑(對抗審修訂後)
| M | 內容 | 驗收 | 狀態 |
|---|---|---|---|
| M0 | workspace + rcs-diag + lexer | 字面量/directive/punct 測試;--version | ✅ 2026-10-21 |
| M1 | preprocessor | object+單層 function-like 巨集/`` `` ``/`` `" ``/ifdef 族/include(-I,環=硬錯)/timescale/__LINE__;`verilator -E` 對拍正規化 | 核心 ✅(pp 測試 17 條綠);-E 對拍待 |
| M2 | parser + **垂直切片** | 全 V2005 文法;多錯誤恢復;**組合邏輯常量折疊 smoke 提前**(對抗審 #10);fuzz | parser ✅ 核心;恢復/fuzz/smoke 待 |
| M3 | elab + lint-only | 符號/參數/generate/寬度;picorv32+serv 零假 WIDTH;**lint 提前為第一可用產物** | |
| M4 | 解釋器核(無時序) | 4-state 值(IEEE §5 表格單測為準,對抗審 #2);tb_race 確定序;加法器 TB 與 oracle stdout byte-diff | |
| M5 | timing+調度 | **counter8 fixture `PASS` byte-exact**(corpus/counter8/,oracle 已錄;含 NBA 區讀值細節);UNOPTFLAT;TIMESCALEMOD 對等硬錯 | fixture 就緒 |
| M6 | VCD | 規則=逐時間戳 (canonical name→value) 集合比對+**alias 組折疊**(oracle 實證多網同碼),禁裸字節 diff(對抗審 #4);參考 VCD 走 --cc+手寫 main 腳本 | |
| M7-M9 | 語言廣度/SV 階梯/perf gate + tier-B ADR | 見 PLAN-DRAFT §5 | |

## 6. CLI 契約(對抗審 #7/#8 修訂)
- 二進制名 `rverilator`;`verilator` 別名等 ≥M9 flag 相容可信後再議(open question)。
- **driver 模式未知旗標 = 硬錯 exit 1**(已實作;oracle 實證 verilator 對 `--frobnicate` 即 `%Error`+exit 1;拼錯旗標靜默續跑=CI 毒藥)。runtime plusargs 是另一層,可寬容。
- `--timing` 語義對齊:上游僅 `--binary` 預設開,`--cc` 需顯式(對抗審 #7);我方 v0 一律即時模擬,等價 --binary 路徑。
- 診斷輸出到 **stdout**(verilator 同);exit 0=綠 / 1=診斷或用法錯。

## 7. 測試策略
單元(值真值表=IEEE 1364-2005 §5 為 oracle;lexer/pp/parser)→ golden(`corpus/<case>/expected.*`)→ **差分 vs 5.050**(stdout 過濾白名單+`--timescale` 統一+VCD 規則化比對;詳細規則=`corpus/counter8/README.md`)→ fuzz(M2 起)。上游 test_regress 永不入 git(LGPL/Artistic),僅本地唯讀。

## 8. Open questions(使用者裁決點)
1. tier B(--gen-rust)是否要作為產品面(M9 ADR)?——建議要。
2. `verilator` 同名別名是否要(M9 後)?
3. 性能目標:建議解釋器 ≤100× 上游(picorv32 boot);tier B ≤10×。
4. SV interfaces 提前與否(現 M10)。
5. P7 cocotb 類:Rust-only 約束 vs Python 生態。
6. suite license 確認 MIT OR Apache-2.0(README 待蓋章)。
