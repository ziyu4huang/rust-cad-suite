module __acc_pipe__acc_pipe(
  input wire clk,
  input wire [7:0] x,
  input wire [7:0] y,
  output wire [15:0] out
);
  // ===== Pipe stage 0:

  // Registers for pipe stage 0:
  reg [7:0] x__input_flop;
  reg [7:0] y__input_flop;
  always @ (posedge clk) begin
    x__input_flop <= x;
    y__input_flop <= y;
  end

  // ===== Pipe stage 1:
  wire [8:0] p1_add_96_comb;
  wire [7:0] p1_bit_slice_103_comb;
  assign p1_add_96_comb = {1'h0, x__input_flop} + {1'h0, y__input_flop};
  assign p1_bit_slice_103_comb = p1_add_96_comb[8:1];

  // Registers for pipe stage 1:
  reg [8:0] p0_add_20;
  reg [7:0] p0_bit_slice_48;
  always @ (posedge clk) begin
    p0_add_20 <= p1_add_96_comb;
    p0_bit_slice_48 <= p1_bit_slice_103_comb;
  end

  // ===== Pipe stage 2:
  wire [9:0] p2_add_98_comb;
  wire [15:0] p2_scaled_comb;
  assign p2_add_98_comb = {2'h0, p0_bit_slice_48} + {1'h0, p0_add_20};
  assign p2_scaled_comb = {5'h00, p2_add_98_comb, p0_add_20[0]};

  // Registers for pipe stage 2:
  reg [15:0] out__output_flop;
  always @ (posedge clk) begin
    out__output_flop <= p2_scaled_comb;
  end
  assign out = out__output_flop;
endmodule
