// Feature gates beyond the core differential: (1) $readmem virtual-file
// injection, (2) rVPI JS callbacks across the wasm boundary.
import init, { WasmSim, run_verilog_with_files } from "../pkg/vl_wasm.js";
import { readFileSync } from "node:fs";
import { join } from "node:path";
const ROOT = join(import.meta.dir, "..", "..", "..");
const read = (p) => readFileSync(join(ROOT, p), "utf8");
await init();

// (1) readmem injection — the 53_readmem corpus case, files provided
// virtually instead of from disk; expectation = the same oracle golden.
const files = {
  "data.hex": read("corpus/e2e/53_readmem/data.hex"),
  "data.bin": read("corpus/e2e/53_readmem/data.bin"),
  "data2.hex": read("corpus/e2e/53_readmem/data2.hex"),
};
const expected = read("corpus/e2e/53_readmem/expected.stdout");
const got = run_verilog_with_files(
  "tb",
  [read("corpus/e2e/53_readmem/tb.v")],
  Object.keys(files),
  Object.values(files),
).stdout;
console.log(got === expected ? "PASS readmem-injection" : `FAIL readmem-injection\n${got}\n---\n${expected}`);
if (got !== expected) process.exit(1);

// (2) rVPI callbacks: ValueChange on lfsr + a timer, all in JS. Callbacks
// receive {time, value, valueHex}; sim reads happen before/after run
// (registered contract — no JS→sim re-entry mid-callback).
const sim = new WasmSim("tb", [read("corpus/e2e/44_lfsr/tb.v")]);
const vcLog = [];
sim.on_value_change("lfsr", (info) => vcLog.push(`${info.time}:${info.valueHex}`));
let sawTimer = false;
sim.on_after_delay(76000n, () => { sawTimer = true; });
sim.run();
// 8 NBA updates (t=5000..75000); the initializer ran before registration.
const okCb = vcLog.length === 8 && vcLog[vcLog.length - 1] === "75000:b0";
const sampled = sim.vpi_get("lfsr", "hex");
console.log(`PASS rVPI-callbacks (vc=${vcLog.length}, last=${vcLog[vcLog.length - 1]}, timer=${sawTimer}, sample=${sampled})`);
if (!okCb || !sawTimer || sampled !== "b0") process.exit(1);
console.log("== wasm feature gates: 2/2 ==");
