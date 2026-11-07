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
14. **D14 golden 自錄快照(§16 立項;開弧對抗審 R16-1/R16-5 修訂)**:
    golden=提取器**自身輸出快照**(非 oracle 期望;§11.8「禁虛構
    golden」讀法=錄製產生禁手寫),用途=行為漂移檢測;釘選 37 檔×
    rules+gaps=74 份 JSON byte-exact,走 wasm 腿(D8 延續:閘驗出
    貨物);`--record` 顯式開關+錄前**進程內**重複調用自檢(跨進程在
    wasm32 腿無判別力:RandomState=調用流位置函數,跨進程首跑 byte
    一致而同進程抖——12 檔 flapper 實證);錄後 `git diff
    plugin/rtl2spec/golden` 逐案對帳=re-record 語義事件紀律;錄製
    順序鐵則=G4+G1 非確定源(gaps.rs:670/402/439)修復且 37 檔
    flapper 清零前禁錄 gaps golden;**golden 快照禁依賴調用流位置**
    (RTL2SPEC-PLUGIN §16.1.5 契約②)。
15. **D15 fixture 矩陣斷言規格層**:G1..G7 正負樣 expected findings
    (類/confidence/條數/id 形)=**斷言規格**(手寫合法,與 sv_rules.rs
    單測同源),與 golden 快照(錄製禁手寫)劃界;含掛死守衛
    (scopes.v 形復現 fixture+10s 守衛)。
16. **D16 評測擴面分層**:L1 質量層 93 檔(yosys-simple 87+repo 自有 6;
    per-file 三檔分類判據,閘級硬 FAIL=終止性/V4/V5-①②④/V5-③ 指紋
    重複/V6 覆蓋)+L2 容錯層 260 檔(sv-tests:零 panic+可終止+parse_err
    如實計數,192 parse_err 不 FAIL);**13 檔驗證集與 V1..V6 閾值
    全部不動**;報告=docs/RTL2SPEC-EVAL.md 機械生成禁手改。
17. **D17 拓展接入=靜態詞表+掃描序註冊點(§17;2026-11-04 增訂)**:
    `GapClass` enum 窮盡 match(as_str/prefix)=編譯期 registry(漏 match
    編譯紅);新破口類=掃描呼叫序列尾部追加=舊類 id/序零漂移;端到端
    配方(extension recipe)=RTL2SPEC-PLUGIN §17.2,實證=G8 dead_branch
    全鏈路(§17.3;26 測/parity/釘選集零觸發)。
18. **D18 版本雙軌**:插件 semver(SSOT=plugin.json 的 version;四點一致
    =manifest/CHANGELOG 頭/dist SERVER_VERSION/serverInfo 引用常量,
    run-plugin-check.sh ①執法)與內核 ABI 版本(rtl_spec_version()=
    Cargo workspace 共享,禁 bump 防基線擾動)分離;CHANGELOG=
    plugin/rtl2spec/CHANGELOG.md;git tag 慣例 rtl2spec-vX.Y.Z
    (文檔慣例,真打由 user 定)。
19. **D19 ABI 裝載校驗**:server.mjs 裝載 dist/wasm 後即核對
    rtl_spec_version()∈EXPECTED_ABI 名單,不匹配=fatal(雙版本+修復
    指引),語義入口前攔;檢查閘獨立重算(從 server.mjs 提取名單+實調
    比對,D8 同款)。

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
4. 總閘序列(R17-1 復審修訂分階段;2026-11-04 S9..S12 輪三規劃閘落地
   後**全數轉現役**):
   - **現役(全部實存,bash 可直跑,全 exit 0)**:`cargo test
     --workspace` ─ `bash scripts/regress.sh` ─ run-mcp-e2e.sh ─
     run-rtl-validation.sh ─ run-plugin-check.sh(D17..D19;版本
     四點一致+manifest 校驗+ABI 裝載校驗+拓展面冒煙,§17.6)─
     **run-golden-regression.sh(D14)** ─ **run-gap-fixtures.sh(D15)**
     ─ **run-realworld-eval.sh(D16;353 檔分層評測→docs/RTL2SPEC-EVAL.md)**。
