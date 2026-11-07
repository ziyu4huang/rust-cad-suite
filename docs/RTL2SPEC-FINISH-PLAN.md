# RTL2SPEC-FINISH-PLAN — C 閉環收尾弧(未覆蓋項)實施規劃(R3 修訂)

- 日期:2026-11-07(R3 修訂同日)。性質:**獨立小檔**(修訂輪只改本檔,禁追加進 RTL2SPEC-PLUGIN.md
  大正典;收尾切片一次性把「§20 未覆蓋收尾實施記錄」寫進正典,既有章節零刪改)。
- 範圍=用戶指令「finish 未覆蓋」:終審 2 medium 收平+等價裁決腿+波形幀診斷腿+T 腿全量擴面
  +§20/版本收尾。基線=工作樹 C 弧產出 68 路徑未提交(conformance 6/6 admitted、mutants=439
  eligible=412 killed=347 equivalent=27;wasm sha 66518257…)。
- 數字來源標註:【勘察】=開工勘察材料;【實跑】=本規劃輪(R3)本機實跑;【審 R2/R3】=對抗審
  實跑、經本輪復核一致;【正典】=自 repo 文檔讀到的記錄。R1/R2 輪假數據訂正鏈見文末修訂記錄。
- R3 修訂=對抗審 R3 六條(1 high/1 medium/4 low)逐條消化,見文末「修訂記錄 R3」;實質修訂處
  標 `[R3-n]`。

## 0. 邊界(全程約束;違反即停弧)

1. LLM 腿永不進確定性閘(10.11);幀診斷=輔助證據,必標 `modality`/`confidence`;**vision
   前置條件=統一時間窗下的機械畫布比較通過(§4.4,程序化剝 title——JS 非貪婪跨行正則,
   禁逐行 sed)**。
2. yosys oracle(vendor-reference/ gitignored,.gitignore:2)僅測試禁 ship;等價裁決腿=測試
   側 harness,ryosys equiv=註冊里程碑(禁實現禁 stub——10.9);**一切 yosys 斷言只在
   record 模式跑(conformance-eval.mjs:119 守衛),默認模式永不硬依賴該二進制**。
3. VCD 讀寫一律走 vl-vcd(ADR-0004);渲染=SVG;值在 `<title>` tooltip、畫布無值文字
   (render.rs:18-19),**esc 保留 \t\n\r(render.rs:96-97)→ title 可跨行**;柵格化只是查看
   手段;核心渲染契約本弧不動(ADR-0005)。
4. 語料 in-git only;zipcpu=GPL 永不入 golden/conformance/幀腿。
5. TB 判別力鐵律不變(Q8);golden/評測基線隨語義面同輪上調禁靜默;重錄=語義事件逐案對賬。
6. 入庫 tb.v 禁手改,換檔必走 `--promote`(S0-② 補條+機檢)。
7. 弧內禁 git commit/push;計數一律實測——**解析與讀數程序化,禁 shell 轉義內嵌讀數**
   (R1 教訓);閘跑不過或指令矛盾如實升級。

## 1. 基線與 R3 復核(逐條可溯源)

- 2 medium 現場:§19.24=RTL2SPEC-PLUGIN.md:3701-3731(工作樹);WAIVE 標籤集=CONFORMANCE-
  PROTOCOL.md:44-45+tblint.mjs:176;「不變」判定=conformance-eval.mjs:822。
- WAIVE 11 條實證【實跑(R1 維持)】:21 變異=20 unadjudicated+1 equivalent(m0054/R042;
  R104 僅 m0178)。
