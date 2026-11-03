// gen-report.mjs — validation-report.md 生成 + 全集判據(§8.2 ④;§8.3/§8.4)。
// 讀 build/verdicts/*.json(check-rulebook.mjs 逐檔產出),執行全集面:
//   V1-total Σrules ≥50;V2-total 行為占比 ≥60%(冗餘防線,R1-7);
//   V3 distinct category ≥4(floor;目標 ≥6,報告實值);
//   V5-③ 鍵串(含 source 維)全集零重複;
//   V6 期望 13 檔全部在場且 ok(skipped 檔報告單列,不混入 FAIL 歸因)。
// 退出碼:0=全過;1=任一判據 FAIL;2=verdicts 缺。
// 機械生成禁手改;入庫=證據頁。

import { readFileSync, readdirSync, writeFileSync, existsSync, statSync } from 'node:fs';
import { join, dirname, basename } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderRulebookMarkdown, renderGapsMarkdown, sumRules } from '../dist/md.mjs';

const pluginRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const verdictDir = join(pluginRoot, 'build', 'verdicts');
const repoRoot = join(pluginRoot, '..', '..');

// 驗證集(13 檔;§8.3 license 表——機械複製進報告)。
const EXPECTED = [
  { path: 'corpus/e2e/59_i2c_master/i2c_master.v', license: 'repo MIT', tag: 'i2c' },
  { path: 'crates/spec-i2c/rtl/i2c_master.v', license: 'repo MIT(乾淨室;OpenCores 唯讀啟發零拷貝)', tag: 'i2c' },
  { path: 'corpus/e2e/58_xls_acc_pipe/acc_pipe.v', license: 'Apache-2.0', tag: 'xls' },
  { path: 'corpus/e2e/57_xls_add3/add3.v', license: 'Apache-2.0', tag: 'xls' },
  { path: 'corpus-external/yosys-simple/subbytes.v', license: 'ISC(COPYING 隨附;唯讀引用,原檔不入 plugin/)', tag: 'yosys' },
  { path: 'corpus-external/yosys-simple/fiedler-cooley.v', license: 'ISC', tag: 'yosys' },
  { path: 'corpus-external/yosys-simple/sincos.v', license: 'ISC', tag: 'yosys' },
  { path: 'corpus-external/yosys-simple/multiplier.v', license: 'ISC', tag: 'yosys' },
  { path: 'corpus-external/yosys-simple/memory.v', license: 'ISC', tag: 'yosys' },
  { path: 'corpus-external/yosys-simple/carryadd.v', license: 'ISC', tag: 'yosys' },
  { path: 'corpus-external/yosys-simple/generate.v', license: 'ISC', tag: 'yosys' },
  { path: 'corpus-external/yosys-simple/mem2reg.v', license: 'ISC', tag: 'yosys' },
  { path: 'corpus-external/yosys-simple/dff_different_styles.v', license: 'ISC', tag: 'yosys' },
];

let failures = 0;
function failCheck(name, detail) {
  console.log(`FAIL ${name} — ${detail}`);
  failures++;
}
function passCheck(name, detail = '') {
  console.log(`PASS ${name}${detail ? ` — ${detail}` : ''}`);
}

if (!existsSync(verdictDir)) {
  console.log('FAIL verdicts dir missing(先跑逐檔 check-rulebook.mjs)');
  process.exit(2);
}

// ---- 載入 verdicts(以 repo 相對路徑為 key) ----
const byPath = new Map();
for (const f of readdirSync(verdictDir)) {
  if (!f.endsWith('.json')) continue;
  const v = JSON.parse(readFileSync(join(verdictDir, f), 'utf8'));
  // verdict.file 是 run-rtl-validation.sh 傳入的路徑(cwd=repo root)。
  byPath.set(v.file.replace(/^\.\/+/, ''), v);
}

// V6:期望檔全在場。
const missing = EXPECTED.filter((e) => !byPath.has(e.path));
if (missing.length > 0) {
  failCheck('V6-coverage', `缺 verdict:${missing.map((m) => m.path).join(', ')}`);
}

