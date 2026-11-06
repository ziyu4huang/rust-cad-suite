// golden-check.mjs — golden 錄製/比對執行器(§16.1.3 段 3+4+5;§16.1.4 re-record 機制)。
//
// 兩模式:
//   默認(比對): 37 檔釘選集跑提取(dist 同一 wasm,D8 閘驗出貨物)→寫
//     build/golden-current/<鏡像>.(rules|gaps).json→與 golden/<鏡像>…
//     逐一 byte-diff(任何 diff→列檔名+首個差異鍵路徑→exit 1);全等後
//     對 golden/ 每份正式樹檔算 sha256 與 PROVENANCE.md 表核對(快照被
//     手改檢測層;表≠檔→exit 1)。
//   --record(錄製): 前置自檢(§16.1.4 實現形=進程內重複調用:每檔每模式
//     3 跑 hash 必全等;任何檔不過→**全輪拒錄** exit 3,不寫任何盤——
//     兩階段防部分錄製)→全過後統一寫 golden/<鏡像>…+重寫
//     golden/PROVENANCE.md 元資料區(命令/日期/bun/wasm sha256/74 份 sha256 表);
//     歷史區(## re-record 歷史 行→檔尾)解析-保留-追加(§5.2 S0;審讀 #7 M7):
//     既有手記行原樣保留,新增錄製行 append 進歷史區(錄製行=機械時間戳+sha
//     前 8 位;語義輪對帳手記由輪內補寫於其後,下次錄製保留)。
//     錄後必讀 `git diff plugin/rtl2spec/golden` 逐案對帳——期望變了而
//     提取器沒變=必須能解釋,否則停(§16.1.4 re-record=語義事件)。
//
// 鏡像路徑規則(§16.1.2「目錄層=來源樹」):corpus-external/<X>/… 剥根→
//   <X>/…(golden/yosys-simple/sincos.rules.json 先例);corpus/…、crates/…
//   保留全路徑(同名 i2c_master.v 兩檔不撞,§16.1.1 全路徑釘同款 R16-7)。
// demo 樣例(golden/plugin-fixtures/)不屬本器管轄:record-golden.mjs 維護,
//   其 PROVENANCE=golden/plugin-fixtures/PROVENANCE.md(run-plugin-check ④d
//   對 demo golden byte-diff,兩域互不覆蓋)。
//
// 用法: bun scripts/golden-check.mjs [--record] [--list <list-file>]
// exit:0=全等/全錄;1=比對差異或 PROVENANCE 核對敗;2=清單/檔缺失;
//       3=--record 自檢不過(拒錄)。
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const pluginRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const repoRoot = dirname(dirname(pluginRoot));

// 裝載 dist 同一 wasm(顯式位元組 init,與 check-rulebook/record-golden 同形,D8)。
const wasmDir = join(pluginRoot, 'dist', 'wasm');
const glue = await import(join(wasmDir, 'rtl_spec_wasm.js'));
await glue.default({
  module_or_path: new Uint8Array(readFileSync(join(wasmDir, 'rtl_spec_wasm_bg.wasm'))),
});

// ---- 參數 ----
const argv = process.argv.slice(2);
let record = false;
let listRel = 'plugin/rtl2spec/pinned-files.list';
for (let i = 0; i < argv.length; i++) {
  if (argv[i] === '--record') record = true;
  else if (argv[i] === '--list') listRel = argv[++i];
  else {
    console.error(`usage: golden-check.mjs [--record] [--list <list-file>] — 未參數 ${argv[i]}`);
    process.exit(2);
  }
}
const listPath = listRel.startsWith('/') ? listRel : join(repoRoot, listRel);
if (!existsSync(listPath)) {
  console.error(`FAIL: 清單缺 ${listPath}`);
  process.exit(2);
}
const files = readFileSync(listPath, 'utf8')
  .split('\n')
  .map((l) => l.trim())
  .filter((l) => l && !l.startsWith('#'));
const missing = files.filter((f) => !existsSync(join(repoRoot, f)));
if (missing.length > 0) {
  console.error(`FAIL: 釘選集缺 ${missing.length} 檔:\n  ${missing.join('\n  ')}`);
  process.exit(2);
}

const sha = (s) => createHash('sha256').update(s).digest('hex');
const mirrorPath = (rel) =>
  rel.startsWith('corpus-external/') ? rel.slice('corpus-external/'.length) : rel;
const artifacts = (rel) => {
  const m = mirrorPath(rel).replace(/\.v$/, '');
  return [`${m}.rules.json`, `${m}.gaps.json`];
};
const runPair = (rel, src) => [
  // S3:預算隨 DEFAULT_MAX_RULES 上調 128→256(i2c 去重復活 152 觸頂)。
  glue.rtl_extract_rules_sv(rel, src, 256),
  glue.rtl_scan_gaps_sv(rel, src),
];

