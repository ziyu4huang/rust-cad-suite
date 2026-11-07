# CONFORMANCE-PROTOCOL — D 腿定向 TB 協議釘檔(§19.3.4;C 閉環弧 C-S4;REIMPL-PROTOCOL 同形)

本檔=D 腿(LLM 定向 testbench)的協議釘檔:模型身份、採樣參數、模板逐字、腿表、機械執行。
D 腿產物=**凍結後**才被 conformance 閘消費;閘判的是「凍結 tb.v 這個工件」的機械可測屬性
(Q1..Q9+形族底線+不劣於基線,§19.4.5),**不是** LLM 的採樣過程或能力(§19.1 閘判據劃界,
R1-3)。LLM 腿永不進確定性閘(AGENTS 10.11);本檔記錄的腿表數字=輔助證據。
conformance 度量**行為復原力**,不是意圖正確性(§19.1)。append-only:任何改動=協議事件,記新
版本號/日期/理由,舊版保留。

## 0. 模型身份、採樣參數、執行日期、污染聲明(開弧釘檔;禁事後改)

- **模型=GLM-5.3-Flash**(與 C 閉環弧實施 agent 同一模型;§19.3.4 規劃輪模型亦同)。
- **採樣**:K=1(D 腿主 TB=K=1 樣本經 ≤3 輪改進後版本,§19.3.4 樣本選擇規則);K=2,3 本弧**未跑**
  (可選項,如實記)。溫度/seed 不可釘(agent 對話內生成)——樣本差異由 `{K}` 驅動,非 API 級隨機
  採樣(REIMPL-PROTOCOL §0 同款誠實邊界)。
- **執行日期**:2026-11-07。
- **污染聲明(盲隔離弱,如實記錄)**:TB 撰寫者=實施 agent 本身,同一 session 曾讀過原版源碼
  (i2c_master.v 端口與 cnt_end 行、fsm.v 全文、subbytes.v 片段、sincos/acc_pipe 錨定行)、看過變異
  探針打印的錨定行與變異文本,以及 C-S3 record.json 的存活清單。本工具環境無子代理(無法在乾淨上下文
  另起撰寫者),故**盲隔離弱於 REIMPL-PROTOCOL 的 E1 原始輪**:D 腿數字只能作「凍結工件的機械屬性」
  證據,**不得**解讀為「LLM 只憑規則書寫 TB 的能力」證據。泄漏核查只能證偽不能證真。
- **模板版本**:v1=RTL2SPEC-PLUGIN §19.3.4 預登記塊,下方逐字轉入。預登記**未提交,僅內容錨**:
  sha256 `cc28f0c73dc4ef3afc1838b839d44903d7dc9ee9b9f15b5ebd3aceaca532a941`(34 行;取法=§19.15 R1-10 的 awk/sed/shasum 管線,C-S4 開工實跑重算相等);不可抵賴
  時間戳=用戶提交本章的 commit 時刻(`git log -S` 定位)。下方圍欄內 34 行與 §19.3.4 逐位元組相同,
  scripts/conformance/dleg-prompt.mjs 每次實例化時重算並打印該 sha。

## 1-3. 模板(逐字;`{...}`=實例化槽位)

<!-- template-block v1 begin -->
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
<!-- template-block v1 end -->

樣本選擇規則(預先登記,防挑最好樣——MVP-NEXT 風險 4):D 腿主 TB=**K=1 樣本**經 ≤3 輪改進後的版本;
K=2,3 只作離散度報告(輔助)。

## 4. 腿表(實跑記錄;append-only)

| 腿 ID | 設計 | 類型 | 輪次 | K/N | 模型 | 日期 | 產物路徑 | 入庫否 | 數字(D 腿 killed/eligible;T 基線) |
|---|---|---|---|---|---|---|---|---|---|
| i2c-r0-k1 | i2c_master | tb | r0 | 1/1 | GLM-5.3-Flash | 2026-11-07 | build/conformance/dleg/i2c_master/tb_r0_k1.v | **是**(C-S4 --promote→fixtures/conformance/i2c_master/tb.v) | 201/255(T 77/255;gain +124;形族 floor guard 7/21 trans 9/9 seq 185/225;notWorse ✓) |
| acc-r0-k1 | acc_pipe | tb | r0 | 1/1 | GLM-5.3-Flash | 2026-11-07 | build/conformance/dleg/acc_pipe/tb_r0_k1.v | **是**(同上→acc_pipe/tb.v) | 10/10(T 10/10;gain 0=T 已滿;floor seq 5/5 rel 5/5;notWorse ✓) |
| fsm-r0-k1 | fsm | tb | r0 | 1/1 | GLM-5.3-Flash | 2026-11-07 | build/conformance/dleg/fsm/tb_r0_k1.v | **是**(同上→fsm/tb.v) | 42/42(T 31/42;gain +11;floor trans 10/10 seq 32/32;notWorse ✓) |
| sincos-r0-k1 | sincos | tb | r0 | 1/1 | GLM-5.3-Flash | 2026-11-07 | build/conformance/dleg/sincos/tb_r0_k1.v | 否(r1 取代) | 75/82(T 74/82;gain +1;guard floor 4/9) |
| sincos-r1-k1 | sincos | tb | r1 | 1/1 | GLM-5.3-Flash | 2026-11-07 | build/conformance/dleg/sincos/tb_r1_k1.v | 否(r2 取代) | 76/82(gain +2;guard 5/9) |
| sincos-r2-k1 | sincos | tb | r2(waive 修訂,無新殺=F2 停止) | 1/1 | GLM-5.3-Flash | 2026-11-07 | build/conformance/dleg/sincos/tb_r2_k1.v | **是**(C-S4 --promote→sincos/tb.v) | 76/82(T 74/82;gain +2;guard 5/9 seq 71/73;notWorse ✓) |

