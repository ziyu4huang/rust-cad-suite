// rtl2spec 自著 fixture(repo MIT):正樣——復位+FSM+counter 齊備
// (e2e 斷言:modules>=1 且 rules>=3)。
module fsm_counter (
  input wire clk,
  input wire rst_n,
  input wire start,
  output reg [3:0] count,
  output reg busy
);
  reg [1:0] state;

  always @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
      state <= 2'b00;
      count <= 4'b0;
      busy  <= 1'b0;
    end else begin
      case (state)
        2'b00: begin
          if (start) begin
            state <= 2'b01;
            busy  <= 1'b1;
          end
        end
        2'b01: begin
          count <= count + 4'd1;
          if (count == 4'd15) begin
            state <= 2'b00;
            busy <= 1'b0;
          end
        end
        default: state <= 2'b00;
      endcase
    end
  end
endmodule
