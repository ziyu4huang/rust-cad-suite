# ADR-0014: CLI-AGENT 憲法(spec 前端與 agent 唯一接口)

日期:2026-11-08。狀態:**接受(隨 D-S6 收尾落地;條文正典=AGENTS 10.13+SKILL
「CLI-AGENT 憲法增補」節;實作記錄=RTL2SPEC-PLUGIN §21)**。前置=ADR-0012
(rtl-to-spec 插件弧)、ADR-0013(C 閉環)、docs/RTL2SPEC-D-PLAN.md(D 弧規劃,
§2 憲法四條/§3 spec 格式契約/§5 兩新閘)。切片編號 D-S0..D-S6。

脈絡:R5「D spec 前端(人寫高級 spec→agent 產 RTL)」的兩道時點判據均已過
(D-PLAN §0:①S5 之後=2026-11-05 復原力弧;②C 全量化之後=2026-11-07 C-S6),
且 R5 主理由「E1 表述質量未達標」已由 contract 節+decl_lint(S1..S3)除名。
用戶 2026-11-08 定調 CLI-AGENT 憲法四條,本 ADR 立其決策形 **D1..D4**(引用
須全限定「ADR-0014 Dn」)。

## 決策

1. **D1 agent-first CLI(憲法①)**:`rtl-spec` bin=agent 唯一接口
   (crates/rtl-spec/src/bin/rtl-spec.rs 薄 main+src/cli.rs host-only)。
   JSON-first:資料子命令 stdout=raw JSON(**無尾隨換行**,與 wasm 導出
   byte-equal——run-spec-cli-parity.sh 判據;CLI 渲染面任何改動必紅本閘,
   D-S2 負探針實錄);人讀診斷一律 stderr 且**自帶下一步可執行提示**。exit
   code 四值:0=成功;1=域內失敗(findings/被測紅/驗證不過);2=用法或環境缺
   (含守衛缺場);3=輸入檔不存在。rationale:CLI 的消費者是 agent 不是人;
   機器可讀單面+穩定 exit 契約讓 agent 無需讀文檔即可接錯。
2. **D2 self-contain(憲法②)**:`--help` 頂部攜完整工作流配方(spec 撰寫→
   spec parse/lint→盲寫 RTL→extract 對拍→tb 生成→conformance 評測,逐步全部
   以 rtl-spec 子命令寫出);每子命令 `--help` 自解釋(Usage/旗標/Exit codes/
   Example)。操作知識內嵌於 CLI 自身,用 CLI 不需要再讀任何文檔。
3. **D3 委派形裁決(憲法③)**:閘類/評測類能力=CLI 子命令 **spawn 既有
   runner**(exit code 原樣轉發+JSON 封套),**禁 Rust 側重實現 JS 判據**
   (Q1..Q9/V1..V6 的 JS 實現=鎖③獨立重算面,雙實現=漂移)。本弧實裝形:
   資料子命令(analyze/extract/gaps/tb/spec parse/spec lint)直連核心(能力
   先落 rtl-spec 核心,10.7);評測床保持 bash 直調形(run-spec-eval.sh 直調
   不進 regress 的例外收編=D-S6 regress S7 塊,iverilog 缺場 exit 2 SKIP)。
   後續若加 conformance/diagnose 子命令,一律委派形,禁攜判據。
4. **D4 core-first 三面跟隨(憲法④)**:新能力先落 crates/rtl-spec 核心
   (語義姿態=永不 panic、錯誤進結果),CLI/wasm/MCP 三面同弧跟隨;wasm 面
   字串進出零 js_sys;**單一面實現=違例**。spec_lint_json=CLI/wasm/MCP 三面
   byte-equal 的單一源(禁各面自拼)。
5. **spec 格式契約(SSOT)**:格式=markdown 節頭逐字(module/param/port/
   state/derived/assumes+## behavior+## scenarios;行級文法 R1-4 五條;check
   行四收形 guard/transition/sequence/relation——**sequence 無 anchor 欄**,
   md.rs 發射面/trim writer 與 spec 文法不可互推、變動不溯及);機械檢查=
   spec_lint 七查(⑥ decl_lint 只掃值側,值側標識符 ⊆ ports∪params∪states
   ∪derived)。SSOT 文本=RTL2SPEC-D-PLAN §3;**改 spec 文法=本 ADR 修訂事件**
   (先改本檔+D-PLAN §3 再改解析器與 fixtures,diff 對賬禁悄悄改語義)。
   正樣語料=plugin/rtl2spec/fixtures/d-spec/ 三檔(counter8 2 條/acc_pipe 9 條/
   i2c_master 94 條,自著 MIT)。
6. **凍結與採樣律**:fixtures/d-spec/reimpl/<design>/top.v 入庫=凍結工件禁手改
   (sha256 釘 REIMPL-PROTOCOL §5;換檔=協議事件);legs.json=斷言規格
   (expected 由首跑錄製禁手寫;mutprobe measured=實測數字,漂移即紅);盲寫
   採樣身份/參數/prompt 逐字 append-only(REIMPL-PROTOCOL §5,**盲隔離揭露**
   =D-S4 曾讀 DUT 原文,數字僅輔助證據);LLM 腿永不進確定性閘(10.11/10.12
   沿用)。
7. **wasm sha 變更=conformance 全量重錄語義事件**(conformance-eval DRIFT
   鏈):重錄對賬判據=34 record.json 剝 tb.simMs/provenance.date 後唯一允許
   diff=provenance.wasmSha256(D-S3 實錄零語義漂移)。
8. **四層對照**:①機械閘=run-spec-cli-parity.sh+run-spec-eval.sh(regress S7
   收編)②正典=SKILL「CLI-AGENT 憲法增補」節③本檔+AGENTS 10.13④證據頁=
   plugin/rtl2spec/spec-eval-report.md(機械生成禁手改;默認模式 byte 比對
   保鮮,陳舊/手改=exit 1)。

## 修訂

- 2026-11-08 初版(D-S6)。ADR-0012 修訂注記:R5「D spec 前端」解封=R5 時點
  判據兩道已過(D-PLAN §0 實證);其立檔「單出 plugin 寫面」約束由 D-PLAN §0
  3+10.12 同款約束延用(build/=gitignored 對賬工件;fixtures/d-spec 子樹內容
  類型放寬=顯式例外)。

## 開放項(非阻塞)

- `invariant` check 形未立文法(parse 遇之=finding ⑤,待有語料再立,立時=
  本 ADR 修訂事件)。
- 「## behavior ≥1 條」在七查枚舉(①..⑦)之外,僅文法註記——零條目 spec
  可過 lint;是否收查=待 ADR 修訂裁決(D-S6 時點未收)。
