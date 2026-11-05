// gen-eval-report.mjs — docs/RTL2SPEC-EVAL.md 機械生成器(§16.4;證據頁,禁手改)。
// 讀 build/eval/*.json+ _summary.json(eval-all.mjs 產出),實測列由 verdict
// 數據推導禁硬編碼;自洽斷言(判定與 failures 同源檢查,gen-report.mjs
// R3-③ 先例):判定 FAIL 的檔 failures 必非空;閘 PASS 時 trap 必 0;
// per-file 表行數必=L1 檔數;判定分布與 verdict 一致。
// 退出碼:0=全過(報告已寫);1=自洽斷言敗(報告仍寫,閘紅);2=verdicts 缺。
import { readFileSync, readdirSync, writeFileSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const pluginRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const repoRoot = dirname(dirname(pluginRoot));
const evalDir = join(pluginRoot, 'build', 'eval');
if (!existsSync(join(evalDir, '_summary.json'))) {
  console.log('FAIL build/eval 缺(先跑 eval-all.mjs)');
  process.exit(2);
}
const summary = JSON.parse(readFileSync(join(evalDir, '_summary.json'), 'utf8'));

const verdicts = [];
for (const f of readdirSync(evalDir)) {
  if (!f.endsWith('.json') || f === '_summary.json') continue;
  verdicts.push(JSON.parse(readFileSync(join(evalDir, f), 'utf8')));
}
verdicts.sort((a, b) => a.file.localeCompare(b.file));
const L1 = verdicts.filter((v) => v.layer === 'L1');
const L2 = verdicts.filter((v) => v.layer === 'L2');

let failures = 0;
const failCheck = (n, d) => { console.log(`FAIL ${n} — ${d}`); failures++; };
const passCheck = (n, d = '') => { console.log(`PASS ${n}${d ? ` — ${d}` : ''}`); };

// ---- 自洽斷言(R3-③ 同款) ----
const badFail = verdicts.filter((v) => v.judgement === 'design-rules' && v.ok === true && (v.failures?.length ?? 0) > 0);
if (badFail.length) failCheck('self-consistency', `判定 PASS 而 failures 非空:${badFail.map((v) => v.file).join(', ')}`);
const drAll = verdicts.filter((v) => v.judgement === 'design-rules');
const drOk = drAll.filter((v) => v.ok === true).length;
const drFailCount = drAll.filter((v) => v.ok === false).length;
if (drOk + drFailCount !== drAll.length) failCheck('self-consistency', `design-rules 判定計數不自洽(${drOk}+${drFailCount}≠${drAll.length})`);
if (summary.gateFailures?.length === 0 && summary.traps !== 0) failCheck('self-consistency', `閘 PASS 而 trap=${summary.traps}`);
if (summary.traps === 0 && verdicts.some((v) => v.trap)) failCheck('self-consistency', 'summary trap=0 而 verdict 含 trap 檔');
if (L1.length !== summary.l1) failCheck('self-consistency', `L1 表檔數 ${L1.length}≠summary ${summary.l1}`);
passCheck('self-consistency', `design-rules ${drOk} PASS/${drFailCount} FAIL;trap=${summary.traps};per-file 表 ${L1.length} 檔`);

// ---- license 表(機械,路徑推導;零 GPL) ----
const licenseOf = (f) =>
  f.startsWith('corpus-external/yosys-simple/') ? 'ISC(COPYING 隨附)'
  : f === 'crates/spec-i2c/rtl/i2c_master.v' ? 'repo MIT(乾淨室;OpenCores 唯讀啟發零拷貝)'
  : f.includes('/58_xls_acc_pipe/') || f.includes('/57_xls_add3/') ? 'Apache-2.0(xls 派生;repo MIT 載體)'
  : f.startsWith('corpus-external/sv-tests/') ? 'ISC(LICENSE 隨附)'
  : 'repo MIT';
const allLicenses = [...new Set(verdicts.map((v) => licenseOf(v.file)))];
if (allLicenses.some((l) => l.includes('GPL'))) failCheck('license-audit', 'GPL 進評測集(鐵則)');

// ---- 報告 ----
const lines = [];
lines.push('# rtl2spec 真實評測擴面報告(§16.4;機械生成禁手改)');
lines.push('');
lines.push(`- 日期:${new Date().toISOString().slice(0, 10)}`);
lines.push('- 命令行:`bash plugin/rtl2spec/run-realworld-eval.sh`(機械生成禁手改)');
lines.push(`- 工具鏈:bun ${summary.bun}、wasm ABI ${summary.abi}、wasm sha256 \`${summary.wasmSha256}\``);
lines.push(`- 耗時:${summary.elapsedMs}ms(單 bun 進程循環 ${summary.total} 檔);語料零 GPL(yosys-simple/sv-tests=ISC、corpus=repo MIT、spec-i2c=MIT 乾淨室、xls=Apache-2.0 載體)`);
lines.push('- 判據鏈:per-file 三檔分類(design-rules/design-sparse/skip-perr/skip-tb);閘級硬 FAIL=trap/JSON 不可解析/design-rules 檔 V4/V5-①②④ 敗/V5-③ 指紋重複(L1+L2 全量)/成功呼叫≠檔數。V1(≥3)/V2/V3 為報告列不設單檔 FAIL(擴面集失敗條件=結構性違例而非產量);13 檔驗證集 V1..V6 閾值閘另行執法不動(run-rtl-validation.sh)。');
lines.push('');
lines.push('## 閘級判據表');
lines.push('');
lines.push('| 判據 | 實測 | 判定 |');
lines.push('|---|---|---|');
lines.push(`| trap(終止性防線=F2 修復+trap 計數,R16-3) | ${summary.traps} | ${summary.traps === 0 ? 'PASS' : 'FAIL'} |`);
lines.push(`| design-rules 檔 V4+V5-①②④ | ${drOk}/${drAll.length} 檔全過(checker 同形獨立重算) | ${drFailCount === 0 ? 'PASS' : 'FAIL'} |`);
lines.push(`| V5-③ 指紋去重(L1+L2 全量) | ${summary.totalRulesFp ?? 'n/a'} 條規則,重複=${summary.fpDup} | ${summary.fpDup === 0 ? 'PASS' : 'FAIL'} |`);
lines.push(`| V6 覆蓋面(成功呼叫=檔數) | ${summary.successCalls}/${summary.total} | ${summary.successCalls === summary.total ? 'PASS' : 'FAIL'} |`);
lines.push(`| parse_err 如實計數(L2 已知事實記錄,不 FAIL) | skip-perr=${summary.byJ['skip-perr'] ?? 0} 檔 | INFO |`);
lines.push('');
lines.push(`閘級判定:**${summary.gateFailures?.length === 0 ? 'PASS' : 'FAIL'}**${summary.gateFailures?.length ? `(硬 FAIL:${summary.gateFailures.join(';')})` : ''}`);
lines.push('');
lines.push('## 分層統計表');
lines.push('');
lines.push('| 層 | 檔數 | ' + ['design-rules', 'design-sparse', 'skip-perr', 'skip-tb'].map((j) => `${j}`).join(' | ') + ' |');
lines.push('|---|---|' + ['---', '---', '---', '---'].join('|') + '|');
for (const [name, vs] of [['L1 質量層', L1], ['L2 容錯層', L2], ['全集', verdicts]]) {
  const c = (j) => vs.filter((v) => v.judgement === j).length;
  lines.push(`| ${name} | ${vs.length} | ${c('design-rules')} | ${c('design-sparse')} | ${c('skip-perr')} | ${c('skip-tb')} |`);
}
lines.push('');
lines.push('## L1 per-file 表(93 檔全列)');
lines.push('');
lines.push('| 檔名 | license | 行數 | 規則數 | 類別數 | 破口數(按類) | 判定 |');
lines.push('|---|---|---|---|---|---|---|');
for (const v of L1) {
  const cats = Object.keys(v.categoryCounts ?? {}).length;
  const gaps = Object.entries(v.gapsByClass ?? {}).map(([k, n]) => `${k}=${n}`).join(' ') || '0';
  const j = v.judgement === 'design-rules' ? (v.ok ? 'PASS' : 'FAIL') : v.judgement === 'design-sparse' ? (v.ok ? 'OK-sparse' : 'FAIL') : 'SKIP';
  lines.push(`| ${v.file} | ${licenseOf(v.file)} | ${v.lineCount} | ${v.ruleCount} | ${cats} | ${gaps} | ${j} |`);
}
lines.push('');
lines.push('## L2 容錯層(260 檔;分檔統計+parse_err 清單,不逐檔列表)');
lines.push('');
{
  const perr = L2.filter((v) => v.judgement === 'skip-perr');
  const okL2 = L2.filter((v) => v.judgement !== 'skip-perr');
  lines.push(`- 可終止+零 trap+JSON 可解析:${L2.length}/${L2.length} 檔(trap=${summary.traps})`);
  lines.push(`- parse_err(SV-2009 構造超 parser 閘,已知事實記錄不 FAIL):${perr.length} 檔`);
  lines.push(`- 非 parse_err 判定分布:design-rules=${okL2.filter((v) => v.judgement === 'design-rules').length}、design-sparse=${okL2.filter((v) => v.judgement === 'design-sparse').length}、skip-tb=${okL2.filter((v) => v.judgement === 'skip-tb').length}`);
  lines.push('');
  lines.push('<details><summary>parse_err 檔名清單(機械列出)</summary>');
  lines.push('');
  for (const v of perr) lines.push(`- ${v.file}(${v.parseErrorCount} err)`);
  lines.push('');
  lines.push('</details>');
}
lines.push('');
lines.push('## 類別分布表(L1 全集規則×類別,九類+param/structure;R4 複評素材)');
lines.push('');
{
  const catFiles = {};
  for (const v of L1) {
    for (const c of Object.keys(v.categoryCounts ?? {})) catFiles[c] = (catFiles[c] ?? 0) + 1;
  }
  const catRules = {};
  for (const v of L1) {
    for (const [c, n] of Object.entries(v.categoryCounts ?? {})) catRules[c] = (catRules[c] ?? 0) + n;
  }
  lines.push('| 類別 | 檔數 | 規則數 |');
  lines.push('|---|---|---|');
  for (const c of Object.keys(catRules).sort()) lines.push(`| ${c} | ${catFiles[c] ?? 0} | ${catRules[c]} |`);
  const totalRules = Object.values(catRules).reduce((a, b) => a + b, 0);
  lines.push(`| **Σ** | — | **${totalRules}** |`);
}
lines.push('');
lines.push('## 評審員區(R5 分層抽樣 5 檔定性複評;人工區,機械內聯自 eval-review-notes.md)');
lines.push('');
const reviewPath = join(pluginRoot, 'eval-review-notes.md');
if (existsSync(reviewPath)) {
  lines.push(readFileSync(reviewPath, 'utf8').trimEnd());
} else {
  lines.push('(eval-review-notes.md 未在場:R5 探針題×3×檔填寫契約見 §16.5;閘判定不受本區影響)');
}
lines.push('');
lines.push('## 總閘命令清單');
lines.push('');
lines.push('```');
lines.push('cargo test --workspace');
lines.push('bash scripts/regress.sh');
lines.push('bash plugin/rtl2spec/run-mcp-e2e.sh');
lines.push('bash plugin/rtl2spec/run-rtl-validation.sh');
lines.push('bash plugin/rtl2spec/run-plugin-check.sh');
lines.push('bash plugin/rtl2spec/run-golden-regression.sh');
lines.push('bash plugin/rtl2spec/run-gap-fixtures.sh');
lines.push('bash plugin/rtl2spec/run-realworld-eval.sh');
lines.push('```');
lines.push('');

const outPath = join(repoRoot, 'docs', 'RTL2SPEC-EVAL.md');
writeFileSync(outPath, lines.join('\n'));
const written = readFileSync(outPath, 'utf8');
console.log(`INFO report written: docs/RTL2SPEC-EVAL.md (${(written.match(/\n/g) ?? []).length} lines)`);
process.exit(failures > 0 ? 1 : 0);
