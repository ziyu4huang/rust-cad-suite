---
name: rtl-conformance
description: 從 RTL 生成模板 testbench 計劃與骨架(rtl_tb_plan),用於行為復原力 conformance 對拍;不是正確性證明
---

# TB 計劃技能(rtl_tb_plan)

## 觸發詞

testbench、TB 生成、conformance、對拍、行為復原力。

## 流程

1. 呼叫 MCP 工具 `rtl_tb_plan`(`path`|`source`/`lang`(缺省按副檔名
   `.il`→rtlil)/`module`?/`seed`?/`cycles`?/`flush`?,缺省一律省略或 0
   =核心默認)。
2. isError=true→原文轉述錯誤文本(`milestone TB1`=範圍外硬錯,不重試、
   不改寫、不猜 workaround);否則讀 structuredContent(`ok:true`)的 plan
   (module/params/ports/clocks/resets/sens_edges/data_inputs/outputs/
   obligations/waivers/scenes)與 `template_v`。
3. 把 `template_v` 寫成檔(建議 `build/tb_<module>.v`),交使用者環境的
   模擬器(iverilog -g2005 等)編譯運行——**skill 不判定結果、不計分、
   不生成期望值**;期望輸出由參考實現錄製(與 oracle 紀律同源:TB 只
   觀測不自判)。
4. 讀 TB 輸出(協議 v1):`SCENE<n> <name>` 頭→場景內 `  key=value` 行→
   `DONE` 尾;`WATCHDOG`/`TIMEOUT` 行=場景未走完,如實上報。

## 讀法

- `resets=∅` 的機械含義:規則書無 guard 規則(R1-11 口徑)→模板不生成
  S-RST/S-ARST,復位路徑只經隨機場景激勵;殺傷率解讀見證據頁機械註記,
  禁誤讀為規則書或設計缺陷。
- `obligations[].observe`:`port`=規則主體是輸出端口(可直接打印);
  `indirect`=主體是內部訊號,效應只能經下游輸出端口表面化——模板腿以
  非定向場景(`COVERS: <n>: *`)覆蓋;**禁層次引用 DUT 內部訊號**。
- `waivers`:`x-valued`=規則值含 x/z 位字面量(觀測面無意義);TB 源碼
  `// WAIVE: <rule_id> <reason>` 註釋同義。

## 禁止宣稱

- 「TB 全過=設計正確/意圖正確」——conformance 度量的是**行為復原力**,
  不是意圖正確性(oracle=原版 RTL 在 iverilog 下的可觀測行為)。
- 模板 TB 是基線腿(T 模板腿),不是完備驗證;判別力數字由 conformance
  閘(變異矩陣)實測,禁由本 skill 宣稱。

## 契約位置

正典=docs/RTL2SPEC-PLUGIN.md §19.3(TB 生成器/雙腿分工/協議 v1);
ADR-0013(D-C2/D-C3);機械閘=run-plugin-check ⑥(skills 內 `rtl_` 標識符
⊆ MCP 工具名)。
