// i2c_master.v — clean-room minimal I2C master (spec-i2c demo DUT).
//
// Provenance: written from the I2C-bus protocol description for the
// rust-cad-suite spec-verification demo. OpenCores' i2c core is GPL and was
// used only as a read-only register-level inspiration (prescale/command/
// status shape); no code was copied.
//
// Documented simplifications (the spec, layer 1 — see crates/spec-i2c/src):
//   - single master, 7-bit address carried as the ordinary first byte
//   - no multi-master arbitration, no clock stretching (scl_i unused), no
//     general call, no bus-free timeout
//   - master always ACKs received bytes (RD)
//   - one queued command: `we` strobes cmd+din; the driver issues `we` only
//     when tip is low (IDLE or HOLD)
//   - split open-drain pins: scl_o/sda_o low-true drives (0 = pull down),
//     scl_oe/sda_oe output enables; bus levels are computed by the testbench
//
// Language subset: posedge/negedge async reset, NBA, case, params — the
// constructs the rverilator differential corpus pins as verified (M5-M7).

module i2c_master (
    input  wire        clk,
    input  wire        rst_n,
    input  wire [15:0] prescale,
    input  wire        we,
    input  wire [3:0]  cmd,     // {STOP, START, RD, WR} = bit[3..0]
    input  wire [7:0]  din,
    output reg  [7:0]  dout,
    output reg         ack_rx,  // ack bit sampled for the last TX byte (0 = ACK)
    output reg         done,    // 1-clk strobe: a byte completed
    output reg         busy,    // a command sequence is in progress
    output reg         tip,     // a byte transfer is in progress
    input  wire        scl_i,   // bus sense (unused v1: no stretching)
    input  wire        sda_i,
    output reg         scl_o,
    output reg         sda_o,
    output reg         scl_oe,
    output reg         sda_oe
);

  localparam [3:0] ST_IDLE = 4'd0;
  localparam [3:0] ST_START = 4'd1;
  localparam [3:0] ST_SHIFT = 4'd2;
  localparam [3:0] ST_ACK = 4'd3;
  localparam [3:0] ST_HOLD = 4'd4;
  localparam [3:0] ST_STOP = 4'd5;

  localparam [3:0] CMD_START = 4'b0001;
  localparam [3:0] CMD_STOP = 4'b0010;
  localparam [3:0] CMD_WR = 4'b0100;
  localparam [3:0] CMD_RD = 4'b1000;

  reg [3:0]  state;
  reg [15:0] cnt;      // half-period counter
  reg        half;     // 0 = SCL low half, 1 = SCL high half
  reg [3:0]  bitcnt;   // shift bit index 0..7 (MSB first)
  reg [1:0]  step;     // START/STOP sequencer step
  reg [7:0]  tx;       // byte being transmitted
  reg        rd_mode;  // current byte direction
  reg        pend_start, pend_stop, pend_wr, pend_rd;

  wire cnt_end = (cnt >= prescale - 1);

  always @(posedge clk or negedge rst_n) begin
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

      // Command latch: driver issues `we` only when tip is low.
      if (we) begin
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

        default: state <= ST_IDLE;
      endcase
    end
  end
endmodule
