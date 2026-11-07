# ADR-0013: C 閉環全量化(規則書→TB 生成+conformance 評測+診斷回灌)

日期:2026-11-06。狀態:**接受(2026-11-07 C-S6 起`run-conformance-eval.sh`轉現役——C-S3 已交付,
原「機械閘分階段」規劃條款由本檔實作記錄 C-S6 條承載修訂;append-only 原文保留於此)**。
正典企劃=docs/RTL2SPEC-PLUGIN.md §19(實作記錄同章追加)。
前置=ADR-0012(rtl-to-spec 插件弧)與 RTL2SPEC-MVP-NEXT §7.3 後續隊列首項。

脈絡:R3「C 閉環全量」當初落選的前提(MVP-NEXT:245「閉環判別力依賴規則書先夠好」)已由復原力弧
消除(§18.2 S5:acc_pipe A1..A3 6/6、i2c A1..A3 11/11);但對拍床是量測器而非判別閘
(scripts/reimpl-eval.mjs:136-139/:205-206)、倉內沒有 TB 判別力的機械證明、且「TB 11/11≠等價」已有
實證(i2c `cnt_end`:net 宣告賦值落 sv/mod.rs:686 `_ => {}` 臂無規則,e1 TB 固定 prescale 看不到
上下文位寬差)。本 ADR 立 C 弧決策 **D-C1..D-C13**——D-C 前綴避讓 ADR-0012 D1..D19 與 ADR-0007
D17,引用須全限定「ADR-0013 D-Cn」;切片編號 C-S1..C-S6(避讓 §18 S0..S6)。

## 決策

1. **D-C1 三層分立+oracle 定義**:L-A TB 入庫判別力閘(確定性)/L-B conformance 計分(計分確定;
   被計分物是 LLM 產物時只作輔助)/L-C 診斷回灌(診斷生成確定;LLM 迭代輔助,≤3 輪)。oracle=原版
   RTL 在 iverilog 13.0 下的可觀測行為;期望一律 `--record` 錄製,TB 只觀測不自判(AGENTS §1)。
   conformance 度量**行為復原力**,不是意圖正確性;禁止宣稱清單=RTL2SPEC-PLUGIN §19.1 六條。
   **閘判據劃界(§19 R1-3)**:凍結入庫工件(T/D 腿 tb.v、錄製 expected.txt/record.json、機械變異)
   的機械可測屬性構成入庫與閘 exit 判據——判的是工件;LLM 採樣過程(prompt/模型/採樣次數/K 選擇/
   輪次)與重寫產物計分永不構成閘 exit 判據(10.11、§18.3 S5 先例本義)。
2. **D-C2 TB 生成器雙腿**:T 模板腿=rtl-spec 核心 `tb_plan`/`tb_template_v`(新能力先落核心,
   10.7/10.11;露面=wasm `rtl_tb_plan_sv`+MCP `rtl_tb_plan`+skill `rtl-conformance`,server.mjs 零
   語義);D 定向腿=LLM,按 plugin/rtl2spec/CONFORMANCE-PROTOCOL.md 釘檔(延續 REIMPL-PROTOCOL 形:
   模型身份/採樣參數/prompt 逐字/腿表 append-only;三模板已於 §19.3.4 預先登記)。兩腿同一入庫
   判據;無採樣條件=T 腿單獨落地並標「環境受限未跑」,確定性腿照常跑通採樣集全量。tb_plan 不支援
   的構造=`(milestone TB1)` 硬錯(AGENTS §3),清單十一條(§19 RV2-8 補全):解析錯誤或零模組(前端已帶
   Mx 者保留)/指定模組不存在/多模組未指定/parametric 端口/inout 或孤兒端口(width 0 且非 parametric)/
   零輸出/多時鐘/時鐘或復位非輸入端口/時鐘復位同訊號/規則書 truncated/RTLIL 輸入。**API(§19 RV1-11)**:
   `tb_plan(input, module) -> Result<TbPlan, TbError>`(計劃期攔全部 TB1)、`tb_template_v(&TbPlan, &TbOpts)
   -> String`(不可失敗,TbOpts 默認值 SSOT=tbplan.rs);wasm `rtl_tb_plan_sv(name, source, module, seed,
   cycles, flush)` 與 **`rtl_tb_plan_rtlil`(同參,核心恆回 TB1;§19 RV2-8)**:空串=未指定、0=核心默認(同
   `max_rules==0` 約定),回傳 `{"ok":true,"plan","template_v"}`|`{"ok":false,"error"}`;MCP `rtl_tb_plan`
   帶 `lang`,沿既有 `langOf` 分派(server.mjs:77-82);native 對拍=emit.rs `tbplan` 模式(sv|rtlil)。tb_plan
   直讀 contract 端口寬度/符號,故以 D-C13(D-c2 修復)為前提。
