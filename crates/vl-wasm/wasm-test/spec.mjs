// Spec-verification layer wasm gate (ADR-0007 browser closed loop):
// the scripted I2C spec check runs wholly inside wasm — correct RTL clean,
// broken variants detected with named invariants, u64 times cross as
// BigInt, the run VCD parses, and two runs are byte-deterministic.
import init, { i2c_spec_check, i2c_rtl, vcd_parse } from "../pkg/vl_wasm.js";

await init();

let failed = 0;
const check = (cond, msg) => {
  if (!cond) {
    console.error(`FAIL ${msg}`);
    failed++;
  }
};

// --- correct variant: clean, cycles>100, BigInt boundary, vcd present ---
const ok = i2c_spec_check(0);
check(ok.violationCount === 0, `correct RTL must be clean (got ${ok.violationCount})\n${ok.render_text()}`);
check(ok.cyclesRun > 100n, `canonical script must exceed 100 cycles (got ${ok.cyclesRun})`);
check(typeof ok.cyclesRun === "bigint", "cyclesRun must cross as BigInt");
check(ok.violation_time !== undefined, "accessors present");
check(typeof ok.vcd === "string" && ok.vcd.length > 0, "run vcd must be captured");
const parsed = vcd_parse(ok.vcd);
check(parsed.varCount > 20, `i2c vcd must carry the port set (got ${parsed.varCount})`);

// determinism: two runs byte-identical (report + vcd)
const ok2 = i2c_spec_check(0);
check(ok2.vcd === ok.vcd, "vcd must be byte-deterministic");
check(ok2.render_text() === ok.render_text(), "report must be deterministic");

// --- lsb-first variant: wholesale + named detection ---
const lsb = i2c_spec_check(1);
check(lsb.violationCount >= 40, `lsb-first must diverge wholesale (got ${lsb.violationCount})`);
let sawLockstep = false;
let sawMsb = false;
for (let i = 0; i < lsb.violationCount; i++) {
  const k = lsb.violation_kind(i);
  if (k.startsWith("lockstep")) sawLockstep = true;
  if (k === "invariant `msb_first_bit_order`") sawMsb = true;
}
check(sawLockstep, "lsb-first must trip the lockstep leg");
check(sawMsb, "lsb-first must trip msb_first_bit_order by name");
check(typeof lsb.violation_time(0) === "bigint", "violation time must cross as BigInt");

// --- no-ack-release variant: STOP completes, ack_release named ---
const noack = i2c_spec_check(2);
let sawAck = false;
for (let i = 0; i < noack.violationCount; i++) {
  if (noack.violation_kind(i) === "invariant `ack_release`") sawAck = true;
}
check(sawAck, "no-ack-release must trip ack_release by name");

// --- error path: unknown variant is a hard error (house contract) ---
let threw = false;
try {
  i2c_spec_check(99);
} catch (e) {
  threw = true;
}
check(threw, "unknown variant must throw");

// --- rtl_for display: broken variants show the flipped line ---
check(i2c_rtl(0).includes("tx[7 - bitcnt[2:0]]"), "correct rtl keeps msb-first select");
check(i2c_rtl(1).includes("tx[bitcnt[2:0]]"), "lsb rtl shows the flipped select");
check(!i2c_rtl(2).includes("sda_oe <= 1'b0;\n              sda_o <= 1'b1;"), "noack rtl shows the flipped ack drive");

if (failed) {
  console.error(`== wasm spec gate: FAIL (${failed}) ==`);
  process.exit(1);
}
console.log("== wasm spec gate: PASS ==");