// 首個差異鍵路徑(byte 不等時對 JSON 淺遞歸定位;解析敗=報非 JSON)。
function firstDiffKey(a, b) {
  let pa, pb;
  try {
    pa = JSON.parse(a);
    pb = JSON.parse(b);
  } catch (e) {
    return `(非 JSON: ${e.message.slice(0, 60)})`;
  }
  const walk = (x, y, path) => {
    if (x === y) return null;
    if (
      typeof x !== typeof y ||
      x === null ||
      y === null ||
      Array.isArray(x) !== Array.isArray(y)
    ) {
      return path || '$';
    }
    if (Array.isArray(x)) {
      if (x.length !== y.length) return `${path}.length(${x.length}vs${y.length})`;
      for (let i = 0; i < x.length; i++) {
        const r = walk(x[i], y[i], `${path}[${i}]`);
        if (r) return r;
      }
      return null;
    }
    if (typeof x === 'object') {
      const ka = Object.keys(x);
      const kb = Object.keys(y);
      if (ka.length !== kb.length) return `${path}.keys(${ka.length}vs${kb.length})`;
      for (const k of ka) {
        if (!(k in y)) return `${path}.${k}(僅現側)`;
        const r = walk(x[k], y[k], path ? `${path}.${k}` : k);
        if (r) return r;
      }
      return null;
    }
    return path || '$';
  };
  return walk(pa, pb, '') || '(值異路徑未定位=序列化空白差)';
}

const goldenRoot = join(pluginRoot, 'golden');

// ================= --record:兩階段錄製 =================
if (record) {
  // 階段 1:全檔進程內 3 跑自檢(§16.1.4;任何檔不過=全輪拒錄 exit 3)
  const selfcheckFail = [];
  const outputs = new Map(); // rel -> [rulesStr, gapsStr]
  for (const rel of files) {
    const src = readFileSync(join(repoRoot, rel), 'utf8');
    const hs = new Set();
    let pair = null;
    for (let k = 0; k < 3; k++) {
      pair = runPair(rel, src);
      hs.add(sha(pair[0]) + sha(pair[1]));
    }
    if (hs.size !== 1) {
      selfcheckFail.push(rel);
      console.log(`FAIL 自檢 ${rel}: 3 跑 ${hs.size} 種 hash(§16.1.4 契約)`);
      continue;
    }
    outputs.set(rel, pair);
  }
  if (selfcheckFail.length > 0) {
    console.log(`== --record 拒錄: ${selfcheckFail.length} 檔自檢不過(exit 3;不寫任何盤) ==`);
    process.exit(3);
  }
  // 階段 2:統一寫盤+PROVENANCE 元資料區重寫+歷史區解析-保留-追加(S0)
  const provRows = [];
  for (const rel of files) {
    const [rulesStr, gapsStr] = outputs.get(rel);
    const [rn, gn] = artifacts(rel);
    for (const [name, content] of [
      [rn, rulesStr],
      [gn, gapsStr],
    ]) {
      const dst = join(goldenRoot, name);
      mkdirSync(dirname(dst), { recursive: true });
      writeFileSync(dst, content);
      provRows.push([name, sha(content)]);
    }
  }
  const wasmSha = sha(readFileSync(join(wasmDir, 'rtl_spec_wasm_bg.wasm')));
  // S0(M7 修復;§5.2;審讀 #7):歷史區解析-保留-追加——元資料區模板機械重寫;
  // 歷史區(## re-record 歷史 行→檔尾)自既有 PROVENANCE.md 原樣保留,
  // 新增錄製行 append 進歷史區,語義輪手記不被錄製覆蓋。
  const nowIso = new Date().toISOString();
  const HIST_HEADER = '## re-record 歷史';
  const SEED_HISTORY = [
    '## re-record 歷史(語義事件日志;語義輪手記逐行)',
    '',
    '- (首錄 2026-11-04,S9;F1a/F1b+F2 確定性修復後基線,§16.6 S9①對帳=全部可解釋首錄)',
  ].join('\n');
  const provPath = join(goldenRoot, 'PROVENANCE.md');
  let historyText;
  if (existsSync(provPath)) {
    const oldLines = readFileSync(provPath, 'utf8').split('\n');
    const histHits = [];
    for (let i = 0; i < oldLines.length; i++) {
      if (oldLines[i].startsWith(HIST_HEADER)) histHits.push(i);
    }
    if (histHits.length > 1) {
      console.error(`FAIL: 既有 PROVENANCE.md 歷史區標頭命中 ${histHits.length} 處(預期 ≤1;拒錄保盤)`);
      process.exit(3);
    }
    // 命中 1=該行→檔尾原樣保留;命中 0(舊格式/空)=seed 起始歷史(同舊模板形)。
    historyText =
      histHits.length === 1
        ? oldLines.slice(histHits[0]).join('\n').replace(/\n$/, '')
        : SEED_HISTORY;
  } else {
    historyText = SEED_HISTORY;
  }
  const prov = [
    '# golden PROVENANCE(機械生成禁手改;--record 機械重寫元資料區+歷史區保留追加錄製行;語義輪手記入歷史區)',
    '',
    '## 正式釘選集 37 檔×2 模式(§16.1.2)',
    '',
    `- 錄製命令: bash plugin/rtl2spec/run-golden-regression.sh --record`,
    `- 錄製時間: ${nowIso}`,
    `- bun: ${process.version}; wasm ABI: ${glue.rtl_spec_version()}; wasm sha256: ${wasmSha}`,
    `- 錄製來源: 提取器真實運行輸出(禁手寫);re-record=語義事件,錄後必讀`,
    `  git diff plugin/rtl2spec/golden 逐案對帳——期望變了而提取器沒變=必須能解釋,`,
    `  否則停(§16.1.4;delta 分類=語義修復/缺陷修復/意外,寫進下方歷史區)`,
    `- 自檢: 每檔每模式進程內 3 跑 hash 全等(兩階段;任何檔不過=全輪拒錄 exit 3)`,
    '',
    '| golden | sha256 |',
    '|---|---|',
    ...provRows.map(([n, h]) => `| ${n} | ${h} |`),
    '',
    '## demo 樣例(不屬本表)',
    '',
    '- golden/plugin-fixtures/ 域=§17 G8 實證 demo(record-golden.mjs 錄製),',
    '  歷史=golden/plugin-fixtures/PROVENANCE.md;run-plugin-check ④d 對其 byte-diff。',
    '',
    historyText,
    `- (re-record ${nowIso}; wasm sha256 ${wasmSha.slice(0, 8)}…)`,
    '',
  ].join('\n');
  writeFileSync(provPath, prov);
  console.log(`== --record: ${files.length} 檔×2 模式錄製完成(+PROVENANCE 元資料區重寫;歷史區保留追加) ==`);
  console.log('== 錄後必讀: git diff plugin/rtl2spec/golden 逐案對帳(§16.1.4) ==');
  process.exit(0);
}

