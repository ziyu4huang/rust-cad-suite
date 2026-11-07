// tb_v4_delay_same_line.v — TB-R1x 向量 TB(自著 MIT;v4 負向量:同行 #1 關窗)。
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
    @(posedge clk); #1; d = 1;
    $display("DONE");
    $finish;
  end
  initial begin
    #1000;
    $display("WATCHDOG");
    $finish;
  end
endmodule
