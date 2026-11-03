# ADR-0012: rtl-to-spec 插件弧(Rust 核心 → wasm → MCP server → zcode plugin)

日期:2026-11-03。狀態:**接受**(正典企劃=docs/RTL2SPEC-PLUGIN.md,實作記錄=
同文「實作記錄」節)。脈絡:使用者原話目標——①讓 agent 有工具分析 RTL、
做完整測試與找破口;②開發 rtl-to-spec plugin,從 RTL 抽取**可檢驗的行為
規則集**(有意義的規則書,不是把 RTL 機械轉成文件);③插件純 Bun 可跑、
跨平台(Rust 經 wasm,bun/node 都能執行);④形態完整一路 Rust 核心→wasm→
MCP server(+skills)→zcode plugin 包;⑤最終用真實 RTL 實測,spec 經得起
「有意義規則書」評審(機械判據+獨立評審員),不是 doc-dump。

## 決策

1. **D1 核心=sibling crate `rtl-spec`,不擴 analyzer-core**:analyzer-core
   立檔自劃禁區「查詢集=既有 IR 欄位投影,求值/解析=禁區」(lib.rs:2-7),
   規則歸納是新語義層;弧約束「禁破壞其測試與 API」——零接觸是唯一保證。
   依賴邊=`rtl-spec → {analyzer-core, vl-syntax, ry-ir, rcs-diag}`,深提取
   走 `Analysis.sv`/`Analysis.rtlil` 原生逃逸口(僅 Rust API 可及的合法後門)。
2. **D2 RTLIL 側 evidence=結構路徑證形,驗證閘 V4/R1 只對 SV 側斷言**:
   ry-ir AST 無 span(view.rs:26-27 span 恆 None)+`line_col` RTLIL=Err
   (lib.rs:127-138);RTLIL evidence 帶 `file="<name>.il"`+`line=0`+
   `path="module/process[2]/sync.posedge"` 結構路徑;驗證集 13 檔全為 SV
   語料,R1(逐位元組溯源)可全量機械。
3. **D3 wasm=新 crate `rtl-spec-wasm`+`--target web`+顯式位元組 init+JSON
   字串進出**:沿 regress 既證的 web 目標(scripts/regress.sh:201;vl-wasm
   glue 在 bun 下 `await init()` 既成事實);**零 js_sys**(純函數進出無回調
   無狀態);行號 u32 內無 BigInt 面;不採未驗證的 `--target bun`。
4. **D4 MCP=stdio newline 分幀 JSON-RPC 2.0,四工具**:工具內錯誤(檔案
   不存在/path+source 雙缺/未支援 lang)走 `isError:true` result;協議級
   錯誤(未知名=-32601/壞參=-32602)走 JSON-RPC error(勘勘察事實 §6
   excalidraw SDK 契約);**stdout 純淨律**:stdout 只出 JSON-RPC 幀,診斷
   全走 stderr。
5. **D5 plugin 包=plugin/rtl2spec/`.zcode-plugin/plugin.json`+mcpServers
   stdio 形+`${ZCODE_PLUGIN_ROOT}` 變量**:對齊 excalidraw 0.3.4 實測
   manifest schema;裝載到使用者 ~/.zcode 不在弧內(repo 內校驗+dry-run 為止)。
6. **D6 閾值 V1..V6**(見 RTL2SPEC-PLUGIN §8.3):V1 全集 ≥50/單檔 ≥3/
   truncated 非空→該檔 FAIL;V2 行為占比全集 ≥60% 任一檔 ≥40%;V3 類別
   ≥4(floor=ask 契約,目標 ≥6);V4 溯源 SV 側 100% 逐位元組;V5 反
   doc-dump 四判據 100%;V6 13/13。降閾唯一路徑=本 ADR 修訂+閘重跑,禁靜默。
7. **D7 規則陳述=中文模板+源碼標識符原文名**:工作語言中文;標識符不改名,
   保證 snippet 對拍/評審溯源一致;模板由提取器按 category+check 機械生成,
   禁自由散文。