5. `bash plugin/rtl2spec/run-golden-regression.sh`(D14;37 檔釘選集
   rules+gaps golden 逐位元組回歸)與 `bash
   plugin/rtl2spec/run-gap-fixtures.sh`(D15;G1..G7 正負樣矩陣)——
   **S9/S10 實作,2026-11-04 起以雙守衛降級形收編 regress.sh 既有
   rtl2spec 塊(S11;守衛不變,在場時 exit≠0 即整閘紅;基線斷言逐字
   不動)**。
6. `bash plugin/rtl2spec/run-realworld-eval.sh`(D16;353 檔分層評測,
   per-file 三檔分類判據→docs/RTL2SPEC-EVAL.md)——**S12 實作,
   2026-11-04 起總閘直調,不進 regress(353 檔規模時長;規劃理由在案)**。
7. `bash plugin/rtl2spec/run-plugin-check.sh`(D17..D19;實存,2026-11-04
   起)——bash 直跑:①版本四點一致 ②manifest 靜態校驗 ③ABI 裝載
   校驗 ④拓展面冒煙(G8 正負樣集合相等+demo golden byte-diff);
   與既有閘不重複(不跑 13 檔集/不跑 MCP 回環/不跑 regress)。

## 非目標

見 RTL2SPEC-PLUGIN §11(13 條:不裝載使用者環境/不動前弧 API 與語料/
GPL 不入集/不做 SVA/不做參數求值/不做跨檔層級/不做 TB 執行/禁手寫
golden/MCP 只 stdio+四工具/不採 --target bun/不做 GUI/不做 4-state
規則表達/不做層級推理)。

## 實作記錄

- 2026-11-03 S1:本 ADR 全文+PLAN P3.8 行+里程碑表 RTL 行+SKILL
  「rtl-to-spec 增補」節骨架+REGISTRY 預登記落盤;rg 交叉引用自驗
  (本檔/正典/PLAN/SKILL/REGISTRY 互相指到的路徑與閘名逐字)。
- 2026-11-03 §16 立項起草(規劃輪):RTL2SPEC-PLUGIN §16 章(341 行
  追加,既有 1063 行零刪改 sha256 斷言 012060d81b84)+本 ADR 增訂
  D14..D16+機械閘改七命令;釘選 37 檔規則數 Σ=630 全 emit 實跑;
  G4 flake(fiedler-cooley 3 跑 3 hash)與 scopes.v 掛死(4s 未終止)
  本弧複驗實錄。
- 2026-11-04 §17 拓展與版本管理(S13..S15)+開弧換人復審 R17-1..R17-9
  逐條消化:D17..D19 本輪增訂落盤(初稿宣稱未落地,R17-1 勘誤)+機械閘
  總閘序列改現役/規劃分階段(run-plugin-check 入現役;三規劃閘注記
  狀態)+record-golden 裝載面改 dist(D8 對齊,R17-2;重錄內容 byte
  不變實證)+probe-determinism 頭注對齊實現(R17-8)+§17.3/§17.9
  修訂(revalidation 3 行 diff 對帳表述/§16 欠賬全景注記)。
