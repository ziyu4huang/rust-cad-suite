// acc_pipe — 盲寫凍結產物(D-S5;REIMPL-PROTOCOL §5)。
// 來源=acc_pipe.spec.md 全文(spec-format:1):module __acc_pipe__acc_pipe;
// 三級流水:B001/B002 x1,y1←x,y;B003 s1≡{1'h0,x1}+{1'h0,y1};B004 h1≡s1[8:1];
// B005/B006 s2in←s1,h2in←h1;B007 s2≡{2'h0,h2in}+{1'h0,s2in};B008 scaled≡{5'h00,s2,s2in[0]};
// B009 out←scaled。全部暫存無復位(上電 x 語義,SCENE1:初值不定)。
module __acc_pipe__acc_pipe(
  input wire clk,
  input wire [7:0] x,
  input wire [7:0] y,
  output wire [15:0] out
);
  reg  [7:0]  x1;
  reg  [7:0]  y1;
  wire [8:0]  s1;
  wire [7:0]  h1;
  reg  [8:0]  s2in;
  reg  [7:0]  h2in;
  wire [9:0]  s2;
  wire [15:0] scaled;
  reg  [15:0] out_q;

  // B001/B002:stage0 輸入捕獲(+1拍)。
  always @(posedge clk) begin
    x1 <= x;
    y1 <= y;
  end

  // B003:s1=零擴展 9 位和(逐拍組合)。
  assign s1 = {1'h0, x1} + {1'h0, y1};
  // B004:h1=s1 高 8 位切片 [8:1]。
  assign h1 = s1[8:1];

  // B005/B006:stage2 輸入暫存(+1拍)。
  always @(posedge clk) begin
    s2in <= s1;
    h2in <= h1;
  end

  // B007:s2=零擴展 10 位和(逐拍組合)。
  assign s2 = {2'h0, h2in} + {1'h0, s2in};
  // B008:scaled=拼接縮放(逐拍組合;out 的背後暫存)。
  assign scaled = {5'h00, s2, s2in[0]};

  // B009:out 裝載 scaled(+1拍)。
  always @(posedge clk) begin
    out_q <= scaled;
  end
  assign out = out_q;

endmodule
