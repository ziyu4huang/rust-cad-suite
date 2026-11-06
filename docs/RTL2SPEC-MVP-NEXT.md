# rtl2spec MVP-NEXT:復原力 MVP(修訂版 R2)決策文檔

- 日期:2026-11-05。基線:rtl2spec v0.2.0(HEAD 200b1c7,工作樹 39 路徑未提交主體+本檔)。
- 本檔為本弧**唯一新增文件**(重寫本弧同日初稿——初稿缺五個必含章節標題詞,本版按規定結構重排;未觸任何其他既有檔),弧內禁 commit。
- 章節按機械閘要求含:「網調綜述」(§1)、「實驗記錄」(§2)、「MVP 提案」(§4)、「切片與驗收」(§5)、「風險與不做」(§6)。
- provenance 標記約定:**【本輪實跑】**=本輪當場執行的命令與輸出;**【檔】**=本輪讀取的檔案及行號;**【材料】**=任務輸入給定的材料(提案 JSON/實驗 JSON/網調 A/網調 B/能力簡報);行號引用均經本輪讀碼核實(核實清單見 §8)。

---

## 0. 目標與北極星

### 0.1 北極星操作化

北極星(用戶原話,【材料】):**從 RTL 程式範例中,取得正確的規格書,讓人類可以做高級的工作,不用再去親自寫 RTL code,只要讓 agent 寫就好。**拆成兩個可度量命題:

1. **規格書夠正確** = agent 只拿規則書能復原設計(可觀察行為對拍原版)=學界 RTL-Reconstruction Score 語義(§1.4,arXiv 2512.00045)。
2. **人只做高級工作** = 人的介入點收斂為「評審規則書/裁決破口清單」,不寫 RTL、不修提取器。

E1(§2)是命題 1 的**能力上界探針**而非嚴格度量【修訂:初稿稱「第一次實測」,依 §2.5 三重稀釋降格】——梯度 6/6、0/6、10/11 把復原阻斷點定位到切片級(N2/N3/N4 各有代碼現場與機械驗收面),但 matched 絕對值受抄寫通道/先驗飽和/非機械盲隔離稀釋;嚴格度量由 S5 重建協議後按切片分段歸因給出。阻斷點按實測排序:契約層為零(N2,3/3 設計全中)>組合覆蓋缺口(N3,acc_pipe 全滅直接死因)>復位語義不可承載(N4,i2c 唯一行為失配)>規則靜默丟失(N1,69 條總數掩蓋信息損失)>分支條件家族(M1/M2/M3,E1 對拍未觀察到直接失配,危害證據=audit 現形+metamorphic 同型,§4.3 R2)。

**結論一句話**:下一輪 MVP = **R1「復原力 MVP」**——修掉 E1 實測復原阻斷點(契約層為零 N2/組合覆蓋缺口 N3/復位語義不可承載 N4)+同輪搭車收平規則完整性家族(M1/M2/M3/N1)+以協議重建方式固化復原對拍評測(prompt 釘檔+證據塊消融+機械主門檻+採樣判定)——機械面驗收為主門檻,盲寫重跑降為輔助證據。

### 0.2 對抗審 12 條消化對照(初稿→本修訂版)

| # | 級 | 意見 | 處置 |
|---|---|---|---|
| 1 | high | S6 指紋內嵌名稱/RHS 文本,「≥20 檔 0 IF」照做必敗;原文比斷言語義非工件指紋 | **採納重寫**:S6 改「歸一化結構比較探針」——先落名稱歸一化+AST 規範化比較層,0 IF 判據作用於歸一化摘要集;AST 無 Paren 節點(`crates/vl-syntax/src/ast.rs:445-530` 【本輪實勘】枚舉=Ident/HierIdent/HierDot/NumLit/RealLit/StrLit/Unary/Binary/Ternary/Concat/Delay3/Repeat/Index/PartSelect/IndexedPart/Call,`Paren` 檢索 0 命中),加括號臂 parser 消費即消失。見 §5 S6。 |
| 2 | med | E1 證據力三重稀釋(證據塊攜源碼行/小設計先驗飽和/盲隔離非機械) | **採納**:§2.5 逐項實勘補錄;E1 定位降為「能力上界探針」(§0.1);R2 落選證據中「M1-M3 非主因」推斷降級;S5 加證據塊消融組。 |
| 3 | med | S5 對照組/實驗組 prompt/模型不同源 | **採納**:S5 加「協議釘檔」前置(prompt 模板逐字入檔+模型身份+採樣參數;本輪實勘 `rg -l "prompt" plugin/rtl2spec/build/mvp-exp/` **exit 1 零命中**——盲寫確未存檔 prompt)。 |
| 4 | med | S1/S3 驗收落單發 LLM 產物會 flake | **採納**:機械面驗收(contract 節逐字對拍/async 欄位/when 互斥/golden diff)升為**主門檻**;盲寫重跑降為輔助證據,帶採樣判定(n≥3 中 ≥2 記 PASS,否則如實報失敗率,永不進確定性閘)。 |
| 5 | med | acc_pipe 0/6 死因消融缺失(N2/N3 同時作用無歸因點) | **採納**:S1/S2 各加 acc_pipe 盲寫重跑採樣點——S1 後記 compiled+matched(歸因 N2),S2 後記 matched 上浮(歸因 N3);0/6→? 曲線按切片分段歸因。 |
| 6 | med | R4(D 落選)VerilogEval-revisit 引用超強度 | **採納訂正**:降格為檢索摘要級引用並訂正實文結論措辞(「prompt engineering remains crucial…varies widely with model and task」,基準設施動機);D 落選主理由改為 E1 實測+reggen 工業現實。 |
| 7 | med | S4 fixture 落點在 gitignored 區+gap-fixtures 閘清單邊界;E1 證據鏈無晉升計劃 | **採納**:S4 正負樣遷 `plugin/rtl2spec/fixtures/`(自著 MIT)並進 gap-fixture-check 清單(【本輪實勘】`plugin/rtl2spec/scripts/gap-fixture-check.mjs:35-38` 現認 `crates/rtl-spec/tests/fixtures/gaps_positive.v`/`gaps_negative.v`+`plugin/rtl2spec/fixtures/gap_missing_reset_no_arm.v`/`gap_hang_repro.v` 四前綴);S5 加證據晉升清單(記錄文檔晉升落點由用戶裁決)。 |
| 8 | low | SpecLoop 數字漂移(35-43% vs 實文 34.6-45.2%) | **採納訂正**(網調弧 WebFetch 實抓原文 Fig.3):Verified RR=1 四模型 81.3/88.7/85.5/91%,Unverified 34.6/45/45.2/42.8%(區間 34.6-45.2%);DeepSeek-V3.1 不在 Fig.3(其 0.940/0.865 屬 VerilogEval 任務表,網調 A 口徑)。見 §1.4。 |
| 9 | low | S1 驗收路徑錯位(57_xls_add3 非 E1 設計;counter8 在 corpus/counter8/) | **採納訂正**:S1 驗收路徑=`corpus/counter8/counter.v`+`corpus/e2e/58_xls_acc_pipe/acc_pipe.v`+`corpus/e2e/59_i2c_master/i2c_master.v`(另有 `crates/spec-i2c/rtl/i2c_master.v` 副本,validation 集兩份都釘;四檔【本輪實勘 ls】全在場);add3 從驗收清單移除。 |
| 10 | low | R3/R4/R6 生態判斷無 URL/來源強度 | **採納補注**:SemiWiki=論壇級來源(警示價值保留、依據強度降級);reggen 補 `github.com/lowRISC/opentitan`;RTLExplain 補 MLCAD 2025/IBM Research(§1 表 URL 保留)。 |
| 11 | low | sequencing「兩次重錄代價」與「每步獨立重錄」自相矛盾 | **採納改寫**:重錄次數在禁攢批鐵律下不隨拆弧變化,搭車論據改為「同一語義面聚合」(§7.2)。 |
| 12 | low | sequencing M8 無文件清單 | **採納+本輪擴勘**:任務稱「三處正文殘文」,初稿 grep 僅定位 2 處;**本輪 grep(`grep -n "10s\|192"`)命中 10 行(PLUGIN.md 9 行+eval-all.mjs 1 行),逐行讀上下文分類=殘文候選 4 行+歷史記載保留 6 行**(§7.2 表逐行列,計數口徑=命中行,4+6=10 可對帳)。 |

---

## 1. 網調綜述:RTL↔spec 雙向生態現狀

lens A=「从 RTL 反推规格/性质」的方法论现状;lens B=「agent/LLM 写 RTL」生态现状。兩輪網調共 **28 條 findings**(A 15+B 13;其中 B4=SpecLoop 與 A1 同文,兩 lens 各計一檔 finding,證據強度按同一份原文計)。下表逐條保留 URL 與 provenance;「抓取」欄三值=**原文**(網調弧 WebFetch 實抓全文)/ **abstract 頁**(僅抓 arXiv abstract 頁,非全文)/ **引述**(據檢索結果摘要或 provenance 標注[評測文/新聞稿],未抓原頁——引用時以此為限)。relevance 為網調弧評定。

### 1.1 Lens A:RTL→spec/性質反推方法論(15 條)