- unadjudicated=57(i2c 46/sincos 6/subbytes 5;另 8 個 null 已被 T 腿殺)【勘察】。
- **強形分類+修正守衛端到端【實跑 R3,本輪新增】**:gold-vs-gold×6+27 存量 equivalent=
  **33/33 equivalent、0 誤判**;ports 表(all/out)=acc 33/16、**fsm 8/4**、i2c 49/16、
  **signedexpr 23/18(臨界通過)**、sincos 64/41、subbytes 86/42;R2 守衛(全端口位數)對
  signedexpr 的誤判實證=cells 18<23 會把 gg/存量 m0006/m0008 誤判 adjudication-failed
  【實跑:`select -count t:$equiv`=18 objects+proven;portBitsAll=23>18】——審 R3 逐值
  吻合,守衛改輸出端口位數(§3.2)。
- **killed 全母體(強形)【實跑 R3,本輪新增】**:347 killed 全量=**346 unproven+1 failed
  (i2c m0011 OP-POL,$pow 類)+0 equivalent**——現有語料上強形零假陽性【審 R3 同結論】。
- blacklist 形廢棄依據(R2 維持):fsm m0045 blacklist 兩形態皆 4 unproven、431 全量無
  區分力、空真可偽造。
- 空真機制(R2 實測維持):全名單構造 → `No $equiv cells found`+exit 0(-assert 同)。
- flatten(R2 實測維持):fulladd top-only+flatten 正常裁決;`$pow`+flatten 仍 No SAT
  model=唯一證明器邊界。
- 幀統一窗(R2 實測維持):sincos tmax 3770000/3680000;統一窗後 fsm/sincos 畫布真可分、
  subbytes 負樣保持;【審 R3】以 JS 跨行正則復核三設計結論一致,且 sincos 在
  [0,min tmax] 窗內畫布仍異(差異不在尾部)。
- 總閘第 12 條牆鐘(R1 實測維持):42.9s PASS 6/6;默認模式無 yosys。

## 2. S0(第一切片)=終審 2 medium 收平(與 R1/R2 相同,零改動)

- 訂正點 A:RTL2SPEC-PLUGIN.md:3722 i2c WAIVE 措辭 →「(全部變異兩腿存活且 yosys 未證
  等價)」+證據指針=21 變異(20+1)。
- 訂正點 B:CONFORMANCE-PROTOCOL 標籤集補 `unadjudicated`;模板 v1→v2 協議事件
  (append-only,v2 圍欄標記 `<!-- template-block v2 begin -->`+新 sha 入檔);dleg-prompt
  硬同步(取最新版圍欄+打印 v2 sha+一正一負探針)。
- 訂正點 C:tblint.mjs:176 枚舉擴閘(一正一負探針);交叉驗證落點 S1。
- 升級鉤:中性形,不預設方向;tb.v tag 本切片不動。
- S0-②:TB-SHA-ALARM(record sha 三方核對,矛盾=exit≠0)+默認模式 directed 腿 sha 一致性
  斷言+協議 §5 禁手改條;驗收=手改紅/還原綠/正常 promote 不誤報。

## 3. S1=等價裁決腿(強形 harness)

### 3.1 oracle 命令形(強形;R2 確立,守衛計數手段本輪補)`[R3-high]`

```
# 每設計一次:端口表(空真守衛用輸出端口位數;全端口位數會誤判組合設計,R3 訂正)
yosys -q -p "read_verilog <src>; hierarchy -top <top>; proc; write_json <ports.json>"
# 逐變異裁決(強形;mut.v=top-only;無 blacklist 無 stash;select -count 為 cells 計數手段
# ——equiv_status 在 proven 時不輸出 cells 總數,守衛不可從 equiv_status 行取數)
yosys -p "read_verilog <src>; rename <top> gold;
          read_verilog <mut-top-only>; rename <top> gate;
          proc; flatten; memory; async2sync; opt_clean;
          equiv_make gold gate equiv; hierarchy -top equiv;
          select -count t:\\$equiv;
          equiv_simple -seq 5; equiv_induct -seq 5; equiv_status"
```

- 選形依據=§1 端到端實測(gg×6+27 存量 33/33、57 unadj 全 unproven、347 killed 0 等價);
  blacklist/-stash 廢棄;**flatten 必須在列**;mut.v=top-only 提取(行級狀態機,禁點星跨塊)。
