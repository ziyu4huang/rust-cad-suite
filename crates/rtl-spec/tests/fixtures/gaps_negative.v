// rtl2spec 自著測試語料(repo MIT):G1..G7 零誤報形——良構設計,各類
// 判準在其上必須零命中(每類判定配零誤報形,AGENTS §4 第 2 條精神)。
module latch_ok(input wire a, input wire sel, output reg y);
  always @* begin
    if (sel) y = a;
    else y = 1'b0;
  end
endmodule

// G1 負樣(R3-①fixture 矩陣):塊首默認賦值形——y/z 有頂層無條件默認
// 賦值,條件分支不對稱亦不鎖存,必須零命中(subbytes.v 3/4 誤報同形)。
module latch_default_ok(input wire a, input wire sel, output reg y, output reg z);
  always @* begin
    y = 1'b0;
    z = 1'b1;
    if (sel) begin
      y = a;
      if (a) z = 1'b0;
    end
  end
endmodule

module case_ok(input wire [1:0] sel, output reg y);
  always @* begin
    case (sel)
      2'b00: y = 1'b1;
      default: y = 1'b0;
    endcase
  end
endmodule

module driven_ok(input wire a, output wire y);
  wire mid;
  assign mid = a;
  assign y = mid;
endmodule

module full_reset(input wire clk, input wire rst_n, output reg a, output reg b);
  always @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
      a <= 1'b0;
      b <= 1'b0;
    end else begin
      a <= b;
      b <= a;
    end
  end
endmodule

module chain_ok(input wire a, output wire x, output wire y);
  wire t1, t2;
  assign t1 = a;
  assign t2 = t1 + 1'b1;
  assign x = t2;
  assign y = x;
endmodule

module word_writes_ok(
  input wire clk,
  input wire rst_n,
  input wire we,
  input wire [2:0] addr,
  input wire [7:0] din,
  output reg [7:0] m0,
  output reg [7:0] m1
);
  always @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
      m0 <= 8'b0;
      m1 <= 8'b0;
    end else if (we) begin
      m0[addr] <= din;
      m1[addr] <= din;
    end
  end
endmodule

module blocking_ok(input wire clk, input wire rst_n, input wire d, output reg q, output wire w);
  always @(posedge clk or negedge rst_n) begin
    if (!rst_n) q <= 1'b0;
    else q = d;
  end
  assign w = q;
endmodule
