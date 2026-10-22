// Parser check against a real in-repo VCD (expect.vcd from 51_vcd_counter).
import { parseVcd } from "../wasm-demo/vcd.mjs";
import { readFileSync } from "node:fs";
import { join } from "node:path";
const ROOT = join(import.meta.dir, "..", "..", "..");
const text = readFileSync(join(ROOT, "corpus/e2e/51_vcd_counter/expect.vcd"), "utf8");
const { signals, tMax } = parseVcd(text);
console.log(`signals=${signals.length} tMax=${tMax}`);
console.log(signals.map((s) => `${s.name}(${s.changes.length})`).join(", "));
if (signals.length === 0 || tMax <= 0) process.exit(1);
console.log("PASS vcd-parser");
