module xprobe;
    reg [3:0] a = 4'b1x00;
    reg [3:0] b = 4'b1x00;
    reg signed [3:0] s = 4'sbx101;
    initial begin
        $display("eq=%b", a == b);
        $display("sum=%b", a + 4'b0001);
        $display("ashr=%b", $signed(s) >>> 1);
        if (a[2]) $display("taken"); else $display("not-taken");
        $display("ternary=%b", a[2] ? 4'b1111 : 4'b0000);
        $display("casez=%b", (a === 4'b1x00));
    end
endmodule
