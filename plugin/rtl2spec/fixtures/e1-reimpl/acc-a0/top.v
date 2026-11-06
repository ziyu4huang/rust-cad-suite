// ==========================================================================
// 盲写重实现:corpus/e2e/58_xls_acc_pipe/acc_pipe.v
// 依据:仅内联规则书(R001..R007,共 7 条)。规格书未定义处按假设 A1..A8
//       实现(逐条见下),这些假设即规格书的缺口证据。
// 目标:Verilog-2001,可综合,单文件单模块。
// ==========================================================================
//
// 假设一览:
//   A1 模块名照抄规则书标题 "__acc_pipe__acc_pipe"(含双前缀限定形)。
//   A2 端口清单规格书未给;由寄存器命名模式 "<port>__<role>_flop"
//      (x__input_flop / y__input_flop / out__output_flop)反推出输入 x、y
//      与输出 out;时钟端口名 clk 取自各时序规则的 anchor 文本。
//   A3 数据位宽规格书完全未给(仅拼接扩展位 1'h0/2'h0 有据)。参数化
//      WIDTH(默认 32),宽度关系由 R003/R006 零扩展自洽反推:
//      x=y=WIDTH 位;p1_add_96_comb=p0_add_20=WIDTH+1 位;
//      p0_bit_slice_48=WIDTH 位(由 R006 {2'h0,·} 与 {1'h0,p0_add_20}
//      等宽自洽支撑);p2_add_98_comb=p2_scaled_comb=out=WIDTH+2 位。
//   A4 p1_bit_slice_103_comb 规则书无定义规则(R005 消费之,规则书沉默)。
//      假设 = 取 p1_add_96_comb 低 WIDTH 位(丢进位回绕加法);取位方向
//      与输入源规格书均未给。
//   A5 p2_scaled_comb 规则书无定义规则(R007 消费之,规则书沉默)。
//      假设 = p2_add_98_comb 左移 1 位(乘 2,满宽截断);缩放方向与量
//      规格书完全未给。
//   A6 无复位、无初值:规则书未提复位;寄存器上电值未定义(X)。
//   A7 时序语句按证据行号分组为 3 个 always @(posedge clk) 块
//      (14-15 / 28-29 / 41);纯 nonblocking、单写者,与单块等价。
//   A8 R003/R006 的 assign 与全部时序语句均逐字照抄证据文本。
// ==========================================================================

module __acc_pipe__acc_pipe #(
    parameter WIDTH = 32   // A3
) (
    input  wire             clk,   // R001..R007 公共 anchor
    input  wire [WIDTH-1:0] x,     // A2
    input  wire [WIDTH-1:0] y,     // A2
    output wire [WIDTH+1:0] out    // A2;宽度见 A3
);

    // ---- stage-1 输入寄存器 ----
    reg [WIDTH-1:0] x__input_flop;  // R001
    reg [WIDTH-1:0] y__input_flop;  // R002

    // ---- p1 阶段组合逻辑 ----
    // R003(证据文本逐字):
    wire [WIDTH:0] p1_add_96_comb;
    assign p1_add_96_comb = {1'h0, x__input_flop} + {1'h0, y__input_flop};

    // A4(规则缺口;信号名逐字取自 R005 证据):
    wire [WIDTH-1:0] p1_bit_slice_103_comb;
    assign p1_bit_slice_103_comb = p1_add_96_comb[WIDTH-1:0];

    // ---- stage-2 寄存器(p1 组合 → p2) ----
    reg [WIDTH:0]   p0_add_20;        // R004
    reg [WIDTH-1:0] p0_bit_slice_48;  // R005

    // ---- p2 阶段组合逻辑 ----
    // R006(证据文本逐字):
    wire [WIDTH+1:0] p2_add_98_comb;
    assign p2_add_98_comb = {2'h0, p0_bit_slice_48} + {1'h0, p0_add_20};

    // A5(规则缺口;信号名逐字取自 R007 证据):
    wire [WIDTH+1:0] p2_scaled_comb;
    assign p2_scaled_comb = p2_add_98_comb << 1;

    // ---- 输出寄存器 ----
    reg [WIDTH+1:0] out__output_flop;  // R007
    assign out = out__output_flop;

    // ---- 时序块 1(R001/R002;证据行 14-15) ----
    always @(posedge clk) begin
        x__input_flop <= x;
        y__input_flop <= y;
    end

    // ---- 时序块 2(R004/R005;证据行 28-29) ----
    always @(posedge clk) begin
        p0_add_20       <= p1_add_96_comb;
        p0_bit_slice_48 <= p1_bit_slice_103_comb;
    end

    // ---- 时序块 3(R007;证据行 41) ----
    always @(posedge clk) begin
        out__output_flop <= p2_scaled_comb;
    end

endmodule