- 能力邊界僅 `$pow` 一條(No SAT model,flatten 不解【實跑】)。
- 雙向措辭:proven≠無條件行為等價(弱等價+flatten/async2sync/memory_map 抽象)、unproven≠
  不等價;eqMeta/§20 一律如實。

### 3.2 eqverdict 三值與守衛(口徑 R3 訂正)`[R3-high]`

- 對 allSurvive 的 rule-pool 變異(record 模式):
  - proven 且 `select -count t:$equiv` 的 cells 總數 ≥ **輸出端口位數合計**(write_json
    ports 中 `direction==="output"` 的 bits 合計) → `equivalent`;
  - proven 但 cells 總數 < 輸出位數 → **`adjudication-failed(empty)`**(空真=證明結構未
    建立,永不判等;組合設計無內部具名信號時 cells 恰=輸出位數,臨界可過——signedexpr
    18/18 實測);
  - 有 unproven cells → `unproven`(中性;未證信號名寫 `unprovenCells` 診斷附註,不帶
    端口/行為語義);
  - `No $equiv cells found`/`$pow` 類 No SAT model/逾時/oracle 缺席 → `adjudication-failed`。
- **fsm 守衛錨=輸出位數 4**(非 8;R2 的全端口口徑訂正)【實跑】`[R3-high]`。
- 現行 `adjudication` 兩值欄位保留;record 頂層 `eqMeta`={yosysVersion、命令模板逐字、
  counts、provenance=「弱等價(-seq 5)+flatten/async2sync/memory_map 抽象」、ports 表
  all/out}。schema 變更→0.5.0(§6)。
- 空真守衛三件套:①cells≥輸出位數前置;②numbersOf 把 equivalent 併入 controlSet 恒真
  (:402-406),Q8 接不住空真——本守衛是唯一防線,寫進閘斷言;③故意空真探針(§3.3-4)。

### 3.3 閘偵測力斷言(record 模式專屬,受 :119 yosysBin 守衛)

1. **gold-vs-gold 六設計帶守衛端到端**【實跑 33/33 集合內 gg 全 equivalent】——先於逐
   變異裁決。
2. 正樣:fsm m0045(OP-NOP)→ `equivalent`【R2 實跑 proven,強形】。
3. 反樣:i2c m0001(golden killed)→ 非 equivalent(中性 unproven;地面真值=行為可分;
   killed 判等即紅)。
4. **空真探針=獨立釘檔模板** `[R3-low4]`:與生產命令形分開的「探針專用模板」(全名單
   blacklist 構造對,唯一用途=製造空真,釘檔聲明不屬生產管線;替代形=gate 側輸出改名,
   實現輪擇一)→ 必判 `adjudication-failed(empty)`【R2 實測:No cells+exit 0,-assert 同】。
5. **killed 全母體可靠性自檢(單點探針升級)** `[R3-low6]`:record 模式對全部 killed 變異
   跑強形裁決,斷言 `equivalent==0`(分鐘級;實測 347 全量=346 unproven+1 failed+0 等,
   i2c 264 次≈1:49【審 R3 同量級】;現有語料零假陽性,S3 新語料由每輪 record 重跑執法)。
6. 27 存量 equivalent 帶守衛重驗(端到端 33/33 含 27/27【實跑】);任一翻變=紅燈列名。
7. **默認模式**:不跑 yosys;eqverdict/eqMeta schema 完整性+counts 一致性;舊版 record=
   綠+顯式 `SKIP eq-verify` 行(窗口限本弧 S0→S1)。

### 3.4 WAIVE 逐規則重裁(數據落地後釘;方向不預設)

- 裁定表逐規則(變異數/eqverdict 分布/killed 數/裁定/理由)釘進 §20。
- **預期數字(R2/R3 實測鏈):57 unadj 強形全 unproven、0 新 equivalent;守衛修正後
  SUMMARY equivalent 維持 27 的聲明成立**(R2 版的此聲明曾被守衛誤判破壞,現端到端復核
  33/33 後恢復)`[R3-high]`;涉及 tb.v 換檔一律 `--promote`(註釋-only,expected 0 行差)。

