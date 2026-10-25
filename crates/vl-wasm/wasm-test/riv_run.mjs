// riverilog-face gate: the wasm build exposes BOTH faces (ADR-0009) —
// riv_build compiles sources to RIV-1 and riv_run interprets them with the
// same kernel `rivvp` drives. Checked against the iverilog-recorded golden
// of the hello case; the $finish banner carries the source path as given
// on the command line, and the browser has no real paths (sources are
// source_N.v), so that one token is normalized before the byte compare.
import init, { riv_build, riv_run } from "../pkg/vl_wasm.js";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const ROOT = join(import.meta.dir, "..", "..", "..");
const read = (p) => readFileSync(join(ROOT, p), "utf8");

await init();

const src = read("corpus/iv-e2e/01_hello/tb.v");
const expected = read("corpus/iv-e2e/01_hello/expected.stdout");

const riv = riv_build("", [src]);
if (!riv.includes("thread") && !riv.includes("var ")) {
  console.log("FAIL: riv_build output does not look like a RIV-1 design");
  process.exit(1);
}
const r = riv_run(riv, []);
const got = r.stdout.replace("source_0.v", "tb.v");
if (got !== expected) {
  console.log(`FAIL: riv face stdout differs\n--- got ---\n${got}\n--- want ---\n${expected}`);
  process.exit(1);
}
console.log(`PASS riv face: riv_build ${riv.length} bytes -> riv_run byte-exact vs iverilog golden`);
