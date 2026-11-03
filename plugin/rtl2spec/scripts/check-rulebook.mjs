// check-rulebook.mjs — 單檔機械判據執行器(§8.2 ③;§8.3 V1..V6 檔級面)。
// 裝載 dist 同一 wasm 產物(D8:閘驗出貨物);**獨立重算**,不信任生成端:
//   V1 單檔 rules≥3(skipped[tb_file] 檔豁免,報告單列)+ truncated 非空→FAIL
//   V2 檔級行為占比 ≥40%(冗餘防線定位:R1-7;主判據=V5-①+V4)
//   V4 每規則 evidence.snippet 與源檔對應行按拼接規則逐位元組相等
//      (多行=各行原文以 \n 連接、去行尾 \r;SV 側 100% 帶 file:line)
//   V5-① 每規則 evidence 含 ≥1 行為類(主判據)
//   V5-② check.form∈五形且 form 專屬欄位非空
//   V5-④ 陳述含 ≥1 個源碼 grep 可得的標識符(冗餘防線)
//   V6 成功呼叫(本進程到達判定即成立)
// 用法:bun check-rulebook.mjs <rtl-file>
// 輸出:verdict JSON(stdout)+ 落盤 build/verdicts/<slug>.json;exit 0/1。

import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join, dirname, basename } from 'node:path';
import { fileURLToPath } from 'node:url';
import { sumRules } from '../dist/md.mjs';

const pluginRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const file = process.argv[2];
if (!file) {
  console.error('usage: check-rulebook.mjs <rtl-file>');
  process.exit(2);
}
const source = readFileSync(file, 'utf8');
const name = file; // fingerprint source 維=路徑原樣(全路徑區分同名模組)

// 裝載 dist 同一 wasm(RTL2SPEC_WASM_DIR 優先;沿 server 同款顯式位元組 init)。
const wasmDir =
  process.env.RTL2SPEC_WASM_DIR && existsSync(process.env.RTL2SPEC_WASM_DIR)
    ? process.env.RTL2SPEC_WASM_DIR
    : join(pluginRoot, 'dist', 'wasm');
const glue = await import(join(wasmDir, 'rtl_spec_wasm.js'));
await glue.default({
  module_or_path: new Uint8Array(readFileSync(join(wasmDir, 'rtl_spec_wasm_bg.wasm'))),
});

const rb = JSON.parse(glue.rtl_extract_rules_sv(name, source, 128));
const gr = JSON.parse(glue.rtl_scan_gaps_sv(name, source));

const failures = [];
function fail(check, detail) {
  failures.push({ check, detail });
}

// ---- 判據材料 ----
const rules = (rb.modules ?? []).flatMap((m) => m.rules ?? []);
const total = sumRules(rb);
const isTbSkipped = (rb.skipped ?? []).some((n) => n.reason === 'tb_file');
const behavioural = (cat) => cat !== 'param' && cat !== 'structure';
const behaviourCount = rules.filter((r) => behavioural(r.category)).length;
const categories = [...new Set(rules.map((r) => r.category))];

// 源檔行(拼接規則的行定義:split \n,去行尾 \r——與生成端同一契約)。
const srcLines = source.split('\n').map((l) => l.replace(/\r$/, ''));
// 源碼標識符全集(V5-④)。
const idents = new Set(source.match(/[A-Za-z_][A-Za-z0-9_$]*/g) ?? []);

// ---- V1(檔級) ----
if (rb.truncated?.length) {
  fail('V1-truncated', `truncated 非空:${rb.truncated.join(', ')}`);
}
if (!isTbSkipped && total < 3) {
  fail('V1-single', `rules=${total} < 3`);
}

// ---- V2(檔級,冗餘防線) ----
const behaviourRatio = total === 0 ? 1 : behaviourCount / total;
if (!isTbSkipped && behaviourRatio < 0.4) {
  fail('V2-single', `behaviour ratio ${(behaviourRatio * 100).toFixed(1)}% < 40%`);
}

