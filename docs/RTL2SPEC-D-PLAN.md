# RTL2SPEC-D-PLAN — D「spec 前端」弧實施規劃(CLI-AGENT 憲法首弧;D-S0..D-S6)

- 日期:2026-11-08(R1 修訂同日;R2 修訂同日,見文末修訂記錄)。
  性質:**獨立小檔**(審查員只讀本檔+它引用的代碼行,禁通讀 63KB 正典;
  RTL2SPEC-FINISH-PLAN 同款)。收尾切片(D-S6)一次性把 §21 寫進 RTL2SPEC-PLUGIN.md
  (append-only,§0..§20 零刪改),本檔不作為正典。
- 範圍=①S0=D-c3 修復(corpus_lint 同行 `#` 關窗+雙 fixture 判別力);②D-S1..D-S6=
  MVP-NEXT R5「D spec 前端(人寫高級 spec→agent 產 RTL)」:新 rtl-spec bin 為 agent
  唯一接口+spec 檔格式+機械檢查+與 conformance 評測床的接線。
- 數字來源標註:【勘察】=開弧勘察材料;【實讀】=本規劃輪讀檔取證;【實跑】=本規劃輪
  實跑;規劃輪**未實跑任何閘**(S0 未實作),閘判據=設計值,實施弧首跑取證。
- R5 時點判據開弧資格(兩道均已過):①「S5 之後」已過——S5=2026-11-05 落地
  (commit 4922352;run-reimpl-eval 23 場景);②「C 全量化之後」已過——2026-11-07 C 閉環
  弧落地(RTL2SPEC-PLUGIN.md:1999 §19 標題;診斷回灌 F3 閘 REGISTRY.md:270-273 PASS)。
  ③R5 主理由「E1 表述質量未達標(acc_pipe 0/6)」已除——五死因 N1..N5 由 S1..S3 修復
  (contract 節+decl_lint=crates/rtl-spec/src/lint.rs 在庫);對拍數字【實跑】verdict.json
  legs 欄:acc-A0 0/6 → acc-A1/A2/A3=6/6/6/6;i2c-A0 compiled=false → i2c-A1/A2/A3=
  compiled=true 11/11;i2c-B1=11/11、acc-B1=6/6。判據與 MVP-NEXT.md:231/:247/:358-360 一致。
- 殘留注意項(不阻 D,落地時須知):D-c4 contract 負數下標報 parametric 未修
  (REGISTRY.md:300-303,安全側拒收);盲寫腿誠實邊界=REIMPL-PROTOCOL §0「盲隔離弱於
  E1 原始輪,數字僅輔助證據」;D-c7 未裁(REGISTRY.md D-c7 條),D 弧不觸等價腿。

## 0. 邊界(全程約束;違反即停弧)

1. **CLI-AGENT 憲法**(用戶 2026-11-08 定調,本弧最高設計約束;詳 §2)。
2. LLM 腿永不進確定性閘(10.11/10.12):盲寫 spec→RTL 的採樣過程(prompt/模型/參數)=
   輔助證據;確定性閘判據=「凍結 top.v 這個工件」的機械可測屬性(編譯成敗+公共 TB
   逐場景 diff+變異負探針)。
3. 語料唯讀+in-git:spec 語料自著 MIT 入 `plugin/rtl2spec/fixtures/d-spec/`;外部語料
   禁入 plugin/(10.10);zipcpu=GPL 永不入。
4. golden/gap-fixtures/EXPECTED_*/SERVER_VERSION 隨語義面同輪上調禁靜默(10.5/10.8);
   golden 重錄逐案對賬;**wasm sha 變更=conformance 全量重錄語義事件**
   (conformance-eval.mjs:911 `prev.provenance.wasmSha256 !== wasmSha → DRIFT 紅`,【實讀】)。
5. 新能力先落 rtl-spec 核心,CLI/wasm/MCP 三面跟隨(10.7/10.10),禁只在單一面實現;
   禁在 Rust 側重實現 check-rulebook.mjs/conformance-eval.mjs 的判據(雙實現漂移;
   JS checker 的獨立重算地位=鎖③,保留)。
6. 新閘必配負向探針實跑必紅;弧內禁 git commit;計數一律實測;
   閘跑不過或指令矛盾如實升級。
7. D 弧無 oracle(spec 提取無 golden 契約沿 ADR-0012 D2/D5):質量閘=機械判據+盲寫數字
   輔助,**禁虛構 golden、禁把盲寫數字包裝成結果達標**(MVP-NEXT §4.2 兩層分寫條款沿用)。

## 1. 基線(現場取證,全部【實讀】除標註者)

- crates/rtl-spec 無 bin 目標:Cargo.toml 無 `[[bin]]`;src/=expr/json/lint/md/rtlil/schema/
  summary/sv/tbplan;native 面唯一出口=examples/emit.rs(S4 冒煙對拍形,非安裝型 CLI)。
  核心 API=crates/rtl-spec/src/lib.rs:34-68(extract_rules/scan_gaps/analyze_summary/
  rulebook_json/tb_plan/…);wasm 面=crates/rtl-spec-wasm/src/lib.rs:21-94(rtl_analyze_sv/
  rtl_extract_rules_sv/rtl_scan_gaps_sv/rtl_tb_plan_sv+rtlil 對應,零 js_sys 字串進出);
  MCP 面=五工具 rtl_analyze/rtl_extract_rules/rtl_scan_gaps/rtl_tb_plan/rtl_extract_batch
  (dist/server.mjs,SERVER_VERSION='0.5.0')【勘察】;skill 條文四節(plugin/rtl2spec/skills/)。
