# CHANGELOG — rtl2spec plugin

版本 SSOT=`.zcode-plugin/plugin.json` 的 `version`(semver);本檔頭部版本
必與之一致(run-plugin-check.sh 執法)。格式:semver 三段
[major.minor.patch]——major=工具面/ABI 破壞性變更;minor=新增能力(規則
類別/破口類/工具);patch=缺陷修復與文檔。git tag 慣例=`rtl2spec-v<X.Y.Z>`
(打 tag 由 user 定,弧內禁 commit 見 AGENTS)。

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