### 3.5 WAIVE↔record 交叉驗證(S1 同輪落地;措辭 R3 收斂)`[R3-low3]`

- 數據謂詞:`unadjudicated` ⇔ 無 killed 且存在 unproven/adjudication-failed 存活變異;
  `unobservable` ⇔ 無 killed 且存活變異全部 equivalent。
- 靜態標籤(x-valued/tb-internal)**可見化** `[R3-low3]`:record-diff 強制打印每設計每標籤
  使用數**變化量**(現 record-diff 只打 numbers,:838 區域);新增靜態標籤須在切片記錄理由。
  措辭=「可見化」,不稱「紅燈」——合法 `--promote` 換檔(sha 斷言攔不住)後計數隨 record
  更新不會紅,可見化+理由記錄是實際防線。
- 預期觸發清單:i2c 11 條+sincos R035(m0061/m0062∈57 強形 unproven)——sincos 算進
  promote/對賬集合(R2 維持)。

### 3.6 ryosys equiv 里程碑註冊(不帶編號硬承諾;能力邊界僅 $pow 一條)

## 4. S2=波形幀診斷腿(統一時間窗)

### 4.1 VCD 產生+構建守衛(拒跑形,腿內不構建——寫面約束保持)

### 4.2 幀渲染=統一時間窗(`--from 0 --to max(tmax)`;分歧定位=killScenes+--signals)

### 4.3 柵格化(sips 首選/qlmanage 備選;幀落點 build/conformance/frames/)

### 4.4 modality 判定=統一窗畫布雙檢+diag 驗證器`[R3-low5]`

- **剝 title=程序化 JS 非貪婪跨行正則** `/<title>[\s\S]*?<\/title>/g`——esc 保留
  \t\n\r(render.rs:96-97),字符串型 VCD 值的 title 可跨行,**逐行 sed 剝不淨會誤判
  「畫布不同」→ 偽放行 vision**(R2 的 sed 寫法訂正);安全依據=title 元素 XML 不可嵌套,
  非貪婪必停在最近閉標籤;**配跨行 title 敵意單測**(10.6 禁令針對刪除手術,此處=程序化
  匹配+單測釘死)。【審 R3】JS 正則復核三設計結論與 §4.5 一致。
- 主判=剝 title 後 SVG 畫布 byte 比較;輔判=PNG sha256;主判相同→強制 text-fallback
  (limitations 記「畫布層不可分」)。
- diag 驗證器:frames 存在+PNG 魔數+`framesRead==frames.length`+modality=vision ⇔ 雙檢
  皆異+confidence 三值;驗證器紅=幀腿工件紅(不驗 LLM 內容)。
- 輸出=`build/conformance/diag-vision/<design>.json`;協議釘檔進 CONFORMANCE-PROTOCOL
  新節(含統一窗/程序化剝 title/雙檔條款)。降級形同 R2。

### 4.5 frameDemo(R2 統一窗結論維持)

| 設計 | 統一窗實測(R2【實跑】+R3 JS 正則復核【審 R3】) | 裁定 |
|---|---|---|
| fsm | 剝 title 畫布異+PNG sha 異 | 保留(vision 正樣) |
| sincos | 統一窗後畫布異(且 [0,min tmax] 窗內仍異=差異不在尾部) | 保留 |
| subbytes | 畫布同+PNG 同 | 維持撤下(負樣) |

## 5. S3=T 腿全量擴面

### 5.1 staging 預錄(機制 R2 補口+硬編碼清單 R3 補全)`[R3-med2]`

- **conformance-eval.mjs 的主 fixtures 綁定點逐項實核(:41/:42/:43/:751/:777/:859-866)**:
  fixRoot:41、outRoot:41、reportPath:42(=主證據頁 plugin/rtl2spec/conformance-report.md!)、
  EXPECTED_DESIGNS=6:43(:729 斷言)、fullSet:751、F3 i2c_master cnt_end 斷言:777、
  部分錄製不重寫證據頁:859-860、默認模式 byte 比對:863-866。
