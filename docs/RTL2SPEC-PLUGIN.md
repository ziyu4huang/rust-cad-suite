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
