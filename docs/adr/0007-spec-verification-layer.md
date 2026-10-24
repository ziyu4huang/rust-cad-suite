# ADR-0007: Spec 驗證層 — lockstep RTL↔golden model + 逐週期不變量

狀態:Accepted(2026-10-23,含一項 open item)。前序:ADR-0003(rVPI)、ADR-0005(GUI 憲法)。

## 背景

使用者目標:開發工具「了解 RTL 的意圖、逆向 spec、驗證 spec 正確性」。方法論裁決
(planner 兩實證探針+對抗審 SHIP-WITH-FIXES 五修):
- 逆向 spec 是推斷問題,工程正解=三層機器可查 spec:①介面契約 ②可執行 golden
  model(Rust 原生,非 DSLX/XLS)③逐週期協議不變量。
- XLS 保持選配合成上游(純 CLI/CI);spec 層 Rust 原生=wasm 可達(ADR-0005 憲法)。
- 刺激用**程式化驅動**(harness 即 testbench)——同一驅動餵 RTL 與 golden,
  lockstep 才成立。PLAN §8 #5(Rust 原生 cosim API)的落地。

## 決策

1. **`Sim::run_until(t)`**(核心唯一變更):sched_loop 抽取,run() 位元組不變
   (57/57 e2e 證明)。init_once(不重播 runnable 種子)、EOS 恰一次、雙 run()
   冪等(D13 修訂)、**D17:rVPI timer 期限參與時間推進**(舊行為拖到下一 #
   事件才補觸發=vpi_gate 期望 20000→16000 更新=C-VPI 準點語義)。
2. **`crates/vl-spec`**:Signals 句柄快照/Invariant trait/GoldenModel trait/
   LockstepSpec(cycle()=clk-low 驅動→兩半 run_until→model.step→投影比對→
   不變量;post-half 刷新鉤子給匯流排模型)。零第三方依賴,wasm 安全。
3. **`crates/spec-i2c`**(publish=false):乾淨室 I2C master RTL(~200 行,V2005
   已驗證子集;OpenCores GPL 唯讀啟發,零代碼拷貝)+ golden model(自帶 bus+slave
   視圖)+ 匯流排驅動器(wire-AND+行為 slave)+ 4 不變量 + scripted/隨機(LCG 固定
   種子)/負向變種測試。
4. **Tick 紀律**:輸入只在 clk-low 變化;put+run_until 結構性成對(put 喚醒不執行);
   取樣在 run_until 返回後(槽完全排空)。正典復位序:**先充高位再拉低**
   (0-init 政策下輸入埠註冊時已為 0,直接「拉低」無沿可觸發——1→0 才是復位)。
5. **驅動器契約**(canon):`we` 只在 tip==0(HOLD/IDLE)時發;與消費沿同拍的
   strobe 在任何 RTL 都有鎖存/FSM 仲裁歧義——輪詢 tip 是唯一正解(實錄:壓沿
   strobe 使 RTL 吞命令/golden 排隊=雙發散)。
6. **偵測力證明**(負向變種):LSB-first RTL→lockstep 腿 80 違例整批抓到;
   no-ack-release RTL→ack_release 不變量命中。golden↔RTL 同作者風險的緩解棧=
   協議不變量(獨立於 model)+ oracle 語料腿(待辦,見下)+ 負向偵測。

## 已抓到的真缺陷(本弧副產品)

- **`~a[i]` 位選取反的上下文寬度 bug**(eval.rs UnOp::BitNot):上下文寬度先擴展
  運算元再取反(IEEE Table 5-22:~ 運算元自決定,結果才擴展)——`wire [7:0] w =
  ~a[7]` 產 8'b11111110 應為 0。e2e 語料未踩過(57/57 證明修復無漂移);由 I2C
  lockstep 首跑即抓——**spec 工具抓模擬器自身缺陷的第一個實例**。

## 非目標/待辦

- ✅ corpus 59 oracle 腿(12083a8):行為級開漏 slave TB(獨立第三作者)+
  verilator 錄製;e2e 58/58@100%。
- ✅ RD 讀事務鎖步測試(12083a8):dout 絕對承載檢查(0xA5);**首跑連抓兩真
  bug**=①vl-sim 首沿丟失(init_once 只播種不執行 prologue;修=排空+寫入前
  播種,shift_repro 三層釘死)②I2C RTL 的 ANSI 埠範圍繼承(1-bit 埠繼承前埠
  寬度,verilator 同解=oracle 一致≠符合自身 spec;修=1-bit 埠全前置)。
- ✅ MsbFirst 取樣器(1024 收口):取樣點移到 SCL 高半第二循環(整個高半準位
  恆定,免疫 harness 的 1-clk-stale 暫存視野;低半取樣曾在位元邊界讀到前一位
  元=0x92);**取樣 master 驅動輸出而非匯流排準位**(slave 對 LSB=1 位元組進
  入讀模式會驅動匯流排,bus-level 採樣被污染)+expected 解碼即消費(START
  phantom 重移位暫存器殘值,恰好一次解碼/期望);LSB-first 負向斷言升級=明確
  要求 msb_first_bit_order。
- ✅ wasm/demo 整合(1024):i2c_spec_check one-shot wasm 面(scenario.rs
  全組合:四不變量+canonical 腳本+三變體)+demo spec 卡(三變體 radio/違例
  表/SVG 波形/RTL 變更檢視)+wasm spec 閘(spec.mjs:0 違例/負向指名/BigInt
  跨界/雙跑確定性/錯誤路徑);目檢=docs/e2e-report/wasm-demo-v4-spec-*.png。
- 時鐘伸展負向測試、多位址/多從機。
- 效能:每週期 ~30 次 vpi 讀——demo 尺度可忽略;perf gate=M9。

## 證據

- `cargo test -p vl-sim`(step_gate 6+vpi_gate 8)
- `cargo test -p vl-spec`(spec_gate 4)
- `cargo test -p spec-i2c`(scripted+lsb/noack 負向+randomized 20 事務;
  msb 取樣器 open item=#[ignore])
- e2e 57/57 `--min-pass 100`(run() 零漂移)
