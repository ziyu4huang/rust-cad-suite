// rtl2spec 自著測試語料(repo MIT):TB 啟發判定($finish → 整檔跳過)。
module tb_sample;
  reg clk;
  initial begin
    #10;
    $display("done");
    $finish;
  end
endmodule
