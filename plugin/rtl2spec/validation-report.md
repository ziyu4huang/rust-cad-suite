# rtl2spec 真實 RTL 驗證報告

- 日期:2026-11-05
- 命令行:`bash plugin/rtl2spec/run-rtl-validation.sh`(機械生成禁手改)
- 工具鏈:bun 1.4.2、wasm sha256 `4b143d3afd22c41f85a0c2cebc23edd0c0176e3eb0f2ae88849fc4c8d855f3cd`
- 本檔=閘契約 V1..V6 的證據頁;評審員區外全部機械生成。

## 閘契約表(V1..V6)

| 判據 | 閾值 | 實測 | 判定 |
|---|---|---|---|
| V1 規則數 | 全集 ≥50;單檔 ≥3;truncated 非空→FAIL | 全集 325;單檔最小 4;截斷模組 0 | PASS |
| V2 行為占比 | 全集 ≥60%;任一檔 ≥40%(冗餘防線,主判據=V5-①+V4,R1-7) | 全集 100.0%;單檔最小 100.0% | PASS |
| V3 類別覆蓋 | ≥4(floor,ask 契約;目標 ≥6) | 5 類:reset/fsm/timing/counter/arithmetic | PASS |
| V4 溯源 | SV 側 100% 帶 file:line 且 snippet 逐位元組相等(拼接規則=各行原文 \n 連接,R1-10) | 325/325 規則過(checker 獨立重讀源檔對拍) | PASS |
| V5 反 doc-dump | ①行為證 ②form 五形+欄位非空(relation when 在場必非空,R3-②) ③鍵串零重複 ④陳述含源碼標識符(②④=冗餘防線) | ①②③④ 全過:325/325 指紋零重複 | PASS |
| V6 全檔成功 | 13/13 | 13/13(本輪 skipped=0) | PASS |

規則總數 325(行為類 325);破口總數 39;主判據聲明=V5-①(行為證)+V4(逐位元組溯源),V2/V5-④ 為守生成端鎖的冗餘防線(R1-7 定位聲明)。

## license 表(驗證集 13 檔;全 in-git,零 GPL)

| 檔案 | license |
|---|---|
| corpus/e2e/59_i2c_master/i2c_master.v | repo MIT |
| crates/spec-i2c/rtl/i2c_master.v | repo MIT(乾淨室;OpenCores 唯讀啟發零拷貝) |
| corpus/e2e/58_xls_acc_pipe/acc_pipe.v | Apache-2.0 |
| corpus/e2e/57_xls_add3/add3.v | Apache-2.0 |
| corpus-external/yosys-simple/subbytes.v | ISC(COPYING 隨附;唯讀引用,原檔不入 plugin/) |
| corpus-external/yosys-simple/fiedler-cooley.v | ISC |
| corpus-external/yosys-simple/sincos.v | ISC |
| corpus-external/yosys-simple/multiplier.v | ISC |
| corpus-external/yosys-simple/memory.v | ISC |
| corpus-external/yosys-simple/carryadd.v | ISC |
| corpus-external/yosys-simple/generate.v | ISC |
| corpus-external/yosys-simple/mem2reg.v | ISC |
| corpus-external/yosys-simple/dff_different_styles.v | ISC |

## 逐檔表

| 檔案 | 行數 | 模組數 | 規則數 | 類別分布 | 破口數(按類) | 判定 |
|---|---|---|---|---|---|---|
| corpus/e2e/59_i2c_master/i2c_master.v | 317 | 1 | 69 | reset=20 fsm=9 timing=38 counter=2 | 0 | PASS |
| crates/spec-i2c/rtl/i2c_master.v | 315 | 1 | 69 | reset=20 fsm=9 timing=38 counter=2 | 0 | PASS |
| corpus/e2e/58_xls_acc_pipe/acc_pipe.v | 45 | 1 | 7 | timing=5 arithmetic=2 | missing_reset=3 | PASS |
| corpus/e2e/57_xls_add3/add3.v | 34 | 1 | 6 | timing=4 arithmetic=2 | missing_reset=2 | PASS |
| corpus-external/yosys-simple/subbytes.v | 83 | 1 | 7 | reset=3 timing=3 arithmetic=1 | latch=1 | PASS |
| corpus-external/yosys-simple/fiedler-cooley.v | 34 | 1 | 11 | timing=11 | missing_reset=1 | PASS |
| corpus-external/yosys-simple/sincos.v | 125 | 1 | 46 | reset=8 timing=31 counter=7 | incomplete_case=1 missing_reset=3 | PASS |
| corpus-external/yosys-simple/multiplier.v | 133 | 4 | 10 | arithmetic=10 | 0 | PASS |
| corpus-external/yosys-simple/memory.v | 320 | 15 | 59 | reset=18 timing=41 | missing_reset=15 undriven_read=4 multi_driver=1 | PASS |
| corpus-external/yosys-simple/carryadd.v | 25 | 1 | 4 | arithmetic=4 | 0 | PASS |
| corpus-external/yosys-simple/generate.v | 326 | 9 | 15 | arithmetic=9 timing=6 | missing_reset=2 undriven_read=1 | PASS |
| corpus-external/yosys-simple/mem2reg.v | 117 | 6 | 5 | reset=1 timing=3 arithmetic=1 | undriven_read=1 missing_reset=1 | PASS |
| corpus-external/yosys-simple/dff_different_styles.v | 104 | 9 | 17 | timing=12 reset=5 | missing_reset=3 | PASS |

## 規則書全文樣本(代表檔)

### i2c_master(corpus/e2e/59,repo MIT)

# 規則書:corpus/e2e/59_i2c_master/i2c_master.v

- 模組數:1;規則總數:69

## 模組 i2c_master(69 條)

### i2c_master.R001 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,state 裝載 ST_IDLE
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=state←ST_IDLE
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|state|!rst_n|state<=ST_IDLE`

### i2c_master.R002 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,cnt 裝載 16'd0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=cnt←16'd0
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|cnt|!rst_n|cnt<=16'd0`

### i2c_master.R003 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,half 裝載 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=half←1'b0
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|half|!rst_n|half<=1'b0`

### i2c_master.R004 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,bitcnt 裝載 4'd0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=bitcnt←4'd0
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|bitcnt|!rst_n|bitcnt<=4'd0`

### i2c_master.R005 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,step 裝載 2'd0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=step←2'd0
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|step|!rst_n|step<=2'd0`

### i2c_master.R006 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,tx 裝載 8'd0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=tx←8'd0
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|tx|!rst_n|tx<=8'd0`

### i2c_master.R007 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,rd_mode 裝載 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=rd_mode←1'b0
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|rd_mode|!rst_n|rd_mode<=1'b0`

### i2c_master.R008 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,pend_start 裝載 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=pend_start←1'b0
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|pend_start|!rst_n|pend_start<=1'b0`

### i2c_master.R009 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,pend_stop 裝載 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=pend_stop←1'b0
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|pend_stop|!rst_n|pend_stop<=1'b0`

### i2c_master.R010 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,pend_wr 裝載 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=pend_wr←1'b0
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|pend_wr|!rst_n|pend_wr<=1'b0`