| # | 工作(URL) | 要點 | provenance | 抓取 | rel |
|---|---|---|---|---|---|
| A1 | SpecLoop — https://arxiv.org/abs/2603.02895 | LLM 產結構化 NL spec(禁含 Verilog)→LLM 重建 RTL→Yosys EQY 等價(depth 10)→counterexample VCD 全量診斷回灌(≤3 輪);DeepSeek-V3.1 RR 0.940 vs 單輪 0.865(VerilogEval 任務表);Fig.3 Verified RR=1 約 81.3% vs Unverified 34.6%(Qwen3-480B 口徑,核數見 §1.4) | 學術 2026(arXiv) | 原文 | high |
| A2 | Assessing LLMs in Generating RTL Design Specifications — https://arxiv.org/html/2512.00045v1 | VerilogEval-V2/RTLLM-2.0 反向跑 RTL→spec;RTL-Reconstruction Score(oracle LLM 重建過 TB,pass@1)區分度最高(46-98%);token 相似度(BLEU/ROUGE/BERTScore)最不可靠;GPT-RTL Score 能抓 async vs sync reset 類錯誤 | 學術 2025(arXiv) | 原文 | high |
| A3 | SVAEval — https://repositum.tuwien.at/handle/20.500.12708/230567 | 無 golden RTL/參考 property 集亦可評測 SVA 生成器(經仿真/形式驗證落地) | 學術 2026(FMCAD 2026,TU Wien) | 摘要 | high |
| A4 | metamorphic 魯棒性 — https://arxiv.org/html/2609.05658v1 | 語義保持變換(重排/重命名/加括號)後 9.7-27% 原本正確 behavior 翻錯;人工歸因=丟 path predicate/分支極性錯/布爾結構破壞;指標 CR/IF/any-flip 比的是**斷言語義正確性**(需 golden/仿真判定) | 學術 2025(arXiv) | 原文 | high |
| A5 | AssertLLM — https://arxiv.org/abs/2411.14436 | spec 全文(NL 句子+波形圖)→結構化模板→定制 LLM 產 SVA;ASPDAC'25 報告優於 GPT-4o/3.5 | 學術 2025(ASPDAC) | abstract | high |
| A6 | AssertMiner — https://arxiv.org/html/2511.10007v1 | RTL AST→模組調用圖/IO 表/數據流→LLM 推斷模組級 spec→模板化 SVA;指標 NN/SS/PP+錐內覆蓋(BFC/SFC/TFC)+突變殺滅+5 次重跑相似度>95%;I2C BFC 82.79% 但 PP 錯誤率 17-54%——LLM 生性質必須過形式工具 | 學術 2025(arXiv) | 原文 | high |
| A7 | Large Lemma Miners — https://arxiv.org/html/2511.02521v1 | LLM 提議引理→FindIndStrength 用 EBMC 1-induction+BMC(bound 30)篩歸納加強集;94 個 (design,property) 對解出 68;LLM 產出零信任——「生成→形式判真偽」的標準形 | 學術 2025(arXiv) | 原文 | high |
| A8 | AutoSVA — https://github.com/PrincetonUniversity/AutoSVA | 端口注解語言→自動生成模組間 safety/liveness FV TB;已用於真實處理器模組;輕量注解→機器展開全部下游資產的最小先例 | 學術 2021 開源(Princeton;arXiv 2104.04003) | 摘要 | high |
| A9 | Spec2RTL-Agent — https://arxiv.org/abs/2506.13905 | 非結構化 NL spec→多 agent 產 RTL;主指標=人類干預次數最多 -75%;評測僅 3 份 spec 文檔,樣本極小 | 工業研究 2025(NVIDIA) | abstract | high |
| A10 | OpenTitan reggen — https://opentitan.org/book/util/reggen/index.html(repo=github.com/lowRISC/opentitan) | HJSON 寄存器描述單一真源→HTML 文檔+reg_pkg/reg_top SV+C 頭+DV 常量全生成——寄存器子域「spec 是源、RTL 是產物」已是工業現實 | 工業文檔/開源(現行) | 原文 | high |
| A11 | 斷言挖掘 survey — https://hal.science/hal-05085439v1/file/Survey_on_Automatic_Assertion_Miners.pdf | GoldMine(DATE 2010,動態 trace 挖掘)→A-TEAM;比較口徑=coverage+compactness;經典路線依賴仿真 trace | 學術 2010/2025(Iman survey) | 摘要(PDF 未直抓) | medium |
| A12 | Daikon — https://plse.cs.washington.edu/daikon | 動態不變量檢測祖師;教訓=數量爆炸、假陽性多、必須再過濾 | 學術 2007(SCP/華盛頓大學) | 摘要 | medium |
| A13 | 突變測試篩 significant spec — https://www.semanticscholar.org/paper/79bc52f283abeb8052e12e2c2f6cde317be4cec3 | 一條規格「有用」的操作定義=能殺死某些突變體;檢出率即規則有用性分數 | 學術(軟件工程) | 摘要 | medium |
| A14 | sphinx-systemverilog — https://github.com/fvutils/sphinx-systemverilog(同生態 hdl-diagrams 用 Yosys 產框圖) | HDL autodoc 生態停在結構級;pyslang=現成 SV parser 參照 | 開源工具(fvutils/SymbiFlow,現行) | 摘要 | medium |
| A15 | Revisiting VerilogEval — https://arxiv.org/html/2408.11053v1 | 材料摘要:0-shot→3-shot 約 5%→35%;**實文核心結論(訂正後口徑)**=「prompt engineering remains crucial…varies widely with model and task」(基準設施動機),非「上游瓶頸」定論 | 學術 2024(arXiv) | 摘要 | medium |

### 1.2 Lens B:agent/LLM 寫 RTL 生態(13 條)

| # | 工作(URL) | 要點 | provenance | 抓取 | rel |
|---|---|---|---|---|---|
| B1 | VerilogEval v2 — https://arxiv.org/html/2408.11053v2 | 156 題轉 spec-to-RTL+失敗自動分類;2024 末前沿 pass@1 58-63%;結論=單輪生成近飽和(加 TB/調試資源的 agent 流 94-95%),重心移向驗證與規格 | 學術 2025(ACM TODAES+NVIDIA) | 摘要 | high |
| B2 | RTLLM 2.0 — https://arxiv.org/html/2503.15112v1(repo hkust-zhiyao/RTLLM) | 50 設計×NL spec+testbench+golden RTL 三元組——spec↔RTL 雙向對拍的現成評測基建,不必自造 golden | 學術 2025(HKUST,ICCAD 2024 DV Workshop) | 摘要 | high |
| B3 | CVDP — https://arxiv.org/abs/2506.14074(repo NVlabs/cvdp_benchmark) | 783 題 13 類;**specification alignment 首次列一級類別**;最強模型 pass@1 僅約 34%,agentic 任務最難——RTL↔spec 一致性已成公認評測維度 | 學術 2025-06(NVIDIA Labs) | 摘要 | high |
| B4 | SpecLoop(同 A1) — https://arxiv.org/html/2603.02895v1 | 定性案例恰為「同步 reset 被誤判異步→counterexample 糾正」——與本弧 N4/M3 失敗類同構;全程 LLM、非確定性、無獨立可機檢產物 | 學術 2026-03 | 原文 | high |
| B5 | FIXME 基準 — https://arxiv.org/html/2507.04276v1 | 180 任務×6 子域端到端;測試用例設計(TD)7.72%=最弱環;其 Agent5 做語義保持規格轉換——spec→測試資產是全鏈最脆一環 | 學術 2025-07 | 摘要 | high |
| B6 | AssertLLM2 — https://arxiv.org/html/2605.27472v1 | 83 真實設計(均值 ~2888 LOC)評測 spec→SVA:語法正確率>90% 但 bug kill 僅 ~10-12%(union ~20%);**結構化 spec 顯著穩於原始 PDF**——規格書機器可檢結構化形態的價值被實證放大 | 學術 2026-05(HKUST) | 摘要 | high |
| B7 | Cadence/ChipStack RTL Generation Agent — https://www.wevolver.com/article/spec-to-rtl-ai-agent | 工業 spec-first=spec→C++→HLS→RTL+雙 RL 閉環(功能/PPA 檢查為閘);聲稱面積 -24%/功耗 -18%、功能 27/27 vs 8/27(未加權平均、無人工 golden、波動大)——工業界投票:驗證器當獎勵信號源 | 工業 2026-09(Cadence 生態) | 引述(評測文,未抓原頁) | high |
| B8 | Siemens Questa One Agentic Toolkit — https://news.siemens.com/en-us/questa-one-agentic-ai-toolkit | 五 agent 在售:NL→RTL Code Agent+驗證計劃 Agent(工程師逐步審核)+Lint/CDC/Debug;**MCP 已是 EDA agent 標準接口面**(與 plugin/rtl2spec MCP stdio 同構) | 工業 2026-02(Siemens 新聞稿) | 引述(新聞稿,未抓原頁) | high |
| B9 | ChipAgents/形式驗證反思 — https://semiwiki.com/eda/chipagents-ai/371335-rethinking-formal-verification-in-the-ai-era | RTL 是證據不是意圖——純 RTL 派生 property 的形式驗證=循環論證;Intel 案例 FV 反哺澄清 78/104 條需求、需求派生斷言只暴露 80% bug——閉環度量行為復原力≠意圖正確性 | 工業博客 2025(**論壇級,強度注記**) | 摘要 | high |
| B10 | EstRTL — https://arxiv.org/abs/2606.09867 | 估計 agent 把 RTL 反轉成 NL 需求按三分數靜態評分(不跑仿真)決定放行/重生成/糾正,正確性 +3.2-9.0%——RTL↔spec 一致性作無仿真靜態信號進 agent loop 被驗證可行 | 學術 2026-06 | abstract | medium |
| B11 | MG-Verilog — https://arxiv.org/abs/2407.01910 | 多粒度 spec/RTL 配對數據集,被 ICCAD LLM4HWDesign 競賽 Phase II 用作目標——確定性 RTL→規則書工具天然是配對數據規模製造機 | 學術 2024-07+競賽生態 | abstract | medium |
| B12 | DeepAssert/AssertCoder — https://arxiv.org/html/2509.14668v1 | RTL→模組級規格在別家只是斷言生成的**內部中間步**,不面向人評審或 agent 復用——生態空位的直接證據 | 學術 2025 | 摘要 | medium |
| B13 | RTLExplain — https://research.ibm.com/publications/rtlexplain-a-structured-approach-to-rtl-code-summarization-and-question-answering-for-medium-to-large-designs-using-llms--1 | IBM 遺留 RTL 摘要+QA(MLCAD 2025),服務「讀懂既有代碼」但不產結構化可驗證可回歸的規格工件——維護場景真實需求,現有解法不可審計 | 工業研究 2025(IBM Research) | 摘要 | medium |

### 1.3 綜合與生態空位

