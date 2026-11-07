// ssot-drift-check.mjs — run-plugin-check ⑤ SSOT 漂移判據(§19.2 D-r1a 防回歸;唯讀,只寫 stdout)。
// SSOT=crates/rtl-spec/src/schema.rs `pub const DEFAULT_MAX_RULES: usize = N;`。
//   (a) plugin/rtl2spec/{dist,scripts} 下全部 .mjs(遞迴;本檔自身除外)中規則提取導出呼叫的
//       第三實參若為數字字面量,必 ∈{0,N};字面量呼叫點數必 ≥1(防判據空轉假綠)。
//   (b) dist/server.mjs 的 max_rules_per_module 描述、skills/rtl-rulebook/SKILL.md 的
//       max_rules_per_module 文案:出現的整數 ⊆{0,N} 且含 N。
//   (c) 回退 token 錨(§19 R1-8):server.mjs `const max =` 三元式的回退分支=字面量 0;
//       rtl_extract_batch 工具段內規則提取呼叫的第三實參=字面量 0。
//   (d) native 面(§19 RV2-10):crates/rtl-spec/examples/emit.rs 含常量名 DEFAULT_MAX_RULES,
//       且參數分派段(`match args.as_slice()` 至其收束 `};`)無 `unwrap_or(<數字>)` 與
//       `<數字>usize` 形缺省。
//   (e)(f) 收尾弧 S4 起:RTL2SPEC-PLUGIN §20 釘選全表==pinned-files.list(逐檔名)+收尾弧欠條「[欠]」已清(見下方 (e)(f) 段)。
// 用法: bun plugin/rtl2spec/scripts/ssot-drift-check.mjs   exit:0=全過;1=判據敗;2=SSOT/檔缺。
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { basename, dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const selfPath = fileURLToPath(import.meta.url);
const pluginRoot = dirname(dirname(selfPath));
const repoRoot = dirname(dirname(pluginRoot));
const FN = 'rtl_extract_rules_sv';

let failures = 0;
const ck = (ok, name, detail = '') => {
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${detail ? ' — ' + detail : ''}`);
  if (!ok) failures++;
};
const read = (p) => {
  try {
    return readFileSync(p, 'utf8');
  } catch (e) {
    console.log(`FAIL guard: cannot read ${relative(repoRoot, p)}: ${e.message}`);
    process.exit(2);
  }
};

// SSOT
const schema = read(join(repoRoot, 'crates/rtl-spec/src/schema.rs'));
const mN = schema.match(/pub const DEFAULT_MAX_RULES: usize = (\d+);/);
if (!mN) {
  console.log('FAIL guard: schema.rs 未見 `pub const DEFAULT_MAX_RULES: usize = N;`');
  process.exit(2);
}
const N = mN[1];
console.log(`INFO SSOT DEFAULT_MAX_RULES=${N}`);

// 呼叫點實參切分:自 `FN(` 起按括號/引號配平取頂層逗號分段(唯讀解析,非刪改)。
function callArgs(src, openIdx) {
  const args = [];
  let depth = 0;
  let cur = '';
  let quote = null;
  for (let i = openIdx; i < src.length; i++) {
    const ch = src[i];
    if (quote) {
      cur += ch;
      if (ch === '\\') {
        cur += src[++i] ?? '';
      } else if (ch === quote) {
        quote = null;
      }
      continue;
    }
    if (ch === "'" || ch === '"' || ch === '`') {
      quote = ch;
      cur += ch;
      continue;
    }
    if (ch === '(') {
      depth++;
      if (depth === 1) continue;
    } else if (ch === ')') {
      depth--;
      if (depth === 0) {
        args.push(cur.trim());
        return args;
      }
    } else if (ch === ',' && depth === 1) {
      args.push(cur.trim());
      cur = '';
      continue;
    }
    cur += ch;
  }
  return null; // 未配平
}
function callSites(src) {
  const out = [];
  let idx = src.indexOf(`${FN}(`);
  while (idx !== -1) {
    const open = idx + FN.length;
    const args = callArgs(src, open);
    const line = src.slice(0, idx).split('\n').length;
    out.push({ line, args });
    idx = src.indexOf(`${FN}(`, open);
  }
  return out;
}
function walkMjs(dir) {
  const out = [];
  for (const name of readdirSync(dir).sort()) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) out.push(...walkMjs(p));
    else if (name.endsWith('.mjs') && p !== selfPath) out.push(p);
  }
  return out;
}

