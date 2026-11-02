# ANALYZER-SERVICE — 統一分析器 + client/server 服務(正典企劃)

日期:2026-11-02。狀態:**起草輪正典**(後續實作輪與審查輪以本文為準;
決策 ADR = docs/adr/0011-analyzer-service-asp1.md,實作期落盤)。
目標(使用者原話):模組化程式碼——把 Verilog/SystemVerilog 分析器拆分出來
獨立可用且高效;原生 SV 與 yosys RTLIL(使用者原文 RTTI,指 RTLIL .il)都能
以一致的 API 吃進來,具備完善可分析性;支援 client/server 模式,client 用最高效
的通訊方式,以純 Bun TypeScript 與 server 通訊。

本弧是**新基礎設施,無 oracle 兼容契約**:禁虛構 oracle golden;行為契約一律
普通單測/集成測試釘死。既有 e2e 語料與 SUMMARY 基線凍結(regress 469/479 +
live 456/456 逐字),本弧不得翻動。弧內產出只呈現於工作樹,禁 git commit。

---

## 0. 決策總表(每條帶理由,正文展開)

| # | 決策 | 理由(證據錨) |
|---|---|---|
| D1 | 兩個新 crate:`analyzer-core`(lib)+ `analyzer-server`(bin `analyzerd`);**零文件搬移**,vl-syntax/ry-ir 原地復用為庫依賴 | 兩者導出面本就 pub 完整且零 include_str 耦合(§2 實測 rg 零匹配);搬文件=大爆炸面,復用=零既有消費端風險;10.7 core-first=核心先行 server 只做 face |
| D2 | 協議 = UDS(SOCK_STREAM)+ 4 字節大端 u32 長度前綴幀 + 負載 = **手寫 JSON 子集(零 serde)**,協議名 **ASP/1** | TS 端 JSON.parse/stringify 原生零依賴;repo 先例=零 serde JSON 子集(rv-xls-sim「JSON 子集零 serde」、ry-cli write_json 手寫);PLAN §2「核心 crates 零外部依賴」政策 |
| D3 | 併發模型 = **thread-per-connection、連線間並發、單連線內請求串行(序保證)**;analyze 無共享可變狀態 | `analyze` 是純函數(輸入→Analysis),線程安全免費;單連線內保持請求序=協議簡單可推理 |
| D4 | 停機 = `shutdown` op:回 ack → unlink socket → listener 轉 non-blocking drain 在途連線(上限 2s)→ exit 0;SIGTERM 硬殺由啟動期殘留清理兜底 | std-only 無 libc 信號處理依賴;analysis 為秒內純函數,損失窗口極小;殘留清理是通用 unix 慣例 |
| D5 | 默認 socket 路徑 = `/tmp/analyzerd.sock`;`--socket` 旗標 + `ANALYZER_SOCK` 環境變量可覆蓋;啟動時對既有 path 先試連:連通=已有活實例硬錯 exit 1,連不通=unlink 重綁;路徑 ≥100 字節硬錯 | macOS sun_path 上限 104 字節(含 NUL,ask 環境事實);留 4 字節餘量;殘留清理=煙測重跑冪等的前提 |
| D6 | 統一視圖 = `Analysis`(模組/端口/參數/nets/cells/實例邊/診斷),由**既有 IR 直讀投影**;SV 側另留原生 AST 引用、RTLIL 側留原生 Design 引用(Rust API,不跨進程) | 「禁發明新語義引擎」=查詢集只能從 ast::SourceFile / ry_ir::Design 既有欄位投影;原生逃逸口保住「完善可分析性」不封頂 |
| D7 | TS↔Rust 編解碼一致性 = 共享向量文件 `clients/analyzer-ts/fixtures/codec-vectors.json`,Rust 測試與 bun test 讀同一文件逐字節斷言;**bun test 進固定閘**(run-smoke.sh 內建步,§6.4) | 端到端 run-smoke 只證明回環通,向量把逐字節一致**單元級**釘死;向量斷言不進閘=改壞不翻紅(R1-3 對抗審);自建向量非 oracle golden |
| D8 | bench = 資訊性(smoke 計時打印),**不引入 criterion、不設硬閾值** | repo 現狀無 criterion 無 benches(§2 實測);脆弱硬閾值=假紅源;既有性能證據先例皆為一次性數字入文檔 |
| D9 | regress.sh 新增 **S5 analyzer 塊**(插在 S4 ryosys 塊後、--with-record 前),帶 `crates/analyzer-server/Cargo.toml` 在場 + `command -v bun` 雙守衛可降級 | L60/L312 既有先例形;總閘四條(build/test/regress/smoke)不破壞既有基線 |

---

## 1. 範圍

**IN**:
- `analyzer-core`:統一入口(枚舉 Sv|Rtlil)、Analysis 統一視圖、v1 查詢集、ASP/1 幀+JSON 子集編解碼(`proto` 模組)、診斷接 rcs-diag。
- `analyzer-server`(bin `analyzerd`):UDS 監聽、握手、幀循環、thread-per-connection、優雅停機、殘留清理、CLI 旗標。
- `clients/analyzer-ts/`(新頂層目錄,repo 現無 clients/,§2 實測):零依賴 TS 客戶端 + smoke.ts + run-smoke.sh + 編解碼向量 fixtures。

**OUT**:見 §11 顯式不做清單。

---

## 2. 已驗證事實(2026-11-02 起草輪實測;非轉抄勘察清單)

以下每條都在本輪親自查驗:

- **workspace 22 members 顯式列出**(`Cargo.toml:3-26`);`[workspace.package]` version 0.1.0 / edition 2021 / license MIT OR Apache-2.0 / authors(根 `Cargo.toml:28-32`);`[workspace.dependencies]` **16 條 path**(rcs-diag..vl-cli,`Cargo.toml:35-50`,rg -c 'path = ' 實測 16)+ wasm-bindgen/js-sys(`Cargo.toml:51-52`)。
- **vl-syntax 導出面**:`pub mod ast/lexer/parser/pp/token` + `pub use rcs_diag`(`crates/vl-syntax/src/lib.rs:9-15`);入口鏈 = `pp::preprocess_source(name, src, sm, diags, opts) -> PpOutput`(PpOutput 有 `tokens: Vec<Token>`,`crates/vl-syntax/src/pp.rs:56-67、24-29`)→ `parser::parse_source_file(toks, diags) -> ast::SourceFile`(`crates/vl-syntax/src/parser/mod.rs:16`)。`PpOptions { include_dirs: Vec<PathBuf>, predefines: Vec<String> }`(`pp.rs:18-22`)。Cargo.toml 僅依賴 rcs-diag(`crates/vl-syntax/Cargo.toml:9`),**零 include_str/include_bytes**(`rg -n 'include_str|include_bytes' crates/vl-syntax` 零匹配)。
- **vl-syntax AST 查詢面既有欄位**:`Module { span, name, params: Vec<ParamEntry>, ports: Vec<Port>, ansi, items: Vec<ModuleItem> }`(`ast.rs:11-20`);`Port { name, dir: Option<Direction>, net: Option<NetKind>, is_reg, signed, range: Option<Range> }`(`ast.rs:94-104`);`Range { msb: Expr, lsb: Expr, span }`(`ast.rs:106-110`);`ParamEntry { name, ty, signed, range, init: Expr }`(`ast.rs:113-122`);`ModuleItem::Decl(Decl)/ContAssign(Vec<AssignPair>)/Always{span,ctrl,body}`(`ast.rs:132-136`);`Decl { kind: DeclKind, range, dir, entries: Vec<DeclEntry> }`、`DeclEntry { name, unpacked, init }`(`ast.rs:187-206`);`Instance { mod_name, params, insts: Vec<InstUnit> }`、`InstUnit { name, array, conns }`(`ast.rs:216-251`)。**Expr 無跨進程序列化面——表達式文本不進 v1 查詢集(見 §11)**。
- **ry-ir 導出面**:`pub mod ast/lexer/parser/printer` + re-export 17 類型與 `parse_rtlil`/`write_rtlil_readable`(`crates/ry-ir/src/lib.rs:25-36`);`parse_design(input: &str) -> Result<Design, RyError>`(`parser.rs:16`,**首錯即斷=硬錯語義**,與 SV 累積式診斷不同——§4.3 記錄此不對稱);`write_readable(&Design) -> String`(`printer.rs:18`)。**零依賴、零 include_str**(`crates/ry-ir/Cargo.toml` [dependencies] 空註釋形;rg 零匹配)。
- **ry-ir AST 查詢面既有欄位**:`Design { autoidx, modules: Vec<Module>, intern_seq, autoidx_base }`(`ast.rs:197-207`);`Module { name: Id, attributes, avail_parameters, parameter_defaults: InsertMap<Const>, wires, memories, cells, processes, connections }`(`ast.rs:222-234`);`Wire { name, width: i64, start_offset, port_id, port_input, port_output, upto, is_signed, attributes }`(`ast.rs:252-262`);`Cell { ctype, name, attributes, parameters, connections: Vec<(Id, SigSpec)> }`(`ast.rs:274-281`)。**全 AST 無 span/行號欄位**——RTLIL 側統一視圖的 span 恆 None(D10 記錄)。
- **rcs-diag 面**:`Diagnostic { severity, code: &'static str, message, span, notes }`(`diagnostic.rs:25-32`);`DiagSink` 僅 pub 字段 `diags: Vec<Diagnostic>` + `new/push/error/warning/error_count/has_errors`,**無 `diags()`/`into_diags()` 方法**(`diagnostic.rs:62-94`,rg 實測 fn 清單);`SourceMap::new()/add(name, source) -> FileId`(`source_map.rs:47-53`);**`line_col` 已是 pub**:`SourceMap::line_col(&self, id: FileId, pos: BytePos) -> (u32, u32)`(`source_map.rs:72-74`;私有者僅 `SourceFile::line_col`,line 30;line 27 是 doc comment——R1 對抗審證實,起草輪 §2 原記載為錯);配套 pub 面 `has()`(:64)/`file_name()`(:68)/`line_start_offset()`(:77)/`line_text()`(:84);`render_diags(out, sm, diags)`(`render.rs:16`)已輸出帶行號 caret 文本;`Span { file, start, end }` 半開字節區間(`span.rs:28-35`)。
- **regress.sh 結構**:S4 ryosys 塊以 `if [ -f crates/ry-cli/Cargo.toml ]; then` 開(L312)、`fi` 收(L353);L355-359 = `--with-record` 塊;L361-365 = exit 判定。**S5 插入點 = L354 後 L355 前**。既有守衛形=文件在場(`-f crates/<c>/Cargo.toml`)+ `command -v` 雙降級(L197/L215/L231 先例)。基線字面:`SUMMARY ys=469/479`(L318)+ known-gap 10 例逐字(L332-334)+ `LIVE ys=456/456`(L348-349)。
- **clients/ 目錄不存在**(ls 實測)——本弧新建頂層目錄。
- **bun 1.4.2 在位**(`/Users/huangziyu/.bun/bin/bun --version` 實測輸出 1.4.2)。
- **全 repo 無 UDS/UnixListener/node:net 既有使用**(`rg -n 'Unix|node:net' crates -g '*.rs' -g '*.mjs'` 零命中)——協議棧從零起,無遺留約束。
- **ADR 編號**:0001..0010 連續(ls 實測),**下一號 = 0011**;頭部形=「日期:…。狀態:**接受**…」+ 決策/後果兩節(ADR-0010:1-40 同構參照)。
- **SKILL 增補節形**:`.agents/skills/eda-dev/SKILL.md` 頂部鐵則 + 「標準循環」+ 按弧追加 `<主題> 增補(日期,ADR-xxxx)` 二級節(rVPI/wasm/riv runner/ryosys 四先例)。
- **REGISTRY Crates 表**:五列形「Crate|職責|依賴|對應上游角色|證據/閘」(`docs/REGISTRY.md:6-9`),新 crate 必同輪登記(AGENTS §10)。

