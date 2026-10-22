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
corpus-external/  外部語料快照(入 git,帶 LICENSE;收編即解除 ignore):
                  yosys-simple/ =YosysHQ/yosys tests/simple 87 檔(ISC)+LINT-DIFF.tsv;
                  sv-tests/     =chipsalliance/sv-tests chapter-5/6/8/9/12 260 檔(ISC)+TSV
                                 (SV 語法多數在 V2005 範圍外,31% 為資訊值非缺陷值);
                  lint 差分協議=EXT=v|sv bash scripts/lint_diff.sh <dir> <tsv>(oracle=-Wno-fatal,
                  判決只看 error 級;warning 類 WIDTH/LATCH 是未來 lint 產品軌)
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
| M2 | parser + **垂直切片** | 全 V2005 文法;多錯誤恢復;**組合邏輯 smoke 提前**(對抗審 #10);fuzz | parser ✅;**垂直切片 ✅ 2026-10-21**(crates/vl-sim:4-state 值/求值器/union-find 迷你 elab/組合收斂/initial+$display;`--binary` 已可跑;corpus/fulladd 與 Verilator 5.050 **byte-diff 全綠**,測試 sim_gate 釘死);fuzz 待 |
| M3 | elab + lint-only | 符號/參數/generate/寬度;picorv32+serv 零假 WIDTH;**lint 提前為第一可用產物** | |
| M4 | 解釋器核(無時序) | 4-state 值(IEEE §5 表格單測為準,對抗審 #2);tb_race 確定序;加法器 TB 與 oracle stdout byte-diff | ✅ 2026-10-21:上下文位寬/符號(§5.4)、函式(靜態)/任務(內聯)、process 序=宣告序(32_proc_order 對拍);算術 X=**LRM 精確**(§5.1.4:任一運算元含 x/z → 整個結果 x——原「保守」措辭是誤標,IEEE 原文如此;值單測軌持續) |
| M5 | timing+調度 | **counter8 fixture `PASS` byte-exact**(corpus/counter8/,oracle 已錄;含 NBA 區讀值細節);UNOPTFLAT;TIMESCALEMOD 對等硬錯 | ✅ 2026-10-21:進程 VM+時間輪+NBA 區+邊沿喚醒;counter8 與 fulladd byte-diff 綠(sim_gate);UNOPTFLAT=組合 100 輪上限;TIMESCALEMOD 對等待 |
| M6 | VCD | 規則=逐時間戳 (canonical name→value) 集合比對+**alias 組折疊**(oracle 實證多網同碼),禁裸字節 diff(對抗審 #4) | ✅ 2026-10-21:vl-sim/vcd.rs 寫出器(作用域樹/有序 id/時間槽 flush)+\$dumpfile/\$dumpvars 接線;閘=vcd_gate(4 例 oracle-ref.vcd 交叉對拍 16/16 訊號 90 采樣點零失配,入 cargo test);oracle 參考=`scripts/oracle_vcd.sh`(--binary --trace 原生支援 \$dumpvars,自寫 main 不需要) |
| M7 | 記憶體陣列+\$readmem+閘原始件 | 30_gen_mux 轉綠+新例 oracle 錄製 | ✅ 2026-10-23:unpack 陣列(單 range)/逐字 VCD(mem[i] 元件變數=oracle 實證)/\$readmemh/\$readmemb(@addr 跳址+start/finish+註解;x-index 寫忽略/OOB 讀 x)/12 種閘原始件(§7 4-state 表,bufif/notif 停動驅 z);**E2E 55/55=100%**(52_mem_basic/53_readmem/54_gates/55_mem_nba oracle 錄製) |
| M7b | lint-only 差分軌(yosys tests/simple 收編) | lint parity 對 verilator --lint-only -Wno-fatal | ✅ 2026-10-23 首輪:**44→66/87 (75.9%)**;harness=scripts/lint_diff.sh;TSV=corpus-external/yosys-simple/LINT-DIFF.tsv;餘 20 差異=M8 路線圖(見下) |
| M8-M9 | 語言廣度/SV 階梯/perf gate + tier-B ADR | 見 PLAN-DRAFT §5;M8 首批候補=層級引用深化(unnamed genblk 鏈×5 檔)、SV fill literals '0/'1(≈11 檔)、>64-bit 多 limb(×6)、intra-assign delay、reals、>8 字元字串、宏貼字面量(`SIZE'ha) | |

## 6. CLI 契約(對抗審 #7/#8 修訂)
- 二進制名 `rverilator`;`verilator` 別名等 ≥M9 flag 相容可信後再議(open question)。
- **driver 模式未知旗標 = 硬錯 exit 1**(已實作;oracle 實證 verilator 對 `--frobnicate` 即 `%Error`+exit 1;拼錯旗標靜默續跑=CI 毒藥)。runtime plusargs 是另一層,可寬容。
- `--timing` 語義對齊:上游僅 `--binary` 預設開,`--cc` 需顯式(對抗審 #7);我方 v0 一律即時模擬,等價 --binary 路徑。
- 診斷輸出到 **stdout**(verilator 同);exit 0=綠 / 1=診斷或用法錯。

## 7. 測試策略
單元(值真值表=IEEE 1364-2005 §5 為 oracle;lexer/pp/parser)→ **fuzz**(fuzz_smoke:語料變異+隨機字節,400+200 迭代/固定種子,守護線程 5s,panic/hang=紅並落盤復現——首輪即抓三個 lexer UTF-8 健壯性 bug)→ golden(`corpus/<case>/expected.*`)→ **差分 vs 5.050**(stdout 過濾白名單+`--timescale` 統一+VCD 規則化比對;詳細規則=`corpus/counter8/README.md`)→ fuzz(M2 起)。上游 test_regress 永不入 git(LGPL/Artistic),僅本地唯讀。

**E2E 回歸閘(2026-10-21 上線)**:`corpus/e2e/` 32 例×oracle 錄製期望已入 git;`cargo test` 內建 ≥80% 閘(`vl-difftest` crate;期望常駐,無需本機 oracle)。當前基線 **31/32=96.9%**(唯一 FAIL=30_gen_mux 存儲器陣列 M7 硬錯,屬聲明範圍)。錄製=`cargo run -p vl-difftest -- --record`。

### 7.5 外部語料普查(2026-10-22,gh api 實搜;回應「官方回歸跑了嗎+找更多回歸語料」)

**官方 verilator test_regress 未跑、也無法全跑**:實測其 repo 有 **3,616 個 .v**,每例由 Perl/C++ harness 驅動、附各自 golden;大量案例測的是 C++ runtime/VPI/編譯器旗標,對獨立模擬器 CLI 不可遷移。本專案的「回歸」= 自建 51 例錄製期望差分套件(oracle=本機 5.050)。test_regress 與 ivtest 同列**唯讀挖掘源,永不入 git**。

| 來源 | 規模(實測) | License(查 LICENSE 檔) | 可入 git? | 用途 |
|---|---|---|---|---|
| chipsalliance/sv-tests | 402⭐;tests/chapter-5..26 按章+generic 主題 | ISC | ✅ | 第三批:chapter-5..9/12 行為子集過濾 |
| YosysHQ/yosys tests/simple | 87 .v(always/case/attribute/const-fold 小例) | ISC | ✅ | **第二批首選**——純 Verilog 小例,正中現有能力 |
| YosysHQ/picorv32 | 全核 RISC-V | ISC | ✅ | M7 後:整合驗證標的 |
| olofk/serv | 1.9k⭐ 位串列 RISC-V | ISC | ✅ | M7 後:整合驗證標的 |
| steveicarus/ivtest | **2,607 個 .v**(ivltests/) | GPL-2.0 | ⚠️ 唯讀挖掘 | 最大 V2005 相容性語料;按主題抽角案例 |
| verilator/verilator test_regress | 3,616 個 .v | LGPL/Artistic(NOASSERTION) | ❌ 唯讀挖掘 | 既有政策不變 |

收編流程=既有 `record_with_oracle`(期望一律 oracle 錄製)+ corpus_lint R1-R3 三閘,零手寫期望。

## 8. Open questions — 裁決建議已「暫定採納」(2026-10-21;使用者隨時可翻案,翻案=改本表+對應 ADR)
| # | 問題 | 暫定裁決 | 理由 |
|---|---|---|---|
| 1 | tier B(--gen-rust 生成 Rust 模型)要不要作產品面 | **要**,M9 ADR-0002 後落地 | 貼近 verilator「Verilog→C++ 編譯」哲學;解釋器先保正確性,tier B 以解釋器為差分 oracle |
| 2 | `verilator` 同名別名 | **M9 後再議**,現用 `rverilator` | PATH 安全(免毒化自家 oracle);別名必帶自標識 `--version` |
| 3 | 性能目標 | 解釋器 ≤100× 上游(picorv32 boot)、tier B ≤10× | 工作目標;M6 bench 後複核 |
| 4 | SV interfaces 時點 | **維持 M10** | 複雜度最高、早期收益最低 |
| 5 | P7 cocotb/Python 生態 | **出範圍;屆時改提 Rust 原生 cosim API**(rverilator 作為庫) | Rust-only 約束下硬搬 Python 是偽命題 |
| 6 | License | **MIT OR Apache-2.0 雙授** | 自寫件全同授;ISC 外部快照保留原證 |
