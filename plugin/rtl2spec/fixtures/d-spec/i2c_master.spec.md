# spec: i2c_master

- spec-format: 1
- module: i2c_master
- param: ST_IDLE = 4'd0
- param: ST_START = 4'd1
- param: ST_SHIFT = 4'd2
- param: ST_ACK = 4'd3
- param: ST_HOLD = 4'd4
- param: ST_STOP = 4'd5
- param: CMD_START = 4'b0001
- param: CMD_STOP = 4'b0010
- param: CMD_WR = 4'b0100
- param: CMD_RD = 4'b1000
- port: clk input wire scalar
- port: rst_n input wire scalar
- port: we input wire scalar
- port: scl_i input wire scalar
- port: sda_i input wire scalar
- port: ack_rx output wire scalar
- port: done output wire scalar
- port: busy output wire scalar
- port: tip output wire scalar
- port: scl_o output wire scalar
- port: sda_o output wire scalar
- port: scl_oe output wire scalar
- port: sda_oe output wire scalar
- port: prescale input wire [15:0]
- port: cmd input wire [3:0]
- port: din input wire [7:0]
- port: dout output wire [7:0]
- state: state [3:0] FSM 現態(ST_* 編碼)
- state: cnt [15:0] 半周期預分頻計數器
- state: half scalar 半周期相位(0=第一半,1=第二半)
- state: bitcnt [3:0] 位計數(0..7)
- state: step [1:0] START/STOP 內子步(0..3)
- state: tx [7:0] 移位暫存(寫=待發字節;讀=移位接收)
- state: rd_mode scalar 當前傳輸方向(1=讀)
- state: pend_start scalar 排隊中的 START 命令
- state: pend_stop scalar 排隊中的 STOP 命令
- state: pend_wr scalar 排隊中的寫命令
- state: pend_rd scalar 排隊中的讀命令
- derived: cnt_end scalar 半周期邊界組合量(cnt ≥ prescale-1 時成立)

## behavior

單主控 I2C 位元組引擎,開漏輸出(oe=1 驅動、oe=0 釋放上拉)。半周期長度由 prescale 給定:
每個半周期經歷 prescale 個時鐘沿。命令經 we/cmd 寫入,在 tip=1(傳輸進行中)期間寫入者排隊,
當前字節完成後於 HOLD 態依 wr > rd > stop > start 優先序接力;done 在每字節 ACK 結束拍置一拍。

### 復位(異步,低有效)

- B001 [reset] 當 !rst_n(異步)時,state 裝載 ST_IDLE
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=state←ST_IDLE
- B002 [reset] 同沿 cnt 裝載 16'd0
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=cnt←16'd0
- B003 [reset] 同沿 half 裝載 1'b0
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=half←1'b0
- B004 [reset] 同沿 bitcnt 裝載 4'd0
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=bitcnt←4'd0
- B005 [reset] 同沿 step 裝載 2'd0
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=step←2'd0
- B006 [reset] 同沿 tx 裝載 8'd0
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=tx←8'd0
- B007 [reset] 同沿 rd_mode 裝載 1'b0
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=rd_mode←1'b0
- B008 [reset] 同沿四個排隊位 pend_start/pend_stop/pend_wr/pend_rd 全部清零
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=pend_start←1'b0
- B009 [reset] 同沿 pend_stop 清零
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=pend_stop←1'b0
- B010 [reset] 同沿 pend_wr 清零
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=pend_wr←1'b0
- B011 [reset] 同沿 pend_rd 清零
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=pend_rd←1'b0
- B012 [reset] 同沿總線釋放為空閒態:scl_o=1、sda_o=1、scl_oe=0、sda_oe=0
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=scl_o←1'b1
- B013 [reset] 同沿 sda_o=1
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=sda_o←1'b1
- B014 [reset] 同沿 scl_oe=0
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=scl_oe←1'b0
- B015 [reset] 同沿 sda_oe=0
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=sda_oe←1'b0
- B016 [reset] 同沿狀態輸出清零:busy=0、tip=0、done=0、dout=0
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=busy←1'b0
- B017 [reset] 同沿 tip=0
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=tip←1'b0
- B018 [reset] 同沿 done=0
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=done←1'b0
- B019 [reset] 同沿 dout 裝載 8'd0
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=dout←8'd0
- B020 [reset] 同沿 ack_rx 裝載 1'b1(無從機應答時的高電平默認)
  check(guard): trigger=clk posedge; when=!rst_n; async=true; effects=ack_rx←1'b1

