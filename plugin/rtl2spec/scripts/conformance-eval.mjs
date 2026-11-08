// conformance-eval.mjs — conformance 閉環評測閘(§19.5;D-C7/D-C8;C-S3 確定性床)。
//
// 默認模式(寫只落 plugin/rtl2spec/build/conformance/):逐設計 tblint(Q6/Q9)→[T 腿]進程內 3 次重生成
// byte 相等且==tb.v→編譯(Q1)→原版 2 跑(Q2..Q5)→verilator(Q7;不在場 SKIP)→變異矩陣(P-rule+
// P-unruled+控制組)→結果比對 record.json(Q8;雙向 DRIFT)→輔助腿計分(不影響 exit)。證據頁重生成到
// build/ 並與 plugin/rtl2spec/conformance-report.md byte 比對(陳舊/手改=紅)。
// `--record --slice <C-Sn> [design…]`:另守衛 yosys;鐵則(P-rule ≥1 killed)不過=拒錄 exit 1;先寫錄前
// 快照 build/conformance/pre-record/<slice>/<design>/,再寫 fixtures/conformance/<design>/{tb.v,
// expected.txt,record.json}+conformance-report.md,並生成 build/conformance/record-diff-<slice>.md。
// 等價裁決腿(協議 §7;僅 --record,默認模式零 yosys 依賴):強形命令+輸出位數空真守衛+eqverdict 三值;gold-vs-gold/空真探針/
// 地面真值/killed 全母體/存量重驗任一不過=拒錄;WAIVE↔record 交叉驗證併入 Q9;默認模式=eqMeta schema 與 counts 一致性(eq-verify)。
// TB-SHA-ALARM(CONFORMANCE-PROTOCOL §5):入庫 tb.v 禁手改——record.tb.sha256/磁碟 tb.v/本輪主腿 TB 三方矛盾
// =REJECTED(默認 exit 1;錄製=拒錄 exit 1),換檔唯一路徑=--record --promote。
// exit:0=全部 ADMITTED 且快照全等;1=任一判據敗/DRIFT/selfcheck 敗/證據頁陳舊/TB-SHA-ALARM;2=守衛/用法。
//
// oracle 紀律(§19.1):度量的是行為復原力(原版 RTL 在 iverilog 13.0 下的可觀測行為),不是意圖正確性。

import { createHash } from 'node:crypto';
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  comparable,
  declaredScenes,
  diffScenes,
  iverilogRun,
  normalizeIverilog,
  normalizeVerilator,
  parseProtocol,
  run,
  sceneBlocks,
  toolVersion,
  verilatorRun,
} from './lib/tbproto.mjs';
import { lintTb, r1SameFile, tbR1x, instantiation, waiveEntriesOf, WAIVE_LABELS } from './conformance/tblint.mjs';
import {
  EQ_COMMAND_TEMPLATE,
  EQ_EMPTY_NAMES_TEMPLATE,
  EQ_EMPTY_PROBE_TEMPLATE,
  EQ_GROUND_TRUTH,
  EQ_PORTS_TEMPLATE,
  EQ_PROVENANCE,
  eqSchemaFindings,
  eqVerdictOf,
  extractTopOnly,
  fillTemplate,
  isKill,
  portBits,
  publicNames,
  tagCounts,
  waiveCrossFindings,
  waiveRows,
} from './conformance/equiv.mjs';
import { latestTemplate, parseTemplates, waiveLabelsOf } from './conformance/proto-template.mjs';
import { axisTmax, decideModality, divergingSignals, isPngBytes, stripTitles, validateDiag } from './conformance/frames.mjs';
import { applyMutant, anchorStats, mutateAssignLine, OPCAT_VERSION, planMutations, PRIORITY } from './conformance/mutate.mjs';
import { leaveOneOut } from './conformance/diagnose.mjs';
import { defLint } from './conformance/def_lint.mjs';

const pluginRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const repoRoot = dirname(dirname(pluginRoot));
// --fixtures-root <dir>(收尾弧 S3 staging 預錄,FINISH-PLAN §5.1):候選 case.json 落 staging 副本跑完整 Q1..Q9+Q8 鐵鏈,
// ADMITTED 判定=解析 staging verdict.json(不看 exit)。staging 根必須在 plugin/rtl2spec/build/ 內;outRoot 與證據頁一律落
// build/conformance-staging/out/(主證據頁零觸碰);EXPECTED_DESIGNS 與 F3 斷言只對主根生效。
const mainFixRoot = join(pluginRoot, 'fixtures', 'conformance');
const stagingArgAt = process.argv.indexOf('--fixtures-root');
const stagingRoot = stagingArgAt >= 0 ? resolve(process.argv[stagingArgAt + 1] ?? '') : null;
if (stagingRoot !== null && !(stagingRoot.startsWith(join(pluginRoot, 'build') + sep) && existsSync(stagingRoot))) {
  console.log(`FAIL usage: --fixtures-root 必須是 plugin/rtl2spec/build/ 內已存在的目錄(寫面約束),得 ${process.argv[stagingArgAt + 1] ?? '<缺>'}`);
  process.exit(2);
}
const staging = stagingRoot !== null;
const fixRoot = staging ? stagingRoot : mainFixRoot;
const lintRoot = join(pluginRoot, 'fixtures', 'conformance-lint');
const outRoot = staging ? join(pluginRoot, 'build', 'conformance-staging', 'out') : join(pluginRoot, 'build', 'conformance');
const reportPath = staging ? join(outRoot, 'staging-report.md') : join(pluginRoot, 'conformance-report.md');
// 設計目錄數斷言(防清單短收——eval-all L2=0 假綠教訓);只對主根。6=§19.7 採樣集;收尾弧 S3 擴面逐批同輪上調:
// 批 1(C-S8-1)+10=P0 已釘 golden 且 staging ADMITTED;批 2(C-S8-2)+18=未釘 12(裁定全收,golden 同輪擴檔 37→49)+P1 已釘 6
// (S3 收編 28=staging ADMITTED 全集;名單=CONFORMANCE-PROTOCOL §9)。6+10+18=34。
const EXPECTED_DESIGNS = 34;
// 主根設計的規則書必須是 golden 釘選快照(收尾弧 S3 裁定:收編設計全部入釘選集,spec 漂移由 golden 回歸閘先紅)。
const pinnedSet = new Set(
  readFileSync(join(pluginRoot, 'pinned-files.list'), 'utf8')
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith('#')),
);
/// 主根設計的規則書必須是 golden 釘選快照;staging 不斷言(收尾弧 S3;判定旗標入 admitted)。
const specPinnedOk = ({ staging: st, specPath, src, pinned }) => st || (specPath.startsWith('plugin/rtl2spec/golden/') && pinned.has(src));
const SIM_TIMEOUT_MS = 30000; // Q3 牆鐘看門狗(實測耗時記入 record,須 ≤ 1/30)

const argv = process.argv.slice(2);
const recordMode = argv.includes('--record');
let slice = null;
let tryDesign = null;
let tryTb = null;
const promote = new Map(); // --record 專用:--promote <design>=<候選 tb 路徑>(D 腿入庫;凍結 LLM 產物)
const designArgs = [];
for (let k = 0; k < argv.length; k++) {
  if (argv[k] === '--record') continue;
  if (argv[k] === '--fixtures-root') {
    k++;
    continue;
  }
  if (argv[k] === '--slice') {
    slice = argv[++k] ?? null;
    continue;
  }
  if (argv[k] === '--try') {
    tryDesign = argv[++k] ?? null;
    tryTb = argv[++k] ?? null;
    continue;
  }
  if (argv[k] === '--promote') {
    const v = argv[++k] ?? '';
    const eq = v.indexOf('=');
    if (eq <= 0) {
      console.log('FAIL usage: --promote <design>=<候選 tb.v 路徑>');
      process.exit(2);
    }
    promote.set(v.slice(0, eq), v.slice(eq + 1));
    continue;
  }
  designArgs.push(argv[k]);
}
if (recordMode && !/^(C|D)-S[0-9]+(-[a-z0-9]+)?$/.test(slice ?? '')) {
  console.log('FAIL usage: --record 必帶 --slice <C-Sn|D-Sn>[-<批次>](錄前快照與 record-diff 的切片標籤;§19.4.4)');
  process.exit(2);
}
if (promote.size && !recordMode) {
  console.log('FAIL usage: --promote 只能與 --record --slice 並用');
  process.exit(2);
}
if ((tryDesign === null) !== (tryTb === null) || (tryDesign !== null && recordMode)) {
  console.log('FAIL usage: --try <design> <tb.v 路徑>(候選 D 腿評測,寫只落 build/conformance/try/;不可與 --record 並用)');
  process.exit(2);
}

const sha256 = (s) => createHash('sha256').update(s).digest('hex');
const rel = (p) => relative(repoRoot, p);

// ---------- 裝載 dist wasm(閘驗出貨物,D8) ----------
const wasmDir = join(pluginRoot, 'dist', 'wasm');
const glue = await import(join(wasmDir, 'rtl_spec_wasm.js'));
const wasmBytes = readFileSync(join(wasmDir, 'rtl_spec_wasm_bg.wasm'));
await glue.default({ module_or_path: new Uint8Array(wasmBytes) });
const wasmSha = sha256(wasmBytes);

const tools = {
  iverilog: toolVersion('iverilog', ['-V']),
  verilator: toolVersion('verilator', ['--version']),
  yosys: null,
};
const yosysBin = (() => {
  // RTL2SPEC_YOSYS=none:視為缺席(驗「默認模式零 yosys 依賴」與 --record 守衛 exit 2 的探針形)。
  const cands = process.env.RTL2SPEC_YOSYS === 'none' ? [] : [process.env.RTL2SPEC_YOSYS, join(repoRoot, 'vendor-reference/suite-dl/oss-cad-suite/bin/yosys'), 'yosys'].filter(Boolean);
  for (const c of cands) {
    const v = toolVersion(c, ['-V']);
    if (v) {
      tools.yosys = v;
      return c;
    }
  }
  return null;
})();
if (!tools.iverilog) {
  console.log('FAIL guard: iverilog 不在場');
  process.exit(2);
}
if (recordMode && !yosysBin) {
  console.log('FAIL guard: --record 需 yosys(RTL2SPEC_YOSYS→vendored→PATH 皆缺)');
  process.exit(2);
}

let failures = 0;
const ck = (ok, name, detail = '') => {
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
  if (!ok) failures++;
  return ok;
};

// ---------- TB-SHA-ALARM(入庫 tb.v 禁手改,換檔必走 --promote;CONFORMANCE-PROTOCOL §5) ----------
/// 三方=入庫/錄前 record.tb.sha256(storedSha)、磁碟 tb.v(diskSha)、本輪主腿 TB(candSha)。回傳矛盾
/// 描述,null=一致。①record 在場 ⇒ 磁碟必==record(--promote 不洗白手改:先還原再換檔);②錄製模式
/// directed 腿無 --promote 且無錄前 record ⇒ tb.v 未經 --promote 入庫;③directed 腿無 --promote ⇒ 主腿
/// TB 必==磁碟(閘永不重生成 D 腿)。T 腿錄製時主腿=模板重生成,與磁碟差異合法,不算矛盾。
function tbShaAlarm({ storedSha, diskSha, candSha, leg, recording, promoted }) {
  const s = (x) => (x ? x.slice(0, 12) : '<缺>');
  if (storedSha && storedSha !== diskSha) {
    return `磁碟 tb.v sha=${s(diskSha)}≠record.tb.sha256=${s(storedSha)}(手改或 record 失配;入庫 tb.v 禁手改,先還原再經 --promote 換檔)`;
  }
  if (leg !== 'directed' || promoted) return null;
  if (recording && !storedSha) return `directed 腿 tb.v(sha=${s(diskSha)})無錄前 record 且未帶 --promote(入庫路徑=--record --promote)`;
  if (candSha !== diskSha) return `directed 腿主腿 TB sha=${s(candSha)}≠磁碟 tb.v sha=${s(diskSha)}(閘永不重生成 D 腿)`;
  return null;
}

