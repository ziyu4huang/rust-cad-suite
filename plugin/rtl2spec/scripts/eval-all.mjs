// eval-all.mjs — 真實評測擴面執行器(§16.4;D16 分層)。
//
// L1 質量層 93 檔=yosys-simple 全 87(ISC;COPYING 隨附)+repo 自有 6
//   (corpus MIT/spec-i2c MIT 乾淨室/xls Apache-2.0 派生載體)——全部跑
//   提取+破口掃描。L2 容錯層 260 檔=corpus-external/sv-tests 全量(ISC;
//   LICENSE 隨附)——只斷言:可終止+零 trap+JSON 可解析+parse_errors 如實
//   入 verdict(193 檔 parse_err=已知事實記錄不 FAIL,SV-2009 構造超
//   parser 閘)。**zipcpu=GPL 永不入;sv-elab 不在庫**(§16.4 排除記錄)。
//
// per-file 三檔分類(判據鏈;V1/V2/V3 為報告列不設單檔 FAIL——擴面集的
//   失敗條件=結構性違例而非產量,規劃理由在案):
//   skip-perr   parse_errors>0 → SKIP(計數如實);
//   skip-tb     TB 檔 → SKIP;
//   design-rules    parse_ok+非 TB+規則數≥3 → V4+V5-①②④ 判據 → PASS/FAIL;
//   design-sparse   parse_ok+非 TB+規則數<3 → 終止性+JSON 可解析+V4
//                   (sparse 檔 V4 違規照樣 FAIL——溯源與產量無關;
//                   V5 只記錄不設 FAIL) → OK-sparse/FAIL。
// 判據形=check-rulebook.mjs V4/V5 同源(獨立重算,不信任生成端)。
//
// 閘級硬 FAIL(任一即整閘紅;由 run-realworld-eval.sh 以本器 exit 1 執法):
//   ①任何檔 wasm trap(終止性防線=F2 修復+trap 計數,R16-3;單進程同步
//     wasm 循環**無計時器守衛**——真掛死時進程內計時器無法觸發,禁宣稱
//     超時防線)②JSON 不可解析 ③design-rules 檔 V4/V5-①②④ 敗
//   ④L1 全量 V5-③ 指紋重複 ⑤成功呼叫數≠檔數。
//
// verdict 寫 build/eval/<slug>.json(gitignored);本器不生成報告
// (gen-eval-report.mjs 職)。exit:0=閘級全過;1=閘級敗;2=語料缺。
import { readFileSync, readdirSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join, dirname, basename } from 'node:path';
import { fileURLToPath } from 'node:url';

const pluginRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const repoRoot = dirname(dirname(pluginRoot));

const wasmDir = join(pluginRoot, 'dist', 'wasm');
const glue = await import(join(wasmDir, 'rtl_spec_wasm.js'));
await glue.default({
  module_or_path: new Uint8Array(readFileSync(join(wasmDir, 'rtl_spec_wasm_bg.wasm'))),
});

// ---- 語料清單(現場目錄掃描,防清單 stale;數=git ls-files 同源) ----
// sv-tests 檔=.sv(2026-11-04 首錄實錄:初版只濾 .v 致 L2=0,EVAL_EXIT
// 0 假綠被「L2=0 全集=93」摘要行當場抓出——成功呼叫=檔數判據只對已列
// 檔生效,清單本身短收不攔;故分層統計行的 L1/L2/全集數也要人讀對帳)。
const listV = (dir, exts = ['.v']) =>
  readdirSync(join(repoRoot, dir))
    .filter((f) => exts.some((e) => f.endsWith(e)))
    .sort()
    .map((f) => `${dir}/${f}`);
const L1 = [
  ...listV('corpus-external/yosys-simple'),
  'corpus/e2e/59_i2c_master/i2c_master.v',
  'crates/spec-i2c/rtl/i2c_master.v',
  'corpus/e2e/58_xls_acc_pipe/acc_pipe.v',
  'corpus/e2e/57_xls_add3/add3.v',
  'corpus/fulladd/fulladd.v',
  'corpus/counter8/counter.v',
];
const L2 = listV('corpus-external/sv-tests', ['.sv']);
const ALL = [...L1, ...L2];
const missing = ALL.filter((f) => !existsSync(join(repoRoot, f)));
if (missing.length > 0) {
  console.error(`FAIL: 語料缺 ${missing.length} 檔:\n  ${missing.slice(0, 10).join('\n  ')}${missing.length > 10 ? '\n  …' : ''}`);
  process.exit(2);
}
console.log(`== eval-all: L1=${L1.length} L2=${L2.length} 全集=${ALL.length} ==`);

