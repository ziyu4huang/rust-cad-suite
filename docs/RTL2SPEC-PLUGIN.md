# RTL2SPEC-PLUGIN.md — rtl-to-spec 插件弧方案正典(2026-11-03 起草,ADR-0012)

狀態:Accepted(方案正典;实作与审查以本档为准)。前序:ADR-0011(分析器服務)、
ADR-0007(spec 驗證層)、ADR-0005(GUI 憲法)。SSOT 鏈:本檔(方案)→
plugin/rtl2spec/(成品)→ validation-report.md(證據)。

弧目標(使用者原話轉寫):
1. 讓 agent 有工具分析 RTL 程式碼、做完整測試與找破口;
2. 開發 rtl-to-spec plugin:從 RTL 抽取「有意義的規則書」——不是把 RTL 機械
   轉成文件,而是**可檢驗的行為規則集**;
3. 插件純 Bun 可跑、跨平台:Rust 經 wasm 編譯,bun 或 node 都能執行;
4. 形態=完整一路:Rust 核心 → wasm → MCP server(+相關 skills)→ zcode
   plugin 包;
5. 最終用真實 RTL 實測:抽出的 spec 經得起「有意義規則書」評審(機械判據+
   獨立評審員),不是 doc-dump。

---

## 0. 勘察事實基座(本檔全部設計的地基;引用格式=勘察事實 §n 或 repo 路徑:行)

- **analyzer-core 逃逸口**:Analysis 持 `sv: Option<vl_syntax::ast::SourceFile>`
  與 `rtlil: Option<ry_ir::Design>`(crates/analyzer-core/src/lib.rs:53-55),
  僅 Rust API 可及——本弧深提取的合法後門。入口 `analyze(&SourceInput)->Analysis`
  永不 panic、錯誤進 diagnostics(lib.rs:66-73)。
- **行列查詢**:`Analysis::line_col(FileId, pos)->(line,col)`,SV 側經既有 pub
  `SourceMap::line_col`;**RTLIL 側恒 Err**(無 SourceMap,lib.rs:127-138)。
- **RTLIL 無 span**:ModuleInfo.span 恆 None(crates/analyzer-core/src/view.rs:26-27)。
- **RTLIL 行程形態富**:Process{root_case: CaseRule, syncs: Vec<SyncRule>},
  SyncType{Low/High/Posedge/Negedge/Edge/Always/Global/Init}+actions+
  mem_writes(crates/ry-ir/src/ast.rs:289-332)——破口分析與 FSM 提取的
  最佳結構源。Cell.connections=Vec<(Id,SigSpec)>(ast.rs:274-281)。
- **SV AST 形態**:Module.items 含 `Always{span,ctrl:Option<EventCtrl>,body}`、
  `Initial{span,body}`、`GenFor`(crates/vl-syntax/src/ast.rs:135-146);
  Stmt 族=Block/BAssign/NbAssign/If/Case/For/While/Fork/Force…(ast.rs:271-374);
  EventCtrl 含 Posedge/Negedge(ast.rs:430-431);Expr=Ident/HierIdent/NumLit/
  Unary/Binary/Ternary/Concat/PartSelect…(ast.rs:445-488+)。
- **analyzer-core 自我禁區**:其 crate 文檔明言「查詢集=既有 IR 欄位投影,
  求值/解析=禁區」(crates/analyzer-core/src/lib.rs:2-7)——規則歸納是新語義
  層,不得塞入該 crate(決策 D1 的直接依據)。
- **wasm 形態先例**:vl-wasm crate-type=cdylib+rlib、`#[wasm_bindgen]` 導出、
  頭注構建命令 `wasm-pack build crates/vl-wasm --target bun`(或 web)
  (crates/vl-wasm/src/lib.rs:1-12);pp→parse 管線 wasm 兼容已被證
  (lib.rs:14-51)。**regress wasm 閘用 `--target web`+bun 跑**(scripts/
  regress.sh:195-211,陳舊檢查=find -newer 重構建),ce29bfc 提交記錄實測
  全 PASS——bun 下 `await init()` 可用是既成事實。
- **TS 客戶端先例**:clients/analyzer-ts 純 Bun 零 npm 依賴(node:net UDS+
  手寫幀編解碼,src/client.ts:1-10)——「零依賴 TS」在庫內有正面先例。
- **MCP 最小面**:JSON-RPC 2.0 over stdio;initialize(SUPPORTED=
  ["2025-11-25","2025-06-18","2025-03-26","2024-11-05","2024-10-07"],請求版
  在列表則回顯否則用 LATEST)、tools/list(Tool.inputSchema=JSON Schema
  2020-12)、tools/call(result.content+structuredContent+isError;**工具內
  錯誤走 isError:true 的 result,協議級錯誤才用 JSON-RPC error**)——
  勘察事實 §6(excalidraw 0.3.4 官方 SDK 編譯產物實讀)。
- **plugin manifest 最小面**:`.zcode-plugin/plugin.json`
  {name,version,description,author,license,skills?,mcpServers?};stdio 形
  command/args/cwd/env 四欄位+`${ZCODE_PLUGIN_ROOT}`/`${ZCODE_PROJECT_DIR}`
  變量替換(excalidraw 0.3.4 實測;勘察事實 §5)。
- **環境實測(2026-11-03 本弧實跑)**:`rustup target list --installed`→
  aarch64-apple-darwin + wasm32-unknown-unknown + wasm32-wasip1;`bun
  --version`→1.4.2;`wasm-pack --version`→0.15.0;`node --version`→v26.12.1;
  `ls plugin`→不存在(全新目錄)。
- **命名避讓**:`vl-spec`(spec 驗證核心,ADR-0007)與 `spec-i2c` 已占
  (docs/adr/0007-spec-verification-layer.md:21-27);本弧=「提取」側,
  命名須區分(決策 D10)。
- **sv-elab/zipcpu 本機不存在**(勘察事實 §4 明確聲明):GPL 語料不入
  驗證集,sv-elab 樹未落地不入集;入庫驗證產物只派生自 in-git 語料。

---

## 1. 架構總覽(完整一路)

```
[Sv|Rtlil 源文本]
      │  analyze(&SourceInput)                    (analyzer-core,零改動)
      ▼
crates/rtl-spec           ← Rust 核心:規則歸納 + 破口掃描 + 結構投影
      │  extract_rules / scan_gaps / analyze_summary   (新 crate,
      │                                                依 analyzer-core 而不擴它;D1)
      ▼  Rulebook / GapReport(Rust 結構)
crates/rtl-spec-wasm      ← wasm 面:JSON 字串跨界(新 crate,沿 vl-wasm
      │  #[wasm_bindgen] 導出                形態;D3;--target web)
      ▼  dist/wasm/{rtl_spec_wasm.js,.bg.wasm}
plugin/rtl2spec/dist/server.mjs   ← MCP server:TS 零 npm 依賴,
      │  stdio JSON-RPC 2.0 四工具            newline 分幀(D4)
      ▼
plugin/rtl2spec/          ← zcode plugin 包:manifest+skills×3+
      │                       閘腳本×2(裝載驗證只在 repo 內;D5)
      ▼
總閘四件套:cargo test --workspace ─ bash scripts/regress.sh ─
           bash plugin/rtl2spec/run-mcp-e2e.sh ─
           bash plugin/rtl2spec/run-rtl-validation.sh
```

一路同構:核心唯一語義源(wasm 只是 face,§10.7 三面紀律);閘驗的
**就是出貨物**(check-rulebook.mjs 裝載 dist 同一 wasm 產物,D8)。

---

## 2. 「規則書」形態(目標①②的資料層)

### 2.1 Rule schema(v1,JSON)

```jsonc
{
  "id": "i2c_master.R001",             // <module>.R<序號>,模組內唯一
  "category": "reset",                  // §2.2 分類學詞表
  "module": "i2c_master",
  "statement": "當 !rst_n 且 clk 上升沿 時,state 裝載 ST_IDLE",
  "qualifier": "derived",               // derived=結構直讀可證;inferred=模式推斷
  "evidence": [                         // 溯源;≥1 條,行為類必居其一(§2.3)
    {"file": "corpus/e2e/59_i2c_master/i2c_master.v", "line": 123, "end_line": 126,
     "class": "process",                // process|assign|case|generate|expr|decl
     "snippet": "if (!rst_n) state <= ST_IDLE;"}   // 與源行逐位元組對拍(V4)
  ],
  "check": {                            // 可檢驗斷言,form 五選一(§2.4)
    "form": "guard",
    "trigger": {"edge": "posedge", "signal": "clk"},
    "when": "!rst_n",
    "effects": [{"lhs": "state", "value": "ST_IDLE"}]
  },
  "fingerprint": "corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|state|!rst_n|state<=ST_IDLE",
                                        // 拼接鍵串(source|category|module|
                                        // subject|when|effects 渲染),零哈希
                                        // 依賴(R1-8/D13);全集零重複(V5-③);
                                        // 示例值=驗證閘實錄(i2c_master.R001,
                                        // build/verdicts 實讀,R3-④)
}
```

欄位契約:
- `id`:模組內單調序;跨模組靠 `module` 區分。
- `fingerprint.source 維(R3-④釘死)`:=傳入 `extract_rules` 的 `name`
  參數**逐字**——驗證閘(check-rulebook.mjs)傳 repo 相對全路徑原樣
  (上示 `corpus/e2e/59_i2c_master/i2c_master.v`,verdict 實證);MCP
  `path` 呼叫=path 字串原樣;inline `source` 呼叫=`name` 參數或缺省
  `input.v`。**禁按 basename 慣例取檔名**:repo 兩個 `i2c_master.v`
  (`corpus/e2e/59_i2c_master/` 與 `crates/spec-i2c/rtl/`)basename 同名
  實證——basename 慣例令 R1-2 要防的「跨檔同名模組誤殺」靜默回歸。
- `statement`:**中文**模板句,標識符保持源碼原名(決策 D7);模板由提取器
  按 category+check 欄位機械生成,禁止自由散文。
- `qualifier`:`derived`(語法結構直接證明,如復位分支的賦值對)/
  `inferred`(模式匹配推斷,如握手模式);評審員按 qualifier 差異化抽查。
- `evidence.class`:`process`(always/initial 塊內)/`assign`(連續賦值)/
  `case`(case 臂)/`generate`(生成塊實例化分支)/`expr`(運算式片段)/
  `decl`(宣告)——**行為類=前五種;decl 只允許作輔助附加,不許單獨成證**
  (§2.3 反 doc-dump 第一鎖)。
- SV 側 evidence 帶 file:line(經 `Analysis::line_col`);RTLIL 側帶
  結構路徑 file 欄=`<name>.il`、line=0、加 `path` 欄
  `"module/process[2]/sync.posedge"`(決策 D2:ry-ir 無 span,
  view.rs:26-27 + lib.rs:127-138 RTLIL=Err)。
- **snippet 拼接規則(R1-10)**:多行 evidence(line..=end_line)的
  snippet=各**行原文**按行序以 `"\n"` 連接(保留縮進,末行不帶終結
  換行);單行=該行原文去行尾換行。生成端與 checker 同一定義,V4
  對拍按此拼接逐位元組比較。

### 2.2 類別分類學 v1(9 類;詞表即機械閘 V2/V3 的域)

| category | 提取源(SV 側) | 陳述模板示例 | check.form |
|---|---|---|---|
| `reset` | 邊沿行程(Always+ctrl 含復位訊號分支)內復位臂賦值對 | 「當 rst_n 為低且 clk 上升沿,{regs} 裝載 {常量}」 | guard |
| `fsm` | 邊沿行程內 case(state_reg) 臂(選擇子=寄存器) | 「state=S_IDLE 且 start 為高時,下一拍 state=S_START」 | transition |
| `handshake` | 組合行程輸出=條件表達式 且該輸出被邊沿行程讀取(模式推斷) | 「ready_o 僅當 {cond} 時為高」 | guard/relation |
| `timing` | 邊沿行程內非復位賦值對(寄存器語義) | 「{sig} 在 {cond} 成立後的下一拍變為 {v}」 | sequence |
| `arithmetic` | 組合塊/連續賦值 LHS=RHS 含**任意 Binary**(算術+位運算/邏輯,R1-3 放寬——carryadd.v 全檔僅 & \| ^ 位運算,嚴格算術讀法 0 行為規則,V1 必敗) | 「C = IN1 & IN2」「sum = a + b + cin」 | relation |
| `counter` | 邊沿行程內 `r <= r ± 1` 或 `r == MAX ? 0 : r+1` 形 | 「count 計到 MAX 後回 0(環繞)」 | sequence/invariant |
| `memory` | DeclEntry.unpacked>0 或 RTLIL memory + 寫/讀行程 | 「mem 在 {cond} 時寫入位址 {addr}」 | guard |
| `protocol` | 跨訊號序列可證序(fsm 臂 × 計數器/輸出驅動交織;best-effort) | 「scl 上升僅發生在 state==S_X 期間」 | sequence |
| `param` | GenFor/GenIf **行為構造**(實例化分支;不求值,勘察事實 §1);宣告內參數引用不產規則(R1-12 裁決,見 §2.3 鎖①) | 「當 DEPTH=8 時生成 8 級移位鏈」 | invariant |

行為類(參與 V2 占比分子)= reset/fsm/handshake/timing/arithmetic/counter/
memory/protocol;`param` 與 `structure`(保留詞,本弧不產)記非行為類。
**設計取向**:reset/fsm/timing/arithmetic/counter 五類提取器最穩(語法
構造直接可辨),是類別覆蓋 floor(V3≥4)的保底;handshake/protocol 標
best-effort,驗證集上稀薄不罰(閘 floor 按 ask 定 4,不虛高)。

### 2.3 反 doc-dump:資料層三鎖(doc-dump=把宣告表/語法清單冒充規則)

1. **生成端鎖**:規則只由行為構造 spawning(Always/Initial/ContAssign/
   case 臂/generate 分支/RTLIL process+connections);純宣告構造(port/
   wire/param 列表)**永不**單獨產規則。evidence 必含 ≥1 行為類
   (class∈{process,assign,case,generate,expr});decl 類只能附在行為證之後。
   **param 類側門裁決(R1-12)**:param 規則僅由 generate 行為構造
   (GenFor/GenIf 實例化分支)spawning;宣告(port/wire 宣告寬度)範圍
   內的參數引用**不**spawn 規則——param 類不成為繞行為證鎖的側門。
2. **資料端鎖**:每規則 `check` 必填且 form∈五形;form 專屬欄位非空
   (§2.4)。`fingerprint` 機械生成,同指紋=生成器 bug(同義反覆的形)。
3. **閘端鎖**(check-rulebook.mjs,§7.3):evidence 行為類斷言(獨立於
   生成端的重算)+ snippet 與源檔逐位元組對拍 + 陳述須含 ≥1 個源碼中
   grep 得到的真實標識符(禁純散文)+ 行為類占比 ≥60%(V2)+ 指紋
   全集零重複(V5)。

三鎖分立的理由:生成端是「不產」,資料端是「產不出殘品」,閘端是
「產了也過不了閘」——任何一鎖被繞過,其餘兩鎖仍攔(與 §10.6
「測試綠不是刪除安全證明」同構的雙保險思想)。

### 2.4 check.form 五形詞表

| form | 欄位 | 語義 |
|---|---|---|
| `guard` | trigger{edge?,signal?}, when, effects[{lhs,value}] | 「觸發條件成立時,{lhs} 裝載 {value}」 |
| `transition` | state_reg, from, to, when | FSM 遷移:「from 且 when → 下一拍 to」 |
| `sequence` | signal, becomes, anchor{expr}, latency, when? | 「anchor 成立後第 latency 拍 signal=becomes」 |
| `relation` | lhs, rhs, when?, width_note? | 組合恆等:「lhs ≡ rhs(逐拍)」;when 非空=**分支域適用**(條件鏈轉寫,R3-②:分支轉寫不冒充全域,讀法=「僅當 when 成立時 lhs ≡ rhs」),缺席=全域恆等 |
| `invariant` | subject, kind(domain|identity), values?/expr? | 值域/恆等式不變量 |

表達式欄位(when/effects.value/rhs)一律存**源碼渲染文本**(AST print,
不改寫、不求值)——可檢驗性由「文本可對回源碼 + form 欄位可機械判定
真假」保證,不由本弧做語義求值(求值是 vl-spec/vl-sim 軌)。

---

## 3. 破口分類學 v1(目標①;破口報告=測試缺口清單)

### 3.1 七類判定(全部基於 analyzer-core 逃逸口可支撐的結構判定)

| id | 類別 | 判定依據 | 證據形態 | suggested_test(自動給出) |
|---|---|---|---|---|
| G1 | latch 推斷 | 組合行程(判準見 §4.2 行程分類律 R1-1:**edges 空或全 Any**)內:訊號在 ≥1 控制路徑被賦值,但存在到達「無賦值」的分支路徑(按 If/Case 樹做路徑敏感 LHS 集合差);**行程頂層無條件賦值(塊首默認賦值形)對沖**——頂層語句各路徑必執行,集內訊號各路徑均有值不鎖存、不報(R3-①:subbytes.v 3/4 誤報同形實錄);剩餘報告=無默認+分支不對稱,Verilog 語義下結構可證,certain 維持 | 行程 span+訊號名 | 「枚舉 {cond} 反相輸入,斷言 {sig} 保持值(鎖存)或改為組合(設計缺陷)」 |
| G2 | 不完整 case | Case 無 default 且臂數 < 2^(選擇子寬度)(寬度=字面量範圍可證時);或選擇子為 FSM 寄存器且全域 case 臂集未覆蓋其值域 | case span+未覆蓋編碼列表 | 「施加未覆蓋編碼 3'b101,釘住輸出行為」 |
| G3 | 無驅動讀取 | 驅動圖:驅動源=ContAssign LHS+行程賦值 LHS+實例輸出連接(InstUnit.conns/Cell.connections);訊號被讀(RHS 出現)但入度=0 且非 input 埠 | 讀點 span | 「該訊號恆為未定義——補驅動或從測試中剔除」 |
| G4 | 缺復位覆蓋 | 邊沿行程有復位臂(存在復位類 If 條件)但某被時序賦值的寄存器不在任何復位臂 effect 集;行程完全無復位臂→單列 | 行程 span+寄存器名 | 「復位後直接讀 {reg},釘住其上電值」 |
| G5 | 組合環 | 組合驅動圖(ContAssign+**組合行程**[§4.2 R1-1 判準]阻塞賦值的 RHS→LHS 邊)存在環(SCC>1 或自環);行程內部語序保守忽略(跨訊號環才報) | 環路徑訊號序列 | 「構造使環上訊號同拍相互依賴的輸入,斷言振盪/鎖死可觀測」 |
| G6 | 多驅動衝突 | 同一 LHS 訊號被 ≥2 驅動源賦值(字級;part-select/位選合併到基底訊號) | 各驅動點 span | 「兩驅動源同拍衝突輸入,釘住解析序(依 AGENTS §2 調度確定性契約,ADR-0001)」 |
| G7 | 邊沿行程阻塞寫 | 邊沿觸發行程內對 reg 的阻塞賦值(BAssign),且該 reg 也被其他行程賦值(跨行程競爭面;對齊 AGENTS §10.5 競爭免疫紀律的靜態面) | 賦值 span | 「構造兩行程同邊沿競爭場景(或標 race-ok 豁免)」 |

### 3.2 與「完整測試」的關係(目標①的第二半)

每個破口=一個**行為未定/未覆蓋點**,即測試必須釘住(或設計必須修掉)
的缺口:`GapReport` 每項攜帶 `suggested_test`(機械模板生成),
直接可轉 TB 計劃條目。破口報告因此**就是測試缺口清單**:
「完整測試」的操作定義=suggested_test 全部有對應 TB 條目(本弧產清單,
TB 執行屬 vl-sim/vl-spec 軌,非目標 §7)。

置信度欄位:`confidence: "certain"|"heuristic"`——G1/G5/G6/G7 結構可證
=certain;G2 值域覆蓋與 G3 驅動圖在 include/宏場景受限=heuristic。
評審員按 confidence 抽查誤報(R6 風險的緩解)。

---

## 4. Rust 核心落點(決策 D1:sibling crate `rtl-spec`,不擴 analyzer-core)

**理由**:①analyzer-core 文檔自劃禁區「求值/解析=禁區,查詢集=既有 IR
欄位投影」(lib.rs:2-7)——規則歸納是新語義層,塞入即違其立檔契約;
②弧約束「禁破壞其測試與 API」——零接觸是唯一保證;③規模:提取器+
破口掃描+驅動圖是數千行級,獨立 crate 依賴邊清晰
(`rtl-spec → {analyzer-core, vl-syntax, ry-ir, rcs-diag}`)。

### 4.1 公開 API(crates/rtl-spec/src/lib.rs 簽名草案)