const verdicts = EXPECTED.map((e) => byPath.get(e.path)).filter(Boolean);
const allRules = verdicts.flatMap((v) => (v.rulebook?.modules ?? []).flatMap((m) => m.rules ?? []));
const total = sumRules({ modules: verdicts.flatMap((v) => v.rulebook?.modules ?? []) });
const behaviourCount = allRules.filter((r) => r.category !== 'param' && r.category !== 'structure').length;
const allCategories = [...new Set(allRules.map((r) => r.category))];

// ---- 全集判據 ----
let v1Total = total >= 50;
v1Total ? passCheck('V1-total', `Σrules=${total} ≥ 50`) : failCheck('V1-total', `Σrules=${total} < 50`);

const ratio = total === 0 ? 0 : behaviourCount / total;
ratio >= 0.6
  ? passCheck('V2-total', `行為占比 ${(ratio * 100).toFixed(1)}% ≥ 60%(冗餘防線定位,R1-7)`)
  : failCheck('V2-total', `行為占比 ${(ratio * 100).toFixed(1)}% < 60%`);

const catOk = allCategories.length >= 4;
catOk
  ? passCheck('V3-categories', `${allCategories.length} 類 ≥ 4(floor;目標 ≥6):${allCategories.join('/')}`)
  : failCheck('V3-categories', `${allCategories.length} 類 < 4`);

const fps = new Set(allRules.map((r) => r.fingerprint));
fps.size === allRules.length
  ? passCheck('V5-fingerprint-dedup', `${fps.size}/${allRules.length} 零重複(含 source 維)`)
  : failCheck('V5-fingerprint-dedup', `${allRules.length - fps.size} 個重複指紋`);

const perFileOk = verdicts.every((v) => v.ok);
perFileOk ? passCheck('per-file checks', `${verdicts.length} 檔全過`) : failCheck('per-file checks', verdicts.filter((v) => !v.ok).map((v) => `${v.file}(${v.failures?.length})`).join(', '));

v1Total && ratio >= 0.6 && catOk && fps.size === allRules.length && perFileOk && missing.length === 0
  ? passCheck('V6-13-files', `${verdicts.length}/13 成功呼叫(skipped 檔報告單列)`)
  : null;

// ---- V4/V5 實測列由 checker failures 推導(R3-③:禁硬編碼通過數——
// 閘紅場景判定列正確而實測列全過=證據頁自相矛盾)。failure.detail 首詞
// =規則 id(V4/V5 斷言均以 id 開頭),命中規則數按 id 去重。
const failsOf = (prefix) =>
  verdicts.flatMap((v) => (v.failures ?? []).filter((f) => f.check.startsWith(prefix)));
const rulesHitBy = (prefix) =>
  new Set(failsOf(prefix).map((f) => f.detail.split(' ')[0])).size;
const v4Hit = rulesHitBy('V4');
const v5Hit = rulesHitBy('V5');
const v4Verdict =
  verdicts.every((v) => !v.failures?.some((f) => f.check.startsWith('V4'))) ? 'PASS' : 'FAIL';
const v5Verdict =
  fps.size === allRules.length &&
  verdicts.every((v) => !v.failures?.some((f) => f.check.startsWith('V5')))
    ? 'PASS'
    : 'FAIL';
const v4Measured = v4Hit === 0
  ? `${allRules.length}/${allRules.length} 規則過(checker 獨立重讀源檔對拍)`
  : `${allRules.length - v4Hit}/${allRules.length} 規則過,${v4Hit} 條規則 V4 違規(checker 獨立重讀源檔對拍)`;
const v5Measured = v5Hit === 0
  ? `①②③④ 全過:${fps.size}/${allRules.length} 指紋零重複`
  : `V5 違規 ${v5Hit} 條規則(共 ${failsOf('V5').length} 條斷言);指紋 ${fps.size}/${allRules.length}`;
