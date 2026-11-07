// tb_v3_neg_same_edge.v — TB-R1x 向量 TB(自著 MIT;v3 正向量:負沿 DUT 同沿阻塞寫)。
module tb_cl;
  reg clk;
  reg d;
  wire q;
  cl_dut tb__dut (
    .clk(clk),
    .d(d),
    .q(q)
  );
  initial clk = 1'b0;
  always #5 clk = ~clk;
  initial begin
    d = 1'b0;
    @(negedge clk); d = 1;
    $display("DONE");
    $finish;
  end
  initial begin
    #1000;
    $display("WATCHDOG");
    $finish;
  end
endmodule
