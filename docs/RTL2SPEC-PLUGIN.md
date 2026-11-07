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

---

## 19. C 閉環全量化(規則書→TB 生成+conformance 評測+診斷回灌;2026-11-06 規劃輪起草;ADR-0013;append-only,§0..§18 零刪改)

本章=RTL2SPEC-MVP-NEXT §7.3 後續隊列首項「C 全量化(TB 生成+診斷回灌)」的方案章。R3 當時
落選理由(MVP-NEXT:245)=「閉環判別力依賴規則書先夠好——acc_pipe 0/6 上閉環全紅無分辨率」;
復原力弧後 acc_pipe A1..A3 6/6、i2c A1..A3 11/11(§18.2 S5 條)——前提已消除,開弧。同弧
搭車收平復原力弧終審三缺陷(REGISTRY D-r1a/r1b/r1c)。決策 SSOT=docs/adr/0013-c-loop-conformance.md
(**D-C1..D-C13**;D-C 前綴避讓 ADR-0012 D1..D19 與 ADR-0007 D17,引用須全限定)。切片編號=
**C-S1..C-S6**(避讓 §18 的 S0..S6 同檔撞名——REGISTRY M7/M8 命名空間注記同款教訓;引用寫
「§19 C-S3」)。provenance:【實跑】=本規劃輪命令實測(探針只寫 /tmp/c-arc-plan/,倉庫寫入僅
本章與 ADR 兩檔);【檔】=在庫工件逐行讀;【材料】=開弧勘察清單(ask 材料,未入庫;其 /tmp 探針
數字不是倉庫工件)。**路徑約定(復審 RV1-14)**:本章 `scripts/regress.sh` 指 repo 根;其餘
`scripts/*.mjs` 短寫一律指 plugin/rtl2spec/scripts/;本弧新檔一律寫全路徑。

### 19.0 開弧事實基座

1. **對拍床=量測器,非判別閘(設計使然)**【檔】:`scripts/reimpl-eval.mjs` 僅原版腿編譯/運行敗
   exit 1(:136-139)、缺腿 exit 3、其餘 exit 0(:205-206),對 matched/compiled 無斷言——這是
   AGENTS 10.11「盲寫腿永不進確定性閘」的正確形,不是缺陷;但它同時意味著**倉內沒有任何「TB
   本身有判別力」的機械證明**(本弧 C-S3 補)。兩個真洞:無看門狗(`execFileSync` 不帶 timeout,
   :75-82;違 AGENTS §4)、`scenesMatched = leg.scenes − 失配數`(:167)不核對解析塊數且失配只在
   原版塊 key 上迭代(:164-166)——TB 少打塊頭時 matched 虛高。
2. **e1 三 TB 非 X-clean**【檔】tb_counter8.v:17-18(上電「q 保持 x」)、tb_acc_pipe.v:22-23
   (power_on_x)【材料】verilator 5.052 跑 tb_counter8 對 iverilog 僅 matched 4/6——只適用 iverilog
   腿,不滿足 AGENTS §2;**C 弧入庫 TB 不復用 e1 TB**(e1 TB 留在 reimpl-eval 床,角色不變)。
3. **TB 11/11≠等價**【材料】:4 條 post-S4 i2c 重寫腿 e1 TB 全 11/11,yosys equiv 全留 `cnt_end`
   unproven(A2/A3 另有未裁決項);根因=`corpus/e2e/59_i2c_master/i2c_master.v:68`
   `wire cnt_end = (cnt >= prescale - 1);`(net 宣告賦值)【檔】落入 `extract_module`
   (`crates/rtl-spec/src/sv/mod.rs:641`)的 `_ => {}` 臂(:686;只認 Always/Initial/ContAssign)【檔】;
   e1 TB 固定 `prescale = 16'd4`(tb_i2c.v:12)【檔】,看不到 32 位與 16 位上下文差。
4. **規則書定義完備性可機械定位**【實跑】(/tmp 唯讀探針,口徑=contract.signals∪輸出端口,減去任一
   規則 lhs/signal/state_reg/effects 的基標識符):i2c=`[cnt_end]`;acc_pipe=`[out]`(`assign out =
   out__output_flop;`=S2「直通別名不產」政策面);subbytes=7 項(直通別名/分支域外常量/Index 形
   混合,Index 形=§18.4-3 待決);fsm/sincos/signedexpr=0。這是本弧回灌對象 F3(19.6)的確定性觸發面。
5. **變異基建現況**【檔】:spec-i2c `Variant::rtl_for`(crates/spec-i2c/src/scenario.rs:28-67;錨點
   replace,未命中回 Err——**只判 `out == RTL`,不判 count==1**);gap-fixture-check selfcheck「正樣
   期望誤配負樣必紅」(scripts/gap-fixture-check.mjs:197-205);corpus_lint R1 同沿競爭規則
   (crates/vl-difftest/tests/corpus_lint.rs:3-6/:53,三向量 :175-180)。**倉內無通用 RTL 語義變異
   工具**【材料 rg】。/tmp 手工 16 變異殺 13,排除等價後 13/15【材料,非倉庫工件】:counter8 `async`
   存活(TB 只在 posedge 後採樣)、acc `nocarry` 等價(yosys 108/108 proven)、i2c `sclI` 存活(TB 固定
   scl_i=1)。
6. **等價腿**【材料】:ryosys 11 個 equiv_* 全是 Y0 stub(crates/ry-cli/src/registry.rs:76-86);yosys
   oracle(vendor-reference/suite-dl/oss-cad-suite/bin/yosys,Yosys 0.70+179)可用,僅測試用;流程=
   `proc; [async2sync;] opt_clean; equiv_make; hierarchy -top equiv; equiv_simple -seq 5;
   equiv_induct -seq 5; equiv_status -assert`,每次 ≤2s;異步 FF 必 async2sync;**unproven≠確證
   不等價**。
7. **工具鏈**【實跑】:`verilator --version`=Verilator 5.052 2026-09-05(AGENTS/ask 所記 5.050 已過時,
   以實跑為準);`iverilog -V`=13.0 (stable);bun 1.4.2;vendored yosys 路徑在場(只 ls,未執行)。
   verilator `--binary` 鏈接需 DEVELOPER_DIR=CLT【材料】——scripts/regress.sh:12-14 三行守衛形【檔】。
8. **採樣候選可編譯**【實跑】`iverilog -g2005 -t null`:19.7 六檔全 rc=0、warning=0。
9. **面現況**【檔】:MCP 4 工具(dist/server.mjs:103/128/162/189);mcp-client ②「恰 4 工具」
   (scripts/mcp-client.mjs:91-100);wasm 7 導出(REGISTRY:14);`crates/rtl-spec-wasm/smoke.mjs`
   6 路 parity=手跑(rg 無任何閘腳本調用)【實跑】;插件版本四點=0.3.0(plugin.json:3/CHANGELOG
   頭/SERVER_VERSION server.mjs:21/serverInfo :284);EXPECTED_ABI=['0.1.0'](:61)。
10. **D-r1c 同形第二處(復審 RV1-1 補)**【檔+實跑】:crates/rtl-spec/src/sv/gaps.rs:192-217 的復位臂
    識別同樣取「首個 pos/neg 邊=觸發」(註釋 :194-195 自述「與規則提取面同一判準」);dist wasm 實探
    復位前置形 `rtl_scan_gaps_sv`→`G4-m-001 missing_reset certain`(假陽性),時鐘前置孿生→`[]`;
    在庫 golden/yosys-simple/mem_arst.gaps.json 即含 `G4-MyMem-001` certain「邊沿行程完全無復位臂」。
    RTLIL 面 crates/rtl-spec/src/rtlil.rs:197-209「觸發=首個 pos/neg sync」為**潛在同形第三處**:
    vendored yosys `write_rtlil` 產的兩序樣(p3/p4.il)sync 序確隨敏感列表(復位前置=`sync negedge`
    在先),但 RTLIL 面對兩序皆產 0 條規則【實跑】——未現形。
11. **D-c2 ANSI 端口宣告屬性繼承缺陷(復審 RV1-4 補;復審 RV2-2/RV2-6 訂正範圍)**【檔+實跑】:
    crates/vl-syntax/src/parser/module.rs:132-171 的 ANSI 端口組解析:只有 peek_starts_port_spec 命中
    (input/output/inout/reg/signed/net 種類/`[`,parser/mod.rs:449-460)才開新組,而**新組不重置**
    cur_net/cur_reg/cur_signed/cur_range——範圍、符號、種類三族屬性都跨組繼承。oracle(IEEE 1800-2017
    §23.2.2.3 同向:方向/種類/型別全省略才整體繼承,否則只有省略的方向繼承、其餘取缺省)七向量見
    19.3.2 C-S2 首步表:iverilog 13.0(-g2005/-g2012)與 verilator 5.052 一致,HEAD 解析七向量全錯
    (dist wasm `rtl_analyze_sv` 實跑)。倉內舊構建 target/debug/rverilator(2026-10-27,非 HEAD 重建)
    對 v2 形為 `busy=1111 bits=4`。contract 現形:fixtures/fsm_counter.v 的 `busy` 報寬 4(源碼 1 位);
    golden mem_arst 只有 ports[5] `WR_i`(mem_arst.v:11)被誤標 parametric——Addr_i/Data_i/Data_o
    (:8-10)本來就是參數寬度(RV2-12 訂正 19.16 RV1-4「四端口誤標」);golden partsel 的 16 個端口被誤標
    signed(`input signed [4:0] n, output reg [31:0] dout` 形,partsel.v:70-71 等)。**定量普查=/tmp 補丁
    實驗**(按 19.3.2 修法給 vl-syntax 打補丁、以 native emit 重跑後與未補丁逐葉比對;探針寫只落
    /tmp/c-arc-r2/):釘選 37×2 檔 same 70 diff 4=24 葉(loops 4/mem_arst 2/memory 2/partsel 16);
    L1 93 檔另加非釘選 retime(1 葉);corpus/ 96 檔與 sv-tests 260 檔 `analyze` 全零差;19.7 六個採樣設計
    零差。初稿「啟發式普查 6 處 5 檔」只掃範圍繼承、且按首端口計數,漏掉 signed 族(partsel)並低估
    同組後續端口(loops 實為 4 端口)——作廢,以本條定量結果為準。另見(非本缺陷,登記為 D-c4):
    partsel_test006/007 的 `[31:-32]` 負數下標範圍在 contract 報 `width 0+parametric`(實為定寬)。

### 19.1 目標、oracle 紀律與禁止宣稱

- **三層分立(D-C1)**:
  - **L-A TB 入庫判別力閘(確定性)**:每個入庫 TB 必配正樣 fixture(原版 RTL 過)+變異樣 fixture
    (≥1 個語義變異必紅),判別力數字錄製入庫——無判別力的 TB=無效測試(AGENTS §2 精神);
  - **L-B conformance 計分(計分確定;被計分物是 LLM 產物時只作輔助證據)**:重寫/重建產物在
    入庫 TB 下的逐場景通過率;
  - **L-C 診斷回灌**:counterexample→結構化診斷(確定性)→回灌對象(重寫迭代/TB 改進/規則書缺陷
    報告;LLM 迭代=輔助,≤3 輪)。
- **oracle=原版 RTL 在 iverilog 13.0 下的可觀測行為**:TB 只觀測不自判;期望=`--record` 從原版
  錄製(AGENTS §1 同律:禁手寫期望值);TB 內不得出現任何期望常量或自檢斷言。
- **禁止宣稱**(報告/條文/commit message 一律適用):①「TB 全過=設計正確/意圖正確」——conformance
  度量的是**行為復原力**,不是意圖正確性(MVP-NEXT:245 循環論證警告);②「conformance 100%=規則書
  完整/重寫等價」(19.0-3 cnt_end 是實證反例);③回灌 round≥1 的分數記作規則書復原力(診斷含 oracle
  期望值=泄漏,只能記「閉環可復原性」);④殺傷率=對**規則錨定語法變異**的敏感度,≠對任意 bug 的
  檢出率;⑤等價只認 yosys `proven`,unproven 既不記等價也不記不等價;⑥模板腿/定向腿、重寫各輪、
  A/B 組的數字一律分列,禁混報。
- **閘判據劃界(R1-3 修訂;19.4.5/19.11-2/ADR-0013 D-C1/D-C5 同一定義)**:**凍結入庫工件**(T 腿或
  D 腿產出的 tb.v、錄製的 expected.txt/record.json、由規則書機械導出的變異體)的**機械可測屬性**
  (Q1..Q9:編譯/正樣/X-clean/協議/靜態約束/跨模擬器/判別力與 record 雙向匹配/D 腿形族底線與不劣於
  基線)構成入庫判據與閘 exit 判據——被判的是工件本身;**LLM 採樣過程**(prompt/模型/採樣次數/K 的
  選擇/改進輪次)與**重寫產物在 TB 下的計分**(verdict `aux` 欄、F1 各輪分數)永不構成閘 exit 判據
  (AGENTS 10.11 與 §18.3 S5 先例本義:盲寫腿數字只作輔助證據)。

### 19.2 前置切片 C-S1:D-r1a/r1b/r1c 收平(實現順序第一片;逐條驗收)

**D-r1a(max_rules 上調未達 MCP/batch 面)**
- 普查【實跑】`grep -n '128\|256'`:插件面 128 實為 **5 處**——dist/server.mjs:139(inputSchema 描述
  「default 128」,REGISTRY 漏列)/:150(默認值)/:219(batch 硬編碼)、scripts/gap-fixture-check.mjs:215、
  skills/rtl-rulebook/SKILL.md:15(文案「默認 128」,REGISTRY 漏列);已對齊 256=golden-check.mjs:80/
  check-rulebook.mjs:39/eval-all.mjs:140/probe-determinism.mjs:81,85/reimpl-eval.mjs:37;SSOT=
  crates/rtl-spec/src/schema.rs:15。**非面**:crates/rtl-spec-wasm/src/lib.rs:98/110/111(crate 內單測
  顯式傳 128,微型 SV 遠低於上限,不屬漂移;普查表注明不改);crates/rtl-spec-wasm/smoke.mjs:53/:80
  (:53=冒煙斷言、:80=wasm↔native parity 兩側同傳 128 的對稱比對,不屬面漂移;**復審 RV2-10 訂正**
  RV1-15 所記 「:79」——實跑 `rg -n '\b128\b' crates/rtl-spec-wasm/smoke.mjs` 命中 :53 與 :80,:79 是
  analyze_sv 行)。**另一面漂移(RV2-10 補,納入修法)**:crates/rtl-spec/examples/emit.rs 的 native
  缺省上限=128——:10 無 max 分支 `128usize`、:16 `mx.parse().unwrap_or(128)`【實跑 sed】;C-S2 還要在此檔
  擴 `tbplan` 模式(19.3.2),故同片收平。
- 修法:server.mjs :150/:219 改傳 **0**——wasm 面既有契約「max_rules==0 取核心默認」
  (crates/rtl-spec-wasm/src/lib.rs:49-54)【檔】,面委託核心 SSOT,**不複製常量**(10.7 零語義);
  :139 描述改「省略或 0=核心 DEFAULT_MAX_RULES(現值 256);overflow marks truncated」;
  gap-fixture-check:215→256(與同族腳本一致);rtl-rulebook SKILL.md:15 文案同步;emit.rs :10/:16 的兩處
  128 改引 `rtl_spec::DEFAULT_MAX_RULES`(RV2-10)。
- 防回歸(機械):run-plugin-check.sh 新判據 **⑤ SSOT 漂移**——從 schema.rs 解析
  `DEFAULT_MAX_RULES: usize = N`,斷言 (a) plugin/rtl2spec/{dist,scripts} 下全部 .mjs 的
  `rtl_extract_rules_sv(…, <字面量>)` 字面量∈{0,N};(b) server.mjs 描述與 rtl-rulebook SKILL 的上限
  數字==N;**(c)(R1-8 修訂)回退 token 錨**——規則工具的實參是變數 `max`,(a) 看不到其回退值,故另以
  錨定 regex 斷言 server.mjs 中 `const max =` 表達式的回退分支字面量==0(現狀 :148-150 為
  `? Math.floor(args.max_rules_per_module) : 128;`【檔】)且 batch 呼叫第三實參==0。紅路演練=臨時把
  :150 回退值或 :219 實參改回 128 各必 FAIL→還原 PASS(兩路實錄入本章實作記錄)。**(d)(RV2-10 補)
  native 面**:crates/rtl-spec/examples/emit.rs 必含常量名 `DEFAULT_MAX_RULES`,且參數分派段無
  `unwrap_or(<數字字面量>)` 與 `<數字>usize` 形缺省(錨定 regex);紅路=臨時改回 `128usize` 必 FAIL。
- 效果斷言:mcp-client 新斷言 **⑧**——對 corpus/e2e/59_i2c_master/i2c_master.v(cwd=插件根,路徑
  `../../corpus/…`;語料唯讀)四次調用:`rtl_extract_rules` ①不傳 max ②顯式 `max_rules_per_module: 0`
  (R1-8 補:顧客顯式 0 走核心默認)均 rule 數=152 且 truncated=[];③顯式 100→rule 數=100 且
  truncated=[i2c_master](上限參數仍被尊重);④`rtl_extract_batch` rule_count=152(現狀 ①④=128+
  truncated【材料】)。
- 驗收:`bash plugin/rtl2spec/run-plugin-check.sh` PASS(含⑤(a)..(d))、`bash plugin/rtl2spec/run-mcp-e2e.sh`
  PASS(含⑧)、`bash plugin/rtl2spec/run-gap-fixtures.sh` PASS(S2 規則面矩陣結果與改前逐條相等);
  REGISTRY D-r1a 行追加關閉注記(日期/C-S1/上述命令與普查口徑)。

**D-r1b(§18.2 S3 對賬數字口徑)**——本節即勘誤表;§18.2 原文不改(歷史快照保留,M8「歷史記載
保留」同款處置),讀者以本表為準。

| §18.2 原文 | 實測【實跑】(`bun /tmp/c-arc-plan/stats.mjs`,讀 HEAD 4922352 的 golden 37 檔 rules.json) | 裁定 |
|---|---|---|
| :1945「37 檔 Σrules=838」 | **843**(sequence 539/relation 196/guard 85/transition 23;truncated=0) | 訂正為 843;838 不可復現(ask 材料所引 838 同此訂正) |
| :1929「guard+async=12」 | async:true **59**、async:false 26(85 條 guard 全帶 async 欄) | 單切片口徑不可復算→歷史保留;累計口徑以 59 為準 |
| :1930「敏感列表證=29」(復審 RV1-9 訂正行號:初稿誤標 :1929) | guard 帶 ≥2 條證據(敏感列表第二證)**85/85** | 同上;以 85 為準 |
| :1930「seq+when=296」 | 帶 when 的 sequence **476** | 同上;以 476 為準 |
| :1930「去重復活=175/零漂移=261」 | 無對應可復算量(golden 的 git 史僅 ec9527f/1361ebc 兩點【實跑 `git log -- plugin/rtl2spec/golden`】;build/golden-current 已於 2026-11-06 被覆蓋【材料】) | 口徑未注明·不可復算·歷史保留 |
| 累計(v0.2.0→v0.3.0) | v0.2.0(`git archive ec9527f` 解到 /tmp)【實跑】=630(seq 361/rel 161/guard 85/trans 23;async/2 證/when 皆 0)→**+213(seq +178、rel +35)** | 唯一可實測重算的口徑 |

- 防復發(「計數一律實測」機械化):新增 `plugin/rtl2spec/scripts/golden-stats.mjs`(唯讀,單行輸出
  `STATS rules=… seq=… rel=… guard=… trans=… async_true=… guard_2ev=… seq_when=… truncated=…`),由
  run-golden-regression.sh 尾部打印(資訊行,不斷言;斷言仍是 74 份 byte-exact);此後文檔對賬數字
  一律引該行原文。
- 驗收:在 D-r1c 修復**之前**跑一次 golden-stats,輸出與上表 HEAD 列逐值相等;D-r1c 重錄後的位移值
  記入本章實作記錄,禁回改本表;REGISTRY D-r1b 行追加關閉注記指向本節。

**D-r1c(復位前置敏感列表 async 分類失明)**
- 修法(**復審 RV1-1 修訂:兩處同改**):規則面 sv/mod.rs:752 起的 trigger 選取與**破口面
  sv/gaps.rs:192-217 的復位臂識別**(後者註釋 :194-195 自述「與規則提取面同一判準」,同取首沿;19.0-10)
  一律改為「先由頂層 If 求復位候選訊號(`reset_cond_signal`,:1024),trigger=敏感列表中**第一個訊號
  ≠復位候選**的 posedge/negedge;無此邊沿時回退首個邊沿」,抽成共用函數 `select_trigger(edges,
  reset_sig)`(位於 sv/ 下單一模組,mod.rs 與 gaps.rs 皆只調它;C-S2 tb_plan 的時鐘識別復用同一函數
  =單一語義源)。回退語義保證既有形零漂移:單沿 `@(negedge rst_n) if (!rst_n)` 仍 is_trigger→無
  復位臂。只修規則面會使規則書稱「有 async guard」、破口報告稱「邊沿行程完全無復位臂」(G4 certain)
  兩面自相矛盾而 D-r1c 被記為收平(看起來綠、語義錯),故兩面同片同改。RTLIL 面 rtlil.rs:197-209
  =潛在同形第三處:本弧**不改**,理由=無現形樣(19.0-10:yosys 產兩序 .il 皆 0 規則),改動無判別力
  fixture 違「語料語義面改動配正負 fixture」;登記於 REGISTRY D-r1c 關閉注記(潛在面)。
- 受害面普查【實跑】(口徑=釘選 37 檔文本 regex `always @( (posedge|negedge) X (or|,) (posedge|negedge)`):
  **11 檔 18 行**含雙沿 always;復位前置**僅 mem_arst.v:20**;其餘 **10 檔**時鐘前置(含 fsm.v:16——
  位於未定義的 `` `ifdef ASYNC_RESET `` 分支內,文本命中但不入解析)。REGISTRY:208「其餘 11 檔」多計 1,
  關閉注記按本口徑訂正為 10。現形=兩面:規則面 mem_arst.rules.json 4 條全 sequence、零 guard;破口面
  mem_arst.gaps.json 含 `G4-MyMem-001` certain(假陽性)。
- fixtures(plugin/rtl2spec/fixtures/,自著 MIT;**R1-2 修訂**:五檔規格逐檔以 dist wasm 實探現行行為
  定案,探針見 19.15 R1-2 與 19.16 RV1-1):`rst_first_async.v`(正樣:`always @(negedge rst_n or
  posedge clk) if (!rst_n) q <= 0; else q <= d;`→修前=零 guard[bug 現形,實探 sequence×2];修後=
  guard×1、async=true、trigger=clk posedge)/`rst_first_twin.v`(同模組改時鐘前置序:修前修後皆
  guard×1 async=true trigger=clk posedge[實探];修後與正樣 **check 載荷相等,evidence 文本與行號不同**
  ——guard 的 evidence[1] 是敏感列表原文,孿生=`  always @(posedge clk or negedge rst_n)`【實跑】,與
  復位前置形必然不同;**復審 RV1-10 訂正**初稿「僅 evidence 行號不同」=照字面寫斷言必紅)/
  `rst_first_compound.v`(負樣:雙沿但頂層 If 條件為**非單訊號形**的複合式 `load && en`
  →reset_cond_signal=None→零 guard;修前修後同[實探])/`rst_first_loadsig.v`(零漂移釘:頂層 If 條件
  為單訊號 `load` 但不在敏感列表→照產 guard×1 when=load、async=false、trigger=首沿 rst_n negedge;
  修前修後同[實探]——本修法不改此類,初稿「雙沿但條件非敏感列表訊號→零 guard」與提取器實際行為
  (sv/mod.rs:784-802 is_trigger=false→照取復位臂)矛盾,已改寫)/`rst_only_edge.v`(負樣:單沿復位
  always→回退首沿→is_trigger→零 guard;修前修後同[實探],舊行為釘死)。
  **兩個矩陣各進五行**:①gap-fixture-check S2 規則面矩陣(上列 guard 期望);②**破口面檔級矩陣
  (復審 RV1-1 新增)**——期望 findings 以 `rtl_scan_gaps_sv` 實探定案【實跑】:rst_first_async 修前
  `G4 certain`(假陽性現形)→**修後零 G4**;rst_first_twin 零(修前修後同);rst_first_compound
  `G4 certain`(無單訊號復位臂=結構可證,修前修後同);rst_first_loadsig 零(同);rst_only_edge
  `G4 certain`(修前修後同;單沿「純復位行程」被報 missing_reset 屬既有行為,**零漂移釘≠正確性背書**,
  語義可議處記入 19.13 待決)。兩矩陣各自 selfcheck(正樣期望誤配負樣必紅);sv_rules.rs +2 單測
  (正樣+順序不變性,兩面各斷言一次;負樣內聯字串)。
- golden(**復審 RV2-4 修訂:對照物=改碼前的 golden/ 副本+修後默認模式 DIFF 清單,按 19.4.4 標準程序
  G0..G5**):G0 改碼前 `cp -R plugin/rtl2spec/golden plugin/rtl2spec/build/golden-pre-C-S1`(本片起點
  golden==HEAD,以 `git diff --quiet HEAD -- plugin/rtl2spec/golden` exit 0 佐證其時代);G2 修後默認模式
  `DIFF` 行的檔名集合必**恰為**{yosys-simple/mem_arst.rules.json, **yosys-simple/mem_arst.gaps.json**
  (復審 RV1-1 放寬)};G3 `--record` 後 `golden-leafdiff.mjs` 對 G0 副本的變更檔⊆上列∪{PROVENANCE.md};
  任何他檔變更=停弧排查(10.5)。因 G0 時代=HEAD,本片 `git diff --stat plugin/rtl2spec/golden` 必與 G3
  變更檔集一致(交叉核對;C-S2 起不再成立)。初稿「先跑一次默認模式得 build/golden-current(=錄前快照)」
  作廢:默認模式寫入的是**當前提取器**輸出(golden-check.mjs:231-246),改碼後跑出的即修後結果、與隨後
  `--record` 寫入的內容相同,比對恆零差(run-golden-regression.sh:56-70 還會先重建陳舊 pkg)。逐條對賬
  預期:rules 面復位臂轉 guard、async=true、trigger=Clk_i(R001 的 `'bx` 值如實保留——X 值規則是提取
  事實,mem_arst 不是本弧 TB 樣);gaps 面 `G4-MyMem-001` 消失,其餘條目(如 `G3-MyMem-001`)不變,任何
  其他變化須逐條可解釋否則停(審方 /tmp 獨立復現:same 72 diff 2,只變這兩檔,G3 保留、G4 消失)。
- 連帶重生成:`bash plugin/rtl2spec/run-realworld-eval.sh`(docs/RTL2SPEC-EVAL.md 機械再生成)與
  `bash plugin/rtl2spec/run-rtl-validation.sh`(13 檔驗證集不含 mem_arst,run-rtl-validation.sh:22-34【檔】);
  對賬=**易變行白名單外的變更逐行歸因(RV2-9 訂正初稿「只許 sha 戳行變」——EVAL 不可滿足)**:白名單=
  兩頁的 `- 日期:` 行(EVAL :3/validation-report :3)與 `- 工具鏈:` 行(兩頁 :5,含 wasm sha)、EVAL 的
  `- 耗時:` 行(:6)【實跑 sed -n 1,8p】——審方材料:D-c2 補丁副本重跑後 EVAL 恰變 :3/:5/:6(耗時
  165ms→99ms)、validation-report 只變 :5(同日重跑,日期行未變)。本片白名單外 EVAL 只許 mem_arst 可歸因
  行(規則/破口/指紋計數),validation-report 白名單外零變。
- 驗收:`cargo test -p rtl-spec`(+2 測)、run-gap-fixtures PASS(規則面+5 行、破口面+5 行)、
  run-golden-regression PASS(重錄後)、上述 diff 範圍斷言、`bun plugin/rtl2spec/scripts/probe-determinism.mjs`
  37/37、REGISTRY D-r1c 關閉注記(含「10 檔」訂正、兩面同修、RTLIL 潛在面)。
- **C-S1 同片登記(10.5 先登記再繼續;復審 RV1-4/RV2-2/RV2-7)**:REGISTRY 新增「C 弧已知未決」小節,首錄
  **D-c1**(net 宣告賦值未提取,cnt_end;19.0-3)、**D-c2**(ANSI 端口宣告範圍/符號/種類跨組繼承;19.0-11
  全部證據,含 partsel signed 面與 retime 非釘選面)、**D-c3**(corpus_lint R1 同行 `#` 不關窗=換行形假陽性,
  corpus_lint.rs:80-95;19.3.3 TB-R1x 條實跑證據;本弧不修 Rust 側)、**D-c4**(contract 對負數下標定寬
  範圍報 `width 0+parametric`,partsel_test006/007 `[31:-32]`;tb_plan 對其 TB1=安全側,本弧不修)——
  D-c2 的修復在 C-S2 首步(19.3.2),登記必須先於修復。

C-S1 不 bump 版本:插件面僅缺陷修復,CHANGELOG `[0.4.0]` 的 Fixed 段於 C-S2 一併登錄(弧內禁
commit,0.3.1 與 0.4.0 兩個未發布版本並存無意義)。

### 19.3 TB 生成器(規則書→testbench;D-C2/D-C3)

#### 19.3.1 雙腿分工

| 腿 | 執行者 | 產物 | 可復現性 | 進閘方式 |
|---|---|---|---|---|
| **T 模板腿(確定性)** | rtl-spec 核心 `tb_plan`+`tb_template_v`(新能力先落核心,10.7/10.11;wasm `rtl_tb_plan_sv`、MCP `rtl_tb_plan` 露面) | 完整可跑 TB:harness+種子化 LFSR 激勵+由規則書導出的復位/異步復位場景 | byte 級確定(同 wasm、同 seed/cycles/flush) | 入庫 tb.v=錄製快照;閘內進程內重生成 byte 比對 |
| **D 定向腿(LLM)** | 編碼 agent,按 CONFORMANCE-PROTOCOL.md(延續 REIMPL-PROTOCOL 形:模型身份/採樣參數/prompt 逐字/腿表 append-only) | 以 T 腿骨架為底,按 obligation 清單逐場景寫定向激勵 | 不可復現(溫度/seed 不可釘,REIMPL-PROTOCOL §0 同款誠實邊界) | 產物凍結入庫後才被閘消費;入庫判據=T 腿同一套+D 腿附加(19.4.5) |

- **降級形(無採樣條件=無可用模型/預算)**:D 腿標「環境受限未跑」,全部設計 tb.v=T 腿產物,閘照常
  全量——**確定性腿不依賴任何 LLM 產物即可跑通採樣集全量**(C-S3 驗收面)。
- T 腿同時是 D 腿的**基線腿**:tb.v 為 D 腿時,閘另把 T 腿模板重生成到
  build/conformance/<design>/tb_template.v 並跑同一變異矩陣,兩腿殺傷數字分列(D 腿增益=D−T,禁合併)。

#### 19.3.2 核心 tb_plan(C-S2;crates/rtl-spec/src/tbplan.rs)