- **--fixtures-root 覆蓋參數須同時覆蓋**:①fixRoot;②outRoot 與 reportPath 指向
  build/conformance-staging/(**staging 報告不得覆蓋主證據頁,否則違寫面聲明+下次默認模式
  byte 比對紅**);③EXPECTED_DESIGNS 與 F3 斷言**僅對主根目錄生效**(staging 跳過——staging
  批次即全集,但集合≠主集合,恒 1 的 exit 不可作準)。
- **ADMITTED 判定=解析 staging 的 verdict.json,不看 exit code**(:729/:777 使 staging
  exit 恒 1)。
- **證據頁重生成路徑(R3 寫死)**:拷入真 fixtures 後,每批做一次**全集重錄**(`--record
  --slice C-S8-n` 全設計集)——局部 record 不重寫證據頁(:859-860)且會連帶 provenance.date
  (:617)差異,故「全集重錄+逐案對賬(record-diff-C-S8-n)」是每批的固定收尾;禁局部錄後
  手補證據頁。
- 其餘引導同 R2(golden 擴檔錄前 flapper 清零/case.json 手寫/-g2012/編不過 staging 剔除)。

### 5.2 時長預算與降級(R2 維持:42.9s 錨、外推 6-10 分鐘、P0/P1、ADR-0013 --quick 路徑)

### 5.3 判別力鐵律逐 TB 保持(不變;Q8 拒錄不降門)

## 6. S4=收尾(R2 維持)

- §20 一次性寫入(逐切片實跑數字+裁定表+預期觸發清單[i2c 11 條+sincos R035]+$pow 邊界+
  SKIP 過渡窗+守衛口徑訂正鏈);REGISTRY/SKILL/OKR 同步;0.4.0→0.5.0 四點一致;wasm sha
  66518257… 保持=閘斷言;總閘 12 條不變(第 12 條默認模式零 yosys 依賴)。

## 7. 切片分解(首個=S0)

- **S0**:終審 2 medium 收平(§2)。
- **S1**:等價裁決腿(§3;強形+select -count 守衛+eqverdict 三值+空真/輸出位數口徑+探針
  六條含 killed 全母體+裁定表+交叉驗證+里程碑)。
- **S2**:幀診斷腿(§4;統一窗+程序化剝 title 雙檢+敵意單測+驗證器;frameDemo=fsm/sincos,
  subbytes=負樣)。
- **S3**:擴面(§5;fixtures-root 全覆蓋清單+verdict.json 判定+每批全集重錄+逐案對賬)。
- **S4**:收尾(§6)。依賴:S0 先行;S1 數據驅動 WAIVE 裁定;S2 依賴 S0;S3 依賴 S1 schema;S4 最後。

## 8. 顯式不做

1. ryosys equiv_* 核心實現。2. 拒收 28 檔 TB 化。3. 擴面新設計 D 腿與幀腿。4. 幀診斷進閘
判據。5. vl-vcd 渲染契約變更(另弧)。6. 點陣 canvas/rsvg-convert/腿內 cargo build。
7. GPL(zipcpu)入閘。8. 手改入庫 tb.v(--promote 例外)。9. git commit/push;降門檻;手寫
期望/golden;yosys 斷言進默認總閘;shell 轉義內嵌讀數;逐行 sed 剝 title。

## 9. 風險與降級形

1. 幀鏈 flake/畫布不可分:統一窗+程序化剝 title 雙檔;不過=強制 text-fallback;不進閘。
2. 等價誤判雙向:三值 eqverdict+空真守衛(輸出位數口徑+select -count 計數)+killed 全母體
   斷言(equivalent=0,347 實測基準)+雙向措辭;adjudication-failed 獨立計數。
