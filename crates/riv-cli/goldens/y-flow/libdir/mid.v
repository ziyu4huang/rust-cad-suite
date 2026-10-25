module mid(q);
  output [7:0] q;
  wire [7:0] d;
  leaf u_leaf(.q(d));
  assign q = d + 8'd1;
endmodule
