# CHANGELOG — rtl2spec plugin

版本 SSOT=`.zcode-plugin/plugin.json` 的 `version`(semver);本檔頭部版本
必與之一致(run-plugin-check.sh 執法)。格式:semver 三段
[major.minor.patch]——major=工具面/ABI 破壞性變更;minor=新增能力(規則
類別/破口類/工具);patch=缺陷修復與文檔。git tag 慣例=`rtl2spec-v<X.Y.Z>`
(打 tag 由 user 定,弧內禁 commit 見 AGENTS)。

## [0.6.0] - 2026-11-08(未發布;弧內禁 commit)

### Added(D spec 前端弧 D-S0..D-S3;RTL2SPEC-D-PLAN/ADR-0014 草案)

- MCP 新工具 `rtl_spec_lint`(本版):高級 spec markdown 七查 lint——收
  {path|source, name},回 {findings, count}(findings=人讀診斷行,①..⑦
  前綴標查號;空陣列=clean)。面零語義(裝載 wasm→編幀),語義全在核心
  `crates/rtl-spec::spec`(10.7 三面一路)。
- wasm 導出 `rtl_spec_spec_parse`/`rtl_spec_spec_lint`(D-S3):字串進出零
  js_sys;lint 輸出與 CLI `rtl-spec spec lint` stdout byte-equal
  (run-spec-cli-parity spec 腿判據)。
- SERVER_VERSION 0.5.0→0.6.0(plugin.json/CHANGELOG 同輪一致,semver 同步律);
  run-mcp-e2e 斷言 19→21(tools/list 5→6 工具+rtl_spec_lint 正負兩斷言)。
- 核心新增(D-S1,隨本版 wasm 載出):spec 檔格式(RTL2SPEC-D-PLAN §3,R1-4
  行級文法+check 行四收形)、parse_spec_md/spec_lint 七查/spec_json;
  fixtures/d-spec/ 三正樣語料(counter8 2 條/acc_pipe 9 條/i2c_master 94 條)。
- CLI(rtl-spec bin,D-S2):agent 唯一接口,analyze/extract/gaps/tb+spec
  parse/lint 子命令;JSON-first+exit code 四值(0/1/2/3)+--help 工作流配方
  (CLI-AGENT 憲法,ADR-0014 草案)。

## [0.5.0] - 2026-11-07(未發布;弧內禁 commit,版本號由收尾弧 S4 生效)

### Added(C 閉環收尾弧 S0..S4;RTL2SPEC-PLUGIN §20/RTL2SPEC-FINISH-PLAN)

- 等價裁決腿(S1;CONFORMANCE-PROTOCOL §7;scripts/conformance/equiv.mjs):`--record` 期以 yosys 強形命令對全腿存活的 P-rule
  變異給 eqverdict(equivalent|unproven|adjudication-failed),equivalent 須 proven 且 cells≥輸出端口位數;偵測力斷言六條
  (gold-vs-gold/正樣/反樣/空真探針/killed 全母體/存量重驗)任一不過=拒錄;默認模式零 yosys 依賴,只驗 eqMeta schema 與計數。
  record.json 新欄:頂層 eqMeta、waiveTable;變異級 eqverdict/eqCells/eqReason/eqDetail/eqUnproven/unprovenCells。新行 SUMMARY-EQ。
- 波形幀診斷腿(S2;§8;run-frame-diag.sh+scripts/frame-diag.mjs+conformance/frames.mjs):counterexample VCD→vl-vcd 統一窗 SVG→
  sips/qlmanage PNG→剝 title 畫布+PNG 雙檢定 modality→視覺診斷或機械 text-fallback;`--validate` 只驗工件。輔助證據,不進任何閘。
- T 腿擴面(S3;§9):`--fixtures-root` staging 預錄、`--slice C-Sn-批次`;conformance 設計 6→34(EXPECTED_DESIGNS 34),主根斷言
  規則書必為 golden 釘選快照;golden 釘選 37→49(98 份,golden-check `EXPECTED_PINNED=49`)。
