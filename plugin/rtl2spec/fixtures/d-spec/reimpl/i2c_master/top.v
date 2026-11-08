// i2c_master — 盲寫凍結產物(D-S5;REIMPL-PROTOCOL §5)。
// 來源=i2c_master.spec.md 全文(spec-format:1;B001..B094+散文行為概述)。
// 誠實推斷點(逐條記錄於協議 §5):①done=單拍脈衝,清零面按 spec 散文
// 「done 在每字節 ACK 結束拍置一拍」(規則面僅 B070 置 1/B018 復位清 0);
// ②half 1→0 的翻回由狀態機推進必然性推得(規則面僅 B058/B069 覆 0→1);
// ③命令鎖存置 case 之後(散文「tip=1 期間寫入者排隊」=鎖存覆寫消費為隊列語義)。
module i2c_master (
    input  wire        clk,
    input  wire        rst_n,
    input  wire        we,
    input  wire        scl_i,   // bus sense (unused v1: no stretching)
    input  wire        sda_i,
    output reg         ack_rx,  // ack bit sampled for the last TX byte (0 = ACK)
    output reg         done,    // 1-clk strobe: a byte completed
    output reg         busy,    // a command sequence is in progress
    output reg         tip,     // a byte transfer is in progress
    output reg         scl_o,
    output reg         sda_o,
    output reg         scl_oe,
    output reg         sda_oe,
    input  wire [15:0] prescale,
    input  wire [3:0]  cmd,     // {STOP, START, RD, WR} = bit[3..0]
    input  wire [7:0]  din,
    output reg  [7:0]  dout
);

  localparam [3:0] ST_IDLE  = 4'd0;
  localparam [3:0] ST_START = 4'd1;
  localparam [3:0] ST_SHIFT = 4'd2;
  localparam [3:0] ST_ACK   = 4'd3;
  localparam [3:0] ST_HOLD  = 4'd4;
  localparam [3:0] ST_STOP  = 4'd5;

  localparam [3:0] CMD_START = 4'b0001;
  localparam [3:0] CMD_STOP  = 4'b0010;
  localparam [3:0] CMD_WR    = 4'b0100;
  localparam [3:0] CMD_RD    = 4'b1000;

  reg [3:0]  state;       // FSM 現態(ST_* 編碼)
  reg [15:0] cnt;         // 半周期預分頻計數器
  reg        half;        // 半周期相位(0=第一半,1=第二半)
  reg [3:0]  bitcnt;      // 位計數(0..7)
  reg [1:0]  step;        // START/STOP 內子步(0..3)
  reg [7:0]  tx;          // 移位暫存
  reg        rd_mode;     // 當前傳輸方向(1=讀)
  reg        pend_start;
  reg        pend_stop;
  reg        pend_wr;
  reg        pend_rd;

  // B021:半周期邊界組合量(derived)。
  wire cnt_end = (cnt >= prescale - 16'd1);

  always @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
      // B001..B020:異步復位(低有效)。
      state     <= ST_IDLE;
      cnt       <= 16'd0;   // B002
      half      <= 1'b0;    // B003
      bitcnt    <= 4'd0;    // B004
      step      <= 2'd0;    // B005
      tx        <= 8'd0;    // B006
      rd_mode   <= 1'b0;    // B007
      pend_start <= 1'b0;   // B008
      pend_stop  <= 1'b0;   // B009
      pend_wr    <= 1'b0;   // B010
      pend_rd    <= 1'b0;   // B011
      scl_o  <= 1'b1;       // B012
      sda_o  <= 1'b1;       // B013
      scl_oe <= 1'b0;       // B014
      sda_oe <= 1'b0;       // B015
      busy   <= 1'b0;       // B016
      tip    <= 1'b0;       // B017
      done   <= 1'b0;       // B018
      dout   <= 8'd0;       // B019
      ack_rx <= 1'b1;       // B020
    end else begin
      // done=單拍脈衝(散文契約);B070 置 1 覆寫於 ACK 結束拍。
      done <= 1'b0;

      // B022/B023:半周期計數(!cnt_end 自增,cnt_end 歸零)。
      if (!cnt_end) cnt <= cnt + 16'd1;
      else          cnt <= 16'd0;

      case (state)
        // B029..B038:IDLE 態(總線釋放;pend_start 消費起步)。
        ST_IDLE: begin
          scl_oe <= 1'b0;
          sda_oe <= 1'b0;
          scl_o  <= 1'b1;
          sda_o  <= 1'b1;
          busy   <= 1'b0;
          tip    <= 1'b0;
          if (pend_start) begin
            pend_start <= 1'b0;
            busy       <= 1'b1;
            rd_mode    <= 1'b0;
            step       <= 2'd0;
            cnt        <= 16'd0;
            half       <= 1'b0;
            bitcnt     <= 4'd0;
            state      <= ST_START;
          end
        end

        // B039..B051:START 態(三子步 start 條件,隨後拉低 SCL 進 SHIFT)。
        ST_START: begin
          if (step == 2'd0) begin
            scl_oe <= 1'b0;
            sda_oe <= 1'b0;
            sda_o  <= 1'b1;
          end else if (step == 2'd1) begin
            sda_o  <= 1'b0;
            sda_oe <= 1'b1;
          end else begin
            scl_oe <= 1'b1;
            scl_o  <= 1'b0;
            sda_oe <= 1'b1;
            sda_o  <= 1'b0;
          end
          if (cnt_end) begin
            if (step == 2'd2) begin
              step   <= 2'd0;
              half   <= 1'b0;
              bitcnt <= 4'd0;
              tip    <= 1'b1;
              state  <= ST_SHIFT;
            end else begin
              step <= step + 2'd1;
            end
          end
        end

        // B052..B062:SHIFT 態(半周期 0=SCL 低移出數據;1=釋放 SCL,讀模式採樣)。
        ST_SHIFT: begin
          if (!half) begin
            scl_oe <= 1'b1;
            scl_o  <= 1'b0;
            if (!rd_mode) begin
              sda_oe <= ~tx[7 - bitcnt[2:0]];  // B054:MSB 先行,開漏(位 0 驅動)
              sda_o  <= 1'b0;
            end else begin
              sda_oe <= 1'b0;                  // B056:讀模式釋放 SDA 交從機
              sda_o  <= 1'b1;
            end
            if (cnt_end) begin
              half   <= 1'b1;                  // B058
              scl_oe <= 1'b0;                  // B059
            end
          end else begin
            if (cnt_end) begin
              if (rd_mode) tx <= {tx[6:0], sda_i};  // B060
              if (bitcnt == 4'd7) begin
                bitcnt <= 4'd0;                     // B061
                state  <= ST_ACK;                   // B062
              end else begin
                bitcnt <= bitcnt + 4'd1;            // B061
              end
              half <= 1'b0;
            end
          end
        end

        // B063..B076:ACK 態(寫=釋放聽從機;讀=主控驅動 ACK 低)。
        ST_ACK: begin
          if (!half) begin
            scl_oe <= 1'b1;
            scl_o  <= 1'b0;
            if (rd_mode) begin
              sda_oe <= 1'b1;
              sda_o  <= 1'b0;
            end else begin
              sda_oe <= 1'b0;
              sda_o  <= 1'b1;
            end
            if (cnt_end) begin
              half   <= 1'b1;                        // B069
              scl_oe <= 1'b0;
            end
          end else begin
            if (cnt_end) begin
              done <= 1'b1;                          // B070
              tip  <= 1'b0;                          // B071
              dout <= tx;                            // B072
              if (!rd_mode) ack_rx <= sda_i;         // B073
              scl_oe <= 1'b1;                        // B074
              scl_o  <= 1'b0;                        // B075
              state  <= ST_HOLD;                     // B076
              half   <= 1'b0;
            end
          end
        end

        // B077..B084:HOLD 態(字節間保持 SCL 低;wr>rd>stop>start 接力)。
        ST_HOLD: begin
          scl_oe <= 1'b1;
          scl_o  <= 1'b0;
          sda_oe <= 1'b0;
          sda_o  <= 1'b1;
          if (pend_wr) begin
            pend_wr  <= 1'b0;
            rd_mode  <= 1'b0;
            bitcnt   <= 4'd0;
            half     <= 1'b0;
            cnt      <= 16'd0;
            tip      <= 1'b1;
            state    <= ST_SHIFT;   // tx 保持命令鎖存時裝載的字節
          end else if (pend_rd) begin
            pend_rd  <= 1'b0;
            rd_mode  <= 1'b1;
            tx       <= 8'd0;
            bitcnt   <= 4'd0;
            half     <= 1'b0;
            cnt      <= 16'd0;
            tip      <= 1'b1;
            state    <= ST_SHIFT;
          end else if (pend_stop) begin
            pend_stop <= 1'b0;
            step      <= 2'd0;
            cnt       <= 16'd0;
            state     <= ST_STOP;
          end else if (pend_start) begin
            pend_start <= 1'b0;
            step       <= 2'd0;
            cnt        <= 16'd0;
            sda_oe     <= 1'b1;
            sda_o      <= 1'b1;
            state      <= ST_START;  // repeated start
          end
        end

        // B085..B094:STOP 態(四子步 stop 條件後回 IDLE)。
        ST_STOP: begin
          if (step == 2'd0) begin
            sda_oe <= 1'b1;
            sda_o  <= 1'b0;
            scl_oe <= 1'b1;
            scl_o  <= 1'b0;
          end else if (step == 2'd1) begin
            scl_oe <= 1'b0;
          end else if (step == 2'd2) begin
            sda_oe <= 1'b0;
            sda_o  <= 1'b1;
          end
          if (step == 2'd3) begin
            busy <= 1'b0;   // B094
            tip  <= 1'b0;
          end
          if (cnt_end) begin
            if (step == 2'd3) state <= ST_IDLE;  // B093
            else step <= step + 2'd1;            // B092
          end
        end

        default: state <= ST_IDLE;
      endcase

      // B024..B028:命令鎖存(we 拍寫入;tip=1 期間亦入隊=條件無態限定;
      // 置 case 之後=鎖存覆寫消費為隊列語義,推斷點③)。
      if (we) begin
        if (cmd == CMD_WR) begin
          pend_wr <= 1'b1;
          tx      <= din;
        end
        if (cmd == CMD_RD)   pend_rd    <= 1'b1;
        if (cmd == CMD_START) pend_start <= 1'b1;
        if (cmd == CMD_STOP)  pend_stop  <= 1'b1;
      end
    end
  end

endmodule