- **兩條方法族**:經典動態 trace 挖掘(Daikon/GoldMine 族,依賴仿真軌跡與 coverage/compactness)vs 新 LLM 管線(靜態結構或 spec 文檔餵 LLM,產 NL spec/SVA;A1/A2/A5/A6 為代表)——後者已成主流,且與本倉「已有 AST、無仿真集成」的處境天然匹配。
- **兩大缺口恰好對著 rtl2spec 的位置**:①spec 質量評測無 golden oracle——學界正用重建對拍(RR Score)、metamorphic 穩定性、突變殺滅、golden-free 判據(SVAEval)補齊,四套皆可機械化,現行 R1-R5 定性複評可升級為這些判據;②「行為轉寫」與「意圖級規格」之間沒有人交付**確定性、帶溯源、帶測試缺口清單、可機檢可回歸**的規格書工件(A12/B12/B13 只做內部中間步或摘要)——byte-exact golden+破口 fixture 矩陣這套工程紀律在檢索到的所有工作中都無對應物,這是可占的差異化生態位。
- **正確性閉環的邊際成本最低路徑**:SpecLoop(Yosys EQY 回灌)與 Large Lemma Miners(EBMC 判真偽)已驗證「形式工具當 spec oracle」,倉內 ryosys(Yosys 等價家族)在手。
- **可偷四件**:①RR Score(規則書→agent 重建→與原 RTL 對拍,直接度量北極星命題 1);②metamorphic 語義保持變換穩定性(其人工審出的「丟 path predicate/分支極性錯」與本弧 M1-M3 分支條件家族同型,可同時當收平度量);③AssertMiner 的 NN/SS/PP+錐內覆蓋+突變殺滅+重跑相似度指標組;④Spec2RTL-Agent 的「人類干預次數」(命題 2 的直接量化)。

### 1.4 學界判據對本弧的直接含義(含核數)