// 自洽斷言(R3-③):判定 FAIL 的行,實測列不得宣稱全過。實測列與判定
// 同源於 failures;此斷言守「實測列手改回硬編碼全過」的回歸。
for (const [tag, verdictStr, measured] of [
  ['V4', v4Verdict, v4Measured],
  ['V5', v5Verdict, v5Measured],
]) {
  if (verdictStr === 'FAIL' && !measured.includes('違規')) {
    failCheck(`self-consistency-${tag}`, `判定 FAIL 而實測列無失敗表達(全過形):${measured}`);
  }
}

if (failures > 0) {
  console.log(`GEN-REPORT: ${failures} global failure(s) — 報告仍生成(exit 1)`);
}

// ---- 報告生成(§8.4) ----
const wasmSha = verdicts[0]?.wasmSha256 ?? 'n/a';
const today = new Date().toISOString().slice(0, 10);
const tbSkippedCount = verdicts.filter((v) => v.isTbSkipped).length;
const lines = [];
lines.push('# rtl2spec 真實 RTL 驗證報告');
lines.push('');
lines.push(`- 日期:${today}`);
lines.push('- 命令行:`bash plugin/rtl2spec/run-rtl-validation.sh`(機械生成禁手改)');
lines.push(`- 工具鏈:bun ${process.env.RTL2SPEC_BUN_VERSION ?? '(見閘輸出)'}、wasm sha256 \`${wasmSha}\``);
lines.push('- 本檔=閘契約 V1..V6 的證據頁;評審員區外全部機械生成。');
lines.push('');
lines.push('## 閘契約表(V1..V6)');
lines.push('');
lines.push('| 判據 | 閾值 | 實測 | 判定 |');
lines.push('|---|---|---|---|');
lines.push(`| V1 規則數 | 全集 ≥50;單檔 ≥3;truncated 非空→FAIL | 全集 ${total};單檔最小 ${Math.min(...verdicts.filter((v) => !v.isTbSkipped).map((v) => v.ruleCount))};截斷模組 0 | ${v1Total ? 'PASS' : 'FAIL'} |`);
lines.push(`| V2 行為占比 | 全集 ≥60%;任一檔 ≥40%(冗餘防線,主判據=V5-①+V4,R1-7) | 全集 ${(ratio * 100).toFixed(1)}%;單檔最小 ${(Math.min(...verdicts.map((v) => v.behaviourRatio)) * 100).toFixed(1)}% | ${ratio >= 0.6 && verdicts.every((v) => v.behaviourRatio >= 0.4) ? 'PASS' : 'FAIL'} |`);
lines.push(`| V3 類別覆蓋 | ≥4(floor,ask 契約;目標 ≥6) | ${allCategories.length} 類:${allCategories.join('/')} | ${catOk ? 'PASS' : 'FAIL'} |`);
lines.push(`| V4 溯源 | SV 側 100% 帶 file:line 且 snippet 逐位元組相等(拼接規則=各行原文 \\n 連接,R1-10) | ${v4Measured} | ${v4Verdict} |`);
lines.push(`| V5 反 doc-dump | ①行為證 ②form 五形+欄位非空(relation when 在場必非空,R3-②) ③鍵串零重複 ④陳述含源碼標識符(②④=冗餘防線) | ${v5Measured} | ${v5Verdict} |`);
lines.push(`| V6 全檔成功 | 13/13 | ${verdicts.length}/13(本輪 skipped=${tbSkippedCount}) | ${verdicts.length === 13 ? 'PASS' : 'FAIL'} |`);
lines.push('');

const totalGaps = verdicts.reduce((n, v) => n + (v.gapCount ?? 0), 0);
lines.push(`規則總數 ${total}(行為類 ${behaviourCount});破口總數 ${totalGaps};主判據聲明=V5-①(行為證)+V4(逐位元組溯源),V2/V5-④ 為守生成端鎖的冗餘防線(R1-7 定位聲明)。`);
lines.push('');
lines.push('## license 表(驗證集 13 檔;全 in-git,零 GPL)');
lines.push('');
lines.push('| 檔案 | license |');
lines.push('|---|---|');
for (const e of EXPECTED) lines.push(`| ${e.path} | ${e.license} |`);
lines.push('');
lines.push('## 逐檔表');
lines.push('');
lines.push('| 檔案 | 行數 | 模組數 | 規則數 | 類別分布 | 破口數(按類) | 判定 |');
lines.push('|---|---|---|---|---|---|---|');
for (const v of verdicts) {
  const cats = Object.entries(v.categoryCounts ?? {})
    .map(([k, n]) => `${k}=${n}`)
    .join(' ');
  const gaps = Object.entries(v.gapsByClass ?? {})
    .map(([k, n]) => `${k}=${n}`)
    .join(' ');
  lines.push(`| ${v.file} | ${v.lineCount} | ${v.moduleCount} | ${v.ruleCount} | ${cats} | ${gaps || '0'} | ${v.ok ? 'PASS' : 'FAIL'} |`);
}
lines.push('');