// (a)
{
  const files = [...walkMjs(join(pluginRoot, 'dist')), ...walkMjs(join(pluginRoot, 'scripts'))];
  const bad = [];
  let literal = 0;
  for (const f of files) {
    for (const s of callSites(read(f))) {
      const third = s.args?.[2];
      if (third !== undefined && /^\d+$/.test(third)) {
        literal++;
        if (third !== '0' && third !== N) bad.push(`${relative(repoRoot, f)}:${s.line} 實參=${third}`);
      }
    }
  }
  ck(literal >= 1, `⑤a-0 字面量呼叫點非空(防空轉)`, `${literal} 處`);
  ck(bad.length === 0, `⑤a 字面量實參 ∈{0,${N}}`, bad.length ? bad.join('; ') : `${literal} 處全合`);
}

// (b)
{
  const server = read(join(pluginRoot, 'dist/server.mjs'));
  const descLine = server.split('\n').find((l) => /max_rules_per_module:\s*\{/.test(l));
  const desc = descLine?.match(/description:\s*'([^']*)'/)?.[1];
  const skill = read(join(pluginRoot, 'skills/rtl-rulebook/SKILL.md'));
  const skillLine = skill.split('\n').find((l) => l.includes('max_rules_per_module'));
  for (const [name, text] of [
    ['server.mjs max_rules_per_module 描述', desc],
    ['rtl-rulebook SKILL max_rules_per_module 文案', skillLine],
  ]) {
    if (text === undefined) {
      ck(false, `⑤b ${name} 在場`);
      continue;
    }
    const nums = (text.match(/\d+/g) ?? []).filter((x) => x !== '0');
    const ok = nums.length >= 1 && nums.every((x) => x === N);
    ck(ok, `⑤b ${name} 上限數字==${N}`, JSON.stringify(nums));
  }
}

// (c)
{
  const server = read(join(pluginRoot, 'dist/server.mjs'));
  const at = server.indexOf('const max =');
  const expr = at === -1 ? null : server.slice(at, server.indexOf(';', at));
  const fallback = expr ? expr.slice(expr.lastIndexOf(':') + 1).trim() : null;
  ck(expr !== null && expr.includes('?') && fallback === '0', '⑤c-1 `const max =` 回退分支=字面量 0', JSON.stringify(fallback));
  const b0 = server.indexOf("name: 'rtl_extract_batch'");
  const b1 = b0 === -1 ? -1 : server.indexOf("name: '", b0 + 1);
  const block = b0 === -1 ? '' : server.slice(b0, b1 === -1 ? server.length : b1);
  const sites = callSites(block);
  const thirds = sites.map((s) => s.args?.[2]);
  ck(sites.length >= 1 && thirds.every((t) => t === '0'), '⑤c-2 batch 規則提取第三實參=字面量 0', JSON.stringify(thirds));
}

// (d)
{
  const emit = read(join(repoRoot, 'crates/rtl-spec/examples/emit.rs'));
  ck(emit.includes('DEFAULT_MAX_RULES'), '⑤d-1 emit.rs 引用常量名 DEFAULT_MAX_RULES');
  const s0 = emit.indexOf('match args.as_slice()');
  const s1 = s0 === -1 ? -1 : emit.indexOf('\n    };', s0);
  const seg = s0 === -1 || s1 === -1 ? null : emit.slice(s0, s1);
  const hits = seg ? [...seg.matchAll(/unwrap_or\(\s*\d+\s*\)|\b\d+usize\b/g)].map((m) => m[0]) : null;
  ck(seg !== null && hits.length === 0, '⑤d-2 emit.rs 參數分派段無數字缺省', seg === null ? '分派段未定位' : JSON.stringify(hits));
}

