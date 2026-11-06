# REIMPL-PROTOCOL — 盲寫復原對拍協議釘檔(S5;§5.2 S5(a);E1 協議重建版)

本檔=盲寫腿的協議釘檔:**盲寫 prompt 模板逐字+模型身份+採樣參數+組織架構**。
盲寫=輔助證據軌,**永不進確定性閘**(機械面=iverilog 編譯成敗+TB 逐場景 diff,
由 run-reimpl-eval.sh 執法);本檔禁事後改寫已跑樣本的參數記錄(append-only)。

## 0. 模型身份與採樣參數(釘檔;禁事後改)

- **模型=GLM-5.3-Flash**(與復原力弧實施 agent 同一模型;E1 原始輪亦同)。
- **採樣上限 n=3**(每設計×每組;實際 n 逐腿記錄於 §3 腿表)。
- 溫度/seed:提取器側無該暴露面(實施 agent 對話內生成),如實記錄=「採樣參數
  不可釘」——樣本間變異由 prompt 內樣本序號驅動(§1 模板 `{K}`),**非 API 級
  隨機採樣**;此為本弧誠實邊界(降級形,§5.2 S5(d) 預案)。
- 執行日期:2026-11-05。執行環境:復原力弧實施 session(與 E1 原始輪同一 agent
  家族;**與 E1 原始輪的差異=本輪 agent 上下文曾讀過原版 .v(切片實勘),盲隔離
  弱於 E1 原始輪的獨立對話**——數字僅輔助證據,泄漏核查只能證偽不能證真
  (README-e1.md §5 同款限制,本輪加重)。

## 1. 盲寫 prompt 模板(逐字;`{...}`=實例化槽位)

```text
你是 RTL 設計工程師。以下是某個 Verilog 模組的「行為規則書」(由規則提取器
從一個你不曾見過的 RTL 檔案自動生成)。請**只依據規則書內容**重寫一個行為
等價的 Verilog-2001 模組,寫到檔案 {OUT}。

規則書:
{RULEBOOK_MD}

硬性要求:
1. 模組名與埠名/埠方向/埠位寬必須使用規則書「接口契約」章的原文名與位寬;
   內部信號聲明清單亦在契約章,可整表照抄。
2. 只用規則書陳述的行為規則;規則書未覆蓋的細節以 `// ASSUMPTION:` 行內註釋
   聲明(每條一行,列出你補的假設)。
3. 本樣本=同規則書第 {K}/{N} 個樣本;與常見寫法拉開結構差異(不同狀態編碼
   組織/不同進程切分),禁止複製你上一個樣本的骨架。
4. 不得請求或引用規則書之外的任何源碼;全程「source not seen」。
5. 輸出單一自包含 .v 檔(\`timescale 1ns/1ps)。
```

實例化:A 組 {RULEBOOK_MD}=當前提取器完整規則書 md(含 evidence 塊+接口契約
+lint);B 組=**裁剪 evidence 塊後**的規則書 md(僅陳述+check 渲染行+接口契約,
由 scripts/reimpl-eval.mjs --trim 確定性生成,生成器可重跑)。

## 2. 兩組對拍組織架構(S5(b);按組分列禁混報)

- **A 組(完整規則書)**:測「規則書→行為」的復原上限。
- **B 組(裁剪 evidence)**:測「抄寫通道」貢獻——A/B matched 差=證據塊攜帶
  源碼原文對盲寫方的貢獻實測值;**A、B matched 數字永遠分列,禁止合併報告**
  (混報會把抄寫通道貢獻記到規則文本頭上)。
- 機械判據(兩組同形):iverilog -g2001 編譯成敗(compiled)+公共 TB 逐場景
  輸出塊與原版 diff(matched scenes)。場景集=23(6 counter+6 acc_pipe+11 i2c)。

## 3. 腿表(實跑記錄;append-only)

| 腿 | 設計 | 組 | n | 規則書時代 | 產物 | 備註 |
|---|---|---|---|---|---|---|
| counter8-A1 | counter8 | A | 1 | E1 期(post-M5) | fixtures/e1-reimpl/counter8-a1/top.v | 遷入,E1 實測 6/6 |
| i2c-A0 | i2c_master | A | 1 | **pre-S1**(無 contract) | fixtures/e1-reimpl/i2c-a0/top.v(+top_fixed 最小修復) | 遷入,E1 實測 compiled=false→修復 10/11 |
| acc-A0 | acc_pipe | A | 1 | **pre-S2**(無組合定義) | fixtures/e1-reimpl/acc-a0/top.v | 遷入,E1 實測 0/6 |
| i2c-A1..A3 | i2c_master | A | 3 | **post-S4**(contract+async+M1/M2) | build/reimpl-eval/reimpl/ | S1 消融點①+S3 消融點 |
| acc-A1..A3 | acc_pipe | A | 3 | **post-S4** | build/reimpl-eval/reimpl/ | S2 消融點② |
| i2c-B1 | i2c_master | B | 1 | post-S4 裁剪 evidence | build/reimpl-eval/reimpl/ | 抄寫通道組 |
| acc-B1 | acc_pipe | B | 1 | post-S4 裁剪 evidence | build/reimpl-eval/reimpl/ | 抄寫通道組 |

## 4. 機械執行

`bash plugin/rtl2spec/run-reimpl-eval.sh`:
① iverilog 守衛(在場+版本記錄;缺場 exit 2——run-golden-regression.sh:24-28
   守衛形先例);②23 場景逐腿編譯+運行+逐場景 diff;③verdict JSON+報告 md
   寫只落 plugin/rtl2spec/build/reimpl-eval/(直調不進 regress);
   ④消融點數字按切片分段輸出(S1/S2/S3+抄寫通道),禁混報。