| fsm-rew-r1 | fsm | rewrite | r1(r0 4 場景失配→FIX) | 1/1 | GLM-5.3-Flash | 2026-11-07 | build/conformance/dleg/fsm/rewrite_r1_k1.v | 否(輔助腿,C-S5 F1) | conformance TB 6/6 場景全配(r0=1 行失配:SCENE5 打印 4 的 rb/gb 兩鍵錯[2026-11-08 終審 R5-5 勘誤:原記『2 行失配』實為 1 行 2 鍵,同步復位拍輸出慢一拍]) |
| sincos-rew-r1 | sincos | rewrite | r1(r0 249 行系統性失配→FIX) | 1/1 | GLM-5.3-Flash | 2026-11-07 | build/conformance/dleg/sincos/rewrite_r1_k1.v | 否(輔助腿,C-S5 F1) | conformance TB 6/6 場景全配(r0=done 載入後逐行系統性失配 249 行[2026-11-08 終審 R5-5 勘誤:原記『差 1』係 firstDiff;根因=末次微旋未折疊入輸出]) |

腿表備註(終審 R5-4 修訂,2026-11-08):四 D 腿 tb.v 的 COVERS/WAIVE 宣告收窄為「觀測映射」形
(場景→規則映射由入庫 record.json 殺傷矩陣復算:規則 r 被場景 s 觀測 ⇔ 歸屬 r 的 killed 變異以 s 為殺傷
場景;`--record --slice C-S6` 重錄,record-diff=expected 0 行差+變異結果全同,僅 tb.sha/宣告註釋變更;
fsm LOO 6/42→7/42=covers 次級排序輸入隨宣告收窄的誠實變化)。fsm SCENE1 實測只觀測 R007..R010(4 條
復位臂 guard),i2c SCENE1 只觀測 10 條,sincos SCENE1 只觀測 R002..R004——初版「SCENE1 打包全規則」的
宣告無觀測支撐;同時消除 sincos 的 COVERS∩WAIVE 矛盾(R005..R008 經 SCENE6 觀測改 COVERS,R035 維持
WAIVE;declaredCoverage sincos 45/46)。**原 C-S4 初版宣告的過寬形與本修訂如下差**:收窄不改變閘判據
(Q9/Q8/殺傷矩陣全同),只使聲明數字獲得分辨力。

腿表備註(C-S5 F1/F3 實跑,2026-11-07):①F1 演示=fsm/sincos 重寫腿(我撰寫,污染聲明沿 §0)+i2c 用在場
i2c-A1..A3/B1(全部 6/6 場景配);回灌機制實錄=fsm r0 兩行失配(SCENE5 同步復位拍)→TB first-diff 定位→r1
修復全配;sincos r0 done 載入差 1(逐周期探針歸因=原版單塊阻塞序使輸出載入含末次微旋)→r1 折疊修復全配;
②i2c-A2 在 D 腿 TB 下 6/6(C-S3 carryOver「SCENE3 失配 3/4」係 T 腿 TB 的 small 景,D 腿 TB 無此景——訂正);
③F3 報告=build/conformance/rulebook-defects.{json,md}(17 項:P-unruled killed 15+def_lint U1 cnt_end
[D-c1 確定性證據]+U2 政策排除 1;i2c 含 cnt_end 已為閘斷言);④留一法自驗(§19.6.4)rate 實測=acc 3/10、
fsm 6/42、i2c 8/201、signedexpr 7/10、sincos 9/76、subbytes 3/8(v1 無下限,數字照報;命中=真值規則集∩top3);
⑤診斷示例=build/conformance/diag/acc-A0.md(classification=rewrite-error,suspects top3=R001..R003)。

腿表備註(C-S4 實跑):①i2c/fsm r0 即大幅新殺(OP-STATE/OP-RVAL/OP-HOLD/OP-CONST 的定向序列),F2 一輪停止
(無進一步存活可觀測差異面);②sincos r1 新場景 async_reset_observe(殺 OP-ASYNC m0002=r0 相對 T 腿的丟殺回復)
+r2 WAIVE d.R005..R008/x-y-z-i 與 d.R035/dz(死存儲:start 分支覆寫/末迭代 dz 只影響內部 z,port 不可觀測);
③acc_pipe T 腿已 10/10,D 腿 gain=0(不劣於 ✓,無增益空間);④K=2,3 未跑(可選項,如實記);
⑤撰寫者污染聲明見 §0(盲隔離弱:D 腿數字只作凍結工件的機械屬性證據)。

## 5. 機械執行

- 實例化(確定性):`bun plugin/rtl2spec/scripts/conformance/dleg-prompt.mjs r0 <design> 1 1` 寫
  build/conformance/dleg/<design>/prompt_r0_k1.md(§1 模板;{{RULEBOOK_MD}}=golden rules.json 經 dist/md.mjs
  renderRulebookMarkdown 渲染[A 組口徑,含 evidence],{{TB_PLAN_JSON}}=dist wasm `rtl_tb_plan_sv` 的 plan,
  {{SKELETON_V}}=同幀 template_v);改進輪 `… rN <design> <prev-tb> <try-record.json> 1` 寫 prompt_rN_k1.md
  (§2 模板;存活清單=eligible survived 變異的歸屬規則,只列規則書內容,不給變異源碼行)。
- 撰寫:撰寫者讀 prompt 寫 build/conformance/dleg/<design>/tb_rN_k1.v(build/ gitignored;候選不入庫)。
- 候選評測(確定性腿,寫只落 build/conformance/try/):`bash plugin/rtl2spec/run-conformance-eval.sh --try
  <design> <tb 路徑>`——同一 Q1..Q9(Q9 用 directed 口徑:每條規則必 COVERS 或 WAIVE)+T 腿基線模板跑同一
  變異矩陣+形族底線+不劣於基線。