- **SpecLoop Fig.3 核數**(網調弧 WebFetch 實抓原文 HTML):Verified RR=1 四模型 81.3/88.7/85.5/91%,Unverified 34.6/45/45.2/42.8%——區間 **81-91% vs 34.6-45.2%**;DeepSeek-V3.1 不在 Fig.3,其 RR 0.940 vs 0.865 屬 VerilogEval 任務表。含義:閉環(驗證回灌)是放大器不是修復器——增益依賴回灌診斷質量,對 R3 落選理由與 S5 範圍劃定(只固化確定性對拍,回灌屬 C 全量化)成立。
- **RR Score 語義**(A2):「規格書質量=agent 靠它復原設計」是學界公認判據,E1/S5 的盲寫對拍即其本弧形態;GPT-RTL Score 抓 async vs sync reset 類錯誤的能力與本弧 N4(S3)直接對應。
- **metamorphic 判據遷移警告**(A4+對抗審 #1):原文 CR/IF/any-flip 比的是斷言語義正確性(需 golden/仿真判定),**不是工件指紋集**——指紋 subject()/effects_text() 內嵌信號名與 RHS 渲染文本(`schema.rs:216`/`:230-239` 【本輪實勘】),重命名/重排必變指紋,字節指紋集不變判據照搬必敗;S6 已按此重寫為歸一化結構比較探針(§5)。
- **循環論證警告**(B9,論壇級來源、與 Intel FV 案例同向):閉環度量行為復原力≠意圖正確性;rtl2spec 的正確角色=回歸錨+破口清單(G1..G8 把歧義/缺口顯式交人裁決),宣稱時分層。

### 1.5 證據強度注記(與 §1.1/§1.2 抓取欄逐條對賬)

按抓取形態三分,28 條 findings=6+2+20:

- **原文全文抓取(6 項)**:A1 SpecLoop、A2 2512.00045、A4 metamorphic、A6 AssertMiner、A7 Large Lemma Miners、A10 OpenTitan reggen——網調弧 WebFetch 實抓全文 HTML/PDF(B4 與 A1 同文,歸同一份原文)。
- **abstract 頁抓取(2 項)**:A5 AssertLLM、A9 Spec2RTL-Agent——抓的是 arXiv abstract 頁,**非全文**;結論以 abstract 聲明範圍為限(A9 樣本僅 3 份 spec 的注記即來自 abstract 自述)。
- **未抓原文(20 項)**:A3/A8/A11-A15(7 項,檢索摘要級,兩個 PDF 因格式未直抓)+B1-B6/B9-B13(11 項,檢索摘要級)+B7/B8(2 項,provenance 指向 Wevolver 評測文/Siemens 新聞稿,未抓原頁)。

**本輪未重抓任何網調原文**(§8),引用時以標注強度為限;上列分組與 §1.1/§1.2 表「抓取」欄逐條一致。

---

## 2. 實驗記錄:E1 盲寫復原對拍 + E2 人級評審

### 2.1 方法

- **樣本**:3 設計複雜度階梯 18/44/316 行——`corpus/counter8/counter.v`/`corpus/e2e/58_xls_acc_pipe/acc_pipe.v`/`corpus/e2e/59_i2c_master/i2c_master.v`【本輪實勘 wc -l=18/44/316,三檔+`crates/spec-i2c/rtl/i2c_master.v` 副本在場】。
- **備料**:經插件真實 MCP stdio 提取規則書(`bun build/mvp-exp/scripts/extract.mjs <rtl> <out.md>`;spawn `bun dist/server.mjs`→initialize→tools/call `rtl_extract_rules`+`rtl_scan_gaps`;serverInfo 實測 {name:rtl2spec,version:0.2.0}【檔 experiment-record.md :9-10】)。產物:`build/mvp-exp/specs/` 3 份,設計映射=**i2c_master 48118 字節(48KB)/acc_pipe 3312(3.3KB)/counter8 790(0.8KB)**【本輪實勘 ls;規模與行數/規則數同向:316 行→69 條→48KB,18 行→2 條→0.8KB】。
- **E1 盲寫對拍**:盲寫 agent 只拿規則書產 `reimpl/<design>/top.v`(盲隔離靠提示詞約束,非機械,§2.5);行為對拍=iverilog 13.0(`/opt/homebrew/bin/iverilog`;verilator 5.052 在場未用【檔 e1/README-e1.md §0】)+公共 TB(6/6/11=23 場景)+wrapper 適配,逐場景 diff。
- **E2 人級評審**:獨立評審按五維度(interfaces/behavior/timingReset/reimpl/highLevel)打分+probe 追問,再對原版源碼實勘裁定 gaps 真偽。
- **TB 自身方法學**:兩輪 bug 修復(task input 調用時一次性求值致假超時/SCENE5 從機位相;SCENE5 決策=從機全釋放,放棄「從機驅動數據」維度——該維度已由 WR ACK 場景覆蓋)【檔 README-e1.md §2.4】。

### 2.2 E1 數據表【檔 experiment-record.md :29-35+README-e1.md §2】

| 設計(行數) | compiled | 場景 | matched | 死因/要點 |
|---|---|---|---|---|
| counter8(18) | true | 6 | **6/6** | diff 零差異;SCENE5 環繞實測 `fd fe ff 00 01`;WIDTH=8 靠檔名暗示碰對(非契約) |
| acc_pipe(44) | true | 6 | **0/6** | 全場景失配。代表值(原始輸出【本輪實勘 diff_accpipe.txt】):SCENE2 穩態 0024(36)→0030(48);SCENE3 0384(900)→02b0(688);SCENE4 05fa(1530)→01f8(504);SCENE1 X 模式差(0Xxx vs 0XxX)。**死因**:R005/R007 消費的 p1_bit_slice_103_comb/p2_scaled_comb 無定義規則(has_binary 閘擋 partsel/拼接)→盲寫版 A4 猜低 WIDTH 位 vs 原版 `[8:1]` 右移(`acc_pipe.v:22` 【本輪實勘】)+A5 左移 vs `{5'h00,·,p0_add[0]}` 拼接;另有接口位寬警告 x/y 期望 32 位 vs 實給 8、out 34 vs 16(README-e1.md §2.2 逐字) |
| i2c_master(316) | **false** | 11 | **10/11**(top_fixed.v 僅補 1 行 `reg [2:0] state` 聲明後;原盲寫產物未動) | 編譯失敗=top.v:59/91 state 未聲明(逐字錯誤存 err_i2c_re.txt);唯一行為失配=SCENE10 中途復位:原版異步(`always @(posedge clk or negedge rst_n)`,`i2c_master.v:70` 【本輪實勘】)vs 盲寫同步——規則書 R001 無法承載敏感列表(N4)。亮點:START/STOP 條件、WR 0x55/0xAA 從機回讀、ack_rx 採樣、done 單拍脈衝、HOLD 四臂優先序(SCENE9 排隊)、reSTART 後 rd_mode 殘留移幽靈字節 0xff 的**原版 quirk 同形復現** |

> SCENE4 記法勘誤:任務材料與 experiment-record.md :32 記「1530→504」(原版終值 vs 盲寫終值,正確);README-e1.md §2.2 記「1528 mod 1024=504」(盲寫版內部中間值取模,算術自洽但 1528 非觀測終值);**「1530 mod 1024=504」的算式寫法不成立**(1530 mod 1024=506)。以原始輸出為準:orig=0x05fa(1530)、reimpl=0x01f8(504)【本輪實勘】。

### 2.3 規則清單核對與洩漏核查

- **規則清單核對**【檔 experiment-record.md :39-43+README-e1.md §3】:i2c 抽 19/69→18 條行為 matched(R001 復位族敏感列表缺失=代表性失配);acc_pipe 抽 7/7(全集)→時序骨架/assign 結構 matched,兩條組合信號無定義規則=全滅根因;counter8 抽 2/2(全集)→全 matched。
- **洩漏核查(只能證偽不能證真)**:
  - i2c:盲寫版 top.v:253 逐字含原版 i2c_master.v:272 行內註釋——**非憑空洩漏**:規則書 R028 證據塊攜帶該原文。**三方對照【本輪實勘】**:`specs/i2c_master.rulebook.md` :829/:876/:923/:970 四行、原版 `i2c_master.v:272`、盲寫版 `reimpl/i2c_master/top.v:253` 三方同為 `sda_oe <= 1'b1;  // SDA back high for the restart sequence`。含義見 §2.5 ①。
  - acc_pipe:A4/A5 兩個猜測方向與原版真值均不同(取低位 vs 右移;左移 vs 拼接)——**猜測不等於洩漏**,反證盲隔離有效。
  - counter8:結構逐字同形源於規則書 check 行有據(trigger=clk posedge/when=rst/effects=q←…);WIDTH=8 是聲明假設(檔名暗示非契約)。
  - cmd 編碼/cnt_end 閾值等規格書外信息全部走 assumptions 聲明;state 未聲明=機械遺漏非洩漏。

### 2.4 E2 數據表與 gaps 裁定【材料·實驗 JSON;裁定過程【檔 experiment-record.md :45-60】】

評分向量(五維度,獨立評審):

| 設計 | interfaces | behavior | timingReset | reimpl | highLevel | verdict 要點 |
|---|---|---|---|---|---|---|
| i2c_master | 2 | 3 | **2**(降:復位敏感列表無法承載,E1 SCENE10 實錘) | 2 | 3 | FSM 骨架與命令閂鎖鏈忠實可復現(fixed 後 10/11 含原版 quirk 同形),但接口契約整缺、復位語義錯、timing 規則因指紋 when 維缺失不完整——夠結構級評審,不夠獨立重實現與復位語義驗收 |
| acc_pipe | 2 | 2 | 3 | **1**(0/6 全錯實錘後評分調整,experiment-record.md :56) | 2 | 評審判斷全面正確,重實現完全失敗——兩條被消費組合信號與位寬鏈是硬洞,規格書撐不起重實現與端到端驗收 |
| counter8 | 2 | 4 | 4 | 4 | 3 | 核心行為可獨立復原(6/6),評審三大 probe 全對;接口契約缺省與覆蓋自證是殘餘缺口——小設計規則書已近夠用 |

**probe 裁定**(評審問→實勘結論):

- i2c:①cnt_end 定義推不出=正確(真缺;原版真相 `wire cnt_end = (cnt >= prescale - 1);` `i2c_master.v:68` 【本輪實勘】,prescale 是 16 位輸入端口,declaration 不 spawn 規則);②「R030 真實 guard 丟失」=**半誤判**——原版 93 行 done<=0 就是无条件序言,guard 本不存在非丟失;評審「單拍脈衝+末寫勝出」推斷被 TB 實測確認(done_fall high=1,兩版 matched);③開漏式合成推斷正確(與 `i2c_master.v:17-18` split open-drain 註釋一致【本輪實勘】),但「仲裁/拉伸無處理痕跡」=**半誤判**——`i2c_master.v:11-13` 明確聲明 no arbitration/no clock stretching 是**有意簡化**【本輪實勘】,規格書視角推不出但非疏漏。
- acc_pipe:三 probe 全對(bit_slice 對哪個信號切片算不出/初值不可推導(兩版同 X 起,SCENE1 模式差)/p1_add→p0_add 位寬截斷無法判定)。
- counter8:三 probe 全對(同步復位高有效 SCENE2/4 實測/環繞回 0 SCENE5 實測/WIDTH 缺省未陳述+無使能,後者置信中——依賴「規則書=全部行為」覆蓋假設,本案實勘成立)。

**gaps 分類**(真缺/誤判/半真):

- **真缺(實勘確認)**:i2c=cnt_end 定義、端口契約(scl_i/prescale[15:0]/cmd 4 位 onehot——盲寫版無 scl_i/prescale、cmd 2 位)、引腳合成、行為窮盡性;acc_pipe=端口表整份缺失、p1_bit_slice_103_comb 無定義規則、p2_scaled_comb 無定義規則、全部位寬未給、無復位/初值語義、行為窮盡性未證(證據僅 7 行,22-27/30-34/36-40 區間 2 條 assign 無規則)、端到端黑盒驗收寫不出、時序收斂信息缺失(超範疇但真);counter8=端口/WIDTH 缺省、上電初值/4-state、具體數字驗收。
- **誤判(可駁回)**:counter8「覆蓋契約未自證」——原版 18 行實勘僅 1 個 always+聲明無第二 process,TB 6/6 佐證;對任意未知檔哲學上成立,本案可實勘駁回一半。
- **半真**:i2c「命令接受與排隊語義確認不了」——規則鏈 R031-R035+R046-R048+R025-R028+R064-R068 已完整承載(TB SCENE9 matched);單命令容量與 START/STOP 合併語義確無規則(源自註釋非代碼)。
- **成立+機制歸因**:「規則列表不可作完整行為清單」——N1 timing 指紋 when 維為空(R056 指紋=`…|timing|i2c_master|tip||1'b1`,fsm 類 R025 有 when 維【本輪實勘 grep 指紋行 :1289】)→全檔同 subject|effects 的 timing 規則被首見吞併,ST_HOLD 四臂 tip<=1/bitcnt<=0 等條件賦值無規則(只在證據塊可見);69 條總數掩蓋信息損失,V5 指紋閘只查重不查丟失=閘盲區。

### 2.5 證據力三重稀釋(限制聲明)【修訂;對抗審 #2】

1. **證據塊攜帶逐字源碼行(含註釋)**——三方對照實錘(§2.3,【本輪實勘】)。含義:i2c 復原的部分通道可能是「從 evidence 塊抄寫」而非「從規則文本推理」——E1 對 i2c 的 matched 數**不能全額記給規則書語義**;S5 以證據塊消融組實測抄寫通道貢獻(§5 S5)。
2. **小設計 LLM 先驗飽和**——counter8(18 行同步復位計數器)是高頻樣式,6/6 可能主要是先驗功勞;「counter8 帶 M3 現形仍 6/6→M1-M3 非主因」的推斷建立在此飽和樣本上(n=3 階梯),**不能據此排除 M1-M3 危害**(R2 落選證據已隨之降級,§4.3)。
3. **盲隔離非機械**——盲隔離靠提示詞約束(盲寫 agent 自報 source not seen),無機械隔離;洩漏核查只能證偽不能證真。**本輪實勘**:`rg -l "prompt" plugin/rtl2spec/build/mvp-exp/` exit 1 零命中——盲寫 prompt 模板與模型身份均未存檔,對照組/實驗組協議同源性無法追溯(S5 協議釘檔的直接動機)。

另加 TB 覆蓋度限制(【檔 README-e1.md §5】):未覆蓋 case 非法編碼(原版 default→IDLE vs 盲寫版無 default 臂——M2 現形的對拍驗證缺失)、RD 從機驅動數據(模型能力邊界)、4-state 輸入組合;wrapper 適配(cmd onehot→2 位/prescale→T_HALF/WIDTH=8/out 零擴展)由對拍方補齊規格書缺口信息——接口差異在 TB 層被「洗掉」,只能靠警告文本與 assumptions 留證。

**結論**:E1 可信部分=三個阻斷點(N2/N3/N4)實測存在且機械可驗(acc_pipe 位寬警告 32 vs 8、無規則信號名、i2c compiled=false 都是確定性事實);不可全信部分=matched 絕對值。headline 度量(0/6→?)由 S5 重建協議後按切片分段歸因給出——**E1 是能力上界探針,不是基線度量**。

### 2.6 記錄底座指針

- 總記錄:`plugin/rtl2spec/build/mvp-exp/experiment-record.md`(75 行,六階段:備料→盲寫→E1 對拍→規則清單→E2 裁定→N1-N5 新發現+產物清單)【本輪全文讀取】。
- 逐命令證據:`plugin/rtl2spec/build/mvp-exp/e1/README-e1.md`(§0 環境/§1 裝備/§2 逐設計實測/§2.4 TB 調試/§3 規則清單/§4 洩漏核查/§5 方學限制)。
- 產物:specs/ 3 份規則書、audit/ 5 份缺陷現形證據、reimpl/ 3 份盲寫產物(含 top_fixed.v)、e1/ 全部 TB/wrapper/sim 輸出/diff/err 文本【本輪實勘 find 41 檔】。**以上全住 gitignored 區,弧後清理即丟**(§6 風險 8)。

---

## 3. 能力現狀與差距(v0.2.0)

### 3.1 三閘基線(全部【本輪實跑】,exit 0)

| 閘 | 命令 | 本輪輸出(節錄) |
|---|---|---|
| golden 回歸 | `bash plugin/rtl2spec/run-golden-regression.sh` | `== golden-check: 74/74 byte-diff PASS, PROVENANCE 核對 PASS ==` |
| 評測擴面 | `bun plugin/rtl2spec/scripts/eval-all.mjs` | `== eval-all: 37 design-rules / 117 sparse / 193 skip-perr / 6 skip-tb / trap=0 / 指紋 672 unique(dup=0) / 90ms ==`+`GATE PASS`(L1=93 L2=260 全集=353) |
| 13 檔定性 | 逐檔 `bun plugin/rtl2spec/scripts/check-rulebook.mjs <file>`(run-rtl-validation.sh 的驗證集清單,13 檔;不跑該閘本身,理由見 §8) | per-file OK=13/13;verdict JSON 匯總(【本輪實跑】bun -e 統計 build/verdicts/ 13 檔)=**Σrules=325、行為占比 100.0%、categories=arithmetic/counter/fsm/reset/timing** |

- **193 skip-perr 本輪再證 M8 登記方向**(文檔舊數 192≠實跑;殘文清單見 §7.2)。
- 閘寫入面核實【本輪實勘讀碼】:golden-check.mjs 默認分支寫只落 `build/golden-current/`(:199-213);eval-all.mjs 寫只落 `build/eval/`(:122 outDir);check-rulebook.mjs 寫只落 `build/verdicts/`(:163-166)——全部 gitignored。**閘後核驗**:dist wasm sha256 前後均為 `4b143d3a…f3cd`(pkg fresh,六依賴目錄無 .rs 新於 pkg——【本輪實勘】),`git status --short` 計數 39 不變,golden/、validation-report.md、RTL2SPEC-EVAL.md mtime 未動。
- `run-rtl-validation.sh` 整腳本未跑:其 ④ 步 `gen-report.mjs` 重寫既有 `validation-report.md`(63KB),違反本弧禁改邊界——以逐檔 check-rulebook+verdict 匯總替代,如實注記(§8)。V5「325/325 指紋零重複」由兩處間接承載:eval-all 全量指紋 672 unique(dup=0)+13/13 per-file failures 判據;validation 閘自身 V5 計數未單獨復算。

### 3.2 M1-M3+組合常量盲區(audit 現形;【檔 audit/ 產物+experiment-record.md :20】)

- **M1 多標簽臂丟標簽**:自著最小樣 `audit/m1_min_case.v`(源 `2'b00, 2'b01: y <= 1'b1;`)提取出 R003 anchor=`s == 2'b00`——2'b01 觸發域丟失。代碼現場:`crates/rtl-spec/src/sv/mod.rs:210` `if let Some(first) = labels.first()`【本輪實勘】。
- **M2 default 臂零承載**:`default: y <= 1'b1;` 在規則書中無任何規則。機制鏈:walk_stmt 對 Default 臂不推條件(`sv/mod.rs:218` `CaseItem::Default(s) => walk_stmt(s, conds, out)`【本輪實勘】)→與首臂同 RHS 時指紋全同被去重吞併→淨效果=default 安全回退轉移不可見。
- **M3 復位 else 臂丟 `!(rst)`**:`sv/mod.rs:536-538` `continuation.push(&**e)` 裸 push(對照:組合路徑 :200-203 的 else 臂有 `!(cond)` 包裹【本輪實勘】)——muxtree.v R003 `when=!rst` state←0 與 R004 state←next_state `when=-` 並存,字面並讀 rst=1 拍既復位又裝載=自相矛盾;counter8 真實規則書同形(R001 when=rst/R002 when=-)。
- **附帶發現(非 M1-M3 家族)**:`always @*` 組合塊對純常量 RHS 臂 0 規則產出(muxtree default_cases 模組整塊 0 條;`sv/mod.rs:444` `if expr::has_binary(a.rhs)` 閘【本輪實勘;experiment-record.md :20 記 :449 為備料時點行號,本輪核實現行為 :444】)。
- 抽查插曲:sv-tests 5.12-attributes-case.sv(真多標簽臂)parse_error=1(SV 構造超 1364 解析器)、corpus/e2e/26_fsm_casez/tb.v 被 tb_file 政策跳過——缺陷現形樣本因此改走 yosys-simple/muxtree.v(in-git,ISC COPYING 實證)+自著最小樣【檔 experiment-record.md :20】。

### 3.3 N1-N5 阻斷點與代碼現場(E1 直接死因;行號全部【本輪實勘】)

| # | 缺口 | E1 實錘 | 代碼現場 |
|---|---|---|---|
| N1 | timing 指紋 when 維缺失→條件賦值被首見吞併 | i2c R056 指紋 when 空,HOLD/IDLE 臂規則丟失;V5 只查重不查丟失 | 指紋構成=`source|category|module|subject|when|effects` 六欄拼接(`sv/mod.rs:292-302`+`schema.rs:262-264`【本輪實勘】);subject()/effects_text() 內嵌名稱與 RHS 文本(`schema.rs:216`/`:230-239`) |
| N2 | declaration 不 spawn 規則→接口契約層為零 | 三設計通病:i2c 端口契約全錯/acc_pipe 位寬警告/counter8 WIDTH 碰對 | 結構投影已有但未入規則書(experiment-record.md :65);修法=contract 節(S1) |
| N3 | 組合 assign 覆蓋缺口 | acc_pipe 兩條組合信號無規則→0/6 全滅 | `sv/mod.rs:444` has_binary 閘 |
| N4 | 復位敏感列表不可承載 | i2c SCENE10 唯一行為失配(異步→同步) | `sv/mod.rs:491-500` `edges.iter().find_map` 只取第一個 posedge/negedge,`negedge rst_n` 被丟棄 |
| N5 | 盲寫產物機械完備性缺檢查 | i2c compiled=false(state 未聲明) | 修法=信號-聲明完備性 lint(S1) |

### 3.4 能力邊界畫像

- **旗艦 i2c_master:69 規則但類別偏科**——按標題精確統計【本輪實勘】:reset 20/fsm 9/timing 38/counter 2(timing 佔 55%,多為逐拍 NBA 轉寫),**無 handshake/protocol/memory/param 類產出**;協議事務語義(START/STOP/ACK 序列)未被任何類別承載,印證「行為構造級轉寫、非意圖級規格」邊界。acc_pipe 7=timing 5+arithmetic 2;counter8 2=reset 1+counter 1。
- **三份正式規則書 gaps 全 0**:G 類破口掃描聚焦特定缺陷族(latch/死分支/懸空),對乾淨設計零產出——破口面是「測試缺口清單」不是「質量度量」。
- **M1-M3+組合常量盲區的後果**:規則書在 case/復位密集設計中系統性教錯轉移域,且現行字節對拍閘(V4/V5)對此**無判別力**——修復必配正負 fixture(沿 gap-fixtures 閘形)。
- 規則書 evidence/指紋全部用倉庫相對路徑(inline source+name 調用,可移植)【材料·能力簡報】。

---

## 4. MVP 提案:候選排序與選定

### 4.1 ranked 總表

| 序 | 候選 | 裁定 | 一句話 gist |
|---|---|---|---|
| R1 | **復原力 MVP(協議重建版)** | ✅ **選定** | 死因三修(N2/N3/N4)+規則完整性收平(M1/M2/M3/N1)+復原對拍評測固化(S5=協議釘檔+證據塊消融+機械主門檻+採樣判定;S6=歸一化結構比較探針) |
| R2 | A「8 medium 語義收平」獨立弧 | ❌ | 無復原對拍就無法度量收益;精華已被 R1 吸收(M1/M2/N1/M3→S3/S4,M4/M5/M6→S4,M7 前置,M8 帶清單順手清) |
| R3 | C「spec→TB→對拍 conformance 閉環全量」獨立弧 | ❌ | 閉環判別力依賴規則書先夠好——acc_pipe 0/6 現在上閉環全紅無分辨率;最小形已吸收進 S5 |
| R4 | B「規格書升維(意圖級/協議事務級)」 | ❌ | E1/E2 實測反轉緊迫性:規則鏈已承載排隊語義;有意省略原理上不可區分;意圖缺口交 G 類破口+assumptions |
| R5 | D「spec 前端(人寫高級 spec→agent 產 RTL)」 | ❌ | E1 實測表述質量未達標(acc_pipe 0/6=給漏水的船裝發動機);reggen 已佔寄存器子域;正確時點=S5 之後 |
| R6 | E「agent 工作流深化(維護-自規格 skills)」 | ❌ | 需求真實(RTLExplain 停在摘要級)但無現成評測法;R1 的復原對拍指標恰是 E 的前置件,順序不可倒 |

### 4.2 選定 R1 的 rationale

**是——但措辭已按對抗審降格。**E1 是北極星的「能力上界探針」而非嚴格度量(§2.5):可信部分=三個阻斷點實測存在且機械可驗(acc_pipe 0/6 的位寬警告 32 vs 8 與無規則信號是確定性事實;i2c compiled=false=聲明缺失);不可全信部分=matched 絕對值(三方對照實錘證據塊攜帶逐字源碼行【本輪實勘】;counter8 18 行 LLM 先驗飽和;盲隔離僅提示詞約束且 prompt 模板未存檔——`rg -l "prompt"` 零命中【本輪實勘】)。因此 R1 的**每個修復切片都配確定性機械主門檻**(contract 節逐字對拍/async 欄位/golden diff——全部不依賴 LLM 產物),盲寫重跑只作輔助證據帶採樣判定(n≥3 中 ≥2 記 PASS,永不進確定性閘);headline 度量(0/6→?)由 S5 重建協議後按切片分段歸因給出(S1 後記 acc_pipe compiled+matched=歸因 N2,S2 後記 matched 上浮=歸因 N3)。

學界判據支撐:RR Score 區分度最高(A2)、SpecLoop 閉環可行(A1,81-91% vs 34.6-45.2% 核數 §1.4)、metamorphic 同型錯誤旁證 M1-M3 危害(A4)。倉內零件齊全:確定性提取+golden 閘紀律+e1/ TB 裝備——**組裝而非發明**。本輪實跑基線:golden 74/74 PASS、eval 353 GATE PASS(193 skip-perr 實錘 M8 方向)、13 檔判據 13/13+Σrules=325+行為占比 100%。

**成功判據分層【審讀補問,禁混宣稱】**:R1 的「MVP 達成」=**過程成立**——六切片機械主門檻全綠(golden 逐案對帳+fixture 全綠+基線同輪上調)+S5 對拍數字按切片分段**如實**入庫(上浮或不上浮都記,§5 S2 的「仍 0/6 如實記錄」條款)。「北極星命題 1 顯著改善」=**結果判據**,另行宣稱:S5 後 acc_pipe matched>0/6 且 i2c 採樣判定達 11/11 記改善;若 acc_pipe 仍 0/6,阻斷點修復的機械面仍成立(contract/組合/復位能力已落地且 golden 釘死),matched 未上浮作為缺口**升級登記**——過程綠不得包裝成結果達標,兩層分開寫進弧末報告。

### 4.3 落選理由逐條

- **R2(A 獨立收平弧)**:①結構缺陷不變——沒有復原對拍就無法度量收益,修完 M1「規格書變好多少」仍是定性;②初稿「counter8 帶 M3 現形仍 6/6→M1-M3 非主因」的推斷依對抗審**降級**——該樣本 LLM 先驗飽和(§2.5 ②)+TB 未覆蓋 M1/M2 歧義場景(非法編碼 default 對拍缺失),不能據此排除危害;M1-M3 危害改由 audit 現形(m1_min_case.v R003 只剩 2'b00 等機械事實)+metamorphic 論文同型錯誤(A4:丟 path predicate/分支極性錯)承擔;③精華已被 R1 吸收,獨立立弧失去意義。
- **R3(C 閉環全量)**:閉環判別力依賴規則書先夠好——0/6 現在上閉環全紅無分辨率;SpecLoop 自身數據印證閉環是放大器不是修復器(§1.4 核數;RR 0.940 vs 0.865 增益依賴全量診斷回灌質量——VerilogEval 任務表口徑)。循環論證警告內建(B9,論壇級;與 Intel FV 反哺需求澄清同向——需求派生斷言只暴露 80% bug):閉環度量行為復原力≠意圖正確性。TB 生成器列 nonGoal 待 S5 穩定後另立項。
- **R4(B 升維)**:E1/E2 實測反轉緊迫性——i2c 命令排隊規則鏈 R031-R035+R046-R048+R025-R028+R064-R068 已完整承載(TB SCENE9 matched),E2 裁定「半真」;兩個原理性障礙:E2 實錄「仲裁/時鐘拉伸為有意簡化,規格書無法區分有意省略與疏漏」(意圖信息在 RTL 中不存在,`i2c_master.v:11-13` 註釋【本輪實勘】)+AssertLLM2 bug kill 10-20%(B6,LLM 側意圖形式化遠不成熟)。意圖缺口由 G 類破口+assumptions 交人。
- **R5(D 前端)**:主理由=E1 實測表述質量未達標;次理由=OpenTitan reggen(A10)已使寄存器子域 spec→RTL 成工業現實,超越須先有可復原的行為級規則書。VerilogEval-revisit 引用降格為輔助並訂正(A15 實文結論=基準設施動機;材料摘要的 0-shot→3-shot 約 5%→35% 為檢索摘要級,未重抓原文)。正確時點=S5 之後。
- **R6(E 工作流)**:需求真實(B13 RTLExplain 停在摘要/QA 級,與論文範圍相符)但無現成評測法;R1 的復原對拍指標恰是 E 的前置件。S5 評測床即未來 E 的評測床。

---

## 5. 切片與驗收(S0+S1..S6;實施弧執行,每條帶可跑驗收)

### 5.1 全局閘紀

每個語義切片落地後:golden `--record` 重錄+`git diff golden/` 逐案對帳(AGENTS.md 10.5 re-record=語義事件)+V5 指紋閘與 eval 基線同輪上調;語料語義面改動配正負 fixture(沿 gap-fixtures 閘形);切片間**禁攢批重錄**。**閘機械面=主門檻;LLM 盲寫重跑=輔助證據,永不進確定性閘**(對抗審 #4 的全局原則)。語料選擇鐵律:實驗/fixture 語料只能 in-git(corpus=MIT/ISC、corpus-external/sv-elab=ISC、plugin/rtl2spec/fixtures=自著 MIT);zipcpu=GPL 永不入。

### 5.2 前置步 S0+六切片(S1..S6)

**S0:PROVENANCE 手記保留(M7 前置步)【審讀 #7 立】**——M7(提案材料編號,§7.2 注)=「`--record` 機械重寫 PROVENANCE 抹掉語義輪手記」。機制現場【本輪實勘】:`scripts/golden-check.mjs:186-190` 的 prov 模板把歷史區**硬編碼為固定三行**(「## re-record 歷史」標題+空行+首錄行),`golden/PROVENANCE.md:95-97` 現狀歷史區僅含該模板固定行——任何後續人工補寫的手記行(S12 式對帳記錄)在下次 `--record` 時被模板整體覆蓋。
- **內容**:`golden-check.mjs` prov 組裝改為「歷史區解析-保留-追加」——讀既有 PROVENANCE.md 從 `## re-record 歷史` 行到檔尾原樣保留,新增錄製行 append 進歷史區;模板只機械重寫元資料區(錄製時間/bun/wasm sha 表)。
- **歸屬**:實施弧**首個 commit 前、S1 首次 `--record` 之前**完成;獨立小改(單檔腳本手術,10.6 命中計數斷言形),不屬提取器語義面、不觸發 golden 重錄。
- **驗收(機械)**:實施弧內先在 PROVENANCE.md 歷史區造一行手記→`bash plugin/rtl2spec/run-golden-regression.sh --record`→斷言①手記行逐字保留(append 不覆蓋)②元資料區機械重寫③`git diff plugin/rtl2spec/golden` 僅含預期變更並逐案對帳。本提案弧禁跑 `--record`(會重寫 golden/),演練在實施弧做。

**S1:接口契約章(N2+N5)**——`rtl-spec` 核心新增 contract 節入規則書,**兩部分**:①端口/參數面(名/方向/位寬+參數名/默認值);②**內部信號聲明清單**(state/cnt/bitcnt 等全部 wire/reg 變量,來源=結構投影既有聲明面,experiment-record.md :65「結構投影(已有)」)——E1 的 i2c 失敗點正是盲寫方漏 `reg [2:0] state` 聲明,聲明清單使盲寫方無需猜聲明。附**信號-聲明完備性 lint(N5)**,定義可執行形【修訂:審讀 #6——初稿「lint 對 i2c 零缺失」未定義比對集合】:對每檔計算兩個集合的差——**A=規則書全部 evidence/check 行引用的標識符集 vs B=原檔聲明集(端口+wire/reg+localparam/參數)**——差集 A∖B 非空即報(規則書引用了原檔不存在的信號=提取器 bug);跑點=lint 結果作為提取器輸出的一部分,隨 golden 74 檔 byte-exact 每輪重算,不另立閘。
主門檻(機械):三 specs 重錄 contract 節(含聲明清單)與原檔逐字對拍——`corpus/counter8/counter.v`(WIDTH=8)、`corpus/e2e/58_xls_acc_pipe/acc_pipe.v`(x/y 8 位、out 16 位)、`corpus/e2e/59_i2c_master/i2c_master.v`+`crates/spec-i2c/rtl/i2c_master.v` 副本(scl_i/prescale[15:0]/cmd 4 位 onehot;state/cnt/bitcnt 在內部聲明清單中;四檔【本輪實勘】在場;初稿 `5{7,8,9}_*` 路徑錯位已訂正,add3 移除);lint 對 i2c 兩副本差集=∅。
誠實注記:lint 基線預計全綠(evidence 攜源碼原文行,行內標識符天然有聲明)——它是**防回歸閘**(防未來提取器改動引入懸空引用),不是 E1 失敗點的直接修復;直接修復=②聲明清單;盲寫產物自身的機械完備性(重實現側漏聲明)仍由 S5 對拍的 compiled 驗收兜底,lint 不攔。
輔助證據(採樣判定):S1 後盲寫重跑 n≥3 中 ≥2 compiled=true 記 PASS(現狀 false);同輪記 acc_pipe 盲寫 compiled+matched=**死因消融點①**(歸因 N2:預期 compiled=true+位寬警告消失、matched 仍低——組合規則未修的對照點)。

**S2:組合邏輯覆蓋放寬(N3)**——`sv/mod.rs:444` has_binary 閘改為,放寬後的產出分界【修訂:審讀 #3——提案材料 gist「純常量 RHS 產 relation」與其負樣「純常量直賦不產」字面相抵,本版以機械可驗分界落定:**分界=賦值是否處於分支域(conds 非空,即 case/if 臂內)**】:
- **產 relation**:RHS 含 PartSelect/Concat/Repeat(分支域內外一律);純常量 RHS **僅限分支域內**——muxtree `default_cases` 的 `default: y <= 4;` 即此形(臂上常量是行為分支的一部分,【本輪實勘】muxtree.v 全文:default_cases 模組=always @* 內 case,五臂 RHS 全為常量 0/4/1/2/3,現狀整模組 0 條);
- **不產(負 fixture 釘死)**:無條件直賦——`assign y = 8'h00;` 純常量直賦、`assign y = x;` 直通別名(緊湊性保留,防 Daikon 式「什麼都報」,A12 教訓寫進註釋)。
主門檻(機械):acc_pipe 規則書含 p1_bit_slice_103_comb(`[8:1]` 右移語義)與 p2_scaled_comb 定義規則(現狀 0 條);muxtree default_cases 模組 0→≥1;gap-fixtures 正負樣全綠;golden 重錄 diff 逐案對帳。
輔助證據(採樣判定):acc_pipe 盲寫重跑 n≥3 記 matched 變動=**死因消融點②**(歸因 N3);若仍 0/6 如實記錄並升級登記缺口,禁隱瞞。

**S3:復位語義承載(N4)+指紋完整性(N1)+M3**——reset guard 載荷加 `async` 欄位(`sv/mod.rs:491-500` 改全邊收集分類,process 敏感列表入 evidence);timing 指紋 when 維並入 conds(`sv/mod.rs:292-302` 構成面;全鏈重錄代價在案 experiment-record.md :64);復位 else 臂補 `!(reset)` 包裹(M3,`sv/mod.rs:536-538`)。
主門檻(機械):i2c R001 載荷含 async=true;重錄後 muxtree R003/R004 when 互斥(現狀自相矛盾,audit 在檔);i2c HOLD/IDLE 臂 tip<=1/bitcnt<=0 條件賦值規則可見(N1 現狀被吞,E2 gap「規則列表不可作完整行為清單」關閉);V1 Σrules 基線隨規則數變化同輪上調。
輔助證據(採樣判定):SCENE10 場景重跑 n≥3 中 ≥2 得 11/11 記 PASS(現狀 10/11),否則如實報。

**S4:分支條件家族收平(M1/M2)+破口面三修(M4/M5/M6)**——多標簽臂逐標簽展開或 OR 合併(`sv/mod.rs:210`);default 臂推「未被任何臂覆蓋的編碼」條件並入指紋可區分域(`sv/mod.rs:218`);G8 遍歷器展開 DelayCtrl/Wait(`gaps.rs:1019` 非容器葉子註釋處【本輪實勘】);missing_reset 的 reset_set 誤報降 heuristic 或補約束(`gaps.rs:643`【本輪實勘】);incomplete_case 補 n_labels≥total 守衛(`gaps.rs:560`【本輪實勘】)。
主門檻(機械;fixture 落點全部 in-git):M1 正負樣(`m1_min_case.v` 自著 MIT)與 M2/M4 正負樣**遷入 `plugin/rtl2spec/fixtures/`**(現狀住 gitignored `build/mvp-exp/audit/`,弧後清理即丟)並加進 `scripts/gap-fixture-check.mjs` 清單(【本輪實勘】:35-38 現認四前綴);驗收斷言=M1 樣規則含 2'b01 觸發域(現狀只剩 2'b00)、M2 樣產出 default 臂規則(現狀零規則)、M4 觸發形(always #1 if(1'b0))正樣命中、M5 分級正確(certain/heuristic)、M6 修後 4/4 覆蓋樣不再報;`fixtures/gap_clean_branches.v` 零命中保持。

**S5:復原對拍評測固化(E1 協議重建版,對抗審三重強化)**——收編 `build/mvp-exp/e1/` 的 3 TB+4 wrapper+對拍流程為可重跑評測腳本(寫只落 build/,直調不進 regress——沿 run-realworld-eval 先例):
(a) **協議釘檔**:盲寫 prompt 模板逐字入檔(現狀未存檔,`rg` 零命中【本輪實勘】)+模型身份/採樣參數/次數記錄——對照組與實驗組同 prompt 同模型同參數;
(b) **證據塊消融**:完整規則書 vs 裁剪 evidence 塊兩組對拍,matched 差=「抄寫通道」貢獻實測值(三方對照已實證),按組分列禁混報;若消融差顯著=規則文本自身承載不足,驅動規則書渲染修先於數字宣稱,禁歸咎 agent 能力;
(c) **iverilog 守衛**:在場+版本記錄(E1 依賴 13.0,README-e1.md §0),守衛形套 `run-golden-regression.sh:24-28` 先例【本輪實勘】,缺場 exit 2 逐項列出+fail 形態注釋定義;
(d) **資源釘檔與降級形【審讀補問】**:盲寫執行者=實施弧內可用的編碼 agent;**模型身份/採樣參數/次數/API 預算本提案弧無法承諾**(E1 連 prompt 都未存檔,§2.5 ③)——開弧時釘檔進協議;若無可復現的模型/預算,降級形=對拍評測只固化確定性腿(iverilog 守衛+公共 TB+diff+消融組織架構)並在報告標「盲寫腿:環境受限未跑」,禁以臆造樣本數充數。
驗收:一鍵跑通 23 場景(6+6+11);S1/S2/S3 消融點數字按切片分段入庫;**證據晉升清單**——自著 MIT .v 樣/TB 遷 fixtures/(與 S4 同批),experiment-record/audit 5 份/specs 3 份/README-e1.md 等記錄文檔的 git 晉升落點**由用戶裁決**(現狀全住 gitignored 區,弧後一次清理即丟全部 E1 證據;裁決前嚴禁清理 build/mvp-exp/)。

**S6:metamorphic 歸一化結構比較探針(對抗審 #1 high 重寫,原稿驗收必敗)**——初稿「變換後字節指紋集不變=0 IF」作廢:指紋 subject()/effects_text() 內嵌信號名與 RHS 渲染文本(`schema.rs:216`/`:230-239`【本輪實勘】),重命名/重排必變指紋;原文 A4 判據比斷言語義正確性非工件指紋。重寫:
(a) 先落比較層=名稱歸一化(標識符映射逆回寫)+AST 規範化(交換律運算 `+ * & | ^` 排序規範化;渲染去括號——AST 無 Paren 節點天然無括號差,`ast.rs:445-530` 枚舉【本輪實勘】),比較對象=歸一化後**結構摘要集**(category|form|when 結構|effects 結構,去名稱化);
(b) 變換集收窄:加括號臂直跑(parser 消費即消失)、重命名臂經逆映射後比較、重排臂限交換律(非交換重排=非語義保持禁入);
(c) 判據與誠實邊界:歸一化摘要集不變=0 IF;比較層自身配正負 fixture 自證「規範化不丟語義」(變換前後 iverilog 同輸出);報告注明本探針是原文判據的**無 oracle 近似形**(語義正確性判斷留給未來 C 全量化)。
驗收:≥20 檔歸一化摘要集 0 IF;IF>0 逐案登記 REGISTRY(先排查比較層誤報再排查提取器);探針與比較層寫只落 build/。

### 5.3 切片依賴

S0(M7 手記保留)先於一切 golden 重錄;S1→S2→S3→S4 為提取器語義鏈(各自獨立 golden 重錄+讀 diff);S5 依賴 S1-S3(對拍數字才有分段歸因);S6 依賴 S4(M1-M3 修平後 IF 率才有意義)。

---

## 6. 風險與不做

### 6.1 risks

1. **N1 指紋修復=全鏈重錄**(golden 74 份+PROVENANCE+V5 閘+eval 指紋計數,experiment-record.md :64 已實錄);M7 未修則 `--record` 抹 PROVENANCE 手記——M7 前置步 S0(§5.2;歸屬/驗收已立)。
2. **契約章改 schema**→核心 json/md writer+檢查器消費面+wasm 導出面全鏈動(contract 數據經 wasm 透傳,server.mjs 零語義透傳原則不破);按 AGENTS.md 10.8 先普查讀取面(`rg -c` 欄位訪問面)再動;74 份 golden 全 churn 屬預期,逐案對帳;wasm 重建後既有 smoke/parity 閘全綠才算面同步完成。
3. **組合覆蓋放寬的規則爆炸與緊湊性回退**——負 fixture+eval 指紋計數守衛(現狀 672 unique dup=0【本輪實跑】)雙防。
4. **盲寫輔助證據 flake**——採樣判定只在報告層,永不進確定性閘;3 採樣 3 樣分歧時如實記錄擴樣或標不可判,禁取最好樣充數。
5. **證據塊消融可能顯示「抄寫通道」貢獻過大**(三方對照已實證 evidence 攜源碼行)——若消融差顯著=合法發現,驅動規則書渲染修先於數字宣稱;禁把消融差歸咎 agent 能力。
6. **S6 比較層是新的規範化邏輯**——「規範化不丟語義」需正負 fixture 自證;交換律排序對有符號溢出邊界單獨測;IF>0 先排查比較層誤報再排查提取器。
7. **async 欄位讓既有 reset 規則 statement/指紋變化**——語義變更附 diff 修復說明+新 case(交付閘 3);基線同輪上調禁靜默。
8. **E1 證據鏈 gitignored 持久性**——experiment-record/audit 5 份/specs 3 份/e1 裝備全住 build/,弧後一次清理即丟(【本輪實勘】41 檔在場);S5 晉升清單是減災手段但記錄文檔落點待用戶裁決,**裁決前嚴禁清理 build/mvp-exp/**。

### 6.2 nonGoals(顯式不做)

- **意圖級/協議事務語義歸納**(B)——規則鏈已承載+有意省略原理上不可區分;缺口由 G 類破口+assumptions 交人。
- **SVA/斷言生成輸出**——AssertLLM2 bug kill 10-20%(B6),等復原力達標另評。
- **spec→RTL 正向生成器/DSL**(D)——等 S5 數字穩定。
- **TB 自動生成器**——FIXME TD 缺口(B5)是終局方向,E1 手寫 TB 已敷用。
- **仿真 trace 挖掘**(Daikon/GoldMine 族)——無模擬器集成,外部 iverilog 對拍已敷用。
- **三面 facade 改動的邏輯面**——新能力先落 `rtl-spec` 核心(AGENTS.md 10.7);`server.mjs`/skills **零語義邏輯不動**(收參→裝載 wasm→調導出→編幀)。**限定**:contract 節使 wasm 導出 schema 必然變更,S1 的重錄驗收走 MCP stdio→wasm 管線——wasm 導出面隨 schema **同輪重建+既有 smoke/六路 parity 閘驗證**,屬「核心先落、面跟隨重建驗證」,不屬「新能力只在單一面實現」違例(與 §6.1 Risk 2 同義,非相抵)。
- **擴 analyzer-core 立檔禁區**(求值/解析)。
- **本弧改任何既有檔/commit**(弧邊界;實施弧按 §7 走;工作樹【本輪實勘】39 路徑=初始 38+本檔)。
- **閉環全量診斷回灌**(SpecLoop VCD counterexample 形)——S5 只固化確定性對拍;回灌屬 C 全量化另立項。
- **機械斷言級語義等價判據**(metamorphic 原文 CR/any-flip 需 golden/仿真判定)——S6 只做歸一化結構近似,語義正確性判斷留給未來 C。

---

## 7. 銜接建議(sequencing)

### 7.1 ① 先提交 38 路徑基線,再開 R1

重錄 diff 歸因要求基線入庫(10.5 re-record=語義事件:diff 混入上一弧未提交變更無法歸因),且本提案引用的全部閘數字錨定當前工作樹(【本輪實跑】三閘輸出=39 路徑工作樹態)。

### 7.2 ② 8 medium 不立獨立弧、搭車 R1;M7 前置;M8 帶清單順手清

- **搭車論據(修訂後)**:重錄次數在「每步獨立重錄禁攢批」鐵律下**不隨拆弧變化**,真實依據=**同一語義面聚合**——M1/M2 與 N1/M3 同觸 `sv/mod.rs` walk_stmt 分支遍歷與指紋構成(`:210/:218` 與 `:536-538`/`:292-302`【本輪實勘】),同弧落地共用同一次語義面評審與 fixture 體系,省的是弧級切換與立項評審開銷。
- **M7 前置**(否則 `--record` 機械重寫 PROVENANCE 抹掉手記歷史區)——已立為**前置步 S0**,內容/歸屬/驗收見 §5.2。
- **M8 清償清單(本輪 grep 全表,逐行讀上下文分類;比初稿清單更全)**。計數口徑=**grep 命中行**(本輪實跑 `grep -n "10s\|192" docs/RTL2SPEC-PLUGIN.md`+`rg -n "192|10s" plugin/rtl2spec/scripts/eval-all.mjs`):共 **10 行**=PLUGIN.md 9 行+eval-all.mjs 1 行;逐行分類=**殘文候選 4 行+歷史記載保留 6 行**(4+6=10,與下表逐行可對帳):

| # | 行 | grep 命中內容(節錄) | 分類 | 實施弧處置 |
|---|---|---|---|---|
| 1 | `docs/RTL2SPEC-PLUGIN.md:1399` | 只斷言:進程可終止(**10s 守衛/檔**)+零 panic | **殘文候選**(「10s/檔」與 R16-3 裁決相抵:單進程 wasm 循環禁宣稱計時器守衛) | 改「wasm 腿=F2 圖規模守衛+trap 計數;native 腿 subprocess 10s」 |
| 2 | `docs/RTL2SPEC-PLUGIN.md:1400` | parse_errors 如實入 verdict(**192 檔** parse_err=…) | **殘文候選**(實跑 193) | 訂正 193 或改「實跑見 RTL2SPEC-EVAL.md」 |
| 3 | `docs/RTL2SPEC-PLUGIN.md:1508` | 1. 不修 sv-tests **192 檔** parse_err | **殘文候選**(實跑 193) | 同 #2 |
| 4 | `plugin/rtl2spec/scripts/eval-all.mjs:7` | 註釋「**192 檔** parse_err=已知事實記錄」 | **殘文候選**(代碼註釋) | 訂正 193(與 #2 同形) |
| 5 | `docs/RTL2SPEC-PLUGIN.md:1215` | (parse_err 192+has_rules 僅 8 檔;開弧 fitness 實掃[未入庫]) | 歷史記載,保留 | — |
| 6 | `docs/RTL2SPEC-PLUGIN.md:1429` | R16-3 修訂:初稿「單進程循環+每檔 10s 守衛」自相矛盾,勘誤 | 歷史記載,保留 | — |
| 7 | `docs/RTL2SPEC-PLUGIN.md:1435` | …「10s 守衛」僅保留在 native 腿 subprocess 形 | 歷史記載(R16-3 裁決正文),保留 | — |
| 8 | `docs/RTL2SPEC-PLUGIN.md:1476` | native 腿 subprocess 10s 守衛內可終止 | **已訂正形態**(與 R16-3 一致,非殘文),保留 | — |
| 9 | `docs/RTL2SPEC-PLUGIN.md:1561` | R16-3 [medium]「單進程循環+每檔 10s 守衛」自相矛盾——已修 | 歷史記載,保留 | — |
| 10 | `docs/RTL2SPEC-PLUGIN.md:1567` | …10s 守衛僅保留 native 腿 subprocess 形 | 歷史記載,保留 | — |

對帳任務「三處 10s 殘文」:唯一正文殘文行=#1 `:1399`(初稿誤把 :1476 當殘文、漏 :1400;#6-#10 五行均為 R16-3 修訂/勘誤/已訂正記載);「192」殘文行=#2/#3/#4 三行。實施弧逐行裁定(殘文改寫/歷史保留),裁定記錄入 commit message。
編號出處注:**M7/M8 為本弧任務材料的 MVP 規劃編號**(「8 medium」家族 M1..M8);`rg -n "M7" docs/RTL2SPEC-PLUGIN.md` 零命中,REGISTRY 既有「M7」=存儲器陣列里程碑(vl-sim 體系,另一命名空間)——實施弧落地 M7/M8 清償時**同輪入 REGISTRY 登記本案編號定義**,免編號撞名。

### 7.3 ③ R1 內部順序與後續隊列

**S0(M7 手記保留,驗收見 §5.2)→S1→S2→S3→S4**(每步獨立重錄+讀 diff,禁攢批)**→S5**(協議重建+分段歸因)**→S6**(歸一化探針);之後推薦隊列(非本提案範圍):C 全量化(TB 生成+診斷回灌)→D(spec 前端)→E(agent 維護工作流,以 S5 評測床為前置)。

### 7.4 容量畫像與拆弧判據(決策者審批輔助;不做工期承諾)

- **規模畫像**:S0=單檔腳本手術+一次錄製演練;S1-S4=各一次提取器語義改動+全鏈重錄+fixture(4 輪「重錄→git diff 逐案對帳→基線上調」循環);S5=收編 e1/ 裝備為腳本+協議釘檔+消融組(寫只落 build/);S6=比較層+變換集+≥20 檔探針(新規範化邏輯,面最大)。對照先例:S9..S12 一弧完成 golden 閘+gap-fixtures 閘+評測擴面三閘落地。
- **拆弧判據(擇一觸發即拆)**:①S1-S4 任一切片重錄 diff 出現不可解釋變更(10.5 鐵則:否則停弧排查)→當前切片內停,已綠切片先行收口;②S6 比較層正負 fixture 自證不過(規範化丟語義)→S6 拆下一弧,S5 照常收口;③盲寫採樣 3 樣全分歧且擴樣仍不可判→S5 的盲寫腿標「不可判」如實收口,不阻塞機械面。M7/M8 同輪入 REGISTRY(§7.2 注)。

---

## 8. 本輪實跑與未核實事項(誠實條款)

### 8.1 本輪實跑/實勘清單(全部當場執行,2026-11-05)

- **三閘**:`bash plugin/rtl2spec/run-golden-regression.sh`→74/74 PASS exit 0;`bun plugin/rtl2spec/scripts/eval-all.mjs`→353 全集 GATE PASS(37/117/**193 skip-perr**/6/trap=0/672 unique dup=0/90ms);13 檔逐檔 `bun plugin/rtl2spec/scripts/check-rulebook.mjs <file>`→13/13 OK,verdict JSON 匯總 Σrules=325/行為占比 100.0%/5 類(bun -e 統計 build/verdicts/)。
- **閘後核驗**:dist wasm sha256 前後同為 `4b143d3aafd22c41f85a0c2cebc23edd0c0176e3eb0f2ae88849fc4c8d855f3cd`(pkg fresh 實勘:六依賴目錄無 .rs 新於 pkg);`git status --short`=39 路徑不變;golden/、validation-report.md、docs/RTL2SPEC-EVAL.md mtime 未動。
- **讀碼核實行號**(全部親讀):`sv/mod.rs` :200-204(組合 else 臂有包裹)/:210/:218/:292-302(指紋六欄)/:444/:448-455(when 構造)/:491-500/:522-541(else 臂裸 push :536-538);`gaps.rs` :557-566(n_labels)/:636-643(reset_set)/:1008-1022(非容器葉子註釋 :1019);`schema.rs` :216/:230-239/:262-264;`ast.rs` :445-530(Expr 枚舉,Paren 0 命中);`run-golden-regression.sh` 全文(守衛 :24-28,stale 才重建 pkg,默認分支寫 build/golden-current/);`eval-all.mjs` :1-12(:7 註釋 192)/:122(outDir=build/eval);`golden-check.mjs` :145-219(record 分支才寫 golden/);`check-rulebook.mjs` :163-166;`gap-fixture-check.mjs` :35-38。
- **實勘斷言**:三方對照(規則書 :829/:876/:923/:970 vs 原版 :272 vs 盲寫版 :253 逐字同);`rg -l "prompt" build/mvp-exp/` exit 1;M8 grep 全表(§7.2);i2c 規則數 69(`grep -c "^### "`)+類別分布按標題精確統計(reset 20/fsm 9/timing 38/counter 2);specs 三檔字節數與設計映射;E1 產物 41 檔在場;`i2c_master.v` :11-13/:17-18/:68/:70、`acc_pipe.v:22` 逐行;18/44/316 行數;SCENE4 原始輸出仲裁(orig=05fa/reimpl=01f8);`muxtree.v` 全文(default_cases=always @* 內 case 五臂常量 RHS——S2 分界的「分支域內常量臂」實樣);`golden/PROVENANCE.md:90-97` 歷史區現狀+`golden-check.mjs:186-190` 模板硬編碼歷史區(M7 機制現場);`rg -n "M7" docs/RTL2SPEC-PLUGIN.md` 零命中+REGISTRY 既有 M7=存儲器陣列里程碑(編號撞名核對,§7.2 注)。
- **檔案全文讀取**:experiment-record.md(75 行)/e1/README-e1.md(130 行)/既有本檔初稿(214 行)。

### 8.2 未跑/未核實事項

- `bash plugin/rtl2spec/run-rtl-validation.sh` **整腳本未跑**(④ 步重寫既有 validation-report.md,違本弧禁改邊界);替代=逐檔 check-rulebook 13 檔+verdict 匯總(§3.1);validation 閘自身 V5 計數未單獨復算,「325/325 指紋零重複」由 eval-all 672 unique(dup=0)+13/13 failures 判據間接承載,如實注記。
- `run-realworld-eval.sh`/`regress.sh` 全量未跑(353 檔時長+基線引用 REGISTRY/commit 200b1c7 message 實測值);`--record` 未跑(會重寫 golden/,禁);M7 未復現。
- E1/E2 的 iverilog 對拍與評審**本輪未重跑**——數字取自 experiment-record.md/README-e1.md 實勘(兩檔本輪全文讀取)+任務材料,三方對照與原始輸出抽勘佐證(SCENE4 diff 逐行核對)。
- 網調 28 條 findings **本輪未重抓任何原文**——抓取強度沿用網調弧記錄(§1.5 三分:原文全文 6 項/abstract 頁 2 項/未抓 20 項,與 §1.1/§1.2 抓取欄逐條一致);SpecLoop Fig.3 核數(81.3/88.7/85.5/91 vs 34.6/45/45.2/42.8)引網調弧 WebFetch 實抓,DeepSeek 0.940/0.865 屬另一張表本弧未逐字核。
- M4/M5/M6 僅核實代碼現場,未現形實驗(觸發形 fixture 留給實施弧 S4);M7 未復現(需跑 `--record`,會改既有 golden 樹,禁)——機制現場已實勘(`golden-check.mjs:186-190` 模板硬編碼歷史區,§5.2 S0),修復與錄製演練歸實施弧 S0。
- SCENE4 記法差異(1530 vs 1528)與「1530 mod 1024=504」算式不成立(實為 506)已在 §2.2 勘誤注記,以原始輸出為準。

---

## 9. 實施記錄(S0..S5;2026-11-05;S6 未執行留後續弧)

- S0..S5 全部落地,機械門檻實跑數字逐切片見 docs/RTL2SPEC-PLUGIN.md §18.2
  (【實跑】標記);re-record 對賬逐切片完成(禁攢批遵守)。
- 計劃外同輪處置(如實記錄):S3 N1 使 i2c 觸頂 DEFAULT_MAX_RULES=128
  截斷(ST_STOP 臂被截=V1 必紅)→同輪上調 256(SSOT schema.rs+四處 JS
  調用點);S2 提案預測「acc matched 仍低」被實測推翻(N3 修復後 6/6)——
  預測偏差如實歸因,禁改預測文本。
- 正典文字面待辦(同弧文檔輪收口):RTL2SPEC-PLUGIN.md「全量重寫」字樣/
  「默認 128」×4/本檔 S1-S5 條文與實作的行號漂移(行號引用按 S0 時點)。
- 輔助證據軌狀態:盲寫採樣(消融點)數字已按切片入庫(PLUGIN.md §18.2);
  盲寫永不進確定性閘(AGENTS.md 10.10/SKILL.md 同輪落條文)。
