// rtl2spec 自著測試語料(repo MIT):復位+FSM+counter+timing+arithmetic 齊備。
module rulebook_sample (
  input wire clk,
  input wire rst_n,
  input wire start,
  output reg [7:0] count,
  output reg busy
);
  reg [1:0] state;

  always @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
      state <= 2'b00;
      count <= 8'b0;
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
          count <= count + 8'd1;
          if (count == 8'd255) begin
            state <= 2'b00;
            busy <= 1'b0;
          end
        end
        default: state <= 2'b00;
      endcase
    end
  end

  wire calc;
  assign calc = start | busy;
endmodule
