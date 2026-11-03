// rtl2spec 自著 fixture(repo MIT):G2 觸發形——case 無 default。
module incomplete_case(input wire [1:0] sel, output reg y);
  always @* begin
    case (sel)
      2'b00: y = 1'b1;
      2'b01: y = 1'b0;
    endcase
  end
endmodule