- CLI bin 先例=crates/vl-cli(lib.rs `run(&args)->i32` + 薄 main.rs `std::process::exit`)。
- conformance 評測床:run-conformance-eval.sh 直調不進 regress(10.12);staging 形=
  conformance-eval.mjs:63-77(`--fixtures-root` 只收 build/ 內目錄,ADMITTED 只看
  staging verdict.json)【勘察】;reimpl-eval 腿表=reimpl-eval.mjs:98-110 凍結 LEGS
  (12 腿;TB=fixtures/e1/tb_{counter8,acc_pipe,i2c}.v;場景 6/6/11=23;iverilog 缺場 exit 2、
  盲寫產物缺 exit 3、寫只落 build/)【實讀】;場景切塊=tbproto.sceneBlocks
  (reimpl-eval.mjs:97 註釋)。
- TB 可判別力先例:reimpl-eval verdict.json acc-A0 0/6 vs acc-A1 6/6(同 TB 同場景協議下
  分出盲寫版本差異)=D 評測床採用同 TB 的直接依據【實跑】。
- 公共 TB 場景名單【實讀】:tb_counter8.v SCENE1..6(power_on_no_reset/sync_reset/
  count_after_release/mid_count_reset/wrap/hold_reset_then_count);tb_acc_pipe.v SCENE1..6
  (power_on_x/steady_5_7/carry_200_100/max_255_255/dynamic_stream/zero_converge);
  tb_i2c.v SCENE1..11(reset_idle/start/wr_55/wr_aa/rd_released/restart/
  wr_0f_after_restart/stop/queue_start_during_tip/mid_transfer_reset/idle_after_stop)。
  接口簽名【實讀】:counter8 無 wrapper(TB 直實例 `counter #(.WIDTH(8))`,tb_counter8.v:11);
  acc_pipe:TB 實例化 wrapper `dut_accpipe dut_i`(tb_acc_pipe.v:14),wrapper 模組
  `dut_accpipe`(fixtures/e1/dut_orig_accpipe.v:3)內包 DUT 模組 `__acc_pipe__acc_pipe`(:9);
  i2c:TB 實例化 `dut_i2c dut`(tb_i2c.v:30),wrapper `dut_i2c`(dut_orig_i2c.v:3,17 端口)
  內包 `i2c_master u`(:22);wrapperO=原版與盲寫版共用(reimpl-eval.mjs:120)。
- D-c3 現場【實讀】:crates/vl-difftest/tests/corpus_lint.rs:73-79 同行 tail 在首個
  `#`/`@(` 處切 cut;:80-95 `closed` 為空操作(:83-85 空循環+:89 自賦值+:91-94
  `if closed {}` 空體);:97-107 跨行窗口擴展循環**不受 closed 影響繼續 push**——
  同行 `#1;` 已時序隔離的後續行寫入被誤捕=假陽性機制。既有測試
  race_lint_catches_the_documented_pattern(:174-182)good fixture 同行寫後緊跟 `@(` 行,
  擴展循環當場 break,測的是「不誤報緊鄰形」,蓋不住換行形假陽性(REGISTRY.md:295-299 同結論)。
- regress 收編落點=scripts/regress.sh:368-394「S7 rtl2spec 插件閘」塊(雙守衛降級形:
  pkg+bun 在場才跑 mcp-e2e/rtl-validation/golden/gap-fixtures,缺場 SKIP)【實讀】。

## 2. CLI-AGENT 憲法(→ ADR-0014;D-S6 立法)

用戶 2026-11-08 定調四條,本弧一切面設計按它裁:

1. **以 agent 使用行為優化**:CLI 的消費者是 agent 不是人——JSON-first 輸出(stdout=
   機器可讀單一 JSON;人讀診斷走 stderr);穩定 exit code 約定(0=成功;1=域內失敗
   (findings/被測紅/驗證不過);2=用法或環境缺(守衛缺場/參數誤);3=輸入檔不存在);
   錯誤訊息自帶下一步可執行提示(例:`spec lint failed: 2 findings — fix B007 unknown
   identifier 'cnt_ed' or run: rtl-spec spec lint --explain B007`)。
2. **self-contain**:操作知識內嵌於 CLI 自身——`rtl-spec --help` 頂部攜完整工作流配方
   (spec 撰寫→盲寫 RTL→`spec lint`→`extract` 對拍→`tb` 生成→conformance 評測迴路,
   逐步全部以 rtl-spec 子命令寫出);每子命令 `--help` 自解釋;用 CLI 不需要再讀任何文檔。