3. **D-C3 TB 協議 v1**:`SCENE<n> <name>`+`  key=value`+`DONE` 行文法;值禁 x/z(X-clean,AGENTS §2);
   正規化=丟行白名單**按模擬器分腿**(禁內容改寫,10.5/10.8):iverilog 腿=錨定 regex 的 `$finish called
   at` 橫幅與 `VCD info:`;verilator 腿=沿 vl-difftest `normalize_oracle_stdout`(lib.rs:124-133)四精確
   前綴 `- `/`-Info`/`-Warning`/`-Error`(corpus_lint.rs:185-188 釘死;§19 R1-1:verilator 在 DONE 後
   追加 4 行含 walltime/cpu/MB 的 `- ` 報告行,原單腿白名單下 Q7 必全紅);協議合法行無 `-` 開頭且 Q5
   作用於 iverilog 腿,故不可能誤吞;源碼機讀註釋 `TB-LEG/SCENES/COVERS/
   WAIVE`;靜態約束=contract 名實例化+全端口具名連接、禁層次引用、禁 $random、**TB-R1x**(§19 RV1-5:
   跨檔、極性感知——E=tb_plan.sens_edges[核心枚舉 DUT 全部沿行程敏感邊],違例=TB 事件控制 `@(e net)`
   後語句窗內對「連到 DUT 輸入端口的 TB 變量」的阻塞寫且 (e, port(net))∈E;語句窗=corpus_lint 文檔語義
   [同行 `#` 即關窗;§19 RV2-7:corpus_lint.rs:80-95 的 `closed` 是空操作,其代碼對「`#1;`+換行寫」報假陽性,
   已登記 D-c3,Rust 側本弧不修];corpus_lint R1 是同檔、不分極性的文本規則,只作 TB 內部沿行程的同檔檢查,
   與其 parity 只聲明於三個同行向量;跨檔五向量進 selfcheck)、等待有界、
   內部看門狗。理由:修 reimpl-eval 的塊數虛高與無看門狗兩洞,並使
   TB 對重寫內部命名中立(port-only;obligations 的 observe 取 `port|indirect`,indirect 須經端口表面化
   否則 WAIVE unobservable,§19 R1-9)。
4. **D-C4 判別力雙 fixture**:正樣=原版 2 跑相等且==錄製 expected.txt(verilator 在場時跨模擬器
   相等=X/競爭第二證);變異樣=**規則錨定**算子目錄 v1(**錨定行=evidence[0] 區間內賦值左值經 norm 後
   ==規則主體的唯一一行**——guard 每個 effect.lhs 各一錨、sequence 的 signal、relation 的 lhs;阻塞 `=` 與
   非阻塞 `<=` 同等;RHS 取源碼原文不用渲染值;norm=去空白+無位寬十進制 N→`'sdN` 單一定義;§19 RV2-5:
   初稿「多行 span 找第一個含目標 token 的行」使 i2c 20 條 guard 只落 7 行、11 條落同一行,六設計按主體
   定行實測全部唯一;行內 token count==1 手術;每規則 ≤2;OP-NOP 控制組;**變異歸屬集**=錨定行相同且
   算子適用的全部規則,逐變異入 record,規則驗證覆蓋拆獨佔/共享兩欄禁合併)+**P-unruled 池**(不在任何 evidence
   區間的賦值/條件行,不入分母,被殺=規則書缺陷的確定性證據)。存活者僅在 `--record` 時交 yosys
   oracle 裁決(僅測試用、永不 ship;ryosys equiv_*=Y0 stub 不可用):proven→equivalent 控制組
   (必存活),否則 unadjudicated(保守入分母)。等價只認 proven。**確定性補充(§19 R1-5/R1-6)**:
   OP-STATE 枚舉源=源碼級(同 state_reg 的 transition evidence 區間內 `<state_reg> <= X;` 的 RHS 原文,
   行號升序首次出現去重成 Vec,循環取下一個),禁集合迭代;OP-ASYNC 覆蓋 `or`/逗號與前後置四形
   (sincos.v:45 為逗號形);可適用但無唯一錨=`ambiguous`、不適用=`n/a`,一律入 record.json,禁靜默跳過。
   跨模擬器腿構建=`--binary --timing -Wno-fatal`,tb.v 位置警告必 0、DUT 位置只記錄(§19 R1-12:
   signedexpr/subbytes 在 verilator 默認告警集 lint rc=1)。
5. **D-C5 入庫門檻**:Q1..Q9(RTL2SPEC-PLUGIN §19.4.3)全過;判別力鐵則=每設計 P-rule ≥1 killed+
   全部控制組存活+結果==錄製(雙向 DRIFT 紅);D 腿附加形族底線(每個出現的 form 族若有 eligible
   變異則 ≥1 被殺)與不劣於 T 腿基線——兩條判的是凍結 D 腿 tb.v 工件,屬 D-C1 劃界內的合法判據。
   斷言粒度=逐變異體(結果類+殺傷場景集與 record 雙向比對),正樣=全輸出 byte 相等;下限只有
   per-design ≥1 killed,per-rule/per-scene 判別力只錄製不斷言(§19 R1-4)。數值殺傷率下限 v1 不設;
   立/降下限唯一路徑=本 ADR 修訂+閘重跑,禁靜默。
