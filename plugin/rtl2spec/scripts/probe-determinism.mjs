// probe-determinism.mjs — 提取器確定性探針(§16.0 F1 驗收/§16.1.5 契約面)。
//
// S8 自驗①②:全釘選檔單 bun 進程內重複調用(wasm 腿=dist 同一產物;C-S2 起
//   含 rtl_tb_plan_sv 逐模組幀=規則書+計劃+模板三面同守 §16.1.5 契約),
//   ①同進程 N 跑 byte-identical(16.1.5 契約①;F1a/F1b 修後 flapper 清零);
//   ②調用序擾動(R17-8 復審修訂,頭注對齊實現)——全檔正序跑完後本檔
//     再跑一輪,擾動輪 hash 必∈N 輪 hash 集(契約②輸出禁依賴調用流
//     位置;F1b loops.v 反例的機械化)。殘差注記:無倒序階段,清單首檔
//     無跨檔前序覆蓋——跨檔-only 耦合為理論殘餘面(主風險=同檔抖動
//     已由 N 輪覆蓋);若未來發現跨檔-only 耦合形,擴倒序階段=語義事件。
//
// 用法:
//   bun scripts/probe-determinism.mjs --runs 12 --list <list-file>
//   bun scripts/probe-determinism.mjs --runs 12 <file> [<file>...]
// list 檔=每行一個 repo 相對路徑(# 註釋行跳過)。
// exit 0=全過;exit 1=任何檔抖動;exit 2=檔缺失/用法錯。
//
// 契約依據:RTL2SPEC-PLUGIN.md §16.1.5(golden 快照合法形三則);
// RandomState 位置耦合面=§16.0 F1b 實證(跨進程無判別力,必須同進程跑)。

import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join, dirname, resolve, isAbsolute } from 'node:path';
import { fileURLToPath } from 'node:url';

const pluginRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const repoRoot = dirname(dirname(pluginRoot));

// 裝載 dist 同一 wasm(顯式位元組 init,與 check-rulebook 同形,D8)。
const wasmDir = join(pluginRoot, 'dist', 'wasm');
const glue = await import(join(wasmDir, 'rtl_spec_wasm.js'));
await glue.default({
  module_or_path: new Uint8Array(readFileSync(join(wasmDir, 'rtl_spec_wasm_bg.wasm'))),
});

// 參數解析
const argv = process.argv.slice(2);
let runs = 4;
let files = [];
for (let i = 0; i < argv.length; i++) {
  if (argv[i] === '--runs') {
    runs = parseInt(argv[++i], 10);
    if (!Number.isInteger(runs) || runs < 2) {
      console.error('usage: --runs 需 ≥2');
      process.exit(2);
    }
  } else if (argv[i] === '--list') {
    const listFile = argv[++i];
    const listPath = isAbsolute(listFile) ? listFile : resolve(repoRoot, listFile);
    files.push(
      ...readFileSync(listPath, 'utf8')
        .split('\n')
        .map((l) => l.trim())
        .filter((l) => l && !l.startsWith('#')),
    );
  } else {
    files.push(argv[i]);
  }
}
if (files.length === 0) {
  console.error('usage: probe-determinism.mjs [--runs N] (--list <list-file> | <file>...)');
  process.exit(2);
}

const sha = (s) => createHash('sha256').update(s).digest('hex');

let bad = 0;
for (const rel of files) {
  const abs = resolve(repoRoot, rel);
  let source;
  try {
    source = readFileSync(abs, 'utf8');
  } catch (e) {
    console.log(`FAIL ${rel}: 讀檔失敗 ${e.message}`);
    bad++;
    continue;
  }
  // tbplan 面(C-S2):逐模組取幀(含 TB1 錯誤幀,其同樣須確定),多模組檔也覆蓋。
  const moduleNames = JSON.parse(glue.rtl_extract_rules_sv(rel, source, 256)).modules.map((m) => m.module);
  const tbFrames = () =>
    moduleNames.map((m) => glue.rtl_tb_plan_sv(rel, source, m, 0, 0, 0)).join('\n');
  // ① 同進程 N 跑(rules/gaps/tbplan 三模式)
  const ruleHashes = new Set();
  const gapHashes = new Set();
  const tbHashes = new Set();
  for (let k = 0; k < runs; k++) {
    ruleHashes.add(sha(glue.rtl_extract_rules_sv(rel, source, 256)));
    gapHashes.add(sha(glue.rtl_scan_gaps_sv(rel, source)));
    tbHashes.add(sha(tbFrames()));
  }
  // ② 調用序擾動:全檔正序跑完後本檔再跑一輪(此時調用流位置已不同)
  const rulePert = sha(glue.rtl_extract_rules_sv(rel, source, 256));
  const gapPert = sha(glue.rtl_scan_gaps_sv(rel, source));
  const tbPert = sha(tbFrames());
  const okRules = ruleHashes.size === 1;
  const okGaps = gapHashes.size === 1 && gapHashes.has(gapPert);
  const okTb = tbHashes.size === 1 && tbHashes.has(tbPert);
  if (okRules && okGaps && okTb) {
    console.log(`PASS ${rel} (runs=${runs} rules=${[...ruleHashes][0].slice(0, 8)} gaps=${[...gapHashes][0].slice(0, 8)} tb=${[...tbHashes][0].slice(0, 8)} modules=${moduleNames.length})`);
  } else {
    console.log(`FAIL ${rel} rules_kinds=${ruleHashes.size} gaps_kinds=${gapHashes.size} gaps_perturb_match=${gapHashes.has(gapPert)} rules_perturb_match=${ruleHashes.has(rulePert)} tb_kinds=${tbHashes.size} tb_perturb_match=${tbHashes.has(tbPert)}`);
    bad++;
  }
}

console.log(`== probe-determinism: ${files.length - bad}/${files.length} PASS (runs=${runs}) ==`);
process.exit(bad === 0 ? 0 : 1);
