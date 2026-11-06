// E1 公共 TB — i2c_master(原版 corpus/e2e/59_i2c_master/i2c_master.v vs 盲写版
// reimpl/i2c_master/top.v)。TB 经统一 wrapper 接口 dut_i2c(=原版签名)驱动;
// 两版各自 wrapper(dut_orig_i2c.v 直连 / dut_reimpl_i2c.v 适配 cmd 编码与 prescale)。
// 总线模型:开漏合成 scl_bus = scl_oe?scl_o:1;sda_bus = sda_oe?sda_o:(slave_oe?slave_sda:1)。
// 受控从机(TB 装备,slave_mode 由场景设定):WR=上升沿采位+第9位驱动ACK;
//   RD=SCL下降沿逐位驱动 slave_tx_ref(MSB first)。位相:START 末下降沿驱动 bit0,
//   master 在随后高半段末采样——与 DUT 的 SHIFT/ACK 位相对齐。
// 全部等待带超时守卫(防 TB 挂死;超时=EV timeout 行,同样参与 diff)。
`timescale 1ns/1ps
module tb_i2c;
  reg clk = 0, rst_n = 0, we = 0;
  reg  [15:0] prescale = 16'd4;
  reg  [3:0]  cmd = 4'b0;
  reg  [7:0]  din = 8'd0;
  reg  scl_i = 1'b1;      // 原版语义:bus sense(unused v1)
  wire ack_rx, done, busy, tip, scl_o, sda_o, scl_oe, sda_oe;
  wire [7:0] dout;
  // 受控从机(声明先于实例化:sda_bus 被 DUT 端口引用)
  reg slave_mode_rd = 0;     // 0=WR(采样+ACK) 1=RD(驱动 tx_ref)
  reg [7:0] slave_tx_ref = 8'hA5;
  reg [7:0] slave_rx = 8'd0;
  reg [3:0] slave_bits = 0;
  reg       slave_oe = 0;
  reg       slave_sda = 1;

  // ---- 总线合成 ----
  wire scl_bus = scl_oe ? scl_o : 1'b1;
  wire sda_bus = sda_oe ? sda_o : (slave_oe ? slave_sda : 1'b1);

  dut_i2c dut (.clk(clk), .rst_n(rst_n), .we(we), .scl_i(scl_i), .sda_i(sda_bus),
    .ack_rx(ack_rx), .done(done), .busy(busy), .tip(tip),
    .scl_o(scl_o), .sda_o(sda_o), .scl_oe(scl_oe), .sda_oe(sda_oe),
    .prescale(prescale), .cmd(cmd), .din(din), .dout(dout));

  integer cyc = 0;
  always @(posedge clk) cyc <= cyc + 1;

  // ---- 从机模型 ----
  // SCL 上升沿:WR 采位;第 9 个上升沿=ACK 已采,打印 rxbyte 并重位
  always @(posedge scl_bus) begin
    if (!slave_mode_rd) begin
      if (slave_bits < 8) begin
        slave_rx = {slave_rx[6:0], sda_bus};
        slave_bits = slave_bits + 1;
      end else begin
        slave_bits = 0;
        $display("EV rxbyte=%02x", slave_rx);
      end
    end
  end
  // SCL 下降沿:WR 第9位驱动 ACK(oe=1,sda=0)、其余释放;RD 逐位驱动
  // (RD 首 bit 由 slave_rd_begin 立即驱动——HOLD→RD 时 SCL 恒低无下降沿可挂)
  always @(negedge scl_bus) begin
    if (!slave_mode_rd) begin
      if (slave_bits == 8) begin
        slave_sda = 0; slave_oe = 1;      // ACK 位
      end else begin
        slave_oe = 0;                      // 第9位已收,释放
      end
    end else begin
      if (slave_bits >= 1 && slave_bits < 8) begin
        slave_sda = slave_tx_ref[7];
        slave_tx_ref = {slave_tx_ref[6:0], 1'b0};
        slave_bits = slave_bits + 1;
        slave_oe = 1;
      end else begin
        slave_oe = 0;                      // 9位结束,释放
      end
    end
  end
  // RD 起始:立即驱动第 0 位(master 释放 SDA 后高半段采样它)
  task slave_rd_begin;
    begin
      slave_bits = 1;
      slave_sda = slave_tx_ref[7];
      slave_tx_ref = {slave_tx_ref[6:0], 1'b0};
      slave_oe = 1;
    end
  endtask

  // ---- START/STOP 检测(SCL 高时 SDA 变化) ----
  reg sda_bus_d = 1, scl_bus_d = 1;
  always @(negedge clk) begin
    if (scl_bus && scl_bus_d && sda_bus !== sda_bus_d) begin
      if (sda_bus_d === 1'b1 && sda_bus === 1'b0) begin
        $display("EV start");
        slave_bits = 0;                    // 每 START 重位
      end
      else if (sda_bus_d === 1'b0 && sda_bus === 1'b1) $display("EV stop");
    end
    sda_bus_d <= sda_bus;
    scl_bus_d <= scl_bus;
  end

  // ---- done 脉冲监测(单拍性) ----
  reg done_d = 0;
  integer done_high = 0;
  always @(posedge clk) begin
    if (done) done_high = done_high + 1; else done_high = 0;
    if (done && !done_d) $display("EV done_rise dout=%02x ack_rx=%b", dout, ack_rx);
    if (!done && done_d) $display("EV done_fall high=%0d", done_high);
    done_d <= done;
  end

  always #5 clk = ~clk;
  task tick; begin @(posedge clk); #1; end endtask

  // 带超时守卫的等待(task input 是调用时一次性求值,故用专用 task 直接读层级信号)
  task wait_tip; integer g; begin
    g = 0; while (tip !== 1'b1 && g < 2000) begin tick; g = g + 1; end
    if (tip !== 1'b1) $display("EV timeout_tip"); end endtask
  task wait_done; integer g; begin
    g = 0; while (done !== 1'b1 && g < 2000) begin tick; g = g + 1; end
    if (done !== 1'b1) $display("EV timeout_done"); end endtask
  task wait_busy0; integer g; begin
    g = 0; while (busy !== 1'b0 && g < 4000) begin tick; g = g + 1; end
    if (busy !== 1'b0) $display("EV timeout_busy0"); end endtask

  task issue(input [3:0] c, input [7:0] d);
    begin
      cmd = c; din = d; we = 1; tick; we = 0; cmd = 4'b0; din = 8'd0;
    end
  endtask
  task sample_status;
    $display("ST busy=%b tip=%b dout=%02x ack_rx=%b scl=%b sda=%b oe=%b%b",
      busy, tip, dout, ack_rx, scl_bus, sda_bus, scl_oe, sda_oe);
  endtask

  integer i;
  initial begin
    // S1 复位与 IDLE 稳态
    $display("SCENE1 reset_idle");
    rst_n = 0; repeat (3) tick;
    rst_n = 1; tick; tick;
    sample_status;

    // S2 START 事务(等 tip 置位,START 无 done)
    $display("SCENE2 start");
    slave_mode_rd = 0; slave_bits = 0;
    issue(4'b0001, 8'd0);   // CMD_START
    wait_tip;
    tick; sample_status;

    // S3 WR 0x55
    $display("SCENE3 wr_55");
    issue(4'b0100, 8'h55);  // CMD_WR
    wait_done;
    tick; sample_status;

    // S4 WR 0xAA(HOLD 直接续)
    $display("SCENE4 wr_aa");
    issue(4'b0100, 8'hAA);
    wait_done;
    tick; sample_status;

    // S5 RD(从机全释放:master 采上拉全 1,期望 dout=ff;对拍 RD 状态机路径
    //  —采样点/位计数/done/ack_rx 保持—;从机驱动数据维度由 WR ACK 场景覆盖)
    $display("SCENE5 rd_released");
    slave_mode_rd = 1; slave_tx_ref = 8'hFF; slave_bits = 8; slave_oe = 0;
    issue(4'b1000, 8'd0);   // CMD_RD
    wait_done;
    tick; sample_status;

    // S6 reSTART(reSTART 后 rd_mode 残留 → SHIFT 移幽灵字节,原版 quirk 亦入对拍)
    $display("SCENE6 restart");
    slave_mode_rd = 0; slave_bits = 0;
    issue(4'b0001, 8'd0);
    wait_tip;
    tick; sample_status;

    // S7 幽灵字节完成后回 HOLD,再 WR 0x0F
    $display("SCENE7 wr_0f_after_restart");
    wait_done;
    tick;
    issue(4'b0100, 8'h0F);
    wait_done;
    tick; sample_status;

    // S8 STOP
    $display("SCENE8 stop");
    issue(4'b0010, 8'd0);   // CMD_STOP
    wait_busy0;
    tick; tick; sample_status;

    // S9 tip=1 期间发命令(排队语义):WR 开始后立刻发 START
    $display("SCENE9 queue_start_during_tip");
    slave_mode_rd = 0; slave_bits = 0;
    issue(4'b0100, 8'h33);
    tick; tick;                 // 进入 SHIFT
    issue(4'b0001, 8'd0);       // tip=1 时发 START(排队)
    wait_done;
    tick;
    wait_tip;
    tick; sample_status;

    // S10 传输中途复位(异步 vs 同步复位差异现形点)
    $display("SCENE10 mid_transfer_reset");
    issue(4'b0100, 8'h77);
    repeat (3) tick;
    rst_n = 0; tick; sample_status;      // 异步=立即复位态;同步=旧值
    rst_n = 1; tick; tick; sample_status;

    // S11 STOP 回 IDLE 稳态
    $display("SCENE11 idle_after_stop");
    issue(4'b0010, 8'd0);
    wait_busy0;
    repeat (3) tick; sample_status;

    $display("DONE");
    $finish;
  end
endmodule
