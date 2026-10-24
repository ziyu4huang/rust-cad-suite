// Differential gate: the WASM engine must reproduce the oracle-recorded
// expectations byte-exactly (same goldens the native CLI is gated on) —
// three engines, one truth: Verilator (recorder) → native Rust → WASM.
import init, { run_verilog_stdout } from "../pkg/vl_wasm.js";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const ROOT = join(import.meta.dir, "..", "..", "..");
const read = (p) => readFileSync(join(ROOT, p), "utf8");

await init();

const cases = [
  { dir: "corpus/fulladd", top: "", files: ["fulladd.v", "tb_fulladd.v"] },
  { dir: "corpus/counter8", top: "", files: ["counter.v", "tb_counter.v"] },
  { dir: "corpus/e2e/44_lfsr", top: "tb", files: ["tb.v"] },
  { dir: "corpus/e2e/52_mem_basic", top: "tb", files: ["tb.v"] },
  { dir: "corpus/e2e/54_gates", top: "tb", files: ["tb.v"] },
  { dir: "corpus/e2e/55_mem_nba", top: "tb", files: ["tb.v"] },
  { dir: "corpus/e2e/57_xls_add3", top: "tb", files: ["add3.v", "tb.v"] },
  { dir: "corpus/e2e/60_wide_values", top: "tb", files: ["tb.v"] },
];

let pass = 0;
for (const c of cases) {
  const sources = c.files.map((f) => read(join(c.dir, f)));
  const expected = read(join(c.dir, "expected.stdout"));
  const got = run_verilog_stdout(c.top, sources);
  if (got === expected) {
    console.log(`PASS ${c.dir}`);
    pass++;
  } else {
    console.log(`FAIL ${c.dir}\n  --- expected ---\n${expected}\n  --- wasm ---\n${got}`);
  }
}
console.log(`== wasm differential: ${pass}/${cases.length} ==`);
if (pass !== cases.length) process.exit(1);
