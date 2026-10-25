module tb;
  wire [7:0] q;
  mid u_mid(.q(q));
  initial begin
    #1 $display("tb got %0d", q);
  end
endmodule
