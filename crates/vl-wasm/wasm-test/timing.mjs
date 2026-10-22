// Native-vs-wasm timing readout (same case, same host).
import init, { run_verilog_stdout } from "../pkg/vl_wasm.js";
import { readFileSync } from "node:fs";
import { join } from "node:path";
const ROOT = join(import.meta.dir, "..", "..", "..");
const src = readFileSync(join(ROOT, "corpus/e2e/55_mem_nba/tb.v"), "utf8");
await init();
// warmup
run_verilog_stdout("tb", [src]);
const t0 = Bun.nanoseconds();
const R = 200;
for (let i = 0; i < R; i++) run_verilog_stdout("tb", [src]);
console.log(`wasm: ${((Bun.nanoseconds()-t0)/1e6/R).toFixed(3)} ms/run (median-ish over ${R} runs, incl. re-elab)`);
