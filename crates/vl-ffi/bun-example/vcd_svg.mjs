// "Agent renders waveforms": run a corpus case through the FFI, then render
// the dump to a deterministic SVG with the SAME renderer the vcd2svg CLI and
// the wasm facade use (rcs_vcd_svg, default options — ABI v1, ADR-0005).
// Writes wave.svg so a human (or a multimodal agent) can look at it.
import { dlopen, CString, suffix } from "bun:ffi";
import { readFileSync, mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

const ROOT = join(import.meta.dir, "..", "..", "..");
const read = (p) => readFileSync(join(ROOT, p), "utf8");

const dylib = join(ROOT, "target", "release", `libvl_ffi.${suffix}`);
const lib = dlopen(dylib, {
  rcs_run: { args: ["cstring"], returns: "ptr" },
  rcs_vcd_svg: { args: ["cstring"], returns: "ptr" },
  rcs_free: { args: ["ptr"], returns: "void" },
});
const take = (ptr) => {
  const s = new CString(ptr).toString();
  lib.symbols.rcs_free(ptr);
  return s;
};

// 1. Run + force the dump (same shape as vcd_read.mjs).
const caseDir = "corpus/e2e/51_vcd_counter";
const tmp = mkdtempSync(join(tmpdir(), "vl-ffi-svg-"));
const vcdPath = join(tmp, "dump.vcd");
const manifest = [
  "top tb",
  `file ${join(ROOT, caseDir, "tb.v")}`,
  `vcd ${vcdPath}`,
].join("\n");
writeFileSync(join(tmp, "manifest.vlm"), manifest);
const stdout = take(lib.symbols.rcs_run(join(tmp, "manifest.vlm")));
if (stdout.startsWith("ERROR: ") || stdout !== read(join(caseDir, "expected.stdout"))) {
  console.log(`FAIL ffi-vcd-svg stdout: ${stdout}`);
  process.exit(1);
}

// 2. Render the dump to SVG via the shared renderer.
const vcdText = readFileSync(vcdPath, "utf8");
const svg = take(lib.symbols.rcs_vcd_svg(vcdText));
if (svg.startsWith("ERROR: ")) {
  console.log(`FAIL ffi-vcd-svg render: ${svg}`);
  process.exit(1);
}
if (!svg.startsWith("<svg") || !svg.includes(">q<") || !svg.includes("00000101")) {
  console.log("FAIL: svg shape wrong (header/q row/final-value title)");
  process.exit(1);
}
const outPath = join(tmp, "wave.svg");
writeFileSync(outPath, svg);
console.log(`svg bytes=${svg.length} → ${outPath}`);

// 3. Error contract: VL-E101 through the ERROR: prefix.
const err = take(lib.symbols.rcs_vcd_svg("$enddefinitions $end\n#abc\n"));
if (!err.startsWith("ERROR: VL-E101")) {
  console.log(`FAIL: expected ERROR: VL-E101, got: ${err}`);
  process.exit(1);
}
console.log("PASS ffi-vcd-svg");
