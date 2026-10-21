`timescale 1ns/1ps
module tb;
  initial begin
    $display("a=%h b=%b o=%o d=%0d", 8'hFF, 4'b1010, 6'o52, 16'd300);
    $display("sd=%0d sx=%h", -8'sd100, 12'shABC);
    $display("big=%h", 16'hABCD);
    $display("unbased=%0d 'hx=%h", 'd10, 'hx);
    $display("zlit=%b", 2'b1z);
    $finish;
  end
endmodule