- **C-S2 首步=D-c2 修復(復審 RV1-4/RV2-1;tb_plan 直讀 contract 的前提;ADR-0012 D1 顯式修訂,見其
  實作記錄修訂指針第三條)**:crates/vl-syntax/src/parser/module.rs 的 ANSI 端口組解析——**開新組的判準
  =本組含任一宣告限定詞**(方向 input/output/inout、種類 reg/net 關鍵字、signed、`[範圍]`;即
  peek_starts_port_spec 命中,module.rs:196-197 的 `continue 'groups` 點);新組**重置全部四個滾動變量**
  cur_net/cur_reg/cur_signed/cur_range,**僅方向跨組繼承**(前組有範圍且本組有方向時=新宣告,LRM/IEEE
  1800-2017 §23.2.2.3)。oracle 證據(19.17 RV2-1/【實跑】/tmp/c-arc-r2/ansi 七向量,iverilog 13.0
  -g2005/-g2012+verilator 5.052 三方一致):

| 向量 | 宣告 | oracle 正解 | HEAD(dist wasm)實錯 |
|---|---|---|---|
| v1 | `input [3:0] a, b, c, output d` | a=b=c=4, d=1 | d=4 |
| v2 | `output reg [3:0] count, output reg busy` | count=4, busy=1 | busy=4 |
| v3 | `input [7:0] a, input b` | a=8, b=1 | b=8 |
| v4 | `input [7:0] a, wire b`(iverilog -g2005 報「Partial ANSI … requires SystemVerilog」,-g2012 接受) | a=8, b=1 | b=8 |
| v5 | `output reg [7:0] a, reg b`(同 v4 的 -g2005 行為) | a=8, b=1 | b=8 |
| v6 | `input signed [7:0] a, [3:0] b` | b 無號,b=15(`b < 0` 比較探針為假) | b signed=true |
| v7 | `input signed [4:0] n, output reg [31:0] dout`(=partsel_test004 形) | n signed, dout 無號 | dout signed=true |

  **同案文件面(復審 RV2-1;D1 修訂指針)**——api.rs:128 釘的正是缺陷語義
  (`sv_ansi_inheritance_leg`,註釋 :119-121 稱「依 LRM §12.3.3 繼承前組」,:128
  `assert_eq!(port("d").width, 4)`【檔】;實跑:對 /tmp 補丁副本 `cargo test -p analyzer-core --test api`
  →`sv_ansi_inheritance_leg FAILED, left: 1 right: 4`;按 oracle 意圖把該測試期望與 :119-121 註釋、
  module.rs:139-140 繼承註釋、ast.rs:97(`None entries inherit` doc)、analyzer-core/src/sv.rs:132-134
  註釋、api.rs:58 同族措辭一併改寫為正確語義——oracle 贏 LRM(AGENTS §1))。vl-syntax 是 rverilator/analyzer-core/rtl-spec
  共用前端,D1「禁破壞其測試與 API」的約束按「測試釘錯了 oracle 語義→同片改測試+修訂指針」處理,不是
  退回修法。**定量擴散面(復審 RV2-2;19.0-11)**:釘選 37×2 same 70 diff 4 檔 24 葉
  (loops/memory/mem_arst/partsel[16 葉 signed])+L1 非釘選 retime;corpus/ 96 檔與 sv-tests 260 檔零差。
  **驗收順序(復審 RV2-3 修訂:先重錄後 regress——regress S7 內含 run-golden-regression
  (regress.sh:383-386),golden 未重錄前 regress 必紅;審方副本實跑 `GATE FAIL: rtl2spec golden-regression`)**
  (19.4.4 G0..G5 標準程序,本片實例化如下)G0 改碼前 `cp -R plugin/rtl2spec/golden
  plugin/rtl2spec/build/golden-pre-C-S2` →G1 補丁(vl-syntax module.rs+四處註釋/doc+api.rs 期望與註釋;
  實作形=四個滾動變量在組循環頂重新綁定,不保留「先初始化後覆寫」的寫法——/tmp 探針補丁那樣寫會報
  unused_assignments,違 AGENTS §8 零 warning)+vl-syntax 新增 ANSI 前端單測(crates/vl-syntax/tests/
  frontend.rs,七向量斷言到 width/signed/parametric 投影;iverilog -g2005 拒 v4/v5/v6 的行為注記為模擬器
  語言模式差異,不進斷言)+`cargo test --workspace` 全綠(/tmp 副本實跑:補丁後 `cargo test --workspace
  --no-fail-fast` 恰兩敗=api.rs:128(預期)+riv-gen roundtrip;前者改期望後 `-p analyzer-core --test api`
  13/13,後者因 fresh 副本缺 target/debug/lib*.rlib,補建對應 lib 後 1/1——審方稱 CARGO_TARGET_DIR 假象=
  同一根因;合併推斷修後全綠,倉庫本體未跑)→G2 修後默認模式 `DIFF` 行檔名集合**恰為**
  {loops,memory,mem_arst,partsel}.rules.json(gaps 37 檔零變【實跑】)→G3 `--record` 後對 G0 副本逐葉恰
  24 葉(loops 4/memory 2/mem_arst 2/partsel 16)+PROVENANCE.md→G4 默認模式 exit 0→G5 連帶重生成
  (19.2 易變行白名單外零變:validation-report 只變白名單行、EVAL 只變 :3/:5/:6——審方材料)+**最後**
  `bash scripts/regress.sh` 全綠且基線行逐字不變(E2E 59/59、riv 兩臂、lint parity、ryosys 469/479+LIVE
  456/456、analyzer smoke——審方副本實見前四類與 analyzer smoke 不變、LIVE 因副本缺 vendor-reference 未驗;
  corpus/ 96 檔與 sv-tests 260 檔 `analyze` 實驗零差【實跑】;任一位移=停弧歸因)→REGISTRY D-c2 關閉注記。
- API(ADR-0012 D12 延續:庫內零 JSON 讀取、收標量;**復審 RV1-11 修訂**:計劃期錯誤改 Result):
  `tb_plan(input, module: Option<&str>) -> Result<TbPlan, TbError>`(內部以 DEFAULT_MAX_RULES 提取,
  不另開上限參數——D-r1a 教訓:少一個可漂移的旋鈕;規則書 truncated 非空=TbError,防計劃靜默殘缺);
  `tb_plan_json(&TbPlan) -> String`;`tb_template_v(&TbPlan, &TbOpts) -> String`(不可失敗:全部失敗
  模式在 tb_plan 期攔下);`TbOpts{seed,cycles,flush}` 默認值 SSOT=tbplan.rs 常量(seed=非零常量,
  xorshift32 不收 0;cycles=每個隨機場景的拍數;flush=默認 8)。`TbError` 的 Display 一律
  `error: <原因> (milestone TB1)`。
- wasm 導出(**復審 RV1-11;RV2-8 補 rtlil 面**):`rtl_tb_plan_sv(name, source, module: &str, seed: u32,
  cycles: u32, flush: u32) -> String`——`module==""`=未指定(單模組自動選取,多模組=TB1);`seed/cycles/
  flush==0`=核心默認(與 rtl_extract_rules_sv 的 `max_rules==0` 同約定,lib.rs:49-54);回傳 JSON 二形:
  `{"ok":true,"plan":{…},"template_v":"…"}` 或 `{"ok":false,"error":"error: … (milestone TB1)"}`
  (錯誤走資料、不 trap)。**`rtl_tb_plan_rtlil(同參)`**=核心對 RTLIL 輸入恆回 TB1 錯誤(硬錯文案由核心
  產生,server 不寫語義);wasm 導出 7→9。MCP `rtl_tb_plan` 收 path|source/name/**lang**/module?/seed?/
  cycles?/flush?——lang 分派沿既有 `langOf`(server.mjs:77-82;顯式 lang 優先,否則 `.il`→rtlil、其餘→sv),
  與其餘四工具同一分派、零新增語義;缺省一律傳 0/空串(面委託核心,零常量複製);ok:false→isError:true
  原文轉出。初稿無 lang 參數且只調 `rtl_tb_plan_sv`,`.il` 會被當 SV 解析、「RTLIL 輸入」TB1 在 wasm/MCP
  面不可達(復審 RV2-8),已改。native 對拍腿:crates/rtl-spec/examples/emit.rs 增 `tbplan` 模式(`cargo
  run -q -p rtl-spec --example emit -- <sv|rtlil> tbplan <name> <file> [module] [seed] [cycles] [flush]`,
  輸出同一 JSON)。
- TbPlan 全部由規則書與 contract 導出(Vec 按規則序;禁 HashSet/HashMap 迭代面進輸出,§16.1.5
  契約③):module/params/ports(contract 直讀);**clocks**=產生 ≥1 條規則的邊沿行程經
  `select_trigger` 得到的訊號+極性;**resets**=guard 規則的訊號+有效電平(由 when 是否否定)+async+
  rule_ids;**sens_edges**=DUT 全部沿行程敏感列表的 (沿, 訊號) 有序去重表(按行程出現序;TB-R1x 的 E,
  復審 RV1-5);data_inputs=輸入端口−時鐘−復位;outputs;**obligations**=每規則一條(rule_id/form/
  category/observe=port|indirect——**R1-9 修訂**:`port`=規則主體是輸出端口,可直接打印;`indirect`=
  主體是內部訊號,其效應只能經下游輸出端口表面化——TB 永不層次引用內部訊號[19.3.3 靜態約束],
  無法表面化者以 `WAIVE <id> unobservable` 宣告;模板腿對 indirect 規則一律以非定向場景 `*` 覆蓋;
  初稿 `internal` 措辭有誘導層次引用之嫌,已改名);**waivers**=值/becomes/rhs 含 x/z 位字面量→
  `x-valued`;scenes(場景表+COVERS)。
- resets 導出口徑(**R1-11 修訂**):v1 **只**由 guard 規則導出,不按端口名/非 guard 規則推測復位(禁
  啟發式猜測);resets=∅ 時不生成 S-RST/S-ARST,guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)亦不產生,
  復位路徑只經隨機場景激勵——採樣集 fsm 即此形(golden 23 條=trans 5/seq 18 零 guard【實跑】;其同步
  復位 `if (reset) state <= 100;` 不在頂層,落為 sequence 規則 R011【實跑 19.15 R1-11】)。證據頁與
  verdict 對 resets=∅ 的設計**機械註記**「S-RST 未生成(規則書無 guard)」及其殺傷率的機械成因,防低
  殺傷率被誤讀為規則書或設計缺陷(與禁止宣稱④呼應);定向腿(fsm 在 tbSample 內)補強且分列。
