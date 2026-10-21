`define CAT(a,b) a``b
`define STR(x) `"x is %0d`"
`define W 8
`ifdef UNDEF_THING
  `define INNER 1
`endif
module preproc;
    wire `W = 0;
    wire `CAT(w,`W) = 0;
    initial $display(`STR(42), `W);
endmodule
