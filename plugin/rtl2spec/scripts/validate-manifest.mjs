// manifest 靜態校驗(§7.4 裝載驗證第一步;repo 內完成,禁動 ~/.zcode)。
// 斷言:必填欄位/型別、佔位符可解析、skills 三目錄在場且 YAML frontmatter
// 含 name+description、dist 產物在場並列印 wasm sha256(報告溯源指紋)。
// exit 0=PASS / 1=FAIL。

import { readFileSync, existsSync, statSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const manifestPath = join(root, '.zcode-plugin', 'plugin.json');

let failures = 0;
function check(name, cond, detail = '') {
  console.log(`${cond ? 'PASS' : 'FAIL'} ${name}${cond ? '' : ` — ${detail}`}`);
  if (!cond) failures++;
}

function readJson(p) {
  return JSON.parse(readFileSync(p, 'utf8'));
}

const PLACEHOLDERS = ['${ZCODE_PLUGIN_ROOT}', '${ZCODE_PROJECT_DIR}'];

const m = readJson(manifestPath);
check('manifest name string', typeof m.name === 'string' && m.name.length > 0);
check('manifest version string', typeof m.version === 'string' && /^\d+\.\d+\.\d+/.test(m.version));
check('manifest description string', typeof m.description === 'string' && m.description.length > 0);
check('manifest author object', typeof m.author === 'object' && m.author !== null);
check('manifest license string', typeof m.license === 'string' && m.license === 'MIT');
check(
  'manifest skills dir declared',
  typeof m.skills === 'string' && m.skills === 'skills',
);
check(
  'manifest mcpServers object',
  typeof m.mcpServers === 'object' && m.mcpServers !== null && typeof m.mcpServers.rtl2spec === 'object',
);
const srv = m.mcpServers?.rtl2spec ?? {};
check('mcp stdio type', srv.type === 'stdio');
check('mcp command bun', srv.command === 'bun');
check('mcp args non-empty', Array.isArray(srv.args) && srv.args.length > 0);
check('mcp cwd present', typeof srv.cwd === 'string' && srv.cwd.length > 0);
check('mcp env RTL2SPEC_WASM_DIR', typeof srv.env?.RTL2SPEC_WASM_DIR === 'string');

// 佔位符可解析:出現的佔位符都在白名單;無未知 ${...}。
const raw = readFileSync(manifestPath, 'utf8');
const found = [...raw.matchAll(/\$\{([A-Z_]+)\}/g)].map((x) => `\${${x[1]}}`);
check(
  'placeholders whitelisted',
  found.every((p) => PLACEHOLDERS.includes(p)),
  `found: ${found.join(', ') || '(none)'}`,
);
check(
  'ZCODE_PLUGIN_ROOT used',
  raw.includes('${ZCODE_PLUGIN_ROOT}'),
  '裝載根變數必用(server/wasm 以插件根定位)',
);

// skills 三目錄在場且 frontmatter 含 name+description。
const skillDirs = ['rtl-analyze', 'rtl-rulebook', 'rtl-gap-hunt'];
for (const d of skillDirs) {
  const p = join(root, m.skills ?? 'skills', d, 'SKILL.md');
  const inPlace = existsSync(p) && statSync(p).isFile();
  check(`skill ${d} SKILL.md in place`, inPlace);
  if (!inPlace) continue;
  const text = readFileSync(p, 'utf8');
  const fm = text.match(/^---\n([\s\S]*?)\n---/);
  check(`skill ${d} frontmatter`, fm !== null);
  if (fm) {
    check(`skill ${d} name`, /^name:\s*\S/m.test(fm[1]));
    check(`skill ${d} description`, /^description:\s*\S/m.test(fm[1]));
  }
}

// dist 產物在場 + wasm sha256(報告溯源)。
for (const f of ['dist/server.mjs', 'dist/server.d.ts', 'dist/wasm/rtl_spec_wasm.js', 'dist/wasm/rtl_spec_wasm_bg.wasm']) {
  const p = join(root, f);
  check(`dist ${f} in place`, existsSync(p) && statSync(p).isFile());
}
const wasmPath = join(root, 'dist/wasm/rtl_spec_wasm_bg.wasm');
if (existsSync(wasmPath)) {
  const sha = createHash('sha256').update(readFileSync(wasmPath)).digest('hex');
  console.log(`INFO wasm sha256 ${sha}`);
}

if (failures > 0) {
  console.log(`VALIDATE FAIL: ${failures} failure(s)`);
  process.exit(1);
}
console.log('VALIDATE PASS');