### 預分頻計數

- B021 [counter] cnt_end 為組合信號:cnt ≥ prescale-1 時成立
  check(relation): cnt_end ≡ (cnt >= prescale - 16'd1) when -
- B022 [counter] 各態內 !cnt_end 時下一拍 cnt 自增一
  check(sequence): +1拍 → cnt=cnt + 16'd1 when !cnt_end
- B023 [counter] cnt_end 成立的沿後下一拍 cnt 歸零(半周期邊界)
  check(sequence): +1拍 → cnt=16'd0 when cnt_end

### 命令鎖存(we 拍寫入,tip=1 時亦入隊)

- B024 [latch] we 且 cmd=CMD_WR 時下一拍 pend_wr=1 且 tx 裝載 din
  check(sequence): +1拍 → pend_wr=1'b1 when we 且 cmd == CMD_WR
- B025 [latch] 同條件 tx 裝載 din
  check(sequence): +1拍 → tx=din when we 且 cmd == CMD_WR
- B026 [latch] we 且 cmd=CMD_RD 時下一拍 pend_rd=1
  check(sequence): +1拍 → pend_rd=1'b1 when we 且 cmd == CMD_RD
- B027 [latch] we 且 cmd=CMD_START 時下一拍 pend_start=1
  check(sequence): +1拍 → pend_start=1'b1 when we 且 cmd == CMD_START
- B028 [latch] we 且 cmd=CMD_STOP 時下一拍 pend_stop=1
  check(sequence): +1拍 → pend_stop=1'b1 when we 且 cmd == CMD_STOP

### IDLE 態

- B029 [drive] state=ST_IDLE 時下一拍總線釋放:scl_oe=0、sda_oe=0、scl_o=1、sda_o=1
  check(sequence): +1拍 → scl_oe=1'b0 when state == ST_IDLE
- B030 [drive] 同態 sda_oe=0
  check(sequence): +1拍 → sda_oe=1'b0 when state == ST_IDLE
- B031 [drive] 同態 scl_o=1
  check(sequence): +1拍 → scl_o=1'b1 when state == ST_IDLE
- B032 [drive] 同態 sda_o=1
  check(sequence): +1拍 → sda_o=1'b1 when state == ST_IDLE
- B033 [status] 同態 busy=0、tip=0
  check(sequence): +1拍 → busy=1'b0 when state == ST_IDLE
- B034 [status] 同態 tip=0
  check(sequence): +1拍 → tip=1'b0 when state == ST_IDLE
- B035 [fsm] state=ST_IDLE 且 pend_start 時:下一拍消費隊列(pend_start=0)、busy=1、
  rd_mode=0、step=0、cnt=0、half=0、bitcnt=0 並轉 ST_START
  check(sequence): +1拍 → pend_start=1'b0 when state == ST_IDLE 且 pend_start
- B036 [fsm] 同條件轉 ST_START
  check(transition): state: ST_IDLE → ST_START when pend_start
- B037 [status] 同條件 busy 置 1
  check(sequence): +1拍 → busy=1'b1 when state == ST_IDLE 且 pend_start
- B038 [status] 同條件 rd_mode 清零
  check(sequence): +1拍 → rd_mode=1'b0 when state == ST_IDLE 且 pend_start

### START 態(三子步:start 條件 = SCL 高時 SDA 由高落低,隨後拉低 SCL)

- B039 [drive] state=ST_START 且 step=0 時下一拍兩線釋放:scl_oe=0、sda_oe=0、sda_o=1
  check(sequence): +1拍 → scl_oe=1'b0 when state == ST_START 且 step == 2'd0
- B040 [drive] 同步 sda_oe=0
  check(sequence): +1拍 → sda_oe=1'b0 when state == ST_START 且 step == 2'd0
- B041 [drive] 同步 sda_o=1
  check(sequence): +1拍 → sda_o=1'b1 when state == ST_START 且 step == 2'd0
- B042 [drive] state=ST_START 且 step=1 時下一拍拉低 SDA(SCL 仍高):sda_o=0、sda_oe=1
  check(sequence): +1拍 → sda_o=1'b0 when state == ST_START 且 step == 2'd1
- B043 [drive] 同步 sda_oe=1
  check(sequence): +1拍 → sda_oe=1'b1 when state == ST_START 且 step == 2'd1
- B044 [drive] state=ST_START 且 step=2 時下一拍拉低 SCL:scl_oe=1、scl_o=0、sda_oe=1、sda_o=0
  check(sequence): +1拍 → scl_oe=1'b1 when state == ST_START 且 step == 2'd2
- B045 [drive] 同步 scl_o=0
  check(sequence): +1拍 → scl_o=1'b0 when state == ST_START 且 step == 2'd2
- B046 [drive] 同步 sda_oe=1
  check(sequence): +1拍 → sda_oe=1'b1 when state == ST_START 且 step == 2'd2
- B047 [drive] 同步 sda_o=0
  check(sequence): +1拍 → sda_o=1'b0 when state == ST_START 且 step == 2'd2
- B048 [fsm] step=0 cnt_end 時下一拍 step=1;step=1 cnt_end 時下一拍 step=2;
  step=2 cnt_end 時下一拍 step=0、half=0、bitcnt=0、tip=1 並轉 ST_SHIFT
  check(sequence): +1拍 → step=2'd1 when state == ST_START 且 step == 2'd0 且 cnt_end
- B049 [fsm] step=2 cnt_end 沿轉 ST_SHIFT
  check(transition): state: ST_START → ST_SHIFT when step == 2'd2 且 cnt_end
- B050 [status] step=2 cnt_end 沿 tip 置 1
  check(sequence): +1拍 → tip=1'b1 when state == ST_START 且 step == 2'd2 且 cnt_end
- B051 [status] step=2 cnt_end 沿 half、bitcnt 清零
  check(sequence): +1拍 → half=1'b0 when state == ST_START 且 step == 2'd2 且 cnt_end

### SHIFT 態(半周期 0=SCL 低移出數據;半周期 1=釋放 SCL 高,讀模式沿採樣)

- B052 [drive] state=ST_SHIFT 且 half=0 時下一拍 SCL 驅動低:scl_oe=1、scl_o=0
  check(sequence): +1拍 → scl_oe=1'b1 when state == ST_SHIFT 且 half == 1'b0
- B053 [drive] 同步 scl_o=0
  check(sequence): +1拍 → scl_o=1'b0 when state == ST_SHIFT 且 half == 1'b0
- B054 [drive] 寫模式(half=0):sda_oe=1(驅動)、sda_o=0;驅動位=tx 的
  bitcnt[2:0] 位反相開漏(MSB 先行,bit0 驅動 ~tx[7])
  check(sequence): +1拍 → sda_oe=~tx[7 - bitcnt[2:0]] when state == ST_SHIFT 且 half == 1'b0 且 !rd_mode
- B055 [drive] 寫模式同拍 sda_o=0
  check(sequence): +1拍 → sda_o=1'b0 when state == ST_SHIFT 且 half == 1'b0 且 !rd_mode
- B056 [drive] 讀模式(half=0):SDA 釋放交從機(sda_oe=0、sda_o=1)
  check(sequence): +1拍 → sda_oe=1'b0 when state == ST_SHIFT 且 half == 1'b0 且 rd_mode
- B057 [drive] 讀模式同拍 sda_o=1
  check(sequence): +1拍 → sda_o=1'b1 when state == ST_SHIFT 且 half == 1'b0 且 rd_mode
- B058 [timing] half=0 cnt_end 沿:下一拍 half=1、SCL 釋放高(scl_oe=0)、cnt=0
  check(sequence): +1拍 → half=1'b1 when state == ST_SHIFT 且 half == 1'b0 且 cnt_end
- B059 [drive] 同沿 scl_oe=0
  check(sequence): +1拍 → scl_oe=1'b0 when state == ST_SHIFT 且 half == 1'b0 且 cnt_end
- B060 [shift] half=1 cnt_end 沿讀模式:tx 左移入 sda_i(tx = {tx[6:0], sda_i})
  check(sequence): +1拍 → tx={tx[6:0], sda_i} when state == ST_SHIFT 且 half == 1'b1 且 cnt_end 且 rd_mode
- B061 [shift] half=1 cnt_end 沿:bitcnt 未到 7 則加一,到 7 則清零並轉 ST_ACK
  check(sequence): +1拍 → bitcnt=4'd0 when state == ST_SHIFT 且 half == 1'b1 且 cnt_end 且 bitcnt == 4'd7
- B062 [fsm] 同沿轉 ST_ACK
  check(transition): state: ST_SHIFT → ST_ACK when half == 1'b1 且 cnt_end 且 bitcnt == 4'd7

### ACK 態(半周期 0=SCL 低;寫=釋放 SDA 聽從機;讀=主控驅動 ACK 低)

- B063 [drive] state=ST_ACK 且 half=0 時下一拍 SCL 驅動低:scl_oe=1、scl_o=0
  check(sequence): +1拍 → scl_oe=1'b1 when state == ST_ACK 且 half == 1'b0
- B064 [drive] 同步 scl_o=0
  check(sequence): +1拍 → scl_o=1'b0 when state == ST_ACK 且 half == 1'b0
- B065 [drive] 讀模式(half=0):主控發 ACK:sda_oe=1、sda_o=0
  check(sequence): +1拍 → sda_oe=1'b1 when state == ST_ACK 且 half == 1'b0 且 rd_mode
- B066 [drive] 讀模式同拍 sda_o=0
  check(sequence): +1拍 → sda_o=1'b0 when state == ST_ACK 且 half == 1'b0 且 rd_mode
- B067 [drive] 寫模式(half=0):SDA 釋放聽 ACK:sda_oe=0、sda_o=1
  check(sequence): +1拍 → sda_oe=1'b0 when state == ST_ACK 且 half == 1'b0 且 !rd_mode
- B068 [drive] 寫模式同拍 sda_o=1
  check(sequence): +1拍 → sda_o=1'b1 when state == ST_ACK 且 half == 1'b0 且 !rd_mode
- B069 [timing] half=0 cnt_end 沿:下一拍 half=1、scl_oe=0、cnt=0
  check(sequence): +1拍 → half=1'b1 when state == ST_ACK 且 half == 1'b0 且 cnt_end
- B070 [status] half=1 cnt_end 沿:done 置 1、tip 清 0、dout 裝載 tx、cnt=0、half=0、
  SCL 驅動低(scl_oe=1、scl_o=0)並轉 ST_HOLD
  check(sequence): +1拍 → done=1'b1 when state == ST_ACK 且 half == 1'b1 且 cnt_end
- B071 [status] 同沿 tip=0
  check(sequence): +1拍 → tip=1'b0 when state == ST_ACK 且 half == 1'b1 且 cnt_end
- B072 [data] 同沿 dout=tx
  check(sequence): +1拍 → dout=tx when state == ST_ACK 且 half == 1'b1 且 cnt_end
- B073 [status] 同沿 ack_rx 裝載 sda_i(僅寫模式)
  check(sequence): +1拍 → ack_rx=sda_i when state == ST_ACK 且 half == 1'b1 且 cnt_end 且 !rd_mode
- B074 [drive] 同沿 scl_oe=1
  check(sequence): +1拍 → scl_oe=1'b1 when state == ST_ACK 且 half == 1'b1 且 cnt_end
- B075 [drive] 同沿 scl_o=0
  check(sequence): +1拍 → scl_o=1'b0 when state == ST_ACK 且 half == 1'b1 且 cnt_end
- B076 [fsm] 同沿轉 ST_HOLD
  check(transition): state: ST_ACK → ST_HOLD when half == 1'b1 且 cnt_end

### HOLD 態(字節間保持 SCL 低;依 wr > rd > stop > start 優先序接力)

- B077 [drive] state=ST_HOLD 時下一拍 SCL 保持驅動低、SDA 釋放:scl_oe=1、scl_o=0、sda_oe=0、sda_o=1
  check(sequence): +1拍 → scl_oe=1'b1 when state == ST_HOLD
- B078 [drive] 同態 sda_oe=0
  check(sequence): +1拍 → sda_oe=1'b0 when state == ST_HOLD
- B079 [fsm] HOLD 且 pend_wr:下一拍消費(pend_wr=0)、rd_mode=0、bitcnt=0、half=0、cnt=0、
  tip=1 並轉 ST_SHIFT(tx 保持命令鎖存時裝載的字節)
  check(sequence): +1拍 → pend_wr=1'b0 when state == ST_HOLD 且 pend_wr
- B080 [fsm] 同條件轉 ST_SHIFT
  check(transition): state: ST_HOLD → ST_SHIFT when pend_wr
- B081 [fsm] HOLD 且 !pend_wr 且 pend_rd:下一拍消費(pend_rd=0)、rd_mode=1、tx 清零、
  bitcnt=0、half=0、cnt=0、tip=1 並轉 ST_SHIFT
  check(sequence): +1拍 → pend_rd=1'b0 when state == ST_HOLD 且 !pend_wr 且 pend_rd
- B082 [fsm] 同條件轉 ST_SHIFT
  check(transition): state: ST_HOLD → ST_SHIFT when !pend_wr 且 pend_rd
- B083 [fsm] HOLD 且無 wr/rd 且 pend_stop:下一拍消費(pend_stop=0)、step=0、cnt=0 並轉 ST_STOP
  check(transition): state: ST_HOLD → ST_STOP when !pend_wr 且 !pend_rd 且 pend_stop
- B084 [fsm] HOLD 且無 wr/rd/stop 且 pend_start:下一拍消費(pend_start=0)、step=0、cnt=0、
  sda_oe=1、sda_o=1 並轉 ST_START(repeated start)
  check(transition): state: ST_HOLD → ST_START when !pend_wr 且 !pend_rd 且 !pend_stop 且 pend_start

### STOP 態(四子步:stop 條件 = SCL 高時 SDA 由低升高)

- B085 [drive] state=ST_STOP 且 step=0 時下一拍兩線驅動低:sda_oe=1、sda_o=0、scl_oe=1、scl_o=0
  check(sequence): +1拍 → sda_oe=1'b1 when state == ST_STOP 且 step == 2'd0
- B086 [drive] 同步 sda_o=0
  check(sequence): +1拍 → sda_o=1'b0 when state == ST_STOP 且 step == 2'd0
- B087 [drive] 同步 scl_oe=1
  check(sequence): +1拍 → scl_oe=1'b1 when state == ST_STOP 且 step == 2'd0
- B088 [drive] 同步 scl_o=0
  check(sequence): +1拍 → scl_o=1'b0 when state == ST_STOP 且 step == 2'd0
- B089 [drive] state=ST_STOP 且 step=1 時下一拍釋放 SCL 高(scl_oe=0;SDA 保持低)
  check(sequence): +1拍 → scl_oe=1'b0 when state == ST_STOP 且 step == 2'd1
- B090 [drive] state=ST_STOP 且 step=2 時下一拍釋放 SDA 高(sda_oe=0、sda_o=1;SCL 仍高)
  check(sequence): +1拍 → sda_oe=1'b0 when state == ST_STOP 且 step == 2'd2
- B091 [drive] 同步 sda_o=1
  check(sequence): +1拍 → sda_o=1'b1 when state == ST_STOP 且 step == 2'd2
- B092 [fsm] STOP 各子步 cnt_end 推進 step(0→1→2→3);step=3 cnt_end 沿轉 ST_IDLE
  check(sequence): +1拍 → step=2'd1 when state == ST_STOP 且 step == 2'd0 且 cnt_end
- B093 [fsm] step=3 cnt_end 沿轉 ST_IDLE
  check(transition): state: ST_STOP → ST_IDLE when step == 2'd3 且 cnt_end
- B094 [status] step=3 沿 busy=0、tip=0
  check(sequence): +1拍 → busy=1'b0 when state == ST_STOP 且 step == 2'd3

## scenarios

- SCENE1 reset_idle: 復位釋放後總線空閒:兩線高、oe 全 0、busy/tip/done=0、ack_rx=1
- SCENE2 start: 隊列 START 後進 START 態,tip 置位,產生 start 條件(SCL 高時 SDA 落低)
- SCENE3 wr_55: 寫 8'h55:MSB 先行逐位移出,ACK 半周期釋放 SDA 聽從機,done 一拍,tx/dout 觀測
- SCENE4 wr_aa: 寫 8'hAA 同上(位序對偶樣本)
- SCENE5 rd_released: 讀方向:SHIFT 半周期 0 釋放 SDA,半周期 1 沿移位採樣 sda_i,dout 收字節
- SCENE6 restart: HOLD 中排隊 START → repeated start(SCL 低保持後重新 start 條件)
- SCENE7 wr_0f_after_restart: restart 後再寫 8'h0f,驗證方向切回寫與隊列接力
- SCENE8 stop: 排隊 STOP → STOP 態四子步產生 stop 條件(SCL 高時 SDA 升高)後回 IDLE
- SCENE9 queue_start_during_tip: tip=1 期間寫入 START 命令 → 必須排隊(當前字節先完成再執行)
- SCENE10 mid_transfer_reset: 傳輸中途拉低 rst_n:異步復位即時生效(同拍舊值、下一態空閒),
  釋放後可重新發命令
- SCENE11 idle_after_stop: STOP 完成後 IDLE 穩態總線電平與狀態輸出