6. **D-C6 斷言規格與錄製快照劃界**(ADR-0012 D14/D15 同形):case.json=斷言規格(手寫合法);
   tb.v(T 腿=錄製快照/D 腿=凍結產物)、expected.txt、record.json=只由 `--record` 寫,禁手改;重錄=
   語義事件(10.5)。**逐切片對賬對照物=錄前快照(§19 RV1-6;AGENTS 10.11)**:fixtures/conformance/
   弧內 untracked,git diff 恆空不可作對照物;`--record` 必帶 `--slice <C-Sn>`,寫前複製現存三件套到
   build/conformance/pre-record/<切片>/,寫後生成 record-diff-<C-Sn>.md,各片實作記錄必引之。**golden 腿
   (§19 RV2-4 訂正 RV1 輪「C-S2 起對照物=錄前 build/golden-current」——默認模式寫入的是當前提取器輸出,
   改碼後即修後結果,比對恆零差)**:兩腿同一原則=對照物恆為被覆寫前的已存工件;golden 重錄片按 §19.4.4
   G0..G5——G0 改碼前 `cp -R` 已存 golden/ 到 build/golden-pre-<片>,G2 修後默認模式 `DIFF` 檔名集合恰等於
   允許清單,G3 `--record` 後 golden-leafdiff.mjs 對 G0 副本逐葉對賬,G5 regress 最後跑(其 S7 含 golden
   回歸,regress.sh:383-386;§19 RV2-3)。
7. **D-C7 閘落點與總閘**:`bash plugin/rtl2spec/run-conformance-eval.sh`=總閘直調成員,**不進
   regress**(理由:變異矩陣分鐘級時長屬評測層,run-realworld-eval/run-reimpl-eval 直調先例;regress
   已以 golden/gap-fixtures 釘死本閘輸入);默認模式寫只落 plugin/rtl2spec/build/conformance/;`--record`
   寫 plugin/rtl2spec/fixtures/conformance/ 與證據頁 **plugin/rtl2spec/conformance-report.md**(§19 RV1-12:
   改落插件根內,不新增出 plugin 寫面——AGENTS 10.10、run-realworld-eval.sh:9-10;默認模式重生成比對,
   手改/陳舊=紅);
   exit 0=全 ADMITTED 且快照全等/1=判據敗或 DRIFT 或 selfcheck 敗/2=守衛(iverilog/bun 缺、dist wasm
   陳舊——本閘不重建 pkg;`--record` 另守衛 yosys);設計數==EXPECTED_DESIGNS 常量防短收。收編
   regress 的重議條件=≤30s `--quick` 子集+本 ADR 修訂。
8. **D-C8 與 run-reimpl-eval 分層**:reimpl-eval=復原力弧 S5 歷史消融量測床,凍結(不擴設計、不改
   LEGS/分段/exit 語義;量測器形符合 10.11);共享 plugin/rtl2spec/scripts/lib/tbproto.mjs(場景切塊/正規化/帶
   timeout 執行器),遷移以 verdict.json legs/segments 深等+runs/out_* byte 相等證 parity;新增的
   scenesParsed **不入** verdict.json(結構凍結),寫 sidecar build/reimpl-eval/scene-protocol.json
   (§19 R1-7);conformance
   不重跑 reimpl LEGS,重寫產物只作 aux 腿計分(e1 TB vs conformance TB 分數差=TB 強度讀數)。
9. **D-C9 診斷回灌**:counterexample(失配場景首差行+差異鍵)→診斷 JSON v1(suspects 三級確定性
   排序 direct/covers/mutant-sensitive[留一法];def_lint U1..U5;分類 rewrite-error/rulebook-gap-
   candidate/tb-protocol)→回灌對象:F1 重寫迭代、F2 TB 改進(LLM 輔助,各 ≤3 輪;F2 只回灌規則書
   文本不給源碼行;round≥1 記「閉環可復原性」,禁併入規則書復原力)、F3 規則書缺陷報告(P-unruled
   killed+def_lint 為確定性主體;**D-c1 net 宣告賦值未提取已於 C-S1 先行登記(連同 D-c2..D-c4,10.5),F3
   只為其補證據**——§19 RV2-13 同步正文 19.2/19.6.2;def_lint 為閘側工具,不經任何面露出)。確定性自驗=
   killed 變異當真值已知缺陷實現的留一法 top-3 定位命中率,錄製釘死。
10. **D-C10 採樣集**:判據 J1..J5(非平凡規則書且 specPath 實存/單模組非參數端口可 X-clean 可終止/
    in-git 授權 MIT 或 ISC,zipcpu 永不入/四形族+同步異步復位+流水線+組合/Σ規則 ≤300);6 設計=
    i2c_master/acc_pipe/fsm/sincos/signedexpr/subbytes(Σ252);D 腿 tbSample=i2c_master/acc_pipe/
    fsm/sincos;排除表與理由=§19.7。