- 改進輪(F2,≤3 輪):候選未入庫門或仍有 eligible 存活 → 以 try record 生成 §2 prompt → 撰寫 r(N+1);
  停止=一輪無新殺或滿 3 輪。
- 入庫:case.json 改 `tb_leg: "directed"`+`protocolLeg: <腿 ID>`(斷言規格,手寫合法)→`bash
  plugin/rtl2spec/run-conformance-eval.sh --record --slice C-S4 --promote <design>=<候選路徑>`(鐵則+入庫門不過=
  拒錄;錄前快照+record-diff-C-S4.md:T 腿轉基線腿的逐變異對賬+D 腿新殺/丟殺)。不達=不入庫(留 build/
  作候選,數字照報)。降級(無採樣條件)=標「環境受限未跑」,C-S3 狀態不變。
- **入庫 tb.v 禁手改(TB-SHA-ALARM;收尾弧 S0-②,2026-11-07 協議事件)**:`fixtures/conformance/<design>/tb.v`
  是凍結工件,換檔唯一路徑=`--record --slice <C-Sn> --promote <design>=<候選路徑>`(T 腿=`--record` 從 tb_plan
  重生成)。機械執法=conformance-eval 的三方核對(錄前/入庫 record.tb.sha256、磁碟 tb.v、本輪主腿 TB):默認模式
  磁碟≠record=REJECTED(exit 1);`--record` 磁碟≠錄前 record=拒錄(exit 1;`--promote` 不洗白手改,先還原再換檔)、
  directed 腿無 `--promote` 且無錄前 record=拒錄、directed 腿無 `--promote` 時主腿 TB≠磁碟=拒錄、寫後磁碟≠新
  record=exit 1;`--try` 不觸 fixtures 不適用。註釋-only 手改同樣紅(實錄:修前對 fsm/tb.v 追加一行註釋默認模式
  exit 0——變異矩陣對註釋無感;修後紅)。

## 6. 模板 v2(協議事件;收尾弧 S0,2026-11-07;append-only,§0 與 v1 圍欄零改)

- **事件**:WAIVE 標籤集補 `unadjudicated`(§1 模板硬性要求第 2 條),其餘逐字同 v1。
- **理由**:終審 medium——i2c 11 條 WAIVE 標 `unobservable`,其 21 個變異實為 20 `unadjudicated`+1 `equivalent`
  (m0054/R042;R104 僅 m0178),「不可觀測」無 yosys 證明支撐;舊標籤集沒有「未證等價」的中性標籤可用。
  RTL2SPEC-PLUGIN §19.24 R5-4 的同處措辭同輪勘誤。
- **標籤語義**:`unobservable`=無 killed 且存活變異全部 equivalent(yosys 已證);`unadjudicated`=無 killed 且存在
  未證等價的存活變異(中性,不帶可/不可觀測方向);`x-valued`/`tb-internal`=靜態標籤(語義不變)。數據謂詞的機械
  交叉驗證落點=等價裁決腿(RTL2SPEC-FINISH-PLAN §3.5)。
- **既有腿影響**:凍結 tb.v 的 WAIVE tag 本事件不動(i2c 11 條+sincos R035 維持 `unobservable`);逐規則重裁=等價裁決
  數據落地後(方向不預設,換檔必走 `--promote`)。§4 腿表既有行的 prompt 均由 v1 實例化,如實保留;本事件起新實例化=v2。
- **硬同步**:dleg-prompt.mjs 一律取最高版本圍欄並打印其 sha(proto-template.mjs:圍欄完整+版本從 1 連續+每版內容
  錨重算相等,任一不符 exit 2,禁回退舊版);tblint.mjs `WAIVE_LABELS` 與本版枚舉逐集合相等,由 conformance-eval
  selfcheck 每次核對(含篡改塊/缺錨/舊版被動的負向量)。
- **內容錨**:v2 sha256 `104d13f1b07c5e465a63fe222c4617170547b83c46ed5c2dcb38707f7d64b02f`(34 行;取法同 v1=圍欄內各行 \n 連接+尾 \n);
  v1 錨 `cc28f0c73dc4ef3afc1838b839d44903d7dc9ee9b9f15b5ebd3aceaca532a941` 不變。機讀錨行(整行形,閘只認此形):

<!-- template-sha v1 cc28f0c73dc4ef3afc1838b839d44903d7dc9ee9b9f15b5ebd3aceaca532a941 -->
<!-- template-sha v2 104d13f1b07c5e465a63fe222c4617170547b83c46ed5c2dcb38707f7d64b02f -->

<!-- template-block v2 begin -->
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
   <x-valued|unobservable|unadjudicated|tb-internal>」宣告豁免。
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
<!-- template-block v2 end -->

## 7. 等價裁決腿(測試側 harness;收尾弧 S1,2026-11-07;append-only,§0..§6 零改)

oracle=yosys 0.70+179(RTL2SPEC_YOSYS→vendor-reference/→PATH),**僅測試用禁 ship**;只在 `--record` 模式跑,默認模式零依賴
(`RTL2SPEC_YOSYS=none` 可驗:默認 exit 0、`--record` 守衛 exit 2)。LLM 零參與,裁決結果是 record 的機械欄位。ryosys equiv_*
核心實現=已註冊里程碑,本弧不實現(registry 層 `milestone Y0` 硬錯不動;實作入口=PLAN.md)。

- **生產命令形(強形,逐字)**:`read_verilog <src>; rename <top> gold; read_verilog <mut-top-only>; rename <top> gate; proc; flatten; memory; async2sync; opt_clean; equiv_make gold gate equiv; hierarchy -top equiv; select -count t:$equiv; equiv_simple -seq 5; equiv_induct -seq 5; equiv_status`。`<mut-top-only>`=top-only 提取(行級狀態機,禁點星跨塊;定義數≠1 或無 endmodule=
  throw)。無 blacklist、無 stash;flatten 必須在列。路徑一律相對 repo 根,record 零機器路徑。