### i2c_master.R011 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,pend_rd 裝載 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=pend_rd←1'b0
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|pend_rd|!rst_n|pend_rd<=1'b0`

### i2c_master.R012 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,scl_o 裝載 1'b1
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=scl_o←1'b1
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|scl_o|!rst_n|scl_o<=1'b1`

### i2c_master.R013 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,sda_o 裝載 1'b1
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=sda_o←1'b1
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|sda_o|!rst_n|sda_o<=1'b1`

### i2c_master.R014 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,scl_oe 裝載 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=scl_oe←1'b0
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|scl_oe|!rst_n|scl_oe<=1'b0`

### i2c_master.R015 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,sda_oe 裝載 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=sda_oe←1'b0
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|sda_oe|!rst_n|sda_oe<=1'b0`

### i2c_master.R016 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,busy 裝載 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=busy←1'b0
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|busy|!rst_n|busy<=1'b0`

### i2c_master.R017 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,tip 裝載 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=tip←1'b0
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|tip|!rst_n|tip<=1'b0`

### i2c_master.R018 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,done 裝載 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=done←1'b0
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|done|!rst_n|done<=1'b0`

### i2c_master.R019 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,ack_rx 裝載 1'b1
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=ack_rx←1'b1
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|ack_rx|!rst_n|ack_rx<=1'b1`

### i2c_master.R020 [reset/derived]

- 陳述:當 !rst_n 且 clk 上升沿 時,dout 裝載 8'd0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:71-92(process)
  ```verilog
      if (!rst_n) begin
        state <= ST_IDLE;
        cnt <= 16'd0;
        half <= 1'b0;
        bitcnt <= 4'd0;
        step <= 2'd0;
        tx <= 8'd0;
        rd_mode <= 1'b0;
        pend_start <= 1'b0;
        pend_stop <= 1'b0;
        pend_wr <= 1'b0;
        pend_rd <= 1'b0;
        scl_o <= 1'b1;
        sda_o <= 1'b1;
        scl_oe <= 1'b0;
        sda_oe <= 1'b0;
        busy <= 1'b0;
        tip <= 1'b0;
        done <= 1'b0;
        ack_rx <= 1'b1;
        dout <= 8'd0;
      end else begin
  ```
- check(guard):trigger=clk posedge; when=!rst_n; effects=dout←8'd0
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|reset|i2c_master|dout|!rst_n|dout<=8'd0`

### i2c_master.R021 [fsm/derived]

- 陳述:state=ST_IDLE 且 pend_start 時,下一拍 state=ST_START
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:107-126(case)
  ```verilog
          ST_IDLE: begin
            scl_oe <= 1'b0;
            sda_oe <= 1'b0;
            scl_o <= 1'b1;
            sda_o <= 1'b1;
            busy <= 1'b0;
            tip <= 1'b0;
            cnt <= 16'd0;
            half <= 1'b0;
            bitcnt <= 4'd0;
            step <= 2'd0;
            if (pend_start) begin
              pend_start <= 1'b0;
              busy <= 1'b1;
              rd_mode <= 1'b0;
              step <= 2'd0;
              cnt <= 16'd0;
              state <= ST_START;
            end
          end
  ```