- run-plugin-check ⑤e/⑤f(S4):RTL2SPEC-PLUGIN §20.5 釘選全表==pinned-files.list 逐檔名;收尾弧欠條已清。

### Changed

- WAIVE 標籤集補 `unadjudicated`(模板 v1→v2 協議事件,§6;v2 sha256 104d13f1…);tblint 對文法外/集外 WAIVE 行 Q9 硬紅;
  WAIVE↔record 交叉驗證併入 Q9。i2c 11 條+sincos d.R035 由 unobservable 按數據改標 unadjudicated(`--promote` 換檔)。
- conformance 默認模式新增:TB-SHA-ALARM(入庫 tb.v 禁手改,§5)、wasm sha 保持斷言(入庫 record 的 provenance.wasmSha256 必==
  dist wasm,否則 DRIFT)。

### Fixed

- 終審 medium-1(WAIVE「不可觀測」措辭無證據)、medium-2(入庫 tb.v 手改假綠),見 §20.2。
- S3 主根釘選斷言初版未入 admitted 判定(空操作),負向探針抓出後修(REGISTRY Incident 11-07)。

### 未變

- MCP 工具面(5 工具)、wasm(sha256 66518257…)、EXPECTED_ABI。

## [0.4.0] - 2026-11-07(未發布;弧內禁 commit,版本號已定 C-S2 生效)

### Added(C 閉環弧 C-S1..C-S2;RTL2SPEC-PLUGIN §19/ADR-0013)

- 核心 tb_plan(crates/rtl-spec/src/tbplan.rs;D-C2/D-C3):規則書→TbPlan(module/params/ports/clocks
  [select_trigger 單一語義源]/resets[只由 guard 導出]/sens_edges/data_inputs/outputs/obligations[port|indirect]/
  waivers[x-valued]/scenes)+tb_template_v(S-RST/S-ARST/S-FULL/S-SMALL/S-HOLD/S-EXH/S-flush 場景模板,
  xorshift32 禁 $random,TB 協議 v1 stdout 文法);計劃期硬錯=TbError `error: … (milestone TB1)` 十一條。
- wasm 導出 rtl_tb_plan_sv+rtl_tb_plan_rtlil(7→9;後者恆 TB1 錯誤幀;0=核心默認/空串=未指定模組,錯誤走
  資料不 trap)+MCP 工具 `rtl_tb_plan`(4→5;lang 分派沿 langOf)+native emit `tbplan` 模式;smoke 三路
  wasm↔native parity(bun+node)。
- skill `rtl-conformance`(3→4):tb_plan 調用編排與欄位解讀,零語義(run-plugin-check ⑥ 執法 rtl_ 標識符
  ⊆工具名)。
- run-plugin-check 新判據:③c 導出面檢查(server.mjs api.* ⊆ dist wasm 導出,閘驗出貨物 D8 同形)+⑥ skill
  零語義驗法;③(ABI 名單)不變。
- conformance 確定性床(C-S3;§19.4/§19.5):run-conformance-eval.sh(總閘直調不進 regress)+scripts/lib/tbproto.mjs
  (分腿丟行正規化/協議解析/帶看門狗執行器;reimpl-eval 遷用並證 parity)+conformance/tblint.mjs(Q6/Q9、TB-R1x 跨檔
  極性感知、同檔 R1)+conformance/mutate.mjs(規則錨定算子目錄 opcat-v1、P-unruled 池、OP-NOP 控制組)+
  conformance-eval.mjs(Q1..Q9、selfcheck、`--record --slice` 錄前快照+record-diff、yosys 僅錄製期存活裁決);
  六設計 T 腿 fixtures/conformance/<design>/{case.json,tb.v,expected.txt,record.json}+證據頁 conformance-report.md
  (度量行為復原力,非意圖正確性)。