- **端口表(每設計一次,輸出位數=守衛錨)**:`read_verilog <src>; hierarchy -top <top>; proc; write_json <ports.json>`。
- **探針專用模板(唯一用途=製造空真,不屬生產管線,與生產形只差 equiv_make 一段)**:全名單取自
  `read_verilog <src>; hierarchy -top <top>; proc; flatten; write_json <names.json>`,裁決走 `read_verilog <src>; rename <top> gold; read_verilog <mut-top-only>; rename <top> gate; proc; flatten; memory; async2sync; opt_clean; equiv_make -blacklist <all-names.txt> gold gate equiv; hierarchy -top equiv; select -count t:$equiv; equiv_simple -seq 5; equiv_induct -seq 5; equiv_status`。擇此形不擇「gate 側輸出改名」:後者實測 `ERROR: Can't match gate port`(rc=1),造不出空真。
- **eqverdict 三值**(只對全腿存活的 P-rule 變異;現行 adjudication 兩值欄位保留,equivalent⇔eqverdict==equivalent,其餘=
  unadjudicated 保守入分母):
  - `equivalent`:proven 且 `select -count t:$equiv` 的 cells≥輸出端口位數合計;
  - `unproven`:有 unproven cells(中性;未證信號名寫 `unprovenCells` 診斷附註,不帶端口/行為語義);
  - `adjudication-failed(<reason>)`:reason∈empty(proven 但 cells<輸出位數,或 `No $equiv cells found`;永不判等)|no-sat-model
    (`No SAT model available`,`$pow` 類,flatten 不解)|yosys-error(其餘 ERROR,首行入 eqDetail)|timeout|oracle-absent|no-count
    |count-mismatch(select -count≠equiv_status 的 Found N)|parse。
- **空真守衛口徑(R3 訂正鏈)**:輸出端口位數,不是全端口位數(組合設計 cells 恰=輸出位數,signedexpr 18/18 臨界通過;全端口 23
  會誤判)。numbersOf 把 equivalent 併入 controlSet 恒真,Q8 接不住空真,本守衛是唯一防線。守衛是必要條件不是充分條件:只判
  「證明結構已建立」,不判每個輸出都被覆蓋;生產形無 blacklist,端口不匹配時 equiv_make 直接報錯,故未另設逐位核對。
- **偵測力斷言(`--record`,任一不過=拒錄)**:①gold-vs-gold 帶守衛(先於逐變異);②正樣 fsm m0045 OP-NOP→equivalent;③反樣
  i2c m0001(golden killed)→非 equivalent;④空真探針→adjudication-failed(empty);⑤killed 全母體(P-rule 任一腿殺)強形裁決,
  斷言 equivalent==0;⑥存量 equivalent 帶守衛重驗,任一翻變列名。
- **默認模式 eq-verify(不跑 yosys)**:eqMeta/eqverdict schema 完整性+counts 一致性+equivalent 的 cells≥輸出位數+命令模板==生產形
  +waiveTable 與重算相等;eqMeta 缺=紅(過渡窗 S0→S1 已於本切片全集重錄後關閉)。
- **雙向措辭**:proven≠無條件行為等價(弱等價(-seq 5)+flatten/async2sync/memory_map 抽象),unproven≠不等價;eqMeta、證據頁、§20 一律如實。
- **WAIVE↔record 交叉驗證(併入 Q9)**:數據標籤必==數據謂詞——`unobservable`⇔無 killed 且存活變異全部 equivalent;`unadjudicated`
  ⇔無 killed 且存在未證等價的存活變異;任一腿殺=observable、無存活變異=no-data,兩者都不得標數據標籤。靜態標籤(x-valued/
  tb-internal)不判、只可見化(record-diff 強制打印每設計每標籤使用數變化量,新增靜態標籤須在切片記錄理由;合法 `--promote` 換檔後
  計數隨 record 更新不會紅,可見化+理由記錄是實際防線)。
- **真實行形(錨定正則,selfcheck 釘死)**:`N objects.`(select -count)、`Found N $equiv cells in equiv:`、`  Of those cells P are
  proven and U are unproven.`、`  Equivalence successfully proven!`、`  Unproven $equiv <cell>: \<sig>_gold [i] \<sig>_gate [i]`、
  `Found a total of U unproven $equiv cells.`、`No $equiv cells found in equiv.`;敵意面=`Found N unproven $equiv cells (K groups) in
  equiv:`、`Found N unproven $equiv cells in module equiv:`、`No selected unproven $equiv cells found in equiv.`。方案 R3 曾記
  「equiv_status 在 proven 時不輸出 cells 總數」,本機 0.70+179 實測有輸出 `Found N`;cells 計數仍走 select -count(方案指定),兩者相等
  作交叉斷言,不等=count-mismatch。
- **實跑記錄(--slice C-S7,2026-11-07;數字=record/SUMMARY-EQ)**:端口 all/out=acc_pipe 33/16、fsm 8/4、i2c_master 49/16、signedexpr
  23/18、sincos 64/41、subbytes 86/42;gold-vs-gold 六設計全 equivalent(cells 108/40/57/18/107/175);空真探針六設計全
  adjudication-failed(empty)。裁決 84=equivalent 27+unproven 57+adjudication-failed 0;killed 全母體 355(主腿 347)=unproven 354+failed
  1+equivalent 0,失敗項=i2c m0011 OP-POL@71(proc 報 `Async reset \rst_n yields non-constant value 1'm for signal \ack_rx.`)。
  **訂正**:RTL2SPEC-FINISH-PLAN §1 記該項為「$pow 類」不確;實測是 proc 前端對反極性異步復位形報錯,不是 SAT 模型缺失。`$pow`
  邊界另以最小構造實測(`assign y = a ** b;` gold-vs-gold 報 `No SAT model available`,rc=1);六設計源碼無 `**`。