- check(transition):state: ST_IDLE → ST_START when pend_start
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|fsm|i2c_master|state|pend_start|ST_START`

### i2c_master.R022 [fsm/derived]

- 陳述:state=ST_START 且 !(step == 2'd0) 且 !(step == 2'd1) 且 cnt_end 時,下一拍 state=ST_SHIFT
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:128-161(case)
  ```verilog
          ST_START: begin
            // step 0: both lines released (SCL high, SDA high)
            // step 1: SDA falls while SCL high (the START condition)
            // step 2: SCL falls; enter the shift state
            if (step == 2'd0) begin
              scl_oe <= 1'b0;
              sda_oe <= 1'b0;
              sda_o <= 1'b1;
              if (cnt_end) begin
                step <= 2'd1;
                cnt <= 16'd0;
              end else cnt <= cnt + 16'd1;
            end else if (step == 2'd1) begin
              sda_o <= 1'b0;
              sda_oe <= 1'b1;
              if (cnt_end) begin
                step <= 2'd2;
                cnt <= 16'd0;
              end else cnt <= cnt + 16'd1;
            end else begin
              scl_oe <= 1'b1;
              scl_o <= 1'b0;
              sda_oe <= 1'b1;
              sda_o <= 1'b0;
              if (cnt_end) begin
                step <= 2'd0;
                cnt <= 16'd0;
                half <= 1'b0;
                bitcnt <= 4'd0;
                tip <= 1'b1;
                state <= ST_SHIFT;
              end else cnt <= cnt + 16'd1;
            end
          end
  ```
- check(transition):state: ST_START → ST_SHIFT when !(step == 2'd0) 且 !(step == 2'd1) 且 cnt_end
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|fsm|i2c_master|state|!(step == 2'd0) 且 !(step == 2'd1) 且 cnt_end|ST_SHIFT`

### i2c_master.R023 [fsm/derived]

- 陳述:state=ST_SHIFT 且 !(half == 1'b0) 且 cnt_end 且 bitcnt == 4'd7 時,下一拍 state=ST_ACK
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:163-199(case)
  ```verilog
          ST_SHIFT: begin
            if (half == 1'b0) begin
              // SCL low half: master drives SCL low; data settles here.
              scl_oe <= 1'b1;
              scl_o <= 1'b0;
              if (rd_mode) begin
                sda_oe <= 1'b0;  // release for the slave to drive
                sda_o <= 1'b1;
              end else begin
                sda_oe <= ~tx[7 - bitcnt[2:0]];
                sda_o <= 1'b0;
              end
              if (cnt_end) begin
                half <= 1'b1;
                scl_oe <= 1'b0;  // release: SCL rises
                cnt <= 16'd0;
              end else cnt <= cnt + 16'd1;
            end else begin
              // SCL high half: data stable. Sample RX at the end of the high
              // half (before the falling edge).
              if (cnt_end) begin
                cnt <= 16'd0;
                half <= 1'b0;
                scl_oe <= 1'b1;
                scl_o <= 1'b0;
                if (rd_mode) begin
                  tx <= {tx[6:0], sda_i};
                end
                if (bitcnt == 4'd7) begin
                  bitcnt <= 4'd0;
                  state <= ST_ACK;
                end else begin
                  bitcnt <= bitcnt + 4'd1;
                end
              end else cnt <= cnt + 16'd1;
            end
          end
  ```
- check(transition):state: ST_SHIFT → ST_ACK when !(half == 1'b0) 且 cnt_end 且 bitcnt == 4'd7
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|fsm|i2c_master|state|!(half == 1'b0) 且 cnt_end 且 bitcnt == 4'd7|ST_ACK`

### i2c_master.R024 [fsm/derived]

- 陳述:state=ST_ACK 且 !(half == 1'b0) 且 cnt_end 時,下一拍 state=ST_HOLD
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:201-237(case)
  ```verilog
          ST_ACK: begin
            if (half == 1'b0) begin
              // 9th clock low half: WR -> release SDA (slave acks);
              // RD -> drive ACK (master always acks, documented).
              scl_oe <= 1'b1;
              scl_o <= 1'b0;
              if (rd_mode) begin
                sda_oe <= 1'b1;
                sda_o <= 1'b0;
              end else begin
                sda_oe <= 1'b0;
                sda_o <= 1'b1;
              end
              if (cnt_end) begin
                half <= 1'b1;
                scl_oe <= 1'b0;
                cnt <= 16'd0;
              end else cnt <= cnt + 16'd1;
            end else begin
              // 9th clock high half: WR samples the slave ack at the end.
              if (cnt_end) begin
                cnt <= 16'd0;
                half <= 1'b0;
                scl_oe <= 1'b1;
                scl_o <= 1'b0;
                if (!rd_mode) begin
                  ack_rx <= sda_i;
                  dout <= tx;
                end else begin
                  dout <= tx;
                end
                done <= 1'b1;
                tip <= 1'b0;
                state <= ST_HOLD;
              end else cnt <= cnt + 16'd1;
            end
          end
  ```
- check(transition):state: ST_ACK → ST_HOLD when !(half == 1'b0) 且 cnt_end
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|fsm|i2c_master|state|!(half == 1'b0) 且 cnt_end|ST_HOLD`

### i2c_master.R025 [fsm/derived]

- 陳述:state=ST_HOLD 且 pend_wr 時,下一拍 state=ST_SHIFT
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:239-276(case)
  ```verilog
          ST_HOLD: begin
            // Between bytes: SCL held low (the classic stretch position),
            // SDA released. Next queued command decides the path.
            scl_oe <= 1'b1;
            scl_o <= 1'b0;
            sda_oe <= 1'b0;
            sda_o <= 1'b1;
            if (pend_wr) begin
              pend_wr <= 1'b0;
              rd_mode <= 1'b0;
              bitcnt <= 4'd0;
              half <= 1'b0;
              cnt <= 16'd0;
              tip <= 1'b1;
              state <= ST_SHIFT;
            end else if (pend_rd) begin
              pend_rd <= 1'b0;
              rd_mode <= 1'b1;
              tx <= 8'd0;
              bitcnt <= 4'd0;
              half <= 1'b0;
              cnt <= 16'd0;
              tip <= 1'b1;
              state <= ST_SHIFT;
            end else if (pend_stop) begin
              pend_stop <= 1'b0;
              step <= 2'd0;
              cnt <= 16'd0;
              state <= ST_STOP;
            end else if (pend_start) begin
              pend_start <= 1'b0;
              step <= 2'd0;
              cnt <= 16'd0;
              sda_oe <= 1'b1;  // SDA back high for the restart sequence
              sda_o <= 1'b1;
              state <= ST_START;
            end
          end
  ```
- check(transition):state: ST_HOLD → ST_SHIFT when pend_wr
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|fsm|i2c_master|state|pend_wr|ST_SHIFT`

### i2c_master.R026 [fsm/derived]

- 陳述:state=ST_HOLD 且 !(pend_wr) 且 pend_rd 時,下一拍 state=ST_SHIFT
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:239-276(case)
  ```verilog
          ST_HOLD: begin
            // Between bytes: SCL held low (the classic stretch position),
            // SDA released. Next queued command decides the path.
            scl_oe <= 1'b1;
            scl_o <= 1'b0;
            sda_oe <= 1'b0;
            sda_o <= 1'b1;
            if (pend_wr) begin
              pend_wr <= 1'b0;
              rd_mode <= 1'b0;
              bitcnt <= 4'd0;
              half <= 1'b0;
              cnt <= 16'd0;
              tip <= 1'b1;
              state <= ST_SHIFT;
            end else if (pend_rd) begin
              pend_rd <= 1'b0;
              rd_mode <= 1'b1;
              tx <= 8'd0;
              bitcnt <= 4'd0;
              half <= 1'b0;
              cnt <= 16'd0;
              tip <= 1'b1;
              state <= ST_SHIFT;
            end else if (pend_stop) begin
              pend_stop <= 1'b0;
              step <= 2'd0;
              cnt <= 16'd0;
              state <= ST_STOP;
            end else if (pend_start) begin
              pend_start <= 1'b0;
              step <= 2'd0;
              cnt <= 16'd0;
              sda_oe <= 1'b1;  // SDA back high for the restart sequence
              sda_o <= 1'b1;
              state <= ST_START;
            end
          end
  ```
- check(transition):state: ST_HOLD → ST_SHIFT when !(pend_wr) 且 pend_rd
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|fsm|i2c_master|state|!(pend_wr) 且 pend_rd|ST_SHIFT`

### i2c_master.R027 [fsm/derived]

- 陳述:state=ST_HOLD 且 !(pend_wr) 且 !(pend_rd) 且 pend_stop 時,下一拍 state=ST_STOP
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:239-276(case)
  ```verilog
          ST_HOLD: begin
            // Between bytes: SCL held low (the classic stretch position),
            // SDA released. Next queued command decides the path.
            scl_oe <= 1'b1;
            scl_o <= 1'b0;
            sda_oe <= 1'b0;
            sda_o <= 1'b1;
            if (pend_wr) begin
              pend_wr <= 1'b0;
              rd_mode <= 1'b0;
              bitcnt <= 4'd0;
              half <= 1'b0;
              cnt <= 16'd0;
              tip <= 1'b1;
              state <= ST_SHIFT;
            end else if (pend_rd) begin
              pend_rd <= 1'b0;
              rd_mode <= 1'b1;
              tx <= 8'd0;
              bitcnt <= 4'd0;
              half <= 1'b0;
              cnt <= 16'd0;
              tip <= 1'b1;
              state <= ST_SHIFT;
            end else if (pend_stop) begin
              pend_stop <= 1'b0;
              step <= 2'd0;
              cnt <= 16'd0;
              state <= ST_STOP;
            end else if (pend_start) begin
              pend_start <= 1'b0;
              step <= 2'd0;
              cnt <= 16'd0;
              sda_oe <= 1'b1;  // SDA back high for the restart sequence
              sda_o <= 1'b1;
              state <= ST_START;
            end
          end
  ```
- check(transition):state: ST_HOLD → ST_STOP when !(pend_wr) 且 !(pend_rd) 且 pend_stop
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|fsm|i2c_master|state|!(pend_wr) 且 !(pend_rd) 且 pend_stop|ST_STOP`

### i2c_master.R028 [fsm/derived]

- 陳述:state=ST_HOLD 且 !(pend_wr) 且 !(pend_rd) 且 !(pend_stop) 且 pend_start 時,下一拍 state=ST_START
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:239-276(case)
  ```verilog
          ST_HOLD: begin
            // Between bytes: SCL held low (the classic stretch position),
            // SDA released. Next queued command decides the path.
            scl_oe <= 1'b1;
            scl_o <= 1'b0;
            sda_oe <= 1'b0;
            sda_o <= 1'b1;
            if (pend_wr) begin
              pend_wr <= 1'b0;
              rd_mode <= 1'b0;
              bitcnt <= 4'd0;
              half <= 1'b0;
              cnt <= 16'd0;
              tip <= 1'b1;
              state <= ST_SHIFT;
            end else if (pend_rd) begin
              pend_rd <= 1'b0;
              rd_mode <= 1'b1;
              tx <= 8'd0;
              bitcnt <= 4'd0;
              half <= 1'b0;
              cnt <= 16'd0;
              tip <= 1'b1;
              state <= ST_SHIFT;
            end else if (pend_stop) begin
              pend_stop <= 1'b0;
              step <= 2'd0;
              cnt <= 16'd0;
              state <= ST_STOP;
            end else if (pend_start) begin
              pend_start <= 1'b0;
              step <= 2'd0;
              cnt <= 16'd0;
              sda_oe <= 1'b1;  // SDA back high for the restart sequence
              sda_o <= 1'b1;
              state <= ST_START;
            end
          end
  ```
- check(transition):state: ST_HOLD → ST_START when !(pend_wr) 且 !(pend_rd) 且 !(pend_stop) 且 pend_start
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|fsm|i2c_master|state|!(pend_wr) 且 !(pend_rd) 且 !(pend_stop) 且 pend_start|ST_START`

### i2c_master.R029 [fsm/derived]

- 陳述:state=ST_STOP 且 !(step == 2'd0) 且 !(step == 2'd1) 且 !(step == 2'd2) 時,下一拍 state=ST_IDLE
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:278-310(case)
  ```verilog
          ST_STOP: begin
            // step 0: drive SDA low (SCL already low)
            // step 1: release SCL (rises)
            // step 2: release SDA while SCL high (the STOP condition)
            // step 3: one more half, then IDLE
            if (step == 2'd0) begin
              sda_oe <= 1'b1;
              sda_o <= 1'b0;
              scl_oe <= 1'b1;
              scl_o <= 1'b0;
              if (cnt_end) begin
                step <= 2'd1;
                cnt <= 16'd0;
              end else cnt <= cnt + 16'd1;
            end else if (step == 2'd1) begin
              scl_oe <= 1'b0;
              if (cnt_end) begin
                step <= 2'd2;
                cnt <= 16'd0;
              end else cnt <= cnt + 16'd1;
            end else if (step == 2'd2) begin
              sda_oe <= 1'b0;
              sda_o <= 1'b1;
              if (cnt_end) begin
                step <= 2'd3;
                cnt <= 16'd0;
              end else cnt <= cnt + 16'd1;
            end else begin
              busy <= 1'b0;
              tip <= 1'b0;
              state <= ST_IDLE;
            end
          end
  ```
- check(transition):state: ST_STOP → ST_IDLE when !(step == 2'd0) 且 !(step == 2'd1) 且 !(step == 2'd2)
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|fsm|i2c_master|state|!(step == 2'd0) 且 !(step == 2'd1) 且 !(step == 2'd2)|ST_IDLE`

### i2c_master.R030 [timing/derived]

- 陳述:clk 上升沿 成立後的下一拍,done 變為 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:93-93(process)
  ```verilog
        done <= 1'b0;
  ```
- check(sequence):anchor(clk 上升沿) +1拍 → done=1'b0 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|done||1'b0`

### i2c_master.R031 [timing/derived]

- 陳述:we 且 cmd == CMD_WR 成立後的下一拍,pend_wr 變為 1'b1
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:98-98(process)
  ```verilog
            pend_wr <= 1'b1;
  ```
- check(sequence):anchor(we 且 cmd == CMD_WR) +1拍 → pend_wr=1'b1 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|pend_wr||1'b1`

### i2c_master.R032 [timing/derived]

- 陳述:we 且 cmd == CMD_WR 成立後的下一拍,tx 變為 din
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:99-99(process)
  ```verilog
            tx <= din;
  ```
- check(sequence):anchor(we 且 cmd == CMD_WR) +1拍 → tx=din when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|tx||din`

### i2c_master.R033 [timing/derived]

- 陳述:we 且 cmd == CMD_RD 成立後的下一拍,pend_rd 變為 1'b1
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:101-101(process)
  ```verilog
          if (cmd == CMD_RD) pend_rd <= 1'b1;
  ```
- check(sequence):anchor(we 且 cmd == CMD_RD) +1拍 → pend_rd=1'b1 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|pend_rd||1'b1`

### i2c_master.R034 [timing/derived]

- 陳述:we 且 cmd == CMD_START 成立後的下一拍,pend_start 變為 1'b1
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:102-102(process)
  ```verilog
          if (cmd == CMD_START) pend_start <= 1'b1;
  ```
- check(sequence):anchor(we 且 cmd == CMD_START) +1拍 → pend_start=1'b1 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|pend_start||1'b1`

### i2c_master.R035 [timing/derived]

- 陳述:we 且 cmd == CMD_STOP 成立後的下一拍,pend_stop 變為 1'b1
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:103-103(process)
  ```verilog
          if (cmd == CMD_STOP) pend_stop <= 1'b1;
  ```
- check(sequence):anchor(we 且 cmd == CMD_STOP) +1拍 → pend_stop=1'b1 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|pend_stop||1'b1`

### i2c_master.R036 [timing/derived]

- 陳述:state == ST_IDLE 成立後的下一拍,scl_oe 變為 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:108-108(process)
  ```verilog
            scl_oe <= 1'b0;
  ```
- check(sequence):anchor(state == ST_IDLE) +1拍 → scl_oe=1'b0 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|scl_oe||1'b0`

### i2c_master.R037 [timing/derived]

- 陳述:state == ST_IDLE 成立後的下一拍,sda_oe 變為 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:109-109(process)
  ```verilog
            sda_oe <= 1'b0;
  ```
- check(sequence):anchor(state == ST_IDLE) +1拍 → sda_oe=1'b0 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|sda_oe||1'b0`

### i2c_master.R038 [timing/derived]

- 陳述:state == ST_IDLE 成立後的下一拍,scl_o 變為 1'b1
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:110-110(process)
  ```verilog
            scl_o <= 1'b1;
  ```
- check(sequence):anchor(state == ST_IDLE) +1拍 → scl_o=1'b1 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|scl_o||1'b1`

### i2c_master.R039 [timing/derived]

- 陳述:state == ST_IDLE 成立後的下一拍,sda_o 變為 1'b1
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:111-111(process)
  ```verilog
            sda_o <= 1'b1;
  ```
- check(sequence):anchor(state == ST_IDLE) +1拍 → sda_o=1'b1 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|sda_o||1'b1`

### i2c_master.R040 [timing/derived]

- 陳述:state == ST_IDLE 成立後的下一拍,busy 變為 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:112-112(process)
  ```verilog
            busy <= 1'b0;
  ```
- check(sequence):anchor(state == ST_IDLE) +1拍 → busy=1'b0 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|busy||1'b0`

### i2c_master.R041 [timing/derived]

- 陳述:state == ST_IDLE 成立後的下一拍,tip 變為 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:113-113(process)
  ```verilog
            tip <= 1'b0;
  ```
- check(sequence):anchor(state == ST_IDLE) +1拍 → tip=1'b0 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|tip||1'b0`

### i2c_master.R042 [timing/derived]

- 陳述:state == ST_IDLE 成立後的下一拍,cnt 變為 16'd0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:114-114(process)
  ```verilog
            cnt <= 16'd0;
  ```
- check(sequence):anchor(state == ST_IDLE) +1拍 → cnt=16'd0 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|cnt||16'd0`

### i2c_master.R043 [timing/derived]

- 陳述:state == ST_IDLE 成立後的下一拍,half 變為 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:115-115(process)
  ```verilog
            half <= 1'b0;
  ```
- check(sequence):anchor(state == ST_IDLE) +1拍 → half=1'b0 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|half||1'b0`

### i2c_master.R044 [timing/derived]

- 陳述:state == ST_IDLE 成立後的下一拍,bitcnt 變為 4'd0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:116-116(process)
  ```verilog
            bitcnt <= 4'd0;
  ```
- check(sequence):anchor(state == ST_IDLE) +1拍 → bitcnt=4'd0 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|bitcnt||4'd0`

### i2c_master.R045 [timing/derived]

- 陳述:state == ST_IDLE 成立後的下一拍,step 變為 2'd0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:117-117(process)
  ```verilog
            step <= 2'd0;
  ```
- check(sequence):anchor(state == ST_IDLE) +1拍 → step=2'd0 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|step||2'd0`

### i2c_master.R046 [timing/derived]

- 陳述:state == ST_IDLE 且 pend_start 成立後的下一拍,pend_start 變為 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:119-119(process)
  ```verilog
              pend_start <= 1'b0;
  ```
- check(sequence):anchor(state == ST_IDLE 且 pend_start) +1拍 → pend_start=1'b0 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|pend_start||1'b0`

### i2c_master.R047 [timing/derived]

- 陳述:state == ST_IDLE 且 pend_start 成立後的下一拍,busy 變為 1'b1
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:120-120(process)
  ```verilog
              busy <= 1'b1;
  ```
- check(sequence):anchor(state == ST_IDLE 且 pend_start) +1拍 → busy=1'b1 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|busy||1'b1`

### i2c_master.R048 [timing/derived]

- 陳述:state == ST_IDLE 且 pend_start 成立後的下一拍,rd_mode 變為 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:121-121(process)
  ```verilog
              rd_mode <= 1'b0;
  ```
- check(sequence):anchor(state == ST_IDLE 且 pend_start) +1拍 → rd_mode=1'b0 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|rd_mode||1'b0`

### i2c_master.R049 [timing/derived]

- 陳述:state == ST_START 且 step == 2'd0 且 cnt_end 成立後的下一拍,step 變為 2'd1
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:137-137(process)
  ```verilog
                step <= 2'd1;
  ```
- check(sequence):anchor(state == ST_START 且 step == 2'd0 且 cnt_end) +1拍 → step=2'd1 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|step||2'd1`

### i2c_master.R050 [counter/derived]

- 陳述:state == ST_START 且 step == 2'd0 且 !(cnt_end) 成立後的下一拍,cnt 變為 cnt + 16'd1(計數/環繞)
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:139-139(process)
  ```verilog
              end else cnt <= cnt + 16'd1;
  ```
- check(sequence):anchor(state == ST_START 且 step == 2'd0 且 !(cnt_end)) +1拍 → cnt=cnt + 16'd1 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|counter|i2c_master|cnt||cnt + 16'd1`

### i2c_master.R051 [timing/derived]

- 陳述:state == ST_START 且 !(step == 2'd0) 且 step == 2'd1 成立後的下一拍,sda_o 變為 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:141-141(process)
  ```verilog
              sda_o <= 1'b0;
  ```
- check(sequence):anchor(state == ST_START 且 !(step == 2'd0) 且 step == 2'd1) +1拍 → sda_o=1'b0 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|sda_o||1'b0`

### i2c_master.R052 [timing/derived]

- 陳述:state == ST_START 且 !(step == 2'd0) 且 step == 2'd1 成立後的下一拍,sda_oe 變為 1'b1
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:142-142(process)
  ```verilog
              sda_oe <= 1'b1;
  ```
- check(sequence):anchor(state == ST_START 且 !(step == 2'd0) 且 step == 2'd1) +1拍 → sda_oe=1'b1 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|sda_oe||1'b1`

### i2c_master.R053 [timing/derived]

- 陳述:state == ST_START 且 !(step == 2'd0) 且 step == 2'd1 且 cnt_end 成立後的下一拍,step 變為 2'd2
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:144-144(process)
  ```verilog
                step <= 2'd2;
  ```
- check(sequence):anchor(state == ST_START 且 !(step == 2'd0) 且 step == 2'd1 且 cnt_end) +1拍 → step=2'd2 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|step||2'd2`

### i2c_master.R054 [timing/derived]

- 陳述:state == ST_START 且 !(step == 2'd0) 且 !(step == 2'd1) 成立後的下一拍,scl_oe 變為 1'b1
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:148-148(process)
  ```verilog
              scl_oe <= 1'b1;
  ```
- check(sequence):anchor(state == ST_START 且 !(step == 2'd0) 且 !(step == 2'd1)) +1拍 → scl_oe=1'b1 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|scl_oe||1'b1`

### i2c_master.R055 [timing/derived]

- 陳述:state == ST_START 且 !(step == 2'd0) 且 !(step == 2'd1) 成立後的下一拍,scl_o 變為 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:149-149(process)
  ```verilog
              scl_o <= 1'b0;
  ```
- check(sequence):anchor(state == ST_START 且 !(step == 2'd0) 且 !(step == 2'd1)) +1拍 → scl_o=1'b0 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|scl_o||1'b0`

### i2c_master.R056 [timing/derived]

- 陳述:state == ST_START 且 !(step == 2'd0) 且 !(step == 2'd1) 且 cnt_end 成立後的下一拍,tip 變為 1'b1
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:157-157(process)
  ```verilog
                tip <= 1'b1;
  ```
- check(sequence):anchor(state == ST_START 且 !(step == 2'd0) 且 !(step == 2'd1) 且 cnt_end) +1拍 → tip=1'b1 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|tip||1'b1`

### i2c_master.R057 [timing/derived]

- 陳述:state == ST_SHIFT 且 half == 1'b0 且 !(rd_mode) 成立後的下一拍,sda_oe 變為 ~tx['sd7 - bitcnt['sd2:'sd0]]
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:172-172(process)
  ```verilog
                sda_oe <= ~tx[7 - bitcnt[2:0]];
  ```
- check(sequence):anchor(state == ST_SHIFT 且 half == 1'b0 且 !(rd_mode)) +1拍 → sda_oe=~tx['sd7 - bitcnt['sd2:'sd0]] when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|sda_oe||~tx['sd7 - bitcnt['sd2:'sd0]]`

### i2c_master.R058 [timing/derived]

- 陳述:state == ST_SHIFT 且 half == 1'b0 且 cnt_end 成立後的下一拍,half 變為 1'b1
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:176-176(process)
  ```verilog
                half <= 1'b1;
  ```
- check(sequence):anchor(state == ST_SHIFT 且 half == 1'b0 且 cnt_end) +1拍 → half=1'b1 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|half||1'b1`

### i2c_master.R059 [timing/derived]

- 陳述:state == ST_SHIFT 且 !(half == 1'b0) 且 cnt_end 且 rd_mode 成立後的下一拍,tx 變為 {tx['sd6:'sd0], sda_i}
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:189-189(process)
  ```verilog
                  tx <= {tx[6:0], sda_i};
  ```
- check(sequence):anchor(state == ST_SHIFT 且 !(half == 1'b0) 且 cnt_end 且 rd_mode) +1拍 → tx={tx['sd6:'sd0], sda_i} when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|tx||{tx['sd6:'sd0], sda_i}`

### i2c_master.R060 [counter/derived]

- 陳述:state == ST_SHIFT 且 !(half == 1'b0) 且 cnt_end 且 !(bitcnt == 4'd7) 成立後的下一拍,bitcnt 變為 bitcnt + 4'd1(計數/環繞)
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:195-195(process)
  ```verilog
                  bitcnt <= bitcnt + 4'd1;
  ```
- check(sequence):anchor(state == ST_SHIFT 且 !(half == 1'b0) 且 cnt_end 且 !(bitcnt == 4'd7)) +1拍 → bitcnt=bitcnt + 4'd1 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|counter|i2c_master|bitcnt||bitcnt + 4'd1`

### i2c_master.R061 [timing/derived]

- 陳述:state == ST_ACK 且 !(half == 1'b0) 且 cnt_end 且 !rd_mode 成立後的下一拍,ack_rx 變為 sda_i
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:227-227(process)
  ```verilog
                  ack_rx <= sda_i;
  ```
- check(sequence):anchor(state == ST_ACK 且 !(half == 1'b0) 且 cnt_end 且 !rd_mode) +1拍 → ack_rx=sda_i when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|ack_rx||sda_i`

### i2c_master.R062 [timing/derived]

- 陳述:state == ST_ACK 且 !(half == 1'b0) 且 cnt_end 且 !rd_mode 成立後的下一拍,dout 變為 tx
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:228-228(process)
  ```verilog
                  dout <= tx;
  ```
- check(sequence):anchor(state == ST_ACK 且 !(half == 1'b0) 且 cnt_end 且 !rd_mode) +1拍 → dout=tx when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|dout||tx`

### i2c_master.R063 [timing/derived]

- 陳述:state == ST_ACK 且 !(half == 1'b0) 且 cnt_end 成立後的下一拍,done 變為 1'b1
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:232-232(process)
  ```verilog
                done <= 1'b1;
  ```
- check(sequence):anchor(state == ST_ACK 且 !(half == 1'b0) 且 cnt_end) +1拍 → done=1'b1 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|done||1'b1`

### i2c_master.R064 [timing/derived]

- 陳述:state == ST_HOLD 且 pend_wr 成立後的下一拍,pend_wr 變為 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:247-247(process)
  ```verilog
              pend_wr <= 1'b0;
  ```
- check(sequence):anchor(state == ST_HOLD 且 pend_wr) +1拍 → pend_wr=1'b0 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|pend_wr||1'b0`

### i2c_master.R065 [timing/derived]

- 陳述:state == ST_HOLD 且 !(pend_wr) 且 pend_rd 成立後的下一拍,pend_rd 變為 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:255-255(process)
  ```verilog
              pend_rd <= 1'b0;
  ```
- check(sequence):anchor(state == ST_HOLD 且 !(pend_wr) 且 pend_rd) +1拍 → pend_rd=1'b0 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|pend_rd||1'b0`

### i2c_master.R066 [timing/derived]

- 陳述:state == ST_HOLD 且 !(pend_wr) 且 pend_rd 成立後的下一拍,rd_mode 變為 1'b1
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:256-256(process)
  ```verilog
              rd_mode <= 1'b1;
  ```
- check(sequence):anchor(state == ST_HOLD 且 !(pend_wr) 且 pend_rd) +1拍 → rd_mode=1'b1 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|rd_mode||1'b1`

### i2c_master.R067 [timing/derived]

- 陳述:state == ST_HOLD 且 !(pend_wr) 且 pend_rd 成立後的下一拍,tx 變為 8'd0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:257-257(process)
  ```verilog
              tx <= 8'd0;
  ```
- check(sequence):anchor(state == ST_HOLD 且 !(pend_wr) 且 pend_rd) +1拍 → tx=8'd0 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|tx||8'd0`

### i2c_master.R068 [timing/derived]

- 陳述:state == ST_HOLD 且 !(pend_wr) 且 !(pend_rd) 且 pend_stop 成立後的下一拍,pend_stop 變為 1'b0
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:264-264(process)
  ```verilog
              pend_stop <= 1'b0;
  ```
- check(sequence):anchor(state == ST_HOLD 且 !(pend_wr) 且 !(pend_rd) 且 pend_stop) +1拍 → pend_stop=1'b0 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|pend_stop||1'b0`

### i2c_master.R069 [timing/derived]

- 陳述:state == ST_STOP 且 !(step == 2'd0) 且 !(step == 2'd1) 且 step == 2'd2 且 cnt_end 成立後的下一拍,step 變為 2'd3
- 證據:corpus/e2e/59_i2c_master/i2c_master.v:302-302(process)
  ```verilog
                step <= 2'd3;
  ```
- check(sequence):anchor(state == ST_STOP 且 !(step == 2'd0) 且 !(step == 2'd1) 且 step == 2'd2 且 cnt_end) +1拍 → step=2'd3 when -
- 指紋:`corpus/e2e/59_i2c_master/i2c_master.v|timing|i2c_master|step||2'd3`


### carryadd(yosys-simple,ISC;R1-3 放寬回歸錨)

# 規則書:corpus-external/yosys-simple/carryadd.v

- 模組數:1;規則總數:4

## 模組 carryadd(4 條)

### carryadd.R001 [arithmetic/derived]

- 陳述:C ≡ IN1 & IN2(逐拍組合)
- 證據:corpus-external/yosys-simple/carryadd.v:14-14(generate)
  ```verilog
  			assign C = IN1 & IN2, Y = IN1 ^ IN2;
  ```
- check(relation):C ≡ IN1 & IN2 when -
- 指紋:`corpus-external/yosys-simple/carryadd.v|arithmetic|carryadd|C||IN1 & IN2`

### carryadd.R002 [arithmetic/derived]

- 陳述:Y ≡ IN1 ^ IN2(逐拍組合)
- 證據:corpus-external/yosys-simple/carryadd.v:14-14(generate)
  ```verilog
  			assign C = IN1 & IN2, Y = IN1 ^ IN2;
  ```
- check(relation):Y ≡ IN1 ^ IN2 when -
- 指紋:`corpus-external/yosys-simple/carryadd.v|arithmetic|carryadd|Y||IN1 ^ IN2`

### carryadd.R003 [arithmetic/derived]

- 陳述:C ≡ IN1 & IN2 | (IN1 | IN2) & STAGE[i - 'sd1].C(逐拍組合)
- 證據:corpus-external/yosys-simple/carryadd.v:16-16(generate)
  ```verilog
  			assign C = (IN1 & IN2) | ((IN1 | IN2) & STAGE[i-1].C),
  ```
- check(relation):C ≡ IN1 & IN2 | (IN1 | IN2) & STAGE[i - 'sd1].C when -
- 指紋:`corpus-external/yosys-simple/carryadd.v|arithmetic|carryadd|C||IN1 & IN2 | (IN1 | IN2) & STAGE[i - 'sd1].C`

### carryadd.R004 [arithmetic/derived]

- 陳述:Y ≡ IN1 ^ IN2 ^ STAGE[i - 'sd1].C(逐拍組合)
- 證據:corpus-external/yosys-simple/carryadd.v:17-17(generate)
  ```verilog
  			       Y = IN1 ^ IN2 ^ STAGE[i-1].C;
  ```
- check(relation):Y ≡ IN1 ^ IN2 ^ STAGE[i - 'sd1].C when -
- 指紋:`corpus-external/yosys-simple/carryadd.v|arithmetic|carryadd|Y||IN1 ^ IN2 ^ STAGE[i - 'sd1].C`


### acc_pipe(xls,Apache-2.0)

# 規則書:corpus/e2e/58_xls_acc_pipe/acc_pipe.v

- 模組數:1;規則總數:7

## 模組 __acc_pipe__acc_pipe(7 條)

### __acc_pipe__acc_pipe.R001 [timing/derived]

- 陳述:clk 上升沿 成立後的下一拍,x__input_flop 變為 x
- 證據:corpus/e2e/58_xls_acc_pipe/acc_pipe.v:14-14(process)
  ```verilog
      x__input_flop <= x;
  ```
- check(sequence):anchor(clk 上升沿) +1拍 → x__input_flop=x when -
- 指紋:`corpus/e2e/58_xls_acc_pipe/acc_pipe.v|timing|__acc_pipe__acc_pipe|x__input_flop||x`

### __acc_pipe__acc_pipe.R002 [timing/derived]

- 陳述:clk 上升沿 成立後的下一拍,y__input_flop 變為 y
- 證據:corpus/e2e/58_xls_acc_pipe/acc_pipe.v:15-15(process)
  ```verilog
      y__input_flop <= y;
  ```
- check(sequence):anchor(clk 上升沿) +1拍 → y__input_flop=y when -
- 指紋:`corpus/e2e/58_xls_acc_pipe/acc_pipe.v|timing|__acc_pipe__acc_pipe|y__input_flop||y`

### __acc_pipe__acc_pipe.R003 [arithmetic/derived]

- 陳述:p1_add_96_comb ≡ {1'h0, x__input_flop} + {1'h0, y__input_flop}(逐拍組合)
- 證據:corpus/e2e/58_xls_acc_pipe/acc_pipe.v:21-21(assign)
  ```verilog
    assign p1_add_96_comb = {1'h0, x__input_flop} + {1'h0, y__input_flop};
  ```
- check(relation):p1_add_96_comb ≡ {1'h0, x__input_flop} + {1'h0, y__input_flop} when -
- 指紋:`corpus/e2e/58_xls_acc_pipe/acc_pipe.v|arithmetic|__acc_pipe__acc_pipe|p1_add_96_comb||{1'h0, x__input_flop} + {1'h0, y__input_flop}`

### __acc_pipe__acc_pipe.R004 [timing/derived]

- 陳述:clk 上升沿 成立後的下一拍,p0_add_20 變為 p1_add_96_comb
- 證據:corpus/e2e/58_xls_acc_pipe/acc_pipe.v:28-28(process)
  ```verilog
      p0_add_20 <= p1_add_96_comb;
  ```
- check(sequence):anchor(clk 上升沿) +1拍 → p0_add_20=p1_add_96_comb when -
- 指紋:`corpus/e2e/58_xls_acc_pipe/acc_pipe.v|timing|__acc_pipe__acc_pipe|p0_add_20||p1_add_96_comb`

### __acc_pipe__acc_pipe.R005 [timing/derived]

- 陳述:clk 上升沿 成立後的下一拍,p0_bit_slice_48 變為 p1_bit_slice_103_comb
- 證據:corpus/e2e/58_xls_acc_pipe/acc_pipe.v:29-29(process)
  ```verilog
      p0_bit_slice_48 <= p1_bit_slice_103_comb;
  ```
- check(sequence):anchor(clk 上升沿) +1拍 → p0_bit_slice_48=p1_bit_slice_103_comb when -
- 指紋:`corpus/e2e/58_xls_acc_pipe/acc_pipe.v|timing|__acc_pipe__acc_pipe|p0_bit_slice_48||p1_bit_slice_103_comb`

### __acc_pipe__acc_pipe.R006 [arithmetic/derived]

- 陳述:p2_add_98_comb ≡ {2'h0, p0_bit_slice_48} + {1'h0, p0_add_20}(逐拍組合)
- 證據:corpus/e2e/58_xls_acc_pipe/acc_pipe.v:35-35(assign)
  ```verilog
    assign p2_add_98_comb = {2'h0, p0_bit_slice_48} + {1'h0, p0_add_20};
  ```
- check(relation):p2_add_98_comb ≡ {2'h0, p0_bit_slice_48} + {1'h0, p0_add_20} when -
- 指紋:`corpus/e2e/58_xls_acc_pipe/acc_pipe.v|arithmetic|__acc_pipe__acc_pipe|p2_add_98_comb||{2'h0, p0_bit_slice_48} + {1'h0, p0_add_20}`

### __acc_pipe__acc_pipe.R007 [timing/derived]

- 陳述:clk 上升沿 成立後的下一拍,out__output_flop 變為 p2_scaled_comb
- 證據:corpus/e2e/58_xls_acc_pipe/acc_pipe.v:41-41(process)
  ```verilog
      out__output_flop <= p2_scaled_comb;
  ```
- check(sequence):anchor(clk 上升沿) +1拍 → out__output_flop=p2_scaled_comb when -
- 指紋:`corpus/e2e/58_xls_acc_pipe/acc_pipe.v|timing|__acc_pipe__acc_pipe|out__output_flop||p2_scaled_comb`


## 破口報告樣本

### corpus/e2e/58_xls_acc_pipe/acc_pipe.v

# 破口報告:corpus/e2e/58_xls_acc_pipe/acc_pipe.v

- 破口數:3

### G4-__acc_pipe__acc_pipe-001 [missing_reset certain] 邊沿行程完全無復位臂,2 個寄存器上電值未定義

- 證據:corpus/e2e/58_xls_acc_pipe/acc_pipe.v:13-16(process)
- 建議測試:復位後直接讀 x__input_flop,釘住其上電值

### G4-__acc_pipe__acc_pipe-002 [missing_reset certain] 邊沿行程完全無復位臂,2 個寄存器上電值未定義

- 證據:corpus/e2e/58_xls_acc_pipe/acc_pipe.v:27-30(process)
- 建議測試:復位後直接讀 p0_add_20,釘住其上電值

### G4-__acc_pipe__acc_pipe-003 [missing_reset certain] 邊沿行程完全無復位臂,1 個寄存器上電值未定義

- 證據:corpus/e2e/58_xls_acc_pipe/acc_pipe.v:40-42(process)
- 建議測試:復位後直接讀 out__output_flop,釘住其上電值


### corpus/e2e/57_xls_add3/add3.v

# 破口報告:corpus/e2e/57_xls_add3/add3.v

- 破口數:2

### G4-__add3__add3-001 [missing_reset certain] 邊沿行程完全無復位臂,3 個寄存器上電值未定義

- 證據:corpus/e2e/57_xls_add3/add3.v:15-19(process)
- 建議測試:復位後直接讀 x__input_flop,釘住其上電值

### G4-__add3__add3-002 [missing_reset certain] 邊沿行程完全無復位臂,1 個寄存器上電值未定義

- 證據:corpus/e2e/57_xls_add3/add3.v:29-31(process)
- 建議測試:復位後直接讀 out__output_flop,釘住其上電值


### corpus-external/yosys-simple/subbytes.v

# 破口報告:corpus-external/yosys-simple/subbytes.v

- 破口數:1

### G1-subbytes_00-001 [latch certain] 組合行程內 data_reg_128 在條件「state」的一側有賦值但對立路徑無賦值(可能鎖存)

- 證據:corpus-external/yosys-simple/subbytes.v:67-79(process)
- 建議測試:枚舉「state」反相輸入,斷言 data_reg_128 保持值(鎖存)或改為組合(設計缺陷)


## 評審員區(S7 定性評審;人工區,機械內聯自 review-notes.md)

# 評審批注(S7 定性評審;gen-report.mjs 機械內聯進 validation-report.md)

> 方法:R5 探針題按「**先只讀規則書作答,後粗讀 RTL 對照**」順序執行,答案
> 引用 rule id;R2 散文感抽查跨檔抽樣。評審 provenance=本實作會話內評審
> (非獨立第三人;獨立複審可重跑兩閘後在本檔追加,gen-report 重新內聯)。

## R5 探針問題×3/代表檔

### acc_pipe(corpus/e2e/58,Apache-2.0)

- Q1:out 在第幾拍後反映 x 與 y 的組合結果?流程是什麼?
  A:三級流水,3 拍後有效:x/y 先進輸入級(R001/R002)→ p1_add_96_comb 組合
  相加(R003)→ 下一拍裝載 p0_add_20 與 p0_bit_slice_48(R004/R005)→
  p2_add_98_comb 組合相加(R006)→ 下一拍裝載 out(R007)。
  對照粗讀:與 stage0/1/2 註釋結構逐段一致。✓
- Q2:p0_bit_slice_48 裝載的是什麼?
  A:依 R005,是 p1_bit_slice_103_comb 的下一拍值;結合 R003(9-bit 和)可推
  切片語義,但**切片關係本身沒有 relation 規則**(rhs=純 PartSelect,無
  Binary,三鎖下不產)——覆蓋注記:讀 R005+R003 可重建,規則書未直答。
  對照粗讀:`p1_bit_slice_103_comb = p1_add_96_comb[8:1]` 一致。✓(帶覆蓋注記)
- Q3:輸出縮放({5'h00, add, p0_add_20[0]})在哪條規則?
  A:**沒有**(rhs=純 Concat,同上取捨面)。out__output_flop 的裝載由 R007 釘住
  來源訊號名,縮放恆等式需回 RTL。對照粗讀一致;覆蓋注記=誠實。✓

### carryadd(corpus-external/yosys-simple,ISC)

- Q1:最低位的進位與和如何構成?
  A:R001(C=IN1&IN2)、R002(Y=IN1^IN2)。對照粗讀:i==0 臘一致。✓
- Q2:非最低位的進位呢?
  A:R003(C=IN1&IN2 | (IN1|IN2)&STAGE[i-1].C)、R004(Y=IN1^IN2^STAGE[i-1].C)
  ——行波進位鏈。對照粗讀:else 臘一致。✓
- Q3:y[i] 從何而來?
  A:規則書**沒有**直接規則(assign y[i]=Y 純線連,不含 Binary);由 R004 的
  Y 與模組結構可推。對照粗讀:`assign y[i]=Y` 一致;規則書答不全需回 RTL
  一眼——「規則書不是文件轉寫」的取捨面,非錯誤。✓

### i2c_master(corpus/e2e/59,repo MIT)

- Q1:一次位元組傳送的 FSM 環流?
  A:R022(START→SHIFT)、R023(SHIFT→ACK,當 bitcnt==7 且非半位低位)→
  R024(ACK→HOLD)→ R025-28(HOLD 按 pend_wr/pend_rd/pend_stop/pend_start
  分派回 SHIFT/STOP/START)→ R029(STOP→IDLE)。
  對照粗讀:與 case(state) 主環一致。✓
- Q2:復位裝載哪些寄存器、成什麼值?
  A:R001-R020(20 條 guard,when=!rst_n:state→ST_IDLE、cnt→16'd0、
  scl_o→1'b1、sda_o→1'b1 等,每寄存器一條)。對照粗讀:復位臂 20 個賦值
  一一對應。✓
- Q3:計數面?
  A:R030+ 的 counter 類(cnt/step 環繞與遞增)+ timing 類的半位計數。
  對照粗讀:cnt 16-bit、step 2-bit 計數結構一致。✓

## R2 散文感抽查

- 抽樣:跨 3 代表檔各抽 ~10 條規則讀陳述句。
- 結論:陳述全部為模板機械生成(「當…且…時,裝載/變為/下一拍…」句式),
  標識符=源碼原文名,無自由散文、無虛構語義。
- 噪音(接受):i2c_master 的深層 else-if 條件鏈渲染為
  「!(step == 2'd0) 且 !(step == 2'd1) 且 cnt_end」形(R022/R029 等)——
  忠實但冗長,屬機械生成契約內;不影響逐條可檢驗性。

## 覆蓋面誠實注記(評審發現,閘外資訊)

1. 純 Concat/PartSelect rhs(切片、縮放、線連)不產 relation 規則——
   R1-3 放寬只涵蓋 Binary 類;R5-Q2/Q3 兩例即此取捨面。
2. i2c_master 的 fsm when 條件鏈含上層否定鏈(機械忠實),人讀需展开。
3. **rhs 為變量/常量拷貝的賦值不產 relation 規則**(收平輪 R3-② 補記):
   relation 提取源=rhs 含 Binary;`next_state = state`、`assign y = Y`
   純線連/拷貝形不產——讀者需回 RTL 對讀。同輪已落的是另一面:分支域
   relation 攜 `when` 適用域(subbytes R007 轉寫為「當 !(state) 時,
   next_state ≡ state + 'sd1」,不再冒充全域恆等);本條缺口若未來要收,
   屬提取器迭代輪,按正典 §8.3「閾值不可降、加強提取器」路徑走。
4. 以上各點不改閘判定(V1..V6 全過);閘外資訊性質不變。

## 總閘命令清單

```
cargo test --workspace
bash scripts/regress.sh
bash plugin/rtl2spec/run-mcp-e2e.sh
bash plugin/rtl2spec/run-rtl-validation.sh
```
