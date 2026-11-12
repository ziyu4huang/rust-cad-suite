# REFLECTION — rtl2spec 戰役進度與反省(2026-11-03..11-08,八弧)

狀態行(2026-11-13 寫):基線=main 5dc55d3 已 push github(gh api 驗證);樹淨;tag rtl2spec-v0.3.0
在 4922352,v0.5.0/v0.6.0 未打(用戶定);下一隊列=E agent 工作流。本文數字全部來自已 push 的
閘實跑輸出與 `git diff --cached --stat` 實測,禁口數;性質=進度+反省合一,反省條文已入
AGENTS.md 對應節的以 AGENTS 為準(本文是實錄與為什麼)。

## 0. 北極星與終局狀態

用戶目標原文:「從 RTL 程式範例中,取得正確的規格書,讓人類可以做高級的工作,不用再去親自
寫 rtl code,只要讓 agent 寫就好。」

八弧後的達成形:**雙向閉環已通**——
- 正向(RTL→spec):規則書提取+破口掃描+TB 計劃,質量由 conformance 評測床機械度量
  (golden 98/98 釘選、34/34 設計 admitted、killed 544/794、rulebook-defects 97 項機械產出);
- 反向(spec→RTL,D 弧):人寫高級 spec 檔→盲寫 agent 只讀 spec 產 RTL→逐場景 diff 對原版
  全 matched(6/6+6/6+11/11)——「人類寫 spec、agent 寫 RTL」的方向性首次實證;
- 接口:rtl-spec CLI bin(CLI-AGENT 憲法:JSON-first/--help 自攜配方/一 CLI 全能力/self-contain),
  agent 用工具不再需要讀任何額外文件。

## 1. 八弧進度總表

| # | 弧 | run id | 日期 | 交付要點 | 總閘 | 版本面 |
|---|---|---|---|---|---|---|
| 1 | 主弧 | dwfrun-d82947a2 | 11-03 | 規則書提取/破口掃描/V1..V6 機械判據/MCP server+wasm 面 | — | 0.1.0 |
| 2 | 收平 | dwfrun-0df5c30c | 11-03 | 3 實質項收平(lit_u64 剥下劃線/sock 三級解析/計數實測) | — | — |
| 3 | 回歸+拓展 | 146ae9fc→Amend 0c14595c | 11-04 | golden 回歸閘/破口 fixture 矩陣/L1+L2 真實評測/單進程 wasm 終止性法條 | S9..S12 | — |
| 4 | 勘探實驗 | dwfrun-c3e8c977 | 11-05 | E1 復原實驗=實測復原阻斷點(N1..N5),證明「給漏水的船裝發動機」 | — | — |
| 5 | 復原力 MVP | dwfrun-d941dadc | 11-05 | contract 節+decl_lint/relation_worthy 分界/M1..M6/消融實證 acc_pipe 0/3→3/3、matched 0/6→6/6、i2c 10/11→11/11 | 7/7 | 0.2.0→0.3.0 |
| 6 | C 閉環全量化 | dwfrun-ea76f6d1 | 11-06..07(13h) | conformance 評測床(規則書→TB→逐場景 diff)/T 腿模板+D 腿凍結產物/盲寫 TB 採樣/rulebook-defects 機械化(E1 cnt_end 錯猜=D-c1) | 12 條 | 0.4.0 |
| 7 | 未覆蓋收尾 | dwfrun-61d349cc | 11-07 | 等價裁決腿(yosys oracle 測試側,record.json eqverdict;adjudicated=311/equivalent=69/failed=0)/golden 37→49 釘選/波形幀診斷腿(VCD→vcd2svg→PNG→視覺 LLM,modality 強制標注)/§20 正典 | 12/12 | 0.5.0 |
| 8 | D spec 前端 | dwfrun-8abe3b82 | 11-08 | D-c3 清償/rtl-spec CLI bin(CLI-AGENT 憲法)/spec 格式+七查 lint/盲寫 spec→RTL 三設計全 matched/兩新閘收編 regress S7/§21+ADR-0014+AGENTS 10.13 | 13/13 | 0.6.0 |