- **WAIVE 重裁(數據驅動,方向未預設)**:首次全集 `--record` 按交叉驗證拒錄(i2c 11 條+sincos d.R035,標 unobservable 而數據=
  unadjudicated;逐條見 conformance-report.md「WAIVE 逐規則」表)。12 條全部改標 `unadjudicated`;無規則升 unobservable(R042 含已證
  m0054 與未證 m0055,仍非全 equivalent)。換檔走 `--record --slice C-S7 --promote`:候選由機械腳本按實測重寫 WAIVE 行,非新 LLM
  樣本,腿 ID 沿用 i2c-r0-k1/sincos-r2-k1;i2c tb.sha d25bc41d2f56→2f0d3422e2fe、sincos 61303cf9333f→e1342900930e,其餘四設計不變。
  對賬:六設計 expected.txt 0 行差;剝新增欄後 record 結構差異只有 tb.simMs(另 i2c/sincos 的 tb.sha256);adjudication 逐變異 0 變更。

## 8. 波形幀診斷腿(輔助證據;收尾弧 S2,2026-11-07;append-only,§0..§7 零改)

「LLM 看視頻要剪一幀」的落地:counterexample VCD → vl-vcd `vcd2svg` 統一窗 SVG → sips/qlmanage 柵格化 PNG → 視覺 LLM 讀幀出結構化診斷。
**永不進確定性閘**(AGENTS 10.11):機械段 `bash plugin/rtl2spec/run-frame-diag.sh` 不屬總閘;驗證器只驗工件機械屬性,不驗 LLM 內容;
診斷不回寫 golden/期望/tb.v。度量行為差異的可見性,不是設計正確性(§19.1)。SVG 才是交付形(ADR-0005),PNG 只是查看手段;vl-vcd 渲染
契約本弧不動。

- **VCD 產生(tb.v 零改)**:build 側 wavecap 第二 top(`$dumpvars(0, tb_<top>.<inst>)`,必須層次實例路徑,實例名取自 tb.v)。寫面只落
  build/conformance/{frames,diag-vision}/。**零擾動斷言**:帶探針跑原版的輸出==入庫 expected.txt、帶探針跑變異的殺傷場景==record.killScenes,
  不等=機械段紅。counterexample 缺省=record 主腿首個 killed 的 P-rule 變異(確定性選取),`--mutant <design>=<id>` 可換。
- **構建守衛=拒跑形**:target/debug/vcd2svg 缺或 `find crates/vl-vcd/src -newer` 非空 → exit 2+指引 `cargo build -p vl-vcd --bin vcd2svg`;腿內
  不構建(寫面約束)。
- **統一時間窗**:TMAX=max(兩 VCD t_max),t_max 取自 vl-vcd 默認窗 SVG 的時間軸末標籤(不手搓 VCD parser);兩幀一律 `--from 0 --to TMAX`。
  各自默認窗會產生軸縮放偽差(sincos 兩 VCD t_max 不等)。
- **程序化剝 title**:`/<title>[\s\S]*?<\/title>/g`。esc 保留 \t\n\r(render.rs:95),字串型值的 title 可跨行,**禁逐行 sed**:剝不淨會誤判「畫布
  不同」→偽放行 vision。title 元素 XML 不可嵌套,非貪婪必停在最近閉標籤;selfcheck 配跨行 title 敵意向量。
- **modality 判定(統一窗雙檢,先於任何 LLM 讀幀)**:主判=剝 title 後 SVG 畫布 byte 比較;輔判=兩幀 PNG sha256。兩者皆異才
  modalityAllowed=vision;主判相同→強制 text-fallback,limitations 記「畫布層不可分」。值只在 title(render.rs:18-19),所以「只改數值、不改沿」
  的差異畫布上本來就看不到。
- **剪幀(展示裁剪,不參與判定)**:只在 vision 可行時產出;窗=title 層首分歧點前 1/16、後 1/8 全窗,`--signals`=時鐘+前 8 個分歧信號。
- **歸因不靠幀**:titleLayer(逐信號首分歧 tick 與值,讀 SVG title 文本)與 stdoutLayer(diagnose.mjs 同款)都是機械歸因。TB 協議不打印模擬
  時間,tick↔SCENE 無機械對應,如實聲明。
- **工件**:frames/<design>/{counterexample.json,gold.png,mutant.png,gold_zoom.png,mutant_zoom.png,svg/,work/};diag-vision/<design>.prompt.md
  (vision 可行時)或 diag-vision/<design>.json(強制 fallback 時的機械診斷,producedBy 註明未經 LLM,framesRead=0)。幀重產=舊診斷作廢。
- **diag schema**:design、mutantId、modality(vision|text-fallback)、frames(實讀 PNG 路徑)、framesRead、findings、confidence(high|medium|low)、
  limitations、verdict。
- **驗證器(`run-frame-diag.sh --validate`,機械,不驗內容)**:frames 每項存在+PNG 魔數;framesRead==frames.length(diag 未列 frames 時,vision 取
  manifest 全部幀、text-fallback 取空);modality=vision ⇒ 雙檢皆異且含兩張主幀;text-fallback ⇒ limitations 非空;confidence 三值。方案 §4.4 寫
  「vision ⇔ 雙檢皆異」,反向落成「雙檢皆異而標 text-fallback 須說明原因」:視覺會話不可用是合法降級,不該紅。驗證器紅=幀腿工件紅,不影響任何閘。
- **降級形**:鏈上任一步不可得(iverilog/vcd2svg/柵格化全缺/視覺會話不可用/雙檢不過)→text-fallback(讀 SVG 文本+counterexample JSON),framesRead
  按實際如實標。
- **污染聲明**:counterexample.json 含變異行與機械歸因,讀幀者不是盲的;診斷只能作「幀上可見什麼」的輔助證據,不得解讀為「LLM 憑波形獨立定因
  的能力」。findings 的 hypothesis 取自 mutant 欄須標「已知變異」。

