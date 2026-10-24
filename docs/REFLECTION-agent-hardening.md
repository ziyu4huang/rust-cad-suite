# Self-reflection: what building the verilator-equivalent taught about
# agent-developed EDA/IC tooling (2026-10-21, arc M0→M5 + E2E gate)

Scope of evidence: commits 2f3d43f → c757a7a. 32-case E2E differential suite
vs Verilator 5.050 at 96.9% (the one failure is a declared M7 hard-error).
Every failure below is one that *shipped* in a commit before being caught —
that is the point of recording them.

## 1. The dominant failure class: hand-authored expectations vs the oracle

Seven times in one arc, a wrong expectation reached the tree, and **every
time the oracle was right and my reasoning was wrong**:

| # | My belief | Oracle truth |
|---|---|---|
| 1 | TB reads `q=5` after 5th posedge | `FAIL q=4` — NBA region commits after the active-region read |
| 2 | `%0d` pads like `%d` | `%0d` = minimal; plain `%d` pads to type width |
| 3 | `4'hABCD` truncates (LRM §2.5.1) | Verilator errors "too many digits" |
| 4 | binary `a ~& b` is an operator | Annex A has no binary `~&`/`~|`; Verilator rejects |
| 5 | `12'hDEAD_BEEF` truncation unit test | error (same as #3) |
| 6 | `'hx` → 32'h0000000x | unknown fills the full 32 bits |
| 7 | `always @*` waits at t0 like a simulator | level-sensitive processes evaluate once at t0 |

Root cause: I treated the LRM as the spec. For a *compatibility* product the
spec is the reference tool's observable behavior; the LRM only explains it.
A hand-written golden file is a guess with a file extension.

**Rule (now AGENTS §2):** expected outputs are *recorded* from the oracle,
never authored. Hand-written expectations are allowed only for error-path
shape (exit codes, milestone pointers), not value semantics.

## 2. Two acceptance tracks, or the X-policy trap

The adversarial audit proved Verilator is 2-state (`1x00+1=1001`). A
4-state-correct implementation would *fail* a naive byte-diff against it on
X-propagating samples — correctness punished as regression. Corollary found
in practice: `z` literals and same-edge scheduling races (FSM case) are the
same trap in disguise — case 26 had two processes racing on one edge, where
IEEE itself is nondeterministic and only Verilator's internal order made it
"pass".

**Rule (AGENTS §3):** byte-diff corpus is X-clean and race-free by policy;
4-state operator correctness and scheduler determinism are gated by their own
unit tests (IEEE §5 tables, declaration-order contract). A parity test that
depends on unresolved order is not a test, it's a coin.

## 3. Error recovery must guarantee progress — with a test

The worst bug of the arc: `task one(input integer n);` hung the parser
forever. `sync_item` stops on structural keywords (`endtask`, `end`…) without
consuming them; the module loop didn't treat them as terminators; the error
path re-derived the same diagnostic forever. It survived because no fixture
combined "task + ANSI typed port". Found by `sample`-ing the live process,
not by reading code.

**Rule (AGENTS §4):** every recovery loop carries a `pos-unchanged → bump`
guard, and the corpus contains one hang-shaped fixture per recovery site.
A watchdog is part of the test harness, not an ops afterthought.

## 4. Semantics bugs cluster where two representations meet

`~x` flipped x→z (bit_not inverted the unknown *kind*); `>>>` filled sign
bits at bit 64 instead of the value's width; ternary-merge used the AND
forcing rule; `%d` dropped the signed flag between Val and Bits. None of
these are parser problems — they live exactly where a bit-pair encoding
`(data, mask)` meets an operator table or a format contract.

**Rule (AGENTS §5):** every 4-state operator gets its x/z-kind-preserving
unit test written *with* the operator, from the IEEE §5 table, before it is
used anywhere. Formatting (`%d/%b/%t`) is a contract too — pin it with
golden tests, the oracle arbitrates.

## 5. Strict resolution surfaced what blind prefixing masked

The first implementation "worked" because it blindly string-prefixed names;
the rewrite to strict scope-chain resolution immediately exposed that ANSI
port declarations never created cells — a real elaboration hole that had been
passing by coincidence (flat-namespace collision). Lesson: making illegal
states *error loudly* converts silent miscompiles into fixable failures; the
same pass turned every unsupported feature into a `milestone Mx` pointer, so
E2E failures self-classify into "bug" vs "declared scope".

**Rule (AGENTS §6):** unresolved = hard error with a named milestone.
Silent fallbacks are banned; a `not yet supported` error is a feature.

## 6. Process debt that paid off (keep doing)

- Adversarial review with *execution* evidence (it ran verilator before
  reviewing) set the X-policy, the normalization spec, and the flag-error
  contract before any of this was written — zero rework attributed to it.
- Oracle-driven corpus authoring: verilator's warnings-as-errors reviewed my
  testbench style for free (WIDTH/ASCRANGE/CASEX warnings).
- Surgical edits via scripts with hit-count assertions; re-read after any
  script mutation of a file. Two Edit-tool staleness rejections and one
  match-arm splice broke a build — the assertion discipline caught what
  confidence would have shipped.

## 7. What is still weak (next hardening)

- No fuzzing yet (M2 gate item) — the recovery-hang bug class is exactly
  what fuzz_parse would have caught in minutes.
- Coverage is pass-rate, not line coverage — a case can pass while never
  exercising a branch. Add per-case coverage instrumentation when the
  scheduler stabilizes.
- VCD parity (M6) still open; the alias-folding comparison rule is written
  down but not implemented.
- Single-oracle risk: everything is pinned to Verilator 5.050 behavior;
  iverilog as a second oracle would catch Verilator-specific quirks being
  copied as "spec".

---

# Round 2 (2026-10-22): corpus expansion → M6 VCD → fuzz arc

Evidence: commits e5880a1 → f87abf1 + 7c282d3. Corpus 32→51 cases, rate held
at 98.0%; VCD cross-check 16/16 signals; fuzz gate landed.

## 8. The normalization filter is a contract with production output

Three iterations in one arc: `- ` filter → ate the oracle's `-Info: $dumpvar
ignored` line; blanket `-` filter → ate the DESIGN's own `-7div2=-3` output
line. Final: exact prefixes (`- `, `-Info`, `-Warning`, `-Error`), pinned by a
unit test that includes a design line starting with a dash.

**Rule (AGENTS §2/§8):** normalizers are specified by exact prefix and pinned
by tests containing hostile fixtures. Recording also runs with `--trace` so
the oracle's `$dumpvars` is honored instead of producing tool notes.

## 9. Re-record is a semantic event

Switching record flags silently rewrote expectations and dropped the pass
rate 98→90. The fix was normalization, but the *lesson*: expectations changed
without any simulator change — that must always be explainable.

**Rule (AGENTS §10):** after `--record`, `git diff --stat corpus/e2e` is read
line-by-line; every changed case must map to an intended semantic delta.
`regress.sh --with-record` prints the diff; blind re-record+commit is banned.

## 10. Pass-rate hides robustness; fuzz covers the orthogonal axis

51/51-style passing cases are all well-formed ASCII Verilog. A 600-iteration
deterministic fuzz (corpus mutation + random bytes, watchdog per mutant)
immediately found three UTF-8 robustness bugs: (1) error recovery advanced 1
byte through multi-byte characters, (2) `skip_trivia` treated continuation
bytes 0x85/0xA0 as Unicode whitespace (NEL/NBSP via byte→char cast), (3)
based-literal base letters / string escapes bumped non-ASCII bytes blindly.
None could ever be reached by expanding the corpus.

**Rule (AGENTS §4b):** a robustness gate (deterministic fuzz with watchdog +
repro-saving) runs beside the pass-rate gate. The two axes are independent;
a green pass-rate says nothing about hostile input.

## 11. LRM re-read can retire planned work

The plan carried "replace conservative all-x arithmetic with bit-serial
tables" as a milestone-sized item. Re-reading IEEE 1364-2005 §5.1.4: *if any
operand bit is x/z, the entire result is x* — the implementation was already
LRM-exact; only Verilator (2-state) differs. One citation retired a milestone.

**Rule (AGENTS §1b):** every planned semantics item cites its LRM clause at
planning time. If the clause says the naive implementation is exact, the work
item dies before spending effort.

## 12. The race-free TB pattern needed two attempts — mechanize it

Case 43 was rewritten "race-free" twice: `@(posedge clk); ld = 1; #1;` still
writes at the edge (the `#1` came after the write), and the residual `rst = 0;`
was a third latent race both oracles agreed on by luck. The final mechanical
pattern is `@(posedge clk); #1; <write>;` — write strictly mid-cycle.

**Rule (AGENTS §2b):** the pattern is enforced by a corpus lint
(`vl-difftest/tests/corpus_lint.rs`): blocking write in the window of an
edge-wait where the variable is read by an edge-triggered always → FAIL
(`// race-ok` to suppress a justified case). The lint caught a real residual
race on its first run over the corpus.

## 13. Meta: tooling of the agent, by the agent

- A Rust raw string literal pasted into a Python heredoc wasted a round —
  multi-language edits must go file→file (Write the snippet, insert it), never
  inline in another language's string literal (AGENTS §7b).
- `grep -c` exits 1 on zero matches and silently broke an `&&` commit chain,
  leaving PLAN.md uncommitted. Counting greps in gate chains need `|| true`,
  and the chain ends with `git status --short` empty (AGENTS §8b).
- `/tmp` is cleaned externally twice-daily on this machine: evidence artifacts
  (oracle-ref.vcd, fuzz repro) live in the repo or do not exist (AGENTS §9).

## 14. 第三輪實錄(2026-10-23,M7+yosys 收編弧)

### 14.1 外部語料差分是結構性 bug 的最快探針(Strategy)

自建 55 例 E2E 全綠,收編 yosys tests/simple 87 檔當輪即抓 14 個前端缺陷,
其中 Scope.push 巢狀丟前綴(`genblk1.foo.x`→`foo.x`)是**結構性 bug**——
自建語料全部單層 generate,永遠踩不到。教訓升格:內建語料=作者想像力的鏡像,
外部語料=他人想像力的採樣;**每個里程碑都應收編一批外部語料當差分面**,
lint-only 差分(accept/reject 判決)是成本最低的形態(無需 testbench)。

### 14.2 手術斷言律對「探針插拔」同樣適用(Incident)

調試插的 eprintln 探針,移除時用 `re.sub(r"if std::env::var_os\(...\)[\s\S]*?\n        \}\n", ...)`
——點星懶匹配跨過探針自己的 16 空格閉括號、一路吃到 8 空格 `}`,吞掉
GenBlock 臂+三個後續 match 臂(cargo `unclosed delimiter` 才暴露)。
規則入 AGENTS §10.6:**探針插拔=文字手術,錨點必須含探針獨有完整內容+精確行界,
禁 `[\s\S]*?` 點星跨行**;插探針時就寫成「刪除安全」形狀(單行/完整塊)。

### 14.3 oracle 行為差異先分「語義 vs 嚴格度」再對齊(Verdict discipline)

lint 差分第一輪 8 個 ORACLE-ONLY-REJECT 全是 `%Warning`(verilator 預設 fatal)
——換 `-Wno-fatal` 後反轉成 43 個 OURS-ONLY-REJECT,才是真差距清單。
同理 `3'h0`:位數檢查→值域檢查的差異、位置參數超界寬容——先判定
「這是 LRM 語義差還是工具嚴格度差」,嚴格度差一律跟 oracle,語義差才走 4-state/單測軌。

### 14.4 語料自審條款:X-clean 紀律會被新功能破壞(Process)

M7 寫 readmem 新例時放了 `x2` 十六進位數字——錄製後 expected 帶 2-state 投影(02),
我方 4-state 跑出 x2 = 假 FAIL。X-clean 紀律(D1)在**新增每個語料例**時都要
重新過一遍,不是入庫一次就永久成立;corpus_lint 可考慮未來加 x-數字掃描(R4 候補)。

### 14.5 錄製成本結構(Tooling)

oracle 錄製 ≈3.5 分/例(每例全量 C++ 構建)。全庫重錄 55 例 ≈3 小時——
**只錄新增/變更例**(symlink 臨時 corpus+`--corpus` 旗標)是必要工具,
本次為此加了 `--corpus`;未來可再省:verilated.cpp 公共物件共享預構建。

## 15. 第四輪實錄(2026-10-23,rVPI+wasm/FFI 三面+XLS 收編弧)

### 15.1 Facade 三面同落:能力落核心,不是落表面(Strategy)

rverilator 現有三個對外面(native CLI / vl-wasm 瀏覽器 / vl-ffi host FFI)。
`$readmem` 虛擬注入落 `Sim.inject_file` **一處**,三面當輪同時受益;反例
教訓是「能力只在 wasm 面實現」——那會讓 CLI 與 FFI 靜默落後。規則:
新能力先問「落核心還是落某一面」;核心純(零外部依賴)是三面都能便宜
移植的前提(500KB wasm、FFI golden PASS、進程內 0.106ms/run 全來自同一個零改動核心)。

### 15.2 planner→adversarial 抓的是「對現有代碼的斷言錯」(Process)

rVPI 設計的 4×P1 全是計畫對代碼的**斷言錯**:`find()` 會路徑壓縮需 `&mut`
(計畫稱 &self 即可)、ValueChange 排水點放在 exec_one 內(過度急切)、
top 模組名未存(cell 鍵是 `q` 非 `tb.q`)、CellKind 需逐站核實——
沒有一個是「實作時才會發現的 bug」,全部是**設計文件對代碼的錯誤宣稱**,
對抗審逐一 file:line 取證後在動工前消滅。附:subagent 額度中斷時,
對抗審可內聯執行——紀律的核心是「每個攻擊點必須代碼取證」,不是 agent 數量。

### 15.3 工具介面衝突以工具自身證據收斂(Tooling)

XLS 收編四輪 CI 迭代:`--ir_path`/`--output_type` 已廢、`--delay_model`
必須顯式、`--pipeline_stages` 必給、`#[pipeline]` 屬性已移除、`s1` 成保留字
——每一項都由工具自己的 `--helpfull`/parse error 定案,零盲猜。
= oracle-first 在「工具介面」域的同族律:上游工具的現行介面,以它的
輸出為 spec,不以記憶/舊文檔為 spec。

### 15.4 跨語言邊界契約由測試釘死(Contract)

wasm 面三條邊界契約各由一個 gate 釘死:①u64 跨界=BigInt(features.mjs
字面量首犯實錄)②回調只收 `{time,value,valueHex}`、JS→sim 中途重入禁
(註冊契約,測試對齊)③js_sys 表面 `#[cfg(target_arch="wasm32")]` 閘
(host 構建 stub)。ABI 邊界的型別投射不是文檔問題,是測試問題。

### 15.5 外部工具產物的正規化是收編協議的一部分(Harvest)

XLS codegen 輸出 `always_ff`(SV 關鍵字)且無 timescale——收割管線內
正規化(always_ff→always、注入 `` `timescale ``、raw 副本保留),並把
正規化記在快照所在處(corpus-external/xls/README.md)。語料的正規化
契約不在語料旁邊=下一個收編者必然重踩(TIMESCALEMOD 致命警告實錄)。

### 15.6 互動頁交付:curl 200 ≠ 目檢(Verification)

瀏覽器 demo 的 serve smoke(四資產 200+no-store)全綠仍不夠——真瀏覽器
點 Run 後才發現該驗的是 stdout 內容與 canvas 波形渲染;截圖入
docs/e2e-report/ 為證。機械閘驗「資產在場」,目檢驗「畫得對」;
兩者不可互替(SKILL wasm 增補已立法)。

## 16. 第五輪實錄(2026-10-24,spec 驗證層收割弧:corpus 59+RD 鎖步+MsbFirst 收口)

### 16.1 oracle 一致 ≠ 符合自身 spec——差分測試的哲學邊界有了實證(Strategy)

本弧最重的一課。I2C RTL 的 1-bit 埠寫在 `prescale [15:0]` 之後,被 ANSI 埠
範圍繼承(LRM 12.3.2)合法地放大成 16/8 位元——**verilator 同解,oracle 錄製
照樣一致,e2e 照樣 PASS**;golden model 按「宣稱的 spec」(1-bit 腳位)建模
才暴露。差分測試錨的是「行為與 oracle 一致」,錨不了「行為與宣稱意圖一致」;
spec 驗證層(ADR-0007)的存在意義由這個案例自身證明:golden 是「意圖」的
機器化身,oracle 是「事實」的機器化身,兩者分歧=意圖與事實的裂縫,這正是
使用者問的「驗證 spec 正確性」的工具化形態。附:修完 RTL 後 corpus 重錄,
輸出 byte-identical(1-bit 接線只動 bit0)——行為面無感,意圖面歸位。

### 16.2 測試設備單實例;獨立性只給受測物(Architecture)

golden master 原本內嵌自己的 slave+匯流排視圖,與驅動 DUT 的 BusDriver slave
形成兩個實例——同一份代碼,吃 RTL 輸出與 golden 輸出兩條**不同滯後**的沿流,
邊沿競態(ack 槽的 rise 落在 acking 清除前還是後)必然分岔。三輪競態硬化
(ack_high_seen 閘/相位修正/偵測器守衛)都只是推遲。判準一句話:**這件東西
是受測物還是測試設備?** slave 是測試設備→單實例共用(CmdInput 帶 Cell 匯流排,
golden 採用 DUT 所採樣的準位);golden master 的 FSM 才是受測物→獨立重實作。
附帶收穫:golden 從此看到的與 DUT 採樣的逐位元組相同,鎖步只剩真分歧。

### 16.3 取樣來源=性質所屬方的輸出,不是共享介質(Invariant design)

「寫字節 MSB-first」是 master 側性質,invariant 卻取樣匯流排準位——匯流排是
wire-AND 共享介質,slave 對 LSB=1 的位元組(隨機位元組一半)進入讀模式驅動
payload,採樣被污染且形態隨資料變化(數值對照完全對不上任何簡單變換)。
修法=取 master 驅動輸出(sda_oe/sda_o)。**除錯手法入冊**:先在正確 RTL 上
證明「這是誤報」,再 per-bit 傾印+逐 byte 分組對照(每 byte 一行 8-bit 樣本
+OK/MISMATCH 標記),污染形態自己現形——數值對照表勝過時間軸推理。

### 16.4 1-clk-stale 視窗的取樣相位律(Timing)

harness 的驅動器在 clk-low put、檢查讀的是本拍 posedge 前的暫存——任何
「每 SCL 週期取樣一次」的檢查,取樣點必須選「準位整窗恆定」的相位(SCL 高半),
低半取樣跨位元邊界(驅動在低半入口 posedge 切換,stale 暫存仍顯示前一位元;
0xA1 曾被讀成 0xD1)。slave 側同構:rise 取樣免疫 stale,fall 取樣讀到下一
位元。推廣:**凡「驅動在窗首切換、取樣在窗末」的協議,取樣點鎖窗中央**。

### 16.5 修復放對事件生命週期的正確端;切片手術後必 diff 對帳(Incident)

兩連:①首沿丟失修復——init_once 的播種放在寫入「後」執行太晚,沿的
prev/new 資訊已丟,正確位置=寫入「前」;時序敏感修復先畫事件生命週期
(種子→寫入→偵測)再下手術。②清儀器的 index 切片(`s[:i]+s[j:]`)把
rise/fall 分支整段靜默刪除——**被刪代碼的職能被其他層覆蓋(slave 的 ack
吞掉機制)所以 cargo test 全綠**,一輪後 slave 完全啞火才暴露。規則升格
(§10.6 同族):index 切片刪除後必印出被刪文本行數/逐行 review,或刪前
斷言區間內包含預期的錨點行;「測試綠」不是刪除安全證明。

### 16.6 偵測力的最終形態=指名道姓(Detection power)

LSB-first 負向測試原本只斷言「lockstep 整批分歧(≥40 違例)」——整批抓到
≠歸因(可能是任何 bug 的連坐)。升級=雙腿:lockstep 整批 **且**
`msb_first_bit_order` 指名。推廣:每個負向變種的斷言應指名「該變種對應的
那條 invariant」;做不到指名=invariant 層有缺口(本次缺口=取樣相位,收口
後自然可指名)。MsbFirst 誤報的解剖同時暴露 invariant 設計三缺陷(相位/
來源/expected 生命週期)——**負向測試綠+正向測試紅的交叉點=設計缺陷最密集處**。

## 17. 第六輪實錄(2026-10-24,M8 多 limb 值 + riverilog R0 弧)

### 17.1 計畫的根因診斷必附重現;對抗審的職責=把假設變量測(Strategy)

planner 對三個 lint 拒收案例的診斷是「需要寬運算」(從案例名 constpower 推斷);
對抗審實跑重現後翻案:**三案例全部死在 `Bits::zeros(w)` 的儲存上限**(512/3594/
256 位元寬埠宣告),constpower 的 `**` 本身只有 32 位元。同一輪還翻案兩處:
「比較 bug 是潛伏」實為活蟲(96-bit 帶號比較今天就與 oracle 分歧,重現釘在
/tmp/widecmp3.v);「~8 個 clone 點」實為 ~16 個破壞叢。**教訓**:計畫裡的根因
斷言若不帶「已重現的命令+輸出」,它就是假設;對抗審的第一動作=逐條跑重現,
不是讀邏輯。這是 §15.2(對抗審抓架構斷言錯)的升格:抓的不只是設計文檔的錯
宣稱,連同輪 planner 的診斷也照翻。

### 17.2 公共型別表示變更,先普查欄位存取面(Mechanical→SKILL 13)

第一版把 `Bits` 改成 enum,`cargo check` 立即炸出 30+ 處 `.width` 欄位讀取
(還會與 AST `NumLit.width` 撞名,盲目 sed 必傷)。終版=**struct 殼保留
`pub width` + 私有二態 repr enum**——欄位讀取面零改動,只有 6 個結構體字面值
與 2 個欄位存取叢需手術。**教訓**:改公共型別的表示法之前,先 `rg -c '\.field'`
普查讀取面;讀取面寬=保留 struct 殼+私有 repr,不裸 enum。附帶法則:結果寬度
≤128 一律 canonicalize 回快臂,熱路徑永不見 heap 形。

### 17.3 Limb/大數契約四律(每律一個真 bug;Mechanical→SKILL 14)

1. **符號中間值必須按 width 遮罩**:limb 級運算跑整 limb(64 的倍數),
   200 位元值佔 256 位元;neg/pow 的 padding 位元垃圾會跨輪複合(spow 產出
   [-8,-9,-9,-9] 型態的 limb)。修=每個帶號中間值過 `mask_width`。
2. **補數表達式必須 care 遮罩**:reduce_and 的 W 臂先遮罩輸入仍錯——
   `!d` 又把寬度以上的 0 padding 展開成 1,掃出一個假「0 位元」。修=遮罩
   表達式本身(`(!d & !m) & care`),不是遮罩輸入。
3. **同號比較不需反轉**:二補數 pattern 序=值序(同號時);只有跨號才走
   符號分支。我寫了 both-negative 反轉,單測全綠(測試只有跨號案例),
   **e2e `12_signed_ops` 首輪抓出翻面**——差分語料抓單測盲區再次實證。
   帶號測試必須覆蓋符號類四象限(正×正/負×負/跨號×2),不是「有一個帶號案例」。
4. **`vec![1u64; n]` 是 [1,1,1,1] 不是 1**:大數常數必須 limb-wise 構造
   (`vec![0; n]; acc[0]=1`)。pow 的初始 acc 用錯,全部 limb 垃圾——且因為
   輸入輸出都在寬域,單看結果「像是個數」極難目檢,是逐步探針才定位的。

機檢層=value.rs 的系統性 N-vs-W property 測試(全運算×寬度掃描:同一對值
在原寬與符號加寬後運算截回,結果必相等;自決定運算按 IEEE §5.4 邊界排除)。
**首跑即連抓兩個已出貨的真 bug**,都不在任何人手工測試的覆盖裡:
①`sadjust` 到 >128 寬時,源寬非 64 對齊的情境頂 limb 的剩餘位元沒被符號填充
 (W 臂缺了 N 臂的 `!low_bits(sw)` 那步);②**pre-M8 就繼承的 N 臂 ashr
 填充溢位**——`fill << w` 在 u128 只能表達 bit 127,w+s>128 的帶號右移
 (127 位元 -1 >>2)填充位元靜默丟失。修法=超界路由 limb 路徑。加上
 之前 e2e 抓的 both-negative 比較反轉,這一弧的符號運算類真 bug 共三個,
 全部由機械閘(而非人審)抓出——3-tiers 的價值主張在本輪有了最直接的一次
 實證。

### 17.4 CLI 契約=實測枚舉;清單類從 man page 取,行為類從實跑取(Riverilog R0)

R0 幾乎發生一次「盲猜立法」:我為 `-g` 旗標拼了一份 feature 清單寫進代碼,
隨後對照真機才發現 **oracle 對任何未知 -g 參數(連 no-bogus)都回同一條
"Unknown/Unsupported Language generation" 訊息**——正解=照鏡這一條可觀測
行為,真 feature 登記表 R1 才從 man page 建。其餘契約同法釘死:`iverilog -h`
exit 1(vs `vvp -h` exit 0 的不對稱)、illegal option 打印首 dash 後一個
字元、無源檔文案、預設輸出 a.out。**教訓**:行為類契約只認實跑輸出;清單類
(旗標表/feature 表)只認上游文檔;兩者都沒有就釘「未知=硬錯」,不發明清單。

### 17.5 期望檔禁機器相關字串;錄製 cwd=案例目錄(Riverilog R0)

$finish 行內嵌編譯期路徑——從 repo 根錄製會把 `corpus/iv-e2e/01_hello/tb.v`
或絕對路徑烤進 goldens(跨機器必漂)。修=在案例目錄內編譯,golden 穩定為
`tb.v:6:` 相對形。**法則**:expected.* 落檔前掃一眼絕對路徑/時鐘/主機名;
錄製協議裡 cwd 是契約的一部分,寫進 harness 而不是靠紀律。

### 17.6 Chase-ledger:能力未到,期望先行(Riverilog R0,新模式)

R0 引擎還不存在,但 10 例期望已錄製入庫。閘的形態:pending 案例必須回答
「里程碑硬錯」(exit 2+R 指針)——這本身就是被釘死的契約;某案例突然能跑而
`case.json` 沒翻 `enforced: true` = DRIFT 紅燈(防假綠的反向閘)。里程碑落地
時翻開案例=同輪 commit,閘自動切精確數比對,harness 零改動。**這是 EDA 工具
漸進式交付的可複用模式**:期望先行、契約先行、能力逐案例點燈,每一步都有
機械閘看著。

### 17.7 Provenance churn 與 oracle 升級(流程)

全量 `--record` 在 oracle 5.050→5.052 後 churn 了 58 個 provenance 檔
(版本橫幅+walltime);**正規化期望零漂移**=一次免費的「升級無語義變化」證明。
處置=還原 provenance churn 保持弧手術性;churn 本身是 oracle 升級的信號,
要讀不要瞞。錄製協議註記:全量重錄只在 oracle 升級輪做,日常用 `--corpus`
指向父層錄單案例。

### 17.8 對 3-tiers 的觀察

本輪兩個弧的四層落點:機檢=N-vs-W property 測試+chase-ledger 閘+既有
regress 全閘(首輪抓 both-negative 翻面);skill=鐵則 13-15;AGENTS=§10.8;
證據=ADR-0008+RIVERILOG-PLAN §0/case60+cli goldens。值得記錄的後設觀察:
**「單測全綠+e2e 紅」的案例(both-negative)在六輪裡第二次出現**(§14 的
race-free TB 也是)——單測證明「我寫的規則對」,e2e 證明「規則覆蓋了真世界」;
兩者不可互替,且新運算子的單測清單應該機械化(符號四象限/寬度邊界 63/64/65/
127/128/129/x 位置),不靠作者直覺列。
