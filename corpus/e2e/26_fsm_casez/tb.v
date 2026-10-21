`timescale 1ns/1ps
module tb;
  reg clk = 0, rst = 1, go = 0;
  reg [1:0] state = 2'd0;
  reg [3:0] steps = 0;
  always #5 clk = ~clk;
  // No same-edge race: control inputs change mid-cycle (after #1), so every
  // process samples stable values — deterministic by construction.
  always @(posedge clk) begin
    if (rst) begin
      state <= 2'd0;
      steps <= 0;
    end else begin
      casez (state)
        2'b00: if (go) state <= 2'b01;
        2'b01: begin state <= 2'b10; steps <= steps + 1; end
        2'b10: begin state <= 2'b00; steps <= steps + 1; end
        default: state <= 2'd0;
      endcase
    end
  end
  initial begin
    #1;
    rst = 0; go = 1;
    repeat (6) @(posedge clk);
    #1;
    $display("state=%b steps=%0d", state, steps);
    $finish;
  end
endmodule