提交節奏(全部弧內禁 commit、工作樹呈現,提交弧收):11-05 四批(200b1c7..e46400d)→
11-06 四批+tag v0.3.0(4922352)→11-08 四批(3da7dde)→11-08 四批(5dc55d3)。每次提交弧前置:
①閘實測全綠 ②重錄對賬(例:D 弧 34 個 record.json 剥 simMs/date 後唯一 diff=wasmSha256,
零語義漂移實證)③10.5 登記在先(D-r1a/r1b/r1c、D-c7)④push+gh 驗證+status 空。

## 2. 當前能力面(組件×閘×證據)

- **核心**:rtl-spec(規則書/破口/TB 計劃/spec 檔解析+七查 lint;src/bin/rtl-spec.rs 僅 6 行
  thin wrapper)、analyzer-core 逃逸口、vl-vcd(SVG 渲染=GUI 憲法)、vl-difftest(corpus_lint)。
- **三面**:CLI(bin)/wasm(rtl-spec-wasm,sha fe64d518)/MCP(dist/server.mjs 手寫純 JS)——
  spec_lint_json 三面 byte-equal=單一源(run-spec-cli-parity 17/17)。
- **閘**(全部在 regress 或總閘直調):regress.sh(E2E/ryosys/analyzer/rtl2spec golden+gap/S5 smoke/
  S7 spec-eval+spec-cli-parity 雙守衛)、run-golden-regression(98/98 byte-diff+EXPECTED_PINNED 49)、
  run-gap-fixtures(13 行矩陣+判別力 selfcheck)、run-conformance-eval(34/34,mutants=863
  eligible=794 killed=544 equivalent=69,TB-SHA-ALARM+--promote 唯一換檔)、run-reimpl-eval、
  run-spec-eval(6 腿+mutprobe 必紅)、run-realworld-eval(93 檔)、run-mcp-e2e、run-plugin-check、
  run-frame-diag(直調不進閘)。證據頁全機械生成禁手改(spec-eval-report/conformance-report/
  RTL2SPEC-EVAL/validation-report/e2e-report)。
- **正典**:AGENTS 10.10..10.13+SKILL 增補節+REGISTRY(組件×閘×分歧×事故)+ADR-0012/0013/0014+
  PLUGIN §16..§21(append-only)。

## 3. 反省(教訓→實錄→硬化層)

### 3.1 commit message 數字病四次復發:病根是結構性的,不是態度問題
實錄:3a696ba→df54db3、784887c(1439+/51- vs 實測 208+/43-)、3da7dde(96+/1- vs 109+/9-)、
D 弧 f12bdc2+aa90f89 兩連(51f/4677+ vs 實測 59f/1678+;1219+/8- vs 649+/11-)。
病根定位:**把 add+stat+commit 寫在同一條 shell 鏈時,實測數字在 message 寫完之後才可見,
等於必然預設**——「下次小心」治不了流程缺陷。硬化=兩步走紀律(第一步 add+stat 看輸出,
第二步才寫 commit),D 弧批次 4 起一次寫對。**教訓:會反復犯的錯,把防禦做進操作序列本身。**

### 3.2 token 經濟:正典肥大讓規劃者破產
實錄:C 弧把規劃章追加進 63KB 正典,規劃者每輪重寫整章,燒 77M token(全弧 307M)。
硬化=方案獨立小檔(≤400 行),審查員只讀小檔+它引用的代碼行;D 弧對抗審 5→3→1 條 R3 PASS
收斂,證明小檔不損審查質量。**教訓:上下文是預算,文檔結構決定預算消耗曲線。**

### 3.3 循環論證邊界=整個項目的方法論基石
conformance 度量「行為復原力」(oracle=原版 RTL 在 iverilog 下的可觀測行為),不是意圖正確性;
「TB 全過=設計正確」禁宣稱;LLM 腿(盲寫 prompt/模型/採樣)永不進確定性閘,閘判的只是
凍結工件(tb.v/spec 檔)的機械可測屬性;盲寫採樣永遠標輔助證據。**教訓:自評系統最大的風險
是拿自己的產物當自己的答案——邊界要立法,不能靠自覺。**