3. 擴面機制:staging 全覆蓋清單+verdict.json 判定+每批全集重錄;42.9s 錨+P0/P1+--quick 路徑。
4. golden 抖動:flapper 清零+逐案對賬。
5. TB-SHA-ALARM 誤傷:矛盾型報警;promote 不誤報。
6. 讀數/解析坑(R1/R2 事故鏈):分類與守衛解析一律程序化(JS);regex 對真實輸出行形單測
   釘死(含 select -count 行/Successfully/No cells/Found a total of N/跨行 title)。

## 10. 固定閘契約對照(7 條逐條落點)

| 契約 | 落點 |
|---|---|
| 1 方案檔=本檔;§20 收尾一次性寫入 | 性質聲明+§6 |
| 2 S0=2 medium 收平 | §2 |
| 3 等價裁決腿 | §3 |
| 4 幀診斷腿 | §4 |
| 5 全量擴面 | §5 |
| 6 版本 0.5.0 四點一致 | §6 |
| 7 總閘 12 條不變 | §6 |

## 修訂記錄 R2(2026-11-07;摘要保留,全表見 R2 版檔史)

R2 對抗審 8 條(high: blacklist 形無區分力;med: 空真守衛/WAIVE 交叉驗證/幀時間軸/預篩
不可判定/flatten 修復;low: 構建守衛寫面/SKIP 狀態)全部消化;R1 假數據訂正四項(fsm m0045
讀數坑/classify 解析 regex 坑/審方 classify 措辭撤回/sincos PNG sha 作廢)。命令形確立為
強形(+flatten+memory,無 blacklist 無 stash);eqverdict 三值;統一時間窗;staging 預錄。

## 修訂記錄 R3(2026-11-07;對抗審 R3 六條逐條消化)

| 審條 | 裁定 | 消化與本輪實測 |
|---|---|---|
| high 空真守衛口徑(全端口位數錯) | **接受** | 本輪逐值復核:signedexpr gg cells=18(`select -count t:$equiv`)、portBitsAll=23、portBitsOut=18——R2 守衛(≥23)會把 proven 誤判 empty;修=守衛改**輸出端口位數**(write_json direction==='output'),cells 計數手段=`select -count t:$equiv`(equiv_status proven 時不打 cells 數);fsm 錨 8→4;**凍結前端到端重跑(命令形+守衛)gg×6+27 存量=33/33 equivalent、0 誤判**【實跑】,六設計 ports 表(acc 33/16、fsm 8/4、i2c 49/16、se 23/18、sincos 64/41、subbytes 86/42)入 §1 |
| med-2 staging 硬編碼清單不全 | **接受** | :41 fixRoot/outRoot、:42 reportPath(主證據頁)、:43+:729 EXPECTED_DESIGNS、:751 fullSet、:777 F3 cnt_end、:859-860 部分錄製不重寫、:863-866 byte 比對逐行實核;三後果(報告覆蓋主頁+byte 比對紅/exit 恒 1 不可作準/局部錄不重寫證據頁+provenance.date :617 連帶差異)成立;§5.1 補全覆蓋清單+「ADMITTED=解析 verdict.json 不看 exit」+「每批全集重錄+逐案對賬」寫死 |
| low-3 靜態標籤防繞過措辭 | **接受** | 真繞過路徑=--promote 合法換檔(sha 斷言攔不住),計數隨 record 更新不紅;record-diff 只打 numbers;修=record-diff 打印每設計每標籤使用數變化量+新增靜態標籤記理由;措辭「紅燈」→「可見化」(§3.5) |
| low-4 blacklist 一詞字面矛盾 | **接受** | §3.1「移除」與 §3.3-4 探針衝突;修=空真探針改「探針專用模板獨立釘檔(唯一用途=製造空真,與生產命令形分開)」,替代形=gate 側輸出改名;實現輪擇一(§3.3-4) |
| low-5 逐行 sed 剝 title 不安全 | **接受** | render.rs:96-97 `'\t'|'\n'|'\r' => push(c)` 字面核實(title 可跨行);修=JS 非貪婪跨行正則+跨行 title 敵意單測(§4.4);審方 JS 正則復核三設計結論與 R2 一致(采信;我方 R2 畫布結論不變),sincos 差異不在尾部【審 R3】;§8/§9 補「禁逐行 sed 剝 title」 |
| low-6 killed 全母體自檢 | **接受** | 本輪 347 killed 強形全量=**346 unproven+1 failed(i2c m0011 OP-POL=$pow 類)+0 equivalent**【實跑,與審 R3 零假陽性結論一致】;§3.3-5 升級:record 模式對全部 killed 裁決+斷言 equivalent==0(分鐘級,i2c 264 次≈1:49【審 R3】;S3 新語料由每輪 record 重跑執法) |