### Fixed(復原力弧終審三缺陷;§19 C-S1)

- D-r1a:max_rules 上調未達 MCP/batch 面——server.mjs 默認/batch 改傳 0(wasm 契約 0=核心
  DEFAULT_MAX_RULES,面委託核心 SSOT)、gap-fixture-check/SKILL 文案/emit.rs 對齊;run-plugin-check ⑤
  SSOT 漂移判據(a 字面量∈{0,N}/b 文案數字/c 回退 token 錨/d emit.rs 常量面)+mcp-e2e ⑧ 四調用。
- D-r1b:§18.2 對賬數字口徑勘誤表(PLUGIN §19.2;§18.2 原文歷史保留)+golden-stats.mjs 資訊行
  (run-golden-regression 尾部,文檔對賬一律引 STATS 行原文)。
- D-r1c:復位前置敏感列表 async 分類失明——select_trigger(規則面 sv/mod.rs 與破口面 sv/gaps.rs 單一
  語義源,首個 ≠復位候選邊沿、回退首沿);5 檔 rst_first_* fixtures 兩矩陣 golden 重錄(mem_arst
  sequence→guard×2 async=true;G4-MyMem-001 假陽性消失)。
- golden-leafdiff.mjs(重錄片葉級對賬工具,G3 --allow 越界斷言)。
- D-c2(§19 C-S2 步 a):vl-syntax ANSI 端口組解析新組重置 net/reg/signed/range、僅方向跨組繼承
  (IEEE 1800-2017 §23.2.2.3 同向;oracle=iverilog 13.0 -g2005/-g2012+verilator 5.052 七向量三方一致)——
  contract 端口寬度/符號/參數化標記訂正(golden 重錄 4 檔 24 葉:loops/memory/mem_arst/partsel,partsel 16 葉
  為 signed);analyzer-core api.rs:128 期望由 oracle 證據訂正(ADR-0013 D-C13 顯式修訂 ADR-0012 D1)。

## [0.3.0] - 2026-11-05

### Added(復原力 MVP S0..S5;RTL2SPEC-MVP-NEXT R2)

- 接口契約章(S1/N2):規則書新增 contract 節(端口/參數/內部聲明清單,結構投影直讀
  +localparam AST 補收)+信號-聲明完備性 lint(N5,decl_lint 隨 rules.json 每輪重算);
  i2c 盲寫 compiled=false(缺 reg [3:0] state)根因關閉。
- 組合邏輯覆蓋放寬(S2/N3):relation 產出分界=結構形(PartSelect/Concat/Repeat)一律
  +純常量限分支域內;無條件直賦/直通別名負 fixture 釘死(Daikon 教訓注釋)。
- 復位語義承載(S3/N4):guard 載荷加 async 欄位(全邊收集分類)+敏感列表入第二證;
  timing 指紋 when 維並入 conds(N1,i2c HOLD/IDLE 條件賦值去重復活);M3 else 臂
  !(復位條件) 包膜。DEFAULT_MAX_RULES 128→256(i2c 152 條觸頂)。
- 分支條件家族(S4/M1-M2)+破口三修(M4-M6):多標簽臂逐標簽展開/default 臂否定合取/
  G8 展開 DelayCtrl-Wait(-EventCtrl)/missing_reset 無條件純常量約束/incomplete_case
  n_labels≥total 守衛。
- 復原對拍評測(S5):run-reimpl-eval.sh(iverilog 守衛 exit 2;23 場景 6+6+11;A/B 兩組
  分列禁混報;消融點 S1/S2/S3 分段;直調不進 regress)+REIMPL-PROTOCOL.md 協議釘檔
  (GLM-5.3-Flash max n=3)+fixtures/e1 裝備遷入。
- 插件 semver 0.2.0→0.3.0(minor=contract 新能力;內核 ABI 軌 workspace 0.1.0 獨立
  不動——vl-cli version golden 釘,§18 雙軌注記)。

