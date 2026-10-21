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
①机械闸(scripts/regress.sh 或 vl-difftest 内断言)
②本 skill 的正典条文
③AGENTS.md 对应节
④证据页/报告(docs/e2e-report/)可核。