---

## 3. Crate 劃分與依賴方向(D1)

### 3.1 依賴圖(新邊只有兩條)

```
rcs-diag ←── vl-syntax ──┐
                         ├── analyzer-core ──→ analyzer-server (bin analyzerd)
ry-ir ───────────────────┘        (lib)              [新邊:analyzer-core 依賴]
                                                        [新邊:server 依賴 core]
```

- `analyzer-core` 依賴:`vl-syntax`、`ry-ir`、`rcs-diag`(全 workspace path)。**零外部依賴**(無 serde/serde_json/tokio/libc)——承 PLAN §2 政策與 ry-ir 空依賴先例。
- `analyzer-server` 依賴:`analyzer-core`(經 workspace 别名)。零外部依賴;UDS 用 `std::os::unix::net::{UnixListener, UnixStream}`(std 自帶,無 libc),線程用 `std::thread`。
- **既有 crate 零改動、零搬移(全弧零接觸底線,無例外)**:vl-syntax/ry-ir/rcs-diag 三者均以既有 pub API 原地消費。審計記錄:vl-syntax/ry-ir 均無 include_str/include_bytes/相對路徑文件內嵌(§2 rg 零匹配實測),10 個既有消費端(vl-cli/vl-difftest/vl-sim/riv-*/vl-spec/vl-ffi/vl-wasm)的 Cargo.toml 依賴邊不動。
- **(R1 撤銷原「唯一例外改動」條款)** 起草輪曾計劃給 rcs-diag 追加 line_col 存取器——對抗審證實為事實錯誤:`SourceMap::line_col` 早已 pub(`source_map.rs:72-74`,返回 `(u32, u32)`;私有者僅 `SourceFile::line_col`,line 30),照原案追加同名方法 = Rust 無重載 → **E0592 duplicate definition,S1 必編譯失敗**。修正 = **零接觸直接消費既有 pub 面**:`line_col`(:72)取行列、越界文件以既有 `has()`(:64)判界。rcs-diag 本弧零改動。

### 3.2 Cargo.toml 形態(跟 ry-ir 新 crate 形)

```toml
# crates/analyzer-core/Cargo.toml
[package]
name = "analyzer-core"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
[dependencies]
vl-syntax = { workspace = true }
ry-ir = { workspace = true }
rcs-diag = { workspace = true }
```
```toml
# crates/analyzer-server/Cargo.toml
[package]
name = "analyzer-server"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
[[bin]]
name = "analyzerd"
path = "src/main.rs"
[dependencies]
analyzer-core = { workspace = true }
```
workspace 根改動僅兩處:members 追加兩行(`Cargo.toml:3-26`);`[workspace.dependencies]` 追加 `analyzer-core = { path = "crates/analyzer-core" }`(L34-51;server 需跨 crate 引用,故入表)。

### 3.3 core-first 合規聲明(10.7)

- 全部語義能力(分析、查詢、協議編解碼)先落 `analyzer-core`;`analyzer-server` 只是 **新 face**(接收→解碼→調 core→編碼→回寫),不含語義;任何未來 face(wasm/FFI/其它 client)直呼 core,零複製。
- `proto` 模組(幀+JSON 子集)放 **core** 而非 server:編解碼律是契約不是面,單測/向量/TS 對拍都消費 core 的 codec;server 只是 codec 的第一個用戶。

---

## 4. 統一 API(D6)

### 4.1 入口與 Analysis(Rust 簽名,最終以編譯產物為準)

```rust
// crates/analyzer-core/src/lib.rs
pub mod view;    // 統一視圖類型
pub mod proto;   // ASP/1 幀 + JSON 子集編解碼(S3/S2)
pub mod sv;      // vl-syntax 投影
pub mod rtlil;   // ry-ir 投影

pub use rcs_diag;

/// 輸入判別(入口枚舉)。
#[derive(Clone, Debug)]
pub enum SourceInput {
    /// Verilog/SystemVerilog 源碼:pp → lex → parse 全管線。
    Sv(SvInput),
    /// RTLIL 文本(oracle 產 -readable 形;ry-ir::parse_rtlil)。
    Rtlil(RtlilInput),
}

#[derive(Clone, Debug, Default)]
pub struct SvInput {
    /// 檔名(診斷渲染/SourceMap 註冊用,可非磁碟路徑)。
    pub name: String,
    pub source: String,
    /// include 目錄與 -D 預定義,直通 vl_syntax::pp::PpOptions。
    pub include_dirs: Vec<std::path::PathBuf>,
    pub defines: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct RtlilInput {
    pub name: String,
    pub source: String,
}

/// 統一分析結果:投影視圖 + 原生 IR 逃逸口 + 診斷。
pub struct Analysis {
    pub kind: InputKind,                       // Sv | Rtlil
    pub diagnostics: Vec<rcs_diag::Diagnostic>,
    pub diag_text: String,                     // render_diags 產物(帶 caret)
    pub modules: Vec<view::ModuleInfo>,
    /// 原生逃逸口(跨進程協議只送投影視圖;此二者僅 Rust API 可及)。
    pub sv: Option<vl_syntax::ast::SourceFile>,
    pub rtlil: Option<ry_ir::Design>,
    /// SV 側 SourceMap(診斷渲染與行列查詢);RTLIL 側 None。
    pub(crate) sm: Option<rcs_diag::SourceMap>,
}

pub enum InputKind { Sv, Rtlil }

/// 唯一入口:分派 + 投影。永不 panic;語法錯誤進 diagnostics 不中止
/// (SV 累積語義;RTLIL 首錯硬錯語義=單條 error 診斷,§4.3)。
pub fn analyze(input: &SourceInput) -> Analysis;
```

SV 管線(S1 實作,全部既有函數直呼):

```rust
fn analyze_sv(input: &SvInput) -> Analysis {
    let mut sm = rcs_diag::SourceMap::new();
    let mut diags = rcs_diag::DiagSink::new();
    let opts = vl_syntax::pp::PpOptions { include_dirs: input.include_dirs.clone(),
                                          predefines: input.defines.clone() };
    let pp_out = vl_syntax::pp::preprocess_source(&input.name, &input.source,
                                                  &mut sm, &mut diags, &opts);
    // pp 輸出即 tokens(PpOutput.tokens,pp.rs:24-29),直進 parser。
    let ast = vl_syntax::parser::parse_source_file(pp_out.tokens, &mut diags);
    let modules = sv::project(&ast, &mut diags);
    let mut text = String::new();
    rcs_diag::render_diags(&mut text, &sm, &diags.diags); // pub 字段借用
    Analysis { kind: InputKind::Sv, diagnostics: diags.diags, // 字段 move
               diag_text: text, modules, sv: Some(ast), rtlil: None, sm: Some(sm) }
    // DiagSink 無 diags()/into_diags()(diagnostic.rs:62-94 僅 pub 字段
    // diags + new/push/error/warning/error_count/has_errors)——R1 修正:
    // 渲染借用 &diags.diags、收集 move diags.diags。
}
```

RTLIL 管線:

```rust
fn analyze_rtlil(input: &RtlilInput) -> Analysis {
    let mut sm = rcs_diag::SourceMap::new();
    sm.add(input.name.clone(), input.source.clone()); // 渲染上下文,非語義
    match ry_ir::parse_rtlil(&input.source) {
        Ok(design) => { /* project + render(零診斷時 diag_text 空) */ }
        Err(e) => /* RyError → 單條 Severity::Error 診斷("RY-E…" 碼延用
                     ry-ir 既有錯誤文本),modules 空,rtlil: None */,
    }
}
```

### 4.2 v1 查詢集(投影視圖;全部既有 IR 欄位直讀,零新語義)

