// dleg-prompt.mjs — D 腿 prompt 實例化(CONFORMANCE-PROTOCOL.md §5;C-S4)。確定性,寫只落 build/。
// 模板=CONFORMANCE-PROTOCOL.md 的 §1/§2 模板塊逐字(本腳本按圍欄抽取,不複製模板文本——
// 協議檔是模板的唯一來源)。硬同步(協議 §6):一律取最高版本圍欄並打印其 sha;全部版本的內容錨
// (v1=§19.3.4 預登記 cc28f0c7…)開工重算,任一不符 exit 2,禁回退舊版(proto-template.mjs)。
// 用法:
//   bun plugin/rtl2spec/scripts/conformance/dleg-prompt.mjs r0 <design> [K] [N]
//     → build/conformance/dleg/<design>/prompt_r0_k<K>.md(§1 TB 生成模板)
//   bun plugin/rtl2spec/scripts/conformance/dleg-prompt.mjs rN <design> <prev-tb> <try-record.json> [K]
//     → build/conformance/dleg/<design>/prompt_r<N>_k<K>.md(§2 TB 改進輪模板;存活清單只列規則書內容,
//       不給變異源碼行——防源碼經回灌泄漏,§19.6.2 F2)
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

import { latestTemplate } from './proto-template.mjs';

const pluginRoot = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
const repoRoot = dirname(dirname(pluginRoot));
const { renderRulebookMarkdown } = await import(join(pluginRoot, 'dist', 'md.mjs'));

const [round, design, ...rest] = process.argv.slice(2);
if (!round || !design) {
  console.error('usage: dleg-prompt.mjs r0 <design> [K] [N] | rN <design> <prev-tb> <try-record.json> [K]');
  process.exit(2);
}
let tpl;
try {
  tpl = latestTemplate(readFileSync(join(pluginRoot, 'CONFORMANCE-PROTOCOL.md'), 'utf8'));
} catch (e) {
  console.error(`CONFORMANCE-PROTOCOL.md 模板硬同步失敗: ${e.message}`);
  process.exit(2);
}
const block = tpl.lines;
console.log(`INFO 模板 v${tpl.version} 塊 ${block.length} 行 sha256=${tpl.sha256}`);
const section = (title) => {
  const a = block.findIndex((l) => l.startsWith(title));
  const b = block.findIndex((l, k) => k > a && /^\[§[0-9]/.test(l));
  return block.slice(a + 1, b < 0 ? block.length : b).join('\n').replace(/\n+$/, '');
};

const cs = JSON.parse(readFileSync(join(pluginRoot, 'fixtures', 'conformance', design, 'case.json'), 'utf8'));
const rb = JSON.parse(readFileSync(join(repoRoot, cs.specPath), 'utf8'));
const wasmDir = join(pluginRoot, 'dist', 'wasm');
const glue = await import(join(wasmDir, 'rtl_spec_wasm.js'));
await glue.default({ module_or_path: new Uint8Array(readFileSync(join(wasmDir, 'rtl_spec_wasm_bg.wasm'))) });
const src = readFileSync(join(repoRoot, cs.src), 'utf8');
const frame = JSON.parse(glue.rtl_tb_plan_sv(cs.src, src, cs.top, cs.seed ?? 0, cs.cycles ?? 0, cs.flush ?? 0));
if (!frame.ok) {
  console.error(frame.error);
  process.exit(1);
}
// 規則書只給該模組(單模組設計);evidence 塊照 A 組口徑完整給出(規則書=提取器真實輸出)。
const rbOne = { ...rb, modules: rb.modules.filter((m) => m.module === cs.top) };
const outDir = join(pluginRoot, 'build', 'conformance', 'dleg', design);
mkdirSync(outDir, { recursive: true });

let text;
let outName;
if (round === 'r0') {
  const K = rest[0] ?? '1';
  const N = rest[1] ?? '1';
  outName = `prompt_r0_k${K}.md`;
  const OUT = relative(repoRoot, join(outDir, `tb_r0_k${K}.v`));
  text = section('[§1 TB 生成模板]')
    .replace('{OUT}', OUT)
    .replace('{RULEBOOK_MD}', renderRulebookMarkdown(rbOne))
    .replace('{TB_PLAN_JSON}', JSON.stringify(frame.plan, null, 2))
    .replace('{SKELETON_V}', frame.template_v)
    .replace('{K}', K)
    .replace('{N}', N);
} else if (/^r[1-3]$/.test(round)) {
  const [prevTb, tryRecord, K0] = rest;
  const K = K0 ?? '1';
  const R = round.slice(1);
  const rec = JSON.parse(readFileSync(tryRecord, 'utf8'));
  // 存活 eligible 變異的歸屬規則(去重,規則序);只列規則書內容(id/陳述/check),不給變異源碼行。
  const surv = new Set(
    rec.mutants
      .filter((m) => m.pool === 'rule' && m.result === 'survived' && m.adjudication !== 'equivalent')
      .flatMap((m) => m.ruleIds),
  );
  const mod = rbOne.modules[0];
  const lines = [];
  for (const r of mod.rules) {
    if (!surv.has(r.id)) continue;
    lines.push(`- ${r.id} [${r.category}] ${r.statement}`);
    lines.push(`  check(${r.check.form}):${JSON.stringify(r.check)}`);
  }
  outName = `prompt_r${R}_k${K}.md`;
  text = section('[§2 TB 改進輪模板]')
    .replace('{PREV_TB}', relative(repoRoot, prevTb))
    .replace('{SURVIVORS_MD}', lines.join('\n') || '(無)')
    .replace('{R}', R);
} else {
  console.error(`未知輪次 ${round}`);
  process.exit(2);
}
const dst = join(outDir, outName);
writeFileSync(dst, text + '\n');
console.log(`== prompt: ${relative(repoRoot, dst)}(${text.split('\n').length} 行;sha256=${createHash('sha256').update(text).digest('hex').slice(0, 16)}) ==`);
