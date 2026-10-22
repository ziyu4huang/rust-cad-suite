module __add3__add3(
  input wire clk,
  input wire [7:0] x,
  input wire [7:0] y,
  input wire [7:0] z,
  output wire [8:0] out
);
  // ===== Pipe stage 0:

  // Registers for pipe stage 0:
  reg [7:0] x__input_flop;
  reg [7:0] y__input_flop;
  reg [7:0] z__input_flop;
  always @ (posedge clk) begin
    x__input_flop <= x;
    y__input_flop <= y;
    z__input_flop <= z;
  end

  // ===== Pipe stage 1:
  wire [8:0] p1_add_38_comb;
  wire [8:0] p1_add_45_comb;
  assign p1_add_38_comb = {1'h0, x__input_flop} + {1'h0, y__input_flop};
  assign p1_add_45_comb = p1_add_38_comb + {1'h0, z__input_flop};

  // Registers for pipe stage 1:
  reg [8:0] out__output_flop;
  always @ (posedge clk) begin
    out__output_flop <= p1_add_45_comb;
  end
  assign out = out__output_flop;
endmodule