11. **D-C11 版本**:新增 wasm 導出 2 個(7→9)+MCP 工具(4→5)+skill(3→4)=插件 semver minor **0.3.0→0.4.0**,
    四點一致+CHANGELOG(Added+Fixed D-r1a/b/c、D-c2);內核 ABI 軌不動(EXPECTED_ABI ['0.1.0'],ADR-0012
    D18);補 run-plugin-check ③c 導出面檢查(server.mjs 引用的 `api.<fn>` ⊆ dist wasm 導出),封住
    「加法導出同 ABI 版本」在調用時才失敗的缺口。**本條顯式修訂 ADR-0012 D4「四工具」、其非目標
    「MCP 只 stdio+四工具」與 RTL2SPEC-PLUGIN §11-9**(§19 RV1-3):stdio 單傳輸與零 HTTP/WS/sampling/
    resources/prompts 面不變,工具數 4→5(C-S2 生效);ADR-0012 原文 append-only 不改,修訂關係記於
    其實作記錄修訂指針條目。run-mcp-e2e 定義點(AGENTS.md:305/REGISTRY.md:84/run-mcp-e2e.sh:7,:57/
    mcp-client.mjs 頭註/OKR.md:62;現狀三分=七/實有 10 個 check 呼叫點/11)由改閘切片同片同步,斷言數取
    實跑 PASS 行數;validate-manifest.mjs:62 的 skill 清單同片加入 rtl-conformance(§19 RV1-8);「四工具」
    字樣的現役同步面另含 PLAN.md:41 與 dist/server.mjs:99(§19 RV2-11 普查全表);skill rtl-conformance 只
    編排 `rtl_tb_plan` 調用與解釋欄位、不判定不計分,零語義以 run-plugin-check ⑥(skills 內 `rtl_` 標識符⊆
    MCP 工具名)機械驗(§19 RV2-14)。
12. **D-C12 前置收平(C-S1,實現順序第一片)**:D-r1a=面委託核心(server.mjs 傳 0,wasm 既有「0 取
    核心默認」契約)+native emit.rs :10/:16 兩處 128 改引 DEFAULT_MAX_RULES(§19 RV2-10)+run-plugin-check
    ⑤ SSOT 漂移判據[(a) 字面量∈{0,N}、(b) 文案數字==N、(c) 回退
    token 錨:`const max =` 回退分支與 batch 實參必為字面量 0,§19 R1-8;(d) emit.rs 引用常量、無數字缺省]+
    mcp-e2e ⑧(i2c 四調用:不傳/
    顯式 0→152 零截斷、顯式 100→100+truncated、batch→152);
    D-r1b=§19.2 勘誤表承載實測口徑(HEAD 843/async 59/2 證 85/when 476;累計 +213;單切片口徑不可
    復算=歷史保留,§18.2 原文不改)+golden-stats 資訊行;D-r1c=trigger 取「首個非復位訊號邊沿」
    (回退首沿保持舊形),**規則面 sv/mod.rs:752 與破口面 sv/gaps.rs:192-217 兩處同改、共用
    select_trigger**(§19 RV1-1:只修一面=規則書稱有 async guard 而破口報告稱「邊沿行程完全無復位臂」
    G4 certain,看起來綠、語義錯;RTLIL 面 rtlil.rs:197-209 同形但無現形樣,只登記不改)+五檔 fixture
    (正樣 rst_first_async/孿生 rst_first_twin[check 載荷相等,evidence 文本與行號不同]/負樣
    rst_first_compound[頂層條件非單訊號形]/零漂移釘 rst_first_loadsig[單訊號非列表條件照產 guard,
    §19 R1-2 實探]/負樣 rst_only_edge)**同時進規則面與破口面兩個矩陣**(破口面期望:rst_first_async
    修後零 G4,其餘四檔修前修後同)+golden 重錄按 §19.4.4 G0..G5(允許清單=mem_arst.rules.json+
    mem_arst.gaps.json,+PROVENANCE.md);同片於 REGISTRY 首錄 D-c1/D-c2/D-c3/D-c4(10.5 先登記)。
13. **D-C13 contract 端口屬性前提=D-c2 修復(C-S2 首步;§19 RV1-4/RV2-1/RV2-2/RV2-6)**:vl-syntax ANSI 端口
    組解析——新組(任一宣告限定詞開頭)重置範圍/符號/種類/型別四個滾動變量,**僅方向跨組繼承**(IEEE
    1800-2017 §23.2.2.3 同向);oracle=iverilog 13.0(-g2005/-g2012)+verilator 5.052 對七向量三方一致,HEAD
    七向量全錯,修後全對(§19.3.2 表)。**本條顯式修訂 ADR-0012 D1 的弧約束「禁破壞 analyzer-core 測試與
    API」**:crates/analyzer-core/tests/api.rs:128 `sv_ansi_inheritance_leg` 把缺陷語義釘成正確(d 寬 4),
    oracle 三方皆 d=1——同片把該期望改為 1 並改寫 :119-121/:58 註釋與 module.rs:139-140、ast.rs:97、
    analyzer-core/src/sv.rs:132-134 的「依 LRM §12.3.3 繼承」措辭;API 簽名零改;此為 D1 唯一例外(AGENTS §1
    oracle 即 spec),修訂關係記於 ADR-0012 實作記錄修訂指針第三條。驗收按 §19.4.4 G0..G5:`cargo test
    --workspace`(/tmp 副本實跑:補丁後唯一真敗即 api.rs:128,改期望後綠)→golden 允許清單=loops/memory/
    mem_arst/partsel 的 .rules.json 共 24 葉(partsel 16 葉為 signed true→false;gaps 零變)→**regress 最後**
    且基線零位移(corpus/ 96 檔與 sv-tests 260 檔 analyze 實驗零差);實作把滾動變量綁定移到組循環頂
    (零 warning)。機械防線=vl-syntax 七向量前端單測+tbplan 單測(busy 寬 1)+mcp-e2e ⑨ 模板同編
    `iverilog -g2005 -t null` 零警告(iverilog 在場才執法)。

