// i2c_master — blind re-implementation from the extracted rule book
// (spec: corpus/e2e/59_i2c_master/i2c_master.v, 69 rules; source not seen).
// Verilog-2001, synthesizable, single synchronous reset process.
// Everything the rule book does not state is a documented assumption:
//   - cnt_end threshold (T_HALF parameter, default 16'd4),
//   - state encoding (3-bit binary) and cmd encoding (2-bit),
//   - port list shape/direction/order (names taken verbatim from the rules).
module i2c_master #(
    parameter [15:0] T_HALF = 16'd4  // ASSUMED: clk cycles per SCL half period
) (
    input  wire       clk,
    input  wire       rst_n,
    input  wire       we,            // host command strobe
    input  wire [1:0] cmd,           // ASSUMED 2-bit command code
    input  wire [7:0] din,           // data byte for CMD_WR
    input  wire       sda_i,         // sampled SDA level
    output reg        scl_o,
    output reg        sda_o,
    output reg        scl_oe,
    output reg        sda_oe,
    output reg        busy,
    output reg        tip,
    output reg        done,
    output reg        ack_rx,
    output reg  [7:0] dout
);

  // ASSUMED encodings (rule book gives names only; 6 states -> 3 bits)
  localparam [2:0] ST_IDLE  = 3'd0;
  localparam [2:0] ST_START = 3'd1;
  localparam [2:0] ST_SHIFT = 3'd2;
  localparam [2:0] ST_ACK   = 3'd3;
  localparam [2:0] ST_HOLD  = 3'd4;
  localparam [2:0] ST_STOP  = 3'd5;

  localparam [1:0] CMD_WR    = 2'd0;
  localparam [1:0] CMD_RD    = 2'd1;
  localparam [1:0] CMD_START = 2'd2;
  localparam [1:0] CMD_STOP  = 2'd3;

  reg [2:0]  state;  // FIXED(实验最小干预):原盲写产物缺本行声明,致 iverilog elaboration 失败
  reg [15:0] cnt;
  reg        half;
  reg [3:0]  bitcnt;
  reg [1:0]  step;
  reg [7:0]  tx;
  reg        rd_mode;
  reg        pend_start;
  reg        pend_stop;
  reg        pend_wr;
  reg        pend_rd;

  // ASSUMED: terminal count of the half-bit timer. cnt_end is used by nearly
  // every timing rule but never defined in the rule book (declarations spawn
  // no rules); inferred from the cnt<=cnt+1 / reset-on-cnt_end usage pattern.
  wire cnt_end = (cnt == (T_HALF - 16'd1));

  always @(posedge clk) begin
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
      done <= 1'b0;
      if (we) begin
        // queue one byte write: capture data, raise the pending flag
        if (cmd == CMD_WR) begin
          pend_wr <= 1'b1;
          tx <= din;
        end
        if (cmd == CMD_RD) pend_rd <= 1'b1;
        if (cmd == CMD_START) pend_start <= 1'b1;
        if (cmd == CMD_STOP) pend_stop <= 1'b1;
      end
      case (state)
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
      endcase
    end
  end

endmodule