```rust
//! rtl-spec — RTL 行為規則書提取與破口掃描核心(ADR-0012)。
//! 語義姿態對齊 analyzer-core:永不 panic、錯誤進結果(lib.rs:66-67 同構);
//! 輸入=源文本字串(零 FS 觸碰→wasm 安全);零 serde,手寫 JSON **只寫不讀**
//! (鍵序=寫出序 byte-exact 契約,單測釘死;沿 analyzer-core report.rs 形;
//! 庫內零 JSON parser 依賴——opts 標量化見 R1-5/D12,備選
//! analyzer_core::proto::json::Json::parse[proto/json.rs:87 pub]不採)。

use analyzer_core::SourceInput;          // 直接複用前弧入口枚舉(零複製)

// opts 標量化(R1-5/D12):不設 options struct、不引 JSON 讀取器。
pub const DEFAULT_MAX_RULES: usize = 128; // extract_rules 的模組內規則上限
// gap 掃描 v1 全類開(過濾在呼叫層做),無 opts。

#[derive(Clone, Debug)]
pub struct Rulebook {
    pub source: String,
    pub parse_errors: Vec<String>,       // **僅**解析錯誤(有誤照提;R2-6:
                                         //  TB 跳過/未支援注記不入此欄)
    pub skipped: Vec<SkippedNote>,       // tb_file|unsupported 注記(R2-6)
    pub modules: Vec<ModuleRules>,       // 一模組一組
    pub truncated: Vec<String>,          // 超 max_rules 被截斷的模組名
}
pub struct ModuleRules { pub module: String, pub rules: Vec<Rule> }
pub struct SkippedNote { pub file: String, pub reason: SkipReason }

#[derive(Clone, Debug)]
pub struct GapReport {
    pub source: String,
    pub parse_errors: Vec<String>,
    pub skipped: Vec<SkippedNote>,       // 同 Rulebook(R2-6)
    pub items: Vec<GapItem>,             // §3.1 七類
}

/// 結構投影摘要(rtl_analyze 的資料源,R2-1 閉合;零新語義=
/// analyzer-core Analysis 公開查詢直讀組合:modules()/instance_edges()/
/// diagnostics,欄位=ModuleInfo view.rs:8-28 直投)。
#[derive(Clone, Debug)]
pub struct AnalysisSummary {
    pub source: String,
    pub has_errors: bool,
    pub diagnostics_count: usize,
    pub modules: Vec<ModuleSummary>,
    pub instance_edges: Vec<(String, String)>,   // analyzer-core 同名直通
}
pub struct ModuleSummary {
    pub name: String,
    pub params: Vec<(String, String)>,            // (name, value 文本)
    pub ports: Vec<(String, String, u32, bool)>,  // (name, dir, width, signed)
    pub nets: Vec<(String, String, u32, bool)>,   // (name, kind, width, is_port)
    pub cells: Vec<(String, String, usize)>,      // (name, ctype, port_count)
    pub proc_count: usize,
    pub assign_count: usize,
}
pub struct GapItem {
    pub id: String,                      // "G1-<module>-<nn>"
    pub class: GapClass,                 // Latch|IncompleteCase|UndrivenRead|
                                         // MissingReset|CombLoop|MultiDriver|
                                         // BlockingInSeq
    pub module: String,
    pub confidence: Confidence,          // Certain|Heuristic
    pub message: String,                 // 中文
    pub evidence: Vec<Evidence>,         // 同 Rule.evidence 形
    pub suggested_test: String,          // §3.1 測試缺口模板
}

/// 提取規則書(永不 panic;parse 錯誤進 Rulebook.parse_errors)。
pub fn extract_rules(input: &SourceInput, max_rules_per_module: usize) -> Rulebook;
/// 破口掃描(同姿態;v1 全類開)。
pub fn scan_gaps(input: &SourceInput) -> GapReport;
/// 結構投影摘要(rtl_analyze 的源;R2-1/R2 新增,零新語義直讀組合)。
pub fn analyze_summary(input: &SourceInput) -> AnalysisSummary;

/// JSON 寫出(手寫,鍵序=本檔 §2.1 順序;單測 byte-exact)。
pub fn rulebook_json(rb: &Rulebook) -> String;
pub fn gap_report_json(gr: &GapReport) -> String;
pub fn analysis_json(s: &AnalysisSummary) -> String;

impl Rulebook    { pub fn render_markdown(&self) -> String; }
impl GapReport   { pub fn render_markdown(&self) -> String; }
```

內部模組切分:`src/schema.rs`(Rule/GapItem 類型)、`src/sv/`(SV 側
提取器與掃描器,走 `Analysis.sv` 逃逸口)、`src/rtlil.rs`(RTLIL 側
子集,走 `Analysis.rtlil` 逃逸口,結構路徑 evidence)、`src/summary.rs`
(analyze_summary=R2-1 新增,analyzer-core 公開查詢直讀組合)、
`src/json.rs`、`src/md.rs`。提取器共通骨架:analyze → 有 errors 記 parse_errors →
逐模組 → 逐構造類 spawn 規則 → 鍵串去重(R1-2/D13:鍵串含 source 維)→
截斷保護。

### 4.2 提取器演算法要點(SV 側;RTLIL 側為子集映射)

- 逐模組建 `DriverMap`:ContAssign LHS / 行程賦值 LHS(遞迴 Stmt 樹,
  路徑敏感)/ 實例輸出連接(InstUnit.conns)→ G3/G5/G6 的圖基礎。
- **行程分類律(R1-1,對齊 parser 實形)**:組合行程=「`ctrl` 為
  `Some(EventCtrl{edges: []})`(`@*`/`@(*)` 兩形,parser/stmt.rs:431-446)
  **或** edges 非空但全部 `EdgeKind::Any`(`@(a or b)` 信號列表,stmt.rs:482)
  **或** `ctrl=None`(裸 always,parser/module.rs:344-356,罕見形)」;
  邊沿行程=edges 含 `Posedge`/`Negedge`(ast.rs:430-431)。**禁**以
  「ctrl 是否 None」二分——@* 塊解析為 Some(空 edges),照舊律兩頭不靠
  (驗證集 generate.v:180、mem2reg.v:10,104 即 @* 形;G1 正樣
  latch_trap.v 天然 @* 形)。Initial=TB 面不產規則(檔案含 `$finish`
  啟發判定 TB→整檔跳過,**跳過記入 skipped 欄**而非 parse_errors
  (R2-6:語義不過載)——TB 不是被提取對象)。
- 復位臂辨識:邊沿行程體首層 If 的 cond 只含一個訊號與否定=
  復位條件候選;其 effect 集即復位值(G4 的反例集)。
- FSM 辨識:邊沿行程內 Case 選擇子=同名寄存器(在該行程被 NbAssign)
  → case 臂=遷移邊(from=臂常量,to=該臂 effect 中 state_reg 新值,
  when=臂內 If 條件渲染)。
- `timing`/`counter` 從同一賦值對集合分流:rhs 含自身=
  counter 候選(`r+1`/`r-1`/三元環繞),否則 timing(sequence 形)。
- `arithmetic`(R1-3 放寬):ContAssign 與組合行程賦值,rhs 含**任意
  Binary 節點**(算術+位運算/邏輯;ast.rs:445 起 Expr::Binary)→
  relation 規則——carryadd.v 全檔僅 & | ^ 位運算,嚴格「算術」讀法
  零行為規則。**適用域(R3-②)**:組合行程賦值攜條件鏈入 relation 的
  `when`(分支域轉寫不冒充全域恆指——subbytes.v「state==0 else 分支
  的 next_state=state+1 被全局誤讀預測狀態環」實錄);ContAssign=
  全域,when=None。rhs 為變量/常量拷貝(無 Binary)不產 relation=
  覆蓋缺口,報告誠實注記載明。
- 表達式渲染:自寫最小 printer(還原運算符/括號優先級),渲染文本
  存進 check 欄位;**不引入求值**(禁區 §0)。
- RTLIL 側:Process.root_case/syncs(ry-ir ast.rs:290-332)直接給
  復位/邊沿/動作對,映射到同 schema;evidence 用結構路徑(D2)。

---

## 5. wasm 面(決策 D3:新 crate `rtl-spec-wasm`,沿 vl-wasm 形態)

- crate-type=["cdylib","rlib"];依賴僅 wasm-bindgen 0.2 + rtl-spec
  (**零 js_sys**——本面無回調無狀態,純函數進出,比 vl-wasm 更簡)。
- 構建:`wasm-pack build crates/rtl-spec-wasm --target web --out-dir pkg`
  ——**沿 regress 既證的 web 目標**(scripts/regress.sh:201;vl-wasm
  run.mjs 在 bun 下 `await init()` 是既成事實),不採未驗證的
  `--target bun`(非目標 §10)。
- 導出面(JSON 字串進出,**無 BigInt 邊界**——行號 u32 上限內,無 u64
  跨界需求;憲法的 BigInt 律在場但不觸發):

```rust
#[wasm_bindgen]
pub fn rtl_spec_version() -> String;
// sv 面(R2-1 閉合:analyze 導出補齊 rtl_analyze 工具的資料源)
#[wasm_bindgen]
pub fn rtl_analyze_sv(name: &str, source: &str) -> String;
#[wasm_bindgen]
pub fn rtl_extract_rules_sv(name: &str, source: &str, max_rules: u32) -> String;
#[wasm_bindgen]
pub fn rtl_scan_gaps_sv(name: &str, source: &str) -> String;
// rtlil 面(R2-1:scan/analyze 補齊,gap_count 對 .il 檔不再無源)
#[wasm_bindgen]
pub fn rtl_analyze_rtlil(name: &str, source: &str) -> String;
#[wasm_bindgen]
pub fn rtl_extract_rules_rtlil(name: &str, source: &str) -> String;
#[wasm_bindgen]
pub fn rtl_scan_gaps_rtlil(name: &str, source: &str) -> String;
```

(共 7 導出=§6.1 四工具逐一面閉合:rtl_analyze←rtl_analyze_{sv,rtlil},
rtl_scan_gaps←rtl_scan_gaps_{sv,rtlil},批量工具同源;opts 標量化對應
R1-5/D12:入參零 JSON 字串,邊界面無讀取器需求。)

  返回恒為 JSON 字串(錯誤內嵌 `{"error":...}`,不拋 JsValue)——
  跨運行時零分歧、與 MCP structuredContent 直接對接。
