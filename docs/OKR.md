# rust-cad-suite OKR(2026-Q4;证据指针制,反灌水:每 KR 必须指向可重跑命令/工件)

> 規則:KR 證據只准指向既有收據(可重跑命令、機械計數、git hash、oracle 實跑輸出)。
> 季末 10-31 check-in;證據死指針由 agent 自查(本 repo 無 cron,由 regress.sh 順帶驗)。

## O1: Verilator 等价模拟器(Phase 1)达到可用的兼容性

- **KR1.1 E2E 差分通过率 ≥95%(对 Verilator 5.050,byte-level)**
  当前:50/51 = **98.0%**(2026-10-21,51 例含 VCD case)。
  证据:`cargo run -p vl-difftest`(exit 0);报告 [docs/e2e-report/index.html](e2e-report/index.html)
  (由 `--html` 机械生成);期望文件 `corpus/e2e/*/expected.stdout` 已入 git(7e68cdd 之后的 commit)。
  唯一 FAIL=30_gen_mux 存储器数组,声明的 M7 硬错(非缺陷)。
- **KR1.2 回归闸机械化(≥80% 断言入 cargo test)**
  证据:`crates/vl-difftest/tests/regression_gate.rs`;`bash scripts/regress.sh` 全绿。
  期望文件常驻 → 无本机 oracle 也能跑闸。
- **KR1.3 单元轨:4-state 语义按 IEEE 1364-2005 §5 真值表钉死**
  当前:8 值语义测试 + 12 词法 + 17 前端 + 2 sim_gate + 1 诊断 = 41 tests 全绿。
  证据:`cargo test`(全绿);`crates/vl-sim/src/value.rs#tests`。
- **KR1.4 里程碑指針契约:未支持構造 100% 硬錯(exit≠0)且帶 `milestone Mx`**
  证据:corpus/e2e/30(M7 指針);`counter8` 早期指針實錄;AGENTS §3。
- **KR1.5 M6 VCD 對拍(alias 折疊規則化比對)** ✅ 2026-10-21。
  證據:`cargo test -p vl-difftest --test vcd_gate`(4 例 oracle-ref.vcd,16/16 訊號、90 采樣點零失配);`vl-difftest --vcd-xcheck ours oracle`;參考錄製=scripts/oracle_vcd.sh。
- **KR1.6 前端 fuzz 閘(語料變異+隨機字節,無 panic/無掛死)** ✅ 2026-10-21。
  證據:`cargo test -p vl-syntax --test fuzz_smoke`(600 迭代固定種子;首輪抓三個 UTF-8 健壯性 bug 並修復)。

## O2: 開發 agent 能力硬化(EDA 工具域)

- **KR2.1 反思文檔帶實錄證據** — `docs/REFLECTION-agent-hardening.md`(7 節,
  7 例手寫期望敗給 oracle 在案)。
- **KR2.2 鐵則 3-tiers+1 落地率 100%**:每條新鐵則同時落
  ①機械閘(scripts/regress.sh + vl-difftest 閘)②正典(.agents/skills/eda-dev/SKILL.md)
  ③AGENTS.md ④證據頁(docs/e2e-report/)。
  證據:本 commit 同輪四件齊;AGENTS §10。
- **KR2.3 定位流程機械化**:掛死=sample;分歧=最小復現並排跑;
  runner 帶 `--only` 二分與每例看門狗。證據:AGENTS §9(實錄兩起)。
- **KR2.4 第二輪反思落四層**(2026-10-22):REFLECTION §8-§13 六條新規則各有
  機械閘(corpus_lint R1-R3 + normalization_contract + fuzz_smoke)、skill、
  AGENTS §10.5、報告頁;證據:`cargo test -p vl-difftest --test corpus_lint`
  (首跑抓 43 號殘留競爭)。✅
- **KR2.5 分歧登記簿與事故登記簿常態化**:docs/REGISTRY.md D1-D8 / 10 起事故
  五欄入帳;新分歧「先登記再動工」。✅(2026-10-22 建簿)
- **KR2.6 robustness 軌與 pass-rate 軌分立**:fuzz 600 迭代入 cargo test,
  失敗自動落盤復現;首輪三 bug 在事故簿。✅

## O3(Phase 2+,未開工): yosys 等价綜合核