### 3.4 誠實歸屬三型
①canon 預測被實測推翻,如實歸屬(S2 消融:canon 預測「仍低」,實測 0/6→6/6,歸因 N3 缺口
修復);②標籤不得宣稱無證據的方向(§19.24 WAIVE 病根:20 個 survived 被洗成「不可觀測」,
根治=等價裁決腿 eqverdict 把三分類機械化);③能力不可分就誠實降級(subbytes 幀診斷 gold/
mutant PNG sha 相同→標 text-fallback,判據改走 title/stdout 層)。**教訓:輔助證據鏈上每一環
的 modality/confidence/provenance 都要真。**

### 3.5 登記-清償循環(10.5)讓「樹淨」與「文檔一致」兼得
實錄:終審發現不阻塞弧,但必須先登記(REGISTRY D-r1a/r1b/r1c、D-c1..D-c7)再繼續;欠條一律
寫成可機檢標記並配清償閘(plugin-check ⑤f 掃「[欠]」標記;D-c3 清償時連 tblint.mjs 舊注記
的陳舊性都如實登記)。**教訓:欠帳不可怕,口頭欠帳才可怕——欠條要能被 grep 到。**

### 3.6 無 oracle 域的質量體系
spec 提取沒有兼容產品可對拍,质量閘=機械判據(V1..V6,降閾唯一路徑=ADR 修訂+閘重跑)+
獨立評審;golden=提取器自身輸出快照(錄製產生禁手寫),re-record=語義事件逐案對賬;
「完整測試」的操作定義=suggested_test 全部有 TB 條目;TB 判別力鐵律=正樣過+變異樣必紅
雙 fixture,無判別力的 TB=無效測試。**教訓:沒有 oracle 就自己造「機械可判」的替身,
且替身要有自證(負向探針/selfcheck)。**

### 3.7 工作樹呈現+閘先於 commit 的節奏
弧內禁 commit→產出全部工作樹呈現→提交弧按性質分批(核心/插件包/正典/立法),每批
單文件精確 add、實測 stat、push 後 status 空終檢。好處:弧與弧之間樹態清晰、重錄對賬
有對照物、commit 批次與語義變更一一對應。**教訓:提交是語義事件的收據,不是保存鍵。**

### 3.8 運行層(workflow 引擎)
Amend 保緩存接續多次驗證(146ae9fc 中斷→0c14595c 續跑省 31M);provider network_error
停滯=自動退避恢復勿干預(21 分鐘實錄);escalation 裁決模式好用(收尾弧 S3 釘選集契約衝突:
主會話裁「欠條登記+S4 閘清償」,既有章節零刪改與同輪更新兩條憲法的衝突就地化解)。
**教訓:阻塞問題答一句話比分叉重跑便宜兩個數量級。**

### 3.9 CLI-AGENT 憲法(用戶 11-08 定調)
「以 agent 使用行為優化、self-contain 不用額外文件、有了 cli 就什麼都有」——D 弧落地=
rtl-spec bin(6 行 wrapper+--help 自攜 7 步配方+三份正樣可抄形)。此前 agent 用工具要穿
skills 文檔+scripts/*.mjs+MCP 三層,現在一個 CLI 到達。**教訓:工具的消費者是 agent 時,
可發現性與可預測性就是生產力本身。**

## 4. 已知未決(不藏)

- D-c5(等價裁決 X 語義盲區)/D-c6(空真探針 memory 名單盲區):方向保守不假放行,相關
  設計入場前須先修;
- **D-c7 行內賦值延遲抽象死鎖:政策待用戶裁**(能力邊界時等價腿是否不適用;涉已入庫設計
  TB 永久不可強化);
- E1 記錄文檔(build/mvp-exp/ 41 檔)晉升落點=默認維持 build/ 不入庫;
- ryosys equiv_* 核心實現=註冊里程碑(PLAN.md RY-EQUIV),禁 stub 過閘;
- 幀診斷=3 設計採樣、盲寫 spec→RTL=3 設計採樣,均非全量;
- tag v0.5.0/v0.6.0 未打(用戶定);
- i2c D 腿丟殺 8 個=D 腿異步復位觀測缺口(§20.8 觀察項,未處理)。

## 5. 下一隊列

**E agent 工作流**(agent 維護 RTL 的工作流,以 S5/conformance/spec-eval 評測床為前置——
床已齊):D 弧之後的正向閉環已通,E 弧要把「人寫 spec→agent 產 RTL→評測床驗收→反饋修」
閉成日常工作流,而不是單次採樣演示。啟動時點=用戶定。