3. **有了 CLI 就什麼都有**:extract/validate/tb/conformance/diagnose 全能力經 CLI 子命令
   可達;scripts/*.mjs 只是閘的內部件,不是 agent 接口。conformance/diagnose 子命令=
   **委派形**(spawn 既有 runner,exit code 原樣轉發+JSON 封套 `{cmd,exit,report}`)——
   禁在 Rust 側重實現 Q1..Q9(§0.5 雙實現漂移)。
4. **core-first 三面跟隨**:新能力(spec 格式/解析/lint)先落 crates/rtl-spec 核心,
   CLI/wasm/MCP 三面同弧跟隨;禁只在單一面實現。

## 3. spec 檔格式契約(D-S1 落地;本弧資料層釘檔)

**格式=markdown 節頭逐字**(人寫、agent 讀、機械可驗)。三正樣已自著入庫(MIT):
`plugin/rtl2spec/fixtures/d-spec/counter8.spec.md`(2 條)、`acc_pipe.spec.md`(9 條)、
`i2c_master.spec.md`(B001..B094=94 條)——檔即 blindSample(§6),亦即 S1 解析器正樣語料。

```
# spec: <design>
- spec-format: 1                      ← 必在,且值=1
- module: <DUT 側模組名>              ← 見下「module 契約」
- param: <NAME> = <expr>              ← 0+ 行
- port: <name> <input|output> <wire|reg> <scalar|[N:0]|[PARAM-1:0]>   ← 1+ 行
- state: <name> <width> <一句角色>     ← 0+ 行(抽象時序狀態;盲寫方自行落地實現)
- derived: <name> <width> <一句角色>   ← 0+ 行(組合中間量聲明通道;R1-1 增)
- assumes: <一句>                      ← 0+ 行(G 類破口 assumptions 通道)

## behavior                            ← 必在,≥1 條
- B001 [<category>] <陳述>
  check(<form>): <結構行>              ← 文法 SSOT=本代碼塊+三正樣所示形(見下)

## scenarios                           ← 必在,≥1 條
- SCENE1 <name>: <一句行為契約>
```

- **module 契約(R1-2)**:`module`=harness 編譯時與盲寫 top.v 同輪連結的 **DUT 側模組名**,
  **不是** TB 實例名。有 wrapper 腿:TB 實例化 wrapper(acc_pipe=`dut_accpipe`、
  i2c=`dut_i2c`,§1),wrapperO 內包 spec.module(`__acc_pipe__acc_pipe`/`i2c_master`),
  盲寫 top.v 與 wrapperO 同輪編譯;無 wrapper 腿(counter8):TB 直實例 spec.module
  (`counter`)。legs.json 接線按此釘(§4 D-S4),禁按 TB 實例名棄用 wrapperO。
- **行級文法(R1-4;三正樣實際用到的全部形)**:①`#`/`##`=節頭(`## behavior`/
  `## scenarios` 逐字必在);②`###` 子節頭=人讀分組,解析器跳過零語義
  (i2c_master.spec.md 9 處,如 :51/:94);③節內非 `- `、非縮進、非節頭的行=散文段,
  跳過零語義(acc_pipe.spec.md:20-22、i2c_master.spec.md:46-48);④`- ` 起新條目;
  其後**以空白縮進**的連續行屬該條目——縮進行 trim 後以 `check(` 起首者=該條目唯一
  結構行(behavior 條目恰 1 行,scenario 條目 0 行),其餘縮進行=續行併入條目陳述
  (acc_pipe.spec.md:50 SCENE5 續行、i2c_master.spec.md:130/:160 等多處);⑤空行=條目界。
  behavior 條目結構行數≠1=lint ⑤ finding。
- **check 行五形與文法 SSOT(R1-3)**:form 詞表=crates/rtl-spec/src/schema.rs:125-129/:146;
  **spec check 行文法 SSOT=本節代碼塊+三正樣所示形**,形如
  `guard: trigger=<sig> <edge>; when=<expr>; async=<true|false>; effects=<sig>←<expr>` /
  `transition: <reg>: <V1> → <V2> when <expr>` / `sequence: +<N>拍 → <sig>=<expr> when <expr>`
  (**無 anchor 欄**)/ `relation: <lhs> ≡ <rhs> when <expr>`(無條件 `when -`)/
  `invariant`(D 弧不收,parse 遇之=finding,待有語料再立文法);條件連接詞 `且`。
  **兩個渲染面不可互推**:md.rs:188-197 規則書發射器的 sequence 臂(格式串 :195)帶
  `anchor(…)` 前綴(golden 釘死的規則書渲染面);reimpl-eval.mjs:57 的 trim writer 產無
  anchor 形(build/reimpl-eval/rulebook-trimmed-*.md)——spec 文法與後者同形,但 SSOT 是本節,
  md.rs/trim writer 後續任何變動**不溯及** spec 文法(改 spec 文法=ADR-0014 修訂事件)。
- **機械檢查(spec_lint 七查,S1)**:①spec-format 行在且=1;②module 非空;
  ③port ≥1 且 dir/type 詞表;④behavior 條目 id 唯一(B\d+);⑤check.form∈四收形
  (guard/transition/sequence/relation)且 form 專屬欄位非空、每 behavior 條目結構行
  恰 1(鎖②詞表重用);⑥**decl_lint:結構行值側引用的標識符 ⊆
  ports∪params∪states∪derived**(字典序輸出)——**只掃值側(R1-1)**:guard 的 trigger
  信號名+when 表達式+effects 每項 `←` 兩側;transition 的 reg 名+V1/V2+when;sequence
  的 sig+`=` 右側+when;relation 的 ≡ 兩側+when;**排除**欄位鍵(trigger/when/async/
  effects)、`check`、五形名、edge 詞(posedge/negedge)、async 值 `true`/`false`、
  `-` 佔位、數值字面量(`16'd1` 等)與 lint.rs:23-38 IEEE 關鍵字表(該表無 true/false,
  故排除表須顯式列此二詞);這是 E1「盲寫方免猜聲明」教訓在 spec 側的對偶:spec 不
  宣告的標識符=盲寫必猜=紅燈;⑦scenarios ≥1 且 SCENE<n> id 唯一。負樣 fixtures 以
  測試內聯字串承載(七查各一條會觸發形+零誤報形;⑥零誤報形必含 `async=true` guard 行
  與 derived 標識符引用,三正樣即真實零誤報語料:20×async=true+1×async=false、
  i2c 22 條結構行引用 cnt_end【實跑 grep】)。
- **spec→rulebook 不做自動橋**:rulebook=RTL 提取產物(golden 釘死),spec=意圖輸入,
  兩者經「`rtl-spec extract` 盲寫 RTL 後的輸出 vs spec」人工/agent 對讀,禁虛構自動
  匹配判據(無 oracle)。D 的行為驗證=評測床 TB 場景 diff(§5),不是規則對齊。

## 4. 切片(D-S0..D-S6;每片獨立可驗收,禁攢批)

### D-S0 = D-c3 修復(corpus_lint 同行 `#` 關窗;雙 fixture 判別力鐵律)

- 手術:crates/vl-difftest/tests/corpus_lint.rs:80-95 死代碼刪除(空循環/自賦值/空 if
  整段移除,10.6 手術紀律:刪除前印出被刪文本);`:97` 起擴展循環包進
  `if cut == tail.len() { … }`(=同行 `#`/`@(` 關窗;同行 `#` 前語句仍在窗內:
  `window.push_str(&tail[..cut])` 不動,既有 bad 形 `@(posedge clk); ld = 1;` 仍紅)。
- 雙 fixture(判別力鐵律,數字實測入 commit message):
  正樣=`@(posedge clk); #1; ld = 1;` 後**仍有後續語句行**(`q = 3;` 行,ld/q 均為
  edge-read var)再遇 `@(posedge clk)` 行 → findings 必空(修前此形=1 假陽性,【實勘】
  REGISTRY 條+勘察材料一致);負樣=窗內競爭形(同行 `@(posedge clk)` 後無 `#` 直接寫,
  或後續行窗內寫)→ findings 必 ≥1。既有三測試(既有 bad=1 finding/nba 零/good 緊鄰形零)
  不得 regress。
- 同輪:REGISTRY.md D-c3 條改「已清償(D-S0,2026-11-08)+證據指針」。
- 閘:`cargo test -p vl-difftest --test corpus_lint` 全綠(新舊≥4 測試)+`cargo build`
  零 error 零 warning;`bash scripts/regress.sh` 基線零擾動(corpus 現狀零違例,
  REGISTRY.md:297 原文)。
- 負向探針:臨時還原關窗(把 `if cut == tail.len()` 改回無條件)→ 新正樣測試必紅,
  實跑留證後還原。

### D-S1 = spec 格式核心(crates/rtl-spec/src/spec.rs)

- 新模組 spec.rs:`SpecDoc`(module/params/ports/states/derived/assumes/behavior/scenarios)+
  `parse_spec_md(&str) -> Result<SpecDoc, SpecError>`(永不 panic,錯誤進結果,沿 crate
  姿態註釋;行級文法=§3 R1-4 五條)+`spec_lint(&SpecDoc) -> Vec<String>`(§3 七查,
  ⑥只掃值側)+`spec_json(&SpecDoc) -> String`
  (復用 crate json.rs 寫出器)。lib.rs 增 `pub mod spec;`(additive,零既有讀取面改動,
  10.8 普查=新模組無讀取面)。
- 測試 crates/rtl-spec/tests/spec_tests.rs:三正樣 fixtures/d-spec/*.spec.md 各 parse 出
  2/9/94 條+零 findings+ports/params/states/derived 逐欄位對拍(i2c derived=1:cnt_end);
  散文段/`###` 子節頭/續行三形各有斷言(散文不成條目、續行併入陳述、結構行恰 1);
  七查負樣(內聯字串)各觸發恰好對應 finding+零誤報形(parametric width `[WIDTH-1:0]`、
  `async=true`、derived 引用皆為合法形,不誤報)。
- 文法出入協議(R1-3 改):S1 解析器若與三正樣有出入,**以 §3 文法為準修解析器**;
  §3 文法本身確有缺陷時,先改本檔 §3+ADR-0014 草案再同輪改 fixtures(diff 對帳,
  禁悄悄改語義);md.rs 發射形永不作為 fixture 改寫依據。
- 閘:`cargo test -p rtl-spec` 全綠+`cargo build` 零 warning;pkg 重建
  (`wasm-pack build crates/rtl-spec-wasm --target web --out-dir pkg`+同步 dist/wasm——
  **sha 將變**,conformance DRIFT 鏈由 D-S3 統一清償,S1 只保證 regress 內閘綠)+
  `bash plugin/rtl2spec/run-golden-regression.sh` PASS 98/98(提取器零改=期望零漂移證明)。

### D-S2 = rtl-spec bin 骨架(agent 唯一接口;四能力子命令)

- crates/rtl-spec/Cargo.toml 增 `[[bin]] name = "rtl-spec" path = "src/bin/rtl-spec.rs"`;
  src/bin/rtl-spec.rs=薄 main(`std::process::exit(rtl_spec::cli::run(&args))`,vl-cli 同款);
  lib.rs 增 `#[cfg(not(target_arch = "wasm32"))] pub mod cli;`(host-only,wasm 構建不含;
  wasm-pack 只建 lib,bin 不入 wasm 面)。
- 子命令(能力已在核心,僅補 native 面=10.7 面跟隨,非新能力):
  `analyze <sv>` / `extract <sv> [--lang sv|rtlil] [--max-rules N] [--format json|md]` /
  `gaps <sv>` / `tb <sv> --module M [--seed S --cycles C --flush F] [--emit-v]`。
  data 子命令 stdout=**raw JSON**(與 wasm 導出 byte-equal——parity 閘的設計依據);
  `--emit-v` 走 tbplan::tb_template_v 寫檔需顯式 `--out <path>`(默認零 FS 寫)。
- 憲法 ①②落地:exit code 約定(§2.1 四值);`--help` 頂部工作流配方(§2.2);
  每子命令自解釋;錯誤訊息自帶下一步提示。
- 閘:`cargo test -p rtl-spec`(cli 模組測試:exit code 四值/JSON 輸出形/--help 含
  配方關鍵字)+**新增閘** `bash plugin/rtl2spec/run-spec-cli-parity.sh`(§5.1)。
- run-mcp-e2e/pkg fresh 不受影響(bin 不入 wasm 產物)。

### D-S3 = spec 三面跟隨(CLI spec 子命令+wasm 導出+MCP 工具+conformance 重錄)

- cli.rs 增 `spec parse <spec.md>`(SpecDoc JSON)與 `spec lint <spec.md>`(findings JSON;
  exit 0=clean/1=findings,stdout 仍出 findings JSON 供 agent 解析)。
- crates/rtl-spec-wasm/src/lib.rs 增 `rtl_spec_spec_parse(name, source) -> String`、
  `rtl_spec_spec_lint(name, source) -> String`(字串進出零 js_sys,沿既有導出形)。
  wasm smoke 測試(crates/rtl-spec-wasm/tests 若無則 lib 內 #[cfg(test)]…沿現形)擴兩導出。
- dist/server.mjs 增工具 `rtl_spec_lint`(收 {name, source} → 裝載 wasm → 編幀;
  零語義,沿五工具形);SERVER_VERSION 0.5.0→0.6.0,plugin.json/CHANGELOG 同輪一致
  (semver 同步律);run-mcp-e2e.sh 斷言數 19→21(新工具正負兩斷言,實作輪按 PASS 行數
  實測定案)。
- **wasm sha 變更清償**(§0.4;conformance-eval.mjs:911 DRIFT 鏈):conformance 全量重錄
  `run-conformance-eval.sh --record`(G0..G5 標準程序:錄前快照 build/conformance-current、
  `--record` 後逐案對賬——**既有 34 設計剝 tb.simMs 後須逐位元組一致,唯一允許 diff=
  provenance.wasmSha256**;任何其他 diff=停弧排查)。
- 閘:`bash plugin/rtl2spec/run-mcp-e2e.sh` PASS(21 斷言)+重錄後
  `bash plugin/rtl2spec/run-conformance-eval.sh` PASS 34/34+`cargo test -p rtl-spec -p
  rtl-spec-wasm`+golden PASS 98/98+`bash plugin/rtl2spec/run-spec-cli-parity.sh` 擴 spec
  檔腿(CLI spec lint vs wasm rtl_spec_spec_lint byte-equal)。

### D-S4 = 評測床接線(run-spec-eval.sh + scripts/spec-eval.mjs)

- 形沿 run-reimpl-eval.sh/reimpl-eval.mjs(§1 基線):腿表=
  `plugin/rtl2spec/fixtures/d-spec/legs.json`(斷言規格,手寫合法):每腿
  {leg, design, spec, tb, origDut, reimplDut, wrapperO?, scenes, expected:{compiled,matched}}。
  **接線契約(R1-2)**:編譯單元=tb+wrapperO(若有)+DUT 檔;DUT 檔頂模組名必==spec.module;
  acc_pipe/i2c 腿 wrapperO 必填(`e1/dut_orig_accpipe.v`/`e1/dut_orig_i2c.v`,原版與盲寫版
  共用,reimpl-eval.mjs:120 先例),counter8 腿無 wrapperO(TB 直實例 `counter`);
  runner 啟動斷言「DUT 檔含 `module <spec.module>`」不過即 exit 1(錯誤帶 spec.module 與
  wrapperO 路徑=憲法 ①下一步提示);orig DUT 頂模組【實讀】counter.v:5/acc_pipe.v:2/
  i2c_master.v:27 皆==spec.module,斷言對三腿成立。
  首批腿=自對拍腿(orig vs orig,證 runner 判別力+TB 場景解析正確):
  counter8/acc_pipe/i2c_master 三腿,expected matched=6/6/11 全綠。
- 機械判據=iverilog 編譯成敗+tbproto.sceneBlocks 逐場景塊 diff(語義逐字移植,沿
  reimpl-eval.mjs:93-97 註釋同款);寫只落 `plugin/rtl2spec/build/spec-eval/`(普通運行;
  `--record --slice <D-Sn>` 才寫已提交工件,沿 conformance 同形);iverilog
  缺場 exit 2、腿產物缺 exit 3;**直調不進 regress**(D-S6 收編時配守衛,見 §5.2)。
- **負向探針(判別力自證)**:對每腿凍結 top.v 施一確定性語義變異(復用
  scripts/conformance/mutate.mjs 變異算子,逐設計記 variant id;mutate.mjs 不適配處
  內聯最小確定性變異並記錄),變異腿必紅(matched<expected 或 compile=false);
  判別力數字實測寫入 legs.json `mutprobe` 欄(10.12 TB 判別力鐵律同款)。
- 閘:`bash plugin/rtl2spec/run-spec-eval.sh` PASS(自對拍 3 腿+負探針全紅)。

### D-S5 = 盲寫採樣凍結(blindSample 3 腿)

- 盲寫協議:輸入=spec 檔全文+§2.1 出口約定(不給 DUT 原文;誠實邊界沿 REIMPL-PROTOCOL
  §0——盲隔離弱於 E1 原始輪,數字僅輔助證據);產出=頂模組名==spec.module、端口逐字合
  spec contract 的 `top.v`(與 wrapperO 同輪編譯,§4 D-S4 接線契約),**凍結入
  `plugin/rtl2spec/fixtures/d-spec/reimpl/<design>/top.v`**(e1-reimpl/ 在庫先例同形)。
  採樣身份/模型/參數/prompt 模板逐字 append 入 REIMPL-PROTOCOL.md 新 D 節
  (append-only,協議釘檔律)。
- legs.json 三腿 expected 由首次實跑錄製(禁手寫預期結果;expected.{compiled,matched}=
  錄製產生的斷言規格,錄後與 §4-D-S4 判據對拍)。
- 閘:`bash plugin/rtl2spec/run-spec-eval.sh` PASS 3 腿(編譯+場景 diff 對 expected 逐字)+
  負探針全紅;spec-eval 報告(build/spec-eval/verdict.json)逐腿記 matched 分段
  (盲寫數字=輔助證據欄,不進任何確定性閘斷言)。

### D-S6 = 正典一次性寫入+閘收編(末片)

- RTL2SPEC-PLUGIN.md 增 **§21 D spec 前端**(append-only,§0..§20 零刪改;逐切片
  D-S0..D-S5 機械門檻+實跑數字+對賬記錄,§19/§20 同形)。
- REGISTRY.md:D-c3 已清償標記(S0 已做,終核);Crates 表增 cli 面行;若弧中出現
  我方-既定行為的新分歧→先登記再繼續(10.5)。
- **ADR-0014**:CLI-AGENT 憲法(§2 四條逐條+rationale:agent 消費者/委派形裁決/三面
  跟隨)+spec 格式契約指針(ADR-0012 修訂節記 D 前端解封=R5 時點判據過)。
- AGENTS.md 增 **10.13 CLI-AGENT 與 spec 前端紀律**節(條文=§2 四條+§3 格式釘檔+
  凍結律:fixtures/d-spec/reimpl 入庫=凍結工件,換檔=協議事件禁手改+fixtures/d-spec
  子樹內容類型放寬=自著 spec md/legs.json(斷言規格)/凍結 top.v,顯式例外沿 10.12 形
  不得默認+wasm sha 變更=conformance 重錄重申+第四層證據頁指針)。
- SKILL.md 增「CLI-AGENT 憲法增補」節(正典條文;eda-dev 同款增補節)。
- **第四層證據頁(R1-5;R2-1 釘確定性)**:10.13 的證據工件=`plugin/rtl2spec/
  spec-eval-report.md`——**確定性頁面**:內容=命令行/口徑/工具鏈(錄製時)/逐腿
  compiled+matched+mutprobe 判別力表/parity 摘要/wasm sha,**零牆鐘欄**(verdict.json 的
  generatedAt 只入 build/,不入頁);保鮮形=conformance-report.md:3 同款——默認模式重生
  成到 build/ 並與已提交頁 byte 比對(陳舊/手改=exit 1),`--record` 才寫已提交頁
  (conformance-eval.mjs:1366-1368 同形;已提交頁永不隨普通運行觸碰)。
  四層對照:①閘=run-spec-cli-parity.sh+run-spec-eval.sh(收編 regress)②正典=SKILL 增補
  節③本文=AGENTS 10.13 ④證據頁=spec-eval-report.md。
- 閘收編:scripts/regress.sh S7 rtl2spec 塊增兩閘(§5 雙守衛降級形:parity 守衛=
  bin+pkg+bun 在場;spec-eval 守衛=iverilog 在場,缺場 exit 2 SKIP);全量終檢=
  `bash scripts/regress.sh` REGRESSION GATE PASS+`bash plugin/rtl2spec/run-plugin-check.sh`
  PASS+`git status --short` 盤點(弧內不 commit,由 user 定)。

## 5. 新閘(會進總閘;雙守衛降級形)

### 5.1 plugin/rtl2spec/run-spec-cli-parity.sh

- 判據:對 fixtures/d-spec/*.spec.md 與 corpus 抽樣輸入,`cargo run -p rtl-spec --bin
  rtl-spec -- extract --format json` 輸出與 `bun` 載 dist 同一 wasm 的
  rtl_extract_rules_sv 輸出 **byte-equal**;spec 檔另腿 CLI `spec lint` vs wasm
  `rtl_spec_spec_lint` byte-equal。守衛:cargo 產物+pkg+bun 任一缺→exit 2(降級);
  在場時任一 byte 不等→exit 1。裝載形沿 golden-check.mjs:37「裝載 dist 同一 wasm
  (顯式位元組 init)」【勘察】。
- 負向探針:臨時在 CLI 側改一處渲染(如 max_rules 默認值)→ parity 必紅,實跑留證還原。

### 5.2 plugin/rtl2spec/run-spec-eval.sh

- 判據:legs.json 全腿——接線斷言(DUT 頂模組==spec.module)+iverilog 編譯+SCENE 塊
  diff 對 expected 逐字;mutprobe 腿全紅;證據頁 spec-eval-report.md 按 conformance 同形
  保鮮(默認模式重生成到 build/ 與已提交頁 byte 比對,陳舊/手改=exit 1;`--record` 才寫
  已提交頁;D-S4 起生成,D-S6 定為 10.13 第四層);exit 0=全過。守衛:iverilog 缺場
  exit 2;腿產物缺 exit 3;寫面=build/spec-eval/+證據頁一檔(10.12 寫面聲明同款)。
- 負向探針:mutprobe 即內建負探針;另驗證「legs 改一個 expected 數字→閘紅」、
  「legs 刪 wrapperO→接線斷言紅」、「手改證據頁一字符→byte 比對紅」各一次
  (實跑留證後還原)。

## 6. 盲寫採樣(blindSample;spec 檔已入庫【本輪自著】)

| design | specPath(已寫入工作樹) | 行為條數 | TB/場景 | 凍結產物(S5) |
|---|---|---|---|---|
| counter8 | plugin/rtl2spec/fixtures/d-spec/counter8.spec.md | 2 | tb_counter8.v/6 | d-spec/reimpl/counter8/top.v |
| acc_pipe | plugin/rtl2spec/fixtures/d-spec/acc_pipe.spec.md | 9 | tb_acc_pipe.v+wrapper/6 | d-spec/reimpl/acc_pipe/top.v |
| i2c_master | plugin/rtl2spec/fixtures/d-spec/i2c_master.spec.md | 94 | tb_i2c.v+wrapper/11 | d-spec/reimpl/i2c_master/top.v |

- 撰寫取證:三檔行為面逐條對照 DUT 原文寫出(counter.v 全文/acc_pipe.v 全文/
  i2c_master.v:68 cnt_end=`(cnt >= prescale - 1)`+build/reimpl-eval/
  rulebook-trimmed-i2c_master.md 152 條逐條)——spec 忠實度本身是 D 評測的自變量,
  盲寫 matched 低=spec 質量信號,如實入報告禁粉飾。
- spec 檔與 DUT 的對應聲明:counter8 無內部 state(q 即狀態);acc_pipe 8 個 state=換名
  抽象(DUT 內名不入 spec);i2c 11 個 state 沿 DUT 概念名+1 個 derived(cnt_end=
  i2c_master.v:68 組合 wire)——「高級 spec 允許提抽象狀態/組合中間量」的格式依據(盲寫方
  自由落地,TB 可觀測行為合即可);三檔 module 行=DUT 側名,合 §3 module 契約。

## 7. 拆弧判據與誠實條款

- 拆弧觸發(擇一即拆):①S3 conformance 重錄 diff 出現 provenance 以外變更(停弧排查,
  10.5);②S1 三正樣按 §3 文法解析出結構性衝突且非解析器缺陷(格式重議=§3+ADR-0014
  修訂+fixtures 同輪改對賬;R1-1/R1-3/R1-4 開弧前已裁三類不再算觸發);③S5 三腿盲寫
  compiled 全敗=spec 格式契約失效,停弧升級登記(過程綠不得包裝成結果達標,MVP-NEXT §4.2)。
- 本規劃輪未跑清單:未實跑 cargo test/build/regress/任何 run-*.sh(S0 未實作);本人未讀
  dist/server.mjs 原文(五工具承【勘察】,SERVER_VERSION 承審 R2 實證);i2c spec 對 11 場景
  的充分性未驗(S5 實測回答,正是評測目的);三 spec 檔未過任何解析器(S1 首測對象)。
- 數字引用規則:commit message 行數/計數一律取提交時刻實測(AGENTS §8);本檔內
  「2/9/94 條」「斷言 19→21」=規劃值,實作輪實測後如有出入同輪訂正本檔+commit message。

## 8. 修訂記錄 R1(2026-11-08 對抗審 5 條逐條消化;每條附實證)

1. 【閘過不了→文法擴通道+掃描面釘死】三正樣「零 findings」確會被原 ⑥ 紅:【實跑】
   `grep -n "check(" i2c_master.spec.md | grep -c cnt_end`=22(首見 :96 B021、末見 :268;
   R2 勘誤:行號係 derived 行插入前實測,現首末=:97/:269),
   cnt_end 不在其 state 清單;修法=§3 增第四聲明通道 `derived:`(組合中間量),⑥集合改
   ports∪params∪states∪derived,i2c_master.spec.md 同輪補 `- derived: cnt_end …` 行。
   假陽性風險同條釘死:⑥**只掃值側**,顯式排除欄位鍵/五形名/posedge/negedge/`-`/數值
   字面量/`true`/`false`(lint.rs:23-38 KEYWORDS 實無 true/false【實讀】);fixtures 實測
   20×async=true+1×async=false,⑥零誤報形必含 async=true guard 行+derived 引用。
2. 【實作翻車點→module 契約反轉釘死】§1 舊文「模組名 __acc_pipe__acc_pipe」確為錯述:
   【實讀】dut_orig_accpipe.v:3 `module dut_accpipe`、內包 :9 `__acc_pipe__acc_pipe u`;
   tb_acc_pipe.v:14 `dut_accpipe dut_i`、tb_i2c.v:30 `dut_i2c dut`。修法=§3 新增
   「module 契約」:spec.module=harness wrapper(wrapperO)所實例化的 DUT 側模組名,
   盲寫 top.v 與 wrapperO 同輪編譯;legs.json 接線按此釘+runner 啟動接線斷言(DUT 檔含
   `module <spec.module>`,不過 exit 1);§1/§4 D-S4/§5.2/§6 同步改寫。
3. 【SSOT 自相矛盾→文法 SSOT 換錨】【實讀】md.rs:188-197(:195 格式串)sequence 發射
   `anchor(…) +N拍 →`,reimpl-eval.mjs:57 trim writer 產無 anchor `+N拍 →`,fixtures=後者
   (實測 sequence 行 1+5+64=70 條全無 anchor)。修法=§3 釘 spec check 行文法 SSOT=
   本節代碼塊+三正樣形(sequence 無 anchor),明寫「md.rs/trim writer=兩個規則書渲染面,
   與 spec 文法不可互推、變動不溯及」;D-S1 出入協議反轉:出入以 §3 為準修**解析器**,
   md.rs 發射形永不作為 fixture 改寫依據(原「以發射形為準修 fixture」句刪)。
4. 【文法未涵蓋自家語料→行級文法五條】§3 增行級文法(R1-4):`###` 子節頭=跳過
   (i2c 9 處,R2 勘誤:現 :51/:94/:103/:116/:140/:170/:196/:228/:250)、節內散文=跳過
   (acc_pipe.spec.md:20-22、i2c_master.spec.md:46-48)、條目續行=縮進併入且 behavior
   條目結構行恰 1(acc_pipe.spec.md:50、i2c :130/:160 等 8 處)、空行=條目界;⑤查同步收
   「結構行數≠1」;S1 測試補三形斷言。fixtures 零改動(文法按實際語料收編)。
5. 【違反 10.4 四層鐵則→第四層補指針】S6 增「第四層證據頁」:
   `plugin/rtl2spec/spec-eval-report.md`(spec-eval.mjs 機械生成禁手改,C 弧
   conformance-report.md 同形先例;閘斷言陳舊=exit 1——保鮮形 R2-1 進一步釘死);
   §5.2/10.13 草案同步;四層對照表入 §4 D-S6(①閘 ②SKILL ③AGENTS 10.13 ④報告頁)。

## 9. 修訂記錄 R2(2026-11-08 對抗審 3 條:1 裁決+1 行號勘誤+1 核銷)

1. 【閘機制自相矛盾→證據頁確定性裁決】R1-5 頁規格含「生成時刻」,與 byte 比對保鮮矛盾:
   verdict.json generatedAt=牆鐘【實跑 python】`2026-11-07T14:36:36.376Z`;先例頁無牆鐘欄,
   conformance-report.md:3「默認模式重生成本頁到 build/ 並 byte 比對」(--record 才寫;
   寫面 conformance-eval.mjs:1366-1368)。裁決=頁內容**零牆鐘**+保鮮形沿先例;§4 D-S4/
   D-S6/§5.2 同步改寫,負向探針增「手改證據頁一字符→byte 比對紅」。
2. 【引用漂移】derived 行插入後行號重測【實跑 grep】:### 實際 :51/:94/:103/:116/:140/
   :170/:196/:228/:250(§3 已改);cnt_end 結構行首末=:97/:269(§8.1 已勘誤);計數不變。
3. 【R1 四條核銷+基線復核】審 R2 結構掃描 dangling=NONE、B 條目恰 1 結構行×94、續行 8 處;
   orig DUT 頂模組 counter.v:5/acc_pipe.v:2/i2c_master.v:27 皆==spec.module(本輪 grep 復核
   一致,已補入 §4 D-S4 接線契約)。