## 與既有 ADR 關係

- **ADR-0012**:D8(閘驗出貨物)→本閘裝載 dist 同一 wasm;D12(庫內零 JSON 讀取)→tb_plan 收源碼
  不收規則書 JSON;D14/D15→D-C6;D16(評測層直調)→D-C7;D17 接入點表(新 wasm 導出/新 MCP 工具兩行)
  →tb_plan 露面路徑;D18/D19→D-C11。**D4「四工具」與非目標「MCP 只 stdio+四工具」由 D-C11 修訂為
  五工具**(stdio 不變)。**D1「禁破壞 analyzer-core 測試與 API」由 D-C13 修訂一處**(api.rs:128 被 oracle
  證偽的期望值;API 零改)。ADR-0012 機械閘的現役 8 條即本 ADR 機械閘 1 的「ADR 現役交付閘 8」;本 ADR
  另加規劃閘 1(run-conformance-eval),固定總閘序列中的 run-reimpl-eval/s6 run.sh 屬環境依賴直調,
  不是 ADR 交付閘。
- **ADR-0007(spec 驗證層)**:spec-i2c `Variant` 錨點手術形被本 ADR 借作變異手術形(加 count==1);
  本弧 TB 走 iverilog oracle,不接 vl-sim lockstep。
- **ADR-0010(ryosys)**:equiv_* 仍 Y0 stub,等價裁決不走 ryosys。
- **ADR-0001(調度確定性)**:TB-R1x 禁 TB 驅動與 DUT 同沿採樣的競爭,與其一致。

## 機械閘

1. 總閘命令序列=ask 固定契約 11 條,腳本直執、全 exit 0;**地位分三類(§19 RV1-2)**——
   **ADR 現役交付閘 8**(in-git 可重現,不依賴 LLM 產物):`cargo test --workspace`/`bash scripts/regress.sh`/
   `bash plugin/rtl2spec/run-mcp-e2e.sh`/`bash plugin/rtl2spec/run-rtl-validation.sh`/
   `bash plugin/rtl2spec/run-golden-regression.sh`/`bash plugin/rtl2spec/run-gap-fixtures.sh`/
   `bash plugin/rtl2spec/run-realworld-eval.sh`/`bash plugin/rtl2spec/run-plugin-check.sh`;
   **現役閘 1(C-S6 起)**=`bash plugin/rtl2spec/run-conformance-eval.sh`(原列規劃閘,C-S3 交付
   後本檔實作記錄記現役;「禁預寫為現役」紀律同款——改寫發生在交付之後);**環境依賴直調 2**=`bash plugin/rtl2spec/run-reimpl-eval.sh`/
   `bash plugin/rtl2spec/build/s6-metamorphic/run.sh`(gitignored 產物;非 ADR 交付閘、非 regress 成員,
   其 exit 不構成本弧任何確定性判據;新 clone 下前者 exit 3、後者不存在=固定契約的已知張力,交用戶
   裁決 §19.13-2)。
2. 本 ADR 新增的判據點(各落其閘):run-plugin-check ⑤(a)..(d)(C-S1)/③c、⑥ 與 ② 的 skill 清單(C-S2);
   mcp-client ⑧(C-S1,四調用)、②改恰 5 工具與 ⑨(含模板同編零警告)⑩⑪(C-S2);gap-fixture-check
   規則面矩陣 +5 行與**破口面矩陣 +5 行**(C-S1);run-golden-regression 尾部 STATS 資訊行(C-S1,不斷言);
   golden 重錄片的 G2 `DIFF` 檔名集合恰等於允許清單、G3 leafdiff 對賬(§19.4.4;C-S1 起);vl-syntax 七向量
   ANSI 前端單測(C-S2);run-conformance-eval 的 selfcheck(含分腿正規化敵意向量、TB-R1x 跨檔五向量)+
   Q1..Q9+SUMMARY 行(C-S3 起)。
3. 驗收手跑命令(非總閘成員,切片驗收點名):`bun crates/rtl-spec-wasm/smoke.mjs`(+node)tb_plan
   parity 三路(sv 正樣/TB1 負樣/.il 輸入;C-S2);`bun plugin/rtl2spec/scripts/probe-determinism.mjs`(C-S1/C-S2)。

## 非目標

見 RTL2SPEC-PLUGIN §19.11(15 條:不宣稱意圖正確性/LLM 採樣過程與重寫計分不作閘 exit 判據[凍結工件
機械屬性才是判據,D-C1 劃界]/不做 SVA 與 TB 內期望/
不做 RTLIL 與參數寬度求值[RTLIL 輸入經核心 TB1 硬錯]/不修 net 宣告賦值洞與 S2 政策、不改 RTLIL 面同形
觸發選取、不修 corpus_lint 窗口(D-c3)與負數下標誤標(D-c4)/不收編 regress/
不實作 ryosys equiv[yosys 只用於 --record 存活裁決與 C-S5 演示殘差查詢兩處,皆非閘判據]/不改
reimpl-eval 語義與不晉升 build/ 產物/S6 不入 git 不入 ADR 交付閘/不做 X 場景 TB/不 bump 內核 ABI/
不改 §0..§18/不做 GUI/若干設計不入採樣集/不做跨檔層級 TB)。