// (e)(f) 釘選集 SSOT 對拍(收尾弧 S4 欠條清償閘;RTL2SPEC-FINISH-PLAN §11)——
//   (e) RTL2SPEC-PLUGIN §20 釘選全表(機讀標記內一行一檔)==pinned-files.list(集合相等、無重複、條數相等);
//       「§16.1.1 原表」行=清單 S3 註釋塊之前的條目且其源檔名在 §16.1.1 節原文出現;「收尾弧 S3 擴檔」行=S3 註釋塊之後的條目。
//   (f) RTL2SPEC-FINISH-PLAN 不得殘留欠條標記「[欠]」(欠條未清=紅)。
//   自證:解析器對敵意向量(標記缺/重複行/來源欄集外)必報錯。
export function pinnedTableRows(text) {
  const b = text.indexOf('<!-- pinned-set-table begin -->');
  const e = text.indexOf('<!-- pinned-set-table end -->');
  if (b < 0 || e < 0 || e < b) throw new Error('釘選全表標記缺或倒置');
  if (text.indexOf('<!-- pinned-set-table begin -->', b + 1) >= 0) throw new Error('釘選全表 begin 標記重複');
  const rows = [];
  for (const l of text.slice(b, e).split('\n')) {
    if (!l.startsWith('| `')) continue;
    const m = l.match(/^\| `([^`]+)` \| (§16\.1\.1 原表|收尾弧 S3 擴檔) \|/);
    if (!m) throw new Error(`釘選全表行形非法: ${l.slice(0, 80)}`);
    rows.push({ path: m[1], origin: m[2] });
  }
  const seen = new Set();
  for (const r of rows) {
    if (seen.has(r.path)) throw new Error(`釘選全表重複行 ${r.path}`);
    seen.add(r.path);
  }
  return rows;
}
{
  const threw = (t) => {
    try {
      pinnedTableRows(t);
      return false;
    } catch {
      return true;
    }
  };
  const B = '<!-- pinned-set-table begin -->\n';
  const E = '<!-- pinned-set-table end -->\n';
  const ok = `${B}| \`a.v\` | §16.1.1 原表 | 1 | — |\n| \`b.v\` | 收尾弧 S3 擴檔 | 2 | b |\n${E}`;
  const vec = [
    !threw(ok) && pinnedTableRows(ok).length === 2,
    threw(`| \`a.v\` | §16.1.1 原表 | 1 | — |\n`),
    threw(`${B}| \`a.v\` | §16.1.1 原表 | 1 | — |\n| \`a.v\` | §16.1.1 原表 | 1 | — |\n${E}`),
    threw(`${B}| \`a.v\` | 其他來源 | 1 | — |\n${E}`),
    threw(`${B}${B}${E}`),
  ];
  ck(vec.every(Boolean), '⑤e-0 釘選全表解析器自證(1 正 4 負:標記缺/重複行/來源集外/標記重複)', JSON.stringify(vec));

  const listText = read(join(pluginRoot, 'pinned-files.list'));
  const lines = listText.split('\n');
  const s3At = lines.findIndex((l) => l.startsWith('# --- 收尾弧 S3 擴檔'));
  const list = [];
  lines.forEach((l, i) => {
    const t = l.trim();
    if (t && !t.startsWith('#')) list.push({ path: t, s3: s3At >= 0 && i > s3At });
  });
  const plugin = read(join(repoRoot, 'docs/RTL2SPEC-PLUGIN.md'));
  let rows = null;
  try {
    rows = pinnedTableRows(plugin);
  } catch (err) {
    ck(false, '⑤e-1 §20 釘選全表可解析', err.message);
  }
  if (rows) {
    const L = new Set(list.map((x) => x.path));
    const R = new Set(rows.map((x) => x.path));
    const onlyList = [...L].filter((p) => !R.has(p));
    const onlyTable = [...R].filter((p) => !L.has(p));
    ck(list.length === rows.length && onlyList.length === 0 && onlyTable.length === 0, `⑤e-1 §20 釘選全表==pinned-files.list(逐檔名;${rows.length}/${list.length})`, JSON.stringify({ onlyList, onlyTable }));
    const s3List = new Set(list.filter((x) => x.s3).map((x) => x.path));
    const originBad = rows.filter((r) => (r.origin === '收尾弧 S3 擴檔') !== s3List.has(r.path)).map((r) => r.path);
    ck(originBad.length === 0 && s3List.size > 0, `⑤e-2 來源欄==清單分組(§16.1.1 原表 ${rows.length - s3List.size}/S3 擴檔 ${s3List.size})`, originBad.join(','));
    const a = plugin.indexOf('**16.1.1 釘選語料集');
    const z = plugin.indexOf('**16.1.2 golden 產物形態');
    const sec = a >= 0 && z > a ? plugin.slice(a, z) : '';
    const stem = (p) => p.split('/').pop().replace(/\.v$/, '');
    const missing = rows.filter((r) => r.origin === '§16.1.1 原表' && !sec.includes(stem(r.path))).map((r) => r.path);
    ck(sec !== '' && missing.length === 0, '⑤e-3 「§16.1.1 原表」行的源檔名皆在 §16.1.1 節原文出現(歷史表對拍)', missing.join(','));
  }
  const plan = read(join(repoRoot, 'docs/RTL2SPEC-FINISH-PLAN.md'));
  const debts = plan.split('\n').filter((l) => l.includes('[欠]'));
  ck(debts.length === 0, '⑤f 收尾弧欠條已清(RTL2SPEC-FINISH-PLAN 無「[欠]」標記)', debts.map((l) => l.trim().slice(0, 60)).join(' | '));
}

console.log(`== ssot-drift-check: ${failures ? 'FAIL' : 'PASS'}(${basename(selfPath)}) ==`);
process.exit(failures ? 1 : 0);