- 模板場景類(確定性):**S-RST**(每個復位:negedge 處置有效 2 拍→釋放→觀測 4 拍;COVERS=該復位全部
  guard 規則)/**S-ARST**(async=true 的復位:兩沿之間於 negedge 後 #2 拉起、#1 後立即觀測、再釋放——
  專打「只在 posedge 後採樣」盲區,對應 19.0-5 counter8 `async` 存活變異)/**S-FULL**(全寬 LFSR
  每拍)/**S-SMALL**(多位輸入遮罩到 3 位——prescale/mode 類量值敏感輸入的廉價覆蓋)/**S-HOLD**
  (每 8 拍換值,讓計數/握手類行為走完);純組合(無時鐘):**S-EXH**(Σ輸入位 ≤12 窮舉,否則 LFSR
  256 向量)+S-SMALL;無復位時序設計:首次觀測前驅動 flush 拍(默認 8,case.json 可調;X-clean 由
  Q4 兜底)。LFSR=TB 內 32 位 xorshift 函數(**禁 $random**:跨模擬器序列不保證一致)。
- 硬錯(AGENTS §3,`error … (milestone TB1)`;**復審 RV2-8 補全清單**):①解析錯誤或零模組(前端診斷
  原樣轉出——前端已帶 `(milestone Mx)` 者保留其 Mx,否則標 TB1)②`module` 參數指定的模組不存在③多模組
  未指定 module④parametric 端口(contract 無數值寬度,§11.5 求值禁區)⑤contract 端口 dir=inout(含
  analyzer-core 的孤兒佔位:兩腿皆無方向證據→Inout+width 0,sv.rs:132-134【檔】)或 width==0 且非
  parametric⑥零輸出端口⑦多時鐘(select_trigger 得到的時鐘訊號 ≥2 個)⑧時鐘或復位訊號不是輸入端口
  (內部生成)⑨同一訊號兼作時鐘與復位⑩規則書 truncated 非空⑪RTLIL 輸入(經 rtl_tb_plan_rtlil 可達)。
  影響面【實跑】:釘選 37 檔 golden 的 guard trigger 零多時鐘、零內部時鐘(/tmp/c-arc-r2/clk.mjs),
  yosys-simple 87 檔零 `inout`(grep -lw)——只影響面向用戶的工具面,不影響 19.7 六個採樣設計。
- 確定性與 parity:probe-determinism 同款進程內 12 跑(6 採樣設計×plan+template)byte 全等;
  `bun crates/rtl-spec-wasm/smoke.mjs`(與 node)新增 tb_plan 兩路 wasm↔native byte parity
  (wasm=rtl_tb_plan_sv/rtl_tb_plan_rtlil,native=emit `tbplan` 模式;三路=fixtures/fsm_counter.v+一個
  TB1 負樣+一個 .il 輸入[TB1];smoke 非總閘成員,C-S2 驗收命令點名手跑)。
- **contract 寬度=TB 可編譯性的前提(復審 RV1-4)**:tb_plan 直讀 contract 端口寬度,寬度錯=TB 端口
  錯配(iverilog 報「Port … expects 1 bit(s), given 4」類警告,Q1 必紅;parametric 誤標還會誤觸 TB1)。
  機械防線兩層:①tbplan 單測對 fsm_counter 形內聯模組斷言 `busy` 寬 1(D-c2 修後才綠,不依賴外部
  工具);②mcp-e2e ⑨ 把 `rtl_tb_plan` 對 fixtures/fsm_counter.v 回傳的 template 落到 build/ 暫存檔,
  與該 fixture 一起 `iverilog -g2005 -t null`,斷言 exit 0 且 warning=0(iverilog 在場才執法,不在場
  打印 SKIP 行——不讓 mcp-e2e 的雙守衛在 regress 內新增硬依賴)。

#### 19.3.3 TB 協議 v1(兩腿共守;閘機械執法)

```text
stdout 行文法(正規化後):
  SCENE<n> <name>                  n=1..N 連續;name=[a-z0-9_]+
  "  "<key>=<val>( <key>=<val>)*   key=標識符;val=%h 或 %0d,禁 x/z
  DONE                             恰一次,最後一個協議行
正規化=丟行白名單,按模擬器分腿(禁內容改寫,10.5/10.8 同律;R1-1 修訂補 verilator 腿):
  iverilog 腿:^[^ ]+:[0-9]+: \$finish called at [0-9]+ \([0-9]+[munpf]?s\)$  與  ^VCD info: 
  verilator 腿:沿 vl-difftest 既有契約 normalize_oracle_stdout(crates/vl-difftest/src/lib.rs:124-133)
    的四個精確前綴 ^-␠ 、^-Info 、^-Warning 、^-Error(␠=一個空格;該契約由 corpus_lint.rs:185-188
    敵意測試釘死,`-7div2=-3` 設計行保留)——吃掉 `- tb.v:N: Verilog $finish`/`- S i m u l a t i o n
    R e p o r t…`/`- Verilator: $finish at …; walltime …`/`- Verilator: cpu … allocated … MB` 四行
    (內嵌 walltime/cpu/MB 機器相關值);
  其餘一律保留。防誤吞論證:協議 v1 的合法行(SCENE/兩空格縮進 kv/DONE/TIMEOUT/WATCHDOG)無一以
  "-" 開頭,且 Q5 文法檢查作用於 iverilog 腿輸出(該腿不丟 "-" 行)——TB 若打印任何 "-" 開頭行,
  iverilog 腿必 Q5 紅,故 verilator 腿丟 "- " 前綴不可能吞掉協議合法行。
TB 源碼機讀註釋:
  // TB-LEG: template tb_plan-v1 seed=<s> cycles=<c> flush=<f> | directed <協議腿 ID>
  // SCENES: N
  // COVERS: <n>: <rule_id>[,<rule_id>…]   或   // COVERS: <n>: *   (*=非定向場景)
  // WAIVE: <rule_id> <x-valued|unobservable|tb-internal>
靜態約束:DUT 以 contract 模組名實例化,具名連接覆蓋全部 contract 端口;禁層次引用 DUT 內部;
  禁 $random/$urandom;輸入只在 DUT 不採樣的邊沿或「@(<沿> ..); #1;」之後改變(TB-R1x,定義見下);
  等待迴圈必有上限,逾時打印「TIMEOUT <tag>」;內部看門狗「#W $display("WATCHDOG"); $finish;」。
TB-R1x(跨檔、極性感知;復審 RV1-5 修訂——corpus_lint R1 是同檔、不分極性的文本規則
  [corpus_lint.rs:30-51 edge_read_vars 只收同檔沿行程標識符;:53-110 任何 posedge/negedge 後的阻塞寫
  皆判],只對 tb.v 移植=永不命中 DUT 輸入競爭,跨檔照搬又會誤判協議允許的 @(negedge clk) 驅動):
  E=DUT 的沿集合 {(沿, 端口)},取自 tb_plan 的 `sens_edges` 欄(核心枚舉 DUT **全部**沿行程敏感列表的
  (沿, 訊號),不限產規則者;閘側不另解析 DUT 文本);M=tb.v 具名連接 `.port(net)` 的 net→port 映射。
  違例 ⇔ tb.v 中事件控制 `@(<e> <net>)`
  之後的語句窗內,存在對某 TB 變量 v 的阻塞賦值,且 v 經 M 連到 DUT 輸入端口,且 (e, M(net)) ∈ E。
  `// race-ok` 豁免沿用。TB 內部若另有沿行程,同檔 R1 規則照跑。
  **語句窗(復審 RV2-7 定案:取 corpus_lint 文檔語義,不取其代碼行為)**:窗=事件控制右括號之後的語句,
  止於**首個** `#`、下一個 `@(` 或以 `end` 起首的行——**同行出現 `#` 即關窗,不再延伸到後續行**
  (corpus_lint.rs:3-6 文檔:「with no intervening `#delay`」)。corpus_lint.rs:72-107 的代碼與其文檔
  不符::80-95 的 `closed` 是空操作,同行 `#` 後窗口仍併入後續行。實跑(把 race_findings 原文
  corpus_lint.rs:14-131 編入 /tmp/c-arc-r2/lint,rustc):`@(posedge clk); #1;`+換行 `ld = 1;` 報 1 條
  (文檔語義下應為 0=假陽性);同行 `@(posedge clk); #1; ld = 1;` 0 條;`#1;` 獨佔一行 0 條;
  `@(posedge clk); ld = 1;` 1 條;NBA 0 條;`@(negedge clk); ld = 1;` 1 條(不分極性)。因而:同檔 R1 與
  跨檔 R1x 的 JS 實作都按文檔語義;與 Rust corpus_lint 的 parity **只聲明於三個同行向量**
  (corpus_lint.rs:175-180),換行形=已登記差異(REGISTRY D-c3,C-S1 登記;Rust 側本弧不修,修否見
  19.13-8),selfcheck 以該換行形向量斷言 JS 側零違例(防誰照 Rust 代碼移植)。
```

#### 19.3.4 定向腿協議(C-S4 落 plugin/rtl2spec/CONFORMANCE-PROTOCOL.md;模板本節預先登記)

章節沿 REIMPL-PROTOCOL.md:§0 模型身份/採樣參數/執行日期/污染聲明(開弧釘檔;本規劃 session
模型=GLM-5.3-Flash,實施弧若不同則記實際;溫度/seed 不可釘→樣本差異由 `{K}` 驅動;**TB 撰寫者
若曾讀過原版源碼=盲隔離弱,如實記錄**)/§1 TB 生成模板/§2 TB 改進輪模板/§3 重寫迭代輪模板/
§4 腿表(append-only:腿 ID/設計/類型 tb|rewrite/輪次/K/N/模型/日期/產物路徑/入庫否/數字)/
§5 機械執行。三個模板在本規劃輪預先登記(先於任何樣本產生=防事後調 prompt),C-S4 逐字轉入。
**時間戳邊界(R1-10 修訂)**:弧內禁 commit,工作樹階段的預登記只有**內容錨**、沒有不可抵賴的時間
戳——內容錨=下方模板塊 sha256 `cc28f0c73dc4ef3afc1838b839d44903d7dc9ee9b9f15b5ebd3aceaca532a941`
(取法見 19.15 R1-10);不可抵賴時間戳=用戶提交本章的 commit 時刻。CONFORMANCE-PROTOCOL §0 必載:
「模板以 §19.3.4 首次提交版為準(`git log -S` 定位該提交);C-S4 開工時重算模板塊 sha256 須等於上述
內容錨;任何改動=協議事件,append-only 記新版本號/日期/理由,舊版保留」;若 C-S4 開工時本章尚未
提交,§0 如實記「預登記未提交,僅內容錨」。

```text
[§1 TB 生成模板]
你是驗證工程師。以下是某個 Verilog 模組的「行為規則書」(由規則提取器從一個你不曾見過的 RTL
檔案自動生成)、一份機械生成的測試計劃 JSON 與一個可編譯的 testbench 骨架。請**只依據規則書與
計劃**寫一個 Verilog-2005 testbench,寫到檔案 {OUT}。
規則書:
{RULEBOOK_MD}
測試計劃(tb_plan):
{TB_PLAN_JSON}
骨架(必須保留其實例化、時鐘、看門狗與輸出協議):
{SKELETON_V}
硬性要求:
1. 輸出協議 v1:每個場景先打印「SCENE<n> <name>」(n 從 1 連續),場景內只打印「  key=value」行
   (值用 %h 或 %0d),最後打印「DONE」;檔頭寫「// SCENES: N」。
2. 每條規則至少被一個場景以「// COVERS: <n>: <rule_id>,...」宣告覆蓋,或以「// WAIVE: <rule_id>
   <x-valued|unobservable|tb-internal>」宣告豁免。
3. 只觀測 DUT 端口;禁止層次引用 DUT 內部;禁止 $random/$urandom(偽隨機用骨架內 xorshift 函數);
   輸入只在時鐘下降沿或「@(posedge clk); #1;」之後改變。
4. 禁止寫任何期望值或自檢斷言——期望輸出由參考實現錄製,你只負責激勵與觀測;禁止在復位前或
   流水線填滿前觀測。
5. 每個等待迴圈必須有上限,逾時打印「TIMEOUT <tag>」後繼續;不得刪除骨架的看門狗。
6. 本樣本=第 {K}/{N} 個;與前一樣本拉開場景組織差異。不得請求或引用規則書之外的任何源碼。

[§2 TB 改進輪模板]
你上一版 testbench(附檔 {PREV_TB})在變異分析中未能區分下列規則的變異實現(僅列規則書內容):
{SURVIVORS_MD}
請在保持協議 v1 與全部既有場景不變的前提下追加場景,使這些規則描述的行為可被觀測區分;新增
場景以「// COVERS:」宣告。這是第 {R}/3 輪。

[§3 重寫迭代輪模板](接在 REIMPL-PROTOCOL §1 原模板之後)
你上一版重寫(附檔 {PREV_V})在公共 testbench 下與參考行為不一致,診斷如下(含參考輸出的首個
差異行):
{DIAG_MD}
請只依規則書與本診斷修正,保持接口契約不變;每處修正以「// FIX(r{R}):」行內註釋標明;規則書
未覆蓋處仍以「// ASSUMPTION:」聲明。這是第 {R}/3 輪。
```

- 樣本選擇規則(預先登記,防挑最好樣——MVP-NEXT 風險 4):D 腿主 TB=**K=1 樣本**經 ≤3 輪改進後的
  版本;K=2,3 只作離散度報告(輔助)。

### 19.4 判別力雙 fixture 與 TB 質量判據(D-C4/D-C5/D-C6)

#### 19.4.1 正樣 fixture
- `expected.txt`=原版 RTL+tb.v 經 `--record` 錄製的正規化輸出(錄製 cwd/絕對路徑不入期望,10.8
  機器無關律);閘內原版跑 **2 次** byte 相等(確定性)且==expected.txt(正樣過)。
- verilator 在場時再跑 `--binary --timing -Wno-fatal`(DEVELOPER_DIR 三行守衛逐字沿 regress.sh:12-14;
  `-Wno-fatal` 理由見 Q7/R1-12),兩腿各按 19.3.3 本腿白名單正規化後須相等(跨模擬器=X 依賴/競爭的
  第二證);在場但構建/運行失敗=紅(可診斷,不靜默),不在場=打印 SKIP 行。

#### 19.4.2 變異樣 fixture(變異怎麼造)
- **規則錨定(復審 RV2-5 修訂:按規則主體定行)**:候選區=規則 evidence[0] 的行區間(V4 鎖保證 snippet
  與源檔逐位元組相等);**錨定行=候選區內「賦值左值經 norm 後==規則主體」的唯一一行**——主體=guard 的
  **每個** effect.lhs(一個 effect 一個錨)/sequence 的 signal/relation 的 lhs/transition 的 state_reg
  (OP-STATE 另見表下注)。左值取法=去掉行首 `begin`/`end`/`else`/`if (…)`(括號配平)/case 標籤
  `<標籤>:` 後,取 `<=` 或 `=` 左側標識符(含下標;排除 `==`/`!=`/`<=`·`>=` 比較位),**阻塞 `=` 與
  非阻塞 `<=` 同等對待**;零匹配或多匹配=`ambiguous`(附原因)。手術=錨定行上目標 token **count==1**
  才替換(spec-i2c rtl_for 錨點手術形的加強版,AGENTS §7);RHS 一律取**源碼原文**,不用規則的渲染值。
  初稿「多行 span 按行序找第一個含目標 token 的行」作廢:guard 同行程共用整段 then 臂為 evidence[0]
  (i2c 20 條共用 :71-92、sincos 8 條 :54-63、subbytes 3 條 :30-34),本輪模擬(/tmp/c-arc-r2/anchor.mjs)
  i2c 20 條只落 7 行、值為 `1'b0` 的 11 條全落 :74(`half <= 1'b0;`——R007 rd_mode 的變異實際改的是
  half),sincos 6/8、subbytes 3/3 的渲染值(`'sd0`/`'sd1`)在源碼不存在=落空。按主體定行本輪實測六設計
  **全部唯一**:guard 20/20、8/8、3/3;sequence 123/123、5/5、18/18、38/38、3/3;relation 4/4、6/6、10/10;
  transition 的 state_reg 行 9/9、5/5;阻塞賦值錨占比=sincos guard 5/8、sequence 34/38,subbytes 16/16。
- **norm(單一定義;只用於「渲染文本↔源碼」比對——錨定行左值比對與 OP-STATE 的 `to` 歸屬)**:去空白+
  無位寬十進制字面量 N→`'sdN`(提取器渲染慣例)。本輪實測:六設計 guard/sequence 的源碼 RHS 經 norm 後
  與渲染值全等(i2c 20+123、sincos 8+38、subbytes 3+3、acc 5、fsm 18);relation 有 2 條(signedexpr)
  因渲染去括號而不等——relation 只用 lhs 錨定、不比 RHS,不受影響。
- 算子目錄 v1(按 form 選用;每規則每算子 ≤1 個、每規則 ≤2 個;變異鍵=(錨定行, 替換後整行文本),同鍵只
  生成一次):

| 算子 | 適用 | 例 |
|---|---|---|
| OP-POL 復位極性 | guard(evidence[0] 首行=復位條件行;同行程全部 guard 共用此行,天然共享) | `if (!rst_n)`→`if (rst_n)` |
| OP-ASYNC 異步→同步 | guard 且 async=true(evidence[1]=敏感列表行,§18 S3 第二證) | 刪去敏感列表中的復位邊沿項,**四形皆覆蓋**(R1-6 修訂):` or <沿> <rst>`/`, <沿> <rst>`/`<沿> <rst> or `/`<沿> <rst>, `——如 i2c `or negedge rst_n`、sincos.v:45 逗號形 `@(posedge clock, posedge reset)` |
| OP-RVAL 復位值 | guard 每個 effect 的錨定行(按 effect.lhs 定行),源碼 RHS 為單一字面量時 | 有位寬字面量值 +1 取模、保留位寬/基數寫法(`1'b0`→`1'b1`、`16'd0`→`16'd1`);無位寬十進制 `0`→`1`(sincos `cos_z0 <= 1;`、subbytes `data_reg = 0;` 這類渲染為 `'sd` 的值即此形);x/z 值=`n/a`(WAIVE x-valued) |
| OP-STATE 遷移目標 | transition | `state <= X;`→同一 state_reg 的另一個 RHS(枚舉源與序=表下注,R1-5 修訂) |
| OP-HOLD 更新丟失 | sequence 的錨定行 | `x <= rhs;`→`x <= x;` 與 `x = rhs;`→`x = x;`(**阻塞/非阻塞皆覆蓋**,RV2-5:sincos 38 條 sequence 中 34 條、subbytes 3/3 錨在阻塞賦值) |
| OP-ARITH/LOGIC/SHIFT | sequence/relation 錨定行的源碼 RHS | 加減互換、與或互換、異或→或、`<<` 與 `>>` 互換、`>>>`→`>>` |
| OP-CMP | 條件/比較 | `>=`→`>`、`==`→`!=`(只動賦值運算子右側;非阻塞賦值的 `<=` 不動) |
| OP-CONST | 數值字面量 | 值 +1(按位寬取模) |
| OP-SIGN | 帶號字面量 | `3'sd0`↔`3'd0`(AGENTS §5.4/§5.5 位寬符號傳播面) |
| OP-SLICE | 部分選擇 | `[8:1]`→`[7:0]` |
| OP-NOP(控制組) | 任一證據行 | 行尾追加 `// conformance-nop`,必存活 |

- **OP-STATE 確定性定義(R1-5 修訂;§16.1.5 契約③同律,禁 HashSet/HashMap 迭代面)**:枚舉源=**源碼**
  而非渲染文本——在同模組、同 state_reg 的全部 transition 規則 evidence 區間內,收集匹配
  `^\s*<state_reg>\s*<=\s*([^;]+);\s*$` 的行的 RHS 原文(去首尾空白),按**行號升序首次出現**去重成
  有序表 C(Vec);對其中每一行 L(RHS=r),替換值=C 中 r 的下一個元素(循環;跳過與 r 相同者);
  |C|<2→`n/a`。歸屬:`to` 與 r 規範相等的 transition 規則(規範=去空白+提取器把無位寬十進制字面量
  渲染為 `'sd<N>` 的單一映射,如 fsm `200`↔`'sd200`【實跑 19.15 R1-5】);無唯一匹配→歸屬該區間全部
  transition 規則並記 `attribution:"span"`。C 逐 state_reg 錄入 record.json。
- **錨點落空一律留痕(R1-6 修訂,適用全部算子)**:按 form 可適用、但找不到 count==1 錨點者記
  `ambiguous`(附原因),按 form 不適用者記 `n/a`,兩類都寫入 record.json 並計入目錄覆蓋的分母——
  **禁靜默跳過**。
- **變異歸屬(復審 RV2-5 補;初稿只給 OP-STATE 定義了歸屬)**:每個變異的**歸屬集**=全部「錨定行==該
  變異所在行且該算子對其 form 適用」的規則(按規則序,Vec);同鍵去重後歸屬集取並集。按主體定行後
  OP-RVAL/OP-HOLD/OP-ARITH 類多為單規則歸屬;OP-POL/OP-ASYNC 天然歸屬同行程全部 guard(i2c 為 20 條);
  case 多標簽臂展開出的多條規則共用同一行時亦為多規則歸屬。歸屬集逐變異錄入 record.json(漂移=DRIFT)。

- **兩個變異池**:**P-rule**(上表,計入 TB 判別力分母)與 **P-unruled**(源檔中「含賦值/條件、但不在
  任何規則 evidence 區間內」的行套同一目錄——如 i2c_master.v:68 的 cnt_end 行;**不入殺傷率分母**;
  被殺=TB 可觀測而規則書未描述的行為=規則書缺陷的確定性證據,餵 19.6 F3)。規則錨定目錄天然看
  不到規則書沒覆蓋的行,P-unruled 專補這個盲區。
- 結果分類:`stillborn`(編譯敗,排除分母)/`killed`(任一正規化行與 expected 異,記殺傷場景)/
  `survived`/`timeout`(牆鐘逾時,記為 killed 的子類並單列)。存活者在 `--record` 時交 yosys oracle
  裁決(僅測試用、僅錄製模式;版本與命令模板記入 record.json):全部 proven→**equivalent**(轉控制組,
  閘內必存活),否則 **unadjudicated**(保守計入分母)。
- **複用與不複用(勘察 2/6 逐項裁定)**:複用=spec-i2c 錨點手術形(+count==1)、gap-fixture-check
  selfcheck 判別力自證形(正樣期望誤配必紅)、S6 selfcheck.mjs:54 的 `$finish` 行剝除寫法(協議正
  規化;**收緊為錨定 regex**——原寫法 `includes('$finish')` 會連帶吞掉含該字樣的設計輸出行,10.5
  「歸一化=精確前綴」)、S6 probe.mjs:57-66 的編譯守衛形(stillborn 分類)、corpus_lint R1 規則文本與三向量(僅作
  TB 內部沿行程的同檔規則+parity selfcheck;跨檔判定=TB-R1x 新定義,19.3.3,復審 RV1-5)。**不複用**=S6 transform 的 paren/reorder/rename 臂作 TB 控制組(port-only
  觀測下由模擬語義保證不變,對 TB 恆空真;rename 還需換端口 wrapper)、S6 digest 層作診斷(重寫的
  內部名沒有已知映射,S6 依賴 rename 臂記錄的映射)。S6 探針留 build/ 邊界不動(10.11)。

#### 19.4.3 TB 質量判據 Q1..Q9(逐條執法;任一敗=該設計 REJECTED=閘紅)

| # | 判據 | 執法 |
|---|---|---|
| Q1 | 編譯:iverilog(case.json 旗標,默認 -g2005)TB+DUT exit 0 且 warning=0 | 編譯 stderr 計數 |
| Q2 | 正樣:原版 2 跑相等且==expected.txt | byte 比對 |
| Q3 | 終止:DONE 為最後協議行;原版無 WATCHDOG/TIMEOUT;牆鐘看門狗未觸發 | 每次模擬帶 timeout(默認 30s,須為實測耗時 ≥30 倍,C-S3 記實測) |
| Q4 | X-clean:expected.txt 的全部 val 無 x/z | 文法掃描(敵意向量:場景名/key 含字母 x 不誤報) |
| Q5 | 協議:SCENE 1..N 連續、N==`// SCENES:`、觀測行皆在場景內、文法合規 | 解析器(修 reimpl-eval:167 虛高洞) |
| Q6 | 靜態約束:contract 名實例化+全端口具名連接、零層次引用、零 $random、TB-R1x 零違例(`// race-ok` 豁免) | tblint;TB-R1x=19.3.3 跨檔極性感知定義(E 取自 tb_plan;語句窗=corpus_lint 文檔語義,復審 RV2-7);同檔 R1 對 TB 內部沿行程照跑,與 corpus_lint.rs:175-180 的 parity 只聲明於三個同行向量;跨檔五向量見 19.5 ②(復審 RV1-5/RV2-7) |
| Q7 | 跨模擬器(verilator 在場):原版輸出經**各自腿**正規化(19.3.3,R1-1)後相等;構建=`--binary --timing -Wno-fatal`,警告中位於 tb.v 者必為 0、位於 DUT 者只記錄不判(R1-12:第三方語料 signedexpr/subbytes 在 verilator 默認告警集下 lint rc=1【實跑】) | 在場即執法;不在場打印 SKIP;分歧經排查屬 DUT 固有(非 TB)時=先入 REGISTRY 分歧登記簿,再以 ADR-0013 修訂列明例外,禁靜默放行 |
| Q8 | 判別力:P-rule ≥1 個 killed(**鐵則**);OP-NOP 與全部 equivalent 控制組存活;全部變異結果==record.json(雙向:新殺/新存活皆紅=DRIFT)。**斷言粒度(R1-4 如實記錄)**=逐變異體(結果類+殺傷場景集都與 record 比對);正樣=全輸出 byte 相等(涵蓋全部場景);下限只有 per-design ≥1 killed——per-rule(規則驗證覆蓋)與 per-scene 判別力**只錄製不斷言**(19.13-5 待決;立下限唯一路徑=ADR-0013 修訂) | 殺傷矩陣快照比對 |
| Q9 | 覆蓋聲明:COVERS/WAIVE 的規則 ID⊆規則書;D 腿每條規則必 COVERS 或 WAIVE(T 腿 COVERS 由 tb_plan 機械生成,非定向場景標 `*`) | 解析+集合運算 |

- **判別力數字**(per design 錄入 record.json 與證據頁):generated/stillborn/equivalent/eligible
  (=generated−stillborn−equivalent)/killed/survived/timeout/殺傷率(killed/eligible);**規則驗證覆蓋
  分兩欄(復審 RV2-5)**:**獨佔**=存在一個 killed 變異其歸屬集**只含**該規則的規則數、**共享**=只經
  歸屬集 ≥2 的 killed 變異而被覆蓋的規則數,分母皆=有 ≥1 eligible 歸屬變異的規則數——報告以獨佔為主數、
  共享括注,禁合併(否則 OP-POL 一個殺傷就把同行程 20 條 guard 全記「已驗證」);**目錄覆蓋**=有 ≥1
  generated 歸屬變異的規則數 ÷ 規則總數;P-unruled 的 generated/killed;**聲明覆蓋**=COVERS 規則數 ÷
  規則總數。聲明(計劃)與驗證(證據)兩數分列。

#### 19.4.4 錄製快照與斷言規格劃界(D-C6;ADR-0012 D14/D15 同形)
- `plugin/rtl2spec/fixtures/conformance/<design>/case.json`=**斷言規格**(手寫合法):design/src/top/
  specPath/iverilog 旗標/tb_leg/協議腿 ID/waivers/flush/輔助重寫腿清單;
- `tb.v`=入庫 TB(T 腿=錄製快照;D 腿=凍結的 LLM 產物);
- `expected.txt`、`record.json`(變異清單+結果+殺傷場景+數字+基線 T 腿結果+規則書缺陷清單+
  provenance[iverilog/verilator/yosys 版本、dist wasm sha256、算子目錄版本、日期])=**只由 `--record`
  寫,禁手改**;重錄=語義事件,期望變了而原版/TB/工具都沒變=停(10.5)。
- **逐切片對賬對照物=錄前快照(復審 RV1-6 修訂;AGENTS 10.11「re-record 對賬對照物」)**:
  fixtures/conformance/ 由 C-S3 新建、弧內禁 commit,始終 untracked——`git diff` 對它恆為空,不能當
  對照物。改為機械化:`--record` 在寫入前把現存 `<design>/{tb.v,expected.txt,record.json}` 原樣複製到
  `plugin/rtl2spec/build/conformance/pre-record/<切片標籤>/<design>/`(切片標籤=`--slice <C-Sn>` 必填
  參數,缺=拒錄 exit 2),寫入後生成 `build/conformance/record-diff-<C-Sn>.md`(逐設計:expected.txt
  行級 diff、變異結果類/殺傷場景集的增刪改、判別力數字前後值);C-S3 首錄時快照為空=全量新增清單。
  每片(C-S3/C-S4/C-S5)的實作記錄必引其 record-diff 檔並逐條對賬。**兩腿同一原則(復審 RV2-4)**:
  對照物=**被覆寫前的已存工件**,永不用重新生成的輸出——conformance 腿複製已存三件套(上述),golden
  腿見下條 G 程序。初稿「C-S2 起 golden 對照物=錄前默認模式產出的 build/golden-current/」作廢:默認
  模式寫入的是當前提取器輸出(golden-check.mjs:231-246),改碼後跑出即修後結果、與 `--record` 寫入
  內容相同,比對恆零差;regress 也會覆蓋該目錄(審方副本實見 regress 後 golden-current 的 mem_arst
  WR_i=(1,false) 而 golden/ 仍 (0,true))。
- **golden 重錄切片標準程序 G0..G5(復審 RV2-3/RV2-4;C-S1、C-S2 首步及日後任何 golden 重錄片通用)**:
  G0 改碼前存檔 `rm -rf plugin/rtl2spec/build/golden-pre-<C-Sn> && cp -R plugin/rtl2spec/golden
  plugin/rtl2spec/build/golden-pre-<C-Sn>`(時代以內容判定,AGENTS 10.11:副本內 PROVENANCE.md 元資料區
  的錄製時刻/wasm sha 即其時代戳;C-S1 另以 `git diff --quiet HEAD -- plugin/rtl2spec/golden` exit 0
  佐證==HEAD)→G1 改碼+單測(含以 oracle 證據支撐的測試期望修正)+該片點名的 `cargo test` 範圍→
  G2 `bash plugin/rtl2spec/run-golden-regression.sh > plugin/rtl2spec/build/golden-diff-<C-Sn>.txt`:有預期
  變更的片**預期 exit 1**,`^DIFF ` 行的檔名集合必**恰等於**該片允許清單、`PROV-DIFF` 行必為 0,多一檔
  或少一檔皆停(10.5)=對賬物 #1→G3 `bash plugin/rtl2spec/run-golden-regression.sh --record` 後
  `bun plugin/rtl2spec/scripts/golden-leafdiff.mjs plugin/rtl2spec/build/golden-pre-<C-Sn>
  plugin/rtl2spec/golden > plugin/rtl2spec/build/golden-leafdiff-<C-Sn>.txt`:變更檔⊆允許清單∪
  {PROVENANCE.md},葉級逐條對賬入實作記錄=對賬物 #2→G4 默認模式再跑 exit 0→G5 連帶重生成(19.2 易變行
  白名單)+其餘閘,**`bash scripts/regress.sh` 最後跑**(其 S7 塊含 golden 回歸,regress.sh:383-386;
  G3 前跑必紅)。新工具 plugin/rtl2spec/scripts/golden-leafdiff.mjs 於 C-S1 引入(唯讀,只寫 stdout;
  演算法=JSON 葉路徑 diff,輸出 `DIFF <檔> / <路徑>: <舊> → <新>`,同本輪 /tmp/c-arc-r2/exp/leafdiff.mjs
  探針——golden JSON 為單行,`git diff` 無法做葉級對賬)。
- **fixtures 內容類型(復審 RV2-14)**:fixtures/conformance/<design>/ 含非 Verilog 檔(case.json=斷言規格、
  expected.txt/record.json=錄製快照),字面超出 AGENTS 10.10「fixtures/ 只放本弧自著 Verilog」
  (AGENTS.md:313-314)與 SKILL fixtures 自著授權律(SKILL.md:235-237);既成先例=fixtures/e1/README.md。
  C-S6 必顯式放寬(不得默認):AGENTS 10.12 與 SKILL「C 閉環增補」寫明「fixtures/conformance/ 允許自著
  tb.v+case.json+expected.txt/record.json;fixtures/conformance-lint/ 只放自著 Verilog 片段;外部語料與
  GPL 仍禁入」,並在 10.10 該句後加「(conformance 子樹例外見 10.12)」。

#### 19.4.5 入庫門檻(D-C5)
- T 腿:Q1..Q9 全過(判別力取鐵則 ≥1 killed)。
- D 腿附加:**形族底線**(規則書出現的每個 form 族——guard/transition/sequence/relation——若該族
  有 eligible 變異,至少 1 個被殺)+**不劣於基線**(同變異集上 D 腿殺傷率 ≥ T 腿殺傷率);不達=
  不入庫(留 build/ 作候選,數字照報)。這兩條判的是**凍結 D 腿 tb.v 這個工件**的機械可測屬性,不是
  LLM 的採樣過程或能力(19.1「閘判據劃界」,R1-3 修訂;與 19.11-2 同一定義,不相抵)。
- 數值殺傷率下限 v1 **不設**(只錄製釘死+鐵則);C-S3/C-S4 實測後若要立下限=ADR-0013 修訂+閘重跑,
  禁靜默(V1..V6 降閾同律)。

### 19.5 conformance 閉環評測閘(D-C7/D-C8)

- 閘=`bash plugin/rtl2spec/run-conformance-eval.sh`(**C-S3 交付前=規劃閘**,§17.11 R17-1 教訓:
  禁把規劃閘寫成現役):
  ① 守衛:iverilog/bun 在場(缺=exit 2,逐項列出);dist/wasm 在場且不陳舊(6 個傳遞依賴目錄
  find -newer,與 run-golden-regression.sh 同清單)——**陳舊=exit 2 並提示先跑
  run-golden-regression.sh,本閘不重建 pkg**(守寫邊界);設計目錄數==`EXPECTED_DESIGNS`(=6,
  腳本常量;防清單短收——eval-all L2=0 假綠教訓);
  ② selfcheck(每次必跑,判別力自證):正規化敵意向量(**R1-1 擴**:iverilog 橫幅 `(1s)`/`(1ps)` 兩種
  時間單位形必丟、含 `$finish` 字樣的協議 kv 行必留;verilator 腿以 corpus_lint.rs:186 原始字串
  逐位元組復現 :188 期望=JS 移植與 Rust 契約 parity;19.15 R1-1 實跑捕獲的 verilator 四行報告必丟;
  `  v=-3` 協議行兩腿皆留)、X 掃描敵意向量、TB 內部沿行程的同檔 R1 三向量 parity、**TB-R1x 跨檔
  五向量(復審 RV1-5/RV2-7)**——(+)DUT `always @(posedge clk)` 讀 d、TB `@(posedge clk); d = 1;`→違例;
  (−)同 DUT、TB `@(negedge clk); d = 1;`→零違例;(+)DUT `always @(negedge clk)` 讀 d、TB
  `@(negedge clk); d = 1;`→違例;(−)TB `@(posedge clk); #1; d = 1;`→零違例;**(−)第五向量(復審 RV2-7)
  TB `@(posedge clk); #1;`+換行 `d = 1;`→零違例**(文檔語義;Rust corpus_lint 對此形報 1 條=D-c3 已登記
  差異)(五向量的 DUT/TB 片段=
  plugin/rtl2spec/fixtures/conformance-lint/ 自著 MIT 小檔,正向量期望誤配負向量必紅)、變異
  手術 count==1 與非阻塞 `<=` 不誤動、「正樣期望誤配給某 killed 變異的輸出必紅」;
  ③ 逐設計:tblint(Q6/Q9)→[T 腿]進程內 3 次重生成 byte 相等且==tb.v→編譯(Q1)→原版 2 跑
  (Q2..Q5)→verilator(Q7)→變異矩陣(P-rule+P-unruled+控制組,寫 build/conformance/<design>/mut/)
  →結果比對 record.json(Q8)→[D 腿]基線 T 腿重生成並跑同一矩陣→診斷自驗(19.6.4)→輔助腿計分
  (不影響 exit);
  ④ 輸出(**默認模式寫只落 plugin/rtl2spec/build/conformance/**):verdict.json+report.md+diag/;SUMMARY 行
  `SUMMARY conformance designs=6/6 admitted=… mutants=… eligible=… killed=… equivalent=… unruled_killed=…`;
  ⑤ exit:0=全部設計 ADMITTED 且快照全等;1=任一判據敗/DRIFT/selfcheck 敗;2=守衛。
  `--record --slice <C-Sn> [<design>…]`:另守衛 yosys(`RTL2SPEC_YOSYS`→vendored 路徑→PATH;缺=exit 2);
  鐵則不過=拒錄 exit 1;先寫錄前快照(19.4.4)再寫 plugin/rtl2spec/fixtures/conformance/<design>/
  {expected.txt,record.json[,tb.v(T 腿)]}+**plugin/rtl2spec/conformance-report.md**(證據頁,機械生成
  禁手改,不含易變時間戳;默認模式把同一頁重生成到 build/ 並 byte 比對,陳舊或手改=紅)。**證據頁落在
  插件根內(復審 RV1-12 修訂)**:初稿 docs/RTL2SPEC-CONFORMANCE.md 會成為第二個出 plugin 的寫面,與
  AGENTS 10.10「寫只落 plugin/rtl2spec/」及 run-realworld-eval.sh:9-10「唯一出 plugin 的寫面」相抵;
  改落 plugin/rtl2spec/(validation-report.md 同位先例),本閘全部寫面都在 plugin/rtl2spec/ 內。
- **per-design verdict 形態**(verdict.json;數字為形態示意,實值由 C-S3 錄製產生):

```json
{"design":"i2c_master","src":"corpus/e2e/59_i2c_master/i2c_master.v","top":"i2c_master",
 "specPath":"plugin/rtl2spec/golden/corpus/e2e/59_i2c_master/i2c_master.rules.json","rules":152,
 "tb":{"file":"tb.v","leg":"template","protocolLeg":"-","compiled":true,"warnings":0,
       "scenesDeclared":0,"scenesParsed":0,"xClean":true,"deterministic":true,
       "watchdog":"ok","crossSim":"agree"},
 "positive":{"matched":0,"total":0,"pass":true},
 "discrimination":{"generated":0,"stillborn":0,"equivalent":0,"eligible":0,"killed":0,
                   "survived":0,"timeout":0,"rate":"0/0","ruleVerified":{"exclusive":"0/0","shared":"0/0"},"catalogCoverage":"0/152",
                   "unruled":{"generated":0,"killed":0},"controlsSurvived":"0/0","recordMatch":true},
 "declaredCoverage":"0/152","baselineTemplate":null,
 "diagSelfcheck":{"looTop3":"0/0","recordMatch":true},
 "aux":[{"leg":"i2c-A1","kind":"rewrite","round":0,"present":true,"compiled":true,
         "matched":0,"total":0,"mismatched":[]}],
 "verdict":"ADMITTED","reasons":[]}
```

- 讀法(TB 通過率×判別力×編譯):正樣通過率必 100%(否則 TB 不可用);判別力=殺傷率+規則驗證覆蓋;
  編譯=TB+DUT 可編譯且零警告;重寫產物的通過率只在 `aux` 欄,**永不影響 exit**(10.11)。
- **與 run-reimpl-eval 的關係(分層不重複,D-C8)**:
  - reimpl-eval=復原力弧 S5 的**歷史消融量測床**(3 份 e1 TB、A/B 兩組、S1/S2/S3/抄寫通道消融點)
    ——凍結:不擴設計、不改 LEGS 與分段語義、不改 exit 語義(量測器形符合 10.11);
  - 共享:新 `plugin/rtl2spec/scripts/lib/tbproto.mjs`(場景切塊+丟行白名單正規化+帶 timeout 的模擬
    執行器;**路徑一律寫全**,復審 RV1-14:本章 `scripts/` 曾同時指插件根與 repo 根,新檔全部在
    plugin/rtl2spec/scripts/ 下;本章引用的 repo 根腳本只有 scripts/regress.sh,路徑約定見章首);
    reimpl-eval 在 C-S3 遷用並**證 parity**——遷移前後 verdict.json 的 legs/segments 深等(除
    generatedAt)且 runs/out_* byte 相等;同輪獲得看門狗(補 AGENTS §4 洞)與 `scenesParsed` 資訊欄
    (不改 exit、不改 matched 計算)。**落點(R1-7 修訂,防兩條驗收互斥)**:verdict.json 結構凍結,
    `scenesParsed` **不入** verdict.json(實勘 reimpl-eval.mjs 的 `clock`(:70)只定義零調用,
    generatedAt 是 verdict 唯一變異源),改寫 sidecar `build/reimpl-eval/scene-protocol.json`+console
    一行;parity 比較器因而無須任何「容忍新鍵」例外;
  - conformance 不重跑 reimpl LEGS;重寫產物(fixtures/e1-reimpl 與在場的 build/reimpl-eval/reimpl/)
    可作 aux 腿在新 TB 下計分——同一產物在 e1 TB 與 conformance TB 下的分數差=兩套 TB 強度差的
    直接讀數(分列報告)。
- **閘落點:總閘直調,不進 regress**。理由:①時長——6 設計規則證據首行去重後 220 行【實跑】
  (i2c 130/sincos 39/fsm 22/subbytes 14/acc 9/signedexpr 6),每規則 ≤2 算子,量級為 10² 次
  編譯+模擬,另加 verilator 6 次編譯,屬分鐘級(估算;C-S3 實測入記錄),與 run-realworld-eval
  (353 檔)/run-reimpl-eval 同屬評測層直調先例;②regress 已含 golden/gap-fixtures 兩閘,釘死了本閘
  的輸入(規則書),本閘獨有價值(TB 判別力)屬評測層;③評測層進交付閘會放大 regress 時長。
  收編重議條件:日後若提供 ≤30s 的 `--quick` 子集(正樣+模板重生成+每設計首個 killed 變異),可經
  ADR-0013 修訂以雙守衛降級形收編 regress S7 塊(regress.sh:368 起)——本弧不做。

### 19.6 診斷回灌(D-C9)

#### 19.6.1 counterexample→結構化診斷(確定性;plugin/rtl2spec/scripts/conformance/diagnose.mjs)
- 輸入:verdict.json+原版/被測輸出+case.json(COVERS)+record.json(殺傷矩陣)+規則書(specPath)。
- 診斷 JSON v1:`{design, leg, round, failing:[{scene, firstDiff:{lineNo, expected, actual},
  keysDiffering, contextBefore(≤3 行)}], suspects:[{ruleId, statement, check, why, rank}],
  undefinedSignals:[{name, class}], assumptions:[被測產物的 // ASSUMPTION: 行], classification}`。
- suspects 排序(確定性):①direct=規則 lhs/signal 恰為差異輸出端口;②covers=失配場景 COVERS 的
  規則;③mutant-sensitive=殺傷矩陣中該場景對該規則變異的殺傷計數(**留一法**:驗證某變異時排除它
  自身);同級按規則 ID 序。
- def_lint(規則書定義完備性,heuristic;19.0-4 探針的正式化):U1 net 宣告賦值洞(`wire x = …;`,
  sv/mod.rs:686 `_ => {}`)/U2 直通別名(S2 政策)/U3 分支域外純常量(S2 政策)/U4 Index/IndexedPart
  非 canon(§18.4-3 待決)/U5 其他——按源碼定義行形態歸類;U2/U3 標「政策排除」,不記缺陷。
  **定位聲明(復審 RV1-14)**:def_lint 是規則書完備性的語義分析,與核心 lint.rs 的 decl_lint 同族,
  本弧卻以 JS 正則歸類 Verilog 行實作——故明定為**閘側工具**:只存在於 plugin/rtl2spec/scripts/
  conformance/,只被 run-conformance-eval 調用,**不經任何面露出**(不進 wasm 導出、不進 MCP 工具、不進
  skills、不進規則書 JSON);其 heuristic 結果只入 record.json 與缺陷報告。晉升為核心能力(lint.rs
  新欄位+規則書 schema 變更+golden 全樹 churn)=後續立項,必須走 10.10「新能力先落核心再露面」。
- 分類:協議違例→`tb-protocol`;被測產物 ASSUMPTION 行提及 undefinedSignals 中的名字、或 suspects
  為空、或 ≥2 個獨立樣本同場景同 firstDiff→`rulebook-gap-candidate`;其餘→`rewrite-error`。

#### 19.6.2 回灌對象與輪數上限(≤3)
- **F1 重寫迭代(LLM,輔助)**:round 0=REIMPL-PROTOCOL §1 原模板(A 組完整規則書);未全過→診斷 md
  經 19.3.4 §3 模板回灌→round 1..**3**;停止=全場景過或滿 3 輪;**每輪分數分列**,round≥1 記「閉環
  可復原性(含 oracle 反例)」,禁併入 round 0 的規則書復原力;過 TB 後(輔助)以 yosys equiv 查等價
  殘差(yosys 的第二個用處,屬採樣演示、非閘、寫只落 build/;與 19.11-7 同一定義,復審 RV1-13),
  unproven 輸出∩undefinedSignals→餵 F3(cnt_end 類問題由此浮出)。
- **F2 TB 改進(LLM,輔助)**:存活 eligible 變異→規則 ID+陳述+check 經 §2 模板回灌 TB 撰寫者
  (不給變異源碼行,防源碼經回灌泄漏)→≤3 輪;停止=一輪無新殺或滿 3 輪;入庫仍走 19.4.5。
- **F3 規則書缺陷報告(確定性為主)**:P-unruled killed 行(確定性,強證據)+def_lint U1/U4
  (heuristic)+F1 的系統性失配與等價殘差(輔助)→build/conformance/rulebook-defects.{json,md};
  採樣設計的 def_lint 與 P-unruled 結果錄入 record.json(漂移=DRIFT 紅,迫使將來提取器修復時同輪
  重錄);人工確認者入 REGISTRY——**D-c1 net 宣告賦值未提取**已於 C-S1 先行登記(19.2;10.5 先登記),
  F3 只為其補證據(證據=i2c_master.v:68+sv/mod.rs:686+19.0-4 探針+rulebook-defects 引用;修復不屬本弧)。

#### 19.6.3 弧內採樣演示方案(C-S5;輔助證據,永不進確定性閘)
- F1:fsm_test 與 d(sincos)按 REIMPL 協議各新寫 round-0 重寫 n=1;i2c 用在場的
  build/reimpl-eval/reimpl/i2c-A1..A3(e1 TB 下 11/11)作 round 0;任一 round-0 未全過即回灌,≤3 輪;
  腿表 append 進 CONFORMANCE-PROTOCOL §4,數字入本章實作記錄(按輪、按腿分列)。
- F2:C-S4 定向 TB 的 K=1 樣本按存活清單改進,≤3 輪。
- 降級:無採樣條件→F1/F2 標「環境受限未跑」;F3 的確定性部分與 19.6.4 自驗照常落地。

#### 19.6.4 診斷層確定性自驗(進閘)
- 以 killed 變異作「真值已知的缺陷實現」:對每個 P-rule killed 變異跑診斷,**留一法 top-3 命中率**=
  真值規則落入 suspects 前 3 的比例;per design 錄入 record.json,閘斷言相等(診斷生成器回歸面);
  v1 不設下限,數字照報。被測物是機械變異而非 LLM 產物——不違 10.11。

### 19.7 擴面與採樣集(D-C10)

- **選取判據**:J1 規則書非平凡=釘選 37 檔 golden(specPath 實存)且規則 ≥3;J2 行為可 TB 化=
  單模組、端口全非 parametric、iverilog -g2005 零警告可編譯、復位後或 flush 後可 X-clean 觀測、
  可終止;J3 in-git 授權=corpus/(repo MIT)或 corpus-external/yosys-simple(ISC,COPYING 在場
  【實跑 ls】),zipcpu 永不入;J4 形族多樣=合起來覆蓋 guard/transition/sequence/relation 四族+
  同步/異步復位+無復位流水線+純組合;J5 規模=Σ規則 ≤300(變異矩陣保持分鐘級)。
- **採樣集(6 設計,Σ規則 252)**【實跑 stats.mjs/iverilog】:

| design 目錄 | top | src | specPath | 規則(形族) | 特性 |
|---|---|---|---|---|---|
| i2c_master | i2c_master | corpus/e2e/59_i2c_master/i2c_master.v | plugin/rtl2spec/golden/corpus/e2e/59_i2c_master/i2c_master.rules.json | 152(guard 20/trans 9/seq 123) | 旗艦;協議 FSM;negedge rst_n 異步復位(時鐘前置);全部輸出有復位值 |
| acc_pipe | __acc_pipe__acc_pipe | corpus/e2e/58_xls_acc_pipe/acc_pipe.v | plugin/rtl2spec/golden/corpus/e2e/58_xls_acc_pipe/acc_pipe.rules.json | 9(seq 5/rel 4) | 旗艦;無復位 3 級流水線(flush) |
| fsm | fsm_test | corpus-external/yosys-simple/fsm.v | plugin/rtl2spec/golden/yosys-simple/fsm.rules.json | 23(trans 5/seq 18) | 同步復位不在頂層 If(`if (reset) state <= 100;` 落為 sequence R011)→零 guard→模板腿 resets=∅、S-RST 不生成、guard 類算子不產生(R1-11:證據頁機械註記此成因);integer 狀態 |
| sincos | d | corpus-external/yosys-simple/sincos.v | plugin/rtl2spec/golden/yosys-simple/sincos.rules.json | 46(guard 8/seq 38) | CORDIC;posedge 異步復位;start/done 握手;帶號算術 |
| signedexpr | signed_test01 | corpus-external/yosys-simple/signedexpr.v | plugin/rtl2spec/golden/yosys-simple/signedexpr.rules.json | 6(rel 6) | 純組合;5 輸入位窮舉;帶號/無號字面量語義 |
| subbytes | subbytes_00 | corpus-external/yosys-simple/subbytes.v | plugin/rtl2spec/golden/yosys-simple/subbytes.rules.json | 16(guard 3/seq 3/rel 10) | negedge 異步復位;陣列+組合次態;def_lint 7 項 |

- **排除表**:counter8(規則 2 條<J1,且不在釘選集無 golden;留 reimpl-eval 床);crates/spec-i2c 副本
  (與 corpus i2c 只差頭部 6 行【材料】=重複);constmuldivmod(46 條 relation 中有除/模零臂→x,模板
  無選擇子避讓能力;後續);mem_arst(復位值 `'bx`+parametric 端口;是 C-S1 D-r1c 的對象而非 TB 樣);
  operators/memory(規模與記憶體初值/X 面,後續);sv-tests L2(規則 3-5 條,多為語法測試)。
- 規模聲明:確定性 T 腿在弧內必跑通 6/6 全量(C-S3 驗收);D 腿 tbSample=4(i2c_master/acc_pipe/
  fsm/sincos)。

### 19.8 版本策略(D-C11)

- 新能力面=wasm 導出 `rtl_tb_plan_sv`+`rtl_tb_plan_rtlil`(7→9;後者只回核心 TB1,RV2-8)+MCP 工具
  `rtl_tb_plan`(4→5)+skill `rtl-conformance`(3→4)→semver **minor 0.3.0→0.4.0**(§17.4 語義);四點一致=
  plugin.json version/CHANGELOG 頭 `## [0.4.0]`/SERVER_VERSION/serverInfo 引用常量;CHANGELOG [0.4.0]=
  Added(tb_plan/模板/工具/skill/conformance 閘)+Fixed(D-r1a/b/c、D-c2)。
- **工具數 4→5 是對既有條文的顯式修訂,不是「既有清單不變」(復審 RV1-3)**:ADR-0012 D4「四工具」
  (adr/0012:27)、其非目標「MCP 只 stdio+四工具」(:138)與本檔 §11-9「MCP 只做 stdio+四工具」(:796)
  三處由 **ADR-0013 D-C11 修訂**:stdio 單一傳輸、零 HTTP/WS/sampling/resources/prompts 面**不變**,
  工具數 4→5(新增 `rtl_tb_plan`,C-S2 生效)。§11-9 與 ADR-0012 原文按 append-only 不改,修訂關係
  記於 ADR-0012 實作記錄的修訂指針條目與本節;19.11 標題的「既有清單不變」據此加注此一例外。
- **run-mcp-e2e 閘定義的同片同步(復審 RV1-3)**:現存定義已三分——「七斷言」(AGENTS.md:305、
  adr/0012:106、REGISTRY.md:84、run-mcp-e2e.sh:7/:57、scripts/mcp-client.mjs:2-7)、「11 斷言」
  (docs/OKR.md:62)、而 mcp-client.mjs 實有 10 個 `check(` 呼叫點(:83/95/111/118/130/135/146/153/
  157/165)【實跑 grep】。規則:改閘的切片同片同步全部定義點,斷言數**一律取該片實跑的 PASS 行數**
  (不取文案、不取呼叫點靜態計數):C-S1 加 ⑧ 後同步一次;C-S2 改 ②「恰 5 工具」並加 ⑨⑩ 後再同步
  一次。同步面=AGENTS.md:305、REGISTRY.md:84、run-mcp-e2e.sh:7/:57、mcp-client.mjs 頭註、OKR.md:62
  (原地改);adr/0012 機械閘 1 原文不改,改以其實作記錄的修訂指針條目指向 ADR-0013 機械閘(append-only)。
  **「四工具」字樣同步面(復審 RV2-11 補,本輪 `rg -n '四工具|4 工具|恰 4|exactly 4'` 普查全表)**:
  現役定義須 C-S2 同片改為五工具=PLAN.md:41(「stdio JSON-RPC 四工具」)、dist/server.mjs:99(註釋
  「工具面(§6.1 四工具)」,同片要改的檔)、REGISTRY.md:84、mcp-client.mjs:6/:91/:96;歷史記載不改=
  CHANGELOG.md:78(0.1.0 條目)、adr/0012:27/:107/:138(由修訂指針承載)、本檔 §6.1/§11-9(append-only);
  命中但無關=gap-fixture-check.mjs:13/:163/:189(「G5 恰 4 條」=破口條數)。
- **validate-manifest 同步(復審 RV1-8)**:plugin/rtl2spec/scripts/validate-manifest.mjs:62 把 skill
  目錄硬編碼為三項;C-S2 新增 `rtl-conformance` 時同片把它加入清單(run-plugin-check ② 復用此腳本),
  並以紅路演練確認(暫刪該 skill 的 frontmatter description 必紅→還原綠)。
- **skill `rtl-conformance` 內容與零語義驗法(復審 RV2-14 補;初稿只有名字)**:SKILL.md 骨架(C-S2 落盤)
  =frontmatter(name=rtl-conformance;description=「從 RTL 生成模板 testbench 計劃與骨架(rtl_tb_plan),用於
  行為復原力 conformance 對拍;不是正確性證明」)/觸發詞(testbench、TB 生成、conformance、對拍)/流程:
  ①調 `rtl_tb_plan`(path|source、lang、module?、seed?、cycles?、flush?)②讀 structuredContent:ok:false→
  原文轉述錯誤(帶 milestone TB1=範圍外,不重試、不改寫)、ok:true→讀 plan(clocks/resets/sens_edges/
  obligations/waivers/scenes)與 template_v③把 template_v 寫成檔,交使用者環境的模擬器跑——**skill 不判定
  結果、不計分、不生成期望值**④讀協議 v1 輸出(SCENE/kv/DONE)/讀法(resets=∅ 時 S-RST 未生成的機械含義;
  obligations 的 indirect 與 WAIVE)/禁止宣稱(19.1:行為復原力≠意圖正確性)。零語義的機械驗法=
  run-plugin-check 新判據 **⑥**:每個 skills/*/SKILL.md 中出現的 `rtl_` 前綴標識符 ⊆ server tools/list 的
  工具名(本輪實跑 `rg -o 'rtl_[a-z_]+' plugin/rtl2spec/skills`:三個現有 skill 只提 rtl_analyze/
  rtl_extract_rules/rtl_scan_gaps,皆在 server.mjs:103/128/162 的工具名內——判據今日即可滿足);
  紅路=在 skill 裡寫一個不存在的工具名必紅。
