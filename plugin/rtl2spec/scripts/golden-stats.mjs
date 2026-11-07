// golden-stats.mjs — golden 規則書計數資訊行(§19.2 D-r1b 防復發;唯讀,只寫 stdout)。
// 用法: bun plugin/rtl2spec/scripts/golden-stats.mjs [golden-dir]
//   缺省 golden-dir=本腳本所在 plugin/rtl2spec/golden/(可指向 G0 副本等同構樹)。
// 輸出恰一行:
//   STATS rules=… seq=… rel=… guard=… trans=… async_true=… guard_2ev=… seq_when=… truncated=…
// 口徑(逐條;「計數一律實測」機械化——此後文檔對賬數字一律引本行原文):
//   rules      = 全部 *.rules.json 的 modules[].rules[] 條數總和
//   seq/rel/guard/trans = check.form 為 sequence/relation/guard/transition 的條數
//   async_true = form=guard 且 check.async===true
//   guard_2ev  = form=guard 且 evidence 條數 ≥2(敏感列表第二證)
//   seq_when   = form=sequence 且 check.when 為非空字串
//   truncated  = 各檔 rulebook.truncated[] 長度總和(被截斷模組數)
// 由 run-golden-regression.sh 尾部打印(資訊行,不斷言;斷言仍是 byte-exact)。
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(process.argv[2] ?? join(here, '..', 'golden'));

function walk(dir) {
  const out = [];
  for (const name of readdirSync(dir).sort()) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) out.push(...walk(p));
    else if (name.endsWith('.rules.json')) out.push(p);
  }
  return out;
}

const files = walk(root);
if (files.length === 0) {
  console.error(`golden-stats: no *.rules.json under ${root}`);
  process.exit(2);
}
const n = {
  rules: 0, seq: 0, rel: 0, guard: 0, trans: 0,
  async_true: 0, guard_2ev: 0, seq_when: 0, truncated: 0,
};
for (const f of files) {
  const rb = JSON.parse(readFileSync(f, 'utf8'));
  n.truncated += Array.isArray(rb.truncated) ? rb.truncated.length : 0;
  for (const m of rb.modules ?? []) {
    for (const r of m.rules ?? []) {
      n.rules += 1;
      const c = r.check ?? {};
      switch (c.form) {
        case 'sequence':
          n.seq += 1;
          if (typeof c.when === 'string' && c.when.length > 0) n.seq_when += 1;
          break;
        case 'relation':
          n.rel += 1;
          break;
        case 'guard':
          n.guard += 1;
          if (c.async === true) n.async_true += 1;
          if ((r.evidence ?? []).length >= 2) n.guard_2ev += 1;
          break;
        case 'transition':
          n.trans += 1;
          break;
        default:
          break;
      }
    }
  }
}
console.log(
  `STATS rules=${n.rules} seq=${n.seq} rel=${n.rel} guard=${n.guard} trans=${n.trans} ` +
    `async_true=${n.async_true} guard_2ev=${n.guard_2ev} seq_when=${n.seq_when} truncated=${n.truncated}`,
);
