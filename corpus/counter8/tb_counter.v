// Self-checking testbench for counter8 (M5 acceptance fixture).
`timescale 1ns/1ps

module tb_counter;

    reg clk = 0;
    reg rst = 1;
    wire [7:0] q;

    counter #(.WIDTH(8)) dut (
        .clk (clk),
        .rst (rst),
        .q   (q)
    );

    always #5 clk = ~clk;

    initial begin
        repeat (3) @(posedge clk);
        rst <= 1'b0;
        repeat (5) @(posedge clk);
        // Read after the NBA region of the 5th edge has committed q (4->5).
        // Without this #1, IEEE 1364-2005 §11 region ordering reads the
        // pre-NBA value 4 (oracle-verified: Verilator 5.050 prints FAIL q=4).
        #1;
        if (q !== 8'd5) $display("FAIL q=%0d", q);
        else            $display("PASS");
        $finish;
    end

endmodule
