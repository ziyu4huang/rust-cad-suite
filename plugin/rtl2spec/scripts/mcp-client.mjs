// MCP stdio 端到端回環驅動(run-mcp-e2e.sh 的斷言腿;§8.1 ③)。
// spawn server → newline 幀回環 → 21 斷言(實跑 PASS 行數,iverilog 在場時;D-S3 起=§19 C-S2 形+rtl_spec_lint ⑫⑬)→ kill 子進程。
// exit 0=PASS / 1=斷言失敗。
//
// 斷言(PASS 行 21:①②③③b④④b⑤⑥⑦⑧a⑧b⑧c⑧d⑨a⑨b⑨c⑩⑪⑫⑬+ping;iverilog 不在場時 ⑨c=SKIP 行,PASS 20):
// ⑫rtl_spec_lint 正樣(D-S3):fixtures/d-spec/counter8.spec.md→findings 空陣列+count=0+isError=false
// ⑬rtl_spec_lint 負樣:inline source 缺 spec-format/module/port/behavior/scenarios→count≥1(①..⑦ 起首)
// ①initialize(serverInfo.name=="rtl2spec" 且 protocolVersion 回顧)
// ②tools/list 恰 5 工具且 inputSchema.type=="object"
// ③rtl_extract_rules@fixtures/fsm_counter.v:modules>=1 且 rules>=3
// ④rtl_scan_gaps@fixtures/latch_trap.v:gaps 含 latch(正樣=@* 形=分類律
//   R1-1 端到端回歸錨)
// ⑤負樣:rtl_extract_rules@不存在檔案 → isError:true
// ⑥負樣:rtl_analyze path/source 雙缺 → isError:true(R1-11)
// ⑦負樣:未知名 method → JSON-RPC error -32601
// ⑧D-r1a 效果(§19.2 C-S1):i2c_master.v 不傳/顯式 0→152 零截斷、顯式 100→100+
//   truncated=[i2c_master]、batch→152(面委託核心 DEFAULT_MAX_RULES)
// ⑨rtl_tb_plan 正樣(§19.8 C-S2):fixtures/fsm_counter.v→ok、clocks/resets≥1、協議標記;
//   模板與 fixture 同編 `iverilog -g2005 -t null` exit 0 且零警告(在場才執法,不在場 SKIP)
// ⑩負樣:parametric 端口→isError 且含 milestone TB1(fixtures/tb1_parametric.v)
// ⑪負樣:.il 輸入→isError 且含 milestone TB1(證 lang 分派可達,RV2-8)

