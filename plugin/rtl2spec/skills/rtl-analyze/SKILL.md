---
name: rtl-analyze
description: 分析 RTL 模組結構(埠表/參數/例化關係/診斷)——rtl2spec 插件的結構投影報告技能;RTL 分析、模組結構、埠表、例化關係時使用
---

# RTL 分析技能(rtl_analyze)

## 觸發詞

RTL 分析/模組結構/埠表/例化關係/診斷 RTL。

## 流程

1. 呼叫 MCP 工具 `rtl_analyze`(入參:`path`(相對 cwd)或 `source` 皆可,
   皆給以 `path` 為準,皆缺=工具級錯誤;`lang` 缺省按副檔名 `.il`→rtlil,
   其餘→sv)。
2. **先讀 `has_errors` 與 `diagnostics_count`**:錯誤在場時,後續規則書
   (`rtl-rulebook`)與破口(`rtl-gap-hunt`)結果標「受限」——parse 錯誤
   進結構化輸出(部分模組照提),不是工具失敗。
3. 產出結構摘要:模組清單(name/params/ports/nets/cells/proc_count/
   assign_count)+ `instance_edges`(例化圖)。

## 要點(防誤讀)

- **投影語義的已知形**:參數化寬度→`width:0` 且 parametric 標記(analyzer-core
  立檔契約,不求值參數);RTLIL 側無 span(ry-ir AST 無 span)——結構
  摘要無位置欄位屬正常,不是缺數。
- RTLIL 名字逐字保留(`\` 前綴在場;ry-ir 不做 unescape)。
- `instance_edges` 只含目標命中本輸入模組名的邊(閘原語/`$cell` 不成邊)。
- 本工具不做語義求值/名字解析(analyzer-core 禁區);深行為面用
  `rtl-rulebook` 與 `rtl-gap-hunt`。

## 契約位置

正典=docs/RTL2SPEC-PLUGIN.md §6.1;ADR-0012;資料源=rtl-spec
`analyze_summary`(analyzer-core Analysis 公開查詢直讀組合,零新語義)。
