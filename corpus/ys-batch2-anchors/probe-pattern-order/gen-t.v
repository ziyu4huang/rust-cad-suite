module top(input clk, input arst, input d, input sb, input sa, input sc, output reg q);
always @(posedge clk or posedge arst) begin
  if (arst) q <= 1'b0;
  else if (sa) q <= (sb ? d : q);
  else q <= (sc ? d : q);
end
endmodule