// 規則書全文樣本(≥3 代表檔:≥1 yosys、i2c_master、≥1 xls)。
const samples = [
  { label: 'i2c_master(corpus/e2e/59,repo MIT)', path: 'corpus/e2e/59_i2c_master/i2c_master.v' },
  { label: 'carryadd(yosys-simple,ISC;R1-3 放寬回歸錨)', path: 'corpus-external/yosys-simple/carryadd.v' },
  { label: 'acc_pipe(xls,Apache-2.0)', path: 'corpus/e2e/58_xls_acc_pipe/acc_pipe.v' },
];
lines.push('## 規則書全文樣本(代表檔)');
lines.push('');
for (const s of samples) {
  const v = byPath.get(s.path);
  if (!v) continue;
  lines.push(`### ${s.label}`);
  lines.push('');
  lines.push(renderRulebookMarkdown(v.rulebook));
  lines.push('');
}

// 破口報告樣本(≥2 檔,含 suggested_test)。
const gapSamples = verdicts.filter((v) => (v.gapCount ?? 0) > 0).slice(0, 3);
lines.push('## 破口報告樣本');
lines.push('');
if (gapSamples.length === 0) {
  lines.push('本輪 13 檔破口總數 0(良構設計零誤報=負樣面全綠);破口形見 plugin fixtures(latch_trap.v/incomplete_case.v)與 rtl-spec 測試(gaps_positive.v)——破口報告形樣本由單測釘死。');
} else {
  for (const v of gapSamples) {
    lines.push(`### ${v.file}`);
    lines.push('');
    lines.push(renderGapsMarkdown({ source: v.file, items: v.gaps }));
    lines.push('');
  }
}

// 評審員區:人工批注=plugin/rtl2spec/review-notes.md(S7 填寫;機械內聯
// 進同一檔案,再生成不丟)。
lines.push('## 評審員區(S7 定性評審;人工區,機械內聯自 review-notes.md)');
lines.push('');
const reviewPath = join(pluginRoot, 'review-notes.md');
if (existsSync(reviewPath)) {
  lines.push(readFileSync(reviewPath, 'utf8').trimEnd());
} else {
  lines.push('(review-notes.md 未在場:R2 散文感抽查記錄 + R5 每代表檔 3 探針問題,');
  lines.push('  答案引用 rule id——填寫契約見本區標題;閘判定不受本區影響)');
}
lines.push('');
lines.push('## 總閘命令清單');
lines.push('');
lines.push('```');
lines.push('cargo test --workspace');
lines.push('bash scripts/regress.sh');
lines.push('bash plugin/rtl2spec/run-mcp-e2e.sh');
lines.push('bash plugin/rtl2spec/run-rtl-validation.sh');
lines.push('```');
lines.push('');

const outPath = join(pluginRoot, 'validation-report.md');
writeFileSync(outPath, lines.join('\n'));
// 行數口徑=真實行數(R3-③:lines 陣列元素≠檔案行數——規則書全文樣本
// 單元素含多換行,實錄 97 vs wc -l 1755);與 `wc -l` 同一口徑=\n 計數。
const written = readFileSync(outPath, 'utf8');
const realLines = (written.match(/\n/g) ?? []).length;
console.log(`INFO report written: plugin/rtl2spec/validation-report.md (${realLines} lines)`);

process.exit(failures > 0 ? 1 : 0);
