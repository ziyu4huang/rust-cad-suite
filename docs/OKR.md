# rust-cad-suite OKR(2026-Q4;证据指针制,反灌水:每 KR 必须指向可重跑命令/工件)

> 規則:KR 證據只准指向既有收據(可重跑命令、機械計數、git hash、oracle 實跑輸出)。
> 季末 10-31 check-in;證據死指針由 agent 自查(本 repo 無 cron,由 regress.sh 順帶驗)。

## O1: Verilator 等价模拟器(Phase 1)达到可用的兼容性

- **KR1.1 E2E 差分通过率 ≥95%(对 Verilator 5.050,byte-level)** ✅ 2026-10-23
  当前:**55/55 = 100%**(2026-10-23,M7 落地:30_gen_mux 轉綠+新增 52-55 四例
  mem/readmem/gates/mem_nba 全部 oracle 錄製)。
  证据:`cargo run -p vl-difftest`(exit 0,== 55/55 ==);报告 [docs/e2e-report/index.html](e2e-report/index.html);
  期望文件 `corpus/e2e/*/expected.stdout` 已入 git。
- **KR1.7 lint-only 差分軌(yosys tests/simple 收編,2026-10-23 立)**
  當前:**80/87 = 92.0%**(首輪 44→二輪 73→三輪 77=u128 ADR-0006→四輪 80=IndexedPart lvalue+埠屬性+defparam 三修)。
  證據:`bash scripts/lint_diff.sh corpus-external/yosys-simple corpus-external/yosys-simple/LINT-DIFF.tsv`
  (oracle=verilator --lint-only -Wno-fatal,判決只看 error 級);TSV 已入 git;
  餘 6 差異=>128 位三檔(多 limb 未來 ADR)/intra-assign/reals/字串(全 M8)。
  sv-tests 第三批 260 檔(ISC)=資訊值:81/260,多數為 SV-only 語法(logic/typedef/fill),非 V2005 範圍。
- **KR1.8 VCD 單一核心三面共用(agentic web 整合軌,2026-10-23 立)**
  當前:✅ vl-vcd(writer+reader)為 CLI/wasm/FFI/difftest 唯一 VCD 實作;JS 手搓 parser 已退役。
  證據:`cargo test -p vl-vcd`(10 測,fixture 含磁碟真樣本);wasm `vcd_parse` 閘
  (BigInt 時序+錯誤路徑);ffi `vcd_read.mjs`(agent 讀波形:dump→解析→斷言);
  e2e 57/57 零漂移;瀏覽器 XLS 獨立驗證卡 ✓ byte-exact ×3(目檢=docs/e2e-report/wasm-demo-v2-verified.png);
  XLS 依賴結論=docs/XLS-VERILATOR-STUDY.md;GTKWave 研讀=docs/GTKWAVE-VCD-STUDY.md;ADR-0004。
- **KR1.9 SVG 波形渲染層+GUI 憲法(ADR-0005,2026-10-23 立)**
  當前:✅ vl-vcd render 為 CLI/wasm/FFI 唯一波形渲染器;demo canvas 全退役;
  GUI=web/底層=Rust/CLI-web 同源 憲法入 AGENTS §10.8。
  證據:`cargo test -p vl-vcd`(render 7+golden 4,goldens/counter.svg byte-diff,
  CLI stdout=wasm one-shot 同字節);regress wasm vcd_svg+ffi vcd_svg 閘;
  目檢=docs/e2e-report/wasm-demo-v3-svg-verified.png;vcd2svg --help。
- **KR1.2 回归闸机械化(≥80% 断言入 cargo test)**
  证据:`crates/vl-difftest/tests/regression_gate.rs`;`bash scripts/regress.sh` 全绿。
  期望文件常驻 → 无本机 oracle 也能跑闸。
- **KR1.3 单元轨:4-state 语义按 IEEE 1364-2005 §5 真值表钉死**
  当前:8 值语义测试 + 12 词法 + 17 前端 + 2 sim_gate + 1 诊断 = 41 tests 全绿。
  证据:`cargo test`(全绿);`crates/vl-sim/src/value.rs#tests`。
- **KR1.4 里程碑指針契约:未支持構造 100% 硬錯(exit≠0)且帶 `milestone Mx`**
  证据:`counter8` 早期指針實錄;AGENTS §3;M7 後指針餘=real/宽>64/多維 unpack(M8)。
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
- **KR2.7 第四輪反思落四層**(2026-10-23,rVPI+wasm/FFI+XLS 弧):REFLECTION
  §15 六條(facade 三面同落/對抗審抓架構斷言錯/工具自證迭代/ABI 邊界契約
  測試釘死/外部產物正規化隨快照/curl≠目檢)各有實錄證據;AGENTS §10.7
  三面 facade 紀律;SKILL wasm 增補;證據=wasm-demo-verified.png+四閘入 regress。
- **KR2.6 robustness 軌與 pass-rate 軌分立**:fuzz 600 迭代入 cargo test,
  失敗自動落盤復現;首輪三 bug 在事故簿。✅

## O3(Phase 2+,未開工): yosys 等价綜合核
