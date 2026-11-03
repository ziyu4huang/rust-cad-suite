// MCP stdio 端到端回環驅動(run-mcp-e2e.sh 的斷言腿;§8.1 ③)。
// spawn server → newline 幀回環 → 七斷言 → kill 子進程。
// exit 0=PASS / 1=斷言失敗。
//
// 七斷言:①initialize(serverInfo.name=="rtl2spec" 且 protocolVersion 回顧)
// ②tools/list 恰 4 工具且 inputSchema.type=="object"
// ③rtl_extract_rules@fixtures/fsm_counter.v:modules>=1 且 rules>=3
// ④rtl_scan_gaps@fixtures/latch_trap.v:gaps 含 latch(正樣=@* 形=分類律
//   R1-1 端到端回歸錨)
// ⑤負樣:rtl_extract_rules@不存在檔案 → isError:true
// ⑥負樣:rtl_analyze path/source 雙缺 → isError:true(R1-11)
// ⑦負樣:未知名 method → JSON-RPC error -32601

import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const pluginRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const serverPath = join(pluginRoot, 'dist', 'server.mjs');

const child = spawn('bun', [serverPath], {
  cwd: pluginRoot, // path 解析基準=插件根(fixtures/…)
  stdio: ['pipe', 'pipe', 'pipe'],
});
let stderrBuf = '';
child.stderr.on('data', (d) => {
  stderrBuf += d.toString();
});

let failures = 0;
function check(name, cond, detail = '') {
  console.log(`${cond ? 'PASS' : 'FAIL'} ${name}${cond ? '' : ` — ${detail}`}`);
  if (!cond) failures++;
}

const rl = createInterface({ input: child.stdout, terminal: false });
const pending = new Map();
let nextId = 1;

function send(method, params) {
  const id = nextId++;
  child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n');
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => reject(new Error(`timeout waiting ${method} id=${id}`)), 15000);
    pending.set(id, {
      resolve: (v) => {
        clearTimeout(t);
        resolve(v);
      },
    });
  });
}

function notify(method, params) {
  child.stdin.write(JSON.stringify({ jsonrpc: '2.0', method, params }) + '\n');
}

rl.on('line', (line) => {
  const text = line.trim();
  if (!text) return;
  let msg;
  try {
    msg = JSON.parse(text);
  } catch {
    console.log(`FAIL stdout purity: non-JSON line: ${text.slice(0, 80)}`);
    failures++;
    return;
  }
  if (msg.id !== undefined && pending.has(msg.id)) {
    pending.get(msg.id).resolve(msg);
    pending.delete(msg.id);
  }
});

async function main() {
  // ① initialize(請求版在 SUPPORTED 列表 → 回顧)。
  const initResp = await send('initialize', {
    protocolVersion: '2025-06-18',
    capabilities: {},
    clientInfo: { name: 'rtl2spec-e2e', version: '0.0.0' },
  });
  check(
    '① initialize serverInfo+version echo',
    initResp.result?.serverInfo?.name === 'rtl2spec' &&
      initResp.result?.protocolVersion === '2025-06-18',
    JSON.stringify(initResp).slice(0, 120),
  );
  notify('notifications/initialized', {});

  // ② tools/list 恰 4 工具。
  const listResp = await send('tools/list', {});
  const tools = listResp.result?.tools ?? [];
  const names = tools.map((t) => t.name).sort();
  check(
    '② tools/list exactly 4 with object schemas',
    names.length === 4 &&
      JSON.stringify(names) ===
        JSON.stringify(['rtl_analyze', 'rtl_extract_batch', 'rtl_extract_rules', 'rtl_scan_gaps']) &&
      tools.every((t) => t.inputSchema?.type === 'object'),
    JSON.stringify(names),
  );

  // ③ 正樣:規則書(fsm_counter.v:reset×3+fsm×3+counter×1 ≥3)。
  const rulesResp = await send('tools/call', {
    name: 'rtl_extract_rules',
    arguments: { path: 'fixtures/fsm_counter.v' },
  });
  const rb = rulesResp.result?.structuredContent?.rulebook;
  const totalRules = (rb?.modules ?? []).reduce((n, m) => n + m.rules.length, 0);
  check(
    '③ extract_rules modules>=1 rules>=3',
    rulesResp.result?.isError === false &&
      (rb?.modules?.length ?? 0) >= 1 &&
      totalRules >= 3,
    `modules=${rb?.modules?.length} rules=${totalRules}`,
  );
  check(
    '③b markdown field present (default both)',
    typeof rulesResp.result?.structuredContent?.markdown === 'string' &&
      rulesResp.result.structuredContent.markdown.includes('# 規則書:'),
  );

  // ④ 正樣:破口(latch_trap.v @* 形 → latch 類)。
  const gapsResp = await send('tools/call', {
    name: 'rtl_scan_gaps',
    arguments: { path: 'fixtures/latch_trap.v' },
  });
  const gaps = gapsResp.result?.structuredContent?.gaps ?? [];
  check(
    '④ scan_gaps finds latch (@* fixture)',
    gaps.some((g) => g.class === 'latch'),
    JSON.stringify(gaps).slice(0, 120),
  );
  check(
    '④b test_plan mirrors suggested_test',
    Array.isArray(gapsResp.result?.structuredContent?.test_plan) &&
      gapsResp.result.structuredContent.test_plan.length === gaps.length,
  );

  // ⑤ 負樣:不存在檔案 → isError。
  const missingResp = await send('tools/call', {
    name: 'rtl_extract_rules',
    arguments: { path: 'fixtures/no_such_file.v' },
  });
  check('⑤ missing file → isError:true', missingResp.result?.isError === true);

  // ⑥ 負樣:path/source 雙缺 → isError(R1-11)。
  const bothMissing = await send('tools/call', {
    name: 'rtl_analyze',
    arguments: {},
  });
  check('⑥ path+source both missing → isError:true', bothMissing.result?.isError === true);

  // ⑦ 負樣:未知名 method → -32601。
  const unknownResp = await send('bogus/method', {});
  check(
    '⑦ unknown method → -32601',
    unknownResp.error?.code === -32601,
    JSON.stringify(unknownResp).slice(0, 80),
  );

  // 額外:ping。
  const pingResp = await send('ping', {});
  check('ping → {}', JSON.stringify(pingResp.result) === '{}');
}

main()
  .catch((e) => {
    console.log(`FAIL driver: ${e.message}`);
    failures++;
  })
  .finally(() => {
    child.kill('SIGTERM');
    setTimeout(() => {
      if (stderrBuf.trim()) {
        console.log(`INFO server stderr: ${stderrBuf.trim().slice(0, 200)}`);
      }
      if (failures > 0) {
        console.log(`MCP-E2E FAIL: ${failures} failure(s)`);
        process.exit(1);
      }
      console.log('MCP-E2E PASS');
      process.exit(0);
    }, 200);
  });