// ---- V4 溯源(SV 側 100%) + V5-①/②/④ ----
const FORM_FIELDS = {
  guard: (c) => Array.isArray(c.effects) && c.effects.length > 0 && c.effects.every((e) => e.lhs && e.value),
  transition: (c) => c.state_reg && c.from && c.to,
  sequence: (c) => c.signal && c.becomes && c.anchor?.expr,
  // relation 的 when=適用域(R3-②):可缺席(全域恆等),在場必須非空
  // 字串——空串 when=資料端鎖「欄位非空」語義違例。
  relation: (c) =>
    c.lhs && c.rhs &&
    (c.when == null || (typeof c.when === 'string' && c.when.length > 0)),
  invariant: (c) => c.subject && ((c.values ?? []).length > 0 || c.expr),
};
const FORMS = new Set(Object.keys(FORM_FIELDS));
const BEH_CLASSES = new Set(['process', 'assign', 'case', 'generate', 'expr']);

for (const r of rules) {
  // V5-① 行為證(主判據)。
  if (!r.evidence?.some((e) => BEH_CLASSES.has(e.class))) {
    fail('V5-behaviour-evidence', `${r.id} 無行為類 evidence`);
  }
  // V5-② form 詞表+專屬欄位非空。
  if (!FORMS.has(r.check?.form) || !FORM_FIELDS[r.check?.form]?.(r.check ?? {})) {
    fail('V5-form-fields', `${r.id} form=${r.check?.form} 欄位不全`);
  }
  // V5-④ 陳述含源碼真標識符(冗餘防線;詞邊界匹配,無長度限定——
  // 單字符訊號名也是真標識符,generate.v gen_test7 `x ≡ 'sd2 ** 'sd1` 實錄)。
  const stmtIdents = (r.statement.match(/[A-Za-z_][A-Za-z0-9_$]*/g) ?? []).filter((x) =>
    idents.has(x),
  );
  if (stmtIdents.length === 0) {
    fail('V5-statement-identifier', `${r.id} 陳述無源碼標識符:${r.statement.slice(0, 40)}`);
  }
  // V4 溯源(SV 側=有 file:line 的 evidence 100% 逐位元組對拍)。
  for (const e of r.evidence ?? []) {
    if (e.line === 0) {
      // RTLIL 結構路徑證形(D2):V4 只對 SV 側斷言;本閘=SV 語料,不應出現。
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

// ---- V6(本檔成功呼叫;到達此處=已產出 rulebook) ----

const gapsByClass = {};
for (const g of gr.items ?? []) {
  gapsByClass[g.class] = (gapsByClass[g.class] ?? 0) + 1;
}
const categoryCounts = {};
for (const r of rules) {
  categoryCounts[r.category] = (categoryCounts[r.category] ?? 0) + 1;
}

const verdict = {
  file,
  name: basename(file),
  ok: failures.length === 0,
  isTbSkipped,
  lineCount: srcLines.length,
  moduleCount: (rb.modules ?? []).length,
  ruleCount: total,
  behaviourCount,
  behaviourRatio,
  categories,
  categoryCounts,
  parseErrorCount: (rb.parse_errors ?? []).length,
  truncated: rb.truncated ?? [],
  gapCount: (gr.items ?? []).length,
  gapsByClass,
  rulebook: rb,
  gaps: gr.items ?? [],
  failures,
  wasmSha256: createHash('sha256')
    .update(readFileSync(join(wasmDir, 'rtl_spec_wasm_bg.wasm')))
    .digest('hex'),
};

const outDir = join(pluginRoot, 'build', 'verdicts');
mkdirSync(outDir, { recursive: true });
const slug = name.replace(/[^A-Za-z0-9._-]+/g, '__');
writeFileSync(join(outDir, `${slug}.json`), JSON.stringify(verdict, null, 2));

console.log(JSON.stringify({ ...verdict, rulebook: undefined, gaps: undefined }));
process.exit(verdict.ok ? 0 : 1);
