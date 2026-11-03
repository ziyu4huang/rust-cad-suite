---
name: rtl-gap-hunt
description: 掃描 RTL 的測試缺口(鎖存推斷/不完整 case/無驅動讀取/缺復位覆蓋/組合環/多驅動衝突/邊沿阻塞寫,每項帶建議測試)——破口、latch、組合環、多驅動、測試缺口時使用;破口報告=測試缺口清單
---

# 破口獵捕技能(rtl_scan_gaps)

## 觸發詞

破口/latch/組合環/多驅動/測試缺口/「這段 RTL 還缺哪些測試」。

## 流程

1. 呼叫 MCP 工具 `rtl_scan_gaps`(`path`|`source`/`lang`)。
2. structuredContent:`gaps`(GapItem 清單)+ `test_plan`(suggested_test
   字串清單,已按破口序)。
3. 按 class 歸併:latch/incomplete_case/undriven_read/missing_reset/
   comb_loop/multi_driver/blocking_in_seq(G1..G7)。
4. **suggested_test 逐條轉 TB 計劃條目**——「完整測試」的操作定義=
   suggested_test 全部有對應 TB 條目(本工具產清單,不執行 TB)。

## confidence 差異化

- `certain`(G1/G4/G5/G6/G7,結構可證):可直接轉測試或修設計。
- `heuristic`(G2/G3,值域覆蓋與驅動圖在 include/宏場景受限):先人工核
  再採——誤報形以正負樣 fixture 釘死(每類判定配「觸發形+零誤報形」)。

## 要點

- 破口=行為未定/未覆蓋點=測試必須釘住(或設計必須修掉)的缺口;
  破口報告**就是**測試缺口清單,不是 lint 風格清單。
- RTLIL 面 v1 子集:G1=switch 無 default 臂(RTLIL 保持值語義=鎖存,
  certain)、G5=connect 自環;其餘類不產(不硬湊,見正典 §4.2)。
- TB 檔(含 `$finish`)整檔跳過,skipped 注記 `tb_file`——不是錯誤。

## 契約位置

正典=docs/RTL2SPEC-PLUGIN.md §3(七類判定/置信度/與完整測試的關係);
ADR-0012。
