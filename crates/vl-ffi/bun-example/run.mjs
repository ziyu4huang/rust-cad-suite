// Bun → native rverilator, in-process via bun:ffi (no process spawn):
//   dlopen libvl_ffi.dylib → rcs_run(manifest) → stdout string.
// Differential: output must equal the oracle-recorded corpus golden.
// The performance-ladder framing (PLAN tier B notes): an in-process FFI
// call skips fork/exec + parse startup entirely.
import { dlopen, CString, suffix } from "bun:ffi";
import { readFileSync, mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

const ROOT = join(import.meta.dir, "..", "..", "..");
const read = (p) => readFileSync(join(ROOT, p), "utf8");

const dylib = join(ROOT, "target", "release", `libvl_ffi.${suffix}`);
const lib = dlopen(dylib, {
  rcs_version: { args: [], returns: "ptr" },
  rcs_run: { args: ["cstring"], returns: "ptr" }, // cstring: JS string marshalled for us
  rcs_free: { args: ["ptr"], returns: "void" },
});
const str = (ptr) => new CString(ptr).toString();

console.log(`version: ${str(lib.symbols.rcs_version())}`);

// Case: 44_lfsr against the oracle-recorded golden.
const caseDir = "corpus/e2e/44_lfsr";
const tmp = mkdtempSync(join(tmpdir(), "vl-ffi-"));
// Manifest paths are relative to the manifest's dir; keep it simple with
// absolute source paths and an inject entry exercised too.
const manifest = [
  "top tb",
  `file ${join(ROOT, caseDir, "tb.v")}`,
  `inject data.hex ${join(ROOT, "corpus/e2e/53_readmem/data.hex")}`,
].join("\n");
writeFileSync(join(tmp, "manifest.vlm"), manifest);
const outPtr = lib.symbols.rcs_run(join(tmp, "manifest.vlm"));
const got = str(outPtr);
lib.symbols.rcs_free(outPtr);

const expected = read(join(caseDir, "expected.stdout"));
if (got === expected) {
  console.log(`PASS ffi-vs-golden\n${got}`);
} else {
  console.log(`FAIL ffi-vs-golden\n--- got ---\n${got}\n--- expected ---\n${expected}`);
  process.exit(1);
}