- 內核 ABI 軌**不動**(EXPECTED_ABI 仍 ['0.1.0'],workspace 版本不 bump,ADR-0012 D18)。新增導出是
  加法變更,但「新 server+舊 wasm」只會在調用時失敗——補 run-plugin-check **③c 導出面檢查**:
  server.mjs 引用的全部 `api.<fn>` 名 ⊆ dist wasm 實際導出(閘驗出貨物,D8 同款);紅路演練=刪一個
  導出名必紅。
- 同輪:server.d.ts 類型契約面;mcp-client ② 改「恰 5 工具」+新斷言 ⑨(rtl_tb_plan 正樣:
  fixtures/fsm_counter.v→ok:true、clocks≥1、resets≥1、模板含 `// SCENES:`/`SCENE1`/`DONE`,且模板與
  fixture 同編 `iverilog -g2005 -t null` exit 0、warning=0[iverilog 在場時;不在場打印 SKIP],復審
  RV1-4)+⑩(負樣:自著 parametric 端口 fixture→isError 且含 `milestone TB1`)+⑪(復審 RV2-8:`.il` 輸入
  →isError 且含 `milestone TB1`,證 lang 分派可達);server.mjs 零語義(收參→裝載→調導出→編幀)。
- tag 慣例 `rtl2spec-v0.4.0`(打 tag 由用戶定)。

### 19.9 切片分解(C-S1..C-S6;每片獨立對賬,禁攢批重錄)

- **C-S1 D-r1 收平**(19.2):三缺陷修復(D-r1c=規則面+破口面兩處同改,select_trigger 單一語義源;D-r1a 含
  emit.rs 兩處 128)+fixtures 5 檔+單測 2+plugin-check ⑤(a)..(d)+mcp-e2e ⑧(四調用)+golden-stats+
  golden-leafdiff.mjs+golden 重錄(19.4.4 G0..G5;允許清單=mem_arst 的 rules+gaps 兩檔)+eval/validation 重生成
  (易變行白名單外只許 mem_arst 可歸因行)+REGISTRY 三行關閉注記+**「C 弧已知未決」小節首錄 D-c1/D-c2/
  D-c3/D-c4**(10.5 先登記)+run-mcp-e2e 定義點按實跑 PASS 行數同步(19.8)。驗收命令(按 G 程序順序):
  `cargo test -p rtl-spec`→golden G2(DIFF 恰兩檔)→G3 `--record`+leafdiff→G4→run-gap-fixtures(規則面+5、
  破口面+5)/run-mcp-e2e/run-plugin-check/run-realworld-eval/run-rtl-validation 全 PASS→`bash
  scripts/regress.sh` 最後;`git diff --stat plugin/rtl2spec/golden` 與 G3 leafdiff 檔集一致(本片 G0==HEAD)。
- **C-S2 D-c2 修復+核心 tb_plan+面**(19.3.2/19.8),片內兩步各自對賬:
  **步 a(D-c2,語義重寫;復審 RV2-1/RV2-2/RV2-3/RV2-6 重寫)**=19.3.2 C-S2 首步的 G0..G5 全程:vl-syntax
  ANSI 新組重置(四個滾動變量,僅方向繼承)+七向量前端單測+api.rs:128 期望 4→1 與 :119-121 註釋+
  module.rs:139-140/ast.rs:97/sv.rs:132-134/api.rs:58 註釋改寫→`cargo test --workspace`→G2 DIFF 恰
  {loops,memory,mem_arst,partsel}.rules.json→G3 `--record`+leafdiff 恰 24 葉(partsel 16 葉為 signed)→G4→
  validation/EVAL 重生成(白名單外零變;memory.v 雖在 13 檔驗證集內,validation-report 只為 i2c_master/
  carryadd/acc_pipe 三模組渲染接口契約[:68/:2406/:2460【實跑 grep】],memory 只在摘要表,故受影響端口不入
  該頁)→
  **最後** `bash scripts/regress.sh`(全部基線零位移,任一位移=停)→REGISTRY D-c2 關閉注記+ADR-0012
  實作記錄 D1 修訂指針。
  **步 b(tb_plan,能力新增)**=tbplan.rs(Result API;TB1 十一條)+單測(6 設計 plan 結構斷言、fsm_counter
  形 busy 寬 1、TB1 硬錯負樣、12 跑確定性)+emit `tbplan` 模式(sv|rtlil)+wasm 兩導出(7→9)+smoke parity
  三路+MCP 工具(含 lang)+server.d.ts+skill(19.8 骨架)+validate-manifest.mjs:62 skill 清單+0.4.0 四點+
  CHANGELOG+③c+⑥+mcp-e2e ②⑨⑩⑪ 與定義點同步(含 PLAN.md:41、server.mjs:99);規則書 schema 不變→步 b 的
  golden 零 churn(斷言 run-golden-regression 不重錄即 PASS——golden-check 的 PROVENANCE 核對只比 golden 檔
  sha、不比 wasm sha,golden-check.mjs:264-283【檔】);**wasm 重建使兩證據頁的 `- 工具鏈:` 行(含 wasm
  sha)與 `- 日期:`/`- 耗時:` 行變,步 b 必重生成兩頁並按 19.2 易變行白名單對賬(復審 RV1-7/RV2-9)**。驗收:
  `cargo test --workspace`;`bun crates/rtl-spec-wasm/smoke.mjs`(+node);run-mcp-e2e(②⑧⑨⑩⑪);
  run-plugin-check(②含新 skill、③c 紅路、⑥ 紅路);run-golden-regression;run-rtl-validation;
  run-realworld-eval;`bash scripts/regress.sh` 最後。
- **C-S3 conformance 確定性床**(19.4/19.5):plugin/rtl2spec/scripts/{lib/tbproto,conformance/tblint,
  conformance/mutate,conformance-eval}.mjs+run-conformance-eval.sh+selfcheck(含 TB-R1x 跨檔五向量)+
  `--record --slice C-S3`(yosys 存活裁決;錄前快照+record-diff-C-S3.md)+6 設計 T 腿 case.json/tb.v/
  expected.txt/record.json 入庫+證據頁 plugin/rtl2spec/conformance-report.md+reimpl-eval 遷 tbproto
  (parity 證)。紅路三路:①篡改 expected.txt 一字節→Q2 紅;②把某 killed 變異的錄製結果改成 survived→
  Q8 DRIFT 紅;③刪 tb.v 一個 SCENE 頭→Q5 紅;還原後綠。驗收:`bash plugin/rtl2spec/run-conformance-eval.sh`
  exit 0(6/6 ADMITTED);`bash plugin/rtl2spec/run-reimpl-eval.sh` exit 0 且 parity 斷言成立(**本機
  build/reimpl-eval/reimpl/ 8 腿在場為前提**,19.10 環境依賴分類);實測時長入記錄。
- **C-S4 D 腿定向 TB**(19.3.4/19.4.5):CONFORMANCE-PROTOCOL.md 釘檔(19.3.4 三模板逐字+內容錨)+tbSample
  4 設計 K=1(K=2,3 可選)+F2 ≤3 輪+入庫(形族底線+不劣於基線)+`--record --slice C-S4` 重錄,以
  record-diff-C-S4.md 逐案對賬(T 腿轉基線腿;**不用 git diff**,19.4.4);降級=標「環境受限未跑」,
  C-S3 狀態不變。驗收:run-conformance-eval exit 0;D/T 兩腿數字分列入證據頁。
- **C-S5 診斷回灌**(19.6):plugin/rtl2spec/scripts/conformance/diagnose.mjs+def_lint(閘側工具,不露面)+
  P-unruled 報告+留一法自驗+`--record --slice C-S5`(record-diff-C-S5.md 對賬)+F1 採樣演示(≤3 輪)+
  REGISTRY D-c1 證據補強(rulebook-defects 引用)。驗收:run-conformance-eval exit 0(含 diagSelfcheck);
  rulebook-defects 對 i2c 含 cnt_end(斷言規格,手寫合法);演示數字按輪/腿分列。
- **C-S6 立法四層+文檔+總閘**:AGENTS 新節 10.12(C 閉環紀律;寫面聲明=本閘全部寫面在
  plugin/rtl2spec/ 內,無新增出 plugin 寫面;**fixtures 內容類型放寬**=fixtures/conformance/ 允許自著
  tb.v+case.json+expected.txt/record.json、fixtures/conformance-lint/ 只放自著 Verilog 片段,10.10 對應句
  加例外指針——復審 RV2-14)+SKILL「C 閉環增補」節(含 fixtures 自著授權律的同款放寬與 rtl-conformance
  skill 的零語義讀法)+REGISTRY(閘行 run-conformance-eval/Crates 行 rtl-spec tbplan、rtl-spec-wasm 9 導出、
  vl-syntax ANSI 修復/D-c1..D-c4 狀態)+**PLAN.md 里程碑表新增 C 弧
  行與 TB1 里程碑族(AGENTS §3;先例 PLAN.md:111)+docs/OKR.md 新 KR 證據指針=`bash
  plugin/rtl2spec/run-conformance-eval.sh`(AGENTS §10;先例 OKR.md:62)**(復審 RV1-12)+證據頁+
  ADR-0013 機械閘轉現役+本章實作記錄(append)+總閘 11 命令全跑。
- 依賴:C-S1(select_trigger;D-c1..D-c4 登記;G 程序與 leafdiff 工具引入)→C-S2(D-c2 修復先於 tb_plan;
  contract 寬度/符號正確)→C-S3(模板 TB)→C-S4(基線腿)→C-S5(殺傷矩陣)→C-S6。
- 拆弧判據:C-S3 任一設計模板腿達不到鐵則(零 killed)→該設計移出採樣集須經 ADR-0013 修訂並如實
  登記,不得放寬鐵則;C-S4/C-S5 的 LLM 腿不可得→降級收口,不阻塞 C-S6。

### 19.10 總閘(D-C7;腳本直執,全部 exit 0 才算交付)

**復審 RV1-2 修訂(分類改寫)**:總閘命令序列=ask 固定契約第 8 條的 11 條命令,由弧總閘腳本直執,本章
不得增刪;但其中各條的**地位**不同,初稿把 gitignored 的兩條寫成「現役」與 19.11-9、
run-reimpl-eval.sh:2「總閘成員外」、ADR-0012 現役 8 條相抵,且 run-reimpl-eval 盲寫產物缺失即 exit 3
(reimpl-eval.mjs:205-206)——等於把 LLM 產物在場與否當交付判據,與 19.1 劃界、19.3.1 降級形相抵。改為三類:
- **ADR 現役交付閘 8**(in-git 可重現,不依賴任何 LLM 產物;=ADR-0012 機械閘 4 的現役 8 條):
  `cargo test --workspace` ─ `bash scripts/regress.sh` ─ `bash plugin/rtl2spec/run-mcp-e2e.sh` ─
  `bash plugin/rtl2spec/run-rtl-validation.sh` ─ `bash plugin/rtl2spec/run-golden-regression.sh` ─
  `bash plugin/rtl2spec/run-gap-fixtures.sh` ─ `bash plugin/rtl2spec/run-realworld-eval.sh` ─
  `bash plugin/rtl2spec/run-plugin-check.sh`。
- **規劃閘 1**(C-S3 交付後轉 ADR 現役,同屬 in-git 可重現;缺席的 aux 腿一律 SKIP 不紅):
  `bash plugin/rtl2spec/run-conformance-eval.sh`。
- **環境依賴直調 2**(固定序列內照跑;**非 ADR 交付閘、非 regress 成員**,其 exit 只證「本機環境下量測床/
  探針可跑」,不構成 C 弧任何確定性判據):`bash plugin/rtl2spec/run-reimpl-eval.sh`(依賴 gitignored
  build/reimpl-eval/reimpl/ 8 腿=LLM 產物;缺則 exit 3=INCOMPLETE)、`bash
  plugin/rtl2spec/build/s6-metamorphic/run.sh`(gitignored 探針,10.11 邊界)。兩者在本機均在場【實跑
  ls】,故本機固定序列可全綠;新 clone 下前者 exit 3、後者不存在——**這是固定契約與「交付閘不依賴
  LLM 產物」之間的已知張力**,解法交用戶裁決(19.13-2:晉升 8 腿入 fixtures 使其 in-git 可重現,或把兩條
  移出固定序列),本弧不擅改 reimpl-eval 的 exit 語義(19.11-8)。

### 19.11 顯式不做(§11/§16.7/§17.9 既有清單不變,**唯一例外=§11-9 的「四工具」由 ADR-0013 D-C11 修訂為五工具**,見 19.8;復審 RV1-3)

1. 不做意圖正確性宣稱(19.1 禁止宣稱全條)。
2. 不讓 **LLM 採樣過程**(prompt/模型/採樣次數/K 選擇/改進輪次)與**重寫產物的計分**(aux 欄、F1 各輪
   分數)成為閘 exit 判據;凍結入庫工件(含 D 腿 tb.v)的機械可測屬性**是**入庫與閘判據——劃界定義=
   19.1「閘判據劃界」(R1-3 修訂:初稿「凍結產物與機械變異只作 fixture 輸入,其分數不構成入庫判據」
   與 19.4.5 字面相抵,且錯把機械變異的殺傷結果[Q8 核心判據]也排除在外,已改寫)。
3. 不做 SVA/斷言生成;TB 內不放期望常量。
4. 不做 RTLIL 側 tb_plan(RTLIL 輸入經 wasm `rtl_tb_plan_rtlil`/MCP lang 分派到達核心後 milestone TB1 硬錯,
   復審 RV2-8),不做 parametric 端口寬度求值(milestone TB1 硬錯)。
5. 不修 net 宣告賦值提取洞(D-c1 只登記)、不擴 S2 canon 名單、不改 S2 直通別名政策(本弧只出缺陷
   報告);不改 RTLIL 面 rtlil.rs:197-209 的同形觸發選取(無現形樣,只登記潛在面;19.2 D-r1c);不修
   corpus_lint.rs R1 同行 `#` 不關窗(D-c3,只登記;修否=19.13-8)與 contract 負數下標範圍誤標
   parametric(D-c4,只登記;tb_plan 對其 TB1=安全側)。
6. 不把 run-conformance-eval 收編 regress(`--quick` 子集也不做)。
7. 不實作 ryosys equiv_*(Y0 stub);yosys oracle **只用於兩處、皆非閘 exit 判據**:①`--record` 的存活
   變異裁決(錄製產生,結果入 record.json);②C-S5 採樣演示的等價殘差查詢(19.6.2 F1,輔助證據,寫只落
   build/)——閘默認模式不調用,永不 ship(復審 RV1-13:初稿「只在 --record 使用」與 F1 相抵,已改寫)。
8. 不擴/不改 reimpl-eval 語義(含其 exit 3 語義);不晉升 build/ 產物(用戶裁決)。
9. 不把 S6 探針移入 git、不把它納入 ADR 交付閘或 regress;它只作為固定總閘序列內的「環境依賴直調」照跑
   (19.10 分類);S6 變換臂不作 TB 控制組。
10. 不做 X/4-state 傳播場景的 TB(X-clean 政策;x 值規則以 WAIVE x-valued 記錄)。
11. 不 bump Cargo workspace/內核 ABI 版本(ADR-0012 D18)。
12. 不改 §0..§18 任何既有文字(§18.2 勘誤由 19.2 勘誤表承載)。
13. 不做 GUI/波形 SVG 面。
14. constmuldivmod/mem_arst/operators/memory 不入採樣集;sv-tests L2 不 TB 化。
15. 不做多模組自動選頂、跨檔層級 TB。

### 19.12 風險清單

| # | 風險 | 緩解/觸發應對 |
|---|---|---|
| R19-1 | **TB 質量 flake**:X 依賴/同沿競爭/模擬器版本漂移 | Q2 雙跑+Q4 X 掃描+Q6 TB-R1x(跨檔極性感知)+Q7 跨模擬器;record.json 釘 iverilog/verilator 版本,升級=全量重錄語義事件(10.8 oracle 升級輪同律) |
| R19-2 | 看門狗誤殺(機器負載致慢) | 牆鐘 timeout ≥ 實測 30 倍;timeout 結果單列子類;TB 內看門狗(模擬時間)為主,牆鐘只防零延遲死循環 |
| R19-3 | **LLM 腿不可復現/污染**(TB 撰寫者讀過源碼) | 產物凍結入庫;協議腿表記模型/日期/污染;數字只作輔助;T 腿=不依賴 LLM 的確定性底線;「規則書→TB 能力」宣稱與機械入庫分層 |
| R19-4 | 變異充分性≠缺陷檢出;規則錨定目錄看不到規則書沒覆蓋的行 | P-unruled 池專打此盲區(不入分母,入缺陷報告);禁止宣稱④ |
| R19-5 | 等價裁決強度有限(unproven≠不等價;async2sync 抽象) | unadjudicated 保守入分母;只認 proven;命令/版本入 record;i2c-A2/A3 額外 unproven 不在本弧裁決 |
| R19-6 | 變異矩陣時長爆炸 | 每規則 ≤2+去重;C-S3 實測;>10 min 時調目錄上限=ADR 修訂;EXPECTED_DESIGNS 釘死,防靜默刪設計 |
| R19-7 | 後續 golden 重錄使變異清單漂移(本閘不在 regress) | SKILL 檢查單:golden `--record` 輪必跑 run-conformance-eval,DRIFT=同輪重錄+對賬 |
| R19-8 | D-r1c 修復波及他檔;或只修一面致規則面/破口面自相矛盾 | 兩面同片同改(select_trigger 單一語義源);普查預測只 mem_arst;diff 範圍=rules+gaps 兩檔+PROVENANCE;越界=停弧 |
| R19-9 | 新導出同 ABI 版本的裝載面缺口 | ③c 導出面檢查+紅路 |
| R19-10 | 模板腿在協議類設計(i2c)判別力弱 | 如實記錄;D 腿補強且分列;不為模板設數值下限 |
| R19-11 | 診斷回灌泄漏 oracle 期望致高估 | 按輪分列;禁止宣稱③;自驗用留一法 |
| R19-12 | 新 clone 下 run-reimpl-eval exit 3、s6 run.sh 缺(固定總閘序列在新 clone 必紅) | 19.10 分類為環境依賴直調(非交付閘);待決 19.13-2;conformance 對缺席 aux 腿一律 SKIP 不紅 |
| R19-13 | verilator 鏈接工具鏈(tapi) | DEVELOPER_DIR 三行守衛逐字沿 regress.sh:12-14;在場但失敗=紅(可診斷),不靜默 SKIP |
| R19-14 | 證據頁 plugin/rtl2spec/conformance-report.md 被手改/陳舊 | 默認模式重生成 byte 比對 |
| R19-15 | (R1 增)Q7 的 verilator 腿受第三方 DUT 告警或 DUT 固有跨模擬器差異拖累(signedexpr/subbytes 默認告警集 lint rc=1【實跑】) | `-Wno-fatal`+tb.v 位置警告必 0、DUT 位置只記錄;分歧先排查歸因,DUT 固有者先入 REGISTRY 分歧登記簿再經 ADR-0013 修訂列例外,禁靜默放行 |
| R19-16 | (復審增)D-c2 修復的擴散面:vl-syntax 是 rverilator/analyzer-core/rtl-spec 共用前端;且有既存測試把缺陷語義釘成正確 | C-S2 步 a 按語義重寫紀律+G0..G5:api.rs:128 等期望以 oracle 證據同片修正並落 ADR-0012 D1 修訂指針(復審 RV2-1);golden 允許清單=4 檔 24 葉(含 partsel 16 葉 signed,復審 RV2-2);regress 最後跑且基線零位移(corpus/ 96 檔與 sv-tests 260 檔 analyze 實驗零差);任一越界=停弧歸因 |
| R19-17 | (復審增)TB-R1x 的 E 漏列 DUT 沿行程→漏報競爭 | E=tb_plan.sens_edges 由核心枚舉 DUT **全部**沿行程敏感邊(不限產規則者);跨檔五向量 selfcheck 每次跑 |
| R19-18 | (復審增)閘定義多處文案漂移(mcp-e2e 已三分:七/10/11) | 改閘切片同片同步全部定義點,斷言數取實跑 PASS 行數;ADR 原文以修訂指針條目追加,不原地改 |
| R19-19 | (復審 R2 增)對賬物取自重新生成的輸出=空對照(golden-current 時代錯位) | 19.4.4 G0..G5:改碼前複製已存 golden/,以修後默認模式 DIFF 清單+leafdiff 雙對賬物;conformance 腿同原則(複製已存三件套) |
| R19-20 | (復審 R2 增)變異歸屬失真(共用 span 的 guard 打錯行、共享殺傷灌水規則驗證覆蓋) | 按主體定行(六設計實測全部唯一)+歸屬集逐變異入 record+規則驗證覆蓋拆獨佔/共享兩欄、禁合併 |

### 19.13 待決(用戶裁決事項)

1. build/mvp-exp/ 記錄文檔的 git 晉升落點(§18.4-1 延續;裁決前禁清理)。
2. build/reimpl-eval/reimpl/ 8 腿與 build/s6-metamorphic/ 是否晉升 git(決定固定總閘序列在新 clone 的
   可跑性;復審 RV1-2:另一解=把兩條移出固定序列)。規劃建議:晉升 8 腿入 plugin/rtl2spec/fixtures/
   (自著 MIT 的 LLM 產物,晉升後 reimpl-eval 在新 clone 可重現、exit 3 依賴消失);S6 探針是否晉升另議。
3. §18.4-3 Index/IndexedPart canon 擴名單——本弧 def_lint 為其提供證據(subbytes)。
4. S2「直通別名不產」政策在復原力角度的代價(acc_pipe `out`/subbytes)——是否以 contract 註記方式
   補足(非規則)。
5. C-S3/C-S4 實測後是否立數值殺傷率下限(ADR-0013 修訂)。
6. D-c1 net 宣告賦值提取修復的立項時點。
7. 單沿「純復位行程」(`always @(negedge rst_n) if (!rst_n) q <= 0;`)被 G4 報 missing_reset certain
   【實跑】——本弧只以 rst_only_edge.v 釘住現行行為(零漂移),該報告是否語義正確留待裁決。
8. D-c3:corpus_lint.rs R1 的同行 `#` 不關窗(:80-95 `closed` 空操作)是否在後續弧修正——修正只會減少
   假陽性(corpus 現狀零違例,不受影響),但改的是 vl-difftest 的測試側 lint,屬本弧範圍外(復審 RV2-7)。
9. D-c4:contract 對負數下標定寬範圍(`[31:-32]`)報 `width 0+parametric` 的修正時點(本弧 tb_plan 以 TB1
   拒收=安全側)。

### 19.14 本輪實跑與未核實(誠實條款)

- **實跑(全部唯讀,探針只寫 /tmp/c-arc-plan/)**:`git status --short`=0 行(開工與收尾);
  `grep -n '128\|256'` 普查;`bun /tmp/c-arc-plan/stats.mjs`(HEAD golden:843/539/196/85/23、
  async 59/26、2 證 85、when 476、truncated 0;v0.2.0:630/361/161/85/23);`git log -- plugin/rtl2spec/golden`;
  `git archive ec9527f`;雙沿普查 grep(11 檔 18 行);`bun /tmp/c-arc-plan/undef.mjs`(def_lint 探針);
  `bun /tmp/c-arc-plan/evlines.mjs`(證據首行 220);`iverilog -g2005 -t null` 六檔;`verilator --version`;
  `iverilog -V`;`bun --version`;ls(vendored yosys、build/ 產物、run-conformance-eval.sh 不存在);
  本章追加前 PLUGIN.md sha256=c9f4b1e5e1a599b0103dd0f986410df92432dc6b0c77b954ebaac372df8141e7(1995 行)。
- **未跑**:cargo build/test、regress.sh、各 run-*.sh、smoke.mjs、spec-i2c 變異測試(規劃輪不寫
  build/ 與 target/);yosys 本輪未執行;變異算子目錄、tb_plan、diagnose 均未實作未試跑(19.0-5 的
  /tmp 探針是勘察材料);時長為估算;def_lint 的 U1..U5 逐項歸類(subbytes 7 項)未逐行核;L1 93 檔
  以外語料的 D-r1c 受害面以勘察材料為準(rg 19 檔、復位前置僅 mem_arst)。

### 19.15 修訂記錄 R1(2026-11-06 對抗審 11 條逐條消化+R1 輪自查 1 條;編號 R1-1..R1-12,章外引用寫「§19 R1-n」)

本節=開弧對抗審意見的逐條處置記錄(append-only;§19 正文按條原地修訂,§0..§18 零刪改)。
provenance 同本章:【實跑】=本修訂輪命令實測(探針只寫 /tmp/c-arc-r1/,倉庫寫入僅本章與 ADR-0013);
【檔】=逐行讀;審方材料中的 /tmp/c-arc-audit 復現本輪未取得原檔,凡引用者均以本輪自跑的同形復現替代。

R1-1 **[high] Q7 正規化白名單缺 verilator 腿錨點**——已修(19.3.3 白名單分腿、19.4.1、Q7 行、19.5 ②)。
本輪同形復現【實跑】:/tmp/c-arc-r1/q7/tb.v(12 行:`SCENE1`/`  v=2a`/`-7div2=-3`/`DONE`/`$finish`)。
`iverilog -g2005 -o sim tb.v && ./sim` 的 stdout 尾行=`tb.v:10: $finish called at 1000 (1ps)`(審方
無 timescale 形為 `(1s)`,錨定 regex 兩形皆蓋);DEVELOPER_DIR=CLT 守衛下 `verilator --binary --timing
-Wno-fatal --Mdir obj tb.v` 後 `./obj/Vtb` 的 stdout 在 DONE 後追加 4 行——`- tb.v:10: Verilog $finish`/
`- S i m u l a t i o n   R e p o r t: Verilator 5.052 2026-09-05`/`- Verilator: $finish at 1ns;
walltime 0.000 s; speed 55.556 us/s`/`- Verilator: cpu 0.000 s on 1 threads; allocated 2 MB`(walltime/
speed/cpu/MB 皆機器相關);`-7div2=-3` 設計行兩腿都在。審方結論成立:原白名單下 Q7 在 verilator 在場的
本機必全紅。修法不即興發明:verilator 腿**沿用倉內既有且已釘死的契約**——vl-difftest
`normalize_oracle_stdout`(crates/vl-difftest/src/lib.rs:124-133)的四個精確前綴 `- `/`-Info`/
`-Warning`/`-Error`,其敵意測試 corpus_lint.rs:185-188(`-7div2=-3` 設計行存活)【檔】;JS 移植以
corpus_lint.rs:186 原始字串逐位元組復現 :188 期望作 parity,並把上述 4 行實錄與 `(1s)`/`(1ps)` 兩種
橫幅、`  v=-3` 協議行列入 selfcheck ② 敵意向量。防誤吞論證寫入 19.3.3:協議 v1 合法行無一以 `-`
開頭,且 Q5 文法檢查作用於不丟 `-` 行的 iverilog 腿,TB 打印任何 `-` 開頭行都會先在 iverilog 腿紅。

