---
name: eda-dev
description: rust-cad-suite 开发正典——AI agent 做 EDA/IC 工具(模拟器/综合器类)的行为规则与流程
---

# EDA tool development skill(rust-cad-suite 正典)

适用:在本 repo 重写/扩展 oss-cad-suite 等价物(模拟器、综合器、PnR)。
配套铁则=repo 根 `AGENTS.md`;路线=PLAN.md;组件关系=docs/REGISTRY.md;
目标与证据=docs/OKR.md;本 skill 是 3-tiers+1 中的「正典层」。

## 铁则(执行顺序即优先级)

1. **Oracle 即 spec**。兼容产品的正确性=参考工具可观测行为。语义分歧时
   oracle 赢,即使 LRM 站你这边。期望输出必须 `vl-difftest --record` 录制,
   禁手写(实录:7 例手写期望全败,见 docs/REFLECTION-agent-hardening.md §1)。
2. **双验收轨**:byte-diff corpus 必须 X-clean 且无同边沿竞争(x/z 传播与
   IEEE 非确定序不可与 2-state oracle 对拍);4-state 算子正确性走 IEEE §5
   真值表单测轨。竞争 TB 改中段写(范式=corpus/e2e/26、43)。
3. **不支持=带里程碑硬错**:`error ... (milestone Mx)`,exit≠0。禁静默跑偏。
4. **恢复必保前进**:每个解析器/执行器恢复循环带「pos 未动→bump」守卫,
   并配一个会触发的挂死 fixture(实录:task ANSI integer 端口死循环)。
5. **算子与真值表同生**:新 4-state 算子同轮写 x/z kind 保全单测;
   格式化(%d/%b/%t)是契约,golden 钉死。
6. **未解析即硬错**:名字/模块/函数解析未命中=报错带 span;禁盲字符串前缀。

7. **归一化=契约**:只用精确前缀过滤 oracle 输出(`- `/`-Info`/`-Warning`/
   `-Error`),裸 `-` 是设计输出;录制带 `--trace`;re-record 后逐 case 读
   `git diff --stat corpus/e2e`。
8. **竞争免疫模式**:`@(posedge clk); #1; <写>;`——corpus_lint R1 机械执法,
   `// race-ok` 显式豁免。
9. **robustness 轨独立**:fuzz_smoke 常驻;pass-rate 绿≠健壮。
10. **计划项必带 LRM 条号**:条文已说明实现精确时,工作项作废(§5.1.4 实录)。
11. **分歧/事故先登记**:docs/REGISTRY.md 的 Divergence / Incident 两簿。

## 标准循环(语义开发)

```
1. 写/改 case(corpus/e2e/,X-clean、race-free、oracle 可终止)
2. cargo run -p vl-difftest -- --record        # oracle 录期望
3. cargo run -p vl-difftest -- --only <case>   # 我方对拍
4. 失败→最小复现(/tmp 可弃,关键复现进 corpus)→修实现→cargo test
5. 全绿:bash scripts/regress.sh → 提交(精确 add)
```

## 定位工具箱

- 挂死:`sample <pid> 1`(macOS)看栈;`vl-difftest --only <case>` 二分。
- 语义分歧:最小 .v 并排跑 `rverilator --binary` vs
  `verilator --binary --timing`;oracle 输出即答案。
- /tmp 会被外部清:关键复现移入 repo。

## 3-tiers+1(改规则时执行)

任何新铁则必须同轮落四层,缺一不算数:
①机械闸(scripts/regress.sh:E2E 差分精确数 100%/lint parity 基线/wasm 差分+
  功能+vcd_parse/demo serve smoke+corpus 白名单/ffi 三闸含 vcd_read——
  或 vl-difftest 内断言)
②本 skill 的正典条文
③AGENTS.md 对应节
④证据页/报告(docs/e2e-report/)可核。

## rVPI 增補(2026-10-23,ADR-0003)

rVPI(`crates/vl-sim/src/vpi.rs`,Rust 原生 VPI 結構等價)新能力必須四證同落:
①vpi_gate 單測(firing-point 斷言)②two-paths-vs-corpus(VPI 取樣==oracle 錄製行)
③rVPI×VCD 內部交叉 ④REGISTRY 分歧行。C-VPI 語義差異先判「語義 vs 嚴格度」,
結構性差異(觸發點/投递語義)登記分歧而非硬仿;禁向 C ABI 兼容漂移(ADR-0003 圍欄)。

## wasm/host facade 增補(2026-10-23,ADR-0003 延伸)

模擬器對外三面(native CLI / vl-wasm 瀏覽器 / vl-ffi host FFI)的新能力,
四證同落(繼承 rVPI 四證制):
①wasm 差分+功能閘(`wasm-test/run|features|vcd_parse.mjs`,regress 自動)
②host FFI 差分(`bun-example/run.mjs`+`spawn.mjs`+`vcd_read.mjs`,regress 自動;dylib 缺=閘略過並提示)
③demo serve smoke(資產 200+no-store+corpus 白名單探針,regress 自動)
④**互動頁交付前必真瀏覽器目檢**(點 Run→stdout→canvas 波形;截圖存
docs/e2e-report/,curl 不算目檢)。

**VCD 正典(ADR-0004)**:讀寫一律走 `vl-vcd` 單一核心——CLI/wasm/FFI/difftest
共用同一 reader;新 tier 手搓 VCD parser=違例(JS 手搓版已退役);
新容忍度先入 docs/GTKWAVE-VCD-STUDY.md 採用/拒收表+ADR-0004 修訂;
writer 值介面=4 慢字串,零漂移錨=e2e expect.vcd byte-diff。

契約備忘:wasm 回調只收 `{time,value,valueHex}`(JS→sim 中途重入=禁,
讀值在 run 前後);u64 跨界=BigInt;`$readmem` 在瀏覽器一律走虛擬注入
(`inject_file`/`run_verilog_with_files`),不碰真 FS;js_sys 表面一律
`#[cfg(target_arch = "wasm32")]`(host 構建用 stub)。