<!-- vision-prompt v1 begin -->
```text
[§8 波形幀診斷模板]
你是波形診斷員。下面是設計 {DESIGN} 的同一 testbench 在參考實現(gold)與待診斷實現(mutant)下的波形幀:統一時間窗,畫布只有沿與帶形,
數值不在畫布上(在 SVG title 裡)。*_zoom 是分歧點附近的剪幀,窗與信號子集見 counterexample 的 zoom 欄。
請逐張讀 PNG:
{FRAMES}
再讀機械摘要 {COUNTEREXAMPLE}(titleLayer/stdoutLayer 是機械歸因,mutant 欄是已知答案)。
把診斷寫到 {OUT},JSON 欄位:design、mutantId、modality、frames(實讀 PNG 路徑)、framesRead(=frames 長度;SVG/JSON 不計)、
findings(每條 {signal,tickWindow,observation,hypothesis})、confidence(high|medium|low)、limitations、verdict(一句)。
誠實條款:
1. modality=vision 只在你確實讀了上列 PNG 時成立;沒讀圖=text-fallback,並在 limitations 寫原因。
2. observation 只寫你從幀上看到的(哪一行信號、哪段時間、沿的有無或錯位);數值只有 title 層才有,引用時標「title 層」,不得說成從幀上看到。
3. hypothesis 來自 mutant 欄時標「已知變異」,自己推斷的標「推斷」。看不清或幀對定因無幫助就降 confidence,並寫進 limitations。
4. 診斷是輔助證據,不是正確性判定;不得修改任何其他檔案。
```
<!-- vision-prompt v1 end -->

- **腿表(實跑記錄;append-only;模型=GLM-5.3-Flash,與實施 agent 同一模型,溫度/seed 不可釘)**:

| 腿 ID | 設計 | counterexample | 模型 | 日期 | framesRead | modality | confidence | 產物(gitignored) |
|---|---|---|---|---|---|---|---|---|
| fsm-vision-1 | fsm | m0001 OP-STATE@33(`state <= 200`→`300`) | GLM-5.3-Flash | 2026-11-07 | 4 | vision | medium | build/conformance/diag-vision/fsm.json |
| sincos-vision-1 | sincos | m0001 OP-RVAL@55(`state = 1'b0`→`1'b1`) | GLM-5.3-Flash | 2026-11-07 | 4 | vision | medium | build/conformance/diag-vision/sincos.json |
| subbytes-fb-1 | subbytes | m0001 OP-RVAL@31 | 機械,未經 LLM | 2026-11-07 | 0 | text-fallback | low | build/conformance/diag-vision/subbytes.json |
| signedexpr-fb-1 | signedexpr | m0001 OP-CONST@9 | 機械,未經 LLM | 2026-11-07 | 0 | text-fallback | low | build/conformance/diag-vision/signedexpr.json |

- **雙檢實測(統一窗,六設計,缺省 counterexample 皆 m0001;`bash plugin/rtl2spec/run-frame-diag.sh`)**:

| 設計 | t_max gold/mutant | 畫布異 | PNG 異 | title 層分歧信號 | modalityAllowed |
|---|---|---|---|---|---|
| acc_pipe | 640000/640000 | 是 | 是 | 9 | vision(診斷未跑,不在 frameDemo) |
| fsm | 880000/880000 | 是 | 是 | 5 | vision |
| i2c_master | 21980000/21500000 | 是 | 是 | 26 | vision(診斷未跑,不在 frameDemo) |
| signedexpr | 288000/288000 | 否 | 否 | 1 | text-fallback |
| sincos | 3770000/3680000 | 是 | 是 | 15 | vision |
| subbytes | 1040000/1040000 | 否 | 否 | 4 | text-fallback |

- **實跑備註**:讀幀的目讀數字事後用 title 層核對,兩處初讀有誤已訂正並記入各診斷的 crossCheck 欄:fsm 的 green_a 回低記成約 205000,實為 195000;
  sincos 把 done 偏移讀成從 50000 累積到 90000,title 層顯示是前 15 次固定 50000、末次 90000。幀能看出「哪些信號、大致哪一段、形狀怎麼變」,
  讀不準精確 tick 和偏移規律——所以歸因不靠幀,診斷只作輔助證據。兩份 vision 診斷都不是盲讀(見污染聲明),confidence 取 medium。

## 9. T 腿全量擴面(收尾弧 S3,2026-11-07;append-only,§0..§8 零改)

本節是 S3 擴面的協議記錄與名單 SSOT(conformance-eval.mjs 的 EXPECTED_DESIGNS 註釋指向本節)。度量仍是行為復原力(§19.1),T 腿
=tb_plan 模板 TB,不涉 LLM。

- **候選池(勘察同口徑復刻)**:build/eval(run-realworld-eval 產物)中規則數≥1 且非 TB 檔=85(L1 66/L2 19)。文件級單 top 取
  `rulebook.modules[0]`,得 TB 化 57/拒收 28,分布 no_output_port 15/frontend_parse_errors 4/parametric_width 7/rulebook_truncated 1/
  reset_not_input 1,與規劃勘察逐值相同;57 全屬 L1(時序 31/組合 26),sv-tests 零入選,§16.7-2 不觸發。對照:top 取核心默認
  (module 參數空串)只得 30,差額 31 是 multi_module 拒收,屬口徑差,不是能力差;兩口徑禁混用。掃描腳本與結果在 build/finish-s3/
  (scan2.mjs/scan2.json)。
- **staging 機制**:`--fixtures-root <dir>` 只收 plugin/rtl2spec/build/ 內已存在目錄(否則 exit 2);staging 的 outRoot=
  build/conformance-staging/out/,證據頁寫 staging-report.md,主 fixtures 與主證據頁零觸碰;EXPECTED_DESIGNS 與 F3 斷言只對主根;
  ADMITTED 判定讀 staging verdict.json,不看 exit(含拒收設計時整批拒錄 exit 1,verdict.json 仍照記)。`--slice` 放寬為
  `C-Sn[-批次]`(C-S8-pre/C-S8-1/C-S8-2)。
