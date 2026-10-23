// Shared-reader gate: the wasm build must expose the SAME vl-vcd reader the
// CLI and Bun-FFI facades use — one VCD parser behind every tier. Checked
// against the real disk artifact corpus/e2e/51_vcd_counter/expect.vcd.
import init, { vcd_parse } from "../pkg/vl_wasm.js";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const ROOT = join(import.meta.dir, "..", "..", "..");
const read = (p) => readFileSync(join(ROOT, p), "utf8");

await init();

const text = read("corpus/e2e/51_vcd_counter/expect.vcd");
const vcd = vcd_parse(text);
console.log(
  `vars=${vcd.varCount} tMax=${vcd.tMax} timescale=${vcd.timescale} warns=${vcd.warningCount}`
);
if (vcd.varCount < 3 || vcd.tMax <= 0 || vcd.warningCount !== 0) {
  console.log("FAIL: unexpected shape");
  process.exit(1);
}

// Times arrive as BigInt64Array (u64 across the boundary).
const names = [];
for (let i = 0; i < vcd.varCount; i++) names.push(vcd.var_name(i));
const qi = names.findIndex((n) => n.endsWith(".q") || n === "q");
if (qi < 0) {
  console.log(`FAIL: q not found in ${names.join(", ")}`);
  process.exit(1);
}
const times = vcd.times(qi);
const values = vcd.values(qi);
if (times.length !== values.length || times.length === 0) {
  console.log("FAIL: times/values misaligned");
  process.exit(1);
}
let mono = true;
for (let i = 1; i < times.length; i++) {
  if (times[i] <= times[i - 1]) mono = false;
}
if (typeof times[0] !== "bigint") {
  console.log(`FAIL: times[0] is ${typeof times[0]}, want bigint`);
  process.exit(1);
}
// The counter TB passes with q reaching 5 = 00000101 at the final change.
const lastVal = values[values.length - 1];
console.log(
  `${names[qi]}: ${times.length} changes, last t=${times[times.length - 1]} v=${lastVal}`
);
if (!mono || !lastVal.endsWith("101")) {
  console.log("FAIL: non-monotonic times or wrong final value");
  process.exit(1);
}

// Error path: malformed time is a VL-E101 rejection, not a silent parse.
try {
  vcd_parse("$enddefinitions $end\n#abc\n");
  console.log("FAIL: malformed #time accepted");
  process.exit(1);
} catch (e) {
  if (!String(e).includes("VL-E101")) {
    console.log(`FAIL: wrong error code: ${e}`);
    process.exit(1);
  }
}
console.log("PASS vcd-parse");
