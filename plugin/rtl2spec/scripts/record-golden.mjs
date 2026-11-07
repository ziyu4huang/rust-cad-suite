// record-golden.mjs — golden 錄製器(wasm 腿=dist 同一產物,D8 閘驗出貨物;§16.1.3/S9 迷你版)。
// 用法:bun scripts/record-golden.mjs <file> [<file>...](file=repo 相對路徑;
//       golden 落點=golden/plugin-fixtures/<stem>.<mode>.json)
// 域界定(S9 起釘明):本器只管 plugin-fixtures demo 域(§17 G8 實證樣例),
//   PROVENANCE=golden/plugin-fixtures/PROVENANCE.md(追加式歷史日志);
//   正式釘選集(檔數=pinned-files.list)golden/ 樹與 golden/PROVENANCE.md(全量重寫形)歸
//   run-golden-regression.sh --record 管轄,兩域互不覆蓋。
// 契約(§16.1.4/--record 前置自檢):錄製=真實運行,禁手寫;每檔先進程內
// 12 跑 hash 自檢(§16.1.4 實現形=進程內重複調用),不唯一→拒錄 exit 3;
// 錄後必讀 git diff golden/ 逐案對帳(re-record=語義事件)。
// exit:0=全錄;3=自檢不過;2=用法/檔缺。
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join, dirname, basename } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
const goldenDir = join(root, 'plugin', 'rtl2spec', 'golden', 'plugin-fixtures');
const files = process.argv.slice(2).filter((a) => !a.startsWith('--'));
if (files.length === 0) { console.error('用法: bun record-golden.mjs <file>...'); process.exit(2); }

// 裝載面=plugin/rtl2spec/dist/wasm(交付面;與 run-plugin-check ④d/probe-determinism/check-rulebook
// 同一產物——pkg/dist 脫節時 golden 不錯釘,R17-2 復審修訂:原誤裝 pkg=錄製面與驗證面平行)
const distWasm = join(root, 'plugin', 'rtl2spec', 'dist', 'wasm');
const { default: init } = await import(join(distWasm, 'rtl_spec_wasm.js'));
await init({ module_or_path: new Uint8Array(readFileSync(join(distWasm, 'rtl_spec_wasm_bg.wasm'))) });
const api = await import(join(distWasm, 'rtl_spec_wasm.js'));

let failed = 0;
for (const f of files) {
  const abs = join(root, f);
  const src = readFileSync(abs, 'utf8');
  // ① 進程內 12 跑自檢(§16.1.4;F1 類抖動在錄製前攔)
  const hashes = new Set();
  let last = '';
  for (let i = 0; i < 12; i++) {
    last = api.rtl_scan_gaps_sv(f, src);
    hashes.add(createHash('sha256').update(last).digest('hex'));
  }
  if (hashes.size !== 1) {
    console.log(`FAIL 自檢 ${f}: 12 跑 ${hashes.size} 種 hash — 拒錄(exit 3;16.1.4 契約)`);
    failed++;
    continue;
  }
  // ② 第 13 跑(真實運行)寫盤
  const out = api.rtl_scan_gaps_sv(f, src);
  mkdirSync(goldenDir, { recursive: true });
  const dst = join(goldenDir, `${basename(f).replace(/\.v$/, '')}.gaps.json`);
  writeFileSync(dst, out);
  // ③ PROVENANCE 機械追加(禁手改;re-record 歷史=語義事件日志 §16.1.4)
  const provPath = join(goldenDir, 'PROVENANCE.md');
  const { existsSync } = await import('node:fs');
  if (!existsSync(provPath)) {
    writeFileSync(provPath, '# golden PROVENANCE — plugin-fixtures demo 域(機械生成禁手改;record-golden.mjs 維護,追加式歷史日志)\n\n');
  }
  const wasmVer = api.rtl_spec_version();
  const line = `- ${new Date().toISOString()} | ${f} | sha256=${createHash('sha256').update(out).digest('hex')} | wasm ABI=${wasmVer} | 自檢 12 跑唯一 | bun=${process.version}\n`;
  let prov = readFileSync(provPath, 'utf8');
  if (!prov.endsWith('\n')) prov += '\n';
  writeFileSync(provPath, prov + line);
  console.log(`PASS 錄製 ${f} → golden/plugin-fixtures/${basename(dst)} (sha256=${createHash('sha256').update(out).digest('hex').slice(0, 12)}… ABI=${wasmVer})`);
}
process.exit(failed ? 3 : 0);
