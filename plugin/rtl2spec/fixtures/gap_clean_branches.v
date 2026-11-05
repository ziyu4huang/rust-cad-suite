// rtl2spec 拓展性實證 fixture(§17/S13;repo MIT):G8 負樣——變量條件/
// param 條件(§11.5 不求值契約)/x 位字面量(X-safe)三形,全部破口類零命中。
module clean_branches(input wire en, input wire [1:0] sel, output reg y);
  always @* begin
    y = 1'b0;
    if (en) begin
      y = 1'b1;
    end else begin
      y = sel[0];
    end
  end
endmodule

module param_cond(input wire a, output reg y);
  parameter WIDE = 1;
  always @* begin
    y = a;
    if (WIDE) begin
      y = ~a;
    end else begin
      y = a;
    end
  end
endmodule

module x_cond(input wire a, output reg y);
  always @* begin
    y = a;
    if (1'bx) begin
      y = ~a;
    end
  end
endmodule
