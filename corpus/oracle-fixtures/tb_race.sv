module tb_race;
    reg a = 0;
    initial begin a = 1'b1; $display("A first? a=%b", a); end
    initial begin $display("B sees a=%b", a); end
endmodule