- **bun/node 雙兼容策略**:web 目標 glue 是純 ESM;裝載一律
  **顯式位元組**——`init({ module_or_path: new Uint8Array(readFileSync(...)) })`,
  繞開 fetch(file://) 的運行時差異(S4 雙運行時冒煙先證,R1)。
  bun 1.4.2 + node v26.12.1 都支援 ESM+WebAssembly;若 node 腿實測
  失敗,如實記錄「bun-only」,manifest/SKILL 注明,不謊稱雙兼容。
- 驗收閘:wasm 輸出與 native 同輸入 **byte 相等 parity**(同 JSON 寫出器,
  同一 wasm 二進制在兩運行時輸出一致)。

---

## 6. MCP server(目標③④;plugin/rtl2spec/dist/server.mjs)

- **手寫純 JS(ESM `.mjs`),零 npm 依賴**(node:* 內建模組 only),源=
  產物零編譯步(R1-4/D11:原「TS」聲明不實——無 tsc/bun build 步;
  擇手寫 JS 而非 analyzer-ts 的 bun 直跑 .ts 先例,理由=雙運行時
  兼容不賭 node type-stripping 版本行為;類型契約面=手寫 server.d.ts
  附注);stdio 上 **newline 分幀 JSON-RPC 2.0**(MCP stdio 傳輸標準;
  自寫緩衝+按 \n 切幀,R2 風險注記)。
- 方法面:`initialize`(protocolVersion 在 SUPPORTED 五版列表則回顯,
  否則回 LATEST="2025-11-25";capabilities={tools:{}};serverInfo=
  {name:"rtl2spec",version:"0.1.0"})/`notifications/initialized`(吞)/
  `tools/list`/`tools/call`/`ping`(回 {})。未知名=-32601;參數形錯
  =-32602。
- **stdout 純淨律**:stdout 只出 JSON-RPC 幀;一切診斷/日誌走 stderr。

### 6.1 工具清單(4 個;inputSchema 均 {type:"object",properties,required})

| tool | 入參 | 出參(structuredContent) | 語義 |
|---|---|---|---|
| `rtl_analyze` | `{path?, source?}`(**至少一**;皆給以 path 為準,皆缺=工具級錯誤[R1-11:平鋪 JSON Schema 表達不了 oneOf,語義由實現裁決+e2e 斷言把守]);`lang?:"sv"\|"rtlil"`(缺省按副檔名 .v/.sv/.vh→sv,.il→rtlil);path 相對 cwd | `{has_errors, diagnostics_count, modules:[{name,params,ports,nets,cells,proc_count,assign_count}], instance_edges}` | 結構投影報告(分析第一站) |
| `rtl_extract_rules` | 同上;`format?:"json"\|"markdown"\|"both"`(默認 both) | `{rulebook:{source,parse_errors,modules:[{module,rules:[Rule…]}],truncated}, markdown}` | 規則書提取(§2) |
| `rtl_scan_gaps` | 同上 | `{gaps:[GapItem…], test_plan:[suggested_test…]}` | 破口獵捕(§3) |
| `rtl_extract_batch` | `{paths:string[](≤64)}` | `{results:[{path,ok,rule_count,gap_count,error?}]}` | 批量提取+匯總(驗證用) |

- 錯誤語義(依勘察事實 §6 的 SDK 契約;**R3-⑤ 對帳代碼現實**):
  **工具級錯誤**(檔案不存在/讀取失敗/**path 與 source 雙缺**[R1-11]/
  batch `paths` 超 64)→ result `{isError:true, content:[text]}`;
  **協議級錯誤**(未知名/壞參,含 batch `paths` 非陣列=invalid params
  -32602,server.mjs 預檢實測)→ JSON-RPC error。規則書提取對 parse
  錯誤**不是**工具級錯誤(parse_errors 進結構化輸出,部分模組照提)。
  **lang 面**:`rtl_analyze`/`rtl_scan_gaps` 均有 sv+rtlil backing 導出
  (§5 七導出,rtl-spec-wasm lib.rs 實證)——**不存在「未支援 lang」
  與「RTLIL 首錯硬錯」錯誤路徑**(server 實現:lang 缺省按副檔名,
  其餘一律 sv;全部 tool×lang 組合可達導出)。`rtl_scan_gaps` ×
  lang=rtlil **非工具級錯誤**:正常返回 v1 子集面(僅 G1 latch/
  G5 comb_loop 類,RTLIL v1 子集契約見 §4.2/S4 實作記錄),其餘類
  在 RTLIL 形不產=空集非錯。
- wasm 裝載:啟動時 `RTL2SPEC_WASM_DIR`(env,manifest 注入
  `${ZCODE_PLUGIN_ROOT}/dist/wasm`;缺省 fallback 同目錄 `wasm/`)讀
  `rtl_spec_wasm_bg.wasm` 位元組→`init`(§5 顯式位元組策略)→四導出
  直呼。載入失敗=server 退出非 0(stderr 說明)——禁「降級假活」。

---

## 7. zcode plugin 包(目標⑤前半;決策 D5)

### 7.1 目錄樹

```
plugin/rtl2spec/
├── .zcode-plugin/plugin.json      # manifest(D5 schema)
├── skills/
│   ├── rtl-analyze/SKILL.md       # RTL 分析技能
│   ├── rtl-rulebook/SKILL.md      # 規則書提取技能
│   └── rtl-gap-hunt/SKILL.md      # 破口獵捕技能
├── dist/
│   ├── server.mjs                 # §6 MCP server(手寫源,R1-4/D11)
│   └── wasm/{rtl_spec_wasm.js, rtl_spec_wasm.d.ts, rtl_spec_wasm_bg.wasm}
│                                   #    ^ 構建產物(同步步刷新)
├── scripts/
│   ├── validate-manifest.mjs      # schema 靜態校驗
│   ├── check-rulebook.mjs         # §7.3 機械判據執行器(裝載 dist wasm)
│   └── gen-report.mjs             # validation-report.md 生成
├── fixtures/                      # 本弧自著小型 Verilog(repo MIT;零外部派生)
│   ├── fsm_counter.v              # 正樣:復位+FSM+counter 齊備
│   ├── latch_trap.v               # G1 觸發形
│   └── incomplete_case.v          # G2 觸發形
├── run-mcp-e2e.sh                 # 總閘成員①(§8.1)
├── run-rtl-validation.sh          # 總閘成員②(§8.2)
└── validation-report.md           # 生成產物,機械生成禁手改,入庫=證據頁
```

dist 分工(R1-4):`dist/wasm`=**構建產物**(由 run-mcp-e2e.sh 的同步步
從 crates/rtl-spec-wasm/pkg 刷新);`dist/server.mjs`=**手寫源**
(源=產物,零編譯步,入庫);fixture/腳本/manifest 均為手寫源。

### 7.2 manifest 草案(勘察事實 §5 schema 逐欄對齊 excalidraw 0.3.4 實測形)

```json
{
  "name": "rtl2spec",
  "version": "0.1.0",
  "description": "RTL behaviour rulebook extraction and gap hunting (analyzer-core wasm face)",
  "author": { "name": "rust-cad-suite" },
  "license": "MIT",
  "skills": "skills",
  "mcpServers": {
    "rtl2spec": {
      "type": "stdio",
      "command": "bun",
      "args": ["${ZCODE_PLUGIN_ROOT}/dist/server.mjs"],
      "cwd": "${ZCODE_PROJECT_DIR}",
      "env": { "RTL2SPEC_WASM_DIR": "${ZCODE_PLUGIN_ROOT}/dist/wasm" }
    }
  }
}
```

command 定 bun(在機 1.4.2 實測);node 路徑=`node dist/server.mjs`
(等價 ESM),寫進 SKILL 注記,S4 驗證後才聲稱。

### 7.3 skills×3 要點(SKILL.md 骨架;正式條文 S5 落盤)

1. **rtl-analyze**:觸發詞=RTL 分析/模組結構/埠表/例化關係。流程=
   rtl_analyze → has_errors 先解讀診斷(錯誤在場時規則書/gap 結果標
   受限)→ 產出結構摘要。要點=投影語義的已知形(parametric 寬度→0、
   RTLIL span 無)——引用勘察事實 §1,防誤讀。
2. **rtl-rulebook**:觸發詞=規則書/spec 提取/行為規則。流程=
   rtl_extract_rules → 自查 V2 行為占比(工具回 structuredContent 含
   category 分布)→ 按五 check form 讀規則。要點=「規則書不是文件轉寫」
   三鎖讀法(§2.3);qualifier 差異化信任度。
3. **rtl-gap-hunt**:觸發詞=破口/latch/組合環/多驅動/測試缺口。流程=
   rtl_scan_gaps → 按 class 歸併 → suggested_test 逐條轉 TB 計劃條目。
   要點=confidence 差異化(certain 可直接修,heuristic 先人工核);
   破口=測試缺口清單(§3.2)。

### 7.4 裝載驗證(repo 內兩步;禁動 ~/.zcode)

- **靜態**:scripts/validate-manifest.mjs——manifest 必填欄位/型別、
  `${ZCODE_PLUGIN_ROOT}`/`${ZCODE_PROJECT_DIR}` 佔位符可解析、skills
  三目錄在場且 YAML frontmatter 含 name+description、dist 產物在場且
  列印 sha256(wasm 二進制指紋=報告溯源)。exit 0/1 表意。
- **動態 dry-run**:即 run-mcp-e2e.sh(§8.1)——真實 spawn+回環,
  等價「假裝 zcode 裝載後的最小使用面」。使用者環境裝載不在本弧
  (非目標 §1)。

---

## 8. 兩條插件閘(總閘成員;腳本直執)

### 8.1 run-mcp-e2e.sh(MCP 端到端閘)

流程:①守衛=bun/wasm-pack 在場(缺→exit 2 表意,不靜默跳過——
本閘是本弧交付證明,跳過=假綠);②pkg 陳舊檢查(`find
crates/rtl-spec/src crates/rtl-spec-wasm/src crates/analyzer-core/src
crates/vl-syntax/src crates/ry-ir/src crates/rcs-diag/src -newer
pkg/...bg.wasm` → `wasm-pack build --target web --out-dir pkg`,沿
regress.sh:199 形——**傳遞依賴目錄必須列全**,R1-6:改 analyzer-core
而不觸發重構建=雙閘驗陳舊 wasm,「閘驗出貨物」(D8)即謊言)→ 同步
pkg→plugin/rtl2spec/dist/wasm;③`bun scripts/mcp-client.mjs` 驅動:
spawn `bun dist/server.mjs` → newline 幀回環:initialize(serverInfo.name==
"rtl2spec"、protocolVersion 回顯)→ tools/list(恰 4 工具,inputSchema
.type=="object")→ tools/call rtl_extract_rules@fixtures/fsm_counter.v
(structuredContent.rulebook.modules≥1 且 rules≥3)→ tools/call
rtl_scan_gaps@fixtures/latch_trap.v(gaps 含 Latch 類,**且正樣為 @* 形
=分類律 R1-1 的端到端回歸錨**)→ 負樣:tools/call 不存在檔案
(isError:true)+rtl_analyze path/source 雙缺(isError:true,R1-11)+
未知名(-32601)→ kill 子進程;
④斷言任一失敗→exit 1;構建失敗→exit 3;成功→exit 0。輸出逐行表意。

### 8.2 run-rtl-validation.sh(真實 RTL 驗證閘)

流程:①守衛同上+驗證集 13 檔案在場(缺→exit 2,檔名逐一列出);
②構建/同步同 8.1;③逐檔 `bun scripts/check-rulebook.mjs <file>`
(裝載 **dist 同一 wasm**——閘驗出貨物,D8):提取規則書+破口,執行
V1..V6 機械判據(§8.3),逐檔 JSON 判定;④`bun scripts/gen-report.mjs`
匯總生成 validation-report.md(§8.4);⑤任一檔任一判據 FAIL→exit 1。
**語料唯讀**:腳本只讀 corpus/與 corpus-external/,寫只落 plugin/rtl2spec/
(弧約束;SUMMARY 基線零接觸)。

### 8.3 機械判據(閘契約;降閾值=ADR-0012 修訂,禁靜默)

| 判據 | 斷言 | 閾值 |
|---|---|---|
| V1 規則數 | 全集 Σrules;單檔 rules;**截斷事件** | 全集 ≥50;單檔 ≥3(skipped[tb_file] 檔不計單檔≥3,報告單列[R2-6]);**truncated 非空→該檔 FAIL**(R1-9:上限的防注水語義必須有執法力,只標註不紅=無閘性;模組內上限參數默認 128) |
| V2 行為占比 | Σ(行為類規則)/Σrules | 全集 ≥60%;任一檔 ≥40%(**冗餘防線**定位見下,R1-7) |
| V3 類別覆蓋 | 全集 distinct category 數 | ≥4(ask floor;目標 ≥6,報告實值) |
| V4 溯源 | 每規則 evidence.snippet 與源檔對應行**按拼接規則**(§2.1 R1-10:多行以 `\n` 連接行原文)逐位元組相等(checker 重讀源檔對拍,非信任生成端);SV 側 100% 帶 file:line | 100% |
| V5 反 doc-dump | ①每規則 evidence 含 ≥1 行為類(**主判據**);②check.form∈五形且 form 欄位非空;③**鍵串**(含 source 維,R1-2)全集零重複;④陳述含 ≥1 個源碼 grep 可得的標識符(**冗餘防線**,D7 模板必嵌訊號名=構造即過) | 100% |
| V6 全檔成功 | 13 檔全部成功呼叫(產 rulebook 或 skipped 注記;skipped 檔報告單列,不混入 FAIL 歸因[R2-6]) | 13/13 |

**V2/V5-④ 定位聲明(R1-7)**:生成端鎖①已保證除 param 外只由行為
構造 spawning,故 V2 實測的常態是 100%(僅量 param 稀釋度),V5-④
因陳述模板必嵌源碼訊號名而構造即過——兩者是**冗餘防線**(守生成端
鎖的回歸探針,防未來鎖①被改壞),**主判據=V5-①(行為證)+V4(逐
位元組溯源)**;validation-report.md 與 SKILL 條文須如實如此定位,禁
把 V2/V5-④ 包裝成 doc-dump 主防線。

驗證集(13 檔;license 表入報告):

| 檔案 | license |
|---|---|
| corpus/e2e/59_i2c_master/i2c_master.v(316 行) | repo MIT |
| crates/spec-i2c/rtl/i2c_master.v(乾淨室,OpenCores 唯讀啟發零拷貝) | repo MIT |
| corpus/e2e/58_xls_acc_pipe/acc_pipe.v、57_xls_add3/add3.v | Apache-2.0 |
| corpus-external/yosys-simple/{subbytes,fiedler-cooley,sincos,multiplier,memory,carryadd,generate,mem2reg,dff_different_styles}.v(9 檔) | ISC(COPYING 隨附;唯讀引用,原檔不入 plugin/) |

(全 in-git;sv-elab 樹本機不存在不入集;zipcpu=GPL 永不入集。)

「有意義規則書」判據與機械化的映射:
- **R1 溯源**=V4(snippet 逐位元組對拍=機械全量,不止抽查);
- **R2 可檢驗**=V5-②(form 詞表完備)機械 + 評審員抽查散文感(定性);
- **R3 非同義反覆**=主腿 V5-①③(行為證+含 source 鍵串去重),V2/V5-④
  為冗餘防線(R1-7 定位);
- **R4 覆蓋面**=V3;
- **R5 增值**=非機械(評審員自擬 3 探針問題,只讀規則書作答,對照
  粗讀 RTL);報告為其提供素材=≥3 代表檔規則書全文樣本+破口樣本
  (§8.4)。

### 8.4 validation-report.md 形態(機械生成禁手改;入庫=證據頁)

```
# rtl2spec 真實 RTL 驗證報告
- 日期 / 命令行 / 退出碼 / 工具鏈版本(bun、wasm-pack、wasm sha256)
- 閘契約表:V1..V6 閾值 + 實測值 + PASS/FAIL
- license 表(§8.3 第二表,機械複製)
- 逐檔表:檔案|行數|模組數|規則數|類別分布|破口數(按類)|判定
- 規則書全文樣本(≥3 代表檔:≥1 yosys、i2c_master、≥1 xls;markdown 渲染全文)
- 破口報告樣本(≥2 檔,含 suggested_test)
- 評審員區(模板佔位:探針問題×3/檔×代表檔、答案引用 rule id、
  R2 散文感抽查記錄)——S7 填寫,屬人工區但寫進同一檔案
- 總閘命令清單(文字塊,機械生成)
```

### 8.5 總閘(腳本執行的固定序列)

```
cargo test --workspace
bash scripts/regress.sh
bash plugin/rtl2spec/run-mcp-e2e.sh
bash plugin/rtl2spec/run-rtl-validation.sh
```

四件全 exit 0 才算過。S7 同輪把兩插件閘以**雙守衛降級形**收編
regress.sh(D9/R2-4:守衛=`crates/rtl-spec-wasm/pkg` 在場+bun 在場,
在場時兩閘 exit≠0 即整閘紅;469/479[regress.sh:318]+456/456[:348]
斷言數字逐字不動)——收編後 regress 內必綠(§10.7 執法形)+總閘直調
雙保險。

---

## 9. 四層立法落點(§10 質量體系;S1 起架,S7 終落)

| 層 | 落點 |
|---|---|
| ①機械閘 | run-mcp-e2e.sh + run-rtl-validation.sh(總閘成員,exit≠0 有執法力;V1..V6 判據即條文)+ regress.sh 雙守衛插件塊(D9/R2-4,§10.7 執法形,S7 收編) |
| ②SKILL 正典 | .agents/skills/eda-dev/SKILL.md 新增「rtl-to-spec 增補」節:反 doc-dump 三鎖條文、五 check form 詞表、V1..V6 閾值指針、RTLIL 結構證形態律、fixtures 自著授權律、零 serde 手寫 JSON 鍵序契約 |
| ③AGENTS 條文 | AGENTS.md 新增 §10.10「rtl-to-spec 紀律」:三面一路形(D1/D3)、規則書資料層三鎖、破口=測試缺口清單、插件閘雙腳本=總閘成員、語料唯讀+dist 重建律 |
| ④證據頁 | plugin/rtl2spec/validation-report.md(機械生成,V1..V6 實測表+wasm sha256);MCP e2e 輸出由 S7 實跑捕獲記入 OKR 證據指針(docs/OKR.md 增 KR 行,只指可重跑命令,反灌水) |

**ADR-0012**(docs/adr/0012-rtl-to-spec-plugin.md)骨架:背景(目標①..⑤+
ADR-0007 關係=在 vl-spec 前面補「提取」一環,產物可對接其三層形態做
驗證閉環,本弧不實做對接)/決策 D1..D10/機械閘/非目標。
**PLAN.md**:P3.8 弧行(rtl-to-spec 插件弧,2026-11-03 立案,ADR-0012)+
里程碑表新行 `RTL1..RTL5 | rtl-to-spec 插件(rtl-spec/rtl-spec-wasm/
plugin/rtl2spec;正典=docs/RTL2SPEC-PLUGIN.md,ADR-0012) | 總閘四件套;
run-mcp-e2e PASS;run-rtl-validation PASS(V1..V6);基線 469/479+456/456
逐字不動`。
**REGISTRY.md**:Crates 表增 rtl-spec/rtl-spec-wasm 行;組件×能力×閘×
證據關係表增插件行(同輪更新義務)。

### 決策記錄(D1..D10;每條帶理由與證據)

- **D1 核心=sibling crate `rtl-spec`,不擴 analyzer-core**:analyzer-core
  自劃禁區(lib.rs:2-7)+弧約束零破壞+規模;提取是新語義層。
- **D2 RTLIL 側 evidence=結構路徑,驗證閘 V4/R1 只對 SV 側斷言**:
  ry-ir 無 span(view.rs:26-27)+line_col RTLIL=Err(lib.rs:127-138);
  驗證集全為 SV 語料故 R1 可全量機械。
- **D3 wasm=新 crate `rtl-spec-wasm`+`--target web`+顯式位元組 init+
  JSON 字串跨界**:regress 既證 web 目標(regress.sh:201);零 js_sys;
  無 BigInt 面;不採未驗證的 --target bun。
- **D4 MCP=stdio newline 分幀 JSON-RPC 2.0,四工具**:MCP stdio 標準;
  工具內錯誤走 isError:true(勘察事實 §6 SDK 契約);stdout 純淨律。
- **D5 manifest=`.zcode-plugin/plugin.json`+mcpServers stdio+ZCODE_PLUGIN_ROOT**:
  excalidraw 0.3.4 實測形(勘察事實 §5);hexin 的根 .mcp.json 落點不用
  (單一落點簡單可校驗)。
- **D6 閾值 V1..V6 如 §8.3**:floor 全部≥ask 契約(≥4 類);V2=60% 取
  「行為類應占實質比例」的可執行化;注水防護=模組內
  max_rules_per_module(默認 128,D12 標量)+ truncated 非空→該檔
  FAIL(V1 執法,R1-9/R2-3 同步——無「檔級 400」機制,舊稿殘留已清);
  降閾值唯一路徑=ADR-0012 修訂+閘重跑。
- **D7 規則陳述=中文+源碼標識符原文名**:工作語言中文;標識符不改名
  保證 snippet/對拍/評審溯源一致。
- **D8 checker 裝載 dist 同一 wasm**:閘驗出貨物而非平行實現;
  native/wasm parity 另由 S4 單測覆蓋(兩閘不重複但互補)。
- **D9 插件閘 S7 以雙守衛降級形收編 regress.sh(R2-4 修)**:§10.7
  明文三面能力變更機械閘=regress.sh 內必綠,analyzer 先例(S5 analyzer
  smoke 塊=守衛 `crates/analyzer-server/Cargo.toml` 在場+bun 在場,
  ce29bfc)即此形;「基線凍結⇒regress.sh 零改動」是加碼解讀——凍結只
  凍 SUMMARY 斷言數字(regress.sh:318 `SUMMARY ys=469/479`、:348
  `LIVE ys=456/456`,本輪實讀確認),守衛塊不觸基線。形:守衛=
  `crates/rtl-spec-wasm/pkg` 在場+bun 在場;在場時
  run-mcp-e2e.sh/run-rtl-validation.sh exit≠0 即 fail=1 整閘紅;總閘
  四件套保持(regress 收編+總閘直調=雙保險)。
- **D10 命名 `rtl-spec`/`rtl-spec-wasm`**:避讓 vl-spec/spec-i2c
  (ADR-0007:21-27);rtl- 前綴標「提取側」,vl- 留給「驗證側」;
  包名 rtl2spec 只用於 plugin(使用者可見形),crate 用連字符正名。
- **D11 server=手寫純 JS(ESM .mjs),源=產物零編譯步(R1-4)**:原
  「TS(ESM)」聲明無構建路徑著落(無 tsc/bun build 步);擇 JS 而非
  analyzer-ts 的 bun 直跑 .ts 先例——雙運行時兼容不賭 node
  type-stripping 的版本行為;類型契約面以手寫 server.d.ts 補。
- **D12 opts 標量化(R1-5)**:extract_rules 收 `max_rules_per_module:
  usize` 標量、scan_gaps 無 opts(v1 全類開,過濾在呼叫層)——庫內
  零 JSON 讀取器;備選複用 analyzer_core::proto::json::Json::parse
  (proto/json.rs:87,pub 實證)不採:標量已覆蓋全部需求,少一個
  跨 crate API 依賴面。
- **D13 fingerprint=拼接鍵串,零哈希(R1-2/R1-8)**:鍵=
  `source|category|module|subject|when|effects 渲染`,去重=字符串
  精確比對——repo 內無 SHA-1/SHA-256 實現(rg 實證:crates/ 內僅
  ry-difftest 測試字符串誤配),零依賴原則下不手搓哈希;source 維
  入鍵(兩檔同名 module i2c_master 實證:spec-i2c/rtl:25 與
  corpus/e2e/59:27),否則跨檔同規則誤殺 V5-③。
  **source 維取值契約(R3-④釘死)**=傳入 `extract_rules` 的 `name`
  參數逐字(§2.1 欄位契約);禁 basename 慣例(兩檔 basename 同名,
  慣例化=R1-2 防線靜默回歸)。when 維含 relation(R3-②同輪)。

---

## 10. 切片分解(S1..S7;每片帶自驗與驗收)

- **S1 立法落盤**:本檔(已落)+ADR-0012 全文+PLAN.md P3.8 行+里程碑表
  RTL 行+SKILL「rtl-to-spec 增補」節骨架+REGISTRY 預登記。
  自驗:rg 交叉引用一致性(本檔/ADR/PLAN 互相指到的路徑與閘名逐字)。
  驗收:四層立法骨架四件在場且引用閉合。
- **S2 rtl-spec 核心**:crate 骨架+schema+手寫 JSON writer(只寫不讀,
  D12)+SV 提取器(reset/fsm/timing/arithmetic[含位運算/邏輯 Binary,
  R1-3]/counter 五基類;行程分類律按 §4.2 R1-1 edges 判準)+
  markdown 渲染+單測(include_str! 自著 fixtures;repo 語料只讀集成
  測試 2 例:carryadd.v/fiedler-cooley.v——**carryadd 必產 ≥3 條**
  =R1-3 放寬的回歸錨)。
  驗收:`cargo test -p rtl-spec` 綠;`cargo test --workspace` 對前弧
  測試數零擾動(analyzer-core 34+1 等不變)。
- **S3 破口掃描器**:DriverMap+G1..G7+confidence 標註+單測
  (**每類一觸發 fixture+一負樣 fixture**——每判定類配「會觸發形」
  與「零誤報形」,沿 AGENTS §4 第 2 條「每個恢復點配一個會觸發的
  掛死形 fixture」精神[R2-5 引用形更正])。
  驗收:cargo test -p rtl-spec 全綠;每類斷言命中/負樣零誤報。
- **S4 RTLIL 子集+wasm 面**:RTLIL 側 reset/fsm/timing 提取(結構路徑
  evidence)+**導出面 7 函數**(含 rtl_analyze_{sv,rtlil} 與
  rtl_scan_gaps_rtlil——§6.1 四工具資料源閉合,R2-1)+crates/
  rtl-spec-wasm+pkg 構建+雙運行時冒煙 mjs(bun+node;wasm vs native
  同輸入 JSON byte 相等 parity,**analyze 面同入 parity**)。
  驗收:bun mjs PASS;node mjs PASS(或如實記錄 bun-only+文檔注明);
  `cargo test -p rtl-spec-wasm` 綠。
- **S5 MCP server+插件包**:server.mjs(手寫 JS,D11)四工具+
  validate-manifest.mjs+skills×3+fixtures×3(latch_trap.v 正樣 @* 形)+
  run-mcp-e2e.sh。
  驗收:`bash plugin/rtl2spec/run-mcp-e2e.sh` exit 0(回環七斷言全過:
  initialize/list/extract/gaps/負樣三形[檔案缺失/雙缺參/未知名])。
- **S6 真實 RTL 驗證閘**:check-rulebook.mjs+gen-report.mjs+
  run-rtl-validation.sh;迭代提取器至 V1..V6 全過(**閾值不可降**,
  不足=加強提取器;確不可支撐的類別按誠實條款在 ADR-0012 修訂後
  才動閾)。
  驗收:`bash plugin/rtl2spec/run-rtl-validation.sh` exit 0;
  validation-report.md 生成且含樣本全文+license 表。
- **S7 評審與總閘**:獨立評審員定性評審(R2 散文感抽查+R5 每代表檔
  3 探針問題,寫入報告評審員區);四層立法終落(SKILL/AGENTS/OKR/
  REGISTRY 條文與證據指針);**regress.sh 雙守衛插件塊收編**(D9/R2-4,
  §10.7 執法形,基線斷言數字逐字不動);總閘四件套全跑實測記錄。
  驗收:總閘四命令逐一 exit 0 且輸出歸檔;regress.sh 帶插件塊跑綠
  (基線 469/479+456/456 逐字不動+插件兩閘塊 PASS);評審意見閉環
  (規則書修訂→閘重跑綠)。

---

## 11. 顯式不做清單(非目標)

1. 不裝載到使用者 ~/.zcode:manifest 校驗+dry-run 全在 repo 內,裝載
   與真實 zcode client 兼容性實測不在本弧(D5/R2 注記)。
2. 不動 analyzer-core/vl-spec/spec-i2c 的 API 與測試;不改 corpus/ 既有
   e2e 語料;regress.sh 僅允許追加雙守衛插件塊(D9/R2-4),既有閘與
   469/479+456/456 斷言數字逐字不動。
3. GPL 語料(zipcpu)不入驗證集、產物不派生自 GPL;sv-elab 樹本機
   不存在不入集;外部語料全部唯讀引用,原檔不入 plugin/。
4. 不做 SVA/屬性解析:vl-syntax parser v0 丟 `(* *)` 屬性(勘察事實 §1,
   parser/mod.rs:139-141);屬性依賴的規則不產(不做 inferred 腦補)。
5. 不做參數求值:參數化寬度沿用投影語義(parametric 標記,view.rs:44-48),
   param 類規則只陳述「參數 X 進入結構」不陳述數值展開。
6. 不做完整 elaborate/名字解析/include 展開:單檔單編譯單元內提取;
   include/宏依賴構造進 parse_errors 注記,不產規則。
7. 不做仿真級驗證閉環:規則書對接 vl-spec 三層形態(ADR-0007)留後續弧;
   本弧只產清單不執行 TB。
8. 不手寫 golden 期望:spec 提取無 oracle 契約(弧約束);質量閘=
   機械判據 V1..V6+獨立評審員,禁虛構 golden。
9. MCP 只做 stdio+四工具;不做 HTTP/WS/sampling/resources/prompts 面。
10. 不採 wasm-pack `--target bun`(未驗證;沿 web 既證形,D3)。
11. 不做 GUI/wasm demo 面(ADR-0005 憲法在本弧無觸點;波形/SVG 不涉及)。
12. 不做 4-state/x 不定值語義的規則表達:x/z 傳播類屬 vl-spec 軌;
    本弧規則只對 2-state 可判定構造產出。
13. 不做跨檔/跨模組層級規則(例化層級協議)——單模組規則+instance_edges
    投影報告已夠 v1;層級推理留後續弧。

---

## 12. 風險清單

| # | 風險 | 緩解/觸發應對 |
|---|---|---|
| R1 | wasm-pack web glue 在 node 的顯式位元組 init 未實測 | S4 雙運行時冒煙先證;失敗→如實記錄 bun-only,manifest/SKILL 注明,不謊稱雙兼容 |
| R2 | MCP stdio 幀形(換行分幀)與真實 zcode client 的兼容 | e2e 回環自證協議自洽+依 MCP stdio 標準;真 client 兼容性屬非目標 §1,報告注明 |
| R3 | 提取質量達不到 V1/V2 閾值 | 閾值是閘:迭代提取器;protocol/handshake 天然稀薄→類別 floor 定 4 不虛高;任何降閾=ADR-0012 修訂+重跑,禁靜默 |
| R4 | ry-ir 無 span → RTLIL 溯源無行號 | D2 結構路徑證形;R1 判據只對 SV 側斷言,報告注明雙證形 |
| R5 | 大檔(i2c 316 行/memory 15 模組)規則爆炸/注水 | 模組內 max_rules_per_module(默認 128)+含 source 鍵串去重+truncated 非空→FAIL(V1 執法,R1-9/R2-3 同步——無檔級 400 機制) |
| R6 | 破口誤報(把良構報成破口) | 每類負樣 fixture+confidence 分級+評審員抽查;誤報形進單測釘死 |
| R7 | bun/wasm-pack 環境漂移 | 雙閘守衛打印版本;缺失→exit 2 表意(不靜默跳過) |
| R7b | 提取器把 TB 檔當設計提取(語料 TB 中心) | 驗證集選設計檔(13 檔實測零 `$finish`,grep 實證);TB 啟發跳過記入**獨立 skipped 欄**(R2-6:parse_errors 僅解析錯誤),報告單列、不入 V1/V6 FAIL 歸因——未來驗證集引入 TB 檔不再誤紅 |
| R8 | 前弧 API/測試被無意破壞 | D1 零接觸+總閘 cargo test --workspace 兜底;S2 驗收含前弧測試數零擾動斷言 |
| R9 | 手寫 JSON writer 轉義 bug(引號/控制字元/非 ASCII) | 轉義敵意 fixture 單測(沿 riv-difftest 敵意 fixture 傳統);鍵序 byte-exact 單測;**只寫不讀**(D12:opts 標量化,庫內無 parser 可出錯) |

---

## 13. 修訂記錄 R1(2026-11-03 對抗審 12 條意見逐條消化;每條附實證)

R1-1 **[high] 行程分類律毒化(§4.2/§3.1 G1/G5)**——已改。實證(本輪
實讀):`always @*`/`@(*)`/`@(*)` 變體解析為
`Some(EventCtrl{edges: Vec::new()})`(crates/vl-syntax/src/parser/stmt.rs
event_ctrl:Punct::Star 與 AttrOpen 兩臂均回 `edges: Vec::new()`);
`@(a or b)` 無 posedge/negedge 關鍵字時逐項 `EdgeKind::Any`(同函
loop);僅裸 always 才 `ctrl=None`(parser/module.rs:344-356,
`at_punct(Punct::At)` 才 Some)。舊律「ctrl=None=組合」照做則 @* 塊
兩頭不靠。修:§4.2 行程分類律改三肢判準(edges 空 ∨ 全 Any ∨ None=
組合;含 Posedge/Negedge=邊沿),§3.1 G1/G5 判準同步引用。**事實
修正**:意見所引 multiplier.v:105 經實讀為 `//always@* foreach…`
**注釋行**,非有效使用——驗證集有效 @* 使用=generate.v:180、
mem2reg.v:10 與 104(grep 實證);意見方向不變(8.1 e2e 增 latch_trap
@* 正樣回歸錨)。

R1-2 **[medium] 指紋缺檔案維,跨檔同名模組誤殺(§2.1/§8.3 V5-③)**
——已改。實證(grep):`crates/spec-i2c/rtl/i2c_master.v:25` 與
`corpus/e2e/59_i2c_master/i2c_master.v:27` 均為 `module i2c_master (`。
修:鍵串入 source 維(D13),V5-③ 斷言同步。

R1-3 **[medium] carryadd.v 在 V1 單檔≥3 必敗(§8.3/§2.2)**——已改。
實證(本輪全文實讀 carryadd.v):零 always,generate-for 內 GenIf
兩臂各 2 條 assign(全為 & | ^ 位運算)+`assign y[i]=Y`。修:
arithmetic 提取源放寬為「rhs 含任意 Binary(含位運算/邏輯)」
(§2.2 表+§4.2),S2 增 carryadd ≥3 條回歸錨;類別詞表不動。

R1-4 **[medium] server 源語言與構建路徑無著落(§6/§7.1/§8.1)**——
已改,擇手寫 JS(ESM .mjs,源=產物,零編譯步)=新決策 D11;dist
分工重寫(dist/wasm=構建產物同步;server.mjs=手寫源入庫);類型
契約面=手寫 server.d.ts。

R1-5 **[medium] opts_json 需 JSON 讀取器,方案只立法了寫出器(§4.1/
§5)**——已改,擇 opts 標量化=新決策 D12(extract_rules 收
`max_rules_per_module: usize` 標量,scan_gaps 無 opts);備選
`analyzer_core::proto::json::Json::parse` 已實證在場且 pub
(proto/json.rs:87 `pub fn parse`)但不採;§4.1 頭注明文「只寫不讀」。

R1-6 **[medium] 陳舊檢查 find 漏傳遞依賴(§8.1)**——已改。find 清單
補全 analyzer-core/vl-syntax/ry-ir/rcs-diag 四目錄(沿 regress.sh:199
既證形),並明文「漏列=雙閘驗陳舊 wasm,D8 謊言」。

R1-7 **[low] V2/V5-④ 偵測力近恆真(§8.3)**——已改,不刪閾值但如實
定位:§8.3 增「V2/V5-④ 定位聲明」段(冗餘防線=守生成端鎖①的回歸
探針;主判據=V5-①+V4),R3 映射行同步;報告與 SKILL 須照此定位。

R1-8 **[low] fingerprint="sha1:…" 無實現依託(§2.1)**——已改。實證
(rg):crates/ 內 sha1 僅 ry-difftest 測試字符串誤配、sha256 零命中。
修:拼接鍵串(D13),去重=字符串精確比對。

R1-9 **[low] V1 上限 400 只標註不 FAIL,無執法力(§8.3)**——已改:
truncated 非空→該檔 FAIL(exit 紅);模組內上限值降為提取器參數
(默認 128),防注水語義獲得閘性。

R1-10 **[low] 多行 snippet 與「逐位元組相等」缺拼接規則(§8.3 V4/
§2.1)**——已改:§2.1 增 snippet 拼接規則(行原文按序 `\n` 連接,
保留縮進,末行不帶終結換行;單行=去換行原文),V4 行引用同一定義。

R1-11 **[low] rtl_analyze {path?|source!} oneOf 不可表達(§6.1)**——
已改:inputSchema 保持平鋪,語義裁決入工具(皆給 path 贏/皆缺
isError),錯誤語義段+§8.1 e2e 負樣斷言補「雙缺參→isError」=自洽
缺口有閘可抓。

R1-12 **[low] param 提取源 vs 鎖①張力未裁決(§2.2/§2.3)**——已改:
§2.3 鎖① 增 param 側門裁決句(param 僅由 generate 行為構造 spawning;
宣告範圍內參數引用不 spawn),§2.2 param 行同步。

同輪文檔一致性改動:§2.2 param/arithmetic 行、§2.3 鎖①、§3.1 G1/G5、
§4.1 頭注+API+DEFAULT_MAX_RULES、§4.2 行程分類律/arithmetic/骨架、
§5 簽名、§6 頭段+§6.1+錯誤語義、§7.1 樹+dist 分工、§8.1 ②③、
§8.3 V1/V2/V4/V5 行+定位聲明+R3 映射、§9 D11..D13、§10 S2/S5、
§12 R9。high 條(R1-1)完成後才允許進 S2/S3。

---

## 14. 複審修訂 R1(2026-11-03 換人複審 6 條意見逐條消化;每條附實證)

R2-1 **[high] 四工具與 wasm 導出面不閉合(§4.1/§5/§6.1)**——已改,
擇「補 analyze 導出」路線(非「rtlil 回 isError」降級路線):核心增
`AnalysisSummary`+`analyze_summary()`(analyzer-core Analysis 公開
查詢直讀組合:modules()/instance_edges()/diagnostics,零新語義——
modules 欄位即 ModuleInfo view.rs:8-28 直投,結構投影=分析器立檔
契約內的合法面)+`analysis_json()`;wasm 導出面 4→**7 函數**(增
rtl_analyze_{sv,rtlil}、rtl_scan_gaps_rtlil),§6.1 四工具資料源逐一
閉合(rtl_analyze←analyze;rtl_scan_gaps lang:rtlil←rtl_scan_gaps_rtlil;
batch 對 .il 檔 analyze/gap 均有源);§1 架構圖、§4.2 切分(src/summary.rs)、
§10 S4(導出面+analyze 入 parity)同步。S5 不再撞牆。

R2-2 **[medium] AGENTS 節號誤引(§3.1 G6/G7)**——已改:G6「依
§10.7/ADR-0001 宣告序契約」→「依 AGENTS §2 調度確定性契約
(process 序=宣告序,ADR-0001)」;G7「對齊 AGENTS §10.7 競爭紀律」→
「對齊 AGENTS §10.5 競爭免疫紀律的靜態面」(§10.7 是三面 facade 紀律,
誤引會隨 S1 自驗把錯號帶進 SKILL/AGENTS 立法)。

R2-3 **[medium] D6/R5 殘留「上限 400」(§9/§12)**——已改:兩處同步
為「模組內 max_rules_per_module 默認 128+truncated 非空→FAIL」;
「檔級 400」在 API(Rulebook 無檔級上限欄位)與 V1 判據中均無對應
機制,係 R1-9 修訂漏改面,已清(R1 同輪清單確實漏了這兩處)。

R2-4 **[medium] D9「regress.sh 零改動」與 §10.7 張力**——已改,撤
加碼解讀:實讀 regress.sh 確認 469/479 斷言在 :318、456/456 在 :348、
analyzer smoke 雙守衛塊(ce29bfc)為既證先例形;修後 D9=S7 同輪以
雙守衛降級形收編(守衛=crates/rtl-spec-wasm/pkg 在場+bun 在場;在場
時兩插件閘 exit≠0 即 fail=1 整閘紅),基線斷言數字逐字不動;§8.5、
§11 非目標 2、§9 表①層、§10 S7 同步;ADR-0012 記錄此收編。

R2-5 **[low] 「沿 §4-4 掛死形 fixture」編號不清(§10 S3)**——已改:
改引「AGENTS §4 第 2 條(每個恢復點配一個會觸發的掛死形 fixture)」。

R2-6 **[low] TB 跳過注記塞 parse_errors=語義過載(§12 R7b/§4.1)**——
已改:Rulebook/GapReport 增獨立 `skipped: Vec<SkippedNote>
{file, reason: TbFile|Unsupported}` 欄,parse_errors **僅**收解析
錯誤;§4.2 TB 行、§8.3 V1(skipped 檔不計單檔≥3)/V6(成功呼叫含
skipped 注記形,報告單列)同步。13 檔實測零 `$finish`(本輪 grep
exit=1 實證)當前不觸發——此修是未來地雷拆除(引入 TB 檔不再誤紅
V1/V6)。

同輪文檔一致性改動:§1 架構圖、§3.1 G6/G7、§4.1(AnalysisSummary/
skipped/analyze_summary/analysis_json)、§4.2(TB 行+src/summary.rs)、
§5 導出面 7 函數、§8.3 V1/V6、§8.5、§9 D6/D9+表①層、§10 S3/S4/S7、
§11 非目標 2、§12 R5/R7b。high 條(R2-1)閉合後才允許進 S4/S5。

---

## 15. 實作記錄(日誌 SSOT;每片一行:切片號/改動摘要/自驗結果)

- **S1 立法落盤(2026-11-03)**:ADR-0012 全文(docs/adr/0012-rtl-to-spec-plugin.md,
  D1..D13+機械閘+非目標)+PLAN.md P3.8 弧行+里程碑表 RTL1..RTL5 行(狀態空列)+
  SKILL「rtl-to-spec 增補」節骨架(九條:三面一路形/反 doc-dump 三鎖/陳述律/
  RTLIL 結構證律/V1..V6 指針/fixtures 授權律/JSON 鍵序契約/破口雙 fixture 律/
  語料唯讀+dist 重建律+regress 執法面)+REGISTRY 預登記(Crates 表 rtl-spec/
  rtl-spec-wasm 兩行+閘表兩插件閘行)。自驗:rg 交叉引用一致性實跑(本檔/ADR/
  PLAN/SKILL/REGISTRY 五檔互相指到的 run-mcp-e2e.sh/run-rtl-validation.sh/
  ADR-0012/RTL2SPEC-PLUGIN.md/rtl-to-spec 增補/RTL1..RTL5/P3.8 七鍵全部命中,
  引用閉合)——通過。
- **S2 rtl-spec 核心(2026-11-03)**:crates/rtl-spec(schema.rs 詞表+AnalysisSummary/
  ModuleSummary+expr.rs 最小 printer 優先級還原+summary.rs analyze_summary 兩形同源+
  json.rs/md.rs+lib.rs 入口)+SV 提取器五基類(reset/fsm/timing/arithmetic/counter;
  行程分類律 R1-1 三肢判準實裝;snippet 行號通路=pp 主檔 FileId(0) 註冊原始文本,
  巨集 span file≠0 → 無證即丟規則,誠實降級)。**偏差一條(記錄)**:JSON 編碼復用
  analyzer_core::proto::json::Json 只寫不讀(R1-5 拒的是 ::parse;寫出器與 report.rs
  同形同源,單一 JSON 寫出器原則,轉義律有 codec 向量背書)——§4.1「手寫 JSON」讀作
  「手寫=repo 內零 serde」非「本 crate 重寫一份轉義」。自驗:cargo test -p rtl-spec
  14 pass(五類分佈精確數/指紋去重/截斷/鍵序 byte-exact/敵意轉義/V4 生成端自對拍/
  carryadd ≥3 回歸錨+fiedler-cooley);cargo test --workspace 397 pass 0 fail,
  analyzer-core 34+1、analyzer-server 15 零擾動。——通過。
- **S3 破口掃描器(2026-11-03)**:src/sv/gaps.rs DriverMap+G1..G7 全類+confidence
  分級(certain/heuristic);v1 保守化三條防誤報(實例連接豁免 G3/聲明初始化=驅動源/
  G6 按源計數同行程位選不誤報;G4 觸發訊號判準=首個 pos/neg 邊,異步復位邊不在其列)。
  每類一觸發形+一零誤報形 fixture(gaps_positive.v 七模組/gaps_negative.v 七模組,
  正樣逐模組斷言類集、負樣全檔零破口)。首跑抓二真 bug:①gaps 側復位判定誤把異步
  復位邊當觸發訊號(良構全復位設計誤報 G4)②FSM default 臂狀態賦值落入 timing 假規則
  (CaseLike 增 default 欄+consumed 排除)③FSM when 用 walk 後已彈空的外層 conds
  (改用 FlatAssign.conds)。自驗:cargo test -p rtl-spec 14 pass。——通過。
- **S4 RTLIL 子集+wasm 面(2026-11-03)**:src/rtlil.rs(reset=1-bit 選擇子+常量臂/
  fsm=選擇子恰為 update 寄存器的 switch 顯式臂/timing=pos-neg sync update;破口
  G1=switch 無 default 臂[RTLIL 保持值語義=certain]+G5=connect 自環;**v1 子集
  契約**:其餘類別與 gap 類在 RTLIL 形不產[oracle 臨時名遮蔽自身引用],不硬湊;
  evidence=結構路徑 D2 形,line=0/snippet="")。測試 fixture=oracle proc 後
  -readable 正典形自著(臨時 wire 連 `[7:0]` 括號入名+必須宣告,corpus case.il
  對讀實證)。crates/rtl-spec-wasm(7 導出,零 js_sys,錯誤內嵌 {"error"})+
  examples/emit.rs native 發射器+smoke.mjs。自驗:cargo test -p rtl-spec 16 pass
  (sv 14+rtlil 2);cargo test -p rtl-spec-wasm 2 pass;SMOKE PASS [bun] 與
  SMOKE PASS [node] 雙運行時各 15 斷言——**6 路 wasm↔native byte parity 全等
  (analyze/rules/gaps × sv/rtlil),R1 兌現=真雙運行時非 bun-only**。——通過。
- **S5 MCP server+插件包(2026-11-03)**:plugin/rtl2spec/(.zcode-plugin/plugin.json
  manifest+skills×3+dist/server.mjs 手寫純 JS D11+dist/server.d.ts 類型契約面+
  dist/md.mjs 渲染共享件[**dist 樹偏差一檔**,server/check-rulebook/gen-report
  三面共用單一渲染器,記錄]+scripts/{validate-manifest,mcp-client}.mjs+fixtures×3
  [latch_trap.v 正樣=@* 形分類律 R1-1 端到端回歸錨]+run-mcp-e2e.sh)。
  自驗:bash plugin/rtl2spec/run-mcp-e2e.sh exit 0——回環 11 斷言全過
  (initialize 回顧/list 恰 4 工具/extract rules≥3+markdown 在場/gaps latch@* 正樣
  +test_plan 映射/負樣三形:缺檔 isError+雙缺 isError+未知名 -32601/ping)+
  manifest 校驗 PASS 含 wasm sha256。——通過。
- **S6 真實 RTL 驗證閘(2026-11-03)**:scripts/check-rulebook.mjs(裝載 dist 同一
  wasm D8;**獨立重算**:V4 checker 重讀源檔逐位元組對拍/V5-①②④ 檔級/V1 檔級+
  truncated 執法)+scripts/gen-report.mjs(全集 V1-total/V2-total/V3/V5-③+V6 覆蓋,
  validation-report.md 機械生成 §8.4 形)+run-rtl-validation.sh(守衛/同步/逐檔/
  匯總;語料唯讀,寫只落 plugin/rtl2spec/build/+)。首跑抓一真誤報:`assign {Cout,S}=…`
  拼接 LHS 不進驅動圖→multiplier.v FullAdder 誤報 G3×2(修:lvalue_bases Concat
  展開;誤報形進單測釘死 concat_lhs_assign_is_a_driver_no_undriven_gap)+
  generate.v 單字符標識符被 len≥2 過濾誤殺 V5-④(改詞邊界無長度限定,契約原形)。
  自驗:bash plugin/rtl2spec/run-rtl-validation.sh exit 0——V1-total 325≥50/
  V2-total 100%/V3 5 類≥4/V5-③ 325/325 零重複/V6 13/13;validation-report.md
  116 行含閘契約表+license 表+逐檔表(破口總數 48,抽查 memtest09 G6=真雙進程
  寫同基底可構造碰撞)+規則書全文樣本×3(i2c/carryadd/acc_pipe)。——通過。
- **S7 評審與總閘(2026-11-03)**:①定性評審=R5 探針題×3×代表檔(acc_pipe/
  carryadd/i2c_master,先只讀規則書作答後粗讀 RTL 對照,答案引用 rule id)+
  R2 散文感抽查(模板機械生成確認;i2c 深層 else-if 條件鏈=忠實噪音,接受)
  +覆蓋面誠實注記(Concat/PartSelect rhs 不產 relation 規則=三鎖取捨面);
  批注落 plugin/rtl2spec/review-notes.md(gen-report 機械內聯進報告評審員區,
  再生成不丟);**provenance 注明=實作會話內評審非獨立第三人**,獨立複審可
  重跑兩閘後追加。②四層立法終落:AGENTS §10.10(rtl-to-spec 紀律六條)+
  OKR KR1.11+REGISTRY 計數刷新(rtl-spec 17/rtl-spec-wasm 2+parity 證據)+
  PLAN RTL 行 🔶+SKILL 增補節(S1 骨架即正式條文,本輪核對無改)。③regress.sh
  雙守衛插件塊收編(D9:守衛=pkg 在場+bun 在場;基線 469/479+456/456 逐字
  未動,rg 核對 6 處原樣)。④總閘四件套實測:cargo test --workspace
  **402 pass 0 fail**;bash scripts/regress.sh **REGRESSION GATE PASS exit 0**
  (含新塊 rtl2spec mcp-e2e: PASS+rtl2spec rtl-validation: PASS,analyzer
  smoke 等既有閘零擾動);run-mcp-e2e.sh exit 0;run-rtl-validation.sh exit 0
  (V1..V6 全過,Σrules=325)。——通過。
- **收平輪(2026-11-03,終審/獨立評審 5 medium 實質項逐條消化;R3 編號)**:
  ①G1 latch 誤報 3/4=subbytes.v G1-002/003/004 標 certain(源證 61/62 行
  塊首默認賦值+54-57 行無條件賦值,對立路徑均有值):gaps.rs `check_paths`
  增**頂層無條件賦值對沖集**(top_level_defaults:首層 Block 直屬賦值的
  LHS 基底,conds 空=頂層語義;集內訊號 If/Case 兩報告點豁免)——實測
  subbytes 破口 4→1(G1-001 data_reg_128 真陽性保留 certain);fixture
  矩陣=gaps_negative.v `latch_default_ok`(默認賦值形零命中)+gaps_positive.v
  `latch_default_matrix`(同行程 x 默認覆蓋不報/y 真 latch 仍報 certain,
  message 斷言不含 x);confidence 鏈復核=剩餘報告=無默認+分支不對稱=
  Verilog 語義下結構可證,certain 維持(§3.1 G1 行+SKILL 同步)。
  ②relation 適用域=`CheckPayload::Relation` 增 `when` 條件維(分支域轉寫
  攜 conds,全域 ContAssign=None;when 入 fingerprint when 維——取小改
  =condition 字段路線,§2.4 行同步;**備選「降 confidence」不採**:Rule
  無 confidence 欄,qualifier 語義不合)——json.rs/md.rs/dist md.mjs/
  check-rulebook V5-②(when 在場必非空串)同輪;測試
  `relation_rule_carries_branch_domain_when`;實測 R007 轉寫=
  「當 !(state) 時,next_state ≡ state + 'sd1(逐拍組合)」,指紋
  `…|next_state|!(state)|state + 'sd1`。「rhs 變量/常量拷貝不產
  relation」覆蓋缺口入 review-notes 誠實注記第 3 條(報告內聯)。
  ③gen-report.mjs 實測列改 checker failures 推導(failure.detail 首詞=
  規則 id 去重計數;V4/V5 兩列)+行數口徑=真實 `\n` 計數(與 wc -l 同源;
  實錄 lines 元素 97 vs 檔案 1755,修後日誌 1745==wc -l 實測 1745)+
  **self-consistency 斷言**(判定 FAIL 而實測列無「違規」表達=failCheck
  紅,守實測列硬編碼回歸)+V6 行 skipped 數改實測推導。④fingerprint
  source 維釘死=「傳入 extract_rules 的 name 逐字」(verdict 實證:
  閘傳 `corpus/e2e/59_i2c_master/i2c_master.v` 原樣入鍵;check-rulebook
  :27 既有注釋同一形)——§2.1 欄位契約新條+D13 條;示例值改驗證閘實錄
  R001 指紋(舊例 `59_i2c_master.v|…` 與任何真實檔名不對應,basename
  慣例禁令入文)。⑤§6.1 錯誤語義段對帳代碼現實:刪「RTLIL 首錯硬錯/
  未支援 lang」(server.mjs 無此二路徑;langOf 缺省按副檔名、其餘 sv);
  **rtl_scan_gaps×lang=rtlil 裁決句=非工具級錯誤**,正常回 v1 子集面
  (僅 G1/G5,空集非錯)——§5 七導出機械對帳(rtl-spec-wasm lib.rs
  grep 逐一相等,零改;server.mjs 四工具 sv+rtlil 全接線 grep 實證)。
  自驗實測:cargo test -p rtl-spec **18 pass**(sv 16+rtlil 2,新增
  relation 適用域測試);cargo test --workspace **403 pass 0 fail**
  (S7 基線 402+新增 1,前弧零擾動);cargo build 零 warning;
  run-mcp-e2e.sh exit 0(loopback 七斷言+manifest 43 PASS 行);
  run-rtl-validation.sh exit 0(Σrules=325/V2 100%/V3 5 類/指紋
  325/325/V6 13/13;破口總數 48→39,validation-report.md 重生成
  1745 行含新注記)。——通過。

- **S8 確定性與終止性修復(2026-11-04 實作落地;規劃=§16.6 S8)**:
  F1a G4 無復位臂有序取樣+F1b G1 If/Case 臂迭代有序化(gaps.rs 三處,
  收集入 Vec+sort 後取首/遍歷;集合邏輯不變)+F2 bfs_path visited 集
  (真根因=bfs 死循環非規模,REGISTRY 11-03 行勘正;守衛保留防禦)+
  gap_hang_repro.v 入 plugin/fixtures+probe-determinism.mjs(37 檔×12 跑
  +調用序擾動)+pinned-files.list 釘選清單。自驗:①probe 37/37 PASS
  (flapper 12 檔清零,2026-11-04 復跑實測);②scopes.v wasm 腿零 trap
  (eval 全集 trap=0);③cargo test -p rtl-spec 全綠;④run-rtl-validation
  exit 0(validation-report.md 重生成 3 行 diff 對帳=wasm sha 戳+G4
  suggested_test 兩處)。——通過(本行為後續輪補記,數字以當場復跑為準)。
- **S9 golden 體系落地(2026-11-04)**:run-golden-regression.sh(五段形
  沿 run-rtl-validation)+scripts/golden-check.mjs(兩階段 --record:全檔
  每模式 3 跑自檢→統一寫盤+PROVENANCE 全量重寫;默認=產出 build/
  golden-current+逐份 byte-diff[首個差異鍵路徑]+PROVENANCE 表核對);
  37 檔×2=74 份首錄入 golden/(鏡像源路徑:corpus-external/<X> 剥根,
  corpus|crates 全路徑);PROVENANCE 兩域拆分(正式樹 vs plugin-fixtures
  demo 域,record-golden.mjs 改指向)。自驗:①錄後對帳=74 份全新庫,
  抽查規則數與 §16.1.1 表逐字一致(operators 85/memory 59/sincos 46/
  subbytes 7/acc_pipe 7);②比對模式 exit 0(74/74+PROVENANCE 核對
  PASS);③紅路雙向=golden 篡一字節(latch→Latch)必紅(DIFF+
  PROV-DIFF 雙層,exit 1)→還原綠;提取器 message 改一字(可能鎖存→
  可能鎖存X)閘自動 rebuild 必紅(僅有 G1 釘選檔 subbytes/loops 2 檔
  DIFF)→還原綠。——通過。
- **S10 破口 fixture 矩陣閘(2026-11-04)**:scripts/gap-fixture-check.mjs
  (條級 (module,class,confidence) multiset 集合相等+msgSubs 關鍵詞
  抽點+selfcheck 判別力自證+終止性守衛行)+run-gap-fixtures.sh+
  fixtures/gap_missing_reset_no_arm.v(零共報設計:恰 1 條 G4 certain,
  suggested_test 逐字含 ra 不含 rb);crates 單測層 G4 無復位臂確定名
  斷言=S8 輪已在(sv_rules.rs gap_missing_reset_no_arm_
  suggested_test_is_deterministic_name)。實錄勘正:矩陣初稿 G5×1/G6×2/
  G7×3 為**類級 dedup**語義,條級實跑全集=G5×2(雙向環)/G6×3(雙行程
  各 G4)/G7×4=14 條(§16.2 表為類級斷言仍成立;閘以條級 multiset,
  兩形不矛盾,R16-2 勘誤的延伸)。自驗:①閘 exit 0;②selfcheck 紅路
  PASS;③G4 新正樣 suggested_test 逐字名(msgSubs 含 ra 不含 rb)
  PASS+12 跑守衛 PASS。——通過。
- **S11 regress 收編+立法四層(2026-11-04)**:regress.sh 既有 rtl2spec
  塊內追加 run-golden-regression.sh+run-gap-fixtures.sh 兩直調(守衛
  不變/SKIP 行不動/基線斷言 469/479+456/456 逐字不動);四層同輪:
  SKILL「golden 回歸與評測增補」條/AGENTS 10.10「回歸與真實評測執法
  面」條/REGISTRY(Crates rtl-spec 行 26 測+閘表三行)/ADR-0012 機械
  閘序列 4..6 條翻現役+修訂記錄行。自驗:見總閘輪(regress exit 0 全
  塊 PASS)。——通過。
- **S12 評測擴面+報告+複評(2026-11-04)**:scripts/eval-all.mjs(單
  bun 進程 353 檔;L1=93 全提取+破口/L2=260 容錯;per-file 三檔分類;
  trap=try/catch 計數)+scripts/gen-eval-report.mjs(實測列 verdict
  推導+自洽斷言,R3-③ 先例)+run-realworld-eval.sh+docs/RTL2SPEC-EVAL.md
  首錄+eval-review-notes.md(R5 五檔探針×3,答案引用 rule id+粗讀 RTL
  對照)。實錄:eval-all L2 清單初版只濾 .v 致 L2=0(EVAL_EXIT 0 假綠),
  被分層統計行「L1=93 L2=0」當場抓出→修 .sv——**清單短收不攔的教訓:
  分層統計數字必人讀對帳**。首錄實測:37 design-rules/117 sparse/
  193 skip-perr/6 skip-tb/trap=0/指紋 672 unique/80ms;報告 509 行。
  自驗:①閘 exit 0(353 檔全終止);②自洽斷言 PASS;③R5 五檔探針
  答案引用 rule id 在場。——通過。
- **S13..S15 補記(§17.8 承諾的本輪清償;數字以 §17.3/§17.8 實錄為準)**:
  S13=G8 dead_branch 全鏈路(26 測/零 warning/釘選集零觸發/probe
  37/37+2/2/parity×2/demo golden 2 份錄製);S14=版本 0.2.0 四點一致
  +EXPECTED_ABI 裝載校驗+run-plugin-check.sh 12 斷言+紅路兩路;
  S15=§17 章+SKILL 拓展與版本增補條+skills 三處+CHANGELOG。——通過
  (前輪實作,本行為日誌 SSOT 補記)。

---

## 16. 回歸與真實評測弧(2026-11-03 立項起草;開弧勘察報告[ask 材料,未入庫]+本弧復驗全實跑取證;開弧對抗審修訂見 §16.9;ADR-0012 增訂 D14..D16)

目標(使用者原話轉寫):**真實評測,建立有效的 regression**——①提取器任何行為
漂移當場紅(rulebook golden 逐位元組回歸+破口種洞雙向 fixture 矩陣,兩閘入
regress.sh 執法面);②真實評測從 13 檔驗證集擴到 ≥30 檔 in-git 真實 RTL,
per-file 質量報告機械生成,R1..R5 在擴面集抽樣複評;③全程遵守 golden 禁
手寫/re-record=語義事件/計數一律實測/GPL 派生不入庫紀律。

### 16.0 前置事實:非確定性與終止性破洞(錄 golden 前必修;S8 前置)

**F1 非確定取樣清單(確定性破洞;三處,同輪修;開弧對抗審 R16-1 修訂擴面)**:
- **F1a G4 無復位臂取樣**:`scan_missing_reset` 無復位臂分支
  `regs.iter().next()`(crates/rtl-spec/src/sv/gaps.rs:670)從
  `HashSet<String>`(lhs_set,gaps.rs:489-496)隨機取樣寄存器名,抖
  suggested_test 欄。
- **F1b G1 路徑敏感報告序(R16-1 補登記)**:`check_paths` 兩處 HashSet
  迭代序直接進輸出 items 序與 id↔內容綁定——If 臂
  `for sig in l.iter().chain(r.iter())`(gaps.rs:402,l/r 均 lhs_set)
  與 Case 臂 `for sig in lhs_set(s)`(gaps.rs:439)。初稿「gaps 的
  id/序穩定」為假命題,已勘誤(16.1.5)。id↔內容互換逐字實證(本弧
  wasm 探針,同進程 6 跑):run2 中 G1-aes-002/003 內容
  data_var↔round_key_var 互換而 id 串不變(其餘 5 跑 round_key_var
  在前)——快照若錄,任何釘選集增刪/調用序變更即大面積 golden churn
  「期望變了而提取器沒變」。
- **按腿症狀(本弧雙腿實測,命令可重跑)**:native 腿跨進程真抖——
  loops.v(釘選集內 G1 檔,**零 G4 觸發**)5 跑得 2 種 sha256
  (bc3bc6fe×3/d353d0d6×2;命令=`for i in 1 2 3 4 5; do
  ./target/debug/examples/emit sv gaps probe
  corpus-external/yosys-simple/loops.v | shasum -a 256; done`);
  fiedler-cooley.v native 5 跑 4 種。wasm32 腿 RandomState=調用流
  位置函數:跨進程首跑 byte 一致(fsm.v 4 個獨立 bun 進程首跑全
  bc8e3dc7——**bug 隱蔽面**,跨進程測試無判別力)、**同進程重複調用
  抖**(fsm.v 同進程 12 跑 5 種 hash)。開弧 flapper 普查(對抗審
  8-20 跑/檔)=釘選集 **12/37 檔**多於 1 種 hash;本弧 wasm 腿同進程
  12 跑全量復驗 **12/12 命中**(hash 種數:memory 9/task_func 7/
  fsm 5/acc_pipe 4/add3 3/always03 3/generate 2/forloops 2/mem_arst
  2/i2c_master_tests 2/loops 2/fiedler-cooley[native 4])。smoke.mjs
  parity 只蓋 gaps_positive.v(單發現面),flapper 檔無 parity 兜底。
- **開弧初登記勘誤(誠實注記)**:初稿引用的三個 fiedler-cooley
  sha256(78d135de…/8fbd3b77…/e310fc9a…)為開弧規劃輪 native emit
  3 跑快照,觀測腿未注明且**在任何固定 build 上不可復現**(hash 集合
  本身逐進程隨機;本弧同命令重跑得另一集合 11077d43/65060307/
  910cbd9b/c696d571)——具體 hash 不作證據,證據只留「N 跑>K 種」
  可重跑事實+上述命令。
- **修法(S8,三處同輪)**:G4 無復位臂分支+G1 If/Case 臂迭代全部改
  **有序迭代**(收集入 Vec+sort 後取首/遍歷;gaps.rs:591-593 G3 與
  :820-821 G6 的 names.sort() 先例);單測釘死「同檔進程內 N 跑
  byte-identical」。驗收=16.1.5 契約(全 37 檔×12 跑+調用序擾動)。

**F2 G5 掃描終止性破洞(按腿症狀不同)**:`corpus-external/yosys-simple/
scopes.v` gaps 模式——**native 腿=掛死**(開弧複驗:4s 觀測窗口未終止,
kill 後遺留孤兒 emit 子進程需手動清;深層棧=開弧勘察 sample 實證[未入庫]
scan_comb_loops→bfs_path→HashMap::get,gaps.rs:692/746/775);**wasm
腿=秒級 panic trap**(本弧實證:rtl_scan_gaps_sv 探針拋
`RuntimeError: Unreachable`——見 16.4 終止性防線,R16-3)。修法(S8):
①G5 驅動圖**規模守衛**(節點/邊數上限,超限=如實「守衛截斷」注記後
終止,禁靜默截斷、禁無限爬);②最小復現形(組合 always+命名塊內 local
reg+雙信號環)入 plugin/rtl2spec/fixtures/gap_hang_repro.v(/tmp 易失,
AGENTS §9)。驗收=16.6 S8②(按腿)。

兩修皆屬**語義事件**:S8 落地後觸發 golden 首錄(16.1)與
validation-report.md 破口樣本區重生成(收平輪先例,數字對帳)。
錄製順序鐵則:**F1a+F1b 修復且 37 檔 flapper 清零前,禁錄任何 gaps
golden**(錄即抖,或錄入調用流位置耦合快照——期望變了而提取器沒變,
按 16.1.4 必須停弧)。

### 16.1 golden 體系(閘=`bash plugin/rtl2spec/run-golden-regression.sh`)

**語義定位(D14,先決澄清)**:golden ≠ oracle 期望。spec 提取無 oracle
(§11.8 不變,「禁虛構 golden」繼續成立);本弧 golden=**提取器自身輸出的
快照**(self-recorded snapshot),用途=行為漂移檢測(任何提取器改動撞
快照即紅,強制「改了什麼必須能說清」),錄製來源=真實運行輸出,禁手寫
——與 AGENTS「期望一律從真實運行錄製」同構,只是參照物從 oracle 換成
自身上一版輸出。golden 紅≠錯,=語義變了;處置只有兩條:能解釋→顯式
re-record(16.1.4);不能解釋→停,排查提取器。

**16.1.1 釘選語料集(37 檔;規則數=本弧 emit sv rules 實測,Σ=630 條)**

| 組 | 檔數 | 清單(規則數實測) | license | 入選依據 |
|---|---|---|---|---|
| yosys-simple 頭部 | 5 | operators 85/memory 59/constmuldivmod 46/sincos 46/partsel 39 | ISC(COPYING 隨附) | 九類規則主力+G2/G3/G4/G6 觸發面(memory=G3+G6,sincos=counter+reset+G2+G4) |
| yosys-simple 中部 | 12 | fsm 18/dff_different_styles 17/generate 15/process 13/fiedler-cooley 11/multiplier 10/i2c_master_tests 9/undef_eqx_nex 8/always03 7/forloops 7/realexpr 7/subbytes 7 | 同上 | fsm 類面+generate/case 臂+G1(subbytes,默認對沖正樣在庫同形) |
| yosys-simple 尾部 | 15 | signedexpr 6/task_func 6/vloghammer 6/wreduce 6/muxtree 6/dff_init 5/mem2reg 5/macros 5/aes_kexp128 4/carryadd 4/loops 4[G1]/attrib04_net_var 4/omsp_dbg_uart 4/mem_arst 4/forgen02 3[G5] | 同上 | 低規則數下限面(≥3 線)+G1(loops)/G5 組合環(forgen02) |
| repo 自有 | 5 | corpus/e2e/59 i2c_master 69/crates/spec-i2c/rtl i2c_master 69/58 acc_pipe 7[G4×3]/57 add3 6/corpus/fulladd/fulladd 3 | MIT(spec-i2c=乾淨室,OpenCores 唯讀啟發零拷貝) | protocol/handshake 重鎮(雙路徑同名檔,fingerprint source 維同理**按完整路徑釘**)+xls 面+G4 |

排除記錄:scopes.v(F2 修復前雙面排除;修後入評測面 L1,是否入 golden
由修後重評);const_branch_finish(TB skip);zero-rules 34 檔(無斷言
價值,入評測面 L1);corpus/counter8/counter.v(2 規則<3,只入評測面;repo 另有 corpus/oracle-fixtures/counter.v 同名檔,find 實證——路徑釘全名,與 fingerprint source 維全路徑紀律同款,R16-7);sv-tests 260 檔
(parse_err 192+has_rules 僅 8 檔 1 條;開弧 fitness 實掃[未入庫],
檔數 260=本弧 `git ls-files 'corpus-external/sv-tests/*.sv' | wc -l`
復驗——只配容錯面 L2,不入 golden);**sv-elab 不在庫**(git ls-files=0 本弧實測);**zipcpu
GPL 永不入**(vendor-reference 唯讀 gitignored)。

**16.1.2 golden 產物形態**:`plugin/rtl2spec/golden/` 內鏡像源路徑
(目錄層=來源樹,如 `golden/yosys-simple/sincos.rules.json`+
`sincos.gaps.json`),37 檔×2 模式=**74 份 JSON**,逐檔逐位元組比對。
rules 側=Rulebook 全量 JSON(含 source/parse_errors/skipped/truncated
欄——截斷與跳過行為同被釘);gaps 側=GapReport 全量 JSON(schema.rs:430-436,
含 suggested_test——16.0 F1 修復後其確定性成立才可入快照)。
`plugin/rtl2spec/golden/PROVENANCE.md`=機械生成:錄製命令/日期/bun 與
wasm sha256/每份 golden 的 sha256 表/re-record 歷史(語義事件日志)。

**授權澄清(防審計誤紅)**:golden JSON 含有界 snippet 片段(證據形,
≤3 行/規則)屬提取產物非原檔拷貝——validation-report.md 內嵌 snippet
在庫已為先例;外部語料**原 .v 檔仍禁入 plugin/**(SKILL fixtures 授權
律不變);PROVENANCE 每行記源檔路徑+license 指針。

**16.1.3 錄製與比對機制(D14:golden 走 wasm 腿=dist 同一 wasm)**

`run-golden-regression.sh` 五段(形沿 run-rtl-validation.sh,其
①守衛②同步③④⑤段:16-19/21-45/47-82/88-101/103-114):

1. 守衛:bun+wasm-pack 在場+37 檔釘選清單在場(缺→exit 2 逐一列出);
2. pkg 陳舊檢查+同步 dist/wasm(6 個傳遞依賴目錄 find -newer,D8 同形
   ——**閘驗出貨物**:比對對象=交付的 dist wasm 裝載面,非平行實現);
3. 逐檔跑 extract_rules+rtl_scan_gaps(wasm;單 bun 進程內循環,
   scripts/golden-check.mjs),產物寫 `build/golden-current/`(gitignored);
4. 逐份 byte-diff vs golden/:diff 清單逐行打印(檔名+首個差異鍵路徑);
5. 任何 diff→exit 1;全等→exit 0+PROVENANCE sha256 表核對。

**選 wasm 腿的理由(記錄)**:①與既有兩閘守衛形一致(pkg+bun 雙守衛
降級形,regress 塊同構);②D8 延續——golden 釘住的是「交付面」行為;
③wasm↔native 語義等價已由 smoke.mjs 6 路 byte parity 釘死(S4 實測),
native 漂移由 parity 閘兜底,兩閘分工不重疊。

**16.1.4 re-record 顯式機制與語義事件紀律**

- 顯式開關:`bash plugin/rtl2spec/run-golden-regression.sh --record`
  (無 flag=純比對,golden/ 樹唯讀)。--record 前置自檢(R16-5 修訂):
  **實現形=進程內重複調用**——同一 bun 進程對抽樣檔各跑 3 次,3 hash
  必須唯一,不過→拒絕錄製 exit 3。理由:跨進程跑在 wasm32 腿**無判別
  力**(RandomState=調用流位置函數,跨進程首跑 byte 一致——F1 未修時
  自檢全綠=假陰性);進程內跑才抓位置耦合抖動。**前提=F1a+F1b 修復且
  全 flapper 清零**(16.0;未清零時進程內自檢抽中 flapper=exit 3 永久
  擋錄、抽不中=漏放,兩向皆失效——自檢有意義以 S8 驗收過為前提)。
- 錄後必讀:`git diff plugin/rtl2spec/golden` 逐案對帳——**期望變了而
  提取器沒變=必須能解釋,否則停**(AGENTS 10.5 re-record=語義事件
  同款;delta 分類:語義修復/缺陷修復/意外,逐檔寫進 PROVENANCE
  re-record 歷史)。
- PROVENANCE.md 只由 --record 機械重寫,禁手改;與 golden/ 同輪提交
  (弧內禁 commit,由 user 定)。
- 新增/移除釘選檔=釘選集變更,屬語義事件:清單變更同輪更新 §16.1.1
  表+PROVENANCE,禁靜默增刪。

**16.1.5 確定性契約(開弧對抗審 R16-1/R16-4 修訂;初稿「gaps 的 id/
class/module/message/序穩定」為假命題,勘誤)**

- 實測現狀(修正):rules 模式穩定(37 檔實跑);gaps 模式**當前非確定**
  ——F1a/F1b 三處 HashSet 迭代序進輸出,flapper=釘選集 12 檔(16.0
  清單);掃描序(gaps.rs:102-108 固定)+G3/G6 names.sort+G5 canonical
  環排序(gaps.rs:750-751)只覆蓋各自類內部,不覆蓋 G1 報告序與 G4
  suggested_test。
- 契約三則(golden 快照合法形):
  ①同輸入→同輸出 byte-exact(進程內任意重複調用);
  ②**輸出禁依賴調用流位置**——同進程第 k 次調用輸出≠首跑輸出=違例
  (golden 禁錄位置耦合快照;反例=16.0 F1b loops.v);
  ③禁 HashSet/HashMap 迭代面進入輸出欄位與 items 序(並行/時間/
  環境隨機同禁)。新代碼入提取路徑須過 16.6 S8① 同款 12 跑制。
- 驗收形(S8;升級自初稿單檔 100 跑):**全 37 檔×12 跑(同進程重複
  調用)逐字不變+調用序擾動檢查**(同進程先跑他檔再跑目標檔,輸出與
  單獨跑一致——抓跨檔位置耦合)。
- 錄製 cwd 無關性:輸入按相對 repo root 傳入(emit name 欄=fingerprint
  source 維逐字,run-rtl-validation.sh 先例),golden 不含機器相關串
  (絕對路徑/主機名;AGENTS 10.8 expected.* 禁機器相關字串同款)。

### 16.2 破口 fixture 矩陣閘(閘=`bash plugin/rtl2spec/run-gap-fixtures.sh`)

**定位**:插件級直跑(不經 cargo test 全量)的 G 類×正負樣雙向矩陣,
斷言**精確到 expected findings**(類/confidence/條數/id 前綴形)。
與 cargo 單測層(sv_rules.rs:328-450:正樣逐類 class/confidence 斷言+
負樣零命中+concat LHS 誤報釘死)的分工:單測=開發期紅綠;本閘=交付面
(dist wasm 裝載)執法+expected findings 全表結構化+掛死守衛——三鎖
精神的閘端複刻。

**expected findings 的性質邊界(與 16.1 禁手寫的劃分,記錄)**:
fixture 本身是自著觸發形(repo MIT,SKILL fixtures 授權律),其
expected findings=**斷言規格**(「這個洞必須報且 confidence=certain」
的設計意圖),手寫合法——同 sv_rules.rs 單測斷言形;golden=運行輸出
**快照**,錄製產生禁手寫。兩者不可混淆:fixtures/gap-expected 斷言表
在 scripts/gap-fixture-check.mjs 內(結構化常量表,注釋指明與
sv_rules.rs 同源),不走錄製。

**矩陣全表(G1..G7 按 gaps.rs 代碼現實;正樣=種洞必報+confidence 契約
成立,負樣=乾淨必靜默)**:

| 類 | confidence | 正樣(種洞形→expected **全集**,含共報) | 負樣(乾淨形→expected) | 現狀 |
|---|---|---|---|---|
| G1 latch | certain | `latch_trap`→[latch]×1 certain;`latch_default_matrix`→[latch]×1 certain(x 對沖不報,message/suggested_test 不含 x) | `latch_ok`/`latch_default_ok`→零命中 | 複用形在 crates fixtures |
| G2 incomplete_case | heuristic | `incomplete_case`→[incomplete_case]×1 heuristic | `case_ok`→零命中 | 複用形在 crates fixtures |
| G3 undriven_read | heuristic | `undriven_read`→[undriven_read]×1 heuristic | `driven_ok`→零命中 | 複用形在 crates fixtures |
| G4 missing_reset | certain | `missing_reset`(有復位臂缺覆蓋)→[missing_reset]×1「不在復位臂 effect 集」形;**新增 `gap_missing_reset_no_arm.v`**(無復位臂)→[missing_reset]×1「N 個寄存器上電值未定義」+suggested_test=**確定寄存器名**(F1a 修後有序取樣釘死,斷言逐字名) | `full_reset`→零命中 | 複用形在 crates fixtures+新增 1 檔在 plugin/fixtures |
| G5 comb_loop | certain | `comb_loop`→[comb_loop]×1;環形=**t1/t2 雙信號環,無自環**(gaps_positive.v:46-52 實讀;初稿「自環+雙信號環」勘誤,R16-2) | `chain_ok`(鏈式無環)→零命中 | 複用形在 crates fixtures |
| G6 multi_driver | certain | `multi_driver`→**共報兩類**=[missing_reset,multi_driver](sv_rules.rs:366 釘死;初稿「G6-001×1」低估勘誤,R16-2) | `word_writes_ok`(多位選同行程=按源計數)→零命中 | 複用形在 crates fixtures |
| G7 blocking_in_seq | certain | `blocking_race`→**共報三類**=[blocking_in_seq,missing_reset,multi_driver](sv_rules.rs:368-370 釘死;初稿「G7-001×1」低估勘誤,R16-2) | `blocking_ok`→零命中 | 複用形在 crates fixtures |
| 終止性守衛 | — | **新增 `gap_hang_repro.v`**(F2 復現形:組合 always+命名塊 local reg+環)→wasm 腿零 trap 且 G5 報告確定(進程內 12 跑逐字不變,R16-3) | — | 新增(plugin/fixtures) |

**fixture 樹兩處(R16-6 釘明路徑)**:複用正負樣形=
`crates/rtl-spec/tests/fixtures/gaps_positive.v`/`gaps_negative.v`
(8+8 模組,sv_rules.rs 單測在用,模組名與上表一一對應——插件閘跨包
唯讀引用,**不搬移不複製**以免雙源漂移);本弧新增 2 檔
(gap_missing_reset_no_arm.v/gap_hang_repro.v)=
`plugin/rtl2spec/fixtures/`(弧約束:新 fixtures 入 plugin 內)。
expected findings 斷言形=**按模組集合相等**(多報少報皆紅),禁
contains 弱斷言——守本節開頭「精確到 expected findings」宣稱(初稿
矩陣低估共報即會逼出弱斷言,R16-2 教訓)。
負樣紅路演練(閘自證):gap-fixture-check.mjs 內建自檢——把某正樣誤配
到負樣斷言必紅(--selfcheck 隱含在每次運行,防斷言表恆真假綠);
crates 單測層同步增 G4 無復位臂確定名斷言(內聯 RTL 字串,不動既有
fixture 檔)。

### 16.3 regress.sh 執法面與四層立法落點

**新塊形態(照既有 S7 塊雙守衛降級形,scripts/regress.sh:368-385 同構)**:
既有 rtl2spec 塊**內部追加**兩直調(守衛不變=pkg 在場+bun 在場;
golden/gap-fixtures 兩閘同走 wasm 腿,守衛語義一致):

```bash
    if bash plugin/rtl2spec/run-golden-regression.sh; then
        echo "rtl2spec golden-regression: PASS"
    else
        echo "GATE FAIL: rtl2spec golden-regression"; fail=1
    fi
    if bash plugin/rtl2spec/run-gap-fixtures.sh; then
        echo "rtl2spec gap-fixtures: PASS"
    else
        echo "GATE FAIL: rtl2spec gap-fixtures"; fail=1
    fi
```

else 分支 SKIP 行改 `rtl2spec gates: SKIP (pkg/bun absent)` 不動;
既有兩閘直調(:373-382)與基線斷言(469/479+456/456)逐字不動;
`--with-record` 腿與 fail 匯總 exit 1(:387-397)不動。**realworld-eval
不收編 regress**(理由:353 檔規模時長+ask 契約邊界只收編 2、3 兩閘;
它進總閘直調序列,見 16.4)。

**失敗語義**:任一新閘 exit≠0→塊內 `GATE FAIL`+`fail=1`→總閘
`REGRESSION GATE: FAIL` exit 1;守衛缺失→SKIP(不靜默失敗也不假綠);
閘自身退出碼表意沿 run-rtl-validation.sh 先例(2=守衛缺,3=構建/自檢
敗,1=判據敗)。

**四層立法同輪落點(缺一不算改規則,AGENTS §10)**:

1. 機械閘:regress.sh 新塊+兩閘 exit≠0 執法(本節);
2. 正典:.agents/skills/eda-dev/SKILL.md「rtl-to-spec 增補」節追加
   「golden 回歸與評測增補」條(golden=自錄快照非 oracle 期望/
   re-record=語義事件必對帳/expected findings=斷言規格與快照的邊界/
   確定性契約/雙守衛降級形收編);
3. 條文:AGENTS.md §10.10 追加「回歸與真實評測執法面」條(三閘形+
   錄製順序鐵則 F1 先於 golden+realworld-eval 總閘直調不進 regress);
4. 證據頁:docs/RTL2SPEC-EVAL.md(16.4,機械生成禁手改)+golden/
   PROVENANCE.md(機械)+validation-report.md(S8 後重生成一次,數字
   對帳)。REGISTRY.md 同輪:閘表加兩行(golden-regression/
   gap-fixtures+realworld-eval),Crates 表 rtl-spec 行閘列補指針。

**ADR-0012 增訂(需要,同輪)**:D14 golden 自錄快照語義與錄製機制;
D15 fixture 矩陣斷言規格層;D16 評測擴面分層(L1 質量層/L2 容錯層)。
V1..V6 閾值**全部不動**(13 檔驗證集閘 run-rtl-validation.sh 原樣);
§11.8「不手寫 golden 期望」讀法=16.1 語義定位,不衝突、同輪在 ADR
注記澄清。

### 16.4 真實評測擴面(閘=`bash plugin/rtl2spec/run-realworld-eval.sh`;報告=docs/RTL2SPEC-EVAL.md 機械生成)

**評測集(D16 分層;總 353 檔全 in-git)**:

- **L1 質量層 93 檔**:yosys-simple 全 87 檔(F2 修復後含 scopes.v;
  parse_err=0/TB skip=1 檔 const_branch_finish——開弧 fitness 實掃
  [未入庫];87=`git ls-files 'corpus-external/yosys-simple/*.v' | wc -l`
  本弧復驗)
  +repo 自有 6 檔(acc_pipe/add3/fulladd/counter.v/e2e-59 i2c/spec-i2c
  i2c)。license:ISC 87(COPYING 隨附)+MIT 6。零 panic+可終止+JSON
  可解析+如實計數,**全部跑提取+破口掃描**。
- **L2 容錯層 260 檔**:corpus-external/sv-tests 全量(ISC,LICENSE
  隨附,SymbiFlow)。只斷言:F2 圖規模守衛+trap 計數(wasm 單進程循環,無計時器
   面;native 腿=subprocess 10s)+零 panic+
  parse_errors 如實入 verdict(193 檔 parse_err=已知事實記錄不 FAIL,
  SV-2009 構造超 parser 閘;開弧 fitness 實掃[未入庫])。
- xls 檔(acc_pipe/add3)已入 L1 repo 組(Apache-2.0 派生在 repo 為
  MIT 載體,e2e 語料先例 §8.3 表);zipcpu=GPL 永不入(重申);sv-elab
  不在庫不入。

**判據鏈(per-file 三檔分類,避免 V1..V6 閾值與擴面衝突——記錄理由:
V1 單檔≥3 是 13 檔精選集的驗收閘,照搬到含 34 檔 zero-rules 的全量集
會把「誠實的稀薄」錯判成 FAIL;擴面集的失敗條件=結構性違例而非產量)**:

- `design-rules`(parse_ok+非 TB+規則數≥3):跑 V4(snippet 逐位元組
  對拍)/V5-①②④/V6 形判據,判定 PASS/FAIL;V1/V2/V3 為報告列
  (規則數/行為占比/類別數)不設單檔 FAIL;
- `design-sparse`(parse_ok+非 TB+規則數<3,含 zero-rules):只跑
  終止性+JSON 可解析+V4 vacuous,判定 OK-sparse(誠實報告,不虛紅);
- `skip-perr`/`skip-tb`:parse_errors>0 / TB 跳過,判定 SKIP(計數
  如實)。
- **閘級硬 FAIL**(任一即整閘紅):任何檔 wasm trap(panic;每例進
  verdict.trap 分類,F2 修後應零——初錄唯一 trap=scopes.v,R16-3)/
  JSON 不可解析/design-rules 檔 V4/V5-①②④ 敗/**全集 V5-③ 指紋
  重複**(去重擴到 L1 全量)/成功呼叫數≠檔數(V6 覆蓋面)。

**腳本形**:run-realworld-eval.sh(守衛 bun+wasm-pack+同步同 16.1.3
段 2)→scripts/eval-all.mjs(單 bun 進程循環 353 檔;verdict 寫
build/eval/,gitignored;判據復用 check-rulebook.mjs 的獨立重算函數
形)→scripts/gen-eval-report.mjs(機械生成 docs/RTL2SPEC-EVAL.md)。
實測列由 verdict 數據推導禁硬編碼+自洽斷言(gen-report.mjs:100-128
先例)。

**終止性防線(開弧對抗審 R16-3 修訂;初稿「單進程循環+每檔 10s 守衛」
自相矛盾,勘誤)**:單 bun 進程內同步 wasm 調用阻塞事件循環,真掛死時
進程內計時器無法觸發——**計時器守衛對掛死無效,僅對同步 throw(可
catch)有效**。按腿明示:wasm 腿終止性防線=**F2 修復本身(圖規模守衛
後理論全終止)+trap 計數**——wasm 掛死形表現為秒級同步 trap(本弧實
證 scopes.v `RuntimeError: Unreachable` 秒級),eval-all 對每檔
catch trap 記 verdict.trap,閘級硬 FAIL=修後 trap 總數>0;「10s 守
衛」僅保留在 **native 腿 subprocess 形**(S8 驗收②的 emit 腿探針等
process 級工具)——subprocess 才對真掛死有效;**禁在單進程 wasm 循
環內宣稱超時守衛**(如需真掛死防線→Worker/subprocess 形,屬實作輪
可選升級,非本弧必需)。

**docs/RTL2SPEC-EVAL.md 形態(機械生成禁手改;入庫=證據頁)**:
日期/命令行/退出碼/工具鏈版本(bun+wasm sha256)→分層統計表(L1/L2
檔數×判定檔數,Σ 行=實測推導)→**per-file 表:檔名|license|行數|
規則數|類別數|破口數(按類)|判定**(93 檔 L1 全列;L2 以分檔統計+
parse_err 清單呈現,不逐檔列表)→類別分布表(九類×檔計數,R4 複評
素材)→評審員區(eval-review-notes.md 機械內聯,同 validation-report
形)。

### 16.5 R1..R5 擴樣複評(擴面集上的抽樣設計)

- **R1 溯源**:V4 逐位元組對拍=L1 全 design-rules 檔機械全量(閘級
  FAIL 條件),擴面即全量複評,無抽樣;
- **R3 非同義反覆**:V5-①③+V5-③ 指紋去重=全集機械;V2/V5-④ 冗餘
  防線照跑(R1-7 定位不變);
- **R4 覆蓋面**:類別分布表(九類×93 檔計數)機械;V3 的 ≥4 類閘值
  仍在 13 檔驗證集執法不動;
- **R5 增值(人工,分層抽樣 5 檔)**:頭部 1 檔=operators(85 條,
  timing/arithmetic 密度面)/中部 2 檔=fsm(18,fsm+counter)+
  multiplier(10,arithmetic+G3 誤報史)/尾部 2 檔=subbytes(7,G1
  對沖正樣+R007 when 適用域先例)+acc_pipe(7,G4×3+Apache 面);
  探針題×3×檔,只讀規則書作答後對照粗讀 RTL,答案引用 rule id——
  落 plugin/rtl2spec/eval-review-notes.md(機械內聯進 EVAL 報告
  評審員區),provenance 注明會話內評審非獨立第三人(S7 先例);
- **R2 散文感**:上 5 檔規則陳述人工讀+抽查(i2c 深層 else-if 條件
  鏈=忠實噪音先例,接受基準)。

### 16.6 切片分解(S8..S12;每片帶自驗與驗收;切片序=依賴序:S8 確定性
先於 S9 錄製)

- **S8 確定性與終止性修復(前置;開弧對抗審 R16-1/R16-3 擴面)**:
  F1 三處有序迭代(F1a G4 gaps.rs:670+F1b G1 gaps.rs:402/439)+單測
  (無復位臂形 suggested_test 逐字名+G1 報告序釘死);F2 G5 圖規模
  守衛+gap_hang_repro.v 入 plugin/fixtures。自驗:①**全 37 檔×12
  跑(同進程重複調用)+調用序擾動(先他檔後目標檔)逐字不變=flapper
  12 檔清零**(對抗審清單逐檔核銷);②scopes.v 按腿:wasm 腿零 trap
  且輸出確定、native 腿 subprocess 10s 守衛內可終止;③cargo test
  -p rtl-spec 全綠(18+新增);④run-rtl-validation.sh exit 0
  (validation-report.md 破口樣本區重生成+Σ 破口數對帳)。
  驗收:①②③④全過。
- **S9 golden 體系落地**:釘選 37 檔×2 模式錄製入 plugin/rtl2spec/
  golden/+PROVENANCE.md+run-golden-regression.sh(+golden-check.mjs,
  含 --record 自檢 3×3)。自驗:①錄後 `git diff plugin/rtl2spec/
  golden` 逐案對帳(全部可解釋=首錄基線);②比對模式 exit 0;
  ③反向紅路演練:臨時改一提取 message 一字→閘紅→還原→綠。
  驗收:三項全過。
- **S10 破口 fixture 矩陣閘**:gap-fixture-check.mjs(expected
  findings 斷言表+--selfcheck)+gap_missing_reset_no_arm.v+
  run-gap-fixtures.sh;crates 單測層 G4 無復位臂確定名斷言同輪。
  自驗:①閘 exit 0;②自檢紅路(正樣配負樣斷言必紅);③G4 新正樣
  suggested_test 逐字名斷言過。驗收:三項全過。
- **S11 regress 收編+立法四層**:regress.sh 塊內追加兩直調(既有兩閘
  與基線逐字不動)+SKILL/AGENTS 10.10/REGISTRY/ADR-0012 D14..D16
  同輪。自驗:①`bash scripts/regress.sh` exit 0 且既有基線
  (E2E 59/59,ryosys 469/479+live 456/456,analyzer smoke)逐字未動;
  ②新塊兩行 PASS 字樣在場;③rg 交叉引用閉合(三閘名+D14..D16 在
  ADR/SKILL/REGISTRY/AGENTS 四檔互指)。驗收:三項全過。
- **S12 評測擴面+報告+複評**:run-realworld-eval.sh+eval-all.mjs+
  gen-eval-report.mjs+docs/RTL2SPEC-EVAL.md 首錄+R5 分層抽樣 5 檔
  複評落 eval-review-notes.md。自驗:①閘 exit 0(353 檔全終止,
  零守衛觸發);②報告實測列自洽斷言過(判定 FAIL 而實測列無違約=
  紅,gen-report 先例);③R5 五檔探針答案引用 rule id 在場。
  驗收:①②③+總閘七命令(cargo test --workspace/bash scripts/regress.sh/
  run-mcp-e2e.sh/run-rtl-validation.sh/run-golden-regression.sh/
  run-gap-fixtures.sh/run-realworld-eval.sh)全 exit 0。

### 16.7 顯式不做清單(§16 增量;§11 既有 13 條不變)

1. 不修 sv-tests 193 檔 parse_err(構造超 parser 閘,配 L2 容錯面
   即可;解析容錯能力屬 analyzer/vl-syntax 軌);
2. 不把 sv-tests 任何檔入 golden 釘選集(16.1.1 排除記錄);
3. 不對 golden 做 partial/fuzzy 比對(byte-exact 唯一形態,禁模糊
   逃逸口);
4. 不動 13 檔驗證集與 V1..V6 閾值(run-rtl-validation.sh 原樣;
   擴面判據用 16.4 三檔分類不與之衝突);
5. 不把 run-realworld-eval.sh 收編 regress(總閘直調成員);
6. F1(a+b)/F2 修復且 flapper 清零前不錄任何 gaps golden(16.0 錄製順序鐵則);
7. 不手寫/手改 golden 快照、PROVENANCE.md、RTL2SPEC-EVAL.md(全機械
   生成;評審員區屬人工區但只在指定檔 eval-review-notes.md 填寫);
8. 不做 suggested_test→TB 自動生成與執行(破口=清單,§11.7 不變);
9. 不做 golden 的跨平台差分(macOS 本機單平台;linux oracle 面屬
   10.7 CI 機制,本弧無觸)。

### 16.8 風險清單(§16 增量;§12 既有 R1..R9 不變)

| # | 風險 | 緩解/觸發應對 |
|---|---|---|
| R10 | 隱藏非確定源殘留(F1a+F1b 修後仍有 HashSet/HashMap 迭代面或調用流位置耦合進輸出;wasm 跨進程一致性使跨進程測試無判別力) | --record 進程內自檢(16.1.4,前提=S8 flapper 清零)+驗收 12 跑制×37 檔+調用序擾動檢查(16.1.5);golden 紅且無語義變更=立即排查 |
| R11 | G5 規模守衛截斷真環(誤殺) | 超限=「守衛截斷」顯式注記入 verdict(非靜默);閾值取值=S8 實測 scopes.v 邊數×安全係數,注記附實數;修後 golden/評測對帳 |
| R12 | 擴面時長(353 檔)失控 | 單 bun 進程循環(進程啟動一次);L1 首錄實測耗時寫進報告 provenance;若 >5min→檔分批+報告注明(不降判據);終止性防線=trap 計數非進程內計時器(16.4,R16-3 勘誤) |
| R13 | F1 修復波及既有斷言面 | validation-report.md 破口樣本區內嵌 suggested_test 會 churn——重生成一次+數字對帳(收平輪先例);cargo 單測層 G4 相關斷言同步 |
| R14 | golden 與 wasm/native 語義漂移方向不一致 | golden 釘 wasm 腿(交付面);native parity 由 smoke.mjs 6 路 byte parity 兜底;parity 面擴類時同輪補對應 golden 檔 |
| R15 | 外部語料上游變更致 golden 失配 | corpus-external 隨 git 鎖定(唯讀引用);上游升級=語義事件輪,re-record+逐案對帳同 16.1.4 |
| R16 | bun/wasm-pack 環境缺導致執法面靜默退化 | 雙守衛降級形 SKIP 行在 regress 輸出可見(非靜默);本機 bun 1.4.2+wasm32 target 在場(環境事實);缺失輪由 user 決策補環境 |

### 16.9 修訂記錄(2026-11-03 開弧對抗審 7 條意見逐條消化;意見編號 R16-1..R16-7;每條附本弧實證)

R16-1 **[high] 非確定清單不完整(G1 兩處漏登記;golden 釘選集 12/37 檔
實測抖動)**——已修(§16.0 F1 重寫為 F1a+F1b/§16.1.5 契約重寫/S8 擴面/
§16.1.4 自檢重定義)。本弧實證:①gaps.rs:402 `for sig in
l.iter().chain(r.iter())`+gaps.rs:439 `for sig in lhs_set(s)`
(lhs_set=gaps.rs:489-496 HashSet)實讀確認;②loops.v(G1 檔零 G4)
native 5 跑 2 種 hash(bc3bc6fe×3/d353d0d6×2)——「僅 G4 抖」不成立;
③12 檔 flapper 清單(對抗審)wasm 腿同進程 12 跑全量復驗 **12/12
命中**(hash 種數 memory 9/task_func 7/fsm 5/acc_pipe 4/add3 3/
always03 3/generate 2/forloops 2/mem_arst 2/i2c_master_tests 2/
loops 2/fiedler-cooley native 4);④wasm 跨進程首跑 byte 一致(fsm.v
4 個獨立 bun 進程全 bc8e3dc7)+同進程抖=調用流位置耦合雙面實證;
⑤初稿三個 sha256 勘誤(§16.0 誠實注記:觀測腿未注明+不可復現)。
契約補:golden 快照禁依賴調用流位置(§16.1.5 契約②);驗收升級=
全 37 檔×12 跑+調用序擾動(§16.6 S8①)。

R16-2 **[medium] 矩陣 G5/G6/G7 expected 低估(與 sv_rules.rs:366-370
矛盾)**——已修(§16.2 矩陣行重寫+集合相等斷言形釘明)。實證:
sv_rules.rs:366 `classes_of("multi_driver")==
["missing_reset","multi_driver"]`、:368-370
`classes_of("blocking_race")==["blocking_in_seq","missing_reset",
"multi_driver"]`、:341 `classes_of("latch_trap")==["latch"]`(單類)
實讀;gaps_positive.v:46-52 comb_loop=t1/t2 雙信號環無自環(初稿
「自環+雙信號環」勘誤)。

R16-3 **[medium] 「單進程循環+每檔 10s 守衛」自相矛盾**——已修
(§16.4 終止性防線節新增+閘級 FAIL 條件+S8②+矩陣守衛行+R12)。實證:
scopes.v wasm 腿=秒級 trap(本弧 rtl_scan_gaps_sv 探針實拋
`RuntimeError: Unreachable`;對抗審 eval-scale 353 檔實跑唯一 trap,
耗 1305ms),native 腿=掛死(開弧 4s 未終止);單 bun 進程同步 wasm
調用阻塞事件循環,進程內計時器對掛死無效。防線改=wasm 腿 F2 修復+
trap 計數(verdict.trap,修後應零),10s 守衛僅保留 native 腿
subprocess 形;禁在單進程 wasm 循環內宣稱超時守衛。

R16-4 **[medium] 證據指針不可溯源(「勘察事實 §n」懸空+sha256 不可
復現)**——已修(§16.0 勘誤段+標題行+§16.1.1/§16.4 五處引用改寫)。
實證:RTL2SPEC-PLUGIN §2=規則書形態/§4=Rust 核心落點(章節號錯位——
「勘察事實 §n」實指開弧 ask 材料的勘察報告,該報告未入庫,§16 所引
flake 普查/fitness 實掃內容未經 §0 基座章收錄);初稿三個
fiedler-cooley hash 在當前 build 雙腿不可復現(本弧 native 5 跑=
另一集合 11077d43/65060307/910cbd9b/c696d571)。修=§16 內懸空引用
清零(sed 區間驗證=0 命中),改為本弧可重跑命令+當場 hash 集合+
git ls-files 復驗數;sha256 具體值不作證據;觀測腿逐項注明。

R16-5 **[medium] --record 自檢雙向失效(跨進程無判別力/進程內抽樣
漏放)**——已修(§16.1.4 實現形釘明=進程內重複調用+前提=S8 flapper
清零)。實證=§16.0 跨進程(4 進程全一致)/同進程(12 跑 5 種)雙面。

R16-6 **[low] fixture 位置未注明(兩處樹)**——已修(§16.2 fixture
樹段釘明兩處路徑+跨包唯讀引用不搬移)。實證:gaps_positive.v/
gaps_negative.v=crates/rtl-spec/tests/fixtures/(本弧實讀在場,模組
名與矩陣一一對應);新增 2 檔入 plugin/rtl2spec/fixtures/(弧約束)。

R16-7 **[low] counter.v 路徑歧義(repo 兩個同名檔)**——已修(§16.1.1
全路徑=corpus/counter8/counter.v+同名檔注記)。實證:`find . -name
counter.v`(排除 target/vendor)=corpus/oracle-fixtures/counter.v+
corpus/counter8/counter.v 兩檔。

同輪同步:ADR-0012 D14 修訂+實作記錄補行。未動:§0..§15(sha256
斷言)、既有閘與基線、crates 代碼(修復屬 S8 實作輪)。

---

## 17. 拓展性與版本管理(2026-11-04 起草+實證落地;ADR-0012 D17..D19 增訂於同輪[初稿宣稱未落地,復審 R17-1 勘誤後補];§16 零刪改)

目標(使用者原話轉寫):整個工具鏈要能拓展,作為「分析提取 RTL 源碼規格
書」的 agent tool 成熟化;zcode plugin 包裝好了,要有版本管理機制。
本章雙交付:①拓展配方正典(extension recipe)+**按配方真實新增一個破口
類的端到端實證**(禁只寫文檔不做演示,見 17.3);②版本管理機制(semver
SSOT/CHANGELOG/四點一致/ABI 裝載校驗/檢查閘 run-plugin-check.sh)。

### 17.0 設計決策(D17..D19;已增訂入 docs/adr/0012-rtl-to-spec-plugin.md 修訂記錄 2026-11-04 行;域內裸引,D 外引用須全限定「ADR-0012 D17..D19」——ADR-0007 已有本域 D17=rVPI timer)

- **D17 拓展接入=靜態詞表+掃描序註冊點(enum 窮盡 match=編譯期
  registry)**:不引入運行時 registry(字串註冊表/動態加載)——本弧
  「零 serde/手寫 JSON 鍵序 byte-exact」契約下,`GapClass` enum+兩處
  窮盡 match(schema.rs `as_str`/`prefix`)=編譯期註冊表:新增成員漏
  match 直接編譯紅(比字串 registry 多一層編譯期保護);掃描序=
  scan_gaps_sv 內顯式呼叫序列(gaps.rs scan_latches..scan_blocking_in_seq,
  :107-108 區),**新類=序列尾部追加一行**——舊類 id/序零漂移
  (輸出序=掃描序契約 §16.1.5)。
- **D18 版本雙軌**:插件 semver(SSOT=`plugin/rtl2spec/.zcode-plugin/
  plugin.json` 的 `version`,面向使用者的功能版本)與內核 ABI 版本
  (`rtl_spec_version()`=Cargo workspace 版本,內核能力面)分離——
  workspace version 被全 repo crate 共享(`version.workspace = true`),
  bump 有擾動 ryosys/E2E 基線風險且語義(整倉)≠插件面;雙軌各對各的
  執法:插件四點一致(17.4)+內核 ABI∈EXPECTED_ABI(17.5)。
- **D19 ABI 裝載時校驗**:server.mjs 裝載 dist/wasm 後即核對
  `rtl_spec_version()`∈`EXPECTED_ABI` 名單,不匹配=fatal(清晰錯誤
  含雙版本+修復指引)——在「語義入口」前攔,裝載失敗=exit 1 stderr
  (沿既有「載入失敗=exit 1」姿態,server.mjs:11)。

### 17.1 拓展面接入點盤點(§17 配方的地圖)

| 拓展方向 | 接入點 | 觸碰檔 | 不變式 |
|---|---|---|---|
| 新破口類 | `GapClass` enum 成員+`as_str`+`prefix`(schema.rs:369-405 區)+掃描函數+呼叫序列尾部(schema.rs/gaps.rs) | 2 檔 5 處 | 舊類 id/序零漂移;新檢測器過 12 跑制(§16.1.5 契約③);JSON 鍵序=寫出序(byte-exact 由共用 writer 承載,新類零改) |
| 新規則類別 | category 詞表(schema.rs Rule 區)+提取器函數+extract 呼叫序列+V3 類別計數聯動(check-rulebook.mjs 類別面) | 同構 | fingerprint/category 詞表新增=CHANGELOG minor;V3 閾值不動(13 檔集) |
| 新 wasm 導出 | rtl-spec-wasm lib.rs `#[wasm_bindgen]` 函數+EXPECTED_ABI 名單+smoke parity 面 | +1 檔 | 零 js_sys;JSON 字串進出 |
| 新 MCP 工具 | server.mjs tools/list 註冊+handler | +1 檔 | 工具內錯誤=isError:true;協議級=JSON-RPC error(§7) |

### 17.2 extension recipe(新破口類端到端配方;S13 按此實證)

1. **判定設計**:寫清判定依據+confidence(certain=結構可證/heuristic=
   受限場景)+X-safe 面+param 不求值面(§11.5)——一類一判定文;
2. **schema.rs 註冊**:`GapClass` enum 追加成員+`as_str` 字串+`prefix`
   `G<n>`(n=既有最大+1);若 Statement 容器型新變體,gaps.rs
   `collect_ifs` 類遍歷器同步展開(漏展開=靜默漏報,遍歷器 `_` 臂有
   注記錨);
3. **gaps.rs 掃描器**:新函數(簽名同族
   `scan_x(ctx, sc, counters, items)`)+呼叫序列**尾部**追加;遍歷用
   Vec 按語句樹序,**禁 HashSet/HashMap 迭代面進輸出**(§16.1.5 契約③,
   F1 實錄);
4. **單測雙向**(sv_rules.rs,內聯 RTL 字串不動既有 fixture 檔):正樣
   (觸發形,斷言 id 前綴/class/confidence/message 關鍵詞+**零共報
   斷言**)+負樣(變量/param/x 位三形至少覆蓋兩形,零命中);
5. **plugin fixtures 正負樣檔**(弧約束:新 fixtures 入
   plugin/rtl2spec/fixtures/):正樣檔**設計為零共報**(期望 findings
   全集=純新類,使整份 gaps JSON 可入 golden——§16.0 錄製順序鐵則的
   合法繞行面:鐵則禁的是「含 F1 抖動形的全量錄製」,零共報設計檔
   不含抖動源);負樣檔全零;
6. **wasm 重建+parity**:`wasm-pack build crates/rtl-spec-wasm --target
   web --out-dir pkg`+同步 dist/wasm(三檔;run-mcp-e2e.sh/
   run-rtl-validation.sh 陳舊檢查同形)→wasm↔native byte parity
   (trim 對齊=smoke.mjs:75 先例;native emit 的 println! 尾換行非語義);
7. **golden 錄製**:`bun scripts/record-golden.mjs <正樣> <負樣>`(12 跑
   自檢前置+第 13 跑寫盤+PROVENANCE 機械追加,錄製=真實運行禁手寫);
   錄後 `git diff golden/` 逐案對帳;
8. **skills+版本**:skills 類清單行+confidence 行同步;CHANGELOG
   minor bump+四點一致;`bash run-plugin-check.sh` 綠+紅路演練。

### 17.3 拓展性實證:G8 `dead_branch`(常量死分支;S13 全鏈路實錄,本節數字全部當場實跑)

**判定**:If 條件為純字面量(NumLit)且值可判定——恆假(值=0)→then 臂
不可達;恆真(值≠0)→else 臂不可達(若有)。x/z 位 digits 使
`lit_u64` 回 None→不報(X-safe);param 名為 Ident 非 NumLit→不報
(§11.5 求值禁區)。confidence=**certain**(常量真值結構可證)。

**鏈路逐步實測**:
1. Rust 核心:schema.rs 3 處(enum+as_str "dead_branch"+prefix "G8")+
   gaps.rs 2 處(`collect_ifs` 遍歷器[23 個 Stmt 變體顯式展開+容器型
   注記錨]+`scan_dead_branches`)+呼叫序列尾部追加——5 處錨點手術各
   命中==1;`cargo build -p rtl-spec` 零 warning(grep warning=0);
2. 單測:sv_rules.rs +2 測(正樣=恆假/恆真兩模組恰 2 條 certain+零
   共報斷言;負樣=變量+param+x 位三形零命中)→`cargo test -p
   rtl-spec` **26 測全綠**(2 lib+2 rtlil+22 sv,原 24+新增 2);
3. 基線零擾動:**釘選集 37 檔 G8 觸發=0**(emit sv gaps 逐檔 grep
   `"G8-`=無輸出)→probe-determinism 37/37 PASS、run-rtl-validation
   exit 0;報告**行數不變 wc -l=1745+重生成 3 行 diff 對帳在案**:
   wasm sha256 戳 eb69248c→4b143d3a+G4 suggested_test
   y__input_flop→x__input_flop 兩處=§16 F1a 有序取樣修復收據(即
   §16.6 S8④ 的重生成對帳;R17-4 復審修訂勘誤初稿「不變」表述,
   讀作檔案不變則誤導)——舊類 id/序與既有 findings 全集 byte 不變;
4. wasm:wasm-pack build+dist/wasm 同步→wasm↔native **byte parity
   PASS×2**(gap_dead_branch/gap_clean_branches;trim 對齊=smoke.mjs
   :75 同形,native println! 尾換行非語義);probe 12 跑 2/2 PASS;
5. MCP 面:server.mjs **零改**(scan_gaps 透傳 JSON)→run-mcp-e2e.sh
   exit 0(SERVER_VERSION bump 同驗);
6. fixture/golden:正負樣 2 檔入 plugin/fixtures(正樣零共報設計,
   expected findings 注記在檔頭)→record-golden.mjs 錄製 2 份 demo
   golden(gap_dead_branch.gaps.json sha256=e811ea79…/gap_clean_
   branches.gaps.json sha256=9407d571…)+PROVENANCE.md 機械生成
   (含 wasm ABI 戳 0.1.0);
7. skills:rtl-gap-hunt/SKILL.md 3 處(description 類清單/歸併清單
   G1..G8/certain 清單加 G8)。

### 17.4 版本管理(semver SSOT+四點一致;D18)

- **SSOT**=`plugin/rtl2spec/.zcode-plugin/plugin.json` 的 `version`;
  semver 語義:major=工具面/ABI 破壞性變更;minor=新增能力(規則類別/
  破口類/MCP 工具);patch=缺陷修復與文檔;
- **CHANGELOG**=`plugin/rtl2spec/CHANGELOG.md`(Keep-a-Changelog 形:
  Added/Fixed/Breaking;頭部 `## [X.Y.Z]` 必等於 SSOT——檢查閘①a
  執法);0.1.0=初版(S1..S7),0.2.0=G8+版本機制+run-plugin-check
  (本次);
- **四點一致**:①manifest.version ②CHANGELOG 頭版本 ③
  dist/server.mjs `SERVER_VERSION` ④serverInfo.version——④的執法形=
  驗證 serverInfo 行**引用 SERVER_VERSION 常量**而非字面量(禁硬編碼
  漂移;檢查閘①b);
- **雙軌**(D18):內核 ABI 版本=rtl_spec_version()(0.1.0,workspace
  共享)獨立於插件 semver(0.2.0)——同輪實證:插件 bump 後 mcp-e2e/
  rtl-validation/probe 三閘零擾動;
- **git tag 慣例(文檔慣例,真打 tag 由 user 定)**:`rtl2spec-v<X.Y.Z>`
  對齊 CHANGELOG 版本;打 tag 前提=run-plugin-check.sh PASS(弧內禁
  commit/tag,工作樹呈現由 user 裁量)。

### 17.5 兼容契約:ABI 裝載校驗(D19)

server.mjs 裝載段(:46-54 區)後新增:`EXPECTED_ABI=['0.1.0']` 名單,
`rtl_spec_version()` 不在名單→fatal:`wasm ABI 版本不兼容: 內核=X
期望=Y + 修復指引(wasm-pack build 同步)`——語義入口前攔。名單語義
=兼容集(多個內核版本可並列;當前=單元素)。檢查閘③a/③b 獨立重算
(從 server.mjs 提取名單+裝載 dist wasm 實調比對——閘驗出貨物 D8 同款)。

### 17.6 run-plugin-check.sh 判據表(bash 直跑;總閘成員)

| # | 判據 | 紅路面 |
|---|---|---|
| ①a | manifest=CHANGELOG頭=SERVER_VERSION 三點相等 | 任一漂移(紅路實證:SERVER_VERSION 篡改→紅) |
| ①b | serverInfo 行引用 SERVER_VERSION 常量 | 字面量硬編碼漂移 |
| ①c | manifest 版本 semver 三段 | 非法版本串 |
| ② | manifest 靜態校驗(復用 validate-manifest.mjs) | 必填欄/skills/dist 產物缺 |
| ③a | EXPECTED_ABI 名單非空 | 名單被清空 |
| ③b | 裝載 dist wasm 實調 rtl_spec_version()∈名單 | 內核升級未同步名單 |
| ④a | 正樣 expected findings **集合相等**(恰 2 條 G8,§16.2 形) | 檢測器漏報/多報 |
| ④b | 正樣 confidence 全 certain | 置信契約破 |
| ④c | 負樣零命中 | 誤報(誤報紅路面) |
| ④d | 兩檔輸出與 demo golden byte-diff | 語義漂移(golden 紅=語義事件,紅路實證:golden 篡改→紅) |

與既有閘不重複:不跑 13 檔驗證集(run-rtl-validation.sh)、不跑 MCP
全程回環(run-mcp-e2e.sh)、不跑 regress。「無現狀即綠」防線=④c 負樣
(檢測器誤報必紅)+①③對真值比對(非存在性檢查)+紅路演練兩路實錄
(17.3 步 8;SERVER_VERSION→9.9.9 必紅/golden 篡一字節必紅/還原綠)。

### 17.7 文檔面與上手路徑

- skills 已同步:rtl-gap-hunt/SKILL.md 三處 G8(17.3 步 7);
  repo 正典=.agents/skills/eda-dev/SKILL.md「rtl-to-spec 增補」節
  追加「拓展與版本增補」條(同輪);
- **外部 agent 上手路徑**:manifest→skills(rtl-rulebook 提取/rtl-
  gap-hunt 破口/rtl-analyze 投影)→CHANGELOG 看能力邊界與 breaking→
  extension recipe(§17.2)拓能力→run-plugin-check.sh 自證交付;
- git tag 慣例見 17.4(文檔慣例;真打 tag 由 user 定)。

### 17.8 切片分解(S13..S15;本輪三片全部落地——切片與實作同輪,自驗實錄內聯)

- **S13 拓展性實證(G8 全鏈路)**:見 17.3;自驗:26 測綠/零 warning/
  釘選集零觸發/probe 37/37+2/2/parity×2/demo golden 錄製對帳。——
  通過。
- **S14 版本管理+檢查閘**:bump 0.2.0(兩點)+CHANGELOG 新建+EXPECTED_ABI
  裝載校驗+run-plugin-check.sh(12 斷言)+紅路兩路必紅還原綠+mcp-e2e
  exit 0(版本 bump 零擾動)。——通過。
- **S15 文檔面**:本節(§17 章)+SKILL 增補條+skills 三處+CHANGELOG。
  §15 日誌 SSOT 的 S13..S15 條目**留後續輪補記**(本輪受「既有章節
  零刪改」約束;§15 自身定義=每片一行,補記時數字以本節實錄為準)。
  ——通過(補記義務移交後續輪,如實注記)。

### 17.9 顯式不做清單(§17 增量;§11/§16.7 既有不變)

1. 不做規則類別新增的實作演示(配方同構已盤點 §17.1/§17.2;破口類
   全鏈路已證=「或等價小步」裁量,記錄理由:同構工作量×2 而邊際
   證明力不增);
2. 不 bump Cargo workspace 版本(內核 ABI 軌獨立,D18;全 crate 共享
   version 的基線擾動風險);
3. 不做運行時動態 registry/動態插件加載(D17 靜態 enum=編譯期窮盡
   保護;動態面非目標);
4. G8 v1 只掃 If 形(行程內):三元運算子常量死分支(ContAssign/
   Ternary)、case 選擇子常量不匹配臂不做(配方注記=可拓展點,非
   缺陷);
5. 不把 run-plugin-check.sh 收編 regress(總閘成員直調;收編立法屬
   後續輪,沿 §16.3 四層落點形態);
6. 正式 37 檔 golden 樹(S9)不在本輪:本輪只 demo 樣例
   (golden/plugin-fixtures/,PROVENANCE 注記演示性質),S9 錄製鐵則
   與機制照 §16 不變;
7. §15 日誌不在本輪追加(零刪改約束;S13..S15 補記移交後續輪);
8. **§16 S9..S12 交付物全景欠賬注記(R17-5 復審補)**——現狀盤點
   (2026-11-04 ls 實證):已實作=S8(確定性三修+probe-determinism.mjs+
   pinned-files.list+gap_hang_repro.v)與本輪 S13..S15;**未實作=**
   run-golden-regression.sh(S9)、run-gap-fixtures.sh+scripts/
   gap-fixture-check.mjs+fixtures/gap_missing_reset_no_arm.v(S10)、
   run-realworld-eval.sh+scripts/eval-all.mjs+gen-eval-report.mjs+
   docs/RTL2SPEC-EVAL.md(S12)與 regress 收編(S11)——讀者對照 §16 時
   以本條為全景狀態表,禁把規劃閘當實存閘(ADR 機械閘條 4 分階段
   同款勘誤);golden 樹現存內容僅本輪 demo 樣例(17.9.6)。
   docs/RTL2SPEC-EVAL.md(S12)與 regress 收編(S11)——讀者對照 §16 時
   以本條為全景狀態表,禁把規劃閘當實存閘(ADR 機械閘條 4 分階段
   同款勘誤);golden 樹現存內容僅本輪 demo 樣例(17.9.6)。
9. **上條欠賬清償勘誤(2026-11-04 S9..S12 輪追加;原第 8 條為快照
   不改)**:§16 S9..S12 已全部落地——run-golden-regression.sh(S9,
   74 份 golden+PROVENANCE 全量重寫形)/run-gap-fixtures.sh+
   gap-fixture-check.mjs+gap_missing_reset_no_arm.v(S10)/
   run-realworld-eval.sh+eval-all.mjs+gen-eval-report.mjs+
   docs/RTL2SPEC-EVAL.md(S12,353 檔首錄 trap=0)/regress 收編(S11,
   雙守衛塊兩直調)——三閘轉現役(ADR-0012 機械閘條 4 同輪更新);
   §16 錄製順序鐵則已按前提成立(F1a/F1b 修復+flapper 37/37 清零)
   後執行首錄;正式 37 檔 golden 樹已在庫(demo 樣例與正式樹兩域並存,
   PROVENANCE 分域注記,17.9.6 的 demo-only 狀態同步解除);§15 日誌
   S8..S15 補記同輪清償。

### 17.10 風險清單(§17 增量;§12/§16.8 既有不變)

| # | 風險 | 緩解/觸發應對 |
|---|---|---|
| R17a | 新檢測器引入非確定源(HashSet 迭代面/位置耦合) | recipe 步 3 禁令+步 7 錄前 12 跑自檢強制(機械化=record-golden.mjs 內建)+§16.1.5 契約③ |
| R17b | 版本四點手工漂移 | run-plugin-check ①a/b/c 執法+紅路實證;bump 檢查單=SSOT 先改→CHANGELOG→SERVER_VERSION→閘驗 |
| R17c | 內核升級未同步 EXPECTED_ABI | 裝載時 fatal 清晰錯誤(雙版本+修復指引,D19);名單多元素兼容集可吸收次版本漂移 |
| R17d | demo golden 與 S9 正式樹結構衝突 | PROVENANCE 注記演示性質;S9 實作時以 pinned-files.list 鏡像路徑統一,demo 檔遷移=語義事件對帳 |
| R17e | G8 誤報面(位寬截斷/字面量上下文) | 條件為自決定上下文(§5.4),字面量按自身值取真——負樣三形釘死+④c 閘執法;新形誤報=單測釘死+CHANGELOG patch |

### 17.11 復審修訂記錄(2026-11-04 換人復審 8 條意見逐條消化;編號 R17-1..R17-8;每條附本輪實證)

R17-1 **[high] 「ADR-0012 增訂 D17..D19」宣稱未落地+總閘定義分裂**——
已修。實證(本輪復跑):`rg 'D17|D18|D19' docs/adr/0012-rtl-to-spec-plugin.md`
與 `rg '2026-11-04'` 同檔初查均 0 命中(exit 1)——初稿 §17.0/SKILL
以完成時態宣稱而 ADR 實未動(ADR diff 43/2 與 §16 輪相同)。修=同輪
增訂 ADR-0012:D17(靜態詞表+掃描序註冊點)/D18(版本雙軌)/D19(ABI
裝載校驗)三條+修訂記錄 2026-11-04 行;**總閘序列改現役/規劃分階段**
——初稿七命令把三個未實作規劃閘(golden-regression/gap-fixtures/
realworld-eval)寫成現役=定義分裂,勘誤為現役五命令(cargo test
--workspace/regress.sh/run-mcp-e2e.sh/run-rtl-validation.sh/**
run-plugin-check.sh 入列**)+規劃三命令(存在性由 S9..S12 交付,禁預寫
CI);SKILL 引用改全限定形「ADR-0012 D17..D19」(ADR-0007 已有本域
D17=rVPI timer,裸引歧義);本節標題同步注記。

R17-2 **[medium] record-golden.mjs 裝載 pkg 非 dist(錄製面與驗證面
平行,脫節時 golden 錯釘非交付面)**——已修=改 dist 裝載(優於頭注
豁免:D8「閘驗出貨物」語義本位)。實作與除錯實錄:首輪修復只改了
init 行、api import 仍指 pkg→`wasm.__wbindgen_free undefined`(pkg
glue 從未 init,棧指向 pkg 路徑=線索);雙路徑調試探針實證 glue/wasm
byte 相同且各自可載後,補改 `const api = await import(join(distWasm,...))`;
重錄兩檔**內容 byte 不變**(sha256 e811ea79…/9407d571… 與初錄一致=
錄/驗面同源證),PROVENANCE 機械追加重錄行。

R17-3 **[medium] F2 根因賬本分裂(REGISTRY 11-03 行勘正真根因=bfs_path
死循環[prev 自映射,實測 nodes=2/edges=7,規模爆炸假說證偽;修法=
visited 集,守衛防禦性保留],§16.0/§16.4 凍結於舊歸因)**——本輪
§16 零刪改不可動,登記移交義務:**後續輪解除零刪改時,§16.0 F2 原地
補勘誤注記+§16.4「圖規模守衛後理論全終止」論證改引 REGISTRY 11-03
行**(真根因=死循環已修,守衛=防未來真大圖的防禦層;REGISTRY 行措辭
「§16.0 F2 記錄勘正」在補注記前讀作「待補」)。本輪驗證:REGISTRY:162
行內容實讀在案;bfs_path visited 修復+gap_hang_repro.v+
gap_comb_loop_self_edges_terminate_and_report 單測=S8 輪已落地
(工作樹在案)。

R17-4 **[low] §17.3 步 3「1745 行不變」表述含糊**——已修(步 3 原地)。
實證:`git diff validation-report.md`=3 insertions+3 deletions
(1 行 wasm sha256 戳 eb69248c→4b143d3a+2 行 G4 suggested_test
y__input_flop→x__input_flop=F1a 有序取樣修復收據);新表述=「行數
不變 wc -l=1745+重生成 3 行 diff 對帳在案」,指向 §16.6 S8④ 承諾。

R17-5 **[low] §17.9 欠賬交代不全(讀者對照 §16 會誤判只剩 S9/S11)**——
已修(§17.9 新增第 8 條全景狀態表):已實作=S8+本輪 S13..S15;未實作=
run-golden-regression.sh/run-gap-fixtures.sh+gap-fixture-check.mjs+
gap_missing_reset_no_arm.v/run-realworld-eval.sh+eval-all.mjs+
gen-eval-report.mjs+RTL2SPEC-EVAL.md+regress 收編(ls 實證)。

R17-6 **[low] pinned-files.list:4「乾浙室」錯字**——已修=「乾淨室
MIT(OpenCores 唯讀啟發零拷貝)」。

R17-7 **[low] run-plugin-check.sh:57 死變量 abiList**——已刪(③a 實際
用 expectedAbi 精確提取;死變量誤導讀者以為存在額外名單斷言)。

R17-8 **[low] probe-determinism.mjs 頭注與實現不符(「倒序重跑」不存在;
實現=per-file N 輪+全檔正序後本檔再跑一輪擾動)**——已修=頭注對齊
實現+殘差注記(無倒序階段,清單首檔無跨檔前序覆蓋=跨檔-only 耦合
理論殘餘面;主風險=同檔抖動已由 N 輪覆蓋;未來發現跨檔-only 耦合形
擴倒序階段=語義事件)。

同輪同步:ADR-0012(D17..D19+總閘分階段+修訂記錄 2026-11-04 行)/
SKILL(全限定引用)/record-golden.mjs+run-plugin-check.sh+
probe-determinism.mjs+pinned-files.list 四檔腳本與清單。未動:
§0..§16+§17.0..§17.10 主體(除標題/步 3/17.9 三處 R17 修訂點);
修復後復跑 run-plugin-check.sh PASS+dist 腿重錄 byte 不變(見各條)。

---

## 18. 復原力弧實施記錄(S0..S5;2026-11-05;RTL2SPEC-MVP-NEXT R2 執行;append-only)

本章=復原力 MVP 弧(S0..S5)的實施記錄;既有章節零刪改。provenance 標記:
【實跑】=本弧命令實測;【檔】=在庫工件;【材料】=提案/網調材料引文。

### 18.1 M8 殘文候選十行逐行裁定(【實勘】+S5 授權訂正)

候選來源=殘文掃描(與 R16-3 勘誤條/實測計數牴觸的 remnant)。裁定:
- **訂正 4 行**(2026-11-05 S5 同輪):
  1. PLUGIN.md:1399「進程可終止(10s 守衛/檔)」→「F2 圖規模守衛+trap 計數
     (wasm 單進程循環,無計時器面;native 腿=subprocess 10s)」——與 §16.6
     R16-3 條(1430-1439)對齊;
  2. PLUGIN.md:1400「192 檔 parse_err」→193(實測 193,R16 實跑);
  3. PLUGIN.md:1508「不修 sv-tests 192 檔」→193(同上);
  4. scripts/eval-all.mjs:7 註釋「192 檔 parse_err」→193(註釋與實測對齊)。
- **歷史記載保留 6 行**(逐行實勘=當時實測快照/勘誤記錄本身,不改寫歷史):
  ①1215(parse_err 192=開弧 fitness 實掃快照);②1430-1439(R16-3 勘誤條
  本身——初稿謬誤+修正後法條同存,勘誤記錄不可刪);③1477(native 腿
  subprocess 10s=現行正確法條);④1530(R12 風險表=trap 計數勘誤記錄);
  ⑤1562(R16-3 記錄行);⑥1685/1862 區(S9/S12 wasm sha 過渡日志
  eb69248c→4b143d3a,S0 時已裁定保留)。

### 18.2 各切片機械門檻實跑數字(【實跑】;詳證在各切片對賬記錄/本弧 session)

- **S0**(PROVENANCE 手記保留):演練斷言①手記行逐字保留②元資料區機械重寫
  ③git diff golden 僅 PROVENANCE(+4/-2)+74 JSON byte-identical;seed 回退路
  (暫移檔→--record→seed+錄製行)演練通過。
- **S1**(contract 節+lint):三 specs 對拍 62 PASS/0 FAIL(counter WIDTH=8/
  acc_pipe x/y[8] out[16]/i2c scl_i+prescale[16]+cmd[4]+state[4]+cnt[16]+
  bitcnt[4]+10 localparam 逐值);lint i2c 兩副本差集=∅;37 檔 B 面三次
  證據驅動擴充後懸空=0/37;golden 對賬 37 檔 rules 深等(純增量)。
- **S2**(組合覆蓋放寬):acc_pipe p1_bit_slice/p2_scaled 定義規則在場;
  muxtree default_cases 0→4;正負 fixture(rel_positive/rel_negative)閘行
  全綠;golden 對賬+33 條全數謂詞命中、零刪改。
- **S3**(復位語義+指紋+M3):i2c R001 async=true(敏感列表第二證);
  muxtree R003/R004 when 互斥(!rst vs !(!rst));HOLD/IDLE 臂 tip/bitcnt
  可見;DEFAULT_MAX_RULES 128→256(i2c 152 觸頂);對賬 guard+async=12/
  敏感列表證=29/seq+when=296/去重復活=175/零漂移=261/零刪除。
- **S4**(M1/M2/M4/M5/M6):M1 2'b01 觸發域在場;M2 default 臂否定合取;
  六 fixtures 入 fixtures/+閘清單 8 行;golden 對賬 7 档全歸因
  (muxtree/generate/process=M1+M2、sincos/constmuldivmod/fiedler-cooley=
  M2 否定鏈、partsel.gaps=M6 精確移除「4/4 覆蓋」誤報);gap_clean_branches
  零命中保持。
- **S5**(復原對拍):23 場景一鍵跑通(iverilog 13.0 守衛);消融點分段——
  S1 點:i2c A0=compiled=false(pre-S1 對照)→A1..A3 全 compiled+11/11
  (判準 n≥3 中 ≥2 compiled=true=PASS);S2 點:acc A0=0/6→A1..A3 全 6/6
  (提案時預測「matched 仍低」被實測推翻=N3 修復後滿分,如實記錄預測偏差);
  S3 點:A0f(top_fixed)=SCENE10 失配 10/11→A1..A3 全 11/11(PASS);
  抄寫通道:A/B 差=0(i2c-B1 11/11、acc-B1 6/6)——B 組執行者與 A 組同一
  agent,上下文污染雙側對稱,差值解讀力受限(REIMPL-PROTOCOL.md §0),如實
  分列禁混報。
- 基線位移(阈形/結構形閘,無釘數):V1-total 506(13 檔);eval 指紋 1212
  dup=0 保持;37 檔 Σrules=838(cap=256 零截斷);閘級 SUMMARY(E2E 59/59、
  ryosys 469/479、live 456/456)零擾動。

### 18.3 過程/結果判據分層(S5 立法核心)

- **結果判據(機械,可重跑)**:iverilog 編譯成敗+TB 逐場景 diff——
  run-reimpl-eval.sh 執法,寫只落 build/reimpl-eval/。
- **過程判據(輔助,永不進確定性閘)**:盲寫腿(prompt 模板+模型身份+採樣
  n;REIMPL-PROTOCOL.md 釘檔)——本弧盲寫腿模型=GLM-5.3-Flash max n=3;
  盲寫執行者與實施弧同源,上下文污染記錄在案;泄漏核查只能證偽不能證真。
- **預測 vs 實測**:提案材料中的預測(canon S2「matched 仍低」)與實測
  (滿分)分歧時,記實測+預測偏差歸因,禁改預測文本。

### 18.4 §18 待決清單(用戶裁決事項;禁清理 build/mvp-exp/)

1. build/mvp-exp/ 記錄文檔(experiment-record*.md/audit 5 份/specs 3 份/
   README-e1.md/e1 TB 舊件/reimpl 舊件)的 git 晉升落點——裝備已遷
   fixtures/(byte-identical),記錄文檔晉升與否待裁決;
2. M4 EventCtrl 展開=canon 名單二字外同形容器補齊——正典追認與否;
3. S2 IndexedPart/Index 不在名單(canon 字面三形)——擴名單與否;
4. 內核 ABI 車(workspace 0.1.0)不隨插件 semver 0.3.0 上調——雙軌制
   (§17.5)下 vl-cli version golden 釘 workspace 版,晉升策略待裁決;
5. B 組(裁剪 evidence)樣本在本弧條件下的解讀力(上下文污染對稱)——
   獨立對話盲寫輪是否補跑。

### 18.5 S6 未執行記錄

metamorphic 歸一化結構比較探針(§5.2 S6)本弧未執行(留後續弧);其邊界
條文已隨 S5 同輪落 AGENTS.md 10.10 與 SKILL.md rtl-to-spec 增補節
(探針寫只落 build/、無 oracle 近似形聲明、變換集收窄)。

### 18.6 S6 實施記錄(2026-11-05 後續補錄;探針與比較層寫只落 build/s6-metamorphic/)

- **結果(【實跑】)**:`bash plugin/rtl2spec/build/s6-metamorphic/run.sh` exit 0——
  比較層自檢 ALL PASS(10 斷言:paren/rename/reorder 三臂 iverilog 同輸出+有符號
  溢出邊界單獨測($signed 加法重排)+負樣 a-b≠b-a 雙檢(iverilog 輸出異+摘要判異));
  探針=pinned 37 檔非平凡全集×三臂(paren=單行賦值 RHS 整體包裹/rename=系統
  重命名附映射/reorder=純標識符交換律交換)→**IF=0**(≥20 檔達標)。
- **比較層實現形**(與 canon §5.2 S6(a) 對應):名稱歸一化=標識符映射逆回寫
  (rename 臂記錄映射,變體經逆映射還原後進 base 詞彙表排序規範名 n1..nk);
  AST 規範化=優先級爬升解析+交換律運算(+ * & | ^ ==)孩子排序;AST 無 Paren
  節點→渲染天然去括號;when 頂層按「 且 」合取切分排序(提取器 conds 慣例)。
- **比較層誤報排查記錄(IF>0 → 0 的迭代,全部=比較層缺陷非提取器)**:
  ①單字符運算符分類漏 `~`(~tx 情形);②基數字面量字符類漏 s/o/h/H(S3 的
  `'sd7`/S2 的 `'h0` 情形);③`$signed` 系統函數無 Call 面;④IndexedPart 渲染形
  `+::` 未識別;⑤when 頂層「 且 」合取未切分;⑥PARSE-ERR 標記內嵌原文→
  rename 臂生偽 IF(改去名稱 token 序列);⑦transform paren 臂吞逗號續行;
  ⑧rename 臂改寫 `` `timescale `` 參數(編譯指令整行跳過)——⑧項屬變換面
  侷限(編譯守衛攔截計數,非 IF),已修。
- **誠實邊界(聲明)**:本探針=原文判據的無 oracle 近似形——摘要集相等≠語義
  正確性證明;語義正確性判斷留給未來 oracle 全量化(§18.3 分層同源)。