- **預篩與 case.json**:51 個新候選(57 減既有 6)由 build/finish-s3/prescreen.mjs 生成 staging case.json(斷言規格,手寫合法):
  top=rulebook.modules[0];tb_leg=template;iverilog 先試 -g2005,有錯或有警告再試 -g2012。實測 46 個 -g2005 零警告;generate/
  hierdefparam/mem2reg_bounds_tern/paramods/values 共 5 個帶 DUT 警告,-g2012 也不能消,照 -g2005 寫入由 Q1 拒收。
- **staging 預錄(`--record --slice C-S8-pre`,5:21)**:ADMITTED 28(原已釘 golden 16/未釘 12;時序 18/組合 10),REJECTED 23。拒收
  原因類(可多選):Q8 鐵則 15、Q7 跨模擬器 13、Q4 X 值 8、Q1 警告 5、EQ gold-vs-gold 2、EQ 空真探針 1、EQ killed 全母體 1。
  逐設計見下表。
- **golden 擴檔(升級裁定)**:12 個未釘 ADMITTED 全部入釘選集,pinned-files.list 37→49,golden 74→98 份;入選依據改為
  「staging ADMITTED=TB 實殺變異的直接證據」,取代 §16.1.1 的「<3 條規則無斷言價值」代理判據;§16.1.1 表的更新欠到 S4 §20,欠條
  與清償閘在 RTL2SPEC-FINISH-PLAN §11。`run-golden-regression.sh --record` 實跑:既有 74 份 sha 逐值不變,新增 24 份,
  PROVENANCE 只改標題/時間/wasm sha 與追加錄製行;新 12 份 rules.json 與 staging 生成的 spec 逐位元組相等。
- **主根新斷言**:主根設計的 specPath 必須在 plugin/rtl2spec/golden/ 下且 src∈釘選清單(staging 不斷言);golden-check 的
  EXPECTED_PINNED=49 精確數斷言(只對正式清單)。
- **分批全集重錄**:
  - 批 1 `--record --slice C-S8-1`(5:20):+10=P0 原已釘 ADMITTED(add3、aes_kexp128、always03、dff_different_styles、dff_init、
    fiedler-cooley、forloops、i2c_master_tests、omsp_dbg_uart、spec_i2c_master);16/16 ADMITTED;既有 6 個剝 tb.simMs 後逐位元組一致。
    批 1 後默認模式 86.0s(16 設計)。方案寫「P0 前 12」,實取 10:其餘 P0 ADMITTED 需 golden 擴檔,等裁定後併入批 2。
  - 批 2 `--record --slice C-S8-2`(6:39):+18=新釘 12(always01、always02、attrib01_module、attrib02_port_decl、attrib06_operator_suffix、
    attrib08_mod_inst、attrib09_case、graphtest、loop_prefix_case、retime、scopes、usb_phy_tests)+P1 原已釘 6(fulladd、loops、partsel、
    realexpr、vloghammer、wreduce);34/34 ADMITTED;既有 16 個剝 tb.simMs 後逐位元組一致。
- **數字(主 SUMMARY;S3 前→後)**:designs 6→34;mutants 439→863;eligible 412→794;killed 347→544;equivalent 27→69;
  unruled_killed 15→84。SUMMARY-EQ:adjudicated 84→311、equivalent 27→69、unproven 57→242、adjudication_failed 0→0;killed 全母體
  355→552(主腿 347→544)=unproven 354→549、failed 1→3、equivalent 0→0。規則書缺陷報告(rulebook-defects)17→97 項。證據頁
  conformance-report.md 100→279 行(只增不刪:刪 0 增 179)。
- **時長(實跑,重校準)**:默認模式 34 設計 3:08(逐設計均值 5.5s,最大 i2c_master 17.1s);方案外推 57 設計 6-10 分鐘,實際收編
  34 個,未觸發 P0/P1 分層或 `--quick` 降級;`--record` 6:39(i2c_master≈110s、spec_i2c_master≈101s 主導,含 yosys 裁決)。
- **判別力鐵律**:新收 28 個全部照現行機械門(Q1..Q9+Q8 鐵則+等價裁決六斷言)過關,未降門;stillborn/無殺傷的候選一律留在拒收表。
- **觀察(不阻塞本切片,已交接)**:
  - operators 只因 EQ gold-vs-gold 失敗被拒:源碼有 `**`,yosys 報 `No SAT model available`($pow),TB 側 41/125 已殺。現行規則是
    gold-vs-gold 不過即拒錄,本切片不放寬;「能力邊界時降級為等價腿不適用、不拒設計」屬政策變更,需另行裁定。
  - constmuldivmod:killed 全母體自檢抓到 3 個「TB 殺得掉卻被 yosys 證 equivalent」的變異(m0003 `A % 8'd0`→`A % 8'd1`、m0033、
    m0064 同形)。除零在模擬是 x,yosys 把未定義當任意值,故證等價——proven≠無條件行為等價的實例。該設計另因 Q4/Q7 被拒;交接給
    等價裁決腿的 eqverdict 分類面(REGISTRY D-c5)。
  - arrays01:空真探針名單取自未跑 memory pass 的 write_json,記憶體映射後的 FF 名不在名單內,探針留下 64 個 unproven 而非空真。
    方向保守(拒設計,不會假放行);另因 Q4/Q7 被拒。memory 類設計入場前需修探針名單(REGISTRY D-c6)。

逐設計 staging 判定(51 個;REJECTED 23 個即拒收登記):

