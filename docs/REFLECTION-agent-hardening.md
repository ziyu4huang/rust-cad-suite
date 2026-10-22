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