import { spawn, spawnSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
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

  // ② tools/list 恰 5 工具(ADR-0013 D-C11 修訂 D4)。
  const listResp = await send('tools/list', {});
  const tools = listResp.result?.tools ?? [];
  const names = tools.map((t) => t.name).sort();
  check(
    '② tools/list exactly 6 with object schemas',
    names.length === 6 &&
      JSON.stringify(names) ===
        JSON.stringify(['rtl_analyze', 'rtl_extract_batch', 'rtl_extract_rules', 'rtl_scan_gaps', 'rtl_spec_lint', 'rtl_tb_plan']) &&
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

  // ⑧ D-r1a 效果斷言(§19.2 C-S1):max_rules 面委託核心 SSOT(DEFAULT_MAX_RULES)。
  // 語料唯讀:corpus/e2e/59_i2c_master/i2c_master.v(cwd=插件根→../../)。修前=128+truncated。
  const I2C = '../../corpus/e2e/59_i2c_master/i2c_master.v';
  const rulesOf = (resp) => {
    const book = resp.result?.structuredContent?.rulebook;
    return {
      n: (book?.modules ?? []).reduce((acc, m) => acc + m.rules.length, 0),
      truncated: JSON.stringify(book?.truncated ?? null),
      err: resp.result?.isError,
    };
  };
  const r8a = rulesOf(await send('tools/call', {
    name: 'rtl_extract_rules',
    arguments: { path: I2C, format: 'json' },
  }));
  check('⑧a i2c 不傳 max → 152 條且零截斷', r8a.err === false && r8a.n === 152 && r8a.truncated === '[]', JSON.stringify(r8a));
  const r8b = rulesOf(await send('tools/call', {
    name: 'rtl_extract_rules',
    arguments: { path: I2C, format: 'json', max_rules_per_module: 0 },
  }));
  check('⑧b i2c 顯式 0 → 152 條且零截斷(0=核心默認)', r8b.err === false && r8b.n === 152 && r8b.truncated === '[]', JSON.stringify(r8b));
  const r8c = rulesOf(await send('tools/call', {
    name: 'rtl_extract_rules',
    arguments: { path: I2C, format: 'json', max_rules_per_module: 100 },
  }));
  check('⑧c i2c 顯式 100 → 100 條且 truncated=[i2c_master]', r8c.err === false && r8c.n === 100 && r8c.truncated === '["i2c_master"]', JSON.stringify(r8c));
  const batchResp = await send('tools/call', {
    name: 'rtl_extract_batch',
    arguments: { paths: [I2C] },
  });
  const batchRows = batchResp.result?.structuredContent?.results ?? [];
  check(
    '⑧d batch i2c rule_count=152',
    batchResp.result?.isError === false && batchRows.length === 1 && batchRows[0].ok === true && batchRows[0].rule_count === 152,
    JSON.stringify(batchRows).slice(0, 160),
  );

  // ⑨ rtl_tb_plan 正樣(§19.8):計劃+模板+同編零警告(contract 寬度=TB 可編譯性前提,D-c2)。
  const tbResp = await send('tools/call', {
    name: 'rtl_tb_plan',
    arguments: { path: 'fixtures/fsm_counter.v' },
  });
  const tbSc = tbResp.result?.structuredContent;
  check(
    '⑨a tb_plan ok clocks>=1 resets>=1',
    tbResp.result?.isError === false &&
      tbSc?.ok === true &&
      (tbSc?.plan?.clocks?.length ?? 0) >= 1 &&
      (tbSc?.plan?.resets?.length ?? 0) >= 1,
    JSON.stringify(tbResp.result ?? tbResp).slice(0, 160),
  );
  const tpl = tbSc?.template_v ?? '';
  check(
    '⑨b template has // SCENES: + SCENE1 + DONE',
    tpl.includes('// SCENES:') && tpl.includes('SCENE1') && tpl.includes('DONE'),
    tpl.slice(0, 120),
  );
  const ivProbe = spawnSync('iverilog', ['-V'], { encoding: 'utf8' });
  if (ivProbe.error) {
    console.log('SKIP ⑨c template compiles zero-warning (iverilog absent)');
  } else {
    const tbDir = join(pluginRoot, 'build', 'mcp-e2e');
    mkdirSync(tbDir, { recursive: true });
    const tbPath = join(tbDir, 'tb_fsm_counter.v');
    writeFileSync(tbPath, tpl);
    const iv = spawnSync('iverilog', ['-g2005', '-t', 'null', tbPath, 'fixtures/fsm_counter.v'], {
      cwd: pluginRoot,
      encoding: 'utf8',
    });
    check(
      '⑨c template compiles zero-warning (iverilog -g2005 -t null)',
      iv.status === 0 && iv.stderr.trim() === '' && iv.stdout.trim() === '',
      `rc=${iv.status} stderr=${iv.stderr.slice(0, 160)}`,
    );
  }

  // ⑩ 負樣:parametric 端口→TB1。
  const tbParam = await send('tools/call', {
    name: 'rtl_tb_plan',
    arguments: { path: 'fixtures/tb1_parametric.v' },
  });
  check(
    '⑩ tb_plan parametric port → isError+milestone TB1',
    tbParam.result?.isError === true &&
      String(tbParam.result?.content?.[0]?.text ?? '').includes('milestone TB1'),
    JSON.stringify(tbParam.result ?? tbParam).slice(0, 160),
  );

  // ⑪ 負樣:.il 輸入(langOf 副檔名分派)→TB1(語料唯讀)。
  const tbIl = await send('tools/call', {
    name: 'rtl_tb_plan',
    arguments: { path: '../../crates/rtl-spec/tests/fixtures/sample.il' },
  });
  check(
    '⑪ tb_plan .il input → isError+milestone TB1',
    tbIl.result?.isError === true &&
      String(tbIl.result?.content?.[0]?.text ?? '').includes('milestone TB1'),
    JSON.stringify(tbIl.result ?? tbIl).slice(0, 160),
  );

  // ⑫ rtl_spec_lint 正樣(D-S3):正樣語料零 findings。
  const specLintResp = await send('tools/call', {
    name: 'rtl_spec_lint',
    arguments: { path: 'fixtures/d-spec/counter8.spec.md' },
  });
  const slSc = specLintResp.result?.structuredContent;
  check(
    '⑫ spec_lint counter8 → findings empty count=0',
    specLintResp.result?.isError === false &&
      Array.isArray(slSc?.findings) &&
      slSc.findings.length === 0 &&
      slSc.count === 0,
    JSON.stringify(specLintResp.result ?? specLintResp).slice(0, 160),
  );

  // ⑬ rtl_spec_lint 負樣:inline 劣化 spec(缺 spec-format/module/port/behavior/scenarios)→count≥1。
  const specBadResp = await send('tools/call', {
    name: 'rtl_spec_lint',
    arguments: { name: 'broken.spec.md', source: '- module: m\n' },
  });
  const badSc = specBadResp.result?.structuredContent;
  const badFindings = badSc?.findings ?? [];
  check(
    '⑬ spec_lint broken spec → count>=1 ①-prefixed',
    specBadResp.result?.isError === false &&
      badSc?.count >= 1 &&
      badFindings.length === badSc.count &&
      badFindings.every((f) => /^[①-⑦]/.test(f)),
    JSON.stringify(badFindings).slice(0, 160),
  );

  // 額外:ping。  // 額外:ping。
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
