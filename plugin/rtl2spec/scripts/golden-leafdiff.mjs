// golden-leafdiff.mjs — golden 樹葉級對賬工具(§19.4.4 G3;唯讀,只寫 stdout)。
// golden JSON 為單行,`git diff` 無法做葉級對賬;本工具逐檔比較兩棵同構樹:
//   *.json → JSON 葉路徑 diff(物件按鍵、陣列按索引遞迴;僅一側存在的子樹整體當葉);
//   其他文字檔(PROVENANCE.md 等)→ 行級 diff(按行號對位)。
// 輸出:每個差異葉一行 `DIFF <檔> / <路徑>: <舊> → <新>`(值=JSON.stringify;缺=<absent>),
//   檔級增刪=`DIFF <檔> / (file): <absent> → <present>`(或反向);
//   尾行 `LEAFDIFF files_changed=<n> leaves=<m> changed=[<檔,…>]`。
// 用法: bun plugin/rtl2spec/scripts/golden-leafdiff.mjs <舊樹> <新樹> [--allow <檔1,檔2,…>]
//   --allow:變更檔必 ⊆ 允許清單 ∪ {PROVENANCE.md},否則 exit 1(G3 機械斷言);缺省只報不判。
// exit:0=比較完成(且滿足 --allow);1=越界變更;2=參數/目錄錯。
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative, resolve } from 'node:path';

const argv = process.argv.slice(2);
let allow = null;
const pos = [];
for (let i = 0; i < argv.length; i++) {
  if (argv[i] === '--allow') allow = (argv[++i] ?? '').split(',').filter(Boolean);
  else pos.push(argv[i]);
}
if (pos.length !== 2) {
  console.error('usage: golden-leafdiff.mjs <old-dir> <new-dir> [--allow f1,f2]');
  process.exit(2);
}
const [oldRoot, newRoot] = pos.map((p) => resolve(p));
for (const d of [oldRoot, newRoot]) {
  if (!existsSync(d) || !statSync(d).isDirectory()) {
    console.error(`not a directory: ${d}`);
    process.exit(2);
  }
}

function files(root) {
  const out = [];
  const walk = (d) => {
    for (const name of readdirSync(d).sort()) {
      const p = join(d, name);
      if (statSync(p).isDirectory()) walk(p);
      else out.push(relative(root, p));
    }
  };
  walk(root);
  return out;
}

const lines = [];
const changed = new Set();
const show = (v) => (v === undefined ? '<absent>' : JSON.stringify(v));
const isObj = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);

function leafDiff(file, path, a, b) {
  if (Array.isArray(a) && Array.isArray(b)) {
    const n = Math.max(a.length, b.length);
    for (let i = 0; i < n; i++) leafDiff(file, `${path}/${i}`, a[i], b[i]);
    return;
  }
  if (isObj(a) && isObj(b)) {
    const keys = [...Object.keys(a), ...Object.keys(b).filter((k) => !(k in a))];
    for (const k of keys) leafDiff(file, `${path}/${k}`, a[k], b[k]);
    return;
  }
  if (JSON.stringify(a) !== JSON.stringify(b)) {
    lines.push(`DIFF ${file} / ${path || '/'}: ${show(a)} → ${show(b)}`);
    changed.add(file);
  }
}

function textDiff(file, a, b) {
  // LCS 行對齊(插入/刪除一行不致位移整檔;PROVENANCE.md ~10^2 行,DP 足夠)。
  const la = a.split('\n');
  const lb = b.split('\n');
  const n = la.length;
  const m = lb.length;
  const dp = Array.from({ length: n + 1 }, () => new Int32Array(m + 1));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      dp[i][j] = la[i] === lb[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  let i = 0;
  let j = 0;
  while (i < n || j < m) {
    if (i < n && j < m && la[i] === lb[j]) {
      i++;
      j++;
    } else if (j < m && (i === n || dp[i][j + 1] >= dp[i + 1][j])) {
      lines.push(`DIFF ${file} / +L${j + 1}: <absent> → ${show(lb[j])}`);
      changed.add(file);
      j++;
    } else {
      lines.push(`DIFF ${file} / -L${i + 1}: ${show(la[i])} → <absent>`);
      changed.add(file);
      i++;
    }
  }
}

const oldFiles = new Set(files(oldRoot));
const newFiles = new Set(files(newRoot));
const all = [...new Set([...oldFiles, ...newFiles])].sort();
for (const f of all) {
  if (!oldFiles.has(f) || !newFiles.has(f)) {
    lines.push(`DIFF ${f} / (file): ${oldFiles.has(f) ? '<present>' : '<absent>'} → ${newFiles.has(f) ? '<present>' : '<absent>'}`);
    changed.add(f);
    continue;
  }
  const a = readFileSync(join(oldRoot, f), 'utf8');
  const b = readFileSync(join(newRoot, f), 'utf8');
  if (a === b) continue;
  if (f.endsWith('.json')) {
    let ja;
    let jb;
    try {
      ja = JSON.parse(a);
      jb = JSON.parse(b);
    } catch {
      textDiff(f, a, b);
      continue;
    }
    leafDiff(f, '', ja, jb);
    if (!changed.has(f)) {
      // 位元組不同但葉全等(鍵序/空白差):如實報檔級差,不吞。
      lines.push(`DIFF ${f} / (bytes): JSON 葉全等但位元組不同`);
      changed.add(f);
    }
  } else {
    textDiff(f, a, b);
  }
}

for (const l of lines) console.log(l);
const ch = [...changed].sort();
console.log(`LEAFDIFF files_changed=${ch.length} leaves=${lines.length} changed=[${ch.join(',')}]`);
if (allow) {
  const ok = new Set([...allow, 'PROVENANCE.md']);
  const out = ch.filter((f) => !ok.has(f));
  if (out.length) {
    console.log(`LEAFDIFF FAIL 越界變更(允許清單外):${out.join(',')}`);
    process.exit(1);
  }
  console.log(`LEAFDIFF PASS 變更檔 ⊆ 允許清單∪{PROVENANCE.md}`);
}
process.exit(0);
