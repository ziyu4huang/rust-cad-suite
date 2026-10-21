`define CAT(a,b) a``b
`define MSG `"%s=%0d`"
`define W 8
module preproc2;
    wire [`W-1:0] v = 0;
    localparam string S = `MSG(v);
    initial $display(`MSG(v), `CAT(v,1));
endmodule
