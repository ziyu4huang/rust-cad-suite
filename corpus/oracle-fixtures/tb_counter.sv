`timescale 1ns/1ps
module tb_counter;
    localparam int W = 4;
    reg clk = 0;
    reg rst_n = 0;
    wire [W-1:0] count;
    counter #(.WIDTH(W)) dut (.clk(clk), .rst_n(rst_n), .count(count));

    always #5 clk = ~clk;

    initial begin
        $monitor("[%0t] rst=%b count=%h", $time, rst_n, count);
        rst_n = 0;
        #23;
        rst_n = 1;
        #200;
        if (count == 4'hD) $display("PASS count=%h", count);
        else               $display("FAIL count=%h", count);
        $finish;
    end
endmodule
