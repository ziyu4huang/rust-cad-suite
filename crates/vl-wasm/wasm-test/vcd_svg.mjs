// Shared-renderer gate: the wasm build renders the SAME deterministic SVG
// the vcd2svg CLI emits (byte-diff against the committed golden), supports
// handle-based re-render (VcdJs::to_svg — no re-parse on toggle), rejects
// hostile data via XML escaping, and surfaces VL-E codes on bad input.
import init, { vcd_parse, vcd_to_svg } from "../pkg/vl_wasm.js";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const ROOT = join(import.meta.dir, "..", "..", "..");
const read = (p) => readFileSync(join(ROOT, p), "utf8");

await init();

const text = read("corpus/e2e/51_vcd_counter/expect.vcd");
const golden = read("crates/vl-vcd/tests/goldens/counter.svg");

// 1. One-shot == CLI golden (three tiers, one renderer).
const oneShot = vcd_to_svg(text);
if (oneShot !== golden) {
  console.log("FAIL: one-shot vcd_to_svg != golden");
  process.exit(1);
}

// 2. Handle re-render (toggle path) with a row subset keeps determinism.
const vcd = vcd_parse(text);
const names = [];
for (let i = 0; i < vcd.varCount; i++) names.push(vcd.var_name(i));
const subset = names.filter((n) => n === "clk" || n === "dut.q");
const svg2 = vcd.to_svg(subset, 0, 0, 0n, 0n, 0);
if (!svg2.startsWith("<svg") || !svg2.includes("dut.q") || svg2.includes("dut.rst")) {
  console.log("FAIL: subset render wrong");
  process.exit(1);
}
const again = vcd.to_svg(subset, 0, 0, 0n, 0n, 0);
if (again !== svg2) {
  console.log("FAIL: re-render not deterministic");
  process.exit(1);
}

// 3. Window flags: t0/t1 are u64 (BigInt) gated by flag bits.
const win = vcd.to_svg([], 0, 0, 10000n, 40000n, 1 | 2);
if (!win.startsWith("<svg") || !win.includes(`0`) || !win.includes("40000")) {
  console.log("FAIL: window render wrong");
  process.exit(1);
}

// 4. Hostile VCD: XML escaping through the wasm boundary.
const hostile = [
  "$scope module tb $end",
  "$var wire 4 ! <img src=x onerror=alert(1)> $end",
  "$var string 64 $ s $end",
  "$upscope $end",
  "$enddefinitions $end",
  "#0",
  "b0101 !",
  "s<script>alert(1)</script> $",
].join("\n");
const hsvg = vcd_to_svg(hostile);
if (hsvg.includes("<script>") || hsvg.includes("<img") || !hsvg.includes("&lt;script&gt;")) {
  console.log("FAIL: hostile data not escaped");
  process.exit(1);
}

// 5. Error path: VL-E101 surfaces with the house format.
try {
  vcd_to_svg("$enddefinitions $end\n#abc\n");
  console.log("FAIL: bad #time accepted");
  process.exit(1);
} catch (e) {
  if (!String(e).includes("VL-E101")) {
    console.log(`FAIL: wrong error: ${e}`);
    process.exit(1);
  }
}
console.log("PASS vcd-svg");
