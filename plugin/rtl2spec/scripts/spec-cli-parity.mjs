// spec-cli-parity.mjs — CLI↔wasm byte-equal 比對(run-spec-cli-parity.sh ④;D-S2)。
// 裝載形沿 golden-check.mjs:37「裝載 dist 同一 wasm(顯式位元組 init)」。
// 名契約:name=CLI 入參路徑逐字(兩側同字串→evidence.file 同形→byte-equal)。
// 判據:每腿 got==expected(utf8 逐位元組)且 CLI exit==期望(全 0,除
// tb-rtlil 恆 TB1 幀=1);任一不符=exit 1。
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const parityDir = process.argv[2];
if (!parityDir) {
  console.error('usage: spec-cli-parity.mjs <parityDir>');
  process.exit(2);
}
const pluginRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const repoRoot = dirname(dirname(pluginRoot));

const wasmDir = join(repoRoot, 'plugin', 'rtl2spec', 'dist', 'wasm');
const glue = await import(join(wasmDir, 'rtl_spec_wasm.js'));
await glue.default({
  module_or_path: new Uint8Array(readFileSync(join(wasmDir, 'rtl_spec_wasm_bg.wasm'))),
});

// [leg, file(name 逐字), wasm 導出, 導出尾參]
const legs = [
  ['analyze-sv', 'crates/rtl-spec/tests/fixtures/rulebook_sample.v', 'rtl_analyze_sv', []],
  ['analyze-rtlil', 'crates/rtl-spec/tests/fixtures/sample.il', 'rtl_analyze_rtlil', []],
  ['extract-sv-def', 'crates/rtl-spec/tests/fixtures/rulebook_sample.v', 'rtl_extract_rules_sv', [0]],
  ['extract-sv-128', 'crates/rtl-spec/tests/fixtures/rulebook_sample.v', 'rtl_extract_rules_sv', [128]],
  ['extract-rtlil', 'crates/rtl-spec/tests/fixtures/sample.il', 'rtl_extract_rules_rtlil', []],
  ['extract-dspec', 'plugin/rtl2spec/fixtures/d-spec/counter8.spec.md', 'rtl_extract_rules_sv', [0]],
  ['extract-corpus', 'corpus/e2e/01_arith_widths/tb.v', 'rtl_extract_rules_sv', [0]],
  ['gaps-sv', 'crates/rtl-spec/tests/fixtures/rulebook_sample.v', 'rtl_scan_gaps_sv', []],
  ['gaps-rtlil', 'crates/rtl-spec/tests/fixtures/sample.il', 'rtl_scan_gaps_rtlil', []],
  ['tb-sv', 'crates/rtl-spec/tests/fixtures/rulebook_sample.v', 'rtl_tb_plan_sv', ['rulebook_sample', 0, 0, 0]],
  ['tb-rtlil', 'crates/rtl-spec/tests/fixtures/sample.il', 'rtl_tb_plan_rtlil', ['m', 0, 0, 0]],
  // spec 腿(D-S3):parse/lint(name 沿導出形;核心解析零名消費,byte-equal 由同字串源保證)
  ['spec-parse-counter8', 'plugin/rtl2spec/fixtures/d-spec/counter8.spec.md', 'rtl_spec_spec_parse', []],
  ['spec-parse-accpipe', 'plugin/rtl2spec/fixtures/d-spec/acc_pipe.spec.md', 'rtl_spec_spec_parse', []],
  ['spec-parse-i2c', 'plugin/rtl2spec/fixtures/d-spec/i2c_master.spec.md', 'rtl_spec_spec_parse', []],
  ['spec-lint-counter8', 'plugin/rtl2spec/fixtures/d-spec/counter8.spec.md', 'rtl_spec_spec_lint', []],
  ['spec-lint-accpipe', 'plugin/rtl2spec/fixtures/d-spec/acc_pipe.spec.md', 'rtl_spec_spec_lint', []],
  ['spec-lint-i2c', 'plugin/rtl2spec/fixtures/d-spec/i2c_master.spec.md', 'rtl_spec_spec_lint', []],
];

function firstDiff(a, b) {
  const n = Math.min(a.length, b.length);
  for (let i = 0; i < n; i++) {
    if (a[i] !== b[i]) return i;
  }
  return n;
}

let fail = 0;
for (const [leg, file, exportName, extra] of legs) {
  const src = readFileSync(join(repoRoot, file), 'utf8');
  const expected = glue[exportName](file, src, ...extra);
  let got;
  try {
    got = readFileSync(join(parityDir, `cli-${leg}.json`), 'utf8');
  } catch {
    console.log(`FAIL ${leg}: cli 產物缺 cli-${leg}.json`);
    fail++;
    continue;
  }
  const exit = parseInt(readFileSync(join(parityDir, `cli-${leg}.exit`), 'utf8').trim(), 10);
  const wantExit = leg === 'tb-rtlil' ? 1 : 0;
  if (got !== expected) {
    const d = firstDiff(got, expected);
    console.log(
      `FAIL ${leg}: byte diff @${d} (got ${got.length}B want ${expected.length}B; got=${JSON.stringify(got.slice(Math.max(0, d - 20), d + 20))})`,
    );
    fail++;
  } else if (exit !== wantExit) {
    console.log(`FAIL ${leg}: exit ${exit} != ${wantExit}`);
    fail++;
  } else {
    console.log(`PASS ${leg} (${expected.length}B, exit ${exit})`);
  }
}
console.log(`== spec-cli-parity: ${legs.length - fail}/${legs.length} byte-equal ==`);
process.exit(fail ? 1 : 0);
