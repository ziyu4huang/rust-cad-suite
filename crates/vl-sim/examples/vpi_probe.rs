fn main() {
    use vl_syntax::parser;
    use vl_syntax::pp::{self, PpOptions};
    use vl_sim::interp::Sim;
    use vl_sim::vpi::*;
    let src = r#"`timescale 1ns/1ps
module tb;
  reg clk = 0;
  reg [7:0] q = 0;
  always #5 clk = ~clk;
  always @(posedge clk) q <= q + 1;
  initial begin
    repeat (3) @(posedge clk);
    #1 $finish;
  end
endmodule
"#;
    let mut sm = rcs_diag::SourceMap::new();
    let mut diags = rcs_diag::DiagSink::new();
    let out = pp::preprocess_source("t.v", src, &mut sm, &mut diags, &PpOptions::default());
    let parsed = parser::parse_source_file(out.tokens, &mut diags);
    let mut sim = Sim::build(&parsed.modules, "tb", out.timescale.as_deref()).unwrap();
    let q = sim.vpi_handle_by_name("q").unwrap();
    println!("q handle={} name={} size={}", q.0, sim.vpi_name(q), sim.vpi_size(q));
    let n = std::rc::Rc::new(std::cell::RefCell::new(0u32));
    sim.register_cb(CbReason::ValueChange(q), {
        let n = n.clone();
        Box::new(move |_s, i| { *n.borrow_mut() += 1; println!("VC fired t={} val={:?}", i.time, i.value.as_ref().map(|b| b.fmt_dec())); })
    });
    sim.run().unwrap();
    println!("vc count = {}", *n.borrow());
}