R1-2 **[medium] D-r1c 負樣 `rst_first_noreset.v` 規格與提取器實際行為矛盾**——已修(19.2 fixtures 段、
驗收「+5 行」、19.9 C-S1「5 檔」)。dist wasm 實探【實跑】`bun /tmp/c-arc-r1/probe/p.mjs
plugin/rtl2spec/dist/wasm`(rtl_extract_rules_sv,max=0):p1 `@(negedge rst_n or posedge clk) if (load)
q<=1; else q<=d;`→`guard(when=load,async=false,trig=rst_n negedge)`+sequence——**非零 guard**,與審方
一致(sv/mod.rs:784-802:`load`≠trigger→照取復位臂);p2 頂層條件複合式 `if (load && en)`→零 guard
(sequence×2);p3 復位前置 `if (!rst_n)`→零 guard(sequence×2,即 D-r1c bug 現形);p4 時鐘前置孿生→
`guard(when=!rst_n,async=true,trig=clk posedge)`+sequence;p5 單沿 `@(negedge rst_n) if (!rst_n)`→
零 guard(sequence×1)。據此:負樣改為 `rst_first_compound.v`(p2 形,「頂層 If 條件非單訊號形」);
p1 形改作零漂移釘 `rst_first_loadsig.v`(修前修後同為 guard when=load async=false trigger=首沿);
p3/p4/p5 三形規格不變(p3 修後期望=與 p4 載荷逐欄相等,屬修法規格,本輪無法實測)。

R1-3 **[medium] 19.4.5 與 19.11-2(及 ADR D-C5 與非目標)字面相抵**——已修。補單一定義「閘判據劃界」
(19.1 新 bullet):凍結入庫工件(T/D 腿 tb.v、錄製 expected/record、機械變異)的機械可測屬性構成
入庫與閘 exit 判據;LLM 採樣過程(prompt/模型/採樣次數/K 選擇/輪次)與重寫產物計分不構成閘 exit
判據(10.11 與 §18.3 S5 先例本義)。19.4.5 加註「判的是凍結工件」;19.11-2 改寫並注明初稿另一錯:
「機械變異的分數不構成入庫判據」與 Q8(殺傷矩陣=核心判據)自相矛盾。ADR-0013 D-C1/D-C5/非目標同步。

R1-4 **[low] 判別力粒度**——如實記錄,判據不改(Q8 行加「斷言粒度」)。實際粒度:變異側=逐變異體
(結果類+殺傷場景集都與 record.json 雙向比對);正樣=全輸出 byte 相等(涵蓋全部場景);selfcheck ②
含「正樣期望誤配某 killed 變異輸出必紅」;鐵則下限僅 per-design ≥1 killed,per-rule(規則驗證覆蓋)
與 per-scene 判別力只錄製不斷言——屬已登記的設計選擇(19.13-5 待決),立下限唯一路徑=ADR-0013 修訂。

R1-5 **[low] OP-STATE 枚舉源與排序鍵未定義**——已修(算子表 OP-STATE 行+表下「確定性定義」注)。
定義改為源碼級:同模組同 state_reg 的 transition 規則 evidence 區間內 `<state_reg> <= X;` 行的 RHS
原文,按行號升序首次出現去重成 Vec,循環取下一個;`to` 歸屬用單一規範映射。實勘依據【實跑】
`bun /tmp/c-arc-r1/probe/fsm.mjs plugin/rtl2spec/golden/yosys-simple/fsm.rules.json`:fsm 的 R001/R002
evidence 同為 31-36(`100: begin` 臂區間),`to` 渲染為 `'sd200`/`'sd300`(源碼 `200`/`300`)——若按
渲染文本或集合迭代取值,record 必漂;C 逐 state_reg 錄入 record.json。

R1-6 **[low] OP-ASYNC 只有 `or` 形**——已修(OP-ASYNC 行四形皆蓋+全算子「錨點落空一律留痕」注)。
依據【實跑,首輪雙沿普查】`corpus-external/yosys-simple/sincos.v:45:always @(posedge clock, posedge
reset) begin: DESIGN_PROCESSOR`=逗號形;i2c_master.v:70/subbytes.v:28 為 `or` 形。四形=` or <沿>
<rst>`/`, <沿> <rst>`/`<沿> <rst> or `/`<沿> <rst>, `;恰一形 count==1 才手術,否則記 `ambiguous`,
不適用記 `n/a`,兩類都入 record.json 並計入目錄覆蓋分母,禁靜默跳過。

R1-7 **[low] scenesParsed 落點與 parity 斷言互斥**——已修(19.5 D-C8 段)。實勘【實跑】`grep -n 'clock'
plugin/rtl2spec/scripts/reimpl-eval.mjs`→僅 `70:const clock = (label, fn) => {`(只定義零調用),verdict
唯一變異源=generatedAt。裁定:verdict.json 結構凍結,scenesParsed 寫 sidecar
build/reimpl-eval/scene-protocol.json+console 一行,parity 比較器不需任何新鍵容忍例外。

R1-8 **[low] ⑤ 漏檢回退 token**——已修(19.2 D-r1a ⑤(c)、⑧ 四調用)。實勘【實跑】`sed -n 146,151p
plugin/rtl2spec/dist/server.mjs`→`const max = Number.isFinite(args.max_rules_per_module) &&
args.max_rules_per_module > 0 ? Math.floor(args.max_rules_per_module) : 128;`——實參是變數 `max`,⑤(a)
字面量斷言看不到回退值。補 ⑤(c) 回退值與 batch 實參的錨定 regex(必為字面量 0),⑧ 補顯式 0(走
核心默認)與顯式 100(上限仍被尊重,truncated=[i2c_master])兩次調用;紅路演練兩路(:150/:219 各
改回 128 必紅)。

R1-9 **[low] obligations `observe=internal` 誘導層次引用**——已修(19.3.2 obligations 改名
`port|indirect` 並定義:indirect=效應須經下游輸出端口表面化,不能則 `WAIVE <id> unobservable`;模板腿
以 `*` 非定向場景覆蓋)。

R1-10 **[low] 預登記缺不可抵賴時間戳**——已修(19.3.4 前言「時間戳邊界」段)。內容錨【實跑】:
`awk '/^#### 19\.3\.4/{f=1} f&&/^```text$/&&!s{s=NR;next} s&&!e&&/^```$/{e=NR} END{print s, e}'
docs/RTL2SPEC-PLUGIN.md` 定位圍欄,`sed -n "$((s+1)),$((e-1))p" docs/RTL2SPEC-PLUGIN.md | shasum -a
256`=`cc28f0c73dc4ef3afc1838b839d44903d7dc9ee9b9f15b5ebd3aceaca532a941`(34 行);本輪原地修訂使塊
位移(現 2248-2281),修訂後重算 sha 不變(實跑兩次)。不可抵賴時間戳=用戶提交本章的 commit;
CONFORMANCE-PROTOCOL §0 必載首次提交版為準+C-S4 開工重算 sha 相等+改動=協議事件 append-only。

R1-11 **[low] fsm 無 guard→模板腿復位路徑不受定向激勵**——已修(19.3.2 新增「resets 導出口徑」、19.7
fsm 行)。實勘【實跑】:fsm golden 23 條=trans 5/seq 18、guard 0(首輪 stats.mjs);同步復位
`state <= 100;`(fsm.v:28)落為 sequence `fsm_test.R011`(when=`reset`,fsm.mjs 探針)。v1 不以端口名/
非 guard 規則猜復位(禁啟發式);resets=∅ 時 S-RST/S-ARST 不生成、guard 類算子不產生,證據頁與 verdict
機械註記成因,防低殺傷率被誤讀為規則書/設計缺陷(禁止宣稱④);fsm 在 tbSample 內,由定向腿補強且分列。

R1-12 **[自查新增,medium] Q7 的 verilator 構建會被第三方 DUT 告警擊穿**——已修(19.4.1、Q7 行、風險
R19-15)。實跑【實跑】(cwd=/tmp/c-arc-r1,絕對路徑):`verilator --lint-only --timing <design.v>` 六檔
——i2c_master/acc_pipe/fsm/sincos rc=0 告警 0;signedexpr rc=1(%Warning-WIDTHEXPAND,2 條);subbytes
rc=1(%Warning-LATCH+%Warning-WIDTHTRUNC,2 條)。`--binary` 默認告警致命,照初稿 Q7 這兩檔必構建失敗,
與 X/競爭無關。裁定:構建加 `-Wno-fatal`;位於 tb.v 的 verilator 告警必為 0(TB 自身仍守警告即錯誤
精神,AGENTS §2),位於 DUT 的只記錄(第三方唯讀語料不可改);Q7 分歧經排查屬 DUT 固有者=先入
REGISTRY 分歧登記簿(10.5)再經 ADR-0013 修訂列例外,禁靜默放行。

- **同輪同步**:ADR-0013 D-C1(劃界)/D-C3(分腿正規化)/D-C4(OP-STATE 源碼級定義、OP-ASYNC 四形、
  錨點落空留痕、Q7 構建旗標)/D-C5(劃界+粒度)/D-C8(sidecar)/D-C12(⑤(c)、⑧ 四調用、五檔 fixture)/
  機械閘第 2 條(+5 行)/非目標摘要/實作記錄 R1 行。§0..§18 零刪改(前 1995 行 sha256 斷言
  c9f4b1e5e1a5…不變);§19.3.4 模板塊內容零改(sha256 cc28f0c7… 不變)。
- **本修訂輪未跑**:cargo build/test、regress.sh、各 run-*.sh、smoke.mjs、yosys 均未執行(規劃輪不寫
  build/ 與 target/);R1-2 的「修後」期望(p3→guard async=true)是修法規格,修法未實作、無法實測;
  verilator 腿的 `-Wno-fatal` 構建只在 R1-1 的最小 TB 上實跑過,六設計的實際構建/運行與 Q7 相等性
  留待 C-S3 實測;審方 /tmp/c-arc-audit 原檔未取得,未逐字比對。

### 19.16 復審修訂 R1(2026-11-07 換人復審 14 條意見逐條消化+復審自查 2 條;編號 RV1-1..RV1-16,章外引用寫「§19 RV1-n」)

本節=換人復審意見的逐條處置記錄(append-only;§19 正文按條原地修訂,§0..§18 零刪改,§19.15 既有條目
不改)。provenance 同本章;探針只寫 /tmp/c-arc-rr/。

RV1-1 **[high] D-r1c 漏掉 gaps.rs 同款首沿邏輯(G4 假陽性),且 diff 範圍斷言會擋住必要變更**——已修
(19.2 D-r1c 修法段、19.0-10、fixtures 段、golden 段、19.9 C-S1、R19-8)。實證【實跑】:`crates/rtl-spec/
src/sv/gaps.rs:192-217` 復位臂識別同取首沿(註釋 :194-195「與規則提取面同一判準」);dist wasm
`rtl_scan_gaps_sv` 五形探針(/tmp/c-arc-rr/g.mjs):復位前置→`G4-m-001 missing_reset certain`、時鐘
前置→`[]`、複合條件→certain、loadsig→`[]`、單沿→certain;在庫 golden/yosys-simple/mem_arst.gaps.json
含 `G4-MyMem-001` certain。裁定:兩面同片同改(共用 `select_trigger`),否則規則書稱有 async guard、
破口報告稱無復位臂,自相矛盾而 D-r1c 被記為收平(看起來綠、語義錯);**破口面檔級矩陣新增五行**(期望以
實探定案:rst_first_async 修後零 G4、其餘四檔修前修後同),diff 範圍放寬為
`mem_arst.rules.json+mem_arst.gaps.json+PROVENANCE.md`。RTLIL 面 rtlil.rs:197-209=潛在同形第三處
(19.0-10 實探:yosys 產兩序 .il sync 序確隨敏感列表,但 RTLIL 面對兩序皆 0 規則,未現形)——本弧不改、
只登記(19.11-5),無現形樣的改動無判別力 fixture。

RV1-2 **[medium] 「現役 10」把 gitignored 的兩條列為總閘現役,與 19.11-9/腳本頭自述/ADR-0012 現役
8 條相抵;run-reimpl-eval exit 3 使 LLM 產物在場成為交付判據**——已修(19.10 重寫、19.11-8/9、
R19-12、19.13-2)。總閘 11 命令序列(ask 固定契約)不動,地位改三類:ADR 現役交付閘 8(=ADR-0012
機械閘 4 的現役 8 條,in-git 可重現)/規劃閘 1(conformance,C-S3 後轉現役)/環境依賴直調 2
(reimpl-eval、s6 run.sh——非 ADR 交付閘、非 regress 成員,exit 只證本機環境可跑;reimpl-eval 依賴
LLM 產物=量測床性質,19.1 劃界下不作任何確定性判據)。固定序列與「交付閘不依賴 LLM 產物」的張力
升級為待決 19.13-2(晉升 8 腿入 fixtures=規劃建議,或移出固定序列)。

RV1-3 **[medium] MCP 4→5 相抵於 ADR-0012 D4/非目標與 §11-9「既有清單不變」;run-mcp-e2e 定義已
三分且 C-S6 未列同步**——已修(19.8 兩個新 bullet、19.11 標題、19.9 C-S1/C-S2、R19-18)。①D4/
非目標/§11-9 三處「四工具」由 ADR-0013 D-C11 **顯式修訂**:stdio 單傳輸+零 HTTP/WS 面**不變**,工具
4→5(C-S2 生效);原文 append-only 不改,修訂關係記於 ADR-0012 實作記錄修訂指針條目+19.8;
19.11 標題加注此唯一例外。②定義同步面實勘【實跑 grep '七斷言'】:AGENTS.md:305、adr/0012:106、
REGISTRY.md:84、run-mcp-e2e.sh:7/:57、mcp-client.mjs:2-7 全部寫「七斷言/恰 4 工具」,OKR.md:62 寫
「11 斷言」,而 mcp-client.mjs 實有 10 個 `check(` 呼叫點——三分。規則:改閘切片同片同步全部定義
點,斷言數一律取該片實跑 PASS 行數;C-S1(⑧)與 C-S2(②⑨⑩)各同步一次;adr/0012 機械閘 1 原文
不改、由修訂指針條目指向 ADR-0013。

RV1-4 **[medium] contract ANSI 端口寬度繼承缺陷使 tb_plan 生成寬度錯配 TB;⑨ 只查字串偵測不到**——
已修(19.0-11、19.2 C-S1 末新增登記、19.3.2 新增 C-S2 首步與 contract 前提段、19.8 ⑨、19.9 C-S2 步 a、
R19-16)。實證【實跑】:parser module.rs:132-171 消費方向關鍵字時不清 cur_range/cur_signed;oracle 對照
(/tmp 最小樣):iverilog 13.0 與 verilator 5.052 皆 `busy=1 bits=1`,倉內 2026-10-27 舊構建
target/debug/rverilator 為 `busy=1111 bits=4`(HEAD 未重建,記錄時注明);dist wasm:fsm_counter.v 的
busy 報寬 4、golden mem_arst 四端口誤標 parametric(width 0+P);啟發式普查(L1 93 檔+fsm_counter)
6 處 5 檔(mem_arst/retime/memory/loops×2/fsm_counter),corpus/ 96 檔 0 處,19.7 六採樣設計不受影響。
處置:C-S2 **首步**=vl-syntax ANSI 組重置+三形單測+全量 cargo test+regress 基線零位移+golden 錄前快照
對賬(只許 mem_arst/memory/loops 的 contract 欄位);REGISTRY 「C 弧已知未決」首錄 **D-c2**(10.5 先
登記,必須先於修復)。⑨ 補機械偵測:rtl_tb_plan 模板與 fixture 同編 `iverilog -g2005 -t null` 斷言
exit 0+warning=0(iverilog 在場才執法,不在場 SKIP 行,防 regress 雙守衛新增硬依賴);tbplan 單測另斷言
busy 寬 1(不依賴外部工具)。

RV1-5 **[medium] TB-R1「移植+三向量 parity」不可行:R1 是同檔不分極性文本規則**——已修(19.3.2
TbPlan 增 `sens_edges` 欄、19.3.3 靜態約束改寫+TB-R1x 定義、19.4.1/19.4.3 Q6、19.5 ②、19.11、
R19-1/R19-17、19.15 復用條目注記)。實證【檔】corpus_lint.rs:30-51(edge_read_vars 只收同檔沿行程
標識符)、:53-110(任何 posedge/negedge 後阻塞寫皆判,不分極性)。TB-R1x 新定義(跨檔、極性感知):
E=tb_plan 的 sens_edges(核心枚舉 DUT 全部沿行程敏感邊,不限產規則者;閘側不解析 DUT 文本),M=tb.v
具名連接 net→port 映射;違例=tb.v 事件控制後語句窗(窗口規則同 corpus_lint)內對某 TB 變量的阻塞寫,
且該變量經 M 連到 DUT 輸入端口且 (e,M(net))∈E;協議允許的「@(<沿>); #1;」天然不在窗內;TB 內部
沿行程照跑同檔 R1 原規則+三向量 parity。跨檔四向量進 selfcheck ②(正/負×pos/neg,DUT/TB 片段=
fixtures/conformance-lint/ 自著 MIT 小檔,正向量誤配負向量必紅);正文「輸入只在 DUT 不採樣的邊沿…改變」
同步改寫。

RV1-6 **[medium] 「錄後必讀 git diff fixtures/conformance」對 untracked 目錄恆空,對賬形同虛設**——
已修(19.4.4 對照物段、19.9 C-S3/C-S4/C-S5)。改為 AGENTS 10.11「錄前 compare 快照」機械化:
`--record` 必帶 `--slice <C-Sn>`(缺=exit 2),寫前把現存三件套複製到
plugin/rtl2spec/build/conformance/pre-record/<切片>/,寫後生成 record-diff-<C-Sn>.md(逐設計 expected
行級 diff+變異結果/殺傷場景集增刪改+判別力數字前後值);C-S3 首錄=全量新增清單;每片實作記錄必引其
record-diff 檔。golden 同律:C-S1=弧內首次重錄(對 HEAD 的 git diff 即本片變更),C-S2 起對照物=錄前
build/golden-current/。

RV1-7 **[low] C-S2 wasm 重建使 validation-report.md:5 與 docs/RTL2SPEC-EVAL.md:5 的 sha 戳變,
切片未列重生成對賬**——已修(19.9 C-S2 步 b)。實證【實跑】兩頁 :5 內嵌 wasm sha256 現值
23af31de63d6…。步 b 必重生成兩頁並對賬「只許 sha 戳行變」;步 a(D-c2)另有可歸因變更(EVAL 的
contract 欄位衍生物),驗收面已在步 a 列明。

RV1-8 **[low] validate-manifest.mjs:62 硬編碼三個 skill 目錄,新 skill 不受校驗**——已修(19.8 新
bullet、19.9 C-S2 步 b)。實證【實跑】`sed -n 58,66p`:`const skillDirs=['rtl-analyze','rtl-rulebook',
'rtl-gap-hunt']`。C-S2 同片加入 `rtl-conformance`+紅路演練(暫刪其 frontmatter description 必紅→還原綠)。

RV1-9 **[low] D-r1b 勘誤表行號錯標(:1929 應為 :1930)**——已修(19.2 表內該行)。審方並確認其餘
數字(843/539/196/85/23、async 59/26、2 證 85、when 476、v0.2.0 630/+213)獨立重算吻合。

RV1-10 **[low] rst_first_twin「僅 evidence 行號不同」不準確**——已修(19.2 fixtures 段)。實證【實跑
g.mjs】p4 guard 的 evidence[1]=`  always @(posedge clk or negedge rst_n)`,與復位前置形文本必然不同;
斷言改為「check 載荷相等,evidence 文本與行號不同」。

RV1-11 **[low] `tb_plan(...) -> TbPlan` 不會失敗的簽名;wasm 導出參數/回傳形態未定義**——已修
(19.3.2 API 段重寫)。`tb_plan(input, module) -> Result<TbPlan, TbError>`(TB1 全部在計劃期攔,含
truncated 非空;上限參數移除=少一個可漂移旋鈕);`tb_template_v(&TbPlan, &TbOpts) -> String`(不可
失敗;TbOpts 默認值 SSOT=tbplan.rs 常量,seed 非零);wasm `rtl_tb_plan_sv(name, source, module,
seed, cycles, flush) -> String`:空串=未指定/0=核心默認(與 max_rules==0 同約定),回傳
`{"ok":true,"plan":…,"template_v":…}` 或 `{"ok":false,"error":"error: … (milestone TB1)"}`(錯誤走
資料不 trap);native 對拍腿=emit.rs 增 `tbplan` 模式(實勘 emit.rs 已有 lang/mode/name/file/max 五元
組分派,:8-51,擴展而非另立)。

RV1-12 **[low] C-S6 未列 PLAN.md 里程碑表與 OKR KR 指針;證據頁成第二個出 plugin 寫面**——已修
(19.9 C-S6、19.5 ④、19.12 風險 R19-14)。實證【實跑】PLAN.md:111(RTL1..RTL5 行
先例)、docs/OKR.md:62(KR1.11 證據指針先例)、run-realworld-eval.sh:9-10「唯一出 plugin 的寫面」。
C-S6 加 PLAN 里程碑表新行(TB1 里程碑族,AGENTS §3)+OKR 新 KR 指針=run-conformance-eval;證據頁
改落 **plugin/rtl2spec/conformance-report.md**(validation-report.md 同位先例),本閘全部寫面收進
plugin/rtl2spec/ 內,AGENTS 10.12 同步聲明「無新增出 plugin 寫面」。

RV1-13 **[low] 19.11-7「yosys 只在 --record 使用」與 F1 等價殘差查詢相抵**——已修(19.11-7、19.6.2
F1 條加注)。yosys 兩個用處並列:--record 存活裁決(結果入 record)+F1 演示的殘差查詢(輔助,寫只落
build/);皆非閘 exit 判據。

RV1-14 **[low] `scripts/` 根目錄歧義;def_lint 是語義分析卻放 JS 正則腳本**——已修(章首路徑約定、
19.5 tbproto 全路徑、19.6.1 標題全路徑、19.6.1 def_lint 定位聲明、19.9 C-S3/C-S5 全路徑)。實證【實跑】
§19 內 scripts/ 引用普查(對 HEAD 後的章節文本 uniq -c):4×scripts/regress.sh(repo 根)+其餘皆插件
根短寫。約定:`scripts/regress.sh`=repo 根;其餘短寫=plugin/rtl2spec/scripts/;新檔一律全路徑。def_lint
明定=閘側工具(只在 plugin/rtl2spec/scripts/conformance/、只被 run-conformance-eval 調用、不經 wasm/
MCP/skills/規則書 JSON 露面),晉升核心=後續立項走 10.10。

RV1-15 **[復審自查] D-r1a 普查漏 smoke.mjs:79**——已修(19.2 D-r1a「非面」段)。實證【實跑】
`grep -n "128\|256" crates/rtl-spec-wasm/smoke.mjs`→:79 parityCases 兩側同傳 128(對稱比對,非面
漂移);首輪普查只掃 plugin/ 與兩個 Rust 檔而漏列,已補。

RV1-16 **[復審自查] D-r1c 單沿形被 G4 報 missing_reset certain(既非本弧改動面、亦未登記)**——
已登記(19.13-7 新待決)。實證【實跑 g.mjs】p5 `always @(negedge rst_n) if (!rst_n)`→G4 certain;
本弧以 rst_only_edge.v 零漂移釘住現行行為,語義可議性(單沿「純復位行程」是否該報 missing_reset)
留待用戶裁決,本弧不改。

- **同輪同步**:ADR-0013(D-C1..D-C13/機械閘/非目標/實作記錄 RV1 行);ADR-0012 實作記錄追加第二條
  修訂指針(D4 四工具→五工具、機械閘 1 定義同步規則;原文零改)。§0..§18 零刪改(前 1995 行 sha256
  斷言 c9f4b1e5e1a5…不變);§19.3.4 模板塊內容零改(sha256 cc28f0c7… 不變)。
- **本復審輪未跑**:cargo build/test、regress.sh、各 run-*.sh、smoke.mjs、舊 rverilator 重建(ANSI
  oracle 用 2026-10-27 舊構建,HEAD 行為未實測——但 oracle 判準是 iverilog/verilator 兩模擬器一致,
  舊構建僅輔助展示缺陷面);g.mjs/p.mjs/c.mjs/普查腳本為唯讀 dist wasm 與文本掃描,倉庫零寫入。

### 19.17 復審修訂 R2(2026-11-07 換人復審第二輪 14 條意見逐條消化+1 條覆蓋面聲明照錄+本輪自查 1 條;編號 RV2-1..RV2-15,章外引用寫「§19 RV2-n」)

本節=第二輪換人復審意見的處置記錄(append-only;§19 正文按條原地修訂,§0..§18 零刪改,§19.15/§19.16
既有條目不改——其中的錯誤在此勘誤)。provenance 同本章;本輪探針與實驗寫只落 /tmp/c-arc-r2/(含一份
`rsync` 出的 workspace 副本 /tmp/c-arc-r2/ws,倉庫本體零寫入,倉庫內未跑任何閘)。

RV2-1 **[high] D-c2 修法照做必使 `cargo test --workspace` 紅:既有測試把缺陷釘成正確;ADR-0012 D1 被觸犯
卻無修訂指針**——已修(19.3.2 C-S2 首步「同案文件面」、19.9 步 a、R19-16、ADR-0013 D-C13、ADR-0012 實作
記錄修訂指針第三條)。實證:①【檔】crates/analyzer-core/tests/api.rs:111-129 `sv_ansi_inheritance_leg`
的源碼即 `module m(input [3:0] a, b, c, output d)`,:119-121 註釋稱「依 LRM §12.3.3 繼承前組 [3:0]」,
:128 `assert_eq!(port("d").width, 4)`;同族措辭=module.rs:139-140、ast.rs:97、analyzer-core/src/sv.rs:132、
api.rs:58。②【實跑 oracle】/tmp/c-arc-r2/ansi/v1.v(同一宣告+`$bits` 打印):iverilog 13.0 `-g2005` 與
`-g2012`、verilator 5.052 `--binary --timing` 三方皆 `a=4 b=4 c=4 d=1`——測試期望錯、修法對(AGENTS §1)。
③【實跑】在 /tmp 副本按 19.3.2 修法打補丁後 `cargo test --offline --workspace --no-fail-fast`:86 個
suite 綠、2 個敗——`sv_ansi_inheritance_leg`(api.rs:128 `left: 1 right: 4`)與 riv-gen
`gen_rust_roundtrip_on_subset`;後者錯誤為 `extern location for vl_values does not exist: …/target/debug/
libvl_values.rlib`,在副本 `cargo build -p vl-values -p vl-ops -p riv-ir -p riv-kernel` 後重跑 `cargo test
-p riv-gen --test roundtrip` 1/1 綠=fresh 副本缺前置 lib 產物的環境假象(審方歸因 CARGO_TARGET_DIR=同一根
因);把副本 api.rs:128 改為 `1` 後 `cargo test -p analyzer-core --test api` 13/13 綠。處置:api.rs:128 期望
與上列註釋/doc 同片改寫進 C-S2 步 a;D1「禁破壞其測試與 API」由 ADR-0013 D-C13 顯式修訂(API 簽名零改,
只改一個被 oracle 證偽的期望值與註釋),ADR-0012 追加修訂指針。

RV2-2 **[high] golden diff 允許清單照做必越界:修法同時重置 cur_signed,partsel 也必變**——已修(19.0-11、
19.3.2、19.9 步 a、R19-16)。實證【實跑】:副本以 native `emit`(cargo build -p rtl-spec --example emit)
對釘選 37×2 先跑未補丁版——與在庫 golden 逐位元組 74/74 相同(方法校驗),再跑補丁版並以
/tmp/c-arc-r2/exp/leafdiff.mjs 逐葉比對:`same 70 diff 4`=loops 4 葉(ports[6]/[7]/[13]/[14] width 4→1)、
mem_arst 2 葉(ports[5] width 0→1、parametric true→false)、memory 2 葉(modules[10] ports[5]/[6] width
4→1)、**partsel 16 葉(modules[3..6] ports[2] 與 modules[7] ports[4..15] 的 signed true→false)**——與審方
數字全同;gaps 37 檔零變。【檔】partsel.v:70-71 `input signed [4:0] n, output reg [31:0] dout`,golden 中
partsel_test004 的 dout 為 signed:true;oracle(v7 同形)三方皆 dout 無號(`dout=4294967295`、`dout<0` 為假)。
擴大普查:L1 93 檔 rules+analyze 另見非釘選 retime(1 葉);corpus/ 96 檔與 sv-tests 260 檔 `analyze` 全零差。
根因同審方:初稿普查只掃範圍繼承且按首端口計(「6 處 5 檔」),漏 signed 族並低估同組後續端口。

RV2-3 **[medium] 步 a 把 regress 排在 golden 重錄之前,regress S7 內含 golden 回歸必紅**——已修(19.3.2、
19.4.4 G 程序 G5、19.9 C-S1/步 a)。【檔】scripts/regress.sh:383-386 為 run-golden-regression 調用。改為
G0..G5:重錄並對賬(G2/G3)→默認模式綠(G4)→**最後** regress 取綠且基線行逐字不變。審方副本同次實跑的
基線數字(E2E 59/59、lint 83/87、riv 58/59 與 21/22、ys-e2e 469/479、analyzer smoke PASS)未動=零位移預測
成立,錯的只是順序——本輪未在倉庫或副本跑 regress(副本缺 vendor-reference,LIVE ys 不可驗)。

RV2-4 **[medium] golden 腿「錄前快照=build/golden-current」是空對照**——已修(19.2 C-S1 golden 條、19.4.4
「兩腿同一原則」與 G 程序、R19-19)。【檔】golden-check.mjs:231-246:默認模式把**當前提取器**輸出寫入
build/golden-current 再與 golden/ 比;run-golden-regression.sh:56-70 先重建陳舊 pkg——改碼後產出的就是修後
結果,與隨後 `--record` 寫入相同,比對恆零差。改為:G0 改碼前 `cp -R` 已存 golden/ 到
build/golden-pre-<片>(時代以內容判定,AGENTS 10.11),對賬物 #1=修後默認模式的 `DIFF` 清單(錄製前、
對照=工作樹 golden/),#2=`--record` 後 golden-leafdiff.mjs 對 G0 副本的逐葉差;conformance 腿(RV1-6)
同原則——對照物永遠是被覆寫前的已存工件。C-S1 的「git diff --stat 與快照一致」改寫為只在 G0==HEAD 時
成立的交叉核對。