```rust
// crates/analyzer-core/src/view.rs
pub struct ModuleInfo {
    pub name: String,
    pub params: Vec<ParamInfo>,     // SV: ParamEntry(init 值取字面量形,§4.5)
                                    // RTLIL: avail_parameters + parameter_defaults
    pub ports: Vec<PortInfo>,       // SV: Module.ports;RTLIL: port_id>0 的 wires
    pub nets: Vec<NetInfo>,         // SV: Decl 面(Net/Reg 族,含 unpacked 維數);
                                    // RTLIL: wires 全量
    pub cells: Vec<CellInfo>,       // SV: Instance(閘原語與模組例化同形);
                                    // RTLIL: cells
    pub proc_count: usize,          // SV: Always+Initial item 計數;RTLIL: processes 數
    pub assign_count: usize,        // SV: ContAssign 對數;RTLIL: connections 數
    pub span: Option<SpanInfo>,     // SV: Module.span;RTLIL: 恆 None(D10)
}

pub struct ParamInfo { pub name: String, pub value: String }
pub struct PortInfo  { pub name: String, pub dir: Dir, pub width: u32,
                       pub parametric: bool,   // 寬度非字面量(§4.2 派生規則)
                       pub signed: bool, pub span: Option<SpanInfo> }
pub enum Dir { Input, Output, Inout }
pub struct NetInfo   { pub name: String, pub kind: NetKindTag, // wire/reg/int/…
                       pub width: u32, pub is_port: bool,
                       pub unpacked_dims: u32,   // SV 記憶體維數;RTLIL 恆 0
                       pub span: Option<SpanInfo> }
pub struct CellInfo  { pub name: String, pub ctype: String,
                       pub port_count: usize,    // 連接數(SV: conns;RTLIL: connections.len)
                       pub span: Option<SpanInfo> }
pub struct SpanInfo  { pub file: String, pub start_line: u32, pub start_col: u32,
                       pub end_line: u32, pub end_col: u32 }

impl Analysis {
    pub fn has_errors(&self) -> bool;
    pub fn diagnostics(&self) -> &[rcs_diag::Diagnostic];
    pub fn modules(&self) -> &[view::ModuleInfo];
    pub fn module(&self, name: &str) -> Option<&view::ModuleInfo>;
    /// 跨模組實例邊:SV=Instance.mod_name;RTLIL=cell.ctype 命中 design
    /// 模組名集合(兩側同一查詢形=「一致 API」的直接體現)。
    pub fn instance_edges(&self) -> Vec<(String, String)>; // (from_module, to_module)
    /// 行列查詢(SV 側直通既有 pub `rcs_diag::SourceMap::line_col`,
    /// source_map.rs:72-74——**零接觸,禁追加同名存取器**;RTLIL=Err
    /// ("no span info"))。**file 維度必帶(R2-3)**:pp 的 include/宏/
    /// paste 向同一 SourceMap add 多文件(pp.rs:406/:353/:650 實證),
    /// `SourceMap::line_col` 是 `(FileId, BytePos)` 雙參——單 pos 無所屬
    /// 文件語義,含 include 的源會取錯行列表;入參=(FileId, byte offset),
    /// 出參=1-based (line, col)。跨進程協議不暴露此查詢(Rust API only)。
    pub fn line_col(&self, file: rcs_diag::FileId, pos: u32) -> Result<(u32, u32), String>;
}
```

寬度派生規則(既有欄位直讀,無推導引擎):
- SV `Port.range: Option<Range>` → `width = msb - lsb + 1` 由 `Range.msb/lsb: Expr` 的**字面量形**求值(`Expr::NumLit` 直取;非字面量參數化寬度 → width 記 0 + `parametric: true` 標記,不發明參數求值器——求值是 riv-elab/vl-sim 既有職責,禁本弧重造)。
- RTLIL `Wire.width: i64` 直取;`dir` 由 `port_input/port_output` 兩 bool 派生(both=true→Inout;port_id≤0→非端口進 nets)。

**SV 端口投影雙腿規則(R2-1 全面改寫;R1-8 原文本只有 ANSI 半邊)**:
- **ANSI 腿**(`Module.ansi=true`):`ast.rs:94-96` doc 明載 `Port.dir/net` 為 Option、`None entries inherit the previous port's`——投影時按 ports 序遍歷攜帶 `last_dir/last_net` 狀態逐字段獨立繼承;`PortInfo.dir` 非可空,繼承必做。
- **非 ANSI 腿**(`Module.ansi=false`):`Port.dir/net/signed/range` **恆 None**(parser/module.rs:217-225 非 ANSI 分支實證全 None)——方向與寬度在模組體 `ModuleItem::Decl` 承載(`ast.rs:194` doc「Direction when this is a non-ANSI `input/output/inout` declaration」),投影按 `DeclEntry.name` 掃 items 建名→Decl 映射回填 dir/net/signed/range。**探針實跑證據(2026-11-02,/tmp 獨立 crate 直呼 vl-syntax)**:`module top(a, b);\ninput a, b;\nendmodule` → `diag_count=0 has_errors=false`(**合法源無 parser 診斷——R1-8 原文「無任何顯式 dir=非法源」斷言為錯,撤**),`ansi=false`、兩個 `Port.dir` 均 None,模組體 `Decl{kind:Net(Wire), dir:Some(Input), entries:[a,b]}`;不實現回填則 `a/b` 投影成無 dir 佔位=錯誤 API 輸出。
- **佔位語義(修正後)**:`Dir::Inout`+width=0 佔位僅適用於**真孤兒端口**——ANSI 腿序列內無任何顯式 dir **且** items 無對應 Decl.dir 的名字(非法源);合法非 ANSI 源經回填後 dir 恆正確,寬度規則同 §4.2 既有派生(字面量求值/parametric 標記,Decl.range 與 Port.range 同法)。

### 4.3 語義不對稱(記錄,不修)

SV 側 parser 是恢復式累積診斷(rcs-diag sink);RTLIL 側 `parse_design` 是 `Result<Design, RyError>` 首錯硬斷(`parser.rs:16`)。統一 API 層如實呈現:SV=多診斷+部分模組;RTLIL=單 error 診斷+零模組。**禁為對稱而改 ry-ir 錯誤語義**(那是 ryosys 契約面)。

### 4.4 診斷碼

- SV 側原樣透傳 vl-syntax 既有碼(VL-E…;include 找不到等 pp 診斷由 vl-syntax pp 內部產 VL 碼,`preprocess_file` 先例形 `pp.rs:40-46`——analyzer 走 `preprocess_source` 內存源,**無 AN 側開檔失敗路徑**,故無 AN-E002,R1 撤銷死碼);RTLIL 側 RyError → 包一層 `AN-E001`(code 穩定、message 帶 ry-ir 原文);本弧自身診斷碼僅此一條。新增碼同輪列進本文 §13 字典。

### 4.5 參數值文本形

`ParamInfo.value` = SV `ParamEntry.init: Expr` 的**字面量形渲染**(NumLit 的 width/base/digits 既有欄位拼回;非字面量 init → `"<expr>"` 佔位);RTLIL 取 `parameter_defaults` 的 `Const`(Bits→十六進制 `N'h…`、Str→帶引號)。理由:值文本是查詢集可交付的最深層;表達式求值=禁區(§11)。

---

## 5. 線協議規範:ASP/1(Analyzer Service Protocol v1,D2/D3/D4/D5)

### 5.1 幀格式

```
+----------------------+---------------------------+
| len: u32 (大端, 4B)  | payload: len 字節(UTF-8) |
+----------------------+---------------------------+
```
- `len` 只計負載。`len == 0` = 協議違約。`len > MAX_FRAME`(默認 64 MiB,`--max-frame` 可調,clamp 上限 1 GiB)= 協議違約。
- 違約處置:**讀到前綴即檢查、先於任何分配**;違約 → 回一幀 `E_PROTO` 錯誤(若 len 合理可回)後**立即關閉連線**;超限幀不讀負載不分配(記憶體安全)。

### 5.2 負載編碼:JSON 子集(手寫,零 serde)

```rust
// crates/analyzer-core/src/proto/json.rs
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),   // 保序:寫出序=插入序(向量可 byte-exact 的前提)
}
impl Json {
    pub fn write(&self, out: &mut String);              // 正典寫律 §5.3
    pub fn parse(s: &str) -> Result<Json, CodecError>;  // 嚴格子集,見禁項
}
```
Reader 禁項(命中即 `E_CODEC`):浮點/指數字面量(`1.5`、`1e5`——v1 負載無浮點)、越 i64 整數、前導零、對象重複鍵、尾逗號、註釋、NaN/字面量、非 UTF-8(幀級已保 UTF-8)、控制字符裸字節。
理由:整數+字符串+結構已覆蓋分析負載全部需求;禁浮點使 TS↔Rust 數字序列化逐字節一致變成平凡命題(D2/風險 R2)。

**TS 側禁項執法 = 實現約束(非聲明即可)**(R1-7,bun 1.4.2 本機實測):`JSON.parse` 對浮點(`JSON.parse("1.5")` accepted)、指數(`"1e5"`→100000)、重複鍵(後值勝)、越 2^53 精度整數(`9007199254740993`→`…992`)**全部靜默放行**;`Buffer.toString('utf8')` 對非法 UTF-8 靜默替換 U+FFFD。故 §6.2 codec.ts **禁用 JSON.parse 與 `toString('utf8')`**,改 = `TextDecoder('utf-8',{fatal:true})`(實測非法 UTF-8 拋 TypeError)+ 手寫遞歸下降文本 parser(與 Rust 同律)。Rust 側 parser 天然嚴格,無此問題。

### 5.3 正典寫律(兩端逐字節一致的法源;向量釘死)

1. 緊湊形,零空白:`{"a":1,"b":[null,true]}`。
2. Int:十進制,負數帶 `-`,禁前導零。**兩端序列化律(R1-2 釘死)**:Rust=i64 Display(`-9223372036854775808`);TS=**`bigint` 為 Int 的唯一表示型別,編碼恆 `BigInt.toString(10)`**(bun 1.4.2 實測輸出精確同形)。**TS 側禁用 `JSON.stringify` 處理含 Int 負載**:實測 `JSON.stringify(-9223372036854775808)`(number 路線)輸出 `-9223372036854776000`(精度丟真),`JSON.stringify(bigint)` 拋 TypeError——兩路皆不可用;decode 側文本級解析後做 i64 範圍校驗(越界=E_CODEC)。
3. Str:僅轉義 `\" \\ \b \f \n \r \t`;其余 < 0x20 → `\u00xx`(**小寫**十六進制,同 JS `JSON.stringify` 行為);其余字符原樣 UTF-8(與 ES2019 well-formed stringify 同形,astral 字符不拆代理對)。
4. Obj 鍵**不重排**(插入序即寫出序)——兩端構造時同序即可逐字節對齊。
5. 幀校驗和/壓縮:**無**(UDS 本機零丟包面;保持最薄)。

### 5.4 消息形與握手

每幀負載 = 一個 JSON Obj。請求:`{"id":<i64 非零>,"op":"<str>","args":{…}}`;
響應成功:`{"id":N,"ok":true,"result":{…}}`;響應失敗:`{"id":N,"ok":false,"error":{"code":"<str>","message":"<str>"}}`。

**握手**:連線首幀必須是 `op=hello` 且 `args.proto == 1`;server 回 `{"proto":1,"server":"analyzerd","version":"<semver>","ops":["hello","ping","analyze","shutdown"]}`。首幀非 hello 或 proto 不支持 → `E_VERSION` 錯誤幀後關閉。版本兼容律:向後兼容變更=只增字段(雙端忽略未知字段——reader 按鍵查找、嚴格禁重複鍵但不拒未知鍵);破壞性變更=proto 升 2(新協議族)。

