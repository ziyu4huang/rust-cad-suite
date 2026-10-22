// Bun → native rverilator, subprocess form (zero native build; the CLI
// binary IS the interface). Process spawn tax ≈3ms/run — use the FFI form
// (`run.mjs`) when calling in a loop; use this when CLI flags suffice.
import { $ } from "bun";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const ROOT = join(import.meta.dir, "..", "..", "..");
const caseDir = "corpus/e2e/44_lfsr";

const proc = Bun.spawn({
  cmd: [
    join(ROOT, "target/debug/rverilator"),
    "--binary",
    join(ROOT, caseDir, "tb.v"),
    "--top-module",
    "tb",
  ],
  cwd: ROOT,
});
const got = await new Response(proc.stdout).text();
await proc.exited;

const expected = readFileSync(join(ROOT, caseDir, "expected.stdout"), "utf8");
if (got === expected) {
  console.log(`PASS spawn-vs-golden\n${got}`);
} else {
  console.log(`FAIL spawn-vs-golden\n--- got ---\n${got}\n--- expected ---\n${expected}`);
  process.exit(1);
}