本輪新增實測(進 §20 證據鏈):signedexpr gg 守衛誤判復現(18/23/18)+select -count 計數形+
帶守衛端到端 33/33+六設計 ports 表+347 killed 全量(0 等價)+staging 綁定點逐行核實+esc
\t\n\r 保留字面核實。

## 11. 追蹤(doc-debt 與交接;S3 實施輪登記,2026-11-07)

本節由 S3 實施輪按升級裁定追加,不改 §0..§10 與修訂記錄。

- **[已清償 2026-11-07 S4] §16.1.1 表欠條**:清償處=RTL2SPEC-PLUGIN §20.5 釘選全表(49 檔,現行釘選集 SSOT)+判據變更記錄;
  清償閘=run-plugin-check ⑤e(全表==pinned-files.list 逐檔名、來源欄==清單分組、§16.1.1 行對拍歷史表)+⑤f(本檔無欠條標記)。
  以下為原欠條內容(存檔):現狀:pinned-files.list 49 檔(37=§16.1.1 原表+12=S3 擴檔),
  §16.1.1 表仍只列 37 檔。S4 寫 §20 時一次補齊:①表條目與 pinned-files.list 逐檔名一致;②判據變更記錄——舊判據「<3 條規則
  無斷言價值」(條數代理)→新判據「conformance staging ADMITTED=TB 實殺變異的直接證據」,9 個不足 3 條的新釘設計列為更迭證據:
  always01、always02、attrib01_module、attrib02_port_decl、attrib06_operator_suffix、attrib08_mod_inst、loop_prefix_case、retime、scopes。
  **S4 閘必須斷言清償**:表條目與清單逐檔名對拍一致且本欠條已刪,未清=閘紅。
  - 數字對照(如實):裁定原文寫「欠條=28 條目」「pinned-files.list 條目數=28」。28 是 S3 收編的設計數(16 原已釘+12 新釘),
    清單實際條目數是 49。機械斷言按實數落:golden-check.mjs `EXPECTED_PINNED=49`(golden 98/98);conformance-eval 主根斷言每個設計
    的 specPath 在 golden/ 下且 src∈釘選清單(34 個設計全過)。
- **[交接] constmuldivmod 的 X 語義案例**:killed 全母體自檢抓到 3 個 TB 殺得掉、yosys 卻證 equivalent 的變異(m0003/m0033/m0064,
  `A % 8'd0`→`A % 8'd1` 類;除零在模擬是 x,yosys 把未定義當任意值)。交等價裁決腿 eqverdict 分類面接手,不阻塞 S3;
  登記=REGISTRY D-c5,CONFORMANCE-PROTOCOL §9。
- **[交接] 空真探針的 memory 盲區**:探針名單取自未跑 memory pass 的 write_json,memory 類設計會留下記憶體映射 FF 而非空真(arrays01
  實測 64 unproven)。保守方向,不會假放行;memory 類設計入場前修。登記=REGISTRY D-c6。
- **[待裁] operators**:只因 gold-vs-gold 的 `$pow` No SAT model 被拒(TB 41/125 已殺)。「能力邊界時等價腿降級為不適用而不拒設計」
  屬政策變更,本切片不放寬,留用戶裁定。