RV2-5 **[medium] 變異錨點對 guard 打錯行、去重無歸屬語義、渲染值與阻塞賦值未覆蓋**——已修(19.4.2 規則錨定/
norm/算子表/變異歸屬、判別力數字兩欄、verdict 形態、R19-20)。實證【實跑 /tmp/c-arc-r2/anchor.mjs,讀在庫
golden+源檔】:①初稿「多行 span 找第一個含目標 token 的行」模擬=i2c 20 條 guard 只落 7 行,值為 `1'b0` 的
11 條全落 :74(`half <= 1'b0;`),sincos 2 條落 :55、6 條落空,subbytes 3 條全落空——與審方一致。②改為
「候選區內賦值左值經 norm 後==規則主體的唯一一行」:六設計全部唯一(guard 20/20、8/8、3/3;sequence
123/123、5/5、18/18、38/38、3/3;relation 4/4、6/6、10/10;transition 行 9/9、5/5)。③阻塞賦值錨:sincos
guard 5/8、sequence 34/38(非阻塞 4/38,與審方「非阻塞僅 4/38」互洽;審方所記「15 條」本輪未能以同一
口徑重現,以本輪實測為準)、subbytes 16/16——OP-HOLD 改為 `<=`/`=` 皆覆蓋。④norm(去空白+無位寬十進制
N→`'sdN`)下六設計 guard/sequence 源碼 RHS 與渲染值全等;relation 2 條(signedexpr)因渲染去括號不等,
但 relation 不比 RHS。⑤歸屬集逐變異入 record;規則驗證覆蓋拆「獨佔/共享」兩欄,禁合併(OP-POL/OP-ASYNC
天然共享)。

RV2-6 **[low] 只在方向關鍵字處重置,與「方向/種類/型別全省略才繼承」不一致**——已修(19.0-11、19.3.2 修法
與七向量表、19.9 步 a)。實證【實跑】oracle 七向量(/tmp/c-arc-r2/ansi/v1..v7):v4 `input [7:0] a, wire b`、
v5 `output reg [7:0] a, reg b`、v6 `input signed [7:0] a, [3:0] b` 在 iverilog `-g2005` 報「Partial ANSI port
declaration requires SystemVerilog」,`-g2012` 與 verilator 皆 b=1、b=1、b 為 4 位無號(b=15);HEAD(dist
wasm `rtl_analyze_sv`)報 b 寬 8、寬 8、signed=true——七向量全錯。R2 修法=**新組一律重置四個滾動變量,只
繼承方向**(新組=peek_starts_port_spec 命中,parser/mod.rs:449-460 涵蓋方向/reg/signed/net/`[`);副本補丁後
`emit … analyze` 七向量全部與 oracle 相符(v1 d=1、v2 busy=1、v3 b=1、v4 b=1、v5 b=1、v6 b 無號 4 位、
v7 dout 無號),且其釘選 37×2 擴散面與審方的「僅方向重置」補丁完全相同(RV2-2 的 24 葉)——寬化重置未
擴大 golden 變更面。實作須把四個滾動變量的綁定移到組循環頂(探針補丁那種「先初始化後覆寫」寫法實測報
4 條 unused_assignments,違 AGENTS §8)。

RV2-7 **[low] TB-R1x 窗口定義與 corpus_lint 實際行為不符**——已修(19.3.3 TB-R1x 語句窗定案、Q6 行、19.5 ②
第五向量、19.11-5、19.13-8;REGISTRY D-c3 於 C-S1 登記)。實證【實跑】:把 corpus_lint.rs:14-131(idents/
edge_read_vars/race_findings)原文編入 /tmp/c-arc-r2/lint/main.rs(`rustc --edition 2021`)跑六向量——
同行 `@(posedge clk); ld = 1;` 1 條、同行 `@(posedge clk); #1; ld = 1;` 0 條、NBA 0 條、**`@(posedge clk); #1;`
+換行 `ld = 1;` 1 條**、`#1;` 獨佔一行 0 條、`@(negedge clk); ld = 1;` 1 條。【檔】:80-95 的 `closed` 是空操作,
與 :3-6 文檔「no intervening #delay」相悖。定案:TB-R1x(及 TB 內部的同檔 R1)取文檔語義(同行 `#` 即關窗),
與 Rust 的 parity 只聲明於三個同行向量,換行形為已登記差異並以第五向量釘住 JS 側行為。

RV2-8 **[low] TB1 清單未覆蓋解析錯誤/模組不存在/inout/孤兒端口/多時鐘/內部時鐘;MCP 無 lang 致「RTLIL 輸入」
TB1 不可達**——已修(19.3.2 硬錯十一條、wasm `rtl_tb_plan_rtlil`、MCP lang、smoke 三路、mcp-e2e ⑪、19.8、
19.11-4)。【檔】analyzer-core/src/sv.rs:132-134 註明孤兒端口投影為 Inout+width 0;server.mjs:77-82 的 `langOf`
已是其餘工具共用的分派。影響面【實跑】:釘選 37 檔 golden 的 guard trigger 多時鐘 0、內部時鐘 0
(/tmp/c-arc-r2/clk.mjs),`grep -lw inout corpus-external/yosys-simple/*.v` 0 檔——只影響工具面,不影響六個
採樣設計(與審方結論同)。

RV2-9 **[low] EVAL「只許 sha 戳行變」不可滿足**——已修(19.2 連帶重生成、19.3.2、19.9 步 a/b)。【實跑】
`sed -n 1,8p` 兩頁:EVAL 的 `- 日期:`(:3)/`- 工具鏈:`(:5,含 wasm sha)/`- 耗時:`(:6);validation-report
的 `- 日期:`(:3)/`- 工具鏈:`(:5)。改為按行首前綴定義的易變行白名單,白名單外變更逐行歸因;審方實見
EVAL 變 :3/:5/:6(165ms→99ms)、validation-report 只變 :5(同日重跑)。

RV2-10 **[low] RV1-15 行號不實;普查漏 emit.rs 默認值**——已修(19.2 D-r1a 非面、修法、⑤(d))。【實跑】
`grep -n '128' crates/rtl-spec-wasm/smoke.mjs` 命中 :53 與 :80(非 :79);`sed -n 1,60p
crates/rtl-spec/examples/emit.rs`:無 max 分支 :10 `128usize`、:16 `mx.parse().unwrap_or(128)`。emit.rs 兩處
納入 D-r1a 修法(改引 `rtl_spec::DEFAULT_MAX_RULES`)並以 ⑤(d) 錨定 regex 防回歸;19.16 RV1-15 的錯誤行號
在此勘誤(該節 append-only 不改)。

RV2-11 **[low] 4→5 同步面仍缺 PLAN.md:41 與 dist/server.mjs:99**——已修(19.8「四工具」字樣同步面全表、
19.9 步 b)。【實跑】`rg -n '四工具|4 工具|恰 4|exactly 4'` 普查全表分三類:現役須改=PLAN.md:41、
server.mjs:99、REGISTRY.md:84、mcp-client.mjs:6/:91/:96;歷史記載不改=CHANGELOG.md:78、adr/0012:27/
:107/:138、本檔 §6.1/§11-9;無關命中=gap-fixture-check.mjs:13/:163/:189(G5 條數)。

RV2-12 **[low] RV1-4「golden mem_arst 四端口誤標 parametric」不實**——勘誤(19.0-11 已寫對,本條訂正 19.16
RV1-4 與上一輪 PlanSummary 的同一錯句)。【檔】mem_arst.v:8-10 的 Addr_i/Data_i/Data_o 本為參數寬度,只有
:11 `WR_i` 誤標;【實跑】補丁實驗只變 ports[5](width 0→1、parametric true→false)。

RV2-13 **[low] ADR-0013 D-C9 仍寫「確認者入 REGISTRY,首條預定 D-c1」**——已修(ADR-0013 D-C9 改為「D-c1..D-c4
於 C-S1 先行登記,F3 只補證據」,與 19.2/19.6.2 一致)。

RV2-14 **[low] fixtures/conformance/ 放非 Verilog 檔超出 AGENTS 10.10 字面;skill rtl-conformance 只有名字**——
已修(19.4.4 fixtures 內容類型條、19.8 skill 骨架與 ⑥、19.9 C-S6)。【檔】AGENTS.md:313-314「fixtures/ 只放
本弧自著 Verilog」、SKILL.md:235-237 同款;先例 fixtures/e1/README.md 在庫。C-S6 必顯式放寬條文(10.12+
SKILL+10.10 例外指針),不得默認。skill 定義為只編排 `rtl_tb_plan` 調用與解釋欄位、不判定不計分;零語義的
機械驗法=run-plugin-check ⑥(skills 內 `rtl_` 標識符⊆工具名;【實跑】`rg -o 'rtl_[a-z_]+'
plugin/rtl2spec/skills` 現只含 rtl_analyze/rtl_extract_rules/rtl_scan_gaps,皆在 server.mjs:103/128/162)。

RV2-15 **[自查新增] contract 對負數下標定寬範圍報 `width 0+parametric`**——登記為 D-c4(C-S1),本弧不修
(19.11-5、19.13-9)。【實跑】golden partsel_test006/007 的 `din`/`dout`(`[31:-32]`)為 `input[0]P`/
`output[0]P`;tb_plan 對 parametric 端口 TB1 拒收,屬安全側,不影響六個採樣設計。

- **審方覆蓋面聲明(非缺陷,照錄)**:審方實算通過前 1995 行 sha c9f4b1e5…、ADR-0012 前 179 行 sha
  76325327…、模板塊 cc28f0c7…(34 行)與上輪 numstat;六個 specPath 各 `rg --files` 命中 1 檔、七個 run-*.sh
  與 s6 run.sh 在場;RV1-1 在 /tmp 獨立實作 select_trigger 後 `same 72 diff 2`(只變 mem_arst rules+gaps,
  G3 保留、G4 消失)、五檔 fixture 兩面期望全部相符;D-c2 的 validation-report 只變 sha、LINT-DIFF.tsv 零差、
  RTLIL 兩序 0 規則、mcp-client 10 個 check、corpus 96 檔相符;手寫 T 腿形 TB 跑六設計 iverilog/verilator
  正規化輸出全等、X 行 0(探針,非 C-S3 驗收)。審方未跑:倉庫本體任何閘、yosys equiv、副本 LIVE ys 與
  zipcpu 腿。
- **同輪同步**:ADR-0013(D-C2/D-C3/D-C4/D-C6/D-C9/D-C11/D-C12/D-C13、機械閘 2、非目標、實作記錄 RV2 行);
  ADR-0012 實作記錄追加第三條修訂指針(D1 例外;既有 179 行零改)。§0..§18 零刪改(前 1995 行 sha256 斷言
  c9f4b1e5e1a5…不變);§19.3.4 模板塊內容零改(sha256 cc28f0c7… 不變)。
- **本輪實跑清單(探針寫只落 /tmp/c-arc-r2/)**:oracle 七向量 21 次運行(iverilog -g2005/-g2012+verilator);
  rustc 編譯 corpus_lint race_findings 原文跑六向量;workspace 副本 `cargo build -p rtl-spec --example emit`
  (未補丁/補丁各一次)、native emit 跑釘選 37×2 與 L1 93×2、corpus 96、sv-tests 260 的兩版輸出並逐葉比對;
  副本 `cargo test --workspace --no-fail-fast`(補丁版)、`cargo test -p vl-syntax -p analyzer-core -p rtl-spec`
  (補丁版)、`cargo test -p riv-gen --test roundtrip`(補建 lib 後)、`cargo test -p analyzer-core --test api`
  (改期望後);dist wasm 跑七向量 HEAD 解析與五形 gaps/anchor/clock 探針;rg/grep/sed 普查。
- **本輪未跑**:倉庫本體的 cargo build/test、regress.sh、各 run-*.sh、smoke.mjs(規劃輪不寫 build/ 與
  target/);副本內未跑 regress 與 golden-regression 腳本(以 native emit 等價比對代替,方法校驗見 RV2-2);
  yosys equiv 未跑。

### 19.18 C-S1 實作記錄(2026-11-07;D-r1a/r1b/r1c 收平;弧內禁 commit=工作樹呈現;append-only)

provenance:【實跑】=本實作輪命令實測(倉庫寫面=本片改動檔+plugin/rtl2spec/build/ 對賬工件;探針與紅路副本
只寫 /tmp/c-s1*,用後清除)。對賬工件(gitignored)=build/golden-pre-C-S1/(G0 副本)、build/golden-diff-C-S1.txt
(G2)、build/golden-record-C-S1.txt+build/golden-leafdiff-C-S1.txt(G3)、build/golden-G4-C-S1.txt(G4)、
build/c-s1/{probe-pre.txt,gapfix-pre-wasm.txt,gapfix-post.txt,mcp-e2e.txt,plugin-check.txt,realworld-eval.txt,
rtl-validation.txt,probe-determinism.txt}。

**G0(改碼前)**【實跑】:`git diff --quiet HEAD -- plugin/rtl2spec/golden` exit 0(本片起點 golden==HEAD);
`cp -R` 得 build/golden-pre-C-S1(78 檔,`diff -r` 全同)。D-r1b 驗收(修前):`bun plugin/rtl2spec/scripts/
golden-stats.mjs`(對 golden/ 與 G0 副本各一次)=`STATS rules=843 seq=539 rel=196 guard=85 trans=23
async_true=59 guard_2ev=85 seq_when=476 truncated=0`,與 19.2 勘誤表 HEAD 列逐值相等(async:false=26 另跑)。
修前實探:dist wasm 23af31de… 對五檔 fixture 的規則面/破口面輸出全部與 19.2 規格一致(rst_first_async=
sequence×2 零 guard+G4 certain 假陽性;twin=guard×1 async=true trigger=clk posedge、零破口;compound=零 guard+
G4 certain;loadsig=guard×1 when=load async=false trigger=rst_n negedge、零破口;rst_only_edge=零 guard+G4
certain),原文存 build/c-s1/probe-pre.txt。

