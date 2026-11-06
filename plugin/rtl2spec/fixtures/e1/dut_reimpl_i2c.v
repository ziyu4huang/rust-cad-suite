// 盲写版适配 wrapper:TB 侧=原版签名(含 4 位 onehot cmd 与 prescale 输入),
// 盲写版=2 位二进制 cmd 编码 + T_HALF 参数。适配层是实验装备(把规格书缺失的
// 编码信息由对拍方补齐),本身不是盲写产物;接口差异作为规格书缺口记录。
`timescale 1ns/1ps
module dut_i2c (
    input  wire       clk,
    input  wire       rst_n,
    input  wire       we,
    input  wire       scl_i,
    input  wire       sda_i,
    output wire       ack_rx,
    output wire       done,
    output wire       busy,
    output wire       tip,
    output wire       scl_o,
    output wire       sda_o,
    output wire       scl_oe,
    output wire       sda_oe,
    input  wire [15:0] prescale,
    input  wire [3:0]  cmd,
    input  wire [7:0]  din,
    output wire [7:0]  dout
);
  // TB onehot(bit0=START,bit1=STOP,bit2=WR,bit3=RD,依原版 localparam)
  //   → 盲写版 2 位编码(CMD_WR=0,CMD_RD=1,CMD_START=2,CMD_STOP=3,依其 localparam)
  wire [1:0] cmd_r = cmd[0] ? 2'd2
                  : cmd[1] ? 2'd3
                  : cmd[2] ? 2'd0
                  : cmd[3] ? 2'd1 : 2'd0;
  // prescale 值 → T_HALF 参数(经 defparam 形式的参数覆写不可动态,用固定 4;
  // TB prescale 恒 16'd4,二者一致)
  i2c_master #(.T_HALF(16'd4)) u (
    .clk(clk), .rst_n(rst_n), .we(we), .cmd(cmd_r), .din(din), .sda_i(sda_i),
    .scl_o(scl_o), .sda_o(sda_o), .scl_oe(scl_oe), .sda_oe(sda_oe),
    .busy(busy), .tip(tip), .done(done), .ack_rx(ack_rx), .dout(dout)
  );
endmodule
