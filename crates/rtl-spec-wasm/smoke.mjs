// S4 雙運行時冒煙(bun+node)+ wasm↔native byte parity(R1 證據面)。
// 用法:bun crates/rtl-spec-wasm/smoke.mjs   (或 node crates/rtl-spec-wasm/smoke.mjs)
// 斷言:
//  ①顯式位元組 init(繞開 fetch(file://) 運行時差;§5 策略)
//  ②9 導出全部回 JSON 且語義面在場(rules/gaps/analyze/tb_plan;parse 錯誤內嵌)
//  ③wasm 輸出 vs native(cargo run --example emit)同輸入 byte 相等
//    ——analyze/rules/gaps × sv/rtlil 六對拍面 + tb_plan 三路(C-S2:
//    sv 正樣/TB1 負樣/.il 輸入恒 TB1)
// 退出碼:0=PASS;1=斷言失敗;2=環境缺(pkg 缺;閘腳本守衛重構建)。

import { readFileSync, existsSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..');
const runtimeName = process.env.SMOKE_RUNTIME ?? 'unknown';

if (!existsSync(join(here, 'pkg', 'rtl_spec_wasm_bg.wasm'))) {
  console.log('SMOKE SKIP-FAIL: pkg 缺(閘應先重構建)');
  process.exit(2);
}

const { default: init } = await import(join(here, 'pkg', 'rtl_spec_wasm.js'));

// ① 顯式位元組 init(§5 策略)。
const wasmBytes = readFileSync(join(here, 'pkg', 'rtl_spec_wasm_bg.wasm'));
await init({ module_or_path: new Uint8Array(wasmBytes) });

const svFixture = join(here, '..', 'rtl-spec', 'tests', 'fixtures', 'rulebook_sample.v');
const latchFixture = join(here, '..', 'rtl-spec', 'tests', 'fixtures', 'gaps_positive.v');
const ilFixture = join(here, '..', 'rtl-spec', 'tests', 'fixtures', 'sample.il');
const tbSvFixture = join(repoRoot, 'plugin', 'rtl2spec', 'fixtures', 'fsm_counter.v');
const tbNegSrc = 'module onlyin(input wire a);\nendmodule\n'; // TB1⑥:零輸出
const svSrc = readFileSync(svFixture, 'utf8');
const latchSrc = readFileSync(latchFixture, 'utf8');
const tbSvSrc = readFileSync(tbSvFixture, 'utf8');
const ilSrc = readFileSync(ilFixture, 'utf8');

let failures = 0;
function check(name, cond, detail = '') {
  if (cond) {
    console.log(`PASS ${name}`);
  } else {
    failures++;
    console.log(`FAIL ${name}${detail ? ` — ${detail}` : ''}`);
  }
}

const api = await import(join(here, 'pkg', 'rtl_spec_wasm.js'));
check('rtl_spec_version', /^\d+\.\d+\.\d+/.test(api.rtl_spec_version()), api.rtl_spec_version());

const analyzeSv = api.rtl_analyze_sv('rulebook_sample.v', svSrc);
check('analyze_sv json', analyzeSv.startsWith('{"source":'), analyzeSv.slice(0, 60));

const rulesSv = api.rtl_extract_rules_sv('rulebook_sample.v', svSrc, 128);
check('extract_sv rules>=9', rulesSv.includes('"rulebook_sample.R009"'), rulesSv.slice(0, 60));

const gapsSv = api.rtl_scan_gaps_sv('gaps_positive.v', latchSrc);
check('scan_sv finds latch', gapsSv.includes('"latch"'), gapsSv.slice(0, 80));

const analyzeIl = api.rtl_analyze_rtlil('sample.il', ilSrc);
check('analyze_rtlil json', analyzeIl.includes('il_counter'), analyzeIl.slice(0, 60));
const rulesIl = api.rtl_extract_rules_rtlil('sample.il', ilSrc);
check('extract_rtlil json', rulesIl.startsWith('{"source":"sample.il"'), rulesIl.slice(0, 60));
const gapsIl = api.rtl_scan_gaps_rtlil('sample.il', ilSrc);
check('scan_rtlil json', gapsIl.includes('latch'), gapsIl.slice(0, 80));

// tb_plan sv 正樣(ok:true 幀含 plan+template_v)。
const tbSv = api.rtl_tb_plan_sv('fsm_counter.v', tbSvSrc, '', 0, 0, 0);
check('tb_plan_sv ok frame', tbSv.startsWith('{"ok":true,') && tbSv.includes('"template_v":'), tbSv.slice(0, 80));

// 錯誤面:未宣告 wire 引用(ry-ir 語義驗證錯)→ parse_errors 進結構化
// 輸出({"error"} 面留給 face 級失敗;此處釘 parse 錯誤內嵌收斂行為)。
const badIl = api.rtl_extract_rules_rtlil('bad.il', 'module \\bad\n  wire width 1 input 0 \\x\n  connect \\y \\x\nend\n');
check('bad rtlil → parse_errors embedded', badIl.includes('"parse_errors":["AN-'), badIl.slice(0, 120));

// ③ wasm ↔ native byte parity(native=cargo run --example emit)。
function native(lang, mode, name, file, max) {
  const args = ['run', '-q', '-p', 'rtl-spec', '--example', 'emit', '--', lang, mode, name, file];
  if (max !== undefined) args.push(String(max));
  return execFileSync('cargo', args, { cwd: repoRoot, encoding: 'utf8' }).trimEnd();
}

// TB1 負樣 native 腿:emit 只收檔——負樣源碼寫入 os.tmpdir() 臨時檔
// (寫面=os tmpdir,語料面零觸碰),native/wasm 兩腿同吃該檔內容。
import { tmpdir } from 'node:os';
function withTempSource(name, source, fn) {
  const dir = mkdtempSync(join(tmpdir(), 'rtl2spec-smoke-'));
  const f = join(dir, name);
  writeFileSync(f, source);
  try {
    return fn(f);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

const parityCases = [
  ['analyze_sv', () => api.rtl_analyze_sv('rulebook_sample.v', svSrc), () => native('sv', 'analyze', 'rulebook_sample.v', svFixture)],
  ['rules_sv', () => api.rtl_extract_rules_sv('rulebook_sample.v', svSrc, 128), () => native('sv', 'rules', 'rulebook_sample.v', svFixture, 128)],
  ['gaps_sv', () => api.rtl_scan_gaps_sv('gaps_positive.v', latchSrc), () => native('sv', 'gaps', 'gaps_positive.v', latchFixture)],
  ['analyze_rtlil', () => api.rtl_analyze_rtlil('sample.il', ilSrc), () => native('rtlil', 'analyze', 'sample.il', ilFixture)],
  ['rules_rtlil', () => api.rtl_extract_rules_rtlil('sample.il', ilSrc), () => native('rtlil', 'rules', 'sample.il', ilFixture)],
  ['gaps_rtlil', () => api.rtl_scan_gaps_rtlil('sample.il', ilSrc), () => native('rtlil', 'gaps', 'sample.il', ilFixture)],
  // tb_plan 三路 parity(C-S2;module/seed/cycles/flush=缺省形)。
  ['tbplan_sv', () => api.rtl_tb_plan_sv('fsm_counter.v', tbSvSrc, '', 0, 0, 0), () => native('sv', 'tbplan', 'fsm_counter.v', tbSvFixture)],
  ['tbplan_tb1_neg',
    () => api.rtl_tb_plan_sv('neg.v', tbNegSrc, '', 0, 0, 0),
    () => withTempSource('neg.v', tbNegSrc, (f) => native('sv', 'tbplan', 'neg.v', f))],
  ['tbplan_rtlil', () => api.rtl_tb_plan_rtlil('sample.il', ilSrc, '', 0, 0, 0), () => native('rtlil', 'tbplan', 'sample.il', ilFixture)],
];
for (const [name, wasmFn, nativeFn] of parityCases) {
  let wasmOut, nat;
  try {
    wasmOut = wasmFn();
  } catch (e) {
    check(`parity ${name}`, false, `wasm leg threw: ${e.message}`);
    continue;
  }
  try {
    nat = nativeFn();
  } catch (e) {
    check(`parity ${name}`, false, `native leg failed: ${String(e.message).slice(0, 120)}`);
    continue;
  }
  check(`parity ${name}`, wasmOut === nat, `bytes differ (wasm ${wasmOut.length} vs native ${nat.length})`);
}

if (failures > 0) {
  console.log(`SMOKE FAIL [${runtimeName}]: ${failures} failure(s)`);
  process.exit(1);
}
console.log(`SMOKE PASS [${runtimeName}]`);