| 設計 | 層/優先 | 原釘選 | 判定 | 殺傷率(staging) | 拒收原因類(staging C-S8-pre) |
|---|---|---|---|---|---|
| add3 | L1/P0 | 是 | ADMITTED | 7/7 | — |
| aes_kexp128 | L1/P0 | 是 | ADMITTED | 8/8 | — |
| always01 | L1/P0 | 否 | ADMITTED | 2/2 | — |
| always02 | L1/P0 | 否 | ADMITTED | 4/4 | — |
| always03 | L1/P0 | 是 | ADMITTED | 7/7 | — |
| arraycells | L1/P1 | 否 | REJECTED | 0/0 | EQ gold-vs-gold;Q8 鐵則(P-rule 零 killed) |
| arrays01 | L1/P0 | 否 | REJECTED | 2/2 | Q4 X 值;Q7 跨模擬器;EQ 空真探針 |
| attrib01_module | L1/P0 | 否 | ADMITTED | 2/3 | — |
| attrib02_port_decl | L1/P0 | 否 | ADMITTED | 2/3 | — |
| attrib04_net_var | L1/P0 | 是 | REJECTED | 3/6 | Q4 X 值;Q7 跨模擬器 |
| attrib06_operator_suffix | L1/P0 | 否 | ADMITTED | 3/4 | — |
| attrib08_mod_inst | L1/P0 | 否 | ADMITTED | 2/3 | — |
| attrib09_case | L1/P1 | 否 | ADMITTED | 4/4 | — |
| constmuldivmod | L1/P1 | 是 | REJECTED | 52/54 | Q4 X 值;Q7 跨模擬器;EQ killed 全母體 |
| dff_different_styles | L1/P0 | 是 | ADMITTED | 1/1 | — |
| dff_init | L1/P0 | 是 | ADMITTED | 1/1 | — |
| dynslice | L1/P0 | 否 | REJECTED | 0/0 | Q4 X 值;Q7 跨模擬器;Q8 鐵則(P-rule 零 killed) |
| fiedler-cooley | L1/P0 | 是 | ADMITTED | 14/14 | — |
| forloops | L1/P0 | 是 | ADMITTED | 5/5 | — |
| fulladd | L1/P1 | 是 | ADMITTED | 2/2 | — |
| func_width_scope | L1/P1 | 否 | REJECTED | 0/0 | Q7 跨模擬器;Q8 鐵則(P-rule 零 killed) |
| generate | L1/P0 | 是 | REJECTED | 5/5 | Q1 編譯警告/失敗 |
| graphtest | L1/P1 | 否 | ADMITTED | 7/7 | — |
| hierdefparam | L1/P1 | 否 | REJECTED | 0/0 | Q1 編譯警告/失敗;Q7 跨模擬器;Q8 鐵則(P-rule 零 killed) |
| i2c_master_tests | L1/P0 | 是 | ADMITTED | 3/5 | — |
| loop_prefix_case | L1/P1 | 否 | ADMITTED | 2/2 | — |
| loop_var_shadow | L1/P1 | 否 | REJECTED | 0/0 | Q8 鐵則(P-rule 零 killed) |
| loops | L1/P1 | 是 | ADMITTED | 13/13 | — |
| macros | L1/P1 | 是 | REJECTED | 0/0 | Q8 鐵則(P-rule 零 killed) |
| mem2reg | L1/P1 | 是 | REJECTED | 0/0 | Q4 X 值;Q7 跨模擬器;Q8 鐵則(P-rule 零 killed) |
| mem2reg_bounds_tern | L1/P0 | 否 | REJECTED | 3/3 | Q1 編譯警告/失敗;Q4 X 值;Q7 跨模擬器 |
| memory | L1/P0 | 是 | REJECTED | 4/4 | Q7 跨模擬器 |
| module_scope_case | L1/P1 | 否 | REJECTED | 0/0 | Q8 鐵則(P-rule 零 killed) |
| omsp_dbg_uart | L1/P0 | 是 | ADMITTED | 5/5 | — |
| operators | L1/P0 | 是 | REJECTED | 41/125 | EQ gold-vs-gold |
| paramods | L1/P1 | 否 | REJECTED | 0/0 | Q1 編譯警告/失敗;Q8 鐵則(P-rule 零 killed) |
| partsel | L1/P1 | 是 | ADMITTED | 1/1 | — |
| process | L1/P1 | 是 | REJECTED | 0/0 | Q8 鐵則(P-rule 零 killed) |
| realexpr | L1/P1 | 是 | ADMITTED | 6/6 | — |
| retime | L1/P0 | 否 | ADMITTED | 2/2 | — |
| rotate | L1/P1 | 否 | REJECTED | 0/0 | Q8 鐵則(P-rule 零 killed) |
| scopes | L1/P1 | 否 | ADMITTED | 1/1 | — |
| signed_full_slice | L1/P1 | 否 | REJECTED | 0/0 | Q8 鐵則(P-rule 零 killed) |
| spec_i2c_master | L1/P0 | 是 | ADMITTED | 77/255 | — |
| task_func | L1/P0 | 是 | REJECTED | 5/5 | Q4 X 值;Q7 跨模擬器 |
| undef_eqx_nex | L1/P1 | 是 | REJECTED | 0/1 | Q4 X 值;Q7 跨模擬器;Q8 鐵則(P-rule 零 killed) |
| usb_phy_tests | L1/P0 | 否 | ADMITTED | 10/11 | — |
| values | L1/P1 | 否 | REJECTED | 0/0 | Q1 編譯警告/失敗;Q7 跨模擬器;Q8 鐵則(P-rule 零 killed) |
| vloghammer | L1/P1 | 是 | ADMITTED | 2/2 | — |
| wandwor | L1/P1 | 否 | REJECTED | 0/0 | Q7 跨模擬器;Q8 鐵則(P-rule 零 killed) |
| wreduce | L1/P1 | 是 | ADMITTED | 4/4 | — |