// ---------- selfcheck(每次必跑;判別力自證) ----------
function selfcheck() {
  // 正規化敵意向量(R1-1):iverilog 兩種時間單位橫幅必丟、含 $finish 字樣的協議 kv 行必留。
  const ivRaw = 'SCENE1 a\n  v=1\n  note_$finish=1\ntb.v:9: $finish called at 120 (1s)\ntb.v:9: $finish called at 120000 (1ps)\nVCD info: dumpfile x opened\n  v=-3\nDONE\n';
  const ivN = normalizeIverilog(ivRaw);
  ck(
    ivN === 'SCENE1 a\n  v=1\n  note_$finish=1\n  v=-3\nDONE\n',
    'selfcheck 正規化 iverilog 腿(橫幅兩形丟/含 $finish 字樣 kv 留/  v=-3 留)',
    JSON.stringify(ivN),
  );
  // verilator 腿:corpus_lint.rs normalization_contract 原始字串逐位元組復現其期望(JS 移植與 Rust 契約 parity)。
  const vlRust = '- S i m u l a t i o n   R e p o r t: Verilator 5.050\n-Info: tb.v:9: $dumpvar ignored, as Verilated without --trace\n-7div2=-3 -7mod2=-1\n- /x/tb.v:24: Verilog $finish\nPASS\n- Verilator: $finish at 76ns; walltime 0.000 s\n';
  ck(normalizeVerilator(vlRust) === '-7div2=-3 -7mod2=-1\nPASS\n', 'selfcheck 正規化 verilator 腿==Rust 契約(corpus_lint normalization_contract)');
  const vl4 = 'SCENE1 a\n  v=-3\nDONE\n- tb.v:113: Verilog $finish\n- S i m u l a t i o n   R e p o r t: Verilator 5.052 2026-09-05\n- Verilator: $finish at 1ns; walltime 0.003 s; speed 1.0 ms/s\n- Verilator: cpu 0.003 s on 1 threads; allocated 9 MB\n';
  ck(normalizeVerilator(vl4) === 'SCENE1 a\n  v=-3\nDONE\n', 'selfcheck verilator 四行報告必丟、  v=-3 協議行留');
  // X 掃描敵意向量:場景名/key 含字母 x 不誤報;值含 x 必報。
  const xs = parseProtocol('SCENE1 exh\n  xu=0\n  x_max=ff\nSCENE2 xz_scene\n  q=1x\nDONE\n');
  ck(xs.xz.length === 1 && xs.xz[0].includes('q=1x'), 'selfcheck X 掃描(名/key 含 x 不誤報,值 x 必報)', JSON.stringify(xs.xz));
  // 同檔 R1 三向量 parity(corpus_lint.rs race_lint_catches_the_documented_pattern)。
  const bad = 'module tb;\nreg clk=0, ld=0; reg [7:0] q;\nalways @(posedge clk) if (ld) q <= 1;\ninitial begin\n  @(posedge clk); ld = 1;\n  @(posedge clk);\nend\nendmodule\n';
  const good = 'module tb;\nreg clk=0, ld=0; reg [7:0] q;\nalways @(posedge clk) if (ld) q <= 1;\ninitial begin\n  @(posedge clk); #1; ld = 1;\n  @(posedge clk);\nend\nendmodule\n';
  const nba = 'module tb;\nreg clk=0, ld=0;\nalways @(posedge clk) if (ld) $display(1);\ninitial begin\n  @(posedge clk); ld <= 1;\nend\nendmodule\n';
  const r1 = [r1SameFile(bad).length, r1SameFile(good).length, r1SameFile(nba).length];
  ck(JSON.stringify(r1) === '[1,0,0]', 'selfcheck 同檔 R1 三向量 parity(1/0/0)', JSON.stringify(r1));
  // TB-R1x 跨檔五向量(E 取自 dist wasm tb_plan.sens_edges)。
  const planOf = (f) => JSON.parse(glue.rtl_tb_plan_sv(f, readFileSync(join(lintRoot, f), 'utf8'), '', 0, 0, 0)).plan;
  const pos = planOf('dut_pos.v');
  const neg = planOf('dut_neg.v');
  const count = (tb, p) => {
    const src = readFileSync(join(lintRoot, tb), 'utf8');
    return tbR1x(src, p.sens_edges, p.ports.filter((x) => x.dir === 'input').map((x) => x.name), instantiation(src, p.module).conns).length;
  };
  const vec = [
    ['tb_v1_pos_same_edge.v', pos, 1],
    ['tb_v2_opposite_edge.v', pos, 0],
    ['tb_v3_neg_same_edge.v', neg, 1],
    ['tb_v4_delay_same_line.v', pos, 0],
    ['tb_v5_delay_next_line.v', pos, 0],
  ];
  const got = vec.map(([tb, p]) => count(tb, p));
  ck(JSON.stringify(got) === JSON.stringify(vec.map((v) => v[2])), 'selfcheck TB-R1x 跨檔五向量(1/0/1/0/0;第五向量=文檔語義,D-c3)', JSON.stringify(got));
  // 判別力自證:正向量期望誤配負向量必紅。
  ck(count('tb_v2_opposite_edge.v', pos) !== 1 && count('tb_v5_delay_next_line.v', pos) !== 1, 'selfcheck TB-R1x 正向量期望誤配負向量必紅');
  // 變異手術:count==1 與非阻塞 `<=` 不誤動。
  const twoPlus = mutateAssignLine('  assign y = a + b + c;', 'OP-ARITH');
  ck(twoPlus.status === 'ambiguous', 'selfcheck 手術 count==1(行內兩個 + → ambiguous 不替換)', JSON.stringify(twoPlus));
  const nbaCmp = mutateAssignLine('    q <= (a >= b);', 'OP-CMP');
  ck(nbaCmp.status === 'ok' && nbaCmp.after === '    q <= (a > b);', 'selfcheck 手術不誤動非阻塞 `<=`(只動 RHS 的 >=)', JSON.stringify(nbaCmp));
  // 「正樣期望誤配給某 killed 變異的輸出必紅」:比對器對任一行差必報場景。
  const e = 'SCENE1 a\n  v=1\nDONE\n';
  ck(diffScenes(e, 'SCENE1 a\n  v=2\nDONE\n').length === 1 && diffScenes(e, e).length === 0, 'selfcheck 比對器判別力(一行差必報/同文零報)');
  // WAIVE 標籤集(收尾 S0-C;一正一負):unadjudicated 合法;標籤集外/一行多規則必紅,規則另有 COVERS 也紅。
  const wv = (ln) => lintTb(`// SCENES: 1\n// COVERS: 1: a.R001,a.R002\n${ln}\n`, { plan: { module: 'm', ports: [], sens_edges: [] }, ruleIds: ['a.R001', 'a.R002'], leg: 'directed' });
  const wOk = wv('// WAIVE: a.R002 unadjudicated');
  ck(wOk.q9.length === 0 && wOk.waived === 1, 'selfcheck WAIVE 標籤 unadjudicated 合法(正)', JSON.stringify(wOk.q9));
  const wBad = [wv('// WAIVE: a.R002 bogus'), wv('// WAIVE: a.R001 a.R002 unobservable')];
  ck(wBad.every((w) => w.waived === 0 && w.q9.some((x) => x.startsWith('WAIVE 行文法外'))), 'selfcheck WAIVE 標籤集外/一行多規則必紅(負;規則另有 COVERS 不得靜默過)', JSON.stringify(wBad.map((w) => w.q9)));
  // 協議模板硬同步(收尾 S0-B;協議 §6):真檔最新版=v2、v1 預登記錨不變、枚舉==WAIVE_LABELS;篡改/缺錨必紅。
  const protoText = readFileSync(join(pluginRoot, 'CONFORMANCE-PROTOCOL.md'), 'utf8');
  const threw = (t) => {
    try {
      parseTemplates(t);
      return null;
    } catch (err) {
      return err.message;
    }
  };
  const protoErr = threw(protoText);
  if (ck(protoErr === null, 'selfcheck 協議模板硬同步(圍欄完整+版本連續+每版內容錨相等)', protoErr ?? '')) {
    const all = parseTemplates(protoText);
    const tpl = latestTemplate(protoText);
    ck(tpl.version === 2 && all[0].sha256 === 'cc28f0c73dc4ef3afc1838b839d44903d7dc9ee9b9f15b5ebd3aceaca532a941', 'selfcheck 最新版=v2、v1 預登記錨 cc28f0c7… 不變', `v${tpl.version} ${tpl.sha256.slice(0, 16)}`);
    ck(JSON.stringify([...waiveLabelsOf(tpl.lines)].sort()) === JSON.stringify([...WAIVE_LABELS].sort()), 'selfcheck 最新版 WAIVE 枚舉==tblint WAIVE_LABELS(逐集合)', waiveLabelsOf(tpl.lines).join('|'));
    ck(!waiveLabelsOf(all[0].lines).includes('unadjudicated'), 'selfcheck v1 枚舉保持舊集(append-only,舊版未被改寫)');
    const inV2 = protoText.lastIndexOf('unadjudicated|tb-internal');
    const inV1 = protoText.indexOf('<x-valued|unobservable|tb-internal>');
    const tamper = [
      ['v2 塊改一字', protoText.slice(0, inV2) + 'Unadjudicated|tb-internal' + protoText.slice(inV2 + 'unadjudicated|tb-internal'.length)],
      ['v1 塊改一字', protoText.slice(0, inV1) + '<x-valued|unobservable|tb-Internal>' + protoText.slice(inV1 + '<x-valued|unobservable|tb-internal>'.length)],
      ['v2 內容錨缺', protoText.split('\n').filter((l) => !l.startsWith('<!-- template-sha v2 ')).join('\n')],
    ];
    const red = tamper.map(([name, t]) => [name, threw(t)]);
    ck(inV2 >= 0 && inV1 >= 0 && red.every(([, m]) => m !== null), 'selfcheck 協議模板負向量必紅(v2 塊改一字/v1 塊改一字/v2 錨缺)', JSON.stringify(red));
    // dleg-prompt 走同一入口(取最高版本,無版本硬編碼;CLI 正負探針實跑記錄見收尾 §20)。
    const dlegSrc = readFileSync(join(pluginRoot, 'scripts', 'conformance', 'dleg-prompt.mjs'), 'utf8');
    ck(/\blatestTemplate\(/.test(dlegSrc) && !/template-block v[0-9]/.test(dlegSrc), 'selfcheck dleg-prompt 經 latestTemplate 取模板(無圍欄版本硬編碼)');
  }
  // TB-SHA-ALARM 向量(收尾 S0-②;純函數面,端到端驗收=手改紅/還原綠/正常 promote 不誤報)。
  const A = 'a'.repeat(64);
  const B = 'b'.repeat(64);
  const sv = [
    ['默認:磁碟==record', { storedSha: A, diskSha: A, candSha: A, leg: 'directed', recording: false, promoted: false }, false],
    ['默認:手改(磁碟≠record)', { storedSha: A, diskSha: B, candSha: B, leg: 'directed', recording: false, promoted: false }, true],
    ['默認 T 腿:手改', { storedSha: A, diskSha: B, candSha: B, leg: 'template', recording: false, promoted: false }, true],
    ['錄製:無 promote 重錄(磁碟==record)', { storedSha: A, diskSha: A, candSha: A, leg: 'directed', recording: true, promoted: false }, false],
    ['錄製:手改後無 promote', { storedSha: A, diskSha: B, candSha: B, leg: 'directed', recording: true, promoted: false }, true],
    ['錄製:正常 promote(磁碟==record,候選異)', { storedSha: A, diskSha: A, candSha: B, leg: 'directed', recording: true, promoted: true }, false],
    ['錄製:手改後 promote(不洗白)', { storedSha: A, diskSha: B, candSha: B, leg: 'directed', recording: true, promoted: true }, true],
    ['錄製:directed 無 record 無 promote', { storedSha: null, diskSha: A, candSha: A, leg: 'directed', recording: true, promoted: false }, true],
    ['錄製:directed 首錄帶 promote', { storedSha: null, diskSha: null, candSha: B, leg: 'directed', recording: true, promoted: true }, false],
    ['錄製:T 腿模板重生成(磁碟==record,主腿異)', { storedSha: A, diskSha: A, candSha: B, leg: 'template', recording: true, promoted: false }, false],
    ['錄製:T 腿首錄', { storedSha: null, diskSha: null, candSha: B, leg: 'template', recording: true, promoted: false }, false],
    ['directed 無 promote 主腿≠磁碟(防禦)', { storedSha: A, diskSha: A, candSha: B, leg: 'directed', recording: false, promoted: false }, true],
  ];
  const svBad = sv.filter(([, inp, red]) => (tbShaAlarm(inp) !== null) !== red).map(([name]) => name);
  ck(svBad.length === 0, `selfcheck TB-SHA-ALARM ${sv.length} 向量(手改紅/promote 不誤報/promote 不洗白)`, svBad.join(' | '));
  // 等價裁決行形(收尾 S1;真實輸出形=yosys 0.70+179 實跑;敵意面=equiv_simple/induct 的相近行不得誤中)。
  const LOG_PROVEN = (n) => `${n} objects.\nFound ${n} unproven $equiv cells (6 groups) in equiv:\nProved ${n} previously unproven $equiv cells.\nNo selected unproven $equiv cells found in equiv.\nProved 0 previously unproven $equiv cells.\nFound ${n} $equiv cells in equiv:\n  Of those cells ${n} are proven and 0 are unproven.\n  Equivalence successfully proven!\n`;
  const LOG_UNPROVEN = '57 objects.\nFound 57 unproven $equiv cells (21 groups) in equiv:\nProved 55 previously unproven $equiv cells.\nFound 2 unproven $equiv cells in module equiv:\nProved 0 previously unproven $equiv cells.\nFound 57 $equiv cells in equiv:\n  Of those cells 55 are proven and 2 are unproven.\n  Unproven $equiv $auto$equiv_make.cc:297:find_same_wires$1542: \\state_gold [1] \\state_gate [1]\n  Unproven $equiv $auto$equiv_make.cc:297:find_same_wires$1541: \\state_gold [0] \\state_gate [0]\nFound a total of 2 unproven $equiv cells.\n';
  const ev = (o, out = 4) => eqVerdictOf({ code: 0, stdout: '', stderr: '', timedOut: false, spawnError: null, ...o }, out);
  const lv = [
    ['proven 且 cells==輸出位數(signedexpr 18/18 臨界)→equivalent', ev({ stdout: LOG_PROVEN(18) }, 18), 'equivalent', undefined],
    ['proven 且 cells<輸出位數(R2 全端口口徑誤判形)→empty', ev({ stdout: LOG_PROVEN(18) }, 23), 'adjudication-failed', 'empty'],
    ['unproven 行形→unproven', ev({ stdout: LOG_UNPROVEN }), 'unproven', undefined],
    ['No cells+exit 0(全名單 blacklist 實測形)→empty', ev({ stdout: '0 objects.\nNo $equiv cells found in equiv.\n' }), 'adjudication-failed', 'empty'],
    ['$pow 類 No SAT model(rc=1 在 equiv_simple)→no-sat-model', ev({ code: 1, stdout: '8 objects.\nFound 8 unproven $equiv cells (1 groups) in equiv:\n', stderr: 'ERROR: No SAT model available for cell $x.v:2$pow$2_gate ($pow).\n' }), 'adjudication-failed', 'no-sat-model'],
    ['proc 錯(OP-POL 異步復位形,實測 i2c m0011)→yosys-error', ev({ code: 1, stderr: "ERROR: Async reset \\rst_n yields non-constant value 1'm for signal \\ack_rx.\n" }), 'adjudication-failed', 'yosys-error'],
    ['逾時→timeout', ev({ timedOut: true, code: 124 }), 'adjudication-failed', 'timeout'],
    ['oracle 缺席→oracle-absent', ev({ spawnError: 'ENOENT', code: 1 }), 'adjudication-failed', 'oracle-absent'],
    ['無 select -count 行→no-count', ev({ stdout: 'Found 40 $equiv cells in equiv:\n  Of those cells 40 are proven and 0 are unproven.\n  Equivalence successfully proven!\n' }), 'adjudication-failed', 'no-count'],
    ['select 與 equiv_status 計數不等→count-mismatch', ev({ stdout: `39 objects.\n${LOG_PROVEN(40).split('\n').slice(1).join('\n')}` }), 'adjudication-failed', 'count-mismatch'],
    ['proven 計數與 unproven 行數不合→parse', ev({ stdout: LOG_UNPROVEN.replace('  Unproven $equiv $auto$equiv_make.cc:297:find_same_wires$1541: \\state_gold [0] \\state_gate [0]\n', '') }), 'adjudication-failed', 'parse'],
  ];
  const lvBad = lv.filter(([, got, v, why]) => got.eqverdict !== v || got.eqReason !== why).map(([n, got]) => `${n} 得 ${JSON.stringify(got)}`);
  ck(lvBad.length === 0, `selfcheck 等價裁決行形 ${lv.length} 向量(含 R2 全端口口徑誤判形)`, lvBad.join(' | '));
  const uv = ev({ stdout: LOG_UNPROVEN });
  ck(uv.eqUnproven === 2 && JSON.stringify(uv.unprovenCells) === '["state[0]","state[1]"]' && uv.eqCells === 57, 'selfcheck unprovenCells 解析(未證信號名+計數)', JSON.stringify(uv));
  // top-only 提取(行級狀態機):只留 top;前綴同名不誤中;註釋內不計;定義數≠1 必 throw。
  const multi = '`timescale 1ns/1ps\nmodule sub(input a);\nendmodule\n// module top(old);\n/*\nmodule top(older);\n*/\nmodule top_extra(input b);\nendmodule\nmodule top(input c);\n  sub u(.a(c)); // endmodule 在註釋\nendmodule // top\nmodule tail(input d);\nendmodule\n';
  ck(extractTopOnly(multi, 'top') === 'module top(input c);\n  sub u(.a(c)); // endmodule 在註釋\nendmodule // top\n', 'selfcheck top-only 多模組只留 top(前綴同名/註釋內/塊註釋內不誤中)');
  const thrown = (fn) => {
    try {
      fn();
      return false;
    } catch {
      return true;
    }
  };
  ck(thrown(() => extractTopOnly('module a;\nendmodule\n', 'top')) && thrown(() => extractTopOnly('module top;\nendmodule\nmodule top;\nendmodule\n', 'top')) && thrown(() => extractTopOnly('module top;\n', 'top')), 'selfcheck top-only 定義缺/重複/無 endmodule 必 throw');
  ck(JSON.stringify(portBits({ modules: { t: { ports: { a: { direction: 'input', bits: [1, 2] }, y: { direction: 'output', bits: [3] }, z: { direction: 'output', bits: [4, 5] } } } } }, 't')) === '{"all":5,"out":3}', 'selfcheck 端口位數表(all/out=輸出位數口徑)');
  // WAIVE↔record 交叉驗證(一正多負;killed 兩腿都算;無資料不得宣稱;靜態標籤不判)。
  const wm = (key, r, adj, ids = ['a.R1']) => ({ key, pool: 'rule', ruleIds: ids, result: r, ...(adj ? { adjudication: adj === 'equivalent' ? 'equivalent' : 'unadjudicated', eqverdict: adj } : {}) });
  const cross = (tag, main, base) => waiveCrossFindings(waiveRows([{ id: 'a.R1', tag }], main, base)).length;
  const cv = [
    ['存活全 equivalent 標 unobservable=過', cross('unobservable', [wm('k1', 'survived', 'equivalent')]), 0],
    ['存活含 unproven 標 unobservable=紅(i2c 11 條形)', cross('unobservable', [wm('k1', 'survived', 'equivalent'), wm('k2', 'survived', 'unproven')]), 1],
    ['存活含 unproven 標 unadjudicated=過', cross('unadjudicated', [wm('k1', 'survived', 'unproven')]), 0],
    ['存活全 equivalent 標 unadjudicated=紅(已證仍標未證)', cross('unadjudicated', [wm('k1', 'survived', 'equivalent')]), 1],
    ['有 killed 標 unadjudicated=紅', cross('unadjudicated', [wm('k1', 'killed'), wm('k2', 'survived', 'unproven')]), 1],
    ['T 腿 killed 標 unobservable=紅(任一腿殺=可觀測)', cross('unobservable', [wm('k1', 'survived', 'equivalent')], [wm('k1', 'killed')]), 1],
    ['無變異標 unobservable=紅(no-data 不得宣稱)', cross('unobservable', []), 1],
    ['無裁決資料(未裁決)標 unobservable=紅(保守)', cross('unobservable', [wm('k1', 'survived')]), 1],
    ['靜態標籤 x-valued 不判', cross('x-valued', [wm('k1', 'killed')]), 0],
  ];
  const cvBad = cv.filter(([, got, want]) => got !== want).map(([n, got]) => `${n} 得 ${got}`);
  ck(cvBad.length === 0, `selfcheck WAIVE↔record 交叉驗證 ${cv.length} 向量`, cvBad.join(' | '));
  // eq-verify schema(默認模式防線;手改 record 必紅)。
  const goodRec = () => ({
    numbers: { equivalent: 1 },
    eqMeta: {
      yosysVersion: 'Yosys x',
      commandTemplate: EQ_COMMAND_TEMPLATE,
      portsTemplate: EQ_PORTS_TEMPLATE,
      emptyProbeTemplate: EQ_EMPTY_PROBE_TEMPLATE,
      provenance: EQ_PROVENANCE,
      ports: { all: 3, out: 2 },
      goldVsGold: { eqverdict: 'equivalent', eqCells: 2 },
      emptyProbe: { eqverdict: 'adjudication-failed', eqCells: 0, eqReason: 'empty' },
      counts: { adjudicated: 2, equivalent: 1, unproven: 1, adjudicationFailed: 0 },
      killedSelfcheck: { total: 1, mainKilled: 1, unproven: 1, adjudicationFailed: 0, equivalent: 0, failed: [] },
    },
    mutants: [
      { id: 'm1', key: 'k1', pool: 'rule', result: 'survived', adjudication: 'equivalent', eqverdict: 'equivalent', eqCells: 2 },
      { id: 'm2', key: 'k2', pool: 'rule', result: 'survived', adjudication: 'unadjudicated', eqverdict: 'unproven', eqCells: 9, eqUnproven: 1 },
      { id: 'm3', key: 'k3', pool: 'rule', result: 'killed' },
    ],
  });
  const tampered = [
    ['刪 eqMeta', (r) => delete r.eqMeta],
    ['counts 改 1', (r) => (r.eqMeta.counts.unproven = 2)],
    ['equivalent 空真(cells<輸出位數)', (r) => (r.mutants[0].eqCells = 1)],
    ['adjudication 與 eqverdict 矛盾', (r) => (r.mutants[1].eqverdict = 'equivalent')],
    ['killed 自檢 equivalent≠0', (r) => (r.eqMeta.killedSelfcheck.equivalent = 1)],
    ['命令模板漂移', (r) => (r.eqMeta.commandTemplate = 'equiv_make -blacklist x gold gate equiv')],
    ['gold-vs-gold 非 equivalent', (r) => (r.eqMeta.goldVsGold = { eqverdict: 'unproven', eqCells: 2 })],
    ['空真探針非 empty', (r) => (r.eqMeta.emptyProbe = { eqverdict: 'equivalent', eqCells: 2 })],
    ['fsm 正樣探針缺', (r) => (r.design = 'fsm')],
    ['i2c 反樣探針判 equivalent', (r) => {
      r.design = 'i2c_master';
      r.eqMeta.groundTruth = { negative: { id: 'm0001', eqverdict: 'equivalent' } };
    }],
  ];
  const tBad = tampered.filter(([, mut]) => {
    const r = goodRec();
    mut(r);
    return eqSchemaFindings(r).length === 0;
  }).map(([n]) => n);
  ck(eqSchemaFindings(goodRec()).length === 0 && tBad.length === 0, `selfcheck eq-verify schema(正樣無 finding+${tampered.length} 條篡改必紅)`, `${JSON.stringify(eqSchemaFindings(goodRec()))} ${tBad.join(' | ')}`);
  // 協議釘檔硬同步:生產命令形/探針模板/端口表/口徑逐字在協議 §7;探針與生產只差 equiv_make 一段。
  const protoSync = [EQ_COMMAND_TEMPLATE, EQ_EMPTY_PROBE_TEMPLATE, EQ_EMPTY_NAMES_TEMPLATE, EQ_PORTS_TEMPLATE, EQ_PROVENANCE].filter((t) => !protoText.includes(t));
  ck(protoSync.length === 0, 'selfcheck 等價裁決命令模板與協議 §7 逐字同步', protoSync.join(' | '));
  ck(EQ_EMPTY_PROBE_TEMPLATE.replace('equiv_make -blacklist <all-names.txt> gold', 'equiv_make gold') === EQ_COMMAND_TEMPLATE && !EQ_COMMAND_TEMPLATE.includes('blacklist') && EQ_COMMAND_TEMPLATE.includes('flatten'), 'selfcheck 生產形無 blacklist 且含 flatten;探針模板與生產只差 equiv_make');
  // 波形幀腿機械前置(收尾 S2;幀診斷本身不進閘,這裡只驗純函數)。敵意面=跨行 title:逐行形剝不淨會誤判畫布不同→偽放行 vision。
  const svgT = (v, extra = '') => `<svg>\n<g><title>tb.q w=4 type=wire</title><title>t=0..10: ${v}</title><rect x="1"/>${extra}</g>\n</svg>\n`;
  const lineWise = (s) => s.split('\n').map((l) => l.replace(/<title>[^<]*<\/title>/g, '')).join('\n');
  const mlA = svgT('line1\nline2');
  const mlB = svgT('line1\nOTHER\nline3');
  ck(lineWise(mlA) !== lineWise(mlB) && stripTitles(mlA) === stripTitles(mlB), 'selfcheck 剝 title 跨行敵意向量(逐行形誤判異,程序化形判同)');
  ck(stripTitles('<title>a</title><rect/><title>b</title>') === '<rect/>' && stripTitles('<title>x&lt;/title&gt;y</title><rect/>') === '<rect/>', 'selfcheck 剝 title 非貪婪(不吞 title 間畫布;轉義閉標籤不提前收)');
  ck(stripTitles(svgT('1')) === stripTitles(svgT('2')) && stripTitles(svgT('1')) !== stripTitles(svgT('1', '<line x1="9"/>')), 'selfcheck 畫布判別力(只差 title 判同;畫布多一筆判異)');
  const axis = (ls) => ls.map((t) => `<text x="1" y="9" font-size="9" fill="#8899aa" text-anchor="middle">${t}</text>`).join('\n');
  ck(axisTmax(axis([0, 220000, 440000, 660000, 880000])) === 880000 && thrown(() => axisTmax(axis([0, 1]))) && thrown(() => axisTmax(axis([5, 6, 7, 8, 9]))), 'selfcheck 時間軸 t_max 取自末標籤(標籤數≠5 或非 0 起點必 throw)');
  const row = (name, segs) => `<g><title>${name} w=1 type=wire</title><text>${name}</text>${segs.map(([a, b, v]) => `<title>t=${a}..${b}: ${v}</title>`).join('')}<polyline/></g>\n`;
  const dv = divergingSignals(
    row('tb.a', [[0, 10, '0'], [10, 100, '1']]) + row('tb.b', [[0, 100, 'b01']]) + row('tb.c', [[0, 50, '0'], [50, 100, '0']]) + row('tb.d', []),
    row('tb.a', [[0, 40, '0'], [40, 100, '1']]) + row('tb.b', [[0, 30, 'b01'], [30, 100, 'b11']]) + row('tb.c', [[0, 100, '0']]) + row('tb.d', []),
  );
  ck(JSON.stringify(dv.map((d) => [d.signal, d.firstTick, d.gold, d.mutant])) === '[["tb.a",10,"1","0"],["tb.b",30,"b01","b11"]]', 'selfcheck title 層分歧表(沿錯位/值差必報、冗餘段不誤報、序=首分歧 tick)', JSON.stringify(dv));
  const md = (o) => decideModality({ goldSvg: svgT('1'), mutantSvg: svgT('1'), goldPngSha: 'A', mutantPngSha: 'A', ...o });
  const mdv = [
    ['雙檢皆異→vision', md({ mutantSvg: svgT('1', '<line/>'), mutantPngSha: 'B' }), 'vision'],
    ['畫布同(只差 title)→強制 fallback', md({ mutantSvg: svgT('2') }), 'text-fallback'],
    ['畫布異但 PNG 同→fallback', md({ mutantSvg: svgT('1', '<line/>') }), 'text-fallback'],
    ['柵格化缺→fallback', md({ mutantSvg: svgT('1', '<line/>'), goldPngSha: null, mutantPngSha: null }), 'text-fallback'],
  ];
  const mdBad = mdv.filter(([, got, want]) => got.modalityAllowed !== want || (want !== 'vision' && got.limitations.length === 0)).map(([n, got]) => `${n} 得 ${JSON.stringify(got)}`);
  ck(mdBad.length === 0, `selfcheck modality 判定 ${mdv.length} 向量(非 vision 必附 limitations)`, mdBad.join(' | '));
  ck(isPngBytes([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0]) && !isPngBytes([0x3c, 0x73, 0x76, 0x67]) && !isPngBytes(undefined), 'selfcheck PNG 魔數');
  const mf = (allowed) => ({ design: 'd', mutant: { id: 'm1' }, frames: ['g.png', 'm.png', 'gz.png', 'mz.png'], primaryFrames: ['g.png', 'm.png'], modalityAllowed: allowed, dualCheck: { canvasDiffer: allowed === 'vision', pngDiffer: allowed === 'vision' } });
  const fio = { exists: (p) => p !== 'gone.png', isPng: (p) => p !== 'fake.png' };
  const dg0 = (o) => ({ design: 'd', mutantId: 'm1', modality: 'vision', framesRead: 2, frames: ['g.png', 'm.png'], findings: [], confidence: 'medium', limitations: [], ...o });
  const vd = [
    ['vision 實讀兩主幀', dg0({}), mf('vision'), 0],
    ['vision 未列 frames 時對 manifest 全幀', dg0({ frames: undefined, framesRead: 4 }), mf('vision'), 0],
    ['機械 fallback(強制)', dg0({ modality: 'text-fallback', frames: [], framesRead: 0, limitations: ['畫布層不可分'] }), mf('text-fallback'), 0],
    ['視覺不可用的降級(雙檢皆異)', dg0({ modality: 'text-fallback', frames: [], framesRead: 0, limitations: ['視覺會話不可用'] }), mf('vision'), 0],
    ['雙檢不過卻標 vision', dg0({}), mf('text-fallback'), 1],
    ['vision 只讀一張主幀', dg0({ frames: ['g.png'], framesRead: 1 }), mf('vision'), 1],
    ['framesRead 與 frames 不等', dg0({ framesRead: 3 }), mf('vision'), 1],
    ['幀不存在', dg0({ frames: ['g.png', 'gone.png'] }), mf('vision'), 1],
    ['幀非 PNG', dg0({ frames: ['g.png', 'fake.png'] }), mf('vision'), 1],
    ['confidence 不合法', dg0({ confidence: 'certain' }), mf('vision'), 1],
    ['modality 不合法', dg0({ modality: 'video' }), mf('vision'), 1],
    ['text-fallback 無原因', dg0({ modality: 'text-fallback', frames: [], framesRead: 0 }), mf('vision'), 1],
    ['設計不符', dg0({ design: 'x' }), mf('vision'), 1],
    ['mutantId 不符', dg0({ mutantId: 'm9' }), mf('vision'), 1],
  ];
  const vdBad = vd.filter(([, d, m, red]) => (validateDiag(d, m, fio).length > 0 ? 1 : 0) !== red).map(([n, d, m]) => `${n} 得 ${JSON.stringify(validateDiag(d, m, fio))}`);
  ck(vdBad.length === 0, `selfcheck diag 驗證器 ${vd.length} 向量(4 正 10 負)`, vdBad.join(' | '));
  ck(protoText.includes('<!-- vision-prompt v1 begin -->') && protoText.includes('<!-- vision-prompt v1 end -->') && !parseTemplates(protoText).some((t) => t.lines.join('\n').includes('波形診斷')), 'selfcheck 視覺 prompt 圍欄在協議 §8 且不混入 TB 模板版本鏈');
  // 主根 spec 釘選判定(收尾弧 S3 斷言的純函數面;初版空操作事故的回歸探針在 finish-s3 驗收,這裡釘判定本身)。
  const pinnedV = new Set(['corpus-external/yosys-simple/a.v']);
  const sp = [
    ['golden 快照+已釘', { staging: false, specPath: 'plugin/rtl2spec/golden/yosys-simple/a.rules.json', src: 'corpus-external/yosys-simple/a.v', pinned: pinnedV }, true],
    ['staging 快照路徑', { staging: false, specPath: 'plugin/rtl2spec/build/conformance-staging/specs/yosys-simple/a.rules.json', src: 'corpus-external/yosys-simple/a.v', pinned: pinnedV }, false],
    ['golden 路徑但源未釘', { staging: false, specPath: 'plugin/rtl2spec/golden/yosys-simple/b.rules.json', src: 'corpus-external/yosys-simple/b.v', pinned: pinnedV }, false],
    ['staging 模式不斷言', { staging: true, specPath: 'plugin/rtl2spec/build/x.rules.json', src: 'x.v', pinned: pinnedV }, true],
  ];
  const spBad = sp.filter(([, inp, want]) => specPinnedOk(inp) !== want).map(([n]) => n);
  ck(spBad.length === 0, `selfcheck 主根 spec 釘選判定 ${sp.length} 向量(1 正 2 負+staging 豁免)`, spBad.join(' | '));
}

// ---------- 設計清單 ----------
const designs = existsSync(fixRoot)
  ? readdirSync(fixRoot)
      .filter((d) => existsSync(join(fixRoot, d, 'case.json')))
      .sort()
  : [];
const selected = tryDesign !== null ? designs.filter((d) => d === tryDesign) : designArgs.length ? designs.filter((d) => designArgs.includes(d)) : designs;

// ---------- 等價裁決腿(僅 --record;測試側 harness,協議 §7;雙向措辭:proven≠無條件等價,unproven≠不等價) ----------
/// 逐設計裁決:端口表→gold-vs-gold→空真探針→地面真值→存活裁決→killed 全母體自檢→存量 equivalent 重驗。
/// 任一偵測力斷言不過=failures(呼叫端轉 REJECTED,錄製即拒錄)。
function eqStage({ design, cs, srcPath, srcText, work, pm, mainRes, baseRes, stored }) {
  const t0 = Date.now();
  const dir = join(work, 'eq');
  mkdirSync(dir, { recursive: true });
  const failures = [];
  const adjOf = new Map();
  const eqOf = new Map();
  const y = (args) => run(yosysBin, args, { cwd: repoRoot, timeoutMs: 120000 });
  const src = rel(srcPath);
  const top = cs.top;
  const portsJson = join(dir, 'ports.json');
  let ports = null;
  try {
    const rp = y(['-q', '-p', fillTemplate(EQ_PORTS_TEMPLATE, { '<src>': src, '<top>': top, '<ports.json>': rel(portsJson) })]);
    if (rp.code !== 0) throw new Error((rp.stderr || rp.spawnError || `exit ${rp.code}`).slice(0, 160));
    ports = portBits(JSON.parse(readFileSync(portsJson, 'utf8')), top);
  } catch (e) {
    failures.push(`端口表失敗: ${e.message}`);
    return { failures, adjOf, eqOf, eqMeta: null };
  }
  const topOnlyFile = (id, text) => {
    const p = join(dir, `${id}.top.v`);
    writeFileSync(p, extractTopOnly(text, top));
    return p;
  };
  const judge = (id, text) => {
    let mut;
    try {
      mut = topOnlyFile(id, text);
    } catch (e) {
      return { eqverdict: 'adjudication-failed', eqCells: null, eqReason: 'parse', eqDetail: e.message.slice(0, 160) };
    }
    return eqVerdictOf(y(['-p', fillTemplate(EQ_COMMAND_TEMPLATE, { '<src>': src, '<top>': top, '<mut-top-only>': rel(mut) })]), ports.out);
  };

  // gold-vs-gold 帶守衛(先於逐變異裁決)。
  const gg = judge('_gg', srcText);
  if (gg.eqverdict !== 'equivalent') failures.push(`gold-vs-gold 非 equivalent: ${JSON.stringify(gg)}`);

  // 空真探針=獨立模板(全名單 blacklist;唯一用途=製造空真,不屬生產管線;必判 adjudication-failed(empty))。
  let emptyProbe;
  try {
    const namesJson = join(dir, 'names.json');
    const rn = y(['-q', '-p', fillTemplate(EQ_EMPTY_NAMES_TEMPLATE, { '<src>': src, '<top>': top, '<names.json>': rel(namesJson) })]);
    if (rn.code !== 0) throw new Error((rn.stderr || `exit ${rn.code}`).slice(0, 160));
    const blacklist = join(dir, 'all-names.txt');
    writeFileSync(blacklist, publicNames(JSON.parse(readFileSync(namesJson, 'utf8')), top).join('\n') + '\n');
    const rProbe = y(['-p', fillTemplate(EQ_EMPTY_PROBE_TEMPLATE, { '<src>': src, '<top>': top, '<mut-top-only>': rel(topOnlyFile('_empty', srcText)), '<all-names.txt>': rel(blacklist) })]);
    emptyProbe = eqVerdictOf(rProbe, ports.out);
  } catch (e) {
    emptyProbe = { eqverdict: 'probe-error', eqDetail: e.message.slice(0, 160) };
  }
  if (emptyProbe.eqverdict !== 'adjudication-failed' || emptyProbe.eqReason !== 'empty') failures.push(`空真探針未判 adjudication-failed(empty): ${JSON.stringify(emptyProbe)}`);

  // 地面真值探針。
  const truth = EQ_GROUND_TRUTH[design] ?? {};
  const groundTruth = {};
  if (truth.positive) {
    const m = pm.mutants.find((x) => x.id === truth.positive);
    if (!m || m.op !== 'OP-NOP') failures.push(`正樣探針 ${truth.positive} 非 OP-NOP(${m?.op})`);
    else {
      const v = judge(m.id, applyMutant(srcText, m));
      groundTruth.positive = { id: m.id, op: m.op, eqverdict: v.eqverdict, eqCells: v.eqCells };
      if (v.eqverdict !== 'equivalent') failures.push(`正樣探針 ${m.id} OP-NOP 未判 equivalent: ${JSON.stringify(v)}`);
    }
  }

  // 逐變異:全腿存活 → eqverdict;任一腿殺 → killed 全母體自檢(斷言零 equivalent)。
  const counts = { adjudicated: 0, equivalent: 0, unproven: 0, adjudicationFailed: 0 };
  const ks = { total: 0, mainKilled: 0, unproven: 0, adjudicationFailed: 0, equivalent: 0, failed: [] };
  const killedEq = [];
  for (let k = 0; k < pm.mutants.length; k++) {
    const m = pm.mutants[k];
    if (m.pool !== 'rule') continue;
    const mr = mainRes[k];
    const br = baseRes ? baseRes[k] : null;
    const allSurvive = mr.result === 'survived' && (!br || br.result === 'survived');
    const anyKilled = isKill(mr) || isKill(br);
    if (!allSurvive && !anyKilled) continue;
    const v = judge(m.id, applyMutant(srcText, m));
    if (allSurvive) {
      eqOf.set(m.key, v);
      adjOf.set(m.key, v.eqverdict === 'equivalent' ? 'equivalent' : 'unadjudicated');
      counts.adjudicated++;
      if (v.eqverdict === 'equivalent') counts.equivalent++;
      else if (v.eqverdict === 'unproven') counts.unproven++;
      else counts.adjudicationFailed++;
    } else {
      ks.total++;
      if (isKill(mr)) ks.mainKilled++;
      if (v.eqverdict === 'equivalent') {
        ks.equivalent++;
        killedEq.push(`${m.id} ${m.op}@${m.line}`);
      } else if (v.eqverdict === 'unproven') ks.unproven++;
      else {
        ks.adjudicationFailed++;
        ks.failed.push(`${m.id} ${m.op}@${m.line} ${v.eqReason}${v.eqDetail ? `: ${v.eqDetail}` : ''}`);
      }
      if (m.id === truth.negative) groundTruth.negative = { id: m.id, op: m.op, eqverdict: v.eqverdict, eqCells: v.eqCells };
    }
  }
  if (killedEq.length) failures.push(`killed 全母體自檢:任一腿殺的變異被判 equivalent ${killedEq.length} 個(${killedEq.slice(0, 8).join(', ')})`);
  if (truth.negative) {
    if (!groundTruth.negative) failures.push(`反樣探針 ${truth.negative} 不在 killed 母體`);
    else if (groundTruth.negative.eqverdict === 'equivalent') failures.push(`反樣探針 ${truth.negative} 被判 equivalent`);
  }

  // 存量 equivalent 帶守衛重驗(任一翻變=紅燈列名)。
  const flips = [];
  for (const o of stored?.mutants ?? []) {
    if (o.adjudication !== 'equivalent') continue;
    const v = eqOf.get(o.key);
    if (!v || v.eqverdict !== 'equivalent') flips.push(`${o.id} ${o.op}@${o.line}→${v ? v.eqverdict : '未裁決(已被殺或消失)'}`);
  }
  if (flips.length) failures.push(`存量 equivalent 翻變 ${flips.length} 個(${flips.slice(0, 8).join(', ')})`);

  const eqMeta = {
    yosysVersion: tools.yosys,
    commandTemplate: EQ_COMMAND_TEMPLATE,
    portsTemplate: EQ_PORTS_TEMPLATE,
    emptyProbeTemplate: EQ_EMPTY_PROBE_TEMPLATE,
    provenance: EQ_PROVENANCE,
    ports,
    goldVsGold: gg,
    emptyProbe,
    ...(Object.keys(groundTruth).length ? { groundTruth } : {}),
    counts,
    killedSelfcheck: ks,
  };
  console.log(
    `INFO eq ${design}: ports all/out=${ports.all}/${ports.out} gg=${gg.eqverdict}(cells ${gg.eqCells}) empty=${emptyProbe.eqverdict}(${emptyProbe.eqReason ?? '-'}) 裁決=${counts.adjudicated}[equivalent ${counts.equivalent}/unproven ${counts.unproven}/failed ${counts.adjudicationFailed}] killed 自檢=${ks.total}[main ${ks.mainKilled};unproven ${ks.unproven}/failed ${ks.adjudicationFailed}/equivalent ${ks.equivalent}] ${Date.now() - t0}ms`,
  );
  for (const x of ks.failed) console.log(`INFO eq ${design} killed 裁決失敗: ${x}`);
  return { failures, adjOf, eqOf, eqMeta };
}

// ---------- 逐設計評測 ----------
function evalDesign(design, { tryTb: tryTbPath = null } = {}) {
  const dir = join(fixRoot, design);
  const cs = JSON.parse(readFileSync(join(dir, 'case.json'), 'utf8'));
  const work = join(outRoot, design);
  rmSync(work, { recursive: true, force: true });
  mkdirSync(join(work, 'mut'), { recursive: true });
  const reasons = [];
  const srcPath = join(repoRoot, cs.src);
  const srcText = readFileSync(srcPath, 'utf8');
  const spec = JSON.parse(readFileSync(join(repoRoot, cs.specPath), 'utf8'));
  const mod = spec.modules.find((m) => m.module === cs.top);
  if (!mod) {
    reasons.push(`specPath 無模組 ${cs.top}`);
    return { design, verdict: { design, verdict: 'REJECTED', reasons } };
  }
  const rules = mod.rules;
  const ruleIds = rules.map((r) => r.id);
  // 收尾弧 S3 驗收實錄:初版只 push reason 未入 admitted 判定=空操作(負向探針 P1/P2 抓出);判定旗標必須入 admitted。
  const specPinned = specPinnedOk({ staging, specPath: cs.specPath, src: cs.src, pinned: pinnedSet });
  if (!specPinned) {
    reasons.push(`specPath 不在 golden 釘選集(src=${cs.src} specPath=${cs.specPath};主根設計的規則書必須是釘選快照)`);
  }
  const flags = cs.iverilog ?? ['-g2005'];

  // [T 腿] 進程內 3 次重生成 byte 相等(且默認模式==入庫 tb.v)。
  const frames = [0, 1, 2].map(() => glue.rtl_tb_plan_sv(cs.src, srcText, cs.top, cs.seed ?? 0, cs.cycles ?? 0, cs.flush ?? 0));
  const deterministic = frames[0] === frames[1] && frames[1] === frames[2];
  const frame = JSON.parse(frames[0]);
  if (!frame.ok) {
    reasons.push(`tb_plan 拒絕: ${frame.error}`);
    return { design, verdict: { design, verdict: 'REJECTED', reasons } };
  }
  const plan = frame.plan;
  const tbPath = join(dir, 'tb.v');
  const baselineTemplate = frame.template_v;
  let tbText;
  let leg = cs.tb_leg === 'directed' ? 'directed' : 'template';
  const reject = (msg) => {
    reasons.push(msg);
    return { design, verdict: { design, verdict: 'REJECTED', reasons }, stored: null, tbText: '', expected: '', iron: false, formFloor: null, notWorseBase: null };
  };
  const promoteSrc = recordMode ? promote.get(design) ?? null : null;
  if (promoteSrc && leg !== 'directed') return reject('--promote 只對 case.json tb_leg=directed 的設計(先改斷言規格)');
  if (tryTbPath) {
    if (!existsSync(tryTbPath)) return reject(`候選 TB 缺: ${tryTbPath}`);
    tbText = readFileSync(tryTbPath, 'utf8');
    leg = 'directed';
  } else if (leg === 'directed') {
    // D 腿=凍結的 LLM 產物:錄製只在 --promote 時換檔,否則沿用入庫 tb.v(閘永不重生成 D 腿)。
    const src = promoteSrc ?? tbPath;
    if (!existsSync(src)) return reject(`directed 腿 tb 缺: ${src}`);
    tbText = readFileSync(src, 'utf8');
  } else if (recordMode) {
    tbText = frame.template_v;
  } else {
    tbText = existsSync(tbPath) ? readFileSync(tbPath, 'utf8') : '';
    if (tbText !== frame.template_v) reasons.push('T 腿模板重生成≠入庫 tb.v(陳舊或手改)');
  }
  if (!deterministic) reasons.push('T 腿進程內 3 次重生成不相等');
  const workTb = join(work, 'tb.v');
  writeFileSync(workTb, tbText);

  // Q6/Q9 靜態 lint。
  const lint = lintTb(tbText, { plan, ruleIds, leg });
  const q6 = lint.q6.length === 0;
  const q9 = lint.q9.length === 0;
  if (!q6) reasons.push(`Q6: ${lint.q6.slice(0, 3).join(' | ')}`);
  if (!q9) reasons.push(`Q9: ${lint.q9.slice(0, 3).join(' | ')}`);

  // Q1 編譯+Q2 原版 2 跑。
  const tbTop = `tb_${cs.top}`;
  const r1 = iverilogRun({ flags, top: tbTop, files: [workTb, srcPath], workDir: work, outName: 'orig.vvp', timeoutMs: SIM_TIMEOUT_MS });
  const q1 = r1.compiled && r1.warnLines.length === 0;
  if (!q1) reasons.push(`Q1: ${r1.compiled ? `warning=${r1.warnLines.length}` : 'compile failed'} ${(r1.warnLines[0] ?? r1.compErr ?? '').slice(0, 120)}`);
  let norm1 = '';
  let norm2 = '';
  let simMs = null;
  let q2 = false;
  let q3 = false;
  let q4 = false;
  let q5 = false;
  let proto = null;
  if (r1.compiled) {
    const runA = run('vvp', ['-n', join(work, 'orig.vvp')], { cwd: work, timeoutMs: SIM_TIMEOUT_MS });
    const runB = run('vvp', ['-n', join(work, 'orig.vvp')], { cwd: work, timeoutMs: SIM_TIMEOUT_MS });
    simMs = Math.max(runA.ms, runB.ms);
    norm1 = normalizeIverilog(runA.stdout);
    norm2 = normalizeIverilog(runB.stdout);
    const expPath = join(dir, 'expected.txt');
    const expected = recordMode ? norm1 : tryTbPath ? null : existsSync(expPath) ? readFileSync(expPath, 'utf8') : null;
    // try 模式:Q2=兩跑確定性(expected.txt 是 T 腿的,與 D 腿 TB 逐位元組不同屬預期;期望值由
    // --record 從原版重錄=oracle 紀律,候選期不比對)。
    q2 = norm1 === norm2 && (expected === null || norm1 === expected);
    if (!q2) reasons.push(`Q2: ${norm1 !== norm2 ? '原版兩跑不相等' : expected === null ? 'expected.txt 缺' : '原版輸出≠expected.txt'}`);
    proto = parseProtocol(norm1);
    const last = proto.doneLast && proto.doneCount === 1;
    q3 = last && proto.watchdog === 0 && proto.timeouts.length === 0 && !runA.timedOut && !runB.timedOut;
    if (!q3) reasons.push(`Q3: done=${proto.doneCount} last=${proto.doneLast} watchdog=${proto.watchdog} timeout=${proto.timeouts.length} wall=${runA.timedOut || runB.timedOut}`);
    q4 = proto.xz.length === 0;
    if (!q4) reasons.push(`Q4: x/z 值 ${proto.xz.length} 處(首:${proto.xz[0]})`);
    const ns = proto.scenes.map((s) => s.n);
    const contiguous = ns.every((n, k) => n === k + 1);
    q5 = contiguous && ns.length === lint.declaredScenes && proto.violations.length === 0;
    if (!q5) reasons.push(`Q5: scenes=${ns.length}/${lint.declaredScenes} contiguous=${contiguous} violations=${proto.violations.slice(0, 2).join(' | ')}`);
  }

  // Q7 跨模擬器(verilator 在場即執法)。
  let crossSim = 'SKIP';
  let q7 = true;
  let vlDiag = { tb: 0, dut: 0 };
  if (tools.verilator && r1.compiled) {
    const v = verilatorRun({ top: tbTop, files: [workTb, srcPath], mdir: join(work, 'vl') });
    vlDiag = {
      tb: v.diag.filter((l) => l.includes(workTb)).length,
      dut: v.diag.filter((l) => l.includes(srcPath)).length,
    };
    if (!v.built || v.timedOut) {
      crossSim = 'build-failed';
      q7 = false;
      reasons.push(`Q7: verilator 建置/運行失敗 ${(v.buildErr ?? '').slice(0, 160)}`);
    } else {
      const same = normalizeVerilator(v.stdout) === comparable(norm1);
      crossSim = same ? 'agree' : 'differ';
      q7 = same && vlDiag.tb === 0;
      if (!same) reasons.push('Q7: 兩腿正規化輸出不等');
      if (vlDiag.tb !== 0) reasons.push(`Q7: tb.v 位置警告 ${vlDiag.tb}`);
    }
  } else if (!tools.verilator) {
    console.log(`SKIP ${design} Q7 verilator 不在場`);
  }

  // 變異矩陣。主腿=tb.v(T 或凍結 D);directed 腿另把 T 腿模板(baselineTemplate)跑同一矩陣,
  // 兩腿數字分列,D 腿增益=D−T,禁合併(§19.3.1)。變異體檔案寫一次、兩腿共用。
  const pm = planMutations(srcText, rules);
  const storedPath = join(dir, 'record.json');
  const stored = existsSync(storedPath) ? JSON.parse(readFileSync(storedPath, 'utf8')) : null;
  // TB-SHA-ALARM(--try 不觸 fixtures 不適用;矛盾=REJECTED,默認模式 exit 1,錄製模式拒錄 exit 1)。
  const tbAlarm = tryTbPath
    ? null
    : tbShaAlarm({
        storedSha: stored?.tb?.sha256 ?? null,
        diskSha: existsSync(tbPath) ? sha256(readFileSync(tbPath, 'utf8')) : null,
        candSha: sha256(tbText),
        leg,
        recording: recordMode,
        promoted: promoteSrc !== null,
      });
  if (tbAlarm) reasons.push(`TB-SHA-ALARM: ${tbAlarm}`);
  const compareRecord = !recordMode && !tryTbPath;
  const prev = compareRecord ? stored : null;
  const prevByKey = new Map((prev?.mutants ?? []).map((m) => [m.key, m]));
  // equivalent 裁決是「變異體與原版」的屬性,與 TB 無關:非錄製模式一律沿用入庫 record 的裁決。
  const storedAdj = new Map((stored?.mutants ?? []).map((m) => [m.key, m.adjudication]));
  const eqFieldsOf = (m) => Object.fromEntries(['eqverdict', 'eqCells', 'eqReason', 'eqDetail', 'eqUnproven', 'unprovenCells'].filter((k) => m[k] !== undefined).map((k) => [k, m[k]]));
  const storedEq = new Map((stored?.mutants ?? []).filter((m) => m.eqverdict).map((m) => [m.key, eqFieldsOf(m)]));
  for (const m of pm.mutants) writeFileSync(join(work, 'mut', `${m.id}.v`), applyMutant(srcText, m));
  // 留一法輸入(C-S5):主腿 killed 變異的實際正規化輸出緩存(純文本)。
  const killedOutputs = new Map();
  const classify = (r, normRef, cacheKey) => {
    if (!r.compiled) return { result: r.timedOut ? 'timeout' : 'stillborn', killScenes: r.timedOut ? ['<timeout>'] : [] };
    if (r.timedOut) return { result: 'timeout', killScenes: ['<timeout>'] };
    const n = normalizeIverilog(r.stdout);
    if (n === normRef) return { result: 'survived', killScenes: [] };
    if (cacheKey !== null) killedOutputs.set(cacheKey, n);
    return { result: 'killed', killScenes: diffScenes(normRef, n) };
  };
  const matrix = (tbFile, normRef, tag, cache) =>
    pm.mutants.map((m) => {
      const r = iverilogRun({ flags, top: tbTop, files: [tbFile, join(work, 'mut', `${m.id}.v`)], workDir: join(work, 'mut'), outName: `${m.id}_${tag}.vvp`, timeoutMs: SIM_TIMEOUT_MS });
      return { ...m, ...classify(r, normRef, cache ? m.key : null) };
    });
  const mainRes = matrix(workTb, norm1, 'main', true);
  let baseRes = null;
  let normT = null;
  if (leg === 'directed') {
    const tTb = join(work, 'tb_template.v');
    writeFileSync(tTb, baselineTemplate);
    const t = iverilogRun({ flags, top: tbTop, files: [tTb, srcPath], workDir: work, outName: 'orig_template.vvp', timeoutMs: SIM_TIMEOUT_MS });
    if (t.compiled) {
      normT = normalizeIverilog(run('vvp', ['-n', join(work, 'orig_template.vvp')], { cwd: work, timeoutMs: SIM_TIMEOUT_MS }).stdout);
      baseRes = matrix(tTb, normT, 'tmpl', false);
    } else {
      reasons.push('基線 T 腿模板編譯失敗');
    }
  }
  // 裁決:錄製模式=強形 harness(僅對全腿存活的 P-rule 變異給 eqverdict;任一腿殺=可觀測差異=非等價,入 killed 全
  // 母體自檢);其餘模式沿用入庫 record。adjudication 兩值欄位保留(equivalent⇔eqverdict==equivalent)。
  const adjOf = new Map();
  const eqOf = new Map();
  let eqMeta = null;
  const eqFailures = [];
  if (recordMode) {
    const st = eqStage({ design, cs, srcPath, srcText, work, pm, mainRes, baseRes, stored });
    for (const [k, v] of st.adjOf) adjOf.set(k, v);
    for (const [k, v] of st.eqOf) eqOf.set(k, v);
    eqMeta = st.eqMeta;
    for (const f of st.failures) {
      eqFailures.push(f);
      reasons.push(`等價裁決: ${f}`);
    }
  } else {
    for (const m of pm.mutants) {
      if (storedAdj.get(m.key)) adjOf.set(m.key, storedAdj.get(m.key));
      if (storedEq.get(m.key)) eqOf.set(m.key, storedEq.get(m.key));
    }
  }
  const withAdj = (rs) => rs.map((m) => (adjOf.has(m.key) ? { ...m, adjudication: adjOf.get(m.key) } : m));
  // eq 欄只掛主腿(裁決是「變異體與原版」的屬性;基線腿沿 adjudication 兩值,避免 record 重複)。
  const results = withAdj(mainRes).map((m) => (eqOf.has(m.key) ? { ...m, ...eqOf.get(m.key) } : m));
  const baseResults = baseRes ? withAdj(baseRes) : null;

  // WAIVE↔record 交叉驗證(§3.5):數據標籤 unobservable/unadjudicated 必==數據謂詞(錄製模式用新裁決;其餘用入庫
  // 裁決,--try 的新存活變異無資料=未裁決,保守)。靜態標籤只可見化(record-diff 打印使用數變化量)。
  const waiveTable = waiveRows(lint.waiveEntries, results, baseResults);
  const waiveFindings = waiveCrossFindings(waiveTable);
  if (waiveFindings.length) {
    reasons.push(`Q9: WAIVE↔record 矛盾 ${waiveFindings.length} 條(首:${waiveFindings[0]})`);
    for (const x of waiveFindings) console.log(`INFO waive-cross ${design}: ${x}`);
  }

  // 數字(§19.4.3;兩腿同一函數)。
  const numbersOf = (rs) => {
    const P = rs.filter((m) => m.pool === 'rule');
    const equivalent = P.filter((m) => m.result === 'survived' && m.adjudication === 'equivalent');
    const eligibleSet = P.filter((m) => m.result !== 'stillborn' && m.adjudication !== 'equivalent');
    const killedNoEq = P.filter(isKill).filter((m) => m.adjudication !== 'equivalent');
    const controlSet = [...rs.filter((m) => m.pool === 'control'), ...equivalent];
    const controlsOk = controlSet.filter((m) => m.result === 'survived').length;
    const ruleHasEligible = new Set(eligibleSet.flatMap((m) => m.ruleIds));
    const exclusive = new Set(killedNoEq.filter((m) => m.ruleIds.length === 1).map((m) => m.ruleIds[0]));
    const sharedOnly = new Set(killedNoEq.filter((m) => m.ruleIds.length >= 2).flatMap((m) => m.ruleIds).filter((id) => !exclusive.has(id)));
    const ruleHasGenerated = new Set(P.flatMap((m) => m.ruleIds));
    const U = rs.filter((m) => m.pool === 'unruled');
    return {
      numbers: {
        generated: P.length,
        stillborn: P.filter((m) => m.result === 'stillborn').length,
        equivalent: equivalent.length,
        eligible: eligibleSet.length,
        killed: killedNoEq.length,
        survived: P.filter((m) => m.result === 'survived' && m.adjudication !== 'equivalent').length,
        timeout: P.filter((m) => m.result === 'timeout').length,
        rate: `${killedNoEq.length}/${eligibleSet.length}`,
        ruleVerified: { exclusive: `${exclusive.size}/${ruleHasEligible.size}`, shared: `${sharedOnly.size}/${ruleHasEligible.size}` },
        catalogCoverage: `${[...ruleHasGenerated].filter((id) => ruleIds.includes(id)).length}/${rules.length}`,
        unruled: { generated: U.length, killed: U.filter(isKill).length },
        controlsSurvived: `${controlsOk}/${controlSet.length}`,
        ambiguous: pm.attempts.filter((a) => a.status === 'ambiguous').length,
        na: pm.attempts.filter((a) => a.status === 'n/a').length,
      },
      P,
      eligibleSet,
      killedNoEq,
      controlSet,
      controlsOk,
      U,
    };
  };
  const main = numbersOf(results);
  const base = baseResults ? numbersOf(baseResults) : null;
  const numbers = main.numbers;

  // C-S5 留一法自驗(§19.6.4):killed 變異=真值已知的缺陷實現;診斷 top-3 命中率錄入 record,
  // 閘斷言相等(v1 不設下限,數字照報)。基於主腿(tb.v 主腿)矩陣的 killed 輸出緩存。
  let loo = null;
  if (killedOutputs.size > 0) {
    loo = leaveOneOut({
      expectedNorm: norm1,
      outputsByKey: killedOutputs,
      mutants: results.filter((m) => m.pool === 'rule' && killedOutputs.has(m.key)),
      plan,
      rules,
      covers: lintCovers(tbText),
    });
  }
  // def_lint(§19.6.1;閘側工具,RV1-14):定義完備性分類,U1/U4=缺陷,U2/U3=政策排除。
  const dl = defLint(srcText, mod);

  // Q8:鐵則+控制組存活+(默認)雙向比對 record(directed 腿兩腿都比)。
  const drift = [];
  const eqVerifyFindings = [];
  const cmpLeg = (label, nowRs, prevRs) => {
    const pk = new Map((prevRs ?? []).map((m) => [m.key, m]));
    const nk = new Map(nowRs.map((m) => [m.key, m]));
    for (const [k, p0] of pk) {
      const c = nk.get(k);
      if (!c) drift.push(`${label} 消失 ${p0.id} ${p0.op}@${p0.line}`);
      else if (c.result !== p0.result || JSON.stringify(c.killScenes) !== JSON.stringify(p0.killScenes)) {
        drift.push(`${label} ${p0.id} ${p0.op}@${p0.line}: ${p0.result}${JSON.stringify(p0.killScenes)}→${c.result}${JSON.stringify(c.killScenes)}`);
      }
    }
    for (const [k, c] of nk) if (!pk.has(k)) drift.push(`${label} 新增 ${c.id} ${c.op}@${c.line}`);
  };
  if (compareRecord) {
    if (!prev) drift.push('record.json 缺');
    else {
      // eq-verify(不跑 yosys):eqMeta/eqverdict schema+counts 一致性+waiveTable 與重算相等。
      // 過渡窗 S0→S1(舊版 record=綠+SKIP)已隨 C-S7 全集重錄關閉:eqMeta 缺=紅。
      for (const f of eqSchemaFindings(prev)) eqVerifyFindings.push(f);
      if (JSON.stringify(prev.waiveTable ?? null) !== JSON.stringify(waiveTable)) drift.push('waiveTable 漂移(WAIVE 標籤/分布與入庫 record 不符)');
      // wasm sha 保持=閘斷言(收尾弧 S4,FINISH-PLAN §6):入庫 record 是哪個 wasm 錄的,出貨的 dist wasm 就必須是它;
      // dist wasm 一變而未重錄=DRIFT 紅(重錄=語義事件)。
      if (prev.provenance?.wasmSha256 !== wasmSha) drift.push(`wasm sha 漂移 record=${(prev.provenance?.wasmSha256 ?? '<缺>').slice(0, 16)} dist=${wasmSha.slice(0, 16)}(dist wasm 變更未重錄)`);
      cmpLeg('main', results, prev.mutants);
      // C-S5 DRIFT 鏘位:def_lint 分類+留一法命中率(§19.6.2:錄入 record 比對漂移=DRIFT 紅;
      // §19.6.4:閘斷言相等=診斷生成器回歸面)。
      const dlA = JSON.stringify(prev.rulebookDefects?.defLint ?? null);
      const dlB = JSON.stringify(dl);
      if (dlA !== dlB) drift.push('defLint 漂移');
      const loA = prev.rulebookDefects?.leaveOneOut?.rate ?? null;
      const loB = loo ? loo.rate : null;
      if (loA !== loB) drift.push(`留一法命中率漂移 ${loA}→${loB}`);
      if (leg === 'directed') {
        const pb = prev.baselineTemplate;
        if (!pb) drift.push('directed 腿 record 缺 baselineTemplate');
        else {
          if (pb.sha256 !== sha256(baselineTemplate)) drift.push('基線模板 sha 漂移');
          if (normT !== null && pb.expectedSha256 !== sha256(normT)) drift.push('基線模板原版輸出 sha 漂移');
          if (baseResults) cmpLeg('baseline', baseResults, pb.mutants);
        }
      } else if (prev.baselineTemplate) {
        drift.push('template 腿 record 卻帶 baselineTemplate');
      }
    }
  }
  const recordMatch = drift.length === 0;
  const iron = main.killedNoEq.length >= 1;
  const q8 = iron && main.controlsOk === main.controlSet.length && recordMatch;
  if (!iron) reasons.push('Q8: 鐵則不過(P-rule 零 killed)');
  if (main.controlsOk !== main.controlSet.length) reasons.push(`Q8: 控制組未全存活 ${numbers.controlsSurvived}`);
  if (!recordMatch) reasons.push(`Q8: DRIFT ${drift.length} 處(首:${drift[0]})`);
  if (eqVerifyFindings.length) reasons.push(`eq-verify: ${eqVerifyFindings.length} 處(${eqVerifyFindings.slice(0, 3).join(' | ')})`);

  // D 腿入庫門(§19.4.5;判「凍結 D 腿 tb.v 這個工件」的機械可測屬性,R1-3 劃界):
  // 形族底線=規則書出現的每個 form 族若有 eligible 歸屬變異,至少 1 個被殺;不劣於基線=同變異集上
  // D 腿殺傷率 ≥ T 腿殺傷率(兩腿 eligible 分母同一變異集時退化為 killed 數比較)。
  let formFloor = null;
  let notWorseBase = null;
  if (leg === 'directed' && base) {
    const perForm = {};
    let ok = true;
    for (const form of ['guard', 'transition', 'sequence', 'relation']) {
      const famIds = new Set(rules.filter((r) => r.check.form === form).map((r) => r.id));
      const fam = main.eligibleSet.filter((m) => m.ruleIds.some((id) => famIds.has(id)));
      if (fam.length === 0) continue;
      const hit = main.killedNoEq.filter((m) => m.ruleIds.some((id) => famIds.has(id))).length;
      perForm[form] = `${hit}/${fam.length}`;
      if (hit < 1) ok = false;
    }
    formFloor = { ok, perForm };
    if (!ok) reasons.push(`入庫門: 形族底線不過 ${JSON.stringify(perForm)}`);
    const dk = main.killedNoEq.length;
    const de = main.eligibleSet.length;
    const tk = base.killedNoEq.length;
    const te = base.eligibleSet.length;
    notWorseBase = de === 0 || te === 0 ? dk >= tk : dk * te >= tk * de;
    if (!notWorseBase) reasons.push(`入庫門: 不劣於基線不過(D ${dk}/${de} < T ${tk}/${te})`);
  }

  // 輔助腿(重寫產物;永不影響 exit,10.11)——在主腿 TB 下計分。
  const aux = [];
  const expBlocks = sceneBlocks(norm1);
  for (const a of cs.aux ?? []) {
    const files = a.files.map((f) => join(pluginRoot, f));
    if (!files.every((f) => existsSync(f))) {
      aux.push({ leg: a.leg, kind: a.kind, present: false });
      continue;
    }
    const r = iverilogRun({ flags, top: tbTop, files: [workTb, ...files], workDir: work, outName: `aux_${a.leg}.vvp`, timeoutMs: SIM_TIMEOUT_MS });
    if (!r.compiled) {
      aux.push({ leg: a.leg, kind: a.kind, present: true, compiled: false, matched: 0, total: expBlocks.size, mismatched: [] });
      continue;
    }
    const got = sceneBlocks(normalizeIverilog(r.stdout));
    const mism = [...expBlocks.keys()].filter((k) => JSON.stringify(expBlocks.get(k)) !== JSON.stringify(got.get(k) ?? null));
    aux.push({ leg: a.leg, kind: a.kind, present: true, compiled: true, matched: expBlocks.size - mism.length, total: expBlocks.size, mismatched: mism, timedOut: r.timedOut || undefined });
  }

  const qs = { Q1: q1, Q2: q2, Q3: q3, Q4: q4, Q5: q5, Q6: q6, Q7: q7, Q8: q8, Q9: q9 && waiveFindings.length === 0 };
  const dGate = leg !== 'directed' || (formFloor?.ok === true && notWorseBase === true);
  const tLegCheck = recordMode || tryTbPath || leg === 'directed' || tbText === frame.template_v;
  const admitted = Object.values(qs).every(Boolean) && deterministic && tLegCheck && dGate && !tbAlarm && eqFailures.length === 0 && eqVerifyFindings.length === 0 && specPinned;
  const baselineSummary = base
    ? { sha256: sha256(baselineTemplate), numbers: base.numbers, gainKilled: main.killedNoEq.length - base.killedNoEq.length, formFloor: formFloor?.perForm ?? null, notWorse: notWorseBase }
    : null;
  const verdict = {
    design,
    src: cs.src,
    top: cs.top,
    specPath: cs.specPath,
    rules: rules.length,
    tb: {
      file: tryTbPath ?? 'tb.v',
      leg,
      protocolLeg: cs.protocolLeg ?? '-',
      compiled: r1.compiled,
      warnings: r1.warnLines.length,
      scenesDeclared: lint.declaredScenes,
      scenesParsed: proto ? proto.scenes.length : 0,
      xClean: q4,
      deterministic,
      watchdog: q3 ? 'ok' : 'fail',
      crossSim,
      verilatorDiag: vlDiag,
      simMs,
    },
    positive: { matched: proto ? proto.scenes.length : 0, total: lint.declaredScenes ?? 0, pass: q2 },
    discrimination: { ...numbers, recordMatch },
    declaredCoverage: `${lint.coversNamed}/${rules.length}`,
    resetsEmpty: plan.resets.length === 0,
    baselineTemplate: baselineSummary,
    diagSelfcheck: loo ? { total: loo.total, hit: loo.hit, rate: loo.rate } : null,
    q: qs,
    aux,
    verdict: admitted ? 'ADMITTED' : 'REJECTED',
    reasons,
    drift: drift.slice(0, 20),
  };
  const anchors = anchorStats(srcText, rules);
  const mutantRec = (m) => ({
    id: m.id,
    key: m.key,
    pool: m.pool,
    op: m.op,
    line: m.line,
    before: m.before,
    after: m.after,
    detail: m.detail,
    ruleIds: m.ruleIds,
    ...(m.attribution ? { attribution: m.attribution } : {}),
    result: m.result,
    killScenes: m.killScenes,
    ...(m.adjudication ? { adjudication: m.adjudication } : {}),
    ...eqFieldsOf(m),
  });
  const record = {
    design,
    src: cs.src,
    top: cs.top,
    specPath: cs.specPath,
    rules: rules.length,
    catalog: { version: OPCAT_VERSION, priority: PRIORITY, perRuleBudget: 2, nopControls: 'first rule of each form' },
    provenance: {
      iverilog: tools.iverilog,
      verilator: tools.verilator,
      yosys: tools.yosys,
      wasmSha256: wasmSha,
      opcat: OPCAT_VERSION,
      date: new Date().toISOString().slice(0, 10),
    },
    tb: { leg, protocolLeg: cs.protocolLeg ?? '-', sha256: sha256(tbText), scenes: lint.declaredScenes, crossSim, verilatorDiag: vlDiag, simMs },
    expected: { sha256: sha256(norm1), lines: norm1.split('\n').length - 1 },
    numbers,
    ...(eqMeta ?? stored?.eqMeta ? { eqMeta: eqMeta ?? stored.eqMeta } : {}),
    waiveTable,
    anchors,
    stateTables: pm.stateTables,
    attempts: pm.attempts.filter((a) => a.status !== 'ok'),
    mutants: results.map(mutantRec),
    baselineTemplate: base
      ? {
          leg: 'template',
          sha256: sha256(baselineTemplate),
          expectedSha256: normT !== null ? sha256(normT) : null,
          numbers: base.numbers,
          gainKilled: main.killedNoEq.length - base.killedNoEq.length,
          formFloor: formFloor?.perForm ?? null,
          notWorse: notWorseBase,
          mutants: baseResults.map(mutantRec),
        }
      : null,
    rulebookDefects: {
      unruledKilled: main.U.filter(isKill).map((m) => ({ line: m.line, op: m.op, before: m.before.trim(), after: m.after.trim(), killScenes: m.killScenes })),
      defLint: dl,
      leaveOneOut: loo ? { total: loo.total, hit: loo.hit, rate: loo.rate } : null,
    },
  };
  return { design, verdict, record, stored, tbText, expected: norm1, iron, formFloor, notWorseBase, loo, dl };
}


// ---------- 證據頁(機械生成;不含易變時間戳/wasm sha/環境依賴輔助腿) ----------
// 默認模式取入庫 record 的錄製期欄(工具版本/跨模擬器結論),使本頁不隨環境(verilator/yosys 在否)漂移。
function renderReport(rows) {
  const recOf = (row) => (recordMode ? row.record : row.stored ?? row.record);
  const L = [];
  L.push('# rtl2spec conformance 證據頁(機械生成禁手改)');
  L.push('');
  L.push('- 命令行:`bash plugin/rtl2spec/run-conformance-eval.sh`(默認模式重生成本頁到 build/ 並 byte 比對;`--record --slice <C-Sn>` 寫本頁)');
  L.push('- 口徑:conformance 度量**行為復原力**(oracle=原版 RTL 在 iverilog 下的可觀測行為),**不是意圖正確性**;TB 全過≠設計正確(§19.1)。');
  L.push(`- 算子目錄:${OPCAT_VERSION}(選用序與每規則 ≤2 預算見 record.json catalog 欄)`);
  const prov = rows[0] ? recOf(rows[0]).provenance ?? {} : {};
  L.push(`- 工具鏈(錄製時):iverilog ${prov.iverilog ?? '-'};verilator ${prov.verilator ?? '-'};yosys ${prov.yosys ?? '-'}(僅錄製期存活裁決)`);
  L.push('');
  L.push('## 逐設計判據(Q1..Q9;§19.4.3)');
  L.push('');
  L.push('| 設計 | top | 規則 | 場景 | Q1 | Q2 | Q3 | Q4 | Q5 | Q6 | Q7 | Q8 | Q9 | 判定 |');
  L.push('|---|---|---|---|---|---|---|---|---|---|---|---|---|---|');
  const b = (x) => (x ? '✓' : '✗');
  for (const row of rows) {
    const v = row.verdict;
    const r = recOf(row);
    L.push(`| ${v.design} | ${v.top} | ${v.rules} | ${v.tb.scenesDeclared} | ${b(v.q.Q1)} | ${b(v.q.Q2)} | ${b(v.q.Q3)} | ${b(v.q.Q4)} | ${b(v.q.Q5)} | ${b(v.q.Q6)} | ${r.tb.crossSim === 'agree' ? '✓' : r.tb.crossSim} | ${b(v.q.Q8)} | ${b(v.q.Q9)} | ${v.verdict} |`);
  }
  L.push('');
  L.push('## 判別力(P-rule 池;§19.4.3 數字定義)');
  L.push('');
  L.push('| 設計 | generated | stillborn | equivalent | eligible | killed | survived | timeout | 殺傷率 | 規則驗證覆蓋 獨佔(共享) | 目錄覆蓋 | 聲明覆蓋 | 控制組存活 | ambiguous | P-unruled gen/killed |');
  L.push('|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|');
  for (const { verdict: v } of rows) {
    const d = v.discrimination;
    L.push(`| ${v.design} | ${d.generated} | ${d.stillborn} | ${d.equivalent} | ${d.eligible} | ${d.killed} | ${d.survived} | ${d.timeout} | ${d.rate} | ${d.ruleVerified.exclusive}(${d.ruleVerified.shared}) | ${d.catalogCoverage} | ${v.declaredCoverage} | ${d.controlsSurvived} | ${d.ambiguous} | ${d.unruled.generated}/${d.unruled.killed} |`);
  }
  L.push('');
  L.push('## D 腿/T 腿基線分列(§19.3.1:D 腿增益=D−T,禁合併;D 腿=凍結 LLM 產物,協議=CONFORMANCE-PROTOCOL.md)');
  L.push('');
  const dRows = rows.filter((row) => recOf(row).baselineTemplate);
  if (dRows.length === 0) {
    L.push('- (無 directed 設計;全部設計 tb.v=T 腿模板)');
  } else {
    L.push('| 設計 | 協議腿 | D 腿殺傷率 | T 腿基線殺傷率 | 增益(D−T killed) | D 獨佔(共享) | T 獨佔(共享) | 形族底線(D 腿) | 不劣於基線 |');
    L.push('|---|---|---|---|---|---|---|---|---|');
    for (const row of dRows) {
      const r = recOf(row);
      const b = r.baselineTemplate;
      L.push(`| ${r.design} | ${r.tb.protocolLeg ?? '-'} | ${r.numbers.rate} | ${b.numbers.rate} | ${b.gainKilled} | ${r.numbers.ruleVerified.exclusive}(${r.numbers.ruleVerified.shared}) | ${b.numbers.ruleVerified.exclusive}(${b.numbers.ruleVerified.shared}) | ${Object.entries(b.formFloor ?? {}).map(([f, v]) => `${f} ${v}`).join(';')} | ${b.notWorse ? '✓' : '✗'} |`);
    }
  }
  L.push('');
  L.push('## 機械註記');
  L.push('');
  for (const { verdict: v } of rows) {
    if (v.resetsEmpty) {
      L.push(`- ${v.design}:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。`);
    }
  }
  L.push('- 聲明覆蓋只計 COVERS 顯式點名的規則(模板非定向場景標 `*`,不折算為點名);驗證覆蓋以獨佔為主數、共享括注(禁合併)。');
  L.push('- 入庫 tb.v 禁手改:TB-SHA-ALARM=record.tb.sha256/磁碟 tb.v/本輪主腿 TB 三方核對,矛盾=REJECTED 或拒錄(exit 1);換檔唯一路徑=--record --promote(CONFORMANCE-PROTOCOL §5)。');
  L.push(`- WAIVE 標籤集=${WAIVE_LABELS.join('|')}(tblint WAIVE_LABELS;文法外/集外 Q9 硬紅;與協議模板最新版枚舉由 selfcheck 逐集合核對)。`);
  L.push('- 波形幀診斷腿=輔助證據,不入本頁任何判據(入口 `bash plugin/rtl2spec/run-frame-diag.sh`;條文 CONFORMANCE-PROTOCOL §8)。');
  L.push('');
  // 等價裁決與 WAIVE 表(§3;錄製期工件,默認模式讀入庫 record)。
  const eqRows = rows.filter((row) => recOf(row).eqMeta);
  if (eqRows.length) {
    const M0 = recOf(eqRows[0]).eqMeta;
    L.push('## 等價裁決(錄製期測試側 harness;協議 §7)');
    L.push('');
    L.push(`- 生產命令形:\`${M0.commandTemplate}\`;oracle=${M0.yosysVersion},僅測試用禁 ship,默認模式不跑。`);
    L.push(`- 雙向措辭:proven≠無條件行為等價(${M0.provenance});unproven≠不等價。eqverdict=equivalent|unproven|adjudication-failed;adjudication-failed 與 unproven 一律保守留在殺傷率分母。`);
    L.push('- 空真守衛:proven 且 `select -count t:$equiv` cells≥輸出端口位數才算 equivalent,否則 adjudication-failed(empty);Q8 接不住空真,本守衛是唯一防線。空真探針=獨立模板(全名單 blacklist),不屬生產管線。');
    L.push('');
    L.push('| 設計 | 端口位數 all/out | gold-vs-gold(cells) | 空真探針 | 裁決數 | equivalent | unproven | adjudication-failed | killed 全母體 總數(主腿) | 其中 unproven/failed/equivalent |');
    L.push('|---|---|---|---|---|---|---|---|---|---|');
    for (const row of eqRows) {
      const r = recOf(row);
      const M = r.eqMeta;
      const ks = M.killedSelfcheck;
      L.push(`| ${r.design} | ${M.ports.all}/${M.ports.out} | ${M.goldVsGold.eqverdict}(${M.goldVsGold.eqCells}) | ${M.emptyProbe.eqverdict}(${M.emptyProbe.eqReason}) | ${M.counts.adjudicated} | ${M.counts.equivalent} | ${M.counts.unproven} | ${M.counts.adjudicationFailed} | ${ks.total}(${ks.mainKilled}) | ${ks.unproven}/${ks.adjudicationFailed}/${ks.equivalent} |`);
    }
    L.push('');
    const fails = eqRows.flatMap((row) => (recOf(row).eqMeta.killedSelfcheck.failed ?? []).map((x) => `- ${recOf(row).design}:${x}`));
    if (fails.length) {
      L.push('killed 全母體裁決失敗項(這些變異 TB 已殺,yosys 判不了;不入 equivalent):');
      L.push('');
      for (const x of fails) L.push(x);
      L.push('');
    }
    L.push('## WAIVE 逐規則(標籤×裁決分布;數據謂詞=協議 §6;靜態標籤只可見化)');
    L.push('');
    const wr = eqRows.flatMap((row) => (recOf(row).waiveTable ?? []).map((w) => ({ design: recOf(row).design, ...w })));
    if (wr.length === 0) {
      L.push('- (無 WAIVE)');
    } else {
      L.push('| 設計 | 規則 | 標籤 | 變異 | killed | equivalent | unproven | adjudication-failed | 未裁決 | 數據謂詞 |');
      L.push('|---|---|---|---|---|---|---|---|---|---|');
      for (const w of wr) L.push(`| ${w.design} | ${w.ruleId} | ${w.tag} | ${w.mutants} | ${w.killed} | ${w.dist.equivalent} | ${w.dist.unproven} | ${w.dist['adjudication-failed']} | ${w.dist.unjudged} | ${w.dataTag} |`);
    }
    L.push('');
  }
  L.push('## P-unruled 被殺行(規則書未描述而 TB 可觀測=規則書缺陷的確定性證據;不入殺傷率分母)');
  L.push('');
  let any = false;
  for (const row of rows) {
    const r = recOf(row);
    for (const u of r.rulebookDefects.unruledKilled) {
      any = true;
      L.push(`- ${r.design} ${r.src}:${u.line} ${u.op}:\`${u.before}\` → \`${u.after}\`(殺傷:${u.killScenes.join(',')})`);
    }
  }
  if (!any) L.push('- (無)');
  L.push('');
  return L.join('\n');
}

/// tb.v 的 COVERS 註釋(診斷 covers 次級排序輸入;tblint 同一正則)。
function lintCovers(tbText) {
  return [...tbText.matchAll(/^\/\/ COVERS: ([0-9]+): (.+)$/gm)].map((m) => ({ n: Number(m[1]), ids: m[2].split(',').map((x) => x.trim()) }));
}

// ---------- 主流程 ----------
console.log(`== conformance-eval ${recordMode ? `--record --slice ${slice}${promote.size ? ` --promote ${[...promote.keys()].join(',')}` : ''}` : tryDesign !== null ? `--try ${tryDesign}` : '(default)'} ==`);
if (staging) console.log(`INFO staging 根=${rel(fixRoot)} 輸出=${rel(outRoot)}(主 fixtures 與主證據頁零觸碰;EXPECTED_DESIGNS/F3 不斷言;ADMITTED 看 verdict.json)`);
console.log(`INFO tools iverilog=${tools.iverilog} verilator=${tools.verilator ?? 'absent'} yosys=${tools.yosys ?? 'absent'} wasm=${wasmSha.slice(0, 16)}`);
selfcheck();
if (!staging && designs.length !== EXPECTED_DESIGNS) {
  ck(false, `設計目錄數==EXPECTED_DESIGNS(${EXPECTED_DESIGNS})`, `實得 ${designs.length}: ${designs.join(',')}`);
}
mkdirSync(outRoot, { recursive: true });
const rows = [];
const t0 = Date.now();
for (const d of selected) {
  const td = Date.now();
  const res = evalDesign(d, tryDesign !== null ? { tryTb: join(process.cwd(), tryTb) } : {});
  rows.push(res);
  const v = res.verdict;
  const ok = v.verdict === 'ADMITTED';
  const b = v.baselineTemplate;
  const legTxt = b ? ` [${v.tb.leg}] D ${v.discrimination?.rate} vs T ${b.numbers.rate} gain=${b.gainKilled} floor=${JSON.stringify(b.formFloor)} notWorse=${b.notWorse}` : '';
  ck(ok, `${d} ${v.verdict}`, ok ? `${v.discrimination.rate} killed/eligible, unruled ${v.discrimination.unruled.killed}/${v.discrimination.unruled.generated}, ${Date.now() - td}ms${legTxt}` : `${v.reasons.join(' | ')}${legTxt}`);
}
const verdicts = rows.map((r) => r.verdict);
writeFileSync(join(outRoot, 'verdict.json'), JSON.stringify(verdicts, null, 2) + '\n');
const auxLines = ['# 輔助腿計分(不影響 exit;10.11 盲寫腿=輔助證據)', ''];
for (const v of verdicts) for (const a of v.aux ?? []) auxLines.push(`- ${v.design} ${a.leg}: ${a.present ? (a.compiled ? `${a.matched}/${a.total}${a.mismatched.length ? ' 失配:' + a.mismatched.join(',') : ''}` : 'compiled=false') : '產物缺'}`);
writeFileSync(join(outRoot, 'aux.md'), auxLines.join('\n') + '\n');

const fullSet = selected.length === designs.length;
// F3 規則書缺陷報告(§19.6.2;確定性部分=P-unruled killed+def_lint U1/U4;F1 輔助部分由記錄承載)。
// 全集跑時生成:默認模式從入庫 record 讀(錄製時代),錄製模式從新錄 record 寫;部分跑不生成。
const defectsPath = join(outRoot, 'rulebook-defects');
if (fullSet) {
  const dl = [];
  for (const row of rows) {
    const r = (recordMode ? row.record : row.stored ?? row.record);
    for (const u of r.rulebookDefects?.unruledKilled ?? []) {
      dl.push({ design: r.design, src: r.src, kind: 'P-unruled-killed', line: u.line, op: u.op, before: u.before, after: u.after, killScenes: u.killScenes, note: 'TB 可觀測而規則書未描述=規則書缺陷的確定性證據(不入殺傷率分母)' });
    }
    for (const i of (r.rulebookDefects?.defLint?.items ?? []).filter((x) => !x.policy && (x.class === 'U1' || x.class === 'U4'))) {
      dl.push({ design: r.design, src: r.src, kind: `def_lint-${i.class}`, line: i.line, signal: i.signal, snippet: i.snippet, note: i.class === 'U1' ? 'net 宣告賦值未提取(extract_module `_ => {}` 臂;REGISTRY D-c1 同形)' : 'Index/IndexedPart 非 canon(§18.4-3 待決)' });
    }
    for (const i of (r.rulebookDefects?.defLint?.items ?? []).filter((x) => x.policy)) {
      dl.push({ design: r.design, kind: `def_lint-${i.class}`, line: i.line, signal: i.signal, policy: true, note: '政策排除(S2:直通別名/分支域外純常量不產規則),不記缺陷' });
    }
  }
  writeFileSync(`${defectsPath}.json`, JSON.stringify({ generatedBy: 'run-conformance-eval.sh', provenanceNote: '確定性部分=P-unruled killed+def_lint;F1 輔助部分見 CONFORMANCE-PROTOCOL §4 腿表(不進本檔)', items: dl }, null, 2) + '\n');
  const md = ['# rtl2spec 規則書缺陷報告(機械生成禁手改;§19.6.2 F3 確定性部分)', '', 'oracle 紀律:本報告是「TB 可觀測而規則書未描述」的確定性證據,不是意圖正確性判定(§19.1)。', ''];
  for (const d of dl) {
    md.push(`- [${d.design}] ${d.kind}${d.line ? ` ${d.src ?? ''}:${d.line}` : ''}: ${d.kind.startsWith('def_lint') ? (d.signal ?? '') + ' — ' + (d.snippet ?? '') : '`' + d.before + '` → `' + d.after + '`(殺傷:' + (d.killScenes ?? []).join(',') + ')'}${d.policy ? '(政策排除)' : ''} — ${d.note}`);
  }
  writeFileSync(`${defectsPath}.md`, md.join('\n') + '\n');
  // 斷言規格(手寫合法):i2c_master 缺陷必含 cnt_end(D-c1 確定性證據;REGISTRY 引用本行)。
  const i2c = dl.filter((x) => x.design === 'i2c_master' && x.kind === 'P-unruled-killed');
  if (!staging) ck(i2c.some((x) => (x.before ?? '').includes('cnt_end')), 'F3 斷言:i2c_master 缺陷含 cnt_end(net 宣告賦值洞 D-c1)', JSON.stringify(i2c.map((x) => `${x.op}@${x.line}`)));
  console.log(`INFO rulebook-defects: ${dl.length} 項 → ${rel(defectsPath)}.{json,md}`);
}
if (tryDesign !== null) {
  const td = join(outRoot, 'try');
  mkdirSync(td, { recursive: true });
  for (const r of rows) {
    writeFileSync(join(td, `${r.design}.verdict.json`), JSON.stringify(r.verdict, null, 2) + '\n');
    if (r.record) writeFileSync(join(td, `${r.design}.record.json`), JSON.stringify(r.record, null, 2) + '\n');
  }
  console.log(`INFO 候選評測只寫 ${rel(td)}/(fixtures 零觸碰;入庫=--record --slice <C-Sn> --promote <design>=<path>)`);
  console.log(`== conformance-eval: ${failures ? 'FAIL' : 'PASS'} ==`);
  process.exit(failures ? 1 : 0);
}
if (recordMode) {
  const blocked = rows.filter((r) => r.verdict.verdict !== 'ADMITTED');
  if (blocked.length) {
    console.log(`FAIL 拒錄:${blocked.map((r) => `${r.design}(${r.verdict.reasons.join(' | ')})`).join('; ')}`);
    process.exit(1);
  }
  const pre = join(outRoot, 'pre-record', slice);
  const diffL = [`# record-diff-${slice}(錄前快照 vs 新錄;逐設計對賬,§19.4.4)`, ''];
  for (const r of rows) {
    const dst = join(fixRoot, r.design);
    const snap = join(pre, r.design);
    rmSync(snap, { recursive: true, force: true });
    mkdirSync(snap, { recursive: true });
    const old = {};
    for (const f of ['tb.v', 'expected.txt', 'record.json']) {
      if (existsSync(join(dst, f))) {
        cpSync(join(dst, f), join(snap, f));
        old[f] = readFileSync(join(dst, f), 'utf8');
      }
    }
    if (r.record.tb.leg === 'template' || promote.has(r.design)) writeFileSync(join(dst, 'tb.v'), r.tbText);
    writeFileSync(join(dst, 'expected.txt'), r.expected);
    writeFileSync(join(dst, 'record.json'), JSON.stringify(r.record, null, 2) + '\n');
    // TB-SHA-ALARM 錄後核對:磁碟 tb.v 必==新 record.tb.sha256(下次默認模式的斷言基準)。
    const diskAfter = sha256(readFileSync(join(dst, 'tb.v'), 'utf8'));
    ck(diskAfter === r.record.tb.sha256, `${r.design} TB-SHA 錄後核對(磁碟 tb.v==新 record)`, diskAfter === r.record.tb.sha256 ? '' : `${diskAfter.slice(0, 12)}≠${r.record.tb.sha256.slice(0, 12)}`);
    diffL.push(`## ${r.design}`, '');
    if (!old['record.json']) {
      diffL.push(`- 錄前快照為空=全量新增:tb.v ${r.tbText.split('\n').length - 1} 行、expected.txt ${r.expected.split('\n').length - 1} 行、變異 ${r.record.mutants.length} 個`);
    } else {
      const oe = (old['expected.txt'] ?? '').split('\n');
      const ne = r.expected.split('\n');
      let changed = 0;
      for (let k = 0; k < Math.max(oe.length, ne.length); k++) if (oe[k] !== ne[k]) changed++;
      diffL.push(`- tb.v ${old['tb.v'] === r.tbText ? '不變' : '變更'};expected.txt 行級差 ${changed} 行`);
      const oldRec = JSON.parse(old['record.json']);
      const dlA = JSON.stringify(oldRec.rulebookDefects?.defLint ?? null);
      const dlB = JSON.stringify(r.record.rulebookDefects?.defLint ?? null);
      if (dlA !== dlB) diffL.push(`- defLint 漂移:${dlA.slice(0, 120)} → ${dlB.slice(0, 120)}`);
      const loA = oldRec.rulebookDefects?.leaveOneOut?.rate ?? null;
      const loB = r.record.rulebookDefects?.leaveOneOut?.rate ?? null;
      if (loA !== loB) diffL.push(`- 留一法命中率 ${loA} → ${loB}`);
      const om = new Map(oldRec.mutants.map((m) => [m.key, m]));
      const nm = new Map(r.record.mutants.map((m) => [m.key, m]));
      for (const [k, m] of nm) {
        const o = om.get(k);
        if (!o) diffL.push(`- 新增 ${m.id} ${m.op}@${m.line} ${m.result}`);
        else if (o.result !== m.result || JSON.stringify(o.killScenes) !== JSON.stringify(m.killScenes) || o.adjudication !== m.adjudication) diffL.push(`- 變更 ${m.id} ${m.op}@${m.line}: ${o.result}/${o.adjudication ?? '-'}→${m.result}/${m.adjudication ?? '-'}`);
      }
      for (const [k, o] of om) if (!nm.has(k)) diffL.push(`- 移除 ${o.id} ${o.op}@${o.line}`);
      diffL.push(`- 數字:${JSON.stringify(oldRec.numbers)} → ${JSON.stringify(r.record.numbers)}`);
      // WAIVE 標籤使用數變化量(強制打印,§3.5 可見化;新增靜態標籤須在切片記錄理由)。
      const tc0 = tagCounts(waiveEntriesOf(old['tb.v'] ?? '').entries);
      const tc1 = tagCounts(waiveEntriesOf(r.tbText).entries);
      diffL.push(`- WAIVE 標籤使用數:${WAIVE_LABELS.map((t) => `${t} ${tc0[t] ?? 0}→${tc1[t] ?? 0}`).join(';')}`);
      // eqverdict 對賬:舊版 record 無 eqverdict=首錄;否則逐變異列變更。
      const eqCount = (rs) => {
        const c = {};
        for (const m of rs) if (m.eqverdict) c[m.eqverdict] = (c[m.eqverdict] ?? 0) + 1;
        return c;
      };
      if (!oldRec.eqMeta) diffL.push(`- eqverdict 首錄:${JSON.stringify(eqCount(r.record.mutants))}(舊版 record 無 eqverdict;adjudication 兩值變更逐條見上)`);
      else {
        const eqCh = [...nm].filter(([k, m]) => om.get(k) && (om.get(k).eqverdict ?? null) !== (m.eqverdict ?? null)).map(([k, m]) => `${m.id} ${om.get(k).eqverdict ?? '-'}→${m.eqverdict ?? '-'}`);
        diffL.push(`- eqverdict:${JSON.stringify(eqCount(oldRec.mutants))} → ${JSON.stringify(eqCount(r.record.mutants))};變更 ${eqCh.length}${eqCh.length ? '(' + eqCh.slice(0, 12).join(', ') + ')' : ''}`);
      }
      const nb = r.record.baselineTemplate;
      if (nb && oldRec.tb?.leg === 'template') {
        // T 腿轉基線腿(§19.9 C-S4):舊主腿(T)逐變異==新 baselineTemplate 才算同一 T 腿。
        const bm = new Map(nb.mutants.map((m) => [m.key, m]));
        let same = 0;
        const diffs = [];
        for (const o of oldRec.mutants) {
          const b = bm.get(o.key);
          if (b && b.result === o.result && JSON.stringify(b.killScenes) === JSON.stringify(o.killScenes)) same++;
          else diffs.push(`${o.id} ${o.op}@${o.line} ${o.result}→${b ? b.result : '<absent>'}`);
        }
        diffL.push(`- T 腿轉基線腿:舊主腿 tb.sha=${oldRec.tb.sha256.slice(0, 12)} vs 新基線 sha=${nb.sha256.slice(0, 12)}(${oldRec.tb.sha256 === nb.sha256 ? '同' : '異'});逐變異結果相同 ${same}/${oldRec.mutants.length}${diffs.length ? ';差異:' + diffs.slice(0, 10).join(', ') : ''}`);
        const gains = r.record.mutants.filter((m) => (m.result === 'killed' || m.result === 'timeout') && !['killed', 'timeout'].includes(bm.get(m.key)?.result)).map((m) => `${m.id} ${m.op}@${m.line}`);
        const losses = r.record.mutants.filter((m) => !['killed', 'timeout'].includes(m.result) && ['killed', 'timeout'].includes(bm.get(m.key)?.result)).map((m) => `${m.id} ${m.op}@${m.line}`);
        diffL.push(`- D 腿相對基線:新殺 ${gains.length}${gains.length ? '(' + gains.slice(0, 12).join(', ') + (gains.length > 12 ? ', …' : '') + ')' : ''};丟殺 ${losses.length}${losses.length ? '(' + losses.join(', ') + ')' : ''}`);
      }
    }
    diffL.push('');
  }
  writeFileSync(join(outRoot, `record-diff-${slice}.md`), diffL.join('\n'));
  if (fullSet) writeFileSync(reportPath, renderReport(rows));
  else console.log('INFO 部分設計錄製:證據頁未重寫(需全集錄製)');
  console.log(`INFO 錄前快照=${rel(pre)};record-diff=${rel(join(outRoot, `record-diff-${slice}.md`))}`);
} else if (fullSet) {
  const page = renderReport(rows);
  writeFileSync(join(outRoot, 'conformance-report.md'), page);
  const stored = existsSync(reportPath) ? readFileSync(reportPath, 'utf8') : null;
  ck(stored === page, '證據頁 conformance-report.md 與重生成 byte 相等', stored === null ? '缺' : stored === page ? '' : '陳舊或手改');
}

const sum = (f) => verdicts.reduce((a, v) => a + (v.discrimination ? f(v.discrimination) : 0), 0);
const admittedN = verdicts.filter((v) => v.verdict === 'ADMITTED').length;
console.log(
  `SUMMARY conformance designs=${verdicts.length}/${staging ? 'staging' : EXPECTED_DESIGNS} admitted=${admittedN} mutants=${sum((d) => d.generated)} eligible=${sum((d) => d.eligible)} killed=${sum((d) => d.killed)} equivalent=${sum((d) => d.equivalent)} unruled_killed=${sum((d) => d.unruled.killed)} elapsed_ms=${Date.now() - t0}`,
);
const eqRecs = rows.map((row) => (recordMode ? row.record : row.stored ?? row.record)).filter((r) => r?.eqMeta);
if (eqRecs.length) {
  const es = (f) => eqRecs.reduce((a, r) => a + f(r.eqMeta), 0);
  console.log(`SUMMARY-EQ designs=${eqRecs.length} adjudicated=${es((m) => m.counts.adjudicated)} equivalent=${es((m) => m.counts.equivalent)} unproven=${es((m) => m.counts.unproven)} adjudication_failed=${es((m) => m.counts.adjudicationFailed)} killed_selfcheck total=${es((m) => m.killedSelfcheck.total)} main=${es((m) => m.killedSelfcheck.mainKilled)} unproven=${es((m) => m.killedSelfcheck.unproven)} failed=${es((m) => m.killedSelfcheck.adjudicationFailed)} equivalent=${es((m) => m.killedSelfcheck.equivalent)}`);
}
console.log(`== conformance-eval: ${failures ? 'FAIL' : 'PASS'} ==`);
process.exit(failures ? 1 : 0);