## 實作記錄

- 2026-11-06 規劃輪:本 ADR 新立+RTL2SPEC-PLUGIN §19 章追加(579 行,`git diff --stat` 實測;既有 1995 行零刪改,前綴
  sha256 斷言 c9f4b1e5e1a5…)+ADR-0012 實作記錄追加一條指針條目(5 行,既有 179 行零改,前綴 sha256 斷言 76325327…);零代碼改動。唯讀實測(探針只寫
  /tmp):golden 37 檔 Σrules=843(seq 539/rel 196/guard 85/trans 23)、async true 59/false 26、
  2 證 85、seq+when 476、v0.2.0=630;雙沿 always 釘選集 11 檔 18 行、復位前置僅 mem_arst;採樣 6 檔
  `iverilog -g2005 -t null` 零警告;def_lint 探針 i2c=[cnt_end]/acc_pipe=[out]/subbytes 7 項;工具=
  Verilator 5.052、Icarus 13.0、bun 1.4.2。總閘本輪未跑(規劃輪不寫 build/ 與 target/)。
- 2026-11-06 對抗審 R1 修訂輪(11 條+自查 1 條逐條消化,記錄=RTL2SPEC-PLUGIN §19.15):§19 原地修訂+
  19.15 追加,PLUGIN.md 對 HEAD 累計 +741/-0(`git diff --numstat` 實測;前 1995 行 sha256 c9f4b1e5… 不變;
  §19.3.4 模板塊 sha256 cc28f0c7… 不變);本 ADR 同步 D-C1/D-C3/D-C4/D-C5/D-C8/D-C12/機械閘第 2 條/
  非目標摘要。R1 實跑(探針只寫 /tmp/c-arc-r1/):verilator 5.052 最小 TB 在 DONE 後追加 4 行 `- ` 報告
  (Q7 分腿正規化依據);dist wasm 五形探針(D-r1c fixture 規格定案);verilator 默認告警集 lint 六檔
  (signedexpr/subbytes rc=1→`-Wno-fatal`);reimpl-eval `clock` 只定義零調用。總閘本輪未跑。
- 2026-11-07 換人復審修訂輪(14 條+自查 2 條逐條消化,記錄=RTL2SPEC-PLUGIN §19.16 RV1-1..RV1-16):§19 原地
  修訂+19.16 追加,PLUGIN.md 對 HEAD 累計 +1027/-0(`git diff --numstat` 實測;前 1995 行 sha256 c9f4b1e5…
  不變;§19.3.4 模板塊 sha256 cc28f0c7… 不變);本 ADR 改 D-C2/D-C3/D-C6/D-C7/D-C8/D-C11/D-C12、新增 D-C13、
  改與既有 ADR 關係/機械閘 1-2/非目標;ADR-0012 追加第二條修訂指針(累計 +11/-0,既有 179 行零改)。復審實跑
  (探針只寫 /tmp/c-arc-rr/):dist wasm `rtl_scan_gaps_sv` 五形(復位前置/複合/單沿=G4 certain,時鐘前置/
  loadsig=[])+golden mem_arst.gaps.json 含 G4-MyMem-001;vendored yosys 產兩序 .il,RTLIL 面兩序皆 0 規則;
  ANSI 最小樣 iverilog/verilator 皆 busy 1 位、舊構建 rverilator 4 位;dist wasm fsm_counter busy 寬 4;
  ANSI 普查 L1+fixture 6 處 5 檔、corpus/ 96 檔 0 處;定義點 grep(七斷言 5 處、check 呼叫點 10、OKR 11)。
  總閘本輪未跑。