- 2026-11-04 §16 S9..S12 實作落地(欠賬清償;本條同輪):S9=golden 體系
  (run-golden-regression.sh+scripts/golden-check.mjs;37 檔×2 首錄,
  自檢兩階段+PROVENANCE 全量重寫形;紅路雙向實證=golden 篡一字節必紅
  [DIFF+PROV-DIFF]+提取器 message 改一字必紅[僅有的 2 個 G1 釘選檔
  DIFF→還原綠]);S10=fixture 矩陣(run-gap-fixtures.sh+
  scripts/gap-fixture-check.mjs+gap_missing_reset_no_arm.v;條級
  multiset 全集 14 條=類級 dedup 斷言之外的第三處低估勘正[G5×2/
  G6×3/G7×4],selfcheck+終止性守衛在閘);S11=regress.sh 塊內追加
  兩直調+立法四層同輪(SKILL golden 回歸與評測增補條/AGENTS 10.10
  回歸與真實評測執法面條/REGISTRY Crates 行 26 測+閘表三行/本節 4..6
  條翻現役);S12=評測擴面(run-realworld-eval.sh+eval-all.mjs+
  gen-eval-report.mjs;353 檔首錄 trap=0/指紋 672 unique/80ms;報告
  docs/RTL2SPEC-EVAL.md 509 行+R5 五檔探針×3 複評內聯;
  eval-all L2 清單初版只濾 .v 致 L2=0 假綠,被分層統計行當場抓出修為
  .sv——清單短收不攔的教訓=分層統計數字必人讀對帳)。D14/D15/D16
  狀態由「規劃」轉「已落地」。
- 2026-11-03 開弧對抗審 R16-1..R16-7(7 條逐條消化;規劃輪修訂):
  RTL2SPEC-PLUGIN §16 修訂(F1 擴 G1 兩處[670/402/439]+按腿症狀/
  矩陣 G5/G6/G7 共報勘誤+集合相等斷言形/終止性防線改 trap 計數/
  懸空「勘察事實 §n」引用清零改可重跑命令/自檢實現形釘明=進程內/
  fixture 兩處路徑/counter.v 全路徑)+§16.9 修訂記錄節+本 D14 修訂;
  12 檔 flapper 12/12 全量復驗(wasm 同進程 12 跑)+跨進程一致/
  同進程抖雙腿實證+scopes.v wasm trap 秒級實證。
- 2026-11-06 C 閉環弧規劃輪(指針;本 ADR 既有條文零改):新立 ADR-0013(D-C1..D-C12:TB 生成雙腿
  [核心 tb_plan 模板腿+LLM 定向腿]/TB 協議 v1/判別力雙 fixture[規則錨定變異+P-unruled 池+yosys
  僅錄製期存活裁決]/conformance 閘 run-conformance-eval.sh[規劃閘,總閘直調不進 regress]/診斷回灌
  ≤3 輪/採樣 6 設計/插件 0.4.0);本 ADR 機械閘第 4 條總閘序列由 ADR-0013「機械閘」延伸;復原力弧
  終審三缺陷 D-r1a/r1b/r1c 收平歸 ADR-0013 D-C12(C-S1);企劃=RTL2SPEC-PLUGIN §19。
- 2026-11-07 C 閉環弧復審修訂指針(本 ADR 既有條文零改;修訂關係=ADR-0013 D-C11/機械閘 1):①D4「四工具」
  與非目標「MCP 只 stdio+四工具」由 ADR-0013 D-C11 修訂為五工具(新增 rtl_tb_plan,C-S2 生效;stdio 單
  傳輸與零 HTTP/WS/sampling/resources/prompts 面不變);②機械閘 1 的「七斷言/恰 4 工具」自 C-S1(⑧)/
  C-S2(②⑨⑩)起以 ADR-0013 機械閘 2 與該片實跑 PASS 行數為準(現狀定義已三分:本檔 :106 七斷言/
  mcp-client.mjs 實有 10 個 check 呼叫點/OKR.md:62 十一斷言);③機械閘 4 的現役 8 條即 ADR-0013 機械閘 1
  的「ADR 現役交付閘 8」,run-reimpl-eval/s6 run.sh 屬環境依賴直調、非本 ADR 交付閘。
