`define CAT(a,b) a``b
`define MSG `"%s=%0d`"
`define W 8
module preproc3;
    wire [`W-1:0] v = 0;
    wire v1 = 1'b1;
    initial $display(`MSG, `CAT(v,1), 5);
endmodule