**D-r1c(G1)**:crates/rtl-spec/src/sv/mod.rs 新增 `select_trigger(edges, reset_sig)`(緊鄰 reset_cond_signal;
首個訊號 ≠復位候選的 pos/neg 沿,無則回退首沿;比較鍵=render(邊沿表達式),in-git 語料 rg 非標識符邊沿表達式
0 例);extract_edge_process 先由頂層 If 求復位候選再選沿;gaps.rs 復位臂識別改調同一函數(兩面同片同改)。
`cargo build` 0 warning。單測 +2(c_s1_d_r1c_reset_first_sensitivity_is_async_guard[正樣+內聯單沿負樣,兩面各
斷言]/c_s1_d_r1c_sensitivity_order_invariance[check 載荷相等+evidence[1] 各自如實+破口簽名相等]):紅路=
HEAD 的 git worktree 副本植入新測與 fixture 跑 `cargo test -p rtl-spec --test sv_rules c_s1_`=2/2 FAIL
(sv_rules.rs:1435 guards.len、:1491 checks 相等),修後 `cargo test -p rtl-spec`=sv_rules 41 passed(含 2 新測)。
修後 native emit 實探五檔與 19.2「修後」列逐項一致。受害面普查【實跑 rg 釘選 37 檔】:雙沿 always=11 檔 18 行,
復位前置僅 mem_arst.v:20,其餘 10 檔時鐘前置(fsm.v:16 在未定義 `ifdef ASYNC_RESET 分支內)。

**矩陣(G1)**:gap-fixture-check.mjs 破口面 TABLE +5 行(期望=19.2 實探定案)+D-r1c 破口面 selfcheck;S2 規則面
+5 行 guard 條級 multiset(簽名=module|when|async|edge signal|lhs<=value)+async↔twin 順序不變性+規則面
selfcheck;run-gap-fixtures.sh 守衛清單 +5 檔。交付面紅路【實跑】:新腳本對修前 dist wasm 23af31de…=恰 4 FAIL
(async 破口行/破口 selfcheck/async 規則行/順序不變性)33 PASS;修後 `bash plugin/rtl2spec/run-gap-fixtures.sh`
PASS(41 PASS 行 0 FAIL,「檔級矩陣 13 行+S2 規則面」)。「S2 規則面矩陣結果與改前逐條相等」:HEAD 版腳本
(:215=128)以修後 wasm 跑出的 24 條 PASS 行全部逐字出現於新腳本輸出(僅總結行 8→13 行不同)。

**D-r1a**:server.mjs :139 描述改「省略或 0=核心 DEFAULT_MAX_RULES(現值 256)…」、:150 回退與 :219(現 :221)batch
實參改傳 0;gap-fixture-check :215→256;rtl-rulebook SKILL.md:15 文案「省略或 0=核心默認 256」;emit.rs 兩處 128
改引 rtl_spec::DEFAULT_MAX_RULES。普查【實跑 `rg -n '\b(128|256)\b'`】:修後插件面(dist/server.mjs+scripts+skills)
128=0 處;非面 rtl-spec-wasm lib.rs:98/110/111、smoke.mjs:53/:80 按 19.2 不改。run-plugin-check ⑤=新
scripts/ssot-drift-check.mjs((a) 8 處字面量全 ∈{0,256}+非空守衛/(b) 兩文案/(c-1) `const max =` 回退=0/(c-2)
batch 實參=0/(d-1)(d-2) emit.rs)。紅路演練【實跑】在 /tmp 副本上變異(倉庫檔零改動,免「改回—還原」風險;
與 19.2「臨時改回 128 再還原」等價形):原樣 exit 0;r1 回退 0→128=⑤c-1 FAIL;r2 batch 0→128=⑤a+⑤c-2 FAIL;
r3 emit 首臂改 `128usize`=⑤d-2 FAIL;r4 SKILL 256→128=⑤b FAIL;r5 `unwrap_or(128)`=⑤d-2 FAIL;五路皆 exit 1。
mcp-client ⑧=四條 check(⑧a 不傳/⑧b 顯式 0→152 且 truncated=[];⑧c 顯式 100→100 且 truncated=["i2c_master"];
⑧d batch rule_count=152);紅路=HEAD 版 server.mjs 副本跑新 client:⑧a/⑧b/⑧d FAIL(實得 128+["i2c_master"],
即 D-r1a 現形)、⑧c PASS。

**golden G2..G4**【實跑】:G2 `bash plugin/rtl2spec/run-golden-regression.sh` exit 1(預期),`DIFF` 檔名集合恰
{yosys-simple/mem_arst.rules.json, yosys-simple/mem_arst.gaps.json},PROV-DIFF 0,72/74(本跑觸發 pkg 重建,
wasm sha256 23af31de…→f689132bc6a0cf9805aa916f025653802b250cff5c41af5d943000bfe07ce8d1)。G3 `--record` exit 0;
`golden-leafdiff.mjs build/golden-pre-C-S1 golden --allow <兩檔>` exit 0:files_changed=3(+PROVENANCE.md)、
leaves=40,逐葉對賬——mem_arst.rules R001/R002 由 timing/sequence(when !Reset_n_i、latency 1)轉 reset/guard
(trigger Clk_i posedge、async=true、effects Data_o<='bx 與 Mem[i]<='sd0,evidence[0]=復位臂 :23-29、evidence[1]
=敏感列表 :20,指紋隨 category/effects 變;'bx 值如實保留),R003/R004 零變;mem_arst.gaps items[1]
G4-MyMem-001(missing_reset certain)消失、G3-MyMem-001 不變;PROVENANCE=錄製時刻/wasm sha 兩行+sha 表兩行+
歷史區追加一行。與審方 /tmp 獨立復現(same 72 diff 2)一致。交叉核對 `git diff --stat plugin/rtl2spec/golden`
=同 3 檔(本片 G0==HEAD 成立)。重錄後 `STATS rules=843 seq=537 rel=196 guard=87 trans=23 async_true=61
guard_2ev=87 seq_when=474 truncated=0`——位移 seq −2/guard +2/async_true +2/guard_2ev +2/seq_when −2,總數/
rel/trans/truncated 不變,全部歸因 mem_arst R001/R002(19.2 勘誤表不回改)。G4 默認模式 exit 0(74/74 PASS)。

**G5 連帶重生成與其餘閘**【實跑】:`bash plugin/rtl2spec/run-realworld-eval.sh` PASS——docs/RTL2SPEC-EVAL.md 變
6 行:白名單 :3 日期(2026-11-05→2026-11-06)/:5 工具鏈(wasm sha)/:6 耗時(165ms→84ms);白名單外=mem_arst
可歸因 3 行(:83 逐檔表 類別數 1→2、破口 `undriven_read=1 missing_reset=1`→`undriven_read=1`;:338-339 類別
分布表 reset 20 檔 92 條→21 檔 94 條、timing 36 檔 507→505 條);指紋 1212 unique dup=0、trap=0 不變。`bash
plugin/rtl2spec/run-rtl-validation.sh` PASS(13/13)——validation-report.md 只變 :5 工具鏈行(同日重跑,日期行
未變),白名單外零變。`bash plugin/rtl2spec/run-mcp-e2e.sh` PASS(回環 14 PASS 行)。`bash plugin/rtl2spec/
run-plugin-check.sh` PASS(含 ⑤ 8 判據行)。`bun plugin/rtl2spec/scripts/probe-determinism.mjs --runs 12 --list
plugin/rtl2spec/pinned-files.list` 37/37 PASS(19.2 驗收所寫無參形實跑=usage exit 2,釘選清單形為該腳本既有
正典調用,見其頭注)。`cargo build` 0 warning;`cargo test --workspace` exit 0=433 passed/0 failed/1 ignored
(88 suites;431→433=本片 2 新測);測試目標既存 warning 8 條(analyzer-server loopback 6/riv-gen roundtrip 1/
riv-kernel vpi_hooks_gate 1)在本片未觸及的 crate,非本片引入。
`bash scripts/regress.sh` PASS(exit 0,7m17s)——基線逐行零位移:E2E 58/59(30_gen_mux M7 已知硬錯)、
iv 21/22、lint parity 83/87(baseline 83)、wasm 三閘+ffi 四閘+xls adopt 自檢+analyzer smoke PASS、
ryosys ys-e2e 469/479+live 456/456、rtl2spec 四閘 PASS(golden 74/74+STATS 行=重錄後新口徑、gap-fixtures
13 行矩陣、rtl-validation 13/13、mcp-e2e 14 斷言)。regress 的 S7 塊成員=mcp-e2e/rtl-validation/
golden-regression/gap-fixtures 四閘(scripts/regress.sh:368-394),run-plugin-check 非其成員=本片
直接實跑 PASS(其餘總閘成員全數經 regress 覆蓋)。

**定義點同步(19.8;實跑 PASS 行數=14)**:AGENTS.md 10.10(七斷言→14 斷言=實跑 PASS 行數,§19 C-S1 起)、
docs/OKR.md KR1.11(11 斷言→同上)、run-mcp-e2e.sh:7/:57、mcp-client.mjs 頭注(斷言清單補 ⑧a..⑧d);
REGISTRY.md 閘行 mcp-e2e/golden-regression/gap-fixtures 三行同步(STATS 行+G0..G5+D-r1c 五檔矩陣)。
ADR-0012 實作記錄追加 C-S1 指針條目(append-only,既有條文零改)。REGISTRY「復原力弧終審已知未決」
D-r1a/D-r1b/D-r1c 三行追加關閉注記+新小節「C 弧已知未決」首錄 D-c1/D-c2/D-c3/D-c4(10.5 先登記;
D-c2 修復歸 C-S2 首步,登記先於修復)。§0..§18 零刪改;§19.15..19.17 既有條目不改。

**不 bump 版本**(19.2 末:插件面僅缺陷修復,CHANGELOG [0.4.0] Fixed 段 C-S2 一併登錄)。

**本片未跑**:yosys equiv(19.0-6 流程屬 C-S3 變異裁決,本片無 TB);`bash plugin/rtl2spec/run-reimpl-eval.sh`
與 build/s6-metamorphic/run.sh(環境依賴直調、非 ADR 交付閘;本片語義面未觸及其斷言對象——復原對拍床
讀 fixtures/e1 語料與場景 diff/編譯成敗,不讀規則數,reimpl-eval.mjs:37 的 max_rules 已達 256 亦不受
影響;S6 探針比較層讀 golden 快照集合,本片 golden 兩檔變更屬預期語義變更,探針直調屬 C-S2 後複驗項);
mcp-client ping 行屬既有輸出非新判據。

### 19.19 C-S2 實作記錄(2026-11-07;D-c2 修復+核心 tb_plan+面;弧內禁 commit=工作樹呈現;append-only)

provenance:【實跑】=本實作輪命令實測。倉庫寫面=本片改動檔+plugin/rtl2spec/build/ 對賬工件;探針與紅路副本只寫
/tmp/c-s2*,用後清除。對賬工件(gitignored)=build/golden-pre-C-S2/(G0)、build/golden-diff-C-S2.txt(G2)、
build/golden-record-C-S2.txt+build/golden-leafdiff-C-S2.txt(G3)、build/golden-G4-C-S2.txt(G4)、build/c-s2/
(eval/validation 步 b 前副本與各閘輸出)。

**步 a(D-c2)**【實跑】:G0 `cp -R` 得 build/golden-pre-C-S2(78 檔,`diff -r` 全同;時代=PROVENANCE 元資料區
2026-11-06T19:10:09.291Z+wasm f689132b…=C-S1 重錄後,本片 G0≠HEAD,19.2「與 git diff 交叉核對」只適用 C-S1,本片
對照物一律用此副本)。G1:module.rs 組循環頂重新綁定四個滾動變量(`cur_dir` 留在循環外=唯一跨組者;零
unused_assignments)+ast.rs Port doc/sv.rs 投影註釋/api.rs:58/:119-121 訂正+api.rs:128 期望 4→1(`cargo test -p
analyzer-core --test api` 13/13);crates/vl-syntax/tests/frontend.rs 新增七向量單測(斷言到 analyzer-core 投影
width/parametric/signed/dir,vl-syntax 因此新增 analyzer-core dev-dependency,Cargo.lock +1 行;dev 環不入正常構建圖);
**判別力紅路**=HEAD 的 git worktree 副本(只替換 frontend.rs 與 Cargo.toml)跑 `cargo test -p vl-syntax --test frontend
d_c2`→FAILED(frontend.rs:504,v1 `d` 寬 4),修後 PASS。`cargo test --workspace` exit 0=434 passed/0 failed(433+1)。
G2 `bash plugin/rtl2spec/run-golden-regression.sh` exit 1(預期),`DIFF` 檔名集合恰 {yosys-simple/loops,memory,mem_arst,
partsel}.rules.json,PROV-DIFF 0,70/74。G3 `--record` exit 0;`golden-leafdiff.mjs build/golden-pre-C-S2 golden --allow
<四檔>` exit 0:files_changed=5(+PROVENANCE.md)、leaves=37=**規則書 24 葉**(loops 4[ports/6,7,13,14 width 4→1]/memory 2
[modules/10 ports/5,6 width 4→1]/mem_arst 2[ports/5 WR_i width 0→1、parametric true→false]/partsel 16[全 signed
true→false])+PROVENANCE 13 行,與 19.3.2 預測逐檔相等;全部葉在 contract 面,規則與 gaps 零變(`STATS` 行前後逐字相同:
rules=843 seq=537 rel=196 guard=87 trans=23 async_true=61 guard_2ev=87 seq_when=474 truncated=0)。G4 默認模式 exit 0
(74/74)。G5:run-rtl-validation PASS、validation-report 僅 :5 工具鏈行變;run-realworld-eval PASS、EVAL 僅白名單行
(工具鏈/耗時)變,指紋 1212 unique dup=0。wasm 隨 vl-syntax 重建 f689132b…→9e9db744…。

**步 b 核心(crates/rtl-spec/src/tbplan.rs)**:`tb_plan`(Result;內部 DEFAULT_MAX_RULES,無上限旋鈕)/`tb_plan_json`/
`tb_template_v`(不可失敗)/`tb_plan_frame`(wasm 與 emit 同一幀形)/`TbOpts::resolve`(0=默認,SSOT=tbplan.rs 三常量:seed
305419896、cycles 32、flush 8)。clocks 復用 C-S1 `select_trigger`;「產生 ≥1 條規則的行程」判定=規則 evidence[0] 行號落在
該 always 語句行域內;sens_edges=全部沿行程敏感列表有序去重;resets 只由 guard 導出(按 signal/active_low/async 分組);
obligations 的 observe 按主體基名判(`q['sd0]`→q);waivers v1 只產 x-valued(含 transition.to),`unobservable` 無機械判定
來源,留閘側(C-S3)。單測 crates/rtl-spec/tests/tbplan_tests.rs 11 個:fsm_counter busy 寬 1、六設計結構斷言、TB1 負樣
①②③④⑤(含孤兒端口)⑥⑦⑧⑩(300 條規則觸發截斷)⑪、錯誤走資料、0=默認、六設計+fsm_counter 12 跑 byte 全等、模板
文本面五個。**⑨(同訊號兼時鐘與復位)無公共路徑可達**:guard 構造上 trigger≠復位訊號(select_trigger),另一行程以
該訊號為時鐘則先觸發 ⑦——保留為防禦性硬錯,未造假負樣。

**模板細化(19.3.2 未逐字規定、由 Q 判據倒推,實作時以探針驗)**:初版模板對 subbytes 打印 x(308 行)且用
`(rnd >> s) & mask` 隱式截斷、`{a,b} = hi`——前者 Q4 紅、後者 Q7(tb.v 位置 verilator 警告須 0)紅。改為:①`#1` 後
一次性把復位置無效電平、數據輸入置 0(t=0 同刻賦值=IEEE 非確定序,組合行程可能錯過跳變)②tb__rnd 初值與場景同
initial(防 t=0 跨 initial 競爭)③輸入驅動寬度精確(切片/補零拼接/>32 位重複拼接)④`` `timescale 1ns/1ps ``(i2c/
acc_pipe 帶 timescale,TB 不帶=tb.v 位置 TIMESCALEMOD)⑤驅動沿取時鐘有效沿反極性,DUT 兩極性都採樣時同行補 `#1;`
(TB-R1x)⑥TB 內部名一律 `tb__` 前綴(端口撞名=Q1 紅)⑦flush 場景 `COVERS: *`(文法不收空表)⑧純組合 small 走
#1 穩定而非時鐘沿(初版對 signedexpr 編譯敗)⑨看門狗取本次 opts.flush。探針(非閘,/tmp/c-s2-probe/run.sh,口徑仿
Q1..Q5/Q7)七設計(六採樣+fsm_counter):iverilog -g2005 rc=0 warn=0、兩跑相等、DONE 收尾、WATCHDOG 0、值域 x/z 0、
SCENES 相符、文法外行 0;verilator 5.052 `--binary --timing -Wno-fatal`:tb.v 位置警告 0、DUT 位置 signedexpr 2/subbytes
2(§19.4.3 Q7 只記錄不判)、跨模擬器正規化輸出全等。acc_pipe 首跑報 DIFFER=探針缺陷:verilator 對含 `__` 的頂層名
做 mangle(二進制名 Vtb___05F_05F_acc_pipe___05F_05Facc_pipe),探針找錯檔;改用實際檔名後全等——**C-S3 閘必顯式
`--prefix`/`-o` 固定輸出名**。兩個手工變異探針(fsm_counter):async→sync 於 SCENE1 首觀測即異、計數保持於 SCENE2 異
(非 Q8 判別力數字,C-S3 才實測)。

**面**:emit `tbplan` 模式;wasm `rtl_tb_plan_sv`/`rtl_tb_plan_rtlil`(7→9;host 測 2/2);`bun crates/rtl-spec-wasm/smoke.mjs`
與 node 各 18 PASS 行、9 路 parity(新增 fsm_counter 正樣/零輸出 TB1 負樣/sample.il 三路);MCP `rtl_tb_plan`(lang 沿 langOf,
缺省傳 0/空串,核心幀原樣轉出,ok:false→isError);server.d.ts、SERVER_VERSION/plugin.json/CHANGELOG 0.4.0(run-plugin-check
①a 三點+①b 引用常量);EXPECTED_ABI 未動。skill rtl-conformance+validate-manifest 清單。run-plugin-check ③c(9 個
api 引用⊆導出)+⑥(skills 9 處 rtl_ 標識符⊆5 工具名)。紅路【實跑,/tmp 副本,倉庫零改動】:原樣 exit 0;刪導出(glue
改名)→③c FAIL;skill 寫假工具名→⑥ FAIL;刪 frontmatter description→② FAIL;刪 skill 目錄→② FAIL,四路皆 exit 1。
mcp-client ②恰 5 工具+⑨a/⑨b/⑨c(模板落 build/mcp-e2e/ 與 fixture `iverilog -g2005 -t null` exit 0 且 stderr 空)+
⑩(fixtures/tb1_parametric.v)+⑪(sample.il 副檔名分派):實跑 19 PASS 行。定義點同步=AGENTS 10.10、OKR KR1.11、
run-mcp-e2e.sh:7/:57、mcp-client 頭注、REGISTRY 閘行(19 斷言+恰 5 工具)、PLAN.md:41 與 server.mjs 工具面註釋(五工具);
歷史記載(CHANGELOG 0.1.0 條、ADR-0012 原文、§6.1/§11-9)按 RV2-11 不改,REGISTRY Crates 行 rtl-spec/rtl-spec-wasm
同輪更新(AGENTS §10;19.9 列在 C-S6,提前不違規)。

**golden 零 churn(步 b)**:wasm 重建後(9e9db744…→66518257…)`bash plugin/rtl2spec/run-golden-regression.sh` 不重錄即
exit 0(74/74),STATS 不變。證據頁對賬(對照物=步 b 前副本 build/c-s2/{eval,validation}-pre-stepb.md):validation-report 僅
:5;EVAL 僅 :5/:6;白名單外零變。probe-determinism 擴 tb_plan 逐模組幀:`bun plugin/rtl2spec/scripts/probe-determinism.mjs
--runs 12 --list plugin/rtl2spec/pinned-files.list` 37/37 PASS(rules/gaps/tb 三面)。

**閘實跑**:`cargo build` 0 warning;`cargo test --workspace` exit 0=445 passed/0 failed/1 ignored(89 suites;434+11);測試
目標既存 warning 8 條(analyzer-server loopback 6/riv-gen roundtrip 1/riv-kernel vpi_hooks_gate 1)非本片引入。run-mcp-e2e/
run-plugin-check/run-gap-fixtures(41 PASS 行)/run-golden-regression/run-rtl-validation/run-realworld-eval 全 exit 0。環境依賴
直調(非 ADR 交付閘):`bash plugin/rtl2spec/run-reimpl-eval.sh` exit 0(23 場景)、`bash plugin/rtl2spec/build/s6-metamorphic/
run.sh` exit 0(37 檔 IF=0;輸出中 carryadd paren 語法錯行=探針既有「編譯守衛敗,變換面侷限,非 IF」分類)——兌現 §19.18
「探針直調屬 C-S2 後複驗項」。

**與規格的偏差(如實)**:①19.8 所列 CHANGELOG Added 含「conformance 閘」——閘未落地,本片不寫(ADR-0012 R17-1 禁預寫
為現役),C-S3 交付時同片補;②19.2 驗收所寫無參 `probe-determinism.mjs` 實為 usage exit 2(§19.18 已記),本片同用
`--runs 12 --list`;③驗收點名的紅路「暫改再還原」一律改在 /tmp 副本做,等價且倉庫零擾動。

**本片未跑**:yosys equiv(C-S3 變異裁決才用);run-conformance-eval.sh 不存在(C-S3 交付物);Q6 tblint、Q8 判別力、Q9 均未
實測(上述 Q1..Q5/Q7 為探針干跑,不是閘);L1 非釘選 retime 1 葉、corpus/ 96 檔與 sv-tests 260 檔零差=規劃輪材料,未單獨複核。

### 19.20 C-S3 實作記錄(2026-11-07;conformance 確定性床;弧內禁 commit=工作樹呈現;append-only)

provenance:【實跑】=本實作輪命令實測。倉庫寫面=plugin/rtl2spec/(scripts/lib/tbproto.mjs、scripts/conformance/{tblint,mutate}.mjs、
scripts/conformance-eval.mjs、run-conformance-eval.sh、fixtures/conformance/<6 設計>/、fixtures/conformance-lint/、
conformance-report.md、scripts/reimpl-eval.mjs 遷用、CHANGELOG)+docs(REGISTRY/ADR-0013/本章)。探針與紅路副本只寫 /tmp/c-s3*,
用後清除。對賬工件(gitignored)=build/conformance/{verdict.json,aux.md,record-diff-C-S3.md,pre-record/C-S3/}。

**交付件**:①tbproto(iverilog/verilator 分腿丟行正規化、協議解析 SCENE/kv/DONE+x/z 值+文法、帶看門狗執行器 spawnSync
timeout、verilator 顯式 `-o vtb` 固定二進制名——§19.19 mangle 教訓)②tblint(Q6:contract 名實例化+全端口具名連接/零層次
引用/零 $random/看門狗在場/TB-R1x 跨檔極性感知[E 取自 tb_plan.sens_edges,語句窗=corpus_lint 文檔語義]/同檔 R1;Q9:COVERS/
WAIVE ⊆ 規則書、SCENES 一致)③mutate(opcat-v1:規則錨定+P-unruled+OP-NOP)④conformance-eval(Q1..Q9、selfcheck、
`--record --slice`、yosys 僅錄製期存活裁決、證據頁默認模式重生成 byte 比對、輔助腿計分不影響 exit)⑤run-conformance-eval.sh
(守衛 bun/iverilog/vvp、dist wasm 陳舊=exit 2 不重建、DEVELOPER_DIR 三行守衛逐字沿 regress.sh)。

**selfcheck 實跑(每次必跑)**:iverilog 腿 `(1s)`/`(1ps)` 橫幅丟、含 `$finish` 字樣 kv 行留;verilator 腿以 corpus_lint.rs
normalization_contract 原始字串逐位元組得其期望;verilator 四行報告丟、`  v=-3` 兩腿留;X 掃描名/key 含 x 不誤報、值 x 必報;同檔 R1
三向量 1/0/0(=corpus_lint 三向量);TB-R1x 五向量 1/0/1/0/0(第五向量換行形=文檔語義,Rust corpus_lint 報 1=D-c3)+正向量期望
誤配負向量必紅;手術 count==1(行內兩個 `+`→ambiguous)、非阻塞 `<=` 不誤動;比對器一行差必報。

**錨定實測(與 19.4.2 規劃數對照)**:guard 20/20、8/8、3/3;sequence 123/123、5/5、18/18、38/38、3/3;relation 4/4、6/6、10/10——
與規劃一致。transition:fsm 5/5;**i2c 7/9(規劃稱 9/9)**——R025/R026 均為 ST_HOLD→ST_SHIFT(when 不同),evidence[0] 同區間
239-276 內兩行 `state <= ST_SHIFT;`,按主體+to 消歧後仍兩行=ambiguous(依 19.4.2 定義留痕入 record.json,不硬磕);兩條仍經
OP-STATE 行驅動以 span 歸屬覆蓋(OP-STATE@253/262/274 各歸 4 條)。

**算子目錄 v1 的實作定案(19.4.2 未逐字規定處;錄入 record.json catalog 欄)**:①每規則兩算子選用序=規則專屬先於行程共享
(guard RVAL→ASYNC→POL;transition STATE→CONST;sequence HOLD→ARITH→CMP→CONST→SIGN→SLICE;relation ARITH→CMP→CONST→SIGN→SLICE)
——若按表序取前兩個,異步 guard 只得 POL+ASYNC 兩個共享變異、獨佔驗證恆 0;②OP-STATE 行驅動(R1-5 逐行生成,to 唯一匹配
否則 span 歸屬)③transition 錨定以 to 消歧④OP-NOP 取每個在場 form 首條規則的 evidence[0] 首行(「任一證據行」的確定性取法,
每設計 ≤4)⑤P-unruled 只收「含賦值」行(條件行的語義已由規則 when 承載,收入會把有規則描述的條件誤報為規則書缺陷)。

**首錄 `bash plugin/rtl2spec/run-conformance-eval.sh --record --slice C-S3`**【實跑】exit 0,2:10.88(yosys 存活裁決佔主體,
i2c 98.9s);錄前快照為空=全量新增(record-diff-C-S3.md)。逐設計(P-rule 池):

| 設計 | generated | equivalent | eligible | killed | survived | 殺傷率 | 獨佔(共享) | 目錄覆蓋 | 聲明覆蓋 | P-unruled gen/killed |
|---|---|---|---|---|---|---|---|---|---|---|
| acc_pipe | 11 | 1 | 10 | 10 | 0 | 10/10 | 9/9(0) | 9/9 | 0/9 | 1/1 |
| fsm | 44 | 2 | 42 | 31 | 11 | 31/42 | 17/23(0) | 23/23 | 0/23 | 2/0 |
| i2c_master | 272 | 17 | 255 | 77 | 178 | 77/255 | 56/152(10) | 152/152 | 20/152 | 3/2 |
| signedexpr | 12 | 2 | 10 | 10 | 0 | 10/10 | 6/6(0) | 6/6 | 0/6 | 0/0 |
| sincos | 82 | 0 | 82 | 74 | 8 | 74/82 | 41/46(4) | 46/46 | 8/46 | 0/0 |
| subbytes | 18 | 5 | 13 | 8 | 5 | 8/13 | 6/11(1) | 16/16 | 3/16 | 14/10 |

stillborn 0、timeout 0、控制組(OP-NOP+equivalent)全存活;鐵則(P-rule ≥1 killed)六設計全過。原版模擬實測 5-10ms/跑,Q3 牆鐘
看門狗 30s(≫30 倍)。expected.txt 合計 3894 行,**subbytes expected.txt x/z 值 0 處(Q4 ✓)**——訂正上一輪 carryOver「subbytes
輸入已初始化仍有後續 x、Q4 會紅」:該句寫於模板修正前的觀察,§19.19 探針(修正後)已實測 xvals=0,本輪錄製再證;無需拆弧。
equivalent 抽核:signedexpr `{a + b} + 3'd0`→`3'sd0`(拼接=自決定上下文,符號無關)、acc_pipe `{5'h00,…}`→`5'sh00`(同理)
被 yosys 證等價,而 `(a + b) + 3'sd0`→`3'd0` 被殺——與 AGENTS §5.4/§5.5 符號傳播一致。i2c 殺傷率 30%=模板腿(隨機激勵)對協議 FSM 的
量測值,v1 不設數值下限(19.4.5),D 腿(C-S4)為增益腿。
**P-unruled 被殺 13 行**(規則書未描述而 TB 可觀測,record.json rulebookDefects 欄;F3 消費歸 C-S5):i2c_master.v:68 `cnt_end`
兩變異(=D-c1 的確定性證據,C-S1 已登記)、acc_pipe.v:43 直通別名 `assign out = out__output_flop;`(S2「直通別名不產」政策面,
19.13-4 待決)、subbytes 10 個(阻塞賦值的組合次態/輸出直賦行,def_lint 歸類歸 C-S5)。

**默認模式 `bash plugin/rtl2spec/run-conformance-eval.sh`**【實跑】exit 0,28.8s:6/6 ADMITTED、全部變異結果==record.json、證據頁
byte 相等;SUMMARY 同首錄。

**紅路三路(/tmp 副本=plugin 全樹 rsync+語料符號鏈接,倉庫零改動;與規格「改後還原」等價)**【實跑】:原樣 exit 0;①signedexpr
expected.txt 改一字節→exit 1(`Q2: 原版輸出≠expected.txt`+證據頁陳舊);②fsm record.json 把 killed 的 m0001 OP-STATE@33 改
survived→exit 1(`Q8: DRIFT 1 處`);③acc_pipe tb.v 刪 `$display("SCENE2 full");`→exit 1(T 腿重生成≠tb.v、Q2、`Q5: scenes=3/4`、Q8)。

**reimpl-eval 遷用 tbproto(parity)**:trySh 改走帶看門狗的 tbproto.run(成功不收 stderr、失敗收 stderr 全文=逐欄同義 execFileSync)、
scenes()→tbproto.sceneBlocks、scenesParsed 只落 sidecar build/reimpl-eval/scene-protocol.json+console 一行(verdict.json 結構不動)。
首次比對 FAIL 一處(i2c-A0 的 warn 欄:遷用版把失敗路徑 stderr 截到 200 字,原版只截 message 回退)→修正為逐欄同義後 PASS。
**同時刻 parity**:`git show HEAD:…/reimpl-eval.mjs` 暫存為 scripts 下臨時檔跑一次(跑完即刪)取前快照,再跑遷用版:verdict.json
legs(12)/segments(4) 深等(除 generatedAt)、runs/out_* 14 檔 byte 相等;`bash plugin/rtl2spec/run-reimpl-eval.sh` exit 0(本機
build/reimpl-eval/reimpl/ 8 腿在場)。

**輔助腿(重寫產物在 conformance TB 下計分,不影響 exit)**:acc-A1/A2/A3/B1 4/4、acc-A0 1/4;i2c-A1/A3/B1 4/4、**i2c-A2 3/4(SCENE3
small 失配)**——該腿在 e1 TB 下 11/11;S-SMALL 把多位輸入遮罩到 ≤3 位(含 prescale),觸到 e1 TB 固定 prescale=4 看不到的取值域。
這是「兩套 TB 強度差」的直接讀數(19.5),**不構成對 A2 意圖正確性的判定**;i2c-A0/A0f 無適配 wrapper 時接口不合=compiled=false
(S1 契約章修復的對象,預期)。

**定義點**:CHANGELOG [0.4.0] Added 補 conformance 確定性床條(C-S2 未預寫,R17-1);REGISTRY 新增 run-conformance-eval 閘行+
run-reimpl-eval 遷用注記;ADR-0013 實作記錄追加 C-S3 條(只記交付事實;「規劃閘→現役」條文改寫與 AGENTS/SKILL/OKR 四層同落歸
C-S6)。AGENTS 10.10「fixtures/ 只放自著 Verilog」與 fixtures/conformance/ 非 Verilog 檔(case.json/expected.txt/record.json)的
字面張力按 19.4.4 由 C-S6 顯式放寬(先例 fixtures/e1/README.md),本片未改 AGENTS。

**本片未跑**:C-S4 D 腿、C-S5 診斷/def_lint/留一法(verdict.json diagSelfcheck=null、baselineTemplate=null);yosys 只在錄製期跑
(默認模式不需要);未對 equivalent 27 條逐條人工複核(抽核 5 條見上)。

### 19.21 C-S4 實作記錄(2026-11-07;D 腿定向 TB 入庫;弧內禁 commit=工作樹呈現;append-only)

provenance:【實跑】=本實作輪命令實測。倉庫寫面=plugin/rtl2spec/(scripts/conformance-eval.mjs directed 腿支持、
scripts/conformance/dleg-prompt.mjs、CONFORMANCE-PROTOCOL.md、fixtures/conformance/ 四設計 {case.json,tb.v,
expected.txt,record.json}、conformance-report.md)+docs(REGISTRY/ADR-0013/本章)。LLM 撰寫者=本實施 agent(GLM-5.3-Flash;
污染聲明=CONFORMANCE-PROTOCOL §0:盲隔離弱,session 曾讀過原版源碼與 C-S3 record;無子代理環境,如實記錄)。

**協議釘檔**:CONFORMANCE-PROTOCOL.md C-S4 交付(§0 模型/採樣/日期/污染聲明;§1-3 模板塊逐字轉入——開工重算內容錨
`awk+sed+shasum`=sha256 cc28f0c7…(34 行)=預登記值,相等;預登記未提交僅內容錨,如實記載;§4 腿表;§5 機械執行)。
prompt 實例化=scripts/conformance/dleg-prompt.mjs(確定性;RULEBOOK_MD=golden rules.json 經 dist/md.mjs 渲染、
TB_PLAN_JSON/template_v=dist wasm rtl_tb_plan_sv 同幀;改進輪存活清單只列規則書內容)。四設計 r0 prompt 行數:
i2c 3728/acc 348/fsm 650/sincos 1115(sha256 打印在案)。

**閘 directed 腿支持(§19.3.1/19.4.5)**:evalDesign 增加 directed 路徑——主腿=凍結 tb.v(D 腿閘永不重生成);
T 腿模板(進程內重生成)作**基線腿**跑同一變異矩陣(變異體檔寫一次兩腿共用;equivalent 裁決是「變異體 vs 原版」
屬性,錄製模式只對「所有已跑腿皆存活」的 P-rule 變異跑 yosys);Q9 directed 口徑(每規則必 COVERS 或 WAIVE;
場景宣告新增 `// SCENE LIST:` 顯式枚舉形,與 `// SCENES: N` 二擇一);tbproto kv 值字符集補 `%0d` 負號
(signed 觀測值,例 sincos `sin=-1`)。入庫門:形族底線(每 form 族有 eligible 歸屬變異者 ≥1 被殺)+不劣於基線
(同變異集 D 殺傷率 ≥ T 殺傷率,交叉相除防除零);record 新增 baselineTemplate 欄(基線 sha/原版輸出 sha/數字/
逐變異/gain/formFloor/notWorse);record-diff 增「T 腿轉基線腿」對賬(舊主腿逐變異==新基線)+D 腿新殺/丟殺清單。
`--try <design> <tb>`=候選評測(寫只落 build/conformance/try/,fixtures 零觸碰;Q2 候選口徑=兩跑確定性,期望值由
--record 從原版重錄);`--record --promote <design>=<路徑>`=入庫(僅 directed case 接受;鐵則+入庫門不過=拒錄;
未 promote 的 template 設計照 C-S3 路徑)。證據頁新增 D/T 分列表(增益=D−T killed,禁合併)。

**F1 無條件聲明**:F1(重寫迭代)屬 C-S5(§19.6.3 採樣演示),本片未跑。

**樣本與 F2 實跑**(`bash plugin/rtl2spec/run-conformance-eval.sh --try …`,四候選全 ADMITTED):
- i2c-r0-k1:201/255(T 77/255)gain **+124**;floor guard 7/21 trans 9/9 seq 185/225;notWorse ✓;F2 一輪停
  (定向命令序列場景已覆蓋可觀測差異面)。
- acc-r0-k1:10/10(T 10/10)gain 0——T 腿已滿,無增益空間;notWorse ✓。
- fsm-r0-k1:42/42(T 31/42)gain **+11**;floor trans 10/10 seq 32/32;notWorse ✓;F2 一輪停。
- sincos:r0 75/82(gain+1,OP-ASYNC m0002 丟殺回復前)→ **r1** +async_reset_observe 景(S-ARST 式兩沿之間觀測,
  殺 m0002)+2=76/82(guard floor 5/9)→ **r2** WAIVE d.R005..R008(x/y/z/i 死存儲:start 分支覆寫,port 不可觀測)
  +d.R035(末迭代 dz 只影響內部 z)=無新殺=F2 停止。腿表 r0/r1/r2 三行如實 append。

**首錄 `bash plugin/rtl2spec/run-conformance-eval.sh --record --slice C-S4 --promote …×4`**【實跑】exit 0(1:31):
6/6 ADMITTED,SUMMARY mutants=439 eligible=412 **killed=347**(C-S3=210,**Δ+137**)equivalent=27 unruled_killed=15。
record-diff-C-S4.md 逐案對賬:T 腿轉基線腿=舊主腿 sha 與新基線 sha 同(1220205fd406/82f49e24122b/113fec7c29a4/
46552e62cde1)且**逐變異結果相同 14/14、48/48、278/278、84/84**;D 腿新殺 i2c 132/fsm 13/sincos 2/acc 0、丟殺 0;
signedexpr/subbytes 不變(tb.v 不變、expected 0 行差)。默認模式 `bash plugin/rtl2spec/run-conformance-eval.sh` exit 0
(70s):6/6 ADMITTED、兩腿變異結果==record、證據頁 byte 相等(D/T 分列表已入頁)。golden 零重錄(wasm 零重建;
`bash plugin/rtl2spec/run-golden-regression.sh` exit 0 74/74)。

**誠實條款**:①污染聲明=撰寫者曾讀原版源碼與 C-S3 record(§0);D 腿數字只作凍結工件機械屬性證據,不解讀為
「LLM 只憑規則書寫 TB 的能力」;②K=2,3 未跑;③F1 未跑(C-S5);④acc_pipe gain=0 如實報(T 已滿);
⑤i2c guard floor 7/21:OP-POL/OP-ASYNC 歸屬 20 條 guard 共享單點變異,該族多數規則的「獨佔驗證」本就不存在
(殺傷率≠規則驗證覆蓋,獨佔 124/152 為主數);⑥sincos guard floor 5/9 的 9=8 guard+R035(dz 死存儲,已 WAIVE)。

**本片未跑**:F1/C-S5 診斷層;K=2,3;verilator 候選腿 Q7 在 --try 已跑(agree),錄製模式照閘正常執行。

### 19.22 C-S5 實作記錄(2026-11-07;診斷回灌;弧內禁 commit=工作樹呈現;append-only)

provenance:【實跑】=本實作輪命令實測。倉庫寫面=plugin/rtl2spec/(scripts/conformance/{diagnose,def_lint,dleg-prompt}.mjs
[dleg-prompt 為 C-S4 產]、scripts/conformance-eval.mjs 診斷層集成、CONFORMANCE-PROTOCOL.md 腿表/備註、fixtures/conformance/
四設計 record.json 與 case.json aux)+docs(REGISTRY D-c1 補強、ADR-0013、本章)。LLM 撰寫者=本實施 agent(污染聲明沿 §0;
D 腿/重寫腿數字=輔助證據,禁混宣稱)。對賬工件(gitignored)=build/conformance/{rulebook-defects.{json,md},diag/acc-A0.md,
record-diff-C-S5.md,pre-record/C-S5/}。

**確定層(閘側工具;RV1-14 定位=只被 conformance-eval 調用,不經任何面露出)**:①diagnose.mjs=診斷 JSON v1(failing
firstDiff/keysDiffering/contextBefore≤3 行+suspects 三級排序[direct=主體∈差異輸出端口→covers=失配場景 COVERS→
mutant-sensitive=失配場景殺傷計數,留一法排除自身;同級規則 ID 序]+undefinedSignals[xz 值 key]+assumptions+
classification[tb-protocol/rulebook-gap-candidate/rewrite-error])+leaveOneOut(killed 變異=真值已知缺陷實現;hit=真值
規則集∩top3)。②def_lint.mjs=定義完備性分類(未入規則主體的信號×定義行形態:U1 net 宣告賦值=缺陷/U2 直通別名=
政策排除/U3 分支域外常量=政策排除/U4 非 canon/U5 其他列明不記)。③閘集成:主腿 killed 變異的實際輸出緩存(留一法
輸入);record 新增 rulebookDefects.defLint+leaveOneOut 欄(Verdict.diagSelfcheck 同步);默認模式 DRIFT 比對擴此二欄;
rulebook-defects.{json,md} 全集跑生成(P-unruled killed+def_lint 缺陷+政策排除列明)+**F3 閘斷言=i2c_master 缺陷必含
cnt_end**(PASS,OP-HOLD/OP-ARITH@:68)。

**`--record --slice C-S5`**【實跑】exit 0(1:18)6/6 ADMITTED;record-diff-C-S5.md 對賬=tb.v/expected/變異結果**全部
不變**(C-S4→C-S5 無語義面變更),新增欄逐設計如實入 diff(defLint:null→…;留一法:null→rate)。留一法 rate 實測
(v1 無下限,數字照報):acc 3/10、fsm 6/42、i2c 8/201、signedexpr 7/10、sincos 9/76、subbytes 3/8——低命中率的機械
成因=direct/covers 次級排序優先與共享歸屬(OP-POL/OP-ASYNC 多規則歸屬使 top3 被真值規則擠出),不是判據錯;record.json
per[] 全量在案可復算。def_lint 分類實測:i2c cnt_end=U1(=D-c1);acc out=U2 政策排除;subbytes 7 項=U5(內部存儲列明
不記缺陷);fsm/sincos/signedexpr 0 項。rulebook-defects 合計 17 項(P-unruled killed 15+U1 1+U2 政策排除 1)。

**F1 採樣演示(輔助證據,永不進閘;按輪分列)**【實跑】:fsm 重寫 r0(兩段式結構,源碼結構差異=§1 模板要求3)在
D 腿 TB 下 SCENE5 兩行失配(同步復位拍輸出慢一拍)→r1 `// FIX(r1):` 復位拍輸出直接歸位→**6/6 場景全配**;
sincos 重寫 r0(控制/數據路徑兩塊+dz function)done 載入差 1(z0=0 sin=1 vs 2)→**逐周期探針歸因**(中間迭代全同,
原版單塊阻塞序使輸出載入含末次微旋)→r1 控制塊折疊末次微旋→**6/6 全配**;i2c 用在場 i2c-A1..A3/B1=全部 6/6
(訂正:C-S4 carryOver「i2c-A2 3/4 SCENE3 失配」係 T 腿 TB 的 small 景,D 腿 TB 無此景,A2 在 D 腿 TB 下全配);
acc-A0(pre-S2 產物)0/4=診斷示例輸入。診斷示例=build/conformance/diag/acc-A0.md:classification=rewrite-error、
suspects top3=R001/R002/R003(輸入暫存 sequence=A0 缺失的族)、assumptions=[]。case.json aux 新增
fsm-rew-r1/sincos-rew-r1 兩行(斷言規格手寫合法);輔助腿計分表(閘 aux.md)隨默認模式機械生成。

**默認模式**【實跑】exit 0(45.7s):6/6 ADMITTED、record 比對(含 defLint/LOO 新欄)零漂移、F3 斷言 PASS、
rulebook-defects 17 項重生成、證據頁 byte 相等。

**與規格偏差/待決(如實)**:①留一法 top-3 命中率低(8/201 量級)——根因初判=共享歸屬變異的真值集(≤20 規則)與
direct/covers 排序的擠壓;真值集交集合取(good look)定義已按 §19.6.4 從寬,任何收緊/下限=後續 ADR-0013 修訂;②
suspects top-20 截斷(record 只存前 20;per[] 全量在 LOO 面);③rulebook-gap-candidate 分類三條件中「≥2 獨立樣本
同場景同 firstDiff」在留一法場景恒不可達(樣本數=1),僅 F1 多樣本場景生效。

**本片未跑**:F2 進一步輪(C-S4 已停);D-c1 修復本身(登記待用戶裁決 §19.13-6);verilator 候選腿(閘正常路徑已跑)。

### 19.23 C-S6 實作記錄(2026-11-07;收尾立法+總閘;append-only)

**立法四層落地(C-S6 清單;寫面聲明=本閘全部寫面在 plugin/rtl2spec/ 內)**【實跑】:
- AGENTS 新節 **10.12 C 閉環紀律**(寫面聲明/fixtures conformance 子樹內容類型放寬[TB 判別力鐵則同節]/
  LLM 腿 10.11 擴/oracle 紀律/G0..G5 通用/閘落點與 D-c 張力指針);10.10 fixtures 句加
  「(conformance 子樹例外見 10.12)」(RV2-14 顯式放寬,不得默認)。
- SKILL「**C 閉環增補**」節(核心-first/tb_plan 三面一路/判別力雙 fixture/D 腿紀律/oracle 紀律/
  fixtures 授權律放寬同款/def_lint 不露面定位);SKILL fixtures 自著授權律加例外指針。
- REGISTRY:conformance 閘行(C-S3/C-S4/C-S5 摘要與 LOO rate)+Crates 行 rtl-spec 59 pass 實測刷新/
  rtl-spec-wasm 9 導出/smoke 18 斷言 9 路 parity(C-S2 輪已做)/vl-syntax 行(ANSI 修復+七向量測數);
  D-r1a/r1b/r1c 關閉注記+「C 弧已知未決」D-c1..D-c4(=C-S1 登記)+D-c2 關閉(C-S2)+D-c1 證據補強(C-S5)。
- PLAN.md 里程碑表新行 **C-S1..C-S6 / TB1 族**(TB1=tb_plan 十一條硬錯里程碑族,M7 形先例);OKR 新
  **KR1.12**(證據指針=`bash plugin/rtl2spec/run-conformance-eval.sh`+conformance-report.md;RV1-12)。
- ADR-0013:**「機械閘 1」規劃閘→現役改寫生效**(C-S3 已交付後改寫;「禁預寫為現役」紀律=改寫發生在
  交付之後,R17-1 同款)+狀態行訂正;ADR-0012 修訂指針三條已在 C-S1/C-S2 輪 append(append-only)。
- 本節+§19.18..19.22 實作記錄=append-only;§0..§18 零刪改。

**總閘 11 命令全跑**【實跑,本輪 exit 0 全數】:①`cargo test --workspace`(445 pass/0 fail/1 ignored)
②`bash scripts/regress.sh` ③`bash plugin/rtl2spec/run-mcp-e2e.sh`(19 斷言)④`bash plugin/rtl2spec/
run-rtl-validation.sh`(13/13)⑤`bash plugin/rtl2spec/run-golden-regression.sh`(74/74)⑥`bash
plugin/rtl2spec/run-gap-fixtures.sh`(13 行矩陣+規則面)⑦`bash plugin/rtl2spec/run-realworld-eval.sh`
(353 檔)⑧`bash plugin/rtl2spec/run-plugin-check.sh`(①..⑥ 含⑤SSOT)⑨**`bash plugin/rtl2spec/
run-conformance-eval.sh`**(現役;SUMMARY designs=6/6 admitted=6 mutants=439 eligible=412 killed=347
equivalent=27 unruled_killed=15;selfcheck+LOO+F3 斷言)⑩⑪環境依賴直調=`bash plugin/rtl2spec/
run-reimpl-eval.sh`(exit 0;23 場景)+`bash plugin/rtl2spec/build/s6-metamorphic/run.sh`(exit 0;
IF=0)——後兩者非 ADR 交付閘(§19.13-2 張力照錄)。逐輪實跑記錄見各片實作記錄;regress 一律最後跑
(基線 469/479+456/456+lint parity 83+58/59+21/22 逐字不動)。本輪 C-S6 收口重跑 regress 用時 7:22.93,
conformance 默認模式 39.9s。

**弧末事實**:工作樹 67 路(C-S1..C-S6 累計;golden 累計 6 檔=mem_arst 2+loops/memory/partsel 4;
wasm sha 66518257…);插件 semver 0.4.0 未發布(弧內禁 commit,版本由用戶提交時生效);E1 記錄文檔
build/mvp-exp/ 與 build/reimpl-eval/reimpl/ 8 腿晉升落點=用戶裁決事項(§19.13-1/-2 延續)。

### 19.24 終審 R5 修訂(2026-11-08;四條意見逐條消化;append-only,§19.23 前零改)

R5-1/R5-3 **[high/medium] fixtures/conformance/ 滯留 4 個未登記 LLM 盲寫 TB 殘留目錄(描述串含 `/` 被建為
嵌套路徑;無 case.json 不入閘枚舉;mtime 09:07-09:28=C-S6 窗口;與已登記腿產物均不匹配)**——已清理:
`find … -name '*(*' -exec rm -rf` 刪 4 目錄(證據固定:刪前 `find -type f` 清單+tb.v 行數 222/377/707/270+頭部
自述「盲写 testbench」實跑存檔;歸檔 /tmp/r5-evidence/*.tgz);修後 `find plugin/rtl2spec/fixtures/conformance
-maxdepth 1 -type d`=恰 6 設計目錄零殘留【實跑】。閘不受影響(評審實跑 PASS 6/6+本輪默認模式重跑 PASS 6/6
killed=347)。根因登記=REGISTRY **Incident register 新行**(11-07;五欄:症狀/根因[寫面衛生:某輪腳本以描述串
當路徑建目錄且未經 try/record 對賬流程;寫面聲明審計缺 fixtures 樹逐輪 find 差分]/修法[刪除+證據固定]/硬化層
[AGENTS 10.12 寫面聲明+本條補記])。寫面聲明(§19.23)自本條起含「fixtures/conformance/ 樹=C-S3/C-S4/C-S5
已登記工件+本輪收窄修訂,零未登記產物」。

R5-2 **[low] AGENTS §10.9 「現行閘=450/460」係 M3 時點數未隨 469/479 同步**——已勘誤:兩處現行值句改
469/479+勘誤注(「原記 450/460 係 M3 時點數」;歷史敘述句中的 450/460 保留於勘誤注內),现行以 REGISTRY
Crates 表 ryosys 行為準【實跑 regress 469/479 逐字不動】。

R5-4 **[medium] D 腿 COVERS 宣告過寬+sincos COVERS∩WAIVE 矛盾(fsm SCENE1 打包 23 條/sincos SCENE1 打包
46 條又 WAIVE 同批 5 條;declaredCoverage 失分辨力)**——已修:**宣告收窄為觀測映射形**。機械定義=場景 s
觀測規則 r ⇔ 歸屬 r 的 killed 變異(非 equivalent)以 s ∈ killScenes;映射由入庫 record.json 殺傷矩陣復算
(探針 /tmp/r5-covers.mjs,輸出存檔)。四 D 腿 tb.v 的 COVERS/WAIVE 註釋塊整塊換為映射形(註釋-only:
iverilog 重編零警告【實跑 sincos】);sincos 矛盾消除=R005..R008 經 SCENE6 觀測改 COVERS(初版 WAIVE 恰與
C-S5 record 的殺傷事實矛盾),R035 維持 WAIVE(declaredCoverage sincos 45/46);i2c 新增 WAIVE 11 條
(全部變異兩腿存活且 yosys 未證等價)[收尾弧 S0 勘誤,2026-11-07:原記「僅 equivalent 存活的規則=不可觀測」與 record
不符;實證=11 條規則歸屬 21 變異=20 unadjudicated+1 equivalent(m0054/R042;其同規則 m0055 未證,故 R042 亦非全
equivalent;R104 僅 m0178),證據指針=fixtures/conformance/i2c_master/record.json 主腿與 baselineTemplate 兩腿按這
11 條 ruleIds 過濾、RTL2SPEC-FINISH-PLAN §1。tb.v 標籤本片不動(11 條仍標 unobservable),逐規則重裁=等價裁決數據
落地後,方向不預設;中性標籤 `unadjudicated` 見 CONFORMANCE-PROTOCOL §6 模板 v2];acc_pipe 4 景全激勵 9 條=映射與原宣告巧合一致(如實)。**重錄**=
`bash plugin/rtl2spec/run-conformance-eval.sh --record --slice C-S6`【實跑】exit 0(1:34)6/6 ADMITTED,
SUMMARY killed=347 與 C-S5 逐值同;record-diff-C-S6.md=**expected.txt 0 行差+變異結果全同六設計**(註釋-only
的機械證明)+fsm 留一法 6/42→7/42(covers 次級排序輸入隨宣告收窄的誠實變化,唯一語義面效應)+tb.sha
C-S4→C-S6 四設計全變(1220205fd406→e972677c3368/82f49e24122b→6792d5a97ee4/113fec7c29a4→d25bc41d2f56/
46552e62cde1→61303cf9333f;錄前快照 C-S4 時代 record 在案)。`--try` 四設計先驗 PASS 後重錄;i2c WAIVE 一行
多規則不合 tblint 文法(一行一規則)首跑紅→拆行後 PASS。默認模式 exit 0(6/6,declaredCoverage 實測
acc 9/9、fsm 23/23、i2c 141/152+WAIVE 11、sincos 45/46+WAIVE 1、signedexpr/subbytes=T 腿 0/6 與 3/16 不變)。

R5-5 **[low] 腿表 fsm-rew/sincos-rew 敘述小出入**——已勘誤(CONFORMANCE-PROTOCOL 腿表兩行行內訂正+勘誤注):
fsm r0「2 行失配」實為 **1 行 2 鍵**(SCENE5 打印 4 的 rb/gb);sincos r0「done 載入差 1」係 firstDiff,
總體 **249 行系統性失配**(同根因=末次微旋未折疊)。r0→r1 迭代敘事與 6/6 數字不變(評審復算證實)。

**本輪未跑**:新增語義面(零——僅 fixtures 註釋/文檔/刪除未登記產物);golden 零重錄。

## 20. 未覆蓋收尾弧實施記錄(2026-11-07;S0..S4;方案=docs/RTL2SPEC-FINISH-PLAN.md;append-only,§0..§19 零刪改)

本章一次性記錄收尾弧(用戶指令「finish 未覆蓋」)的實施結果。§0..§19 原文不動;與既有章節不符處以 §20.7 勘誤為準。
數字一律取自本弧實跑輸出,出處=各片 build/finish-s*/ 日誌與入庫工件(record.json/conformance-report.md/PROVENANCE.md)。
度量口徑不變:conformance 量的是行為復原力,不是意圖正確性(§19.1);LLM 腿(TB 生成/視覺診斷)不進任何確定性閘(AGENTS 10.11)。

### 20.1 範圍、切片與寫面

- 切片:S0 終審 2 medium 收平 → S1 等價裁決腿 → S2 波形幀診斷腿 → S3 T 腿全量擴面 → S4 收尾。弧內零 commit/push。
- 新增寫面全部在 plugin/rtl2spec/ 內(build/ 為 gitignored 工件;入庫者=fixtures/conformance/、golden/、腳本、協議與證據頁),
  另改 docs/、AGENTS.md、.agents/skills/eda-dev/SKILL.md、PLAN.md 的條文。Rust 零改動,dist wasm sha256 全弧保持 66518257…。
- 條文落點:CONFORMANCE-PROTOCOL §5(入庫 tb.v 禁手改)/§6(模板 v2)/§7(等價裁決腿)/§8(幀腿)/§9(擴面名單);
  FINISH-PLAN §11(欠條與交接);REGISTRY(閘行、D-c5/D-c6、Incident 兩行);ADR-0013 實作記錄 S0..S4;ADR-0012 指針兩行。

### 20.2 S0:終審 2 medium 收平

- **medium-1(WAIVE「不可觀測」無證據)**:修前實證 i2c 11 條 WAIVE 標 unobservable,其 21 個變異=20 unadjudicated+1 equivalent
  (m0054/R042)。§19.24 行內勘誤(該處為本弧授權例外);CONFORMANCE-PROTOCOL §6 模板 v1→v2 協議事件,標籤集補
  `unadjudicated`,v2 sha256=104d13f1b07c5e465a63fe222c4617170547b83c46ed5c2dcb38707f7d64b02f(34 行),v1 cc28f0c7… 不變;
  tblint `WAIVE_LABELS` 加文法外硬紅;dleg-prompt 經 proto-template.mjs 取最高版本,任一錨不符 exit 2。
- **medium-2(入庫 tb.v 手改假綠)**:修前實證——對 fsm/tb.v 追加一行註釋(磁碟 sha 1871b042…≠record 6792d5a9…),
  `run-conformance-eval.sh fsm` exit 0。修法=TB-SHA-ALARM 三方核對(record.tb.sha256/磁碟 tb.v/本輪主腿 TB),
  默認 REJECTED、錄製拒錄、寫後核對,皆 exit 1;`--promote` 不洗白手改。
- 驗收:build/finish-s0/acceptance.sh 14 行全過(手改紅/還原綠/正常 promote 不誤報/--try 不適用);selfcheck 18→19。

### 20.3 S1:等價裁決腿(測試側 harness;條文=CONFORMANCE-PROTOCOL §7)

- **強形命令**(逐字在協議 §7,selfcheck 硬同步):`proc; flatten; memory; async2sync; opt_clean; equiv_make; …; select -count
  t:$equiv; equiv_simple -seq 5; equiv_induct -seq 5; equiv_status`,無 blacklist、無 stash,變異取 top-only。
- **三值與守衛**:eqverdict=equivalent|unproven|adjudication-failed;equivalent 須 proven 且 cells≥輸出端口位數合計。
  **守衛口徑訂正鏈**:R2 用全端口位數 → R3 改輸出端口位數(signedexpr gold-vs-gold cells=18、全端口 23、輸出 18:全端口口徑會
  誤判為空真)→ S1 實測六設計端口 all/out=acc_pipe 33/16、fsm 8/4、i2c_master 49/16、signedexpr 23/18、sincos 64/41、
  subbytes 86/42,signedexpr 18/18 臨界過。已知邊界:此守衛是必要非充分條件(只對輸出 blacklist 的探針形,fsm 36 cells 全
  proven 也過得了 cells≥4);生產形無 blacklist 故不觸發。
- **偵測力斷言六條**(`--record` 專屬,任一不過=拒錄):gold-vs-gold 帶守衛;正樣 fsm m0045 OP-NOP→equivalent(cells 40);
  反樣 i2c m0001→unproven;空真探針(全名單 blacklist)→adjudication-failed(empty);killed 全母體 equivalent==0;存量
  equivalent 重驗零翻變。判別力:注入三種假 oracle(proven+0 cells/無條件 yes-man/恒 unproven)全部拒錄,yes-man 被空真
  探針與 killed 全母體雙抓(build/finish-s1/acceptance.sh 19 行)。
- **默認模式零 yosys 依賴**:`RTL2SPEC_YOSYS=none` 默認 exit 0、`--record` exit 2。默認模式 eq-verify=schema+counts+waiveTable。
- **SKIP 過渡窗**:C-S7 重錄前,默認模式對 6 份舊 record 各打一行 `SKIP … eq-verify` 且 exit 0;重錄後刪除該分支,eqMeta 缺=紅。
- **預期觸發清單(實測兌現)**:首次全集 `--record --slice C-S7` 被 WAIVE↔record 交叉驗證拒錄,報出 12 條,恰為方案 §3.5 預期的
  i2c 11 條+sincos d.R035。按數據逐條改標後 `--promote` 換檔(只動 WAIVE 行)全集重錄,expected.txt 0 行差。
- **WAIVE 逐規則裁定表**(數據=入庫 record.waiveTable;方向未預設,無一條升為 unobservable):

| 設計 | 規則 | 裁定前標籤 | 裁定後標籤 | 變異 | killed | equivalent | unproven | 數據謂詞 |
|---|---|---|---|---|---|---|---|---|
| i2c_master | i2c_master.R042 | unobservable | unadjudicated | 2 | 0 | 1 | 1 | unadjudicated |
| i2c_master | i2c_master.R043 | unobservable | unadjudicated | 2 | 0 | 0 | 2 | unadjudicated |
| i2c_master | i2c_master.R044 | unobservable | unadjudicated | 2 | 0 | 0 | 2 | unadjudicated |
| i2c_master | i2c_master.R045 | unobservable | unadjudicated | 2 | 0 | 0 | 2 | unadjudicated |
| i2c_master | i2c_master.R066 | unobservable | unadjudicated | 2 | 0 | 0 | 2 | unadjudicated |
| i2c_master | i2c_master.R087 | unobservable | unadjudicated | 2 | 0 | 0 | 2 | unadjudicated |
| i2c_master | i2c_master.R100 | unobservable | unadjudicated | 2 | 0 | 0 | 2 | unadjudicated |
| i2c_master | i2c_master.R101 | unobservable | unadjudicated | 2 | 0 | 0 | 2 | unadjudicated |
| i2c_master | i2c_master.R104 | unobservable | unadjudicated | 1 | 0 | 0 | 1 | unadjudicated |
| i2c_master | i2c_master.R122 | unobservable | unadjudicated | 2 | 0 | 0 | 2 | unadjudicated |
| i2c_master | i2c_master.R149 | unobservable | unadjudicated | 2 | 0 | 0 | 2 | unadjudicated |
| sincos | d.R035 | unobservable | unadjudicated | 2 | 0 | 0 | 2 | unadjudicated |

- **數字**:SUMMARY 逐值不變(6 設計 mutants=439 eligible=412 killed=347 equivalent=27);SUMMARY-EQ adjudicated=84=
  equivalent 27+unproven 57+adjudication-failed 0;killed 全母體 355(主腿 347)=unproven 354+failed 1+equivalent 0。
- **能力邊界**:唯一的裁決失敗 i2c m0011(OP-POL@71)是 proc 前端報 `Async reset \rst_n yields non-constant value 1'm for signal
  \ack_rx.`,不是 `$pow`;`$pow` 的 `No SAT model available` 另以最小構造 `assign y = a ** b;` 實測(rc=1)。S3 另見 X 語義
  實例(§20.5、D-c5)。equiv_status 在 proven 時實測**有**輸出 `Found N $equiv cells in equiv:`;計數仍取 select -count,兩者
  不等=count-mismatch(全弧 5 次 `--record` 日誌 grep 計 0 次)。
- ryosys equiv_* 只註冊里程碑(PLAN.md RY-EQUIV 行),現狀 `ryosys -p "equiv_make gold gate equiv"` 輸出
  `ERROR: equiv_make: not implemented in ryosys (milestone Y0)`,本弧未實現、未加 stub。

### 20.4 S2:波形幀診斷腿(輔助證據;條文=CONFORMANCE-PROTOCOL §8)

- 鏈路:wavecap 第二 top 取 VCD(tb.v 零改,帶探針原版輸出==expected.txt、殺傷場景==record 為硬斷言)→ vl-vcd `vcd2svg` 統一窗
  `--from 0 --to max(t_max)` → sips(qlmanage 備選)柵格化 → 剝 title 畫布 byte 比較+PNG sha 雙檢定 modality。腿內不構建
  (vcd2svg 缺或陳舊 exit 2);剝 title 用非貪婪跨行正則,禁逐行 sed;t_max 取自 vl-vcd 默認窗軸標籤,不解析 VCD。
- 六設計雙檢(缺省 counterexample=m0001):

| 設計 | t_max gold/mutant | 畫布異 | PNG 異 | title 層分歧信號 | modalityAllowed |
|---|---|---|---|---|---|
| acc_pipe | 640000/640000 | 是 | 是 | 9 | vision |
| fsm | 880000/880000 | 是 | 是 | 5 | vision |
| i2c_master | 21980000/21500000 | 是 | 是 | 26 | vision |
| signedexpr | 288000/288000 | 否 | 否 | 1 | text-fallback |
| sincos | 3770000/3680000 | 是 | 是 | 15 | vision |
| subbytes | 1040000/1040000 | 否 | 否 | 4 | text-fallback |

- 視覺診斷(frameDemo):fsm、sincos 各讀 4 張 PNG,modality=vision、confidence=medium(非盲讀);subbytes、signedexpr 機械寫
  text-fallback。讀幀能看出信號、大致時段與形狀,兩處目讀數字錯並經 title 層訂正:fsm green_a 回低實為 195000(初讀約
  205000);sincos done 前 15 次固定早 50000、末次 90000(初讀為逐步累積)——歸因不靠幀。
- 驗證器只驗工件(14 向量,4 正 10 負);方案 §4.4「vision ⇔ 雙檢皆異」的反向放寬為「雙檢皆異而標 text-fallback 須寫原因」
  (視覺會話不可用是合法降級)。降級實測:sips 壞→qlmanage 出真 PNG;兩者皆壞→不紅並標「柵格化鏈不可得」。
- 確定性:兩次全設計跑 PNG/SVG/manifest sha 全等;fsm PNG sha 9af3c1aa…/2568658c… 與規劃輪 R1 記錄逐字相同。
  驗收 build/finish-s2/acceptance.sh 27 行;selfcheck 28→37。

### 20.5 S3:T 腿全量擴面與釘選集全表(條文=CONFORMANCE-PROTOCOL §9)

- 候選池(勘察同口徑):規則數≥1 且非 TB 檔 85(L1 66/L2 19);文件級單 top 取 rulebook.modules[0] → TB 化 57、拒收 28(分布與規劃
  勘察逐值相同),57 全屬 L1。對照口徑:top 取核心默認只得 30(31 個 multi_module),兩口徑禁混用。
- staging(`--fixtures-root`,寫面限 build/)預錄 51 個新候選:ADMITTED 28、REJECTED 23(Q8 鐵則 15/Q7 13/Q4 8/Q1 5/EQ 4);
  逐設計判定表=CONFORMANCE-PROTOCOL §9(本章不重抄)。
- 升級裁定:12 個未釘 ADMITTED 全收,golden 釘選 37→49(98 份);`run-golden-regression.sh --record` 實跑既有 74 份 sha 逐值不變、
  新增 24 份;新 12 份 rules.json 與 staging 生成 spec 逐位元組相等。
- 分批主根全集重錄:C-S8-1(+10,5:20)、C-S8-2(+18,6:39),EXPECTED_DESIGNS 6→34;既有設計剝 tb.simMs 後逐位元組一致
  (6/6、16/16)。方案「首批 P0 前 12」實取 10:其餘 P0 需 golden 擴檔,裁定後併入批 2。
- 數字(S3 前→後):designs 6→34;mutants 439→863;eligible 412→794;killed 347→544;equivalent 27→69;unruled_killed 15→84;
  SUMMARY-EQ adjudicated 84→311、equivalent 27→69、unproven 57→242、failed 0→0;killed 全母體 355→552=unproven 549/failed 3/
  equivalent 0;rulebook-defects 17→97;證據頁 100→279 行(刪 0)。時長:默認模式 34 設計 3:08(另一次 2:35;均值 5.5s/設計,最大
  i2c_master 17.1s),record 6:39;方案外推 57 設計 6-10 分鐘,未觸發 P0/P1 或 --quick。
- 主根新斷言:specPath 須在 golden/ 下且 src∈釘選清單。初版只 push reason 未入 admitted(空操作),由負向探針抓出後修
  (REGISTRY Incident 11-07);S4 再把判定抽成純函數進 selfcheck(4 向量)。驗收 build/finish-s3/acceptance.sh 14 行。
- **判據變更記錄(欠條②清償)**:golden 入選判據由 §16.1.1 的「<3 條規則無斷言價值」(條數代理)改為「conformance staging
  ADMITTED=TB 實殺變異的直接證據」。更迭證據=9 個不足 3 條的新釘設計:always01、always02、attrib01_module、attrib02_port_decl、
  attrib06_operator_suffix、attrib08_mod_inst、loop_prefix_case、retime、scopes(各自的 TB 在 C-S8-2 至少殺 1 個 P-rule 變異)。
- **釘選集全表(欠條①清償;本表=釘選集 SSOT,取代 §16.1.1 表;run-plugin-check ⑤e 對拍 pinned-files.list 逐檔名、⑤e-3 核對
  「§16.1.1 原表」行的源檔名在 §16.1.1 原文出現;規則數=現行 golden 實測,非 §16.1.1 當時數字)**:

<!-- pinned-set-table begin -->
| 源檔 | 來源 | 規則數(現行 golden) | conformance 設計 |
|---|---|---|---|
| `corpus-external/yosys-simple/operators.v` | §16.1.1 原表 | 85 | — |
| `corpus-external/yosys-simple/memory.v` | §16.1.1 原表 | 59 | — |
| `corpus-external/yosys-simple/constmuldivmod.v` | §16.1.1 原表 | 46 | — |
| `corpus-external/yosys-simple/sincos.v` | §16.1.1 原表 | 46 | sincos |
| `corpus-external/yosys-simple/partsel.v` | §16.1.1 原表 | 43 | partsel |
| `corpus-external/yosys-simple/fsm.v` | §16.1.1 原表 | 23 | fsm |
| `corpus-external/yosys-simple/dff_different_styles.v` | §16.1.1 原表 | 18 | dff_different_styles |
| `corpus-external/yosys-simple/generate.v` | §16.1.1 原表 | 17 | — |
| `corpus-external/yosys-simple/process.v` | §16.1.1 原表 | 14 | — |
| `corpus-external/yosys-simple/fiedler-cooley.v` | §16.1.1 原表 | 11 | fiedler-cooley |
| `corpus-external/yosys-simple/multiplier.v` | §16.1.1 原表 | 11 | — |
| `corpus-external/yosys-simple/i2c_master_tests.v` | §16.1.1 原表 | 12 | i2c_master_tests |
| `corpus-external/yosys-simple/undef_eqx_nex.v` | §16.1.1 原表 | 8 | — |
| `corpus-external/yosys-simple/always03.v` | §16.1.1 原表 | 7 | always03 |
| `corpus-external/yosys-simple/forloops.v` | §16.1.1 原表 | 7 | forloops |
| `corpus-external/yosys-simple/realexpr.v` | §16.1.1 原表 | 7 | realexpr |
| `corpus-external/yosys-simple/subbytes.v` | §16.1.1 原表 | 16 | subbytes |
| `corpus-external/yosys-simple/signedexpr.v` | §16.1.1 原表 | 6 | signedexpr |
| `corpus-external/yosys-simple/task_func.v` | §16.1.1 原表 | 6 | — |
| `corpus-external/yosys-simple/vloghammer.v` | §16.1.1 原表 | 6 | vloghammer |
| `corpus-external/yosys-simple/wreduce.v` | §16.1.1 原表 | 7 | wreduce |
| `corpus-external/yosys-simple/muxtree.v` | §16.1.1 原表 | 16 | — |
| `corpus-external/yosys-simple/dff_init.v` | §16.1.1 原表 | 5 | dff_init |
| `corpus-external/yosys-simple/mem2reg.v` | §16.1.1 原表 | 5 | — |
| `corpus-external/yosys-simple/macros.v` | §16.1.1 原表 | 5 | — |
| `corpus-external/yosys-simple/aes_kexp128.v` | §16.1.1 原表 | 4 | aes_kexp128 |
| `corpus-external/yosys-simple/carryadd.v` | §16.1.1 原表 | 4 | — |
| `corpus-external/yosys-simple/loops.v` | §16.1.1 原表 | 11 | loops |
| `corpus-external/yosys-simple/attrib04_net_var.v` | §16.1.1 原表 | 4 | — |
| `corpus-external/yosys-simple/omsp_dbg_uart.v` | §16.1.1 原表 | 4 | omsp_dbg_uart |
| `corpus-external/yosys-simple/mem_arst.v` | §16.1.1 原表 | 4 | — |
| `corpus-external/yosys-simple/forgen02.v` | §16.1.1 原表 | 4 | — |
| `corpus/e2e/59_i2c_master/i2c_master.v` | §16.1.1 原表 | 152 | i2c_master |
| `crates/spec-i2c/rtl/i2c_master.v` | §16.1.1 原表 | 152 | spec_i2c_master |
| `corpus/e2e/58_xls_acc_pipe/acc_pipe.v` | §16.1.1 原表 | 9 | acc_pipe |
| `corpus/e2e/57_xls_add3/add3.v` | §16.1.1 原表 | 6 | add3 |
| `corpus/fulladd/fulladd.v` | §16.1.1 原表 | 3 | fulladd |
| `corpus-external/yosys-simple/always01.v` | 收尾弧 S3 擴檔 | 1 | always01 |
| `corpus-external/yosys-simple/always02.v` | 收尾弧 S3 擴檔 | 2 | always02 |
| `corpus-external/yosys-simple/attrib01_module.v` | 收尾弧 S3 擴檔 | 2 | attrib01_module |
| `corpus-external/yosys-simple/attrib02_port_decl.v` | 收尾弧 S3 擴檔 | 2 | attrib02_port_decl |
| `corpus-external/yosys-simple/attrib06_operator_suffix.v` | 收尾弧 S3 擴檔 | 2 | attrib06_operator_suffix |
| `corpus-external/yosys-simple/attrib08_mod_inst.v` | 收尾弧 S3 擴檔 | 2 | attrib08_mod_inst |
| `corpus-external/yosys-simple/attrib09_case.v` | 收尾弧 S3 擴檔 | 4 | attrib09_case |
| `corpus-external/yosys-simple/graphtest.v` | 收尾弧 S3 擴檔 | 6 | graphtest |
| `corpus-external/yosys-simple/loop_prefix_case.v` | 收尾弧 S3 擴檔 | 2 | loop_prefix_case |
| `corpus-external/yosys-simple/retime.v` | 收尾弧 S3 擴檔 | 1 | retime |
| `corpus-external/yosys-simple/scopes.v` | 收尾弧 S3 擴檔 | 1 | scopes |
| `corpus-external/yosys-simple/usb_phy_tests.v` | 收尾弧 S3 擴檔 | 10 | usb_phy_tests |
<!-- pinned-set-table end -->

### 20.6 S4:收尾

- 版本 0.4.0→0.5.0,四點一致(plugin.json/CHANGELOG 頭/server.mjs SERVER_VERSION/serverInfo 引用常量);CHANGELOG [0.5.0] 記本弧
  能力(record schema 新欄 eqMeta/waiveTable/eq 欄、WAIVE 標籤集、TB-SHA-ALARM、等價裁決腿、幀腿、擴面 34 設計、golden 49)。
  MCP 工具面與 wasm 未變。
- 欠條清償閘(FINISH-PLAN §11 欠條轉「已清償」):run-plugin-check ⑤e(本章釘選全表==pinned-files.list,來源欄==清單分組,
  §16.1.1 行對拍歷史表)+⑤f(FINISH-PLAN 無「[欠]」);解析器配 1 正 4 負自證向量;負向探針實跑必紅(build/finish-s4/)。
- wasm sha 保持=閘斷言:conformance 默認模式比對入庫 record 的 provenance.wasmSha256 與 dist wasm,不等=DRIFT 紅
  (34 份 record 皆為 66518257…)。
- 總閘實跑(工作樹終態,全部 exit 0;方案契約文字列 11 條命令並稱 12 條,本輪按「11 條+cargo build」跑足 12 條):
  ①`cargo build`(warning/error 0 行)②`cargo test --workspace`(89 suite,445 pass/0 fail/1 ignored)③`run-mcp-e2e.sh`
  (19 斷言含 ping;wasm sha256 66518257…)④`run-rtl-validation.sh`(13/13;validation-report.md 重生成無差)⑤`run-golden-regression.sh`
  (98/98+PROVENANCE)⑥`run-gap-fixtures.sh`(檔級矩陣 13 行+規則面)⑦`run-realworld-eval.sh`(353 檔,trap 0;RTL2SPEC-EVAL.md 重生成
  只差耗時行 85→95ms)⑧`run-plugin-check.sh`(含 ⑤e/⑤f)⑨`run-reimpl-eval.sh`(23 場景)⑩`build/s6-metamorphic/run.sh`(49 檔 IF=0)
  ⑪`RTL2SPEC_YOSYS=none run-conformance-eval.sh`(yosys=absent;34/34;selfcheck 38 行;2:47;證據頁 byte 相等)⑫`scripts/regress.sh`
  (最後跑;59/59、lint parity 83/87、ys-e2e 469/479、live 456/456、analyzer/MCP-E2E/rtl-validation PASS、golden 98/98、gap-fixtures PASS)。
  驗收複跑:build/finish-s0..s3 各 14/19/27/14 行全過,finish-s4 9 行全過(⑤e 少行/來源錯、⑤f 欠條、①a 版本回退、record wasm sha 篡改五形
  必紅,還原後綠)。

### 20.7 勘誤(既有章節原文保留,以本節為準)

1. **§19.21(C-S4 實作記錄)與 ADR-0013 C-S4 記錄「D 新殺 132/13/2/0 丟殺 0」**:i2c 實為丟殺 8(m0010 OP-ASYNC@70、m0022..m0028
   OP-RVAL@83..89;T 腿殺、D 腿存活),證據=build/conformance/record-diff-C-S4.md:288 與現行 record;i2c gain=124=132−8。
2. **FINISH-PLAN §1/§3.1「i2c m0011 屬 $pow 類」「能力邊界僅 $pow 一條」**:m0011 是 proc 前端錯;邊界至少兩類(proc 前端、
   `$pow` 無 SAT 模型),另有 X 語義的 proven 實例(D-c5)。
3. **FINISH-PLAN §3.1「equiv_status 在 proven 時不輸出 cells 總數」**:yosys 0.70+179 實測有輸出。
4. **§19.3.3 WAIVE 語法、§19.3.4 預登記模板 v1、§19.4.3 Q9 行**的三標籤集已由 CONFORMANCE-PROTOCOL §6 模板 v2(四標籤)與
   tblint 文法外硬紅取代;§19.3.4 預登記塊為內容錨,原文保持。
5. **S3 升級裁定原文「清單條目數=28」**:28 是 S3 收編設計數,清單實為 49;機械斷言按 49 落(golden-check EXPECTED_PINNED)。
6. **§16.1.1 表**:列 37 檔的歷史表;現行釘選集以 §20.5 全表為準。

### 20.8 已知未決與交接

- D-c5 等價腿 X 語義盲區(constmuldivmod m0003/m0033/m0064:`A % 8'd0`→`A % 8'd1`,模擬為 x,yosys 證等價),交等價裁決腿
  eqverdict 分類面。D-c6 空真探針對 memory 設計的名單盲區(arrays01)。兩者方向保守(拒設計,不假放行),相關設計入場前須先修。
- 政策待裁:operators 只因 gold-vs-gold 的 `$pow` No SAT model 被拒(TB 41/125 已殺)。是否改為「能力邊界時等價腿不適用、
  不拒設計」屬門檻變更。
- 觀察:i2c D 腿丟殺 8 個(勘誤 1)是 D 腿異步復位觀測缺口,未處理。
- ryosys equiv_* 核心實現=註冊里程碑(PLAN.md RY-EQUIV),未承諾編號。