## [0.2.0] — 2026-11-04

### Added
- **破口類 G8 `dead_branch`**(§17 拓展性實證 S13):常量條件死分支檢測
  ——If 條件為純字面量且值可判定(x/z 位不報=X-safe;param 不求值=§11.5
  契約),恆假→then 臂死/恆真→else 臂死;confidence=certain。實現=
  crates/rtl-spec(schema.rs GapClass::DeadBranch+gaps.rs
  scan_dead_branches,掃描序尾部追加=舊類 id/序零漂移);單測 2 個
  (正樣 2 條 certain+零共報/負樣三形零命中=變量+param+x)。
- **fixtures**:plugin/rtl2spec/fixtures/gap_dead_branch.v(正樣零共報)
  +gap_clean_branches.v(負樣三形零命中)。
- **golden 演示樣例**(S9 迷你版;錄製=record-golden.mjs 真實運行 wasm
  腿,12 跑自檢前置):golden/plugin-fixtures/gap_dead_branch.gaps.json
  +gap_clean_branches.gaps.json+PROVENANCE.md(機械生成禁手改)。
- **拓展配方正典**:docs/RTL2SPEC-PLUGIN.md §17(extension recipe/
  registry 等價機制/版本雙軌)。
- **run-plugin-check.sh**:版本四點一致+manifest 校驗+ABI 裝載校驗+
  拓展面冒煙(含 demo golden byte-diff);bash 可直跑。
- **ABI 裝載校驗**(§17.5):server.mjs 裝載後核對 rtl_spec_version()∈
  EXPECTED_ABI 名單,不匹配=fatal 含雙版本+修復指引。
- **golden 回歸體系**(§16 S9):37 檔釘選集×rules+gaps=74 份 golden
  (golden/ 鏡像源路徑,錄製=run-golden-regression.sh --record 真實運行,
  錄前進程內 3 跑自檢)+PROVENANCE.md(機械全量重寫);閘 bash 直跑,
  regress.sh S11 起收編(雙守衛降級形)。
- **破口 fixture 矩陣閘**(§16 S10):run-gap-fixtures.sh+scripts/
  gap-fixture-check.mjs(G1..G7 條級 multiset 集合相等+selfcheck+
  終止性守衛)+fixtures/gap_missing_reset_no_arm.v(G4 無復位臂正樣,
  suggested_test 確定名)。
- **真實評測擴面**(§16 S12):run-realworld-eval.sh+scripts/eval-all.mjs
  +gen-eval-report.mjs;353 檔分層評測(L1=93/L2=260)→docs/
  RTL2SPEC-EVAL.md(機械生成,含 R5 五檔複評內聯);總閘直調不進 regress。
- **確定性基礎設施**(§16 S8):probe-determinism.mjs(37 檔×12 跑+調用序
  擾動)+pinned-files.list 釘選清單。
### Fixed
- G4 無復位臂 suggested_test 非確定取樣(HashSet 迭代序→字典序首名,
  §16.0 F1a)+G1 報告序非確定(兩處臂迭代有序化,F1b)+G5 scopes.v 形
  掛死(bfs_path visited 集,真根因=死循環非規模,F2/REGISTRY 11-03 行)
  ——§16 S8 軌。

### Breaking
- (無;G8 為尾部追加,既有 G1..G7 id/序零漂移,釘選集 37 檔 gaps 輸出
  byte 不變實證)

## [0.1.0] — 2026-11-03

### Added
- 初版(ADR-0012 S1..S7+收平輪):rtl-spec 核心(規則書九類+破口 G1..G7)
  +rtl-spec-wasm(7 導出,零 js_sys)+plugin 包(manifest+3 skills+MCP
  stdio 四工具)+兩閘(run-mcp-e2e.sh/run-rtl-validation.sh)+validation
  report 1745 行。
