// rtl2spec 自著測試語料(repo MIT):G1..G7 觸發形,一模組一類。
module latch_trap(input wire a, input wire sel, output reg y);
  always @* begin
    if (sel) y = a;
  end
endmodule

// G1 正樣(R3-①fixture 矩陣):同行程內 x 有塊首默認賦值=對沖不報;
// y 無默認+分支不對稱=仍報且 certain(置信度=certain 的結構可證契約)。
module latch_default_matrix(input wire a, input wire b, output reg x, output reg y);
  always @* begin
    x = 1'b0;
    if (a) begin
      if (b) begin
        y = 1'b1;
      end
      x = 1'b1;
    end
  end
endmodule

module incomplete_case(input wire [1:0] sel, output reg y);
  always @* begin
    case (sel)
      2'b00: y = 1'b1;
      2'b01: y = 1'b0;
    endcase
  end
endmodule

module undriven_read(input wire a, output wire y);
  wire dangling;
  assign y = a & dangling;
endmodule

module missing_reset(input wire clk, input wire rst_n, output reg a, output reg b);
  always @(posedge clk or negedge rst_n) begin
    if (!rst_n) a <= 1'b0;
    else begin
      a <= b;
      b <= a;
    end
  end
endmodule

module comb_loop(input wire a, output wire x, output wire y);
  wire t1, t2;
  assign t1 = t2 & a;
  assign t2 = t1;
  assign x = t1;
  assign y = t2;
endmodule

module multi_driver(input wire clk, input wire d, output reg q);
  always @(posedge clk) q <= d;
  always @(negedge clk) q <= ~d;
endmodule

module blocking_race(input wire clk, input wire d, output reg q);
  always @(posedge clk) begin
    q = d;
  end
  always @(negedge clk) begin
    q <= ~q;
  end
endmodule