- 2026-11-07 換人復審修訂第二輪(14 條+覆蓋面聲明照錄+自查 1 條逐條消化,記錄=RTL2SPEC-PLUGIN §19.17 RV2-1..RV2-15):§19 原地修訂+
  19.17 追加,PLUGIN.md 對 HEAD 累計 +1327/-0(`git diff --numstat` 實測;前 1995 行 sha256 c9f4b1e5… 不變;
  §19.3.4 模板塊 sha256 cc28f0c7… 不變);本 ADR 改 D-C2/D-C3/D-C4/D-C6/D-C9/D-C11/D-C12/D-C13/與既有 ADR 關係/
  機械閘 2-3/非目標;ADR-0012 追加第三條修訂指針(D1 例外,累計 +17/-0,既有 179 行零改)。復審 R2 實跑(探針
  與 workspace 副本寫只落 /tmp/c-arc-r2/,倉庫本體零寫入未跑閘):oracle 七向量 21 運行(iverilog -g2005/-g2012+
  verilator 三方一致,HEAD 七向量全錯);rustc 編譯 corpus_lint race_findings 原文六向量(同行 # 不關窗=假陽性實證);
  副本 native emit 未補丁/補丁兩版逐葉比對=釘選 37×2 same 70 diff 4 檔 24 葉(含 partsel 16 葉 signed)+L1 非釘選
  retime 1 葉,corpus/96+sv-tests/260 零差;副本 cargo test --workspace(補丁版)=唯一真敗 api.rs:128+fresh 副本
  缺 lib 產物的 riv-gen 環境假象(補建後 1/1;api.rs 改期望後 13/13);guard 錨定按主體定行六設計全部唯一、
  naive 首含行法 i2c 20 條落 7 行(11 條落同一行);golden 37 檔 guard trigger 多時鐘 0/內部時鐘 0、yosys-simple
  零 inout;smoke.mjs 128 命中 :53/:80(勘誤 RV1-15 的 :79)、emit.rs :10/:16 兩處 128 補入普查。
- 2026-11-07 C-S3 實作(確定性床;記錄=RTL2SPEC-PLUGIN §19.20;弧內禁 commit=工作樹呈現):機械閘 1 的規劃閘
  `run-conformance-eval.sh` 已交付——首錄 `--record --slice C-S3` 6/6 ADMITTED(SUMMARY mutants=439 eligible=412 killed=210
  equivalent=27 unruled_killed=13)、默認模式 exit 0;條文層「規劃閘→現役」改寫與 AGENTS/SKILL/OKR 四層同落按 §19.9 歸
  C-S6(本條只記交付事實,不預寫現役)。D-C4 算子目錄 v1 的未定項於實作中定案並錄入 record.json catalog 欄:每規則兩個算子的
  選用序(規則專屬先於行程共享)、OP-STATE 行驅動(R1-5 逐行生成+to 唯一歸屬否則 span)、transition 錨定以 to 消歧、
  OP-NOP 控制組取每個 form 首條規則、P-unruled 只收賦值行(條件行已由規則 when 承載)。
- 2026-11-07 C-S4 實作(D 腿定向 TB;記錄=RTL2SPEC-PLUGIN §19.21):CONFORMANCE-PROTOCOL.md 交付(模板塊逐字轉入,
  開工重算內容錨 sha256 cc28f0c7…(34 行)=預登記值相等;預登記未提交僅內容錨,如實記);4 設計 K=1 入庫
  (--promote;F2 輪次:i2c/fsm 一輪停、sincos r0→r1 殺 OP-ASYNC→r2 waive 停、acc_pipe gain=0=T 已滿;K=2,3 未跑);
  撰寫者=GLM-5.3-Flash 同 session(污染聲明=盲隔離弱,D 腿數字只作凍結工件機械屬性證據)。SUMMARY killed=347
  (C-S3=210,Δ+137);record-diff-C-S4「T 腿轉基線腿」逐變異 14/48/278/84 全同、D 新殺 132/13/2/0 丟殺 0。
- 2026-11-07 C-S5 實作(診斷回灌;記錄=RTL2SPEC-PLUGIN §19.22):diagnose.mjs(診斷 JSON v1+suspects 三級排序
  [direct/covers/mutant-sensitive 留一法]+leaveOneOut)+def_lint.mjs(U1..U5,閘側工具不露面,RV1-14)入閘;
  record 新增 rulebookDefects.{defLint,leaveOneOut} 欄(默認模式 DRIFT 比對同擴)+rulebook-defects.{json,md}(17 項)
  +F3 閘斷言(i2c 缺陷必含 cnt_end=D-c1 確定性證據)PASS;留一法 rate 實測 acc 3/10、fsm 6/42、i2c 8/201、
  signedexpr 7/10、sincos 9/76、subbytes 3/8(v1 無下限照報;D-C9 判據的命中定義=真值規則集∩top3)。F1 演示=
  fsm/sincos 重寫腿各一輪 FIX 全配(污染聲明沿 §0;i2c 用在場 A1..A3;acc-A0 診斷示例=rewrite-error);
  訂正 C-S4 carryOver(i2c-A2 在 D 腿 TB 下 6/6,C-S3 的 3/4 係 T 腿 TB small 景)。
- 2026-11-07 C-S6 收尾立法(記錄=RTL2SPEC-PLUGIN §19.23):AGENTS 新節 10.12(C 閉環紀律+fixtures
  conformance 子樹例外指針)+SKILL「C 閉環增補」節+PLAN 里程碑表 C-S1..C-S6/TB1 族行+OKR KR1.12
  證據指針+REGISTRY Crates(vl-syntax ANSI 修復測數)與 conformance 閘行 C-S5 摘要;**本 ADR「機械閘 1」
  規劃閘→現役改寫生效**(C-S3 已交付後改寫;本條為其承載)。總閘 11 命令全跑 exit 0。
- 2026-11-07 收尾弧 S0(方案=docs/RTL2SPEC-FINISH-PLAN.md §2;實施記錄歸 §20,收尾切片一次性寫入):終審 2 medium 收平。
  ①WAIVE 措辭+標籤集:§19.24 i2c 11 條行內勘誤(21 變異=20 unadjudicated+1 equivalent);CONFORMANCE-PROTOCOL §6 模板
  v1→v2 協議事件(標籤集補 `unadjudicated`,內容錨 104d13f1…;v1 錨 cc28f0c7… 不變);tblint `WAIVE_LABELS`+文法外
  Q9 硬紅;dleg-prompt 取最高版本圍欄(proto-template.mjs 硬同步)。②TB-SHA-ALARM:record.tb.sha256/磁碟 tb.v/本輪主腿
  TB 三方矛盾=REJECTED 或拒錄(exit 1;修前手改 fsm/tb.v 追加註釋默認模式 exit 0 實錄),協議 §5 禁手改條。SUMMARY
  零變動(mutants=439 eligible=412 killed=347 equivalent=27),fixtures/golden/wasm 零重錄;i2c 11 條+sincos R035 的 tb.v
  標籤本片不動,逐規則重裁=等價裁決數據落地後(方向不預設)。
- 2026-11-07 收尾弧 S1(方案=docs/RTL2SPEC-FINISH-PLAN.md §3;實施記錄歸 §20,收尾切片一次性寫入):D-C4「等價只認 proven」補空真守衛——
  等價裁決腿換強形(+flatten+memory,無 blacklist 無 stash),equivalent 須 proven 且 select -count cells≥輸出端口位數,否則
  adjudication-failed(empty);eqverdict 三值,adjudication 兩值保留;record 新增 eqMeta/waiveTable/變異級 eq 欄,默認模式 eq-verify。
  偵測力斷言六條(gold-vs-gold/正樣 fsm m0045/反樣 i2c m0001/空真探針/killed 全母體/存量重驗)任一不過=拒錄。Q9 併入 WAIVE↔record
  交叉驗證;首次全集 --record 按預期被拒(i2c 11+sincos R035 標 unobservable 而數據 unadjudicated),12 條按數據 --promote 換檔
  (僅 WAIVE 行)後 --slice C-S7 全集重錄:expected.txt 0 行差、變異結果與 adjudication 全同、主 SUMMARY 逐值不變;SUMMARY-EQ
  adjudicated=84 equivalent=27 unproven=57 adjudication_failed=0 killed_selfcheck total=355 main=347 unproven=354 failed=1
  equivalent=0。訂正:FINISH-PLAN §1 記 i2c m0011 為「$pow 類」,實測=proc 報 `Async reset \rst_n yields non-constant value 1'm`
  (OP-POL 異步復位形,proc 前端,非證明器);`$pow` 邊界另以最小構造實測。ryosys equiv_* 僅註冊里程碑(PLAN.md),未實現。
- 2026-11-07 收尾弧 S2(方案=docs/RTL2SPEC-FINISH-PLAN.md §4;實施記錄歸 §20):波形幀診斷腿落地,**不進閘判據**。機械段=run-frame-diag.sh(構建守衛拒跑形、
  wavecap 第二 top 取 VCD 而 tb.v 零改、統一窗、sips 首選 qlmanage 備選、剝 title 畫布+PNG 雙檢定 modality、title 層與 stdout 層機械歸因);診斷=協議 §8 釘 prompt 與 schema,
  驗證器只驗工件機械屬性。雙檢實測:fsm/sincos/acc_pipe/i2c_master 可 vision,subbytes/signedexpr 畫布與 PNG 皆同→強制 text-fallback。方案 §4.4「vision ⇔ 雙檢皆異」的反向落成
  「雙檢皆異而標 text-fallback 須說明原因」,因視覺會話不可用是合法降級。vl-vcd 渲染契約未動;conformance SUMMARY、fixtures、wasm 不變。
- 2026-11-07 收尾弧 S3(方案=docs/RTL2SPEC-FINISH-PLAN.md §5;實施記錄歸 §20):T 腿擴面。勘察口徑復刻得 TB 化 57(全 L1);51 個新候選經
  staging(新增 `--fixtures-root`,寫面限 build/)預錄,ADMITTED 28/REJECTED 23;升級裁定=12 個未釘 ADMITTED 全收,golden 釘選 37→49,
  §16.1.1 表欠 S4 §20(FINISH-PLAN §11)。分兩批主根全集重錄(C-S8-1 +10,C-S8-2 +18),EXPECTED_DESIGNS 6→34,既有設計剝 tb.simMs 後逐位元組
  一致。D-C4/D-C5 機械門未降:新收 28 個全部過 Q1..Q9+Q8 鐵則+等價裁決六斷言。SUMMARY 34/34 mutants=863 eligible=794 killed=544
  equivalent=69;默認模式 3:08(方案外推 57 設計 6-10 分鐘,未觸發 P0/P1 或 --quick)。新增主根斷言 specPath∈golden 釘選集(初版漏入
  admitted,負向探針抓出後修,REGISTRY Incident 11-07)。登記:D-c5(等價腿 X 語義盲區)、D-c6(空真探針 memory 盲區)、operators 因 $pow
  被 gold-vs-gold 拒的政策待裁。
- 2026-11-07 收尾弧 S4(方案=docs/RTL2SPEC-FINISH-PLAN.md §6;記錄=RTL2SPEC-PLUGIN §20):§20 一次性寫入;§16.1.1 表欠條清償(§20.5 釘選全表
  49 檔為 SSOT,run-plugin-check ⑤e/⑤f 執法);conformance 默認模式加 wasm sha 保持斷言(record provenance.wasmSha256==dist wasm);插件
  0.4.0→0.5.0。總閘 12 命令全 exit 0(明細 §20.6)。
- 勘誤(本 ADR C-S4 行「D 新殺 132/13/2/0 丟殺 0」):i2c 實為丟殺 8(m0010 OP-ASYNC@70、m0022..m0028 OP-RVAL@83..89),gain=124=132−8;
  證據=build/conformance/record-diff-C-S4.md:288 與現行 record(RTL2SPEC-PLUGIN §20.7-1)。原行保留。