- 2026-11-07 C 閉環弧復審 R2 修訂指針(本 ADR 既有條文零改;修訂關係=ADR-0013 D-C13):D1 的弧約束「禁破壞
  analyzer-core 測試與 API」有一處由 oracle 證據覆蓋——crates/analyzer-core/tests/api.rs:128
  `sv_ansi_inheritance_leg` 斷言 `module m(input [3:0] a, b, c, output d)` 的 d 寬 4(:119-121 註釋稱依 LRM
  §12.3.3 繼承前組),而 iverilog 13.0(-g2005/-g2012)與 verilator 5.052 三方皆 d=1;C 弧 C-S2 首步修正
  vl-syntax ANSI 新組重置(D-c2)時同片把該期望改為 1 並改寫相關註釋,API 簽名零改;此為 D1 唯一例外
  (AGENTS §1 oracle 即 spec;證據=RTL2SPEC-PLUGIN §19.3.2 七向量表與 §19.17 RV2-1)。
- 2026-11-07 C 閉環弧 C-S1 實作記錄指針(本 ADR 既有條文零改;修訂關係=ADR-0013 D-C12/機械閘 2):機械閘 1
  run-mcp-e2e 的斷言數於 C-S1 加 ⑧(D-r1a max_rules 效果四調用)後實跑 PASS 行數=**14**(①②③③b④④b⑤⑥⑦
  ⑧a⑧b⑧c⑧d+ping;:106「七斷言」原文按 append-only 不改,現役定義以此為準,C-S2 改 ② 並加 ⑨⑩⑪ 後再同步);
  機械閘 4 的 run-plugin-check 新增判據 ⑤ SSOT 漂移(scripts/ssot-drift-check.mjs)、run-golden-regression
  尾部新增 STATS 資訊行(不斷言)、run-gap-fixtures 矩陣 8→13 行(+D-r1c 五檔)——均屬既有閘的判據增補,
  ADR 現役交付閘數仍 8;詳證=RTL2SPEC-PLUGIN §19 C-S1 實作記錄。
- 2026-11-07 C 閉環弧 C-S2 實作記錄指針(本 ADR 既有條文零改;修訂關係=ADR-0013 D-C11/D-C13):D4「四工具」→五工具
  生效(rtl_tb_plan 落地;stdio 單傳輸不變);機械閘 1 斷言數=**19**(實跑 PASS 行數,iverilog 在場時;①②③③b④④b⑤⑥⑦
  ⑧a..d⑨a..c⑩⑪+ping,不在場時 ⑨c=SKIP 行);D1 例外生效(api.rs:128 期望 4→1,oracle 三方 d=1,API 簽名零改);
  wasm 導出 7→9;插件 semver 0.3.0→0.4.0(四點一致,EXPECTED_ABI 仍 ['0.1.0']);run-plugin-check 新增 ③c 導出面+⑥ skill
  零語義判據,均屬既有閘的判據增補,ADR 現役交付閘數仍 8;詳證=RTL2SPEC-PLUGIN §19.19。
- 2026-11-07 收尾弧 S3 釘選集擴檔指針(本 ADR 既有條文零改):D14 golden 釘選集 37→49 檔(+12 個 conformance staging ADMITTED
  設計;golden 74→98 份;`run-golden-regression.sh --record` 實跑既有 74 份 sha 逐值不變);入選依據由「≥3 條規則」改為「TB 實殺變異的
  直接證據」;golden-check 新增 EXPECTED_PINNED=49 精確數斷言;§16.1.1 表欠 S4 §20 清償(RTL2SPEC-FINISH-PLAN §11)。現役交付閘數不變。
- 2026-11-07 收尾弧 S4 指針(本 ADR 既有條文零改):D14 golden 釘選集的 SSOT 由 RTL2SPEC-PLUGIN §16.1.1 表移至 §20.5 全表(49 檔),
  run-plugin-check ⑤e 逐檔名對拍、⑤f 收尾弧欠條已清;插件 semver 0.4.0→0.5.0(四點一致,MCP 工具面與 EXPECTED_ABI 不變)。現役交付閘數不變。