### 5.5 ops 表(v1)

| op | args | result / 行為 |
|---|---|---|
| `hello` | `{proto:1, client?:{name,version}}` | §5.4 握手響應 |
| `ping` | `{}` | `{"pong":true,"uptime_ms":<i64>}`(就緒探針) |
| `analyze` | `{kind:"sv"|"rtlil", name:<str>, source:<str>, include_dirs?:[str], defines?:[str]}` | `{"has_errors":<bool>, "modules":[…], "diagnostics":[…], "diags_text":<str>}`(§13 字典;`kind=sv` 才受理 include_dirs/defines,RTLIL 帶之=`E_ARGS`) |
| `shutdown` | `{}` | 回 `{"bye":true}` → §5.7 停機序列 |

未知 op → `E_OP` 錯誤幀(連線不斷,可續用)。args 缺字段/型別錯 → `E_ARGS`。

### 5.6 錯誤幀字典

| code | 語義 | 連線 |
|---|---|---|
| `E_VERSION` | 首幀非 hello / proto 不支持 | 關閉 |
| `E_PROTO` | 幀長違約/空幀/負載非 UTF-8/非 JSON Obj | 關閉 |
| `E_CODEC` | JSON 子集禁項(§5.2) | 關閉 |
| `E_OP` | 未知 op | 保持 |
| `E_ARGS` | args 缺失/型別錯/RTLIL 帶 pp 選項 | 保持 |
| `E_INTERNAL` | 請求處理 panic(catch_unwind 捕獲;workspace 無 panic=abort 設置,unwind 為真——根 Cargo.toml [profile.release] 僅 debug=1) | 保持 |
| `E_SHUTDOWN` | **僅指既有連線**在停機 drain 窗口內到達的請求:除 `shutdown`/`ping` 外一律 `E_SHUTDOWN` 拒收(shutdown=冪等再 ack;ping=照常 pong 供存活探針)。**新連線在 drain 窗口 = accept 後立即 close、不發任何幀**(R1-6 仲裁:兩種語義各據一句會無從驗收,寫死此種——新連線「被斷開」可由 client connect 失敗/EOF 機械觀測,無幀歧義) | 保持至關閉 |

### 5.7 併發模型與優雅停機(D3/D4)

- **thread-per-connection**:`UnixListener::incoming()` accept 循環在主線程;每連線 `std::thread::spawn`,連線內**串行**處理請求(幀序=響應序);連線間天然並發。共享狀態僅三個原子:`AtomicBool SHUTDOWN`、`AtomicUsize CONN_COUNT`、`Instant START`(只讀)。`analyze` 純函數無共享可變態——線程安全零成本獲得。
- 每請求 `catch_unwind` 包裹;panic → `E_INTERNAL`,連線存活。
- **優雅停機序列**(shutdown op 或最後一條連線送 shutdown):① 回 ack 幀並 flush;② unlink socket 文件(新 client 立即可見服務終止);③ listener `set_nonblocking(true)`,`SHUTDOWN=true`;④ drain:主循環續 accept,**新連線 accept 後立即 close(不發幀,§5.6 仲裁條)**,等 `CONN_COUNT==0` 或 2 秒超時;⑤ `process::exit(0)`。
- SIGTERM/SIGINT:std-only 不裝信號處理器(libc=禁新外部依賴),進程被殺時 socket 文件殘留——由 **啟動期清理** 兜底(§5.8)。此為記錄在案的 v1 邊界。

### 5.8 socket 路徑與殘留清理(D5)

- 默認 `/tmp/analyzerd.sock`;優先級 `--socket PATH` > 環境 `ANALYZER_SOCK` > 默認。
- 啟動時若 path 已存在:`UnixStream::connect(path)` 成功 = **已有活實例,硬錯 exit 1**(帶「already serving」訊息,禁靜默雙實例);connect 失敗 = 殘留死文件,`remove_file` 後重綁。
- 路徑字節長 ≥ 100 → 硬錯 exit 1 帶提示(macOS sun_path 104 含 NUL,留 4 餘量;ask 環境事實)。
- 綁定後 `set_permissions`(0o600,owner-only)= 本機訪問邊界(無認證的替代,§11 記錄)。

### 5.9 server CLI 契約

```
analyzerd [--socket PATH] [--max-frame BYTES] [--version] [--help]
```
未知旗標 = 硬錯 exit 1(承 PLAN §6「driver 未知旗標=硬錯」同款裁決);`--version` 輸出 `analyzerd <semver> (ASP/1)`。退出碼:0=正常(含 shutdown);1=用法/配置錯(旗標/路徑超長/活實例衝突);2=綁定 IO 錯。

---

## 6. TS 客戶端(純 Bun,零原生依賴)

### 6.1 文件布局(全部在 `clients/analyzer-ts/` 下;本弧新測試只放新 crate 與 clients/,合規)

```
clients/analyzer-ts/
├── run-smoke.sh            # 端到端冒煙入口(總閘第四條;bash 直接執行)
├── smoke.ts                # bun 直接執行;對真 server 做請求/響應回環+斷言
├── src/
│   ├── client.ts           # AnalyzerClient:connect/analyze/ping/shutdown/close
│   ├── codec.ts            # 幀讀寫 + JSON 子集 encode/decode(與 Rust 同律 §5.3)
│   └── types.ts            # 協議與 Analysis 視圖 TS 型別
├── test/
│   └── codec.test.ts       # bun test;斷言 fixtures 向量 + 敵意用例
└── fixtures/
    └── codec-vectors.json  # TS↔Rust 共享編解碼向量(§6.3)
```
**零 package.json、零 node_modules、零安裝步驟**:bun 原生執行 TS(ESM),`node:net` 為 Node/Bun 內建模組。「零依賴」主張可機械驗證=目錄樹內無 package.json/lock/node_modules。

### 6.2 client.ts 簽名(TS 側)

```ts
export class AnalyzerClient {
  // maxFrame 默認 64 MiB = server §5.1 同值(兩端同常量;覆蓋僅測試用,
  // 生產路徑禁單端漂移)。超限判定對稱律:client 在 encode/寫出前先拒
  // (拋 RangeError,不發幀)——server 亦拒,行為不對稱的方向=client 先擋,
  // 免發出注定被斷連的幀(R1-12)。
  static async connect(path: string, opts?: { maxFrame?: number }): Promise<AnalyzerClient>;
  async ping(): Promise<{ pong: true; uptime_ms: number }>;
  async analyze(req: AnalyzeRequest): Promise<AnalyzeResult>;   // has_errors/modules/diagnostics/diags_text
  async shutdown(): Promise<{ bye: true }>;
  close(): void;                                                // 本地關閉,不送幀
  // 內部:每請求 Promise + id 自增(number 安全域內,協議側 Int=bigint);
  // 單連線內請求排隊(響應按 id 配對,幀序=請求序,與 §5.7 串行契約互為鏡像)
}
```
`node:net` 的 `createConnection(path)` 建 UDS;socket 設 `noDelay`(寫即推)。幀寫 = 單 `write(Buffer.concat([lenBuf, payload]))`(4 字節前綴+負載一次系統調用);幀讀 = 累積 buffer + 頭 4 字節解 len + 校 `0 < len ≤ maxFrame` + 截取——與 Rust 側同律。

**codec.ts 實現約束(R1-7 實測釘死,禁慣用捷徑)**:
1. **禁 `JSON.parse`**:浮點/指數/重複鍵/越 2^53 整數全被其靜默放行(bun 1.4.2 實測,§5.2 附實測記錄)——decode = 手寫遞歸下降文本 parser,與 Rust `Json::parse` 同律同禁項。
2. **禁 `Buffer.toString('utf8')` 做幀校驗**:非法 UTF-8 靜默替換 U+FFFD(實測)——decode 前置 = `new TextDecoder('utf-8',{fatal:true}).decode()`(實測非法序列拋 TypeError → E_CODEC),通過後方得為文本。
3. **禁 `JSON.stringify` 含 Int 負載**:number 路線精度丟真(`-9223372036854775808`→`…6000` 實測)、bigint 路線拋 TypeError(實測)——encode 的 Int 恆 `BigInt.toString(10)`(實測與 Rust i64 Display 逐字節同形,§5.3 律 2)。
4. Json 型別:`type J = null | boolean | bigint | string | J[] | [string, J][]`(Int=bigint 為唯一整數表示;encode 入參即此型,型別層擋 number 混入)。

### 6.3 TS↔Rust 編解碼一致性釘法(D7)

- `fixtures/codec-vectors.json`:條目形 `{ "name": "<str>", "frame_hex": "<hex>", "note": "<str>" }`。覆蓋:hello 幀、帶嵌套 Arr/Obj 的 analyze 請求、含全部七種轉義的 Str(`\" \\ \b \f \n \r \t`)、控制字符 `\u000f` 小寫角、CJK/astral 字符原樣角、i64 邊界(`-9223372036854775808`——**TS 斷言形=encode(-9223372036854775808n) 含 `BigInt.toString(10)` 產出;number 路線已實測丟真禁入**,R1-2)、錯誤幀、超限 len 前綴、空 len 幀、浮點負載(兩端皆拒)、重複鍵(兩端皆拒)、非 UTF-8 幀(兩端皆拒)。
- **Rust 側**:`crates/analyzer-core/tests/codec_vectors.rs` 以
  `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../clients/analyzer-ts/fixtures/codec-vectors.json"))` 讀同一文件(層級已數:`CARGO_MANIFEST_DIR`=crates/analyzer-core,`../..`=repo 根——AGENTS §7「先數層級」紀律,與 tests/ 檔案相對形的三層上溯是兩種合法形,本弧統一用 manifest-dir 形免歧義),對正例斷言 `Json::write` 產出 `frame_hex` 負載逐字節相等 + `Json::parse(frame_hex 負載)` 回讀相等;對敵意例斷言兩端同判「拒」。
- **TS 側**:`test/codec.test.ts`(`bun test` 執行)讀同一 fixtures,同斷言(`encode(obj) === hex 負載`)。
- 此文件是**自建測試向量**(新基礎設施無 oracle;由 Rust 側測試生成輔助腳本首錄、雙端斷言維持),非虛構 oracle golden,合規。

### 6.4 run-smoke.sh 契約(總閘第四條)

