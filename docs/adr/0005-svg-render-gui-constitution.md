# ADR-0005: SVG 波形渲染層 + GUI 憲法(GUI=web/底層=Rust/CLI-web 同源)

狀態:Accepted(2026-10-23)。前序:ADR-0003(facade 慣例)、ADR-0004(vl-vcd 單一核心)。

## 背景(使用者原話逐字)

> GUI 我們都用web做,需要的底層都用 rust寫,然後cli 與web都共用 web透過wasm 共用,
> 波型生成要用SVG ,不要是圖片型式

拆分前:波形由 demo 頁的 JS canvas `draw()` 手繪(命令式繪圖,驗證只能截圖目檢);
「GUI 產物」沒有進入本 repo 的差分測試哲學。

## 決策

1. **GUI 憲法**(入 AGENTS §10.8):GUI 一律 web;GUI 所需底層一律 Rust 核心
   先落(承 §10.7「先落核心再露面」);CLI 與 web 共用核心,web 經 wasm;
   波形渲染=vl-vcd render 出 SVG——**點陣 canvas 波形=違例**。
2. **渲染住核心**:`vl-vcd/src/render.rs`,`VcdFile → SVG 字串`,零新依賴。
   「可 diff 的 GUI 產物」=SVG 是文字,入 golden byte-diff,與 expect.vcd
   同一紀律;canvas 做不到。
3. **確定性契約**(golden 以機械閘釘死):整數像素座標(u128 中間式);
   列序=signals 給序或宣告序;無時間戳/版本字串/環境資料;
   值一律進 `<title>` tooltip(v1 不畫匯流排值文字);前窗無變更=x 帶
   (誠實未知);向量帶謂詞=含 x 紅/含 z 灰/否則匯流排帶+變更處細刻線;
   real 逐值 f64 映射、不可解析斷線;字串 24 char 硬上限無省略號。
4. **XML 轉義鐵律**:名稱/字串值/real 原文/timescale 全走 `esc()`
   (`&` 先行,`< > " '` 隨後,丟棄 XML 1.0 非法控制字元);資料永不進屬性;
   demo 的 siglist 保持 `textContent`。hostile fixture 三面釘死
   (render 單測/wasm 閘/`<script>`/`<img`/`\a,b` 樣本)。
5. **三面同源**:
   - CLI=**獨立二進位 `vcd2svg`**(vl-vcd `[[bin]]`;輸入=任意 VCD 含 oracle 產;
     `-o` 預設 stdout;未知旗標=硬錯 exit 1 同 rverilator 契約)。
     拒 `--wave-svg` 旗標(翻轉位置參數語意)與子命令(verilator 無子命令;
     GTKWave 家族先例=fst2vcd/vcd2lxt 獨立轉換器)。
   - wasm=**`VcdJs::to_svg`**(句柄法:toggle 重渲不重解析;one-shot
     `vcd_to_svg` 糖);時間參數=**u64(BigInt 跨界,承 §10.7)+flags 位遮罩**
     (對抗審 P1-3:f64 哨兵會造成跨層精度分歧,違反「三層一真相」)。
   - FFI=**`rcs_vcd_svg(text)`** 僅預設渲染;ABI v1 凍結註記
     (新旋鈕=新進入點;行式 manifest 哲學不引入 JSON)。
6. **golden 來源治本**(對抗審 P1-1):`.gitignore` 加 `!corpus/**/*.vcd`
   否定式,oracle 錄製的 8 個 .vcd(各 4KB)入 git——golden 單一事實來源,
   順手修好 vcd_gate/reader fixture 在 fresh clone 的既有 panic
   (`tests/reader_fixtures.rs` 的 disk fixture 現在必在)。
   再生成黃金=vcd2svg 重跑+人眼過圖+golden 隨碼 commit(expected.stdout 慣例)。

## 後果

- demo 頁零 canvas;SVG 經 `innerHTML` 注入(唯一注入點,esc() 覆蓋;
  siglist 不走 innerHTML)。切換=持 `VcdJs` 重呼 `to_svg`(閉包私有 per-view
  狀態——模組級共享曾致跨卡 E105 混染,實錄入 REFLECTION)。
- demo 名稱列排序是 JS 的選擇;renderer 以參數收序。我方 dump 宣告序
  ==writer 平面名排序,故 CLI/golden 行序天然一致。
- 規模邊界(誠實):SVG 適配本 repo 語料尺度(幾十訊號/幾百~幾千變更);
  數萬邊以上=culling 旋鈕(t0/t1/signals)先擋,downsampling 列非目標
  (canvas/WebGL 屬檢視器工程,不在模擬器範圍)。
- 非目標:pan/zoom(DOM 層)、scope 摺疊、delta cycle、FST、
  `$timezero` 套用(ADR-0004 立場)、匯流排畫布值文字(v1)。

## 證據

- golden:`cargo test -p vl-vcd`(render 單測 7+golden/CLI 4;
  goldens/counter.svg byte-diff,CLI stdout 與 wasm one-shot 三面同字節)。
- 閘:regress.sh wasm `vcd_svg`(golden 對比+子集+窗口+hostile+錯誤路徑)
  +ffi `vcd_svg`(run→dump→渲染→q 終值 title 斷言)。
- 目檢:docs/e2e-report/wasm-demo-v3-svg-verified.png(+accpipe)。