8. **D8 checker 裝載 dist 同一 wasm 產物**:閘驗出貨物而非平行實現;
   native/wasm parity 由 S4 單測+冒煙另覆蓋(兩閘互補不重複)。
9. **D9 兩插件閘以雙守衛降級形收編 regress.sh(S7)**:守衛=
   `crates/rtl-spec-wasm/pkg` 在場+bun 在場;在場時
   run-mcp-e2e.sh/run-rtl-validation.sh exit≠0 即整閘紅;既有基線斷言
   (SUMMARY ys=469/479、LIVE ys=456/456)逐字不動;總閘四件套保持直調雙保險。
10. **D10 命名 `rtl-spec`/`rtl-spec-wasm`(crate)/`rtl2spec`(plugin)**:
    避讓既有 `vl-spec`(spec 驗證核心,ADR-0007)與 `spec-i2c`;`rtl-` 前綴
    標「提取側」,`vl-` 留給「驗證側」;fingerprint=拼接鍵串零哈希(D13)。
11. **D11 MCP server=手寫純 JS(ESM .mjs),源=產物零編譯步**:雙運行時
    兼容不賭 node type-stripping;類型契約面=手寫 server.d.ts。
12. **D12 opts 標量化**:`extract_rules(input, max_rules_per_module: usize)`
    收標量、`scan_gaps` 無 opts——庫內零 JSON 讀取器(只寫不讀)。
13. **D13 fingerprint=拼接鍵串,零哈希**:`source|category|module|subject|
    when|effects`,source 維入鍵(兩檔同名 module i2c_master 實證);
    repo 無 SHA 實現,零依賴原則下不手搓。

## 與既有 ADR 關係

- **ADR-0007(spec 驗證層)**:本弧在其**前面**補「提取」一環——rulebook
  的 check.form 五形可對接 vl-spec 的 Invariant/GoldenModel 三層形態做
  驗證閉環,對接本身留後續弧,本弧不實做。
- **ADR-0011(分析器服務)**:本弧零接觸 analyzer-core API;深提取走
  Rust 逃逸口(跨進程協議不暴露 sv/rtlil 逃逸口,本弧同守此界——wasm 面
  收的是 JSON 字串進出的提取**結果**,不是逃逸口本身)。
- **ADR-0005(GUI 憲法)**:本弧無 GUI/wasm demo 觸點;波形/SVG 不涉及。

## 機械閘

1. `bash plugin/rtl2spec/run-mcp-e2e.sh`——MCP 端到端七斷言(initialize/
   tools list 4 工具/extract_rules 正樣/gaps Latch 正樣@* 形/負樣三形:
   檔案缺失 isError+雙缺參 isError+未知名 -32601)。
2. `bash plugin/rtl2spec/run-rtl-validation.sh`——真實 RTL 驗證閘
   (13 檔 in-git 語料,V1..V6 機械判據全過→validation-report.md)。
3. regress.sh S7 起帶雙守衛插件塊(D9)。
4. 總閘四件套:`cargo test --workspace` ─ `bash scripts/regress.sh` ─
   上述兩插件閘。

## 非目標

見 RTL2SPEC-PLUGIN §11(13 條:不裝載使用者環境/不動前弧 API 與語料/
GPL 不入集/不做 SVA/不做參數求值/不做跨檔層級/不做 TB 執行/禁手寫
golden/MCP 只 stdio+四工具/不採 --target bun/不做 GUI/不做 4-state
規則表達/不做層級推理)。

## 實作記錄

- 2026-11-03 S1:本 ADR 全文+PLAN P3.8 行+里程碑表 RTL 行+SKILL
  「rtl-to-spec 增補」節骨架+REGISTRY 預登記落盤;rg 交叉引用自驗
  (本檔/正典/PLAN/SKILL/REGISTRY 互相指到的路徑與閘名逐字)。