// ================= 默認:產出 current+逐份 byte-diff+PROVENANCE 核對 =================
const currentRoot = join(pluginRoot, 'build', 'golden-current');
mkdirSync(currentRoot, { recursive: true });
let diffs = 0;
let checked = 0;
for (const rel of files) {
  const src = readFileSync(join(repoRoot, rel), 'utf8');
  const pair = runPair(rel, src);
  const [rn, gn] = artifacts(rel);
  for (const [name, content] of [
    [rn, pair[0]],
    [gn, pair[1]],
  ]) {
    const cur = join(currentRoot, name);
    mkdirSync(dirname(cur), { recursive: true });
    writeFileSync(cur, content);
    const goldPath = join(goldenRoot, name);
    checked++;
    if (!existsSync(goldPath)) {
      console.log(`DIFF ${name}: golden 缺(新檔?釘選集變更=語義事件,走 --record)`);
      diffs++;
      continue;
    }
    const gold = readFileSync(goldPath, 'utf8');
    if (content === gold) {
      console.log(`PASS ${name}`);
    } else {
      console.log(`DIFF ${name}: byte 不一致,首個差異鍵路徑=${firstDiffKey(gold, content)}`);
      diffs++;
    }
  }
}

// PROVENANCE 核對(快照被手改檢測;僅正式樹域,golden/plugin-fixtures 不在表)
let provBad = 0;
const provText = readFileSync(join(goldenRoot, 'PROVENANCE.md'), 'utf8');
const provMap = new Map();
for (const m of provText.matchAll(/^\| ([^|]+) \| ([0-9a-f]{64}) \|$/gm)) {
  provMap.set(m[1].trim(), m[2]);
}
for (const rel of files) {
  for (const name of artifacts(rel)) {
    const p = join(goldenRoot, name);
    if (!existsSync(p)) continue; // 已在上方 DIFF 計數
    const actual = sha(readFileSync(p));
    const expect = provMap.get(name);
    if (expect === undefined) {
      console.log(`PROV-DIFF ${name}: PROVENANCE 表無此行(手改/表 stale?--record 重錄即修)`);
      provBad++;
    } else if (expect !== actual) {
      console.log(`PROV-DIFF ${name}: golden 與 PROVENANCE 表不符(golden 被手改?)`);
      provBad++;
    }
  }
}

console.log(
  `== golden-check: ${checked - diffs}/${checked} byte-diff PASS, PROVENANCE 核對 ${provBad === 0 ? 'PASS' : `FAIL×${provBad}`} ==`,
);
if (diffs > 0 || provBad > 0) {
  console.log('== golden 紅≠錯,=語義變了:能解釋→--record 顯式重錄;不能解釋→停,排查提取器(§16.1) ==');
  process.exit(1);
}
process.exit(0);