```bash
#!/usr/bin/env bash
# 端到端冒煙:構建(如需)→ 起 server(臨時 UDS)→ bun smoke.ts 真回環 →
# 斷言 → 清理 → 退出碼表意。
set -euo pipefail
cd "$(dirname "$0")"
REPO="$(cd ../.. && pwd)"
command -v bun >/dev/null 2>&1 || { echo "smoke: bun not found" >&2; exit 2; }
command -v cargo >/dev/null 2>&1 || { echo "smoke: cargo not found" >&2; exit 2; }
cargo build -q --manifest-path "$REPO/Cargo.toml" -p analyzer-server || exit 2
BIN="$REPO/target/debug/analyzerd"
TMP="$(mktemp -d)"; SOCK="$TMP/a.sock"
[ "${#SOCK}" -lt 100 ] || { echo "smoke: UDS path too long: $SOCK" >&2; rm -rf "$TMP"; exit 2; }
"$BIN" --socket "$SOCK" & SRV=$!
cleanup() { kill "$SRV" 2>/dev/null || true; wait "$SRV" 2>/dev/null || true; rm -rf "$TMP"; }
trap cleanup EXIT
for _ in $(seq 1 50); do [ -S "$SOCK" ] && break; sleep 0.1; done
[ -S "$SOCK" ] || { echo "smoke: server socket never appeared" >&2; exit 1; }
bun test            # 編解碼向量閘(test/codec.test.ts;D7 的 TS 半進固定閘,
                    # R1-3:向量斷言不進閘=改壞不翻紅,exit≠0 由 set -e 落 1)
bun smoke.ts "$SOCK"
```
- 退出碼:0=全過;1=行為斷言失敗(含 server 起不來);2=前置環境缺(bun/cargo/build 失敗)。
- **內部自行構建**(閘契約第 2 條);臨時 UDS 路徑 mktemp 短路徑 + 長度守衛;trap 保證清理(server 進程與 tmp 目錄),重複執行冪等。

### 6.5 smoke.ts 斷言面

1. `ping` → `pong:true`;
2. SV analyze(內嵌最小模組源)→ `has_errors:false`、模組名/端口寬度逐項斷言;
3. RTLIL analyze(內嵌最小 .il 源)→ 同上(RTLIL 途徑);
4. SV 語法錯源 → `has_errors:true` 且 diagnostics 非空;
5. 未知 op → `E_OP` 錯誤幀;args 缺字段 → `E_ARGS`;
6. 兩連線並發各發 analyze → 兩響應正確(併發模型實證);
7. `shutdown` → ack 後 socket 文件消失(優雅停機實證;**進程退出碼不在本斷言面**——smoke.ts 只收 SOCK 路徑,拿不到 PID/wait,該斷言移址 `analyzer-server/tests/loopback.rs` 的 `child.wait()`,R1-5);
8. 打印資訊性計時(§7):每 op 耗時 ms、server 啟動到首響應耗時。

---

## 7. 性能口徑(D8)

- **復用既有 parser 即性能方案**:vl-syntax(SV 純內存管線,無文件 IO 耦合)與 ry-ir(零依賴直 parse `&str`)本就高效;分析器零額外中間層——幀解碼直得 `&str` 餵 parser,投影是 O(AST) 單遍,編碼單遍 `String` 累積。
- **資訊性 bench,禁硬閾值**:smoke.ts 打印 per-op 延遲與啟動延遲(§6.5-8);數字記入本弧收口時的 REGISTRY/PLAN 行(承「一次性 ad-hoc 數字入文檔」先例:RK5 575k cycles/s、M7d 0.106ms/run);**不引入 criterion**(新外部依賴,與「核心零依賴」政策衝突,且無上游對標意義——本弧無 oracle),不設任何 pass/fail 時間斷言(脆弱閾值=假紅源)。
- **不拖慢既有測試**:新測試全為純內存單測+一次真回環集成測試(server 啟動輪詢帶 5s 硬超時上限);總閘 `cargo test --workspace` 增量為秒級;regress.sh 既有塊零改動,S5 新塊放最後。

---

## 8. 測試與閘

### 8.1 測試布局(新測試只在新 crate 與 clients/,零既有語料改動)