// ---- 判據(V4/V5 與 check-rulebook.mjs 同源;獨立重算) ----
const FORM_FIELDS = {
  guard: (c) => Array.isArray(c.effects) && c.effects.length > 0 && c.effects.every((e) => e.lhs && e.value),
  transition: (c) => c.state_reg && c.from && c.to,
  sequence: (c) => c.signal && c.becomes && c.anchor?.expr,
  relation: (c) =>
    c.lhs && c.rhs && (c.when == null || (typeof c.when === 'string' && c.when.length > 0)),
  invariant: (c) => c.subject && ((c.values ?? []).length > 0 || c.expr),
};
const FORMS = new Set(Object.keys(FORM_FIELDS));
const BEH_CLASSES = new Set(['process', 'assign', 'case', 'generate', 'expr']);

function judgeRules(rules, srcLines, name, srcIdents, { includeV5 }) {
  const failures = [];
  const fail = (check, detail) => failures.push({ check, detail });
  for (const r of rules) {
    if (includeV5) {
      if (!r.evidence?.some((e) => BEH_CLASSES.has(e.class))) {
        fail('V5-behaviour-evidence', `${r.id} 無行為類 evidence`);
      }
      if (!FORMS.has(r.check?.form) || !FORM_FIELDS[r.check?.form]?.(r.check ?? {})) {
        fail('V5-form-fields', `${r.id} form=${r.check?.form} 欄位不全`);
      }
      const stmtIdents = (r.statement.match(/[A-Za-z_][A-Za-z0-9_$]*/g) ?? []).filter((x) =>
        srcIdents.has(x),
      );
      if (stmtIdents.length === 0) {
        fail('V5-statement-identifier', `${r.id} 陳述無源碼標識符:${r.statement.slice(0, 40)}`);
      }
    }
    for (const e of r.evidence ?? []) {
      if (e.line === 0) {
        fail('V4-rtlil-evidence-in-sv-file', `${r.id} line=0(非 SV 證形)`);
        continue;
      }
      if (e.file !== name) {
        fail('V4-file', `${r.id} evidence.file=${e.file} != ${name}`);
        continue;
      }
      if (e.line < 1 || e.end_line < e.line || e.end_line > srcLines.length) {
        fail('V4-range', `${r.id} lines ${e.line}-${e.end_line} 越界(檔 ${srcLines.length} 行)`);
        continue;
      }
      const want = srcLines.slice(e.line - 1, e.end_line).join('\n');
      if (e.snippet !== want) {
        fail('V4-snippet', `${r.id} ${e.file}:${e.line}-${e.end_line} snippet 與源行不對`);
      }
    }
  }
  return failures;
}

// ---- 主循環(單 bun 進程;同步 wasm 調用;trap=try/catch 計數) ----
const outDir = join(pluginRoot, 'build', 'eval');
mkdirSync(outDir, { recursive: true });
const t0 = Date.now();
let traps = 0;
let parseFail = 0;
let successCalls = 0;
const verdicts = [];
const fingerprints = new Set();
let fpDup = 0;

