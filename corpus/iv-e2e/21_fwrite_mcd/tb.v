`timescale 1ns/1ps
// R5 mining: $fopen("w")/$fwrite/$fdisplay/$fclose + MCD stdout bit + a
// read-back loop through $readmemh (the file content self-verifies).
module tb;
  integer fd;
  reg [7:0] rb [0:3];
  initial begin
    fd = $fopen("wdata.hex", "w");
    $fwrite(fd, "aa bb cc\n");
    $fdisplay(fd, "dd");
    // MCD: fd | 1 writes the file AND stdout.
    $fwrite(fd | 1, "both %0d\n", 5);
    $fclose(fd);
    $readmemh("wdata.hex", rb);
    $display("rb=%h %h %h %h", rb[0], rb[1], rb[2], rb[3]);
    $finish;
  end
endmodule
