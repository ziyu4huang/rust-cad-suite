// Exhaustive truth-table testbench for fulladd (delay-free except #1 settle).
`timescale 1ns/1ps

module tb_fulladd;
    reg a, b, cin;
    wire s, cout;
    integer i;

    fulladd dut (.a(a), .b(b), .cin(cin), .s(s), .cout(cout));

    initial begin
        for (i = 0; i < 8; i = i + 1) begin
            {a, b, cin} = i[2:0];
            #1;
            $display("%0d: a=%b b=%b cin=%b -> s=%b cout=%b sum=%0d",
                     i, a, b, cin, s, cout, {cout, s});
        end
        $display("done %h %b %0d", 8'hAB, 4'b1010, 16'd300);
        $finish;
    end
endmodule
