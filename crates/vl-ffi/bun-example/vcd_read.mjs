// "Agent reads waveforms": run a corpus case through the FFI, dump the VCD
// (no TB $dumpvars — the `vcd <path>` manifest keyword forces it), then
// parse it with the SAME shared vl-vcd reader the CLI and wasm facades use,
// via rcs_vcd_parse. Line protocol (no serde):
//   ok timescale=<ts|-> vars=<n> tmax=<t> warns=<k>
//   var <idx> <width> <type> <id> <hier-name>
//   c <idx> <time> <value>
//   warn <code> <offset> <message>
import { dlopen, CString, suffix } from "bun:ffi";
import { readFileSync, mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

const ROOT = join(import.meta.dir, "..", "..", "..");
const read = (p) => readFileSync(join(ROOT, p), "utf8");

const dylib = join(ROOT, "target", "release", `libvl_ffi.${suffix}`);
const lib = dlopen(dylib, {
  rcs_run: { args: ["cstring"], returns: "ptr" },
  rcs_vcd_parse: { args: ["cstring"], returns: "ptr" },
  rcs_free: { args: ["ptr"], returns: "void" },
});
const take = (ptr) => {
  const s = new CString(ptr).toString();
  lib.symbols.rcs_free(ptr);
  return s;
};

// 1. Run the counter case and force the dump.
const caseDir = "corpus/e2e/51_vcd_counter";
const tmp = mkdtempSync(join(tmpdir(), "vl-ffi-vcd-"));
const vcdPath = join(tmp, "dump.vcd");
const manifest = [
  "top tb",
  `file ${join(ROOT, caseDir, "tb.v")}`,
  `vcd ${vcdPath}`,
].join("\n");
writeFileSync(join(tmp, "manifest.vlm"), manifest);
const stdout = take(lib.symbols.rcs_run(join(tmp, "manifest.vlm")));
if (stdout.startsWith("ERROR: ") || stdout !== read(join(caseDir, "expected.stdout"))) {
  console.log(`FAIL ffi-vcd-read stdout: ${stdout}`);
  process.exit(1);
}
console.log(`stdout matches golden: ${JSON.stringify(stdout)}`);

// 2. Parse the VCD through the shared reader.
const vcdText = readFileSync(vcdPath, "utf8");
const parsed = take(lib.symbols.rcs_vcd_parse(vcdText));
if (parsed.startsWith("ERROR: ")) {
  console.log(`FAIL ffi-vcd-read parse: ${parsed}`);
  process.exit(1);
}
const lines = parsed.trim().split("\n");
const header = lines[0];
const vars = lines.filter((l) => l.startsWith("var ")).map((l) => l.split(" "));
const changes = lines.filter((l) => l.startsWith("c ")).map((l) => l.split(" "));

// Header: timescale=1ps (the TB's precision), zero warnings.
const mh = header.match(/^ok timescale=(\S+) vars=(\d+) tmax=(\d+) warns=(\d+)$/);
if (!mh) {
  console.log(`FAIL bad header: ${header}`);
  process.exit(1);
}
const [, timescale, nvars, tmax, nwarns] = mh;
console.log(`header: timescale=${timescale} vars=${nvars} tmax=${tmax} warns=${nwarns}`);
if (timescale !== "1ps" || nwarns !== "0") {
  console.log("FAIL: unexpected timescale or warnings present");
  process.exit(1);
}

// q: width 8, values binary, final value 00000101 (q=5 — the TB PASS anchor),
// times strictly increasing.
// var line: var <idx> <width> <type> <id> <name> — split: [var,i,w,t,id,name]
const q = vars.find((v) => v[5] === "q");
if (!q) {
  console.log(`FAIL: q not in ${vars.map((v) => v[5]).join(", ")}`);
  process.exit(1);
}
if (q[2] !== "8") {
  console.log(`FAIL: q width ${q[2]}, want 8`);
  process.exit(1);
}
const qChanges = changes.filter((c) => c[1] === q[1]);
const times = qChanges.map((c) => Number(c[2]));
for (let i = 1; i < times.length; i++) {
  if (times[i] <= times[i - 1]) {
    console.log("FAIL: non-monotonic times");
    process.exit(1);
  }
}
const lastVal = qChanges[qChanges.length - 1][3];
console.log(`q: ${qChanges.length} changes, last t=${times[times.length - 1]} v=${lastVal}`);
if (lastVal !== "00000101") {
  console.log(`FAIL: final q ${lastVal}, want 00000101`);
  process.exit(1);
}

// 3. Error path: malformed VCD surfaces VL-E101 through the prefix contract.
const err = take(lib.symbols.rcs_vcd_parse("$enddefinitions $end\n#abc\n"));
if (!err.startsWith("ERROR: VL-E101")) {
  console.log(`FAIL: expected ERROR: VL-E101, got: ${err}`);
  process.exit(1);
}
console.log(`error path: ${err.trim()}`);
console.log("PASS ffi-vcd-read");