for (const f of ALL) {
  const src = readFileSync(join(repoRoot, f), 'utf8');
  const srcLines = src.split('\n').map((l) => l.replace(/\r$/, ''));
  const srcIdents = new Set(src.match(/[A-Za-z_][A-Za-z0-9_$]*/g) ?? []);
  const verdict = { file: f, name: basename(f), layer: L1.includes(f) ? 'L1' : 'L2', failures: [] };
  let rb = null;
  let gr = null;
  try {
    rb = JSON.parse(glue.rtl_extract_rules_sv(f, src, 256));
    gr = JSON.parse(glue.rtl_scan_gaps_sv(f, src));
    successCalls++;
  } catch (e) {
    traps++;
    verdict.trap = String(e).slice(0, 200);
    verdict.judgement = 'FAIL';
    verdicts.push(verdict);
    console.log(`TRAP ${f}: ${String(e).slice(0, 100)}`);
    writeFileSync(join(outDir, `${f.replace(/[^A-Za-z0-9._-]+/g, '__')}.json`), JSON.stringify(verdict, null, 2));
    continue;
  }
  const rules = (rb.modules ?? []).flatMap((m) => m.rules ?? []);
  const isTbSkipped = (rb.skipped ?? []).some((n) => n.reason === 'tb_file');
  const parseErrCount = (rb.parse_errors ?? []).length;
  Object.assign(verdict, {
    lineCount: srcLines.length,
    moduleCount: (rb.modules ?? []).length,
    ruleCount: rules.length,
    parseErrorCount: parseErrCount,
    isTbSkipped,
    gapCount: (gr.items ?? []).length,
    gapsByClass: gr.items.reduce((acc, g) => ((acc[g.class] = (acc[g.class] ?? 0) + 1), acc), {}),
    categoryCounts: rules.reduce((acc, r) => ((acc[r.category] = (acc[r.category] ?? 0) + 1), acc), {}),
    truncated: rb.truncated ?? [],
  });
  for (const r of rules) {
    if (fingerprints.has(r.fingerprint)) fpDup++;
    fingerprints.add(r.fingerprint);
  }
  if (parseErrCount > 0) {
    verdict.judgement = 'skip-perr';
  } else if (isTbSkipped) {
    verdict.judgement = 'skip-tb';
  } else if (rules.length >= 3) {
    verdict.judgement = 'design-rules';
    verdict.failures = judgeRules(rules, srcLines, f, srcIdents, { includeV5: true });
    verdict.ok = verdict.failures.length === 0;
  } else {
    verdict.judgement = 'design-sparse';
    verdict.failures = judgeRules(rules, srcLines, f, srcIdents, { includeV5: false });
    verdict.ok = verdict.failures.length === 0;
  }
  verdicts.push(verdict);
  writeFileSync(
    join(outDir, `${f.replace(/[^A-Za-z0-9._-]+/g, '__')}.json`),
    JSON.stringify({ ...verdict, rulebook: rb, gaps: gr.items ?? [] }, null, 2),
  );
}

const elapsedMs = Date.now() - t0;
const byJ = {};
for (const v of verdicts) byJ[v.judgement] = (byJ[v.judgement] ?? 0) + 1;

// ---- 閘級硬 FAIL ----
let gateFail = 0;
const gateFailures = [];
if (traps > 0) { gateFailures.push(`trap×${traps}(終止性防線破)`); gateFail = 1; }
if (parseFail > 0) { gateFailures.push('JSON 不可解析'); gateFail = 1; }
const drFail = verdicts.filter((v) => v.judgement === 'design-rules' && !v.ok);
if (drFail.length > 0) { gateFailures.push(`design-rules V4/V5 敗×${drFail.length}`); gateFail = 1; }
const sparseFail = verdicts.filter((v) => v.judgement === 'design-sparse' && !v.ok);
if (sparseFail.length > 0) { gateFailures.push(`design-sparse V4 敗×${sparseFail.length}`); gateFail = 1; }
if (fpDup > 0) { gateFailures.push(`V5-③ 指紋重複×${fpDup}(L1+L2 全量)`); gateFail = 1; }
if (successCalls !== ALL.length) { gateFailures.push(`成功呼叫 ${successCalls}≠${ALL.length}(V6 覆蓋面)`); gateFail = 1; }

console.log(
  `== eval-all: ${byJ['design-rules'] ?? 0} design-rules / ${byJ['design-sparse'] ?? 0} sparse / ${byJ['skip-perr'] ?? 0} skip-perr / ${byJ['skip-tb'] ?? 0} skip-tb / trap=${traps} / 指紋 ${fingerprints.size} unique(dup=${fpDup}) / ${elapsedMs}ms ==`,
);
if (gateFail) console.log(`== eval-all GATE FAIL: ${gateFailures.join('; ')} ==`);
else console.log('== eval-all GATE PASS ==');
writeFileSync(
  join(outDir, '_summary.json'),
  JSON.stringify({ l1: L1.length, l2: L2.length, total: ALL.length, successCalls, traps, fpDup, totalRulesFp: fingerprints.size, elapsedMs, byJ, gateFailures, wasmSha256: createHash('sha256').update(readFileSync(join(wasmDir, 'rtl_spec_wasm_bg.wasm'))).digest('hex'), bun: process.version, abi: glue.rtl_spec_version() }, null, 2),
);
process.exit(gateFail ? 1 : 0);