| 位置 | 內容 |
|---|---|
| `analyzer-core/tests/api.rs` | 統一 API:SV/RTLIL 正例投影逐項斷言(模組/端口/寬度/參數/cells/實例邊/**ANSI 繼承腿+非 ANSI 回填腿**,R2-1)、SV 語法錯累積、RTLIL 首錯硬斷、parametric 寬度標記、行列查詢(`Analysis::line_col(FileId, pos)` 直通既有 pub 雙參面,include 多文件語義正確,R2-3) |
| `analyzer-core/tests/codec_vectors.rs` | 共享向量雙端一致性(§6.3)+ 敵意 fixture |
| `analyzer-core/src/proto/*` | 內聯單測:JSON 子集 reader 禁項逐條、寫律 5 條逐條 |
| `analyzer-server/tests/loopback.rs` | Rust 真回環:`Command::spawn` analyzerd → `std::os::unix::net::UnixStream` 客戶端 → hello/ping/analyze(SV+RTLIL)/E_OP/oversize 斷連/stale socket 重綁/活實例衝突 exit 1/路徑超長 exit 1/shutdown 後 socket 文件消失 **+ 進程 exit 0(`child.wait()` 實證,smoke.ts 移址至此的斷言,R1-5)**。**每測自建 tmpdir + 唯一短路徑**(`std::env::temp_dir()` 下隨機後綴,長度斷言 <100)——cargo test 默認並行,固定路徑=活實例衝突測試與其它測試互踩假紅(R1-13) |
| `clients/analyzer-ts/test/codec.test.ts` | bun test;向量+敵意(§6.3) |
| `clients/analyzer-ts/smoke.ts` | 端到端(§6.5) |

### 8.2 總閘(腳本執行,順序即優先級)

```
cargo build --workspace                 # 零 error 零 warning(AGENTS §8-1)
cargo test --workspace                  # 全綠(§8-2)
bash scripts/regress.sh                 # 全 PASS;469/479 與 456/456 逐字不動
bash clients/analyzer-ts/run-smoke.sh   # exit 0
```

### 8.3 regress.sh S5 塊(插入 L354 後 L355 前;D9)

```bash
if [ -f crates/analyzer-server/Cargo.toml ] && command -v bun >/dev/null 2>&1; then
    if bash clients/analyzer-ts/run-smoke.sh; then
        echo "analyzer smoke: PASS"
    else
        echo "GATE FAIL: analyzer smoke"; fail=1
    fi
else
    echo "analyzer smoke: SKIP (crate/bun absent)"
fi
```
雙守衛形(文件在場+command -v)= L197/L215/L231 既有降級先例;**既有塊零改動**,基線數字零接觸。向量閘無需在 S5 塊雙跑:run-smoke.sh 內建 `bun test` 步(§6.4,R1-3)已把 TS 半進固定閘,regress 一層調用即覆蓋。

---

## 9. 文檔面要點(實作期落盤,四層同輪)

1. **`docs/adr/0011-analyzer-service-asp1.md`**(下一位編號,ls 實測 0001..0010;文件名與協議名 ASP/1 統一,R1-9 撤銷殘留舊名 udsp):決策=D1..D9 全表+後果(正:零搬移縱切小、core-first 面可擴;負:JSON 子集自養、RTLIL 無 span、停機 drain 上限);頭部「日期/狀態:接受(隨 S1 落地轉正)」同構 ADR-0010。
2. **PLAN.md §5 里程碑表新行**(四列形 `| M | 內容 | 驗收 | 狀態 |`):`| AN1..AN5 | 統一分析器+ASP/1 服務(analyzer-core/analyzerd/clients/analyzer-ts;正典=docs/ANALYZER-SERVICE.md,ADR-0011) | 總閘四條;smoke PASS;基線 469/479+456/456 不動 | ◐ 起草輪 2026-11-02 |`;PLAN §1 路線圖加一條「P3.7 統一分析器服務弧」指針。
3. **SKILL.md 追加二級節「分析器服務增補(日期,ADR-0011)」**,要點:①server=新 face 先落 core(10.7 同款);②ASP/1 編解碼律=契約,禁兩端各自演化,向量文件單一事實源;③UDS 路徑 104 上限守則+殘留清理先試連;④超限幀先檢查後分配;⑤bench 資訊性禁閾值;⑥新 crate 同輪 REGISTRY 登记。
4. **REGISTRY.md Crates 表兩行**(五列形):`analyzer-core`(職責=統一分析 API+ASP/1 codec;依賴=vl-syntax,ry-ir,rcs-diag;上游角色=—(自建);證據=api+codec_vectors 測試+smoke)、`analyzer-server`(職責=bin analyzerd,UDS 面;依賴=analyzer-core;證據=loopback 集成測試+regress S5)。
5. 若實作輪產生**新鐵則**(如「超限幀先檢查後分配」),按 §10 3-tiers+1 同輪落四層:機械閘(loopback 測試斷言)+SKILL 條文+AGENTS 節+證據頁;無新鐵則則不動 AGENTS.md。

---

## 10. 切片分解(S1..S5;每片帶自驗步驟與驗收標準)

### S1 — `analyzer-core` 骨架 + 統一 API(D6/§4)
- 內容:crate 入 workspace;`view/sv/rtlil` 投影;`analyze` 入口;api.rs 測試。**rcs-diag 零接觸**(R1 撤銷原 line_col 存取器裁量項——`SourceMap::line_col` 早已 pub,source_map.rs:72-74,直通消費;越界判界用既有 `has()` :64)。
- 自驗:`cargo build --workspace`(零警告);`cargo test -p analyzer-core`。
- 驗收:SV 正例 ≥5(**ANSI 繼承腿樣例**:`header port 序列 None 繼承前值;**非 ANSI 回填腿樣例**:`module top(a,b); input a,b; endmodule` 投影 `a/b dir=Input`——探針實證此源合法無診斷、Port.dir 恒 None、方向在 Decl 回填,R2-1;含 ANSI 端口/參數/generate 實例)、RTLIL 正例 ≥2(內嵌字符串源,零新語料文件)投影逐項斷言;SV 語法錯→多診斷+部分模組;RTLIL 壞 .il→單 error+零模組;`instance_edges` 兩輸入形各 ≥1 條正確邊。

### S2 — ASP/1 編解碼層(D2/§5.1-5.3)
- 內容:`proto/json.rs`(Json 類型+寫律+嚴格 reader)、`proto/frame.rs`(u32 前綴讀寫,先檢查後分配);共享向量文件首錄;Rust 側 codec_vectors.rs;TS 側 `src/codec.ts` + `test/codec.test.ts`。
- 自驗:`cargo test -p analyzer-core`(codec 內聯單測+向量);`cd clients/analyzer-ts && bun test`(向量同文件)。
- 驗收:向量 ≥12 條(清單見 §6.3)雙端逐字節一致;reader 禁項 §5.2 全表 9 項雙端同判「拒」(**TS 側禁項執法靠手寫 parser+TextDecoder fatal,§6.2 實現約束 1-2**);i64 邊界向量 TS 斷言走 bigint 路(R1-2);Obj 保序寫出有斷言。

### S3 — `analyzer-server` bin(D3/D4/D5/§5.4-5.9)
- 內容:main + 連線線程 + 握手 + ops 分派 + 錯誤幀 + catch_unwind + 停機序列 + 殘留清理 + CLI 旗標;loopback.rs 集成測試。
- 自驗:`cargo test -p analyzer-server`;手跑 `target/debug/analyzerd --help` / `--version`。
- 驗收:loopback 全項(§8.1 行)綠;oversize 幀=斷連且 server 存活續接新連線;shutdown 後 socket 文件消失且 exit 0;二次啟動對活實例=exit 1、對殘留=重綁成功;路徑 ≥100 字節=exit 1。

### S4 — TS 客戶端 + 端到端冒煙(§6)
- 內容:client.ts/types.ts;smoke.ts(八項斷言);run-smoke.sh。
- 自驗:`bash clients/analyzer-ts/run-smoke.sh`(連跑兩次=冪等);`bun test`。
- 驗收:run-smoke.sh exit 0;八項斷言全過(§6.5);退出碼語義(0/1/2)實測正確;trap 清理後 `ls /tmp | grep analyzerd` 零殘留。

### S5 — 文檔面 + regress S5 塊 + 收口(§9/§8.3)
- 內容:ADR-0011;PLAN 行;SKILL 增補節;REGISTRY 兩行;regress.sh 插塊;資訊性計時數字記檔;本文回填實測數字(把「未執行項聲明」逐條兌換成實跑證據)。
- 自驗:總閘四條全跑(§8.2);`bash scripts/regress.sh` 輸出中既有基線行逐字核對(`SUMMARY ys=469/479`、`LIVE ys=456/456`、S5 前全部塊輸出零漂移)。
- 驗收:四條全綠;既有塊輸出與本弧動工前一致;文檔面五項全落盤;工作樹即產出(零 commit)。

依賴序:S1→S2→S3→S4→S5 嚴格線性(每片依賴前片 API 面);S2 的 TS 側可與 S3 並行(僅依賴 S2 codec 律)。

---

## 11. 顯式不做清單(non-goals)

1. 不做 name resolution / elaborate / 位寬符號推導引擎——SV 查詢集=AST 直讀,未解析行為承襲 vl-syntax 既有面;求值是 riv-elab/vl-sim 既有職責,本弧禁重造。
2. 不做兩 IR 互轉(SV→RTLIL 或 RTLIL→SV)——那是 ryosys Y3/riv 既有前端的職責域,本弧只並列雙入口。
3. 不做增量分析/文件監視/緩存——每請求全量重解析(理由:stateless 是 D3 併發模型的前提;增量=索引服務,記為 v2 open question)。
4. 不做 LSP/DAP——本協議是自有 ASP/1,不背 JSON-RPC。
5. 不做 TCP/TLS/WebSocket/HTTP 面、不做多路復用流/服務端推送——UDS 本機 request/response only。
6. 不做認證/加密——UDS 0o600 文件權限即邊界(§5.8);遠程暴露=非目標。
7. 不引入 serde/serde_json/tokio/libc/criterion/任何新外部依賴(workspace 既有外依賴僅 wasm-bindgen/js-sys,保持)。
8. 不改 vl-syntax / ry-ir / rcs-diag 任何文件(**全弧零接觸**;R2-2 清理 R1 殘留:原尾句「rcs-diag 僅允許 §3.1 聲明的單一加法」引用的是已撤銷的 line_col 存取器條款,與 §3.1「零接觸底線,無例外」矛盾——現三 crate 一律零改動;若實作發現必須改=升級提問)。
9. 不動既有 e2e 語料、SUMMARY 基線(469/479)、known-gap 名單、live 基線(456/456);新語料文件零個(測試源全部內嵌字符串)。
10. 不做 Windows named pipe / 非 darwin-first 平台調優;v1 負載無浮點數(§5.2);SV 表達式文本不進查詢集(AST Expr 無打印機,現造=新面非復用,記為 v2 候補)。
11. 不做 bench 硬閾值斷言(§7);不做進程守護/自動重啟(systemd/launchd 集成=用戶域)。

---

## 12. 風險清單

| # | 風險 | 緩解/觸發動作 |
|---|---|---|
| R1 | ry-ir AST 無 span → RTLIL 側查詢與診斷無位置(§4.2 span 恆 None) | v1 接受並記錄(D10);v2 若需要=ry-ir 加 span 欄位,屬公共類型變更,必走 10.8 欄位存取面普查先行,非本弧 |
| R2 | 手寫 JSON 轉義/數字角與 TS 端不一致(小寫十六進制/astral/控制字符/**i64 邊界**);TS 側禁項(§5.2)被慣用 API 靜默放行 | 向量文件把每個角逐字節釘死(§6.3;向量閘已進 run-smoke 固定步,R1-3);v1 禁浮點消滅最大不一致源;**TS Int=bigint + encode=BigInt.toString(10)、JSON.stringify(number)精度丟真實測在案(R1-2)**;**TS decode=手寫 parser+TextDecoder fatal,JSON.parse/toString('utf8') 放行行為實測在案(R1-7,§6.2 實現約束)**;敵意 fixture 雙端同判 |
| R3 | UDS 104 字節上限:mktemp 的 TMPDIR 過長或用戶自定長路徑 | 默認路徑短;run-smoke 長度守衛 exit 2;server 啟動 ≥100 字節硬錯帶提示(§5.8) |
| R4 | `cargo test --workspace` 增量拖慢總閘 | 新測試純內存+單回環,輪詢帶 5s 硬超時;S5 自驗實測總閘時長並記檔,異常即拆分測試 |
| R5 | panic=unwind 假設未來翻 abort(profile 變更) | catch_unwind 退化为不生效→E_INTERNAL 變斷連;協議仍安全(連接關閉非靜默跑偏);記錄依賴面 |
| R6 | regress.sh 插塊與其它弧工作樹衝突 | S5 塊獨立 if 形、追加式零改既有行;插入點單一(L354 後);工作樹即產出、commit 由用戶定,衝突面最小化 |
| R7 | 沙盒/CI 環境禁 /tmp bind UDS | `--socket`/`ANALYZER_SOCK` 可覆蓋;regress 守衛降級形保證缺 bun 機器綠(可診斷非假綠) |
| R8 | SV `Range.msb/lsb` 為非字面量 Expr → width 派生不出 | parametric 標記(width=0+flag)誠實呈現,不求值(§4.2);TS 型別帶 `parametric?: true` |
| R9 | smoke 集成測試對時間敏感(輪詢/超時) | 輪詢步長 0.1s、上限硬編;超時=測試紅(可診斷);CI 慢機上若紅→調上限非刪閘,變更記錄 |

---

## 13. 附:ASP/1 負載字段字典(analyze result)

```
result: {
  has_errors: bool,
  modules: [ {
    name: str,
    params:  [ { name: str, value: str } ],
    ports:   [ { name: str, dir: "input"|"output"|"inout", width: int, signed: bool,
                 span: Span? } ],
    nets:    [ { name: str, kind: str, width: int, is_port: bool,
                 unpacked_dims: int, span: Span? } ],
    cells:   [ { name: str, ctype: str, port_count: int, span: Span? } ],
    proc_count: int, assign_count: int, span: Span?
  } ],
  diagnostics: [ { severity: "error"|"warning"|"note", code: str, message: str,
                   span: Span?, notes: [ { message: str, span: Span? } ] } ],
  diags_text: str
}
Span = { file: str, start_line: int, start_col: int, end_line: int, end_col: int }
```
- RTLIL 側一切 `span` 缺省(JSON Null;§4.2/風險 R1)。
- `nets[].kind` 詞表:SV 側=DeclKind 既有變體名(net/reg/integer/real/realtime/time/genvar/event);RTLIL 側恆 `"wire"`(Wire 無 kind 概念,`memory` 單列為 nets 條目 kind=`"memory"`、width/size 既有欄位)。

---

## 14. 未執行項聲明(誠實條款;R1 修訂輪更新)

起草輪與 R1 修訂輪均為**只讀+文檔手術**:未跑 `cargo build/test`、未跑 `regress.sh`、未寫任何代碼。**(2026-11-02 S1..S5 實作輪兌換注記:上述「未跑」狀態已全部清償——四閘(build/test/regress/smoke)實跑記錄與逐項證據見 §17 實作記錄;本節其餘為起草輪歷史記錄,不回改。)**
已實跑的驗證(本機命令與輸出,均記於 §2/§5.2/§5.3/§6.2):行號核實(rg/sed 於 rcs-diag/diagnostic.rs/source_map.rs/ast.rs/pp.rs/Cargo.toml);bun 1.4.2 序列化與 parser 放行實測(`JSON.stringify(-9223372036854775808)`→`-9223372036854776000`、`JSON.stringify(bigint)`→TypeError、`BigInt().toString(10)`→精確、`JSON.parse` 放行浮點/指數/重複鍵/越精度整數、`Buffer.toString('utf8')`→U+FFFD、`TextDecoder('utf-8',{fatal:true})`→拋 TypeError)。基線數字引用自 `scripts/regress.sh` 原文(L318/L332-334/L348-349)。§6.3 向量、§4 簽名、§5 協議常量均為**方案值**,以實作輪編譯產物與測試為準;不一致處實作輪必回填本文(本文=SSOT,回填=同輪義務)。

---

## 15. 修訂記錄 R1(2026-11-02 對抗審 13 條消化;每條帶核實證據與修訂位置)

對抗審意見逐條消化,先核實後修訂。high 3 條全部核實為真並已修;medium 6 條全部採納;low 4 條全部採納。

| # | 級 | 對抗審意點 | 核實結果(本輪實跑/讀) | 修訂 |
|---|---|---|---|---|
| 1 | high | line_col 並非私有,`SourceMap::line_col` 已 pub(source_map.rs:72-74);原「唯一例外改動」=E0592,S1 必編譯失敗 | **證實**:`sed -n '60,80p'`+rg fn 清單——pub `line_col` 在 :72-74(返回 `(u32,u32)`),私有者僅 `SourceFile::line_col`(:30),:27 是 doc comment;配套 `has()` :64 亦是 pub | §3.1 例外條款整體撤銷(零接觸底線無例外);§2 事實修正;§4.2 `resolve_span` 改名 `line_col` 直通消費;§8.1/S1 自驗項同步;§3.2/§9.1 無殘留 |
| 2 | high | i64 邊界向量走 TS number 路線必丟真;須釘 Int=bigint | **證實**(bun 1.4.2 實測):`JSON.stringify(-9223372036854775808)`→`-9223372036854776000`;`JSON.stringify(bigint)`→TypeError;`BigInt(…).toString(10)`→`-9223372036854775808` 精確 | §5.3 律 2 補兩端序列化律;§6.2 codec 實現約束 3-4(Int=bigint、禁 stringify);§6.3 i64 向量斷言形改 bigint;S2 驗收同步;R2 更新 |
| 3 | high | TS 半向量斷言(bun test)不在任何固定閘:總閘/regress S5 只跑 run-smoke.sh,而 run-smoke.sh 原文無 bun test 步 | **證實**(方案文本自查:§6.4 腳本原文只有 bun smoke.ts) | §6.4 run-smoke.sh 在 bun smoke.ts 前加 `bun test` 步(exit≠0→set -e 落 1);§0-D7 補「進固定閘」;§8.3 補說明(regress 一層調用即覆蓋,無需雙跑) |
| 4 | med | DiagSink 無 diags()/into_diags(),§4.1 偽碼照抄編譯失敗 | **證實**:diagnostic.rs:62-94——僅 pub 字段 `diags` + new/push/error/warning/error_count/has_errors | §4.1 analyze_sv 偽碼改 `&diags.diags` 借用 / `diags.diags` move,並注記 |
| 5 | med | smoke.ts 只收 SOCK 路徑拿不到 PID/wait,「進程退出碼 0」斷言照做必失敗,應移址 loopback.rs | **證實**(方案文本自查:§6.4 smoke.ts 單參、trap wait `|| true` 不檢驗) | §6.5 斷言 7 改為僅 socket 文件消失;§8.1 loopback.rs 補 `child.wait()` exit 0 斷言;§10-S3 驗收措辭對齊 |
| 6 | med | E_SHUTDOWN 行與 drain 步驟④矛盾(新連線被 drop 不可能收幀 vs 讀作可回幀) | **證實**(方案文本自查) | §5.6/§5.7 寫死仲裁:新連線 drain 窗口=accept 後立即 close 不發幀;E_SHUTDOWN 僅指既有連線到達的請求(shutdown/ping 豁免) |
| 7 | med | TS 嚴格 reader 慣用實現(JSON.parse/toString('utf8'))對禁項全靜默放行,須聲明手寫 parser+TextDecoder fatal | **證實**(bun 1.4.2 實測):`JSON.parse("1.5")`/`"1e5"`/重複鍵/`9007199254740993`→992 全放行;`toString('utf8')`→U+FFFD;`TextDecoder('utf-8',{fatal:true})` 拋 TypeError | §5.2 補 TS 執法約束段(帶實測);§6.2 codec 實現約束 1-2;S2/R2 同步 |
| 8 | med | 非 ANSI 端口 dir/net 繼承語義未進投影規則與驗收(Port.dir/net 是 Option+doc 明載 inherit) | **證實**:ast.rs:94-96 doc「None entries inherit the previous port's」 | §4.2 補繼承規則(last_dir/last_net 狀態、dir/net 各自獨立繼承、無任何顯式 dir=parser 診斷+Inout 佔位);S1 驗收補繼承樣例(正例 ≥3→≥4);§8.1 api.rs 行同步 |
| 9 | low | ADR 文件名 udsp 與協議名 ASP/1 不一致 | **證實**(文頭/§9.1 原文) | 全文統一 `0011-analyzer-service-asp1.md`(文頭+§9.1) |
| 10 | low | workspace.dependencies 15 條 path 實測 16 條 | **證實**:`rg -c 'path = '` Cargo.toml=16(L35-50) | §2 修正為 16 條 + 行號 35-52 |
| 11 | low | AN-E002 無觸發點(include 失敗由 vl-syntax pp 產 VL 碼) | **證實**:analyze 走 `preprocess_source`(內存源),pp.rs:40-46 先例形=pp 內部 VL 碼 | §4.4 撤銷 AN-E002,碼表收斂為 AN-E001 一條 |
| 12 | low | client opts.maxFrame 默認值未定,兩端不一致=server 接受/client 拒收不對稱 | **證實**(§6.2 原文無默認值) | §6.2 釘默認 64 MiB=server 同值+超限對稱律(client 先拒不發幀) |
| 13 | low | loopback.rs 並行測試共享固定 socket 路徑互踩(cargo test 默認並行) | **證實**(cargo test 並行默認=常識,方案原文本未提) | §8.1 loopback.rs 行補每測 tmpdir+隨機後綴唯一短路徑(長度斷言<100) |

修訂總面:§0(D7)/§2(2 處)/§3.1/§4.1/§4.2(3 處)/§4.4/§5.2/§5.3/§5.6/§5.7/§6.2(2 處)/§6.3/§6.4/§6.5/§8.1(2 處)/§8.3/§9.1/§10(S1/S2)/§12(R2)/§14;全文 rg 掃描零殘留引用(`udsp`/`15 條`/`resolve_span` 死名/`AN-E002` 活引用僅存於本節撤銷記錄)。方案主幹(crate 劃分/依賴方向/協議框架/切片序/總閘四條/基線凍結)零變動。

---

## 16. 修訂記錄 R2(2026-11-02 第二輪對抗審 3 條消化;每條帶核實證據與修訂位置)

| # | 級 | 對抗審意點 | 核實結果(本輪實跑/讀) | 修訂 |
|---|---|---|---|---|
| 1 | high | §4.2 繼承規則只有 ANSI 半邊:非 ANSI 端口 Port.dir 恆 None(parser/module.rs:217),dir/net/signed/range 在 ModuleItem::Decl(ast.rs:187-196),須按 DeclEntry.name 回填;`input a,b;` 是**合法源無 parser 診斷**,與 R1-8「無任何顯式 dir=非法源,由 parser 診斷承擔」相反——照原文本實現,S1 驗收樣例投影 dir=Inout 佔位+width 缺省,自家斷言必翻紅 | **證實(探針實跑)**:/tmp 獨立 crate 直呼 vl-syntax 全管線跑 `module top(a, b);\ninput a, b;\nendmodule` → `diag_count=0 has_errors=false`、`ansi=false`、兩個 Port.dir 均 None、模組體 `Decl{kind:Net(Wire), dir:Some(Input), entries:[a,b]}`;行號實證:module.rs:217-225 非 ANSI 分支 Port 全 None、ast.rs:194 doc「Direction when this is a non-ANSI input/output/inout declaration」、module.rs:143-153/:317-320 input/output/inout 兩受理路徑 | §4.2 繼承段整體改寫為**雙腿規則**(ANSI 腿=ports 序 last_dir/last_net 繼承,保留 R1-8 正確半邊;非 ANSI 腿=DeclEntry.name→Decl 映射回填 dir/net/signed/range,帶探針證據;佔位語義收窄至真孤兒端口,撤「非法源」錯誤斷言);§10-S1 驗收正例 ≥4→**≥5**(ANSI 繼承腿+非 ANSI 回填腿兩類樣例,非 ANSI 樣例=探針源);§8.1 api.rs 行同步「ANSI 繼承腿+非 ANSI 回填腿」 |
| 2 | med | §11-8 尾句「rcs-diag 僅允許 §3.1 聲明的單一加法」引用已撤銷的 R1 line_col 條款,與 §3.1「零接觸底線,無例外」矛盾;§15「零殘留」聲明與此不符 | **證實**(§11-8 原文本自查;R1 修訂漏改此處) | §11-8 尾句改為「vl-syntax/ry-ir/rcs-diag 三 crate 一律零改動」並注記 R2-2 清理;§15 為 R1 歷史記錄不回改,本節聲明本次清理 |
| 3 | med | §4.2 `Analysis::line_col(&self, pos: u32)` 無 file 維度:pp include/宏/paste 向同一 SourceMap add 多文件,`SourceMap::line_col` 是 `(FileId, BytePos)` 雙參——單 pos 在含 include 的源上無所屬文件語義,會取錯行列表 | **證實**:pp.rs:406(include `sm.add(cand.display()…)`)、:353(`<macro:…>`)、:650(`<paste>`)三處向同一 sm.add;source_map.rs:72-74 雙參簽名 | §4.2 簽名改 `line_col(&self, file: rcs_diag::FileId, pos: u32)`(帶 FileId,零語義損失直通雙參形;跨進程協議不暴露此查詢);§8.1 api.rs 行同步「include 多文件語義正確」 |

修訂總面:§4.2(2 處:雙腿規則改寫+line_col 簽名)/§8.1(api.rs 行)/§10(S1 驗收)/§11(第 8 條);§16 本節。方案主幹(crate 劃分/依賴方向/協議框架/切片序/總閘四條/基線凍結/R1 全部決策)零變動。R2 消化後全文一致性:rg 掃描 `line_col(&self, pos` 與「僅允許 §3.1」的命中**全部**位於 §16 本記錄與 §4.2/§11-8 的撤銷注記文本——**指令面無殘留**(§4.2 現簽名=雙參帶 FileId、§11-8 現文本=三 crate 一律零改動),不存在 R1 型的兩節指令矛盾。

---

## 17. 實作記錄(每片一行:切片號/改動摘要/自驗結果;以本節為日誌 SSOT)

- **S1**(2026-11-02):`analyzer-core` 入 workspace(根 Cargo.toml members+deps 各一行);新 crate `src/{lib,view,sv,rtlil}.rs` + `tests/api.rs`(12 測試)。裁定記錄:①params 收 header ParamEntry+體內 `parameter` 宣告(localparam 不收=對齊 RTLIL 介面參數語義);②Decl/Instance/proc/assign 走查扁平遞入 generate 區(GenIf/GenFor/GenCase/GenBlock),作用域前綴=新語義不發明;③參數值文本:NumLit→`W'sdD`、StrLit→帶引號、RealLit→原文、他→`<expr>`;RTLIL Const→全 01 位 `N's?h<hex>`(ceil(w/4) 位前導零保留)、含 x/z/m/a → `N'b<逐位字符>`、無預設值參數→空串;④實例邊=SV 一 InstUnit 一條/RTLIL 一 cell 一條,名字逐字(帶 `\`);⑤nets=體內 Decl 面(ANSI 頭端口聲明非體內 Decl,不進 nets);⑥ANSI 組規格省略範圍依 LRM §12.3.3 繼承(parser 既有行為,投影如實呈現——`output d` 承接前組 `[3:0]` 寬 4);⑦RTLIL 空輸入=空 design(上游 `ry-ir/src/lexer.rs:81` 對字面空串 index panic,ry-ir 零接觸,守衛落本層;oracle read_rtlil 空檔=空 design 同義);⑧RyError→AN-E001 span 指錯誤行首。自驗:`cargo build --workspace` 零 error 零 warning;`cargo test -p analyzer-core` 12/12 PASS。
- **S2**(2026-11-02):`analyzer-core/src/proto/{mod,json,frame}.rs`(Json 子集+幀層)+ `tests/codec_vectors.rs`(25 向量 Rust 側斷言+#[ignore] 生成器)+ TS 側 `clients/analyzer-ts/src/codec.ts`/`test/codec.test.ts`(bun test 12 測試讀同一 fixtures)。裁定記錄:①TS 對象表示=JObj 包裝類(偏差於 §6.2 原文 `[string,J][]` 聯合型:原形無法區分空對象與空數組,ping `{}` args 無法表示);②reader 增巢狀深度上限 128(防棧溢出,自建契約);③字面孤代理字符在 parse 文本面也拒(Rust/TS 同);④TS 孤代理經 TextEncoder 是無聲 U+FFFD 替換(平臺事實),敵意孤代理測試走原始字節構幀路徑。自驗:`cargo test -p analyzer-core` 34 pass/0 fail(json 17+api 13+codec_vectors 4+1 ignored 生成器;2026-11-03 收平輪刷新——原錄 36 與分解 17+12+3=32 自相矛盾,係 S3 增第 4 個 codec_vectors 後未重測,終審釐正時點真值 33+1,見 §17 收平條);`cd clients/analyzer-ts && bun test` 12/12 pass(向量 canonical/reject 四類雙端同判);`cargo build --workspace` 零警告。fixtures=25 條(canonical 11/reject_codec 11/reject_frame 2/reject_utf8 1),生成器首錄、雙端斷言維持。
- **S3**(2026-11-02):`analyzer-server` crate(bin `analyzerd`,src/{main,conn}.rs)+ `tests/loopback.rs`(10 測試)。core 增 `src/report.rs`(Analysis→§13 Json 編碼,span 消費 pub(crate) sm;10.7 core-first)。裁定記錄:①accept 循環從頭即 non-blocking+5ms 輪詢(plan §5.7③ 的「shutdown 時轉 non-blocking」等價前移,std 無法喚醒已阻塞的 accept);②**macOS accepted fd 繼承 listener O_NONBLOCK 實錄**——conn 線程首讀 EAGAIN(os error 35)誤判 Eof 斷連=client EPIPE,修法=accept 後 `set_nonblocking(false)` 強制阻塞(sample 看棧定位,鐵則 §4);③信封壞形(id 缺/非 Int 零、op 非 Str、args 非 Obj)=E_ARGS 連線保持(非 E_PROTO 關閉;可解碼 Obj 的語義層錯誤走可恢復路);④幀級違約錯誤幀 id=0(請求 id 不可知);⑤**drain 窗口新 client 的機械觀測面=connect 失敗**(§5.7② unlink 先於 ③置位,新 connect 必 ENOENT;「accept 後立即 close」僅覆蓋 unlink 瞬間 backlog 競態連線,兩路皆 0 幀=無歧義,§5.6 仲裁保留);⑥analyze_response 向量改由**真編碼器**產出(生成器直呼 report::analysis_result+穩定性測試釘 stale=重錄收據);⑦測試腳手架鐵則:無 shutdown 觸發的 server 收尾必 Guard Drop(kill+wait),`wait()` 直呼=掛死實錄(eversion 測試,§4 sample 診斷)。自驗:`cargo test -p analyzer-server` 10/10 PASS(連跑 5 輪零 flake);`cargo build --workspace` 零警告;手跑 `analyzerd --version`/`--help` 輸出逐字核對。
- **S4**(2026-11-02):`clients/analyzer-ts/src/{types,client}.ts` + `smoke.ts`(八項斷言)+ `run-smoke.sh`(bun test 向量閘內建步)。裁定記錄:①client 內建握手(connect 即發 hello 並核 proto=1,`serverInfo` 欄位載 server 檔;方法面維持 §6.2 五方法,hello 不外露);②E_OP/E_ARGS 斷言走 smoke 內 raw 幀通道(直呼 codec,不擴 client 公共面);③client 超限對稱律落地=request() 編碼後寫出前檢查,拋 RangeError 不發幀(R1-12);④loopback 測試 tmpdir 清理上收 Guard Drop(Child,dir),stale 測試以 `std::mem::forget` 保殘留檔受試物。自驗:`bash clients/analyzer-ts/run-smoke.sh` exit 0(連跑兩次冪等);退出碼實測:無 bun PATH→2;殘留實測:`ls /tmp | grep analyzerd`=0、loopback tmpdir=0、無孤兒 analyzerd 進程;`cargo test -p analyzer-server` 10/10×3 輪。

- **S5**(2026-11-02):文檔面五項+regress S5 塊+總閘收口。落地:①ADR-0011(D1..D9+後果);②PLAN §1 P3.7 條目+§5 里程碑表 AN1..AN5 行;③SKILL「分析器服務增補」節(八條:core-first/編解碼律/UDS 路徑/先檢查後分配/macOS O_NONBLOCK 實錄/腳手架三律/bench 禁閾值/REGISTRY 同輪);④REGISTRY Crates 表兩行;⑤regress.sh S5 塊(插 ryosys 塊後,雙守衛形,既有塊零改動)。總閘四條實測(build 0.04s 熱緩存/cargo test --workspace 15.5s/regress 408.9s[既有塊全量;S5 新塊增額≈1-2s]/run-smoke 0.2s):`cargo build --workspace` 零 error 零 warning;`cargo test --workspace` 82 個 test-result 行全 ok、零 FAILED(2026-11-03 收平輪複核:本輪新增測試函數不增測試目標,82 仍準確);`bash scripts/regress.sh` exit 0=REGRESSION GATE: PASS,基線逐字(`469/479`+`456/456`+known-gap 10 例名單 grep 維持)+`analyzer smoke: PASS`;`bash clients/analyzer-ts/run-smoke.sh` exit 0(八項斷言+向量閘)。資訊性計時(D8,一次 ad-hoc 數字入檔):smoke 內 connect+hello 8ms/ping 0ms/sv analyze 1ms/rtlil analyze 0ms/兩連線並發 15ms(bun 1.4.2, M 系列本機)。附記:docs/e2e-report/index.html 由 regress 的報告再生成步刷新(內嵌 git HEAD 01fe588→167878d,機械 churn 非手改,regress 裁定=非失敗 note;提交與否用戶定)。工作樹即產出,零 commit。

- **收平輪**(2026-11-03,終審三實質項消化;計數一律當輪實跑)。①**lit_u64 下劃線 bug**:`sv.rs lit_u64` 對 based 字面量直接 `from_str_radix(digits)`,而 lexer 保留源碼下劃線(U-3b parity;lexer.rs:「every value parser filters '_' itself」——本層即 value parser 卻未濾)→ `8'h0_F` Err 誤入 parametric(width=0+parametric=true),等價形 `8'h0F` width=16,實錄復現。修=求值前 `filter(|&c| c != '_')`;x/z/? 剝後仍 Err → parametric 零誤傷;全下劃線剝後空 → None 不 panic。回歸=`tests/api.rs based_literal_underscore_equivalence`(d/h/o/b 四基等價對+x/z/? 兩形+全下劃線形)。②**socket 優先級繞過 env**:`set_max_frame` 無 `--socket` 時硬編碼 DEFAULT_SOCKET 造出 Some(Config),`run()` 的 unwrap_or_else 不再查 env → 單獨 `--max-frame` 繞過 `ANALYZER_SOCK`(實錄復現:env 指 /tmp/env-prec-test.sock 仍落 /tmp/analyzerd.sock)。修=`Config` 兩欄皆 Option、解析點收斂為 `resolve_sock_path(cli, env)`(--socket > ANALYZER_SOCK > 默認;env 注入式,單測零進程環境依賴,並行安全);回歸=`main.rs` 單測 3(resolve 三象限/`--max-frame` 單獨給時 sock_path 必 None 兩語法形/雙旗標獨立+鉗位)+`loopback.rs` 集成 2(真二進制+真 env:env 勝默認回環/CLI 勝 env 且 env 路徑零產物;踩坑一次=新測直發 ping 被首幀必 hello 契約拒 E_VERSION,補握手 helper)。③**計數釐正**:原 S2 行 36 與其自身分解 17+12+3=32 自相矛盾(S3 增第 4 個 codec_vectors 後未重測,終審實測釐正時點真值 33+1);S2/S5/PLAN AN1..AN5 行三處統一刷新為本輪實測(S5 行 82 test-result 行複核不變——新增測試函數不增測試目標)。**冒煙閘收編複核**:regress.sh S5 analyzer 塊(雙守衛形,crate+bun 在場判定,exit≠0 整閘紅)已於 S5 弧落地,本輪確認在執法面實跑 PASS;立法面普查=REGISTRY Crates 表 analyzer-server 行「regress S5 `analyzer smoke` 塊」+ADR-0011 D9 已載,**AGENTS §10.7 機械閘列舉+SKILL 分析器增補節本輪補條目**。自驗(當輪實跑):`cargo test -p analyzer-core` 34 pass+1 ignored(json 17+api 13+codec_vectors 4);`cargo test -p analyzer-server` 15 pass(單測 3+回環 12);`cargo test --workspace` 82 行全 ok 零 FAILED;`bash scripts/regress.sh` 全綠 PASS,基線 469/479+456/456 逐字不動。工作樹即產出,零 commit。
