// rtl2spec MCP server (ADR-0012, §6;手寫純 JS ESM,源=產物零編譯步,D11)。
//
// stdio newline 分幀 JSON-RPC 2.0;stdout 純淨律:stdout 只出 JSON-RPC 幀,
// 一切診斷走 stderr。語義零:收參→裝載 wasm→調導出→編幀(10.7 core-first)。
//
// 方法面:initialize(SUPPORTED 五版列表內回顧,否則 LATEST)/
// notifications/initialized(吞)/tools/list/tools/call/ping。
// 工具內錯誤(檔案不存在/path+source 雙缺/未支援 lang)→ isError:true 的
// result;協議級錯誤(未知名 -32601/壞參 -32602)→ JSON-RPC error。
// 裝載:RTL2SPEC_WASM_DIR(env;缺省=本目錄 wasm/)讀
// rtl_spec_wasm_bg.wasm 位元組→顯式 init——載入失敗=exit 1(stderr 說明),
// 禁「降級假活」。

import { readFileSync, existsSync } from 'node:fs';
import { createInterface } from 'node:readline';
import { join, dirname, extname } from 'node:path';
import { sumRules, countGaps, renderRulebookMarkdown, renderGapsMarkdown } from './md.mjs';
import { fileURLToPath } from 'node:url';

const SERVER_NAME = 'rtl2spec';
const SERVER_VERSION = '0.3.0';
const PROTOCOL_LATEST = '2025-11-25';
const PROTOCOL_SUPPORTED = [
  '2025-11-25',
  '2025-06-18',
  '2025-03-26',
  '2024-11-05',
  '2024-10-07',
];
const MAX_BATCH_PATHS = 64;

// ---- wasm 裝載(顯式位元組;§5 策略) ----

function fatal(msg) {
  process.stderr.write(`rtl2spec: ${msg}\n`);
  process.exit(1);
}

const wasmDir =
  process.env.RTL2SPEC_WASM_DIR && existsSync(process.env.RTL2SPEC_WASM_DIR)
    ? process.env.RTL2SPEC_WASM_DIR
    : join(dirname(fileURLToPath(import.meta.url)), 'wasm');

const gluePath = join(wasmDir, 'rtl_spec_wasm.js');
const wasmPath = join(wasmDir, 'rtl_spec_wasm_bg.wasm');
if (!existsSync(gluePath) || !existsSync(wasmPath)) {
  fatal(`wasm artifacts missing under ${wasmDir} (run-mcp-e2e.sh 同步步負責刷新)`);
}

let api;
try {
  const glue = await import(gluePath);
  await glue.default({ module_or_path: new Uint8Array(readFileSync(wasmPath)) });
  api = glue;
} catch (e) {
  fatal(`wasm init failed: ${e?.message ?? e}`);
}

// ---- ABI 兼容裝載校驗(§17.5;內核 ABI 軌=rtl_spec_version(Cargo workspace),
// 與插件 semver(SERVER_VERSION)雙軌;不匹配=清晰錯誤含雙版本+修復指引) ----
const EXPECTED_ABI = ['0.1.0'];
try {
  const abiVersion = api.rtl_spec_version();
  if (!EXPECTED_ABI.includes(abiVersion)) {
    fatal(
      `wasm ABI 版本不兼容: 內核=${abiVersion} 期望=${EXPECTED_ABI.join('/')}\n` +
      `修復: wasm-pack build crates/rtl-spec-wasm --target web --out-dir pkg 後同步 dist/wasm/` +
      `(run-mcp-e2e.sh / run-rtl-validation.sh 均自動同步;EXPECTED_ABI 名單=dist/server.mjs 頂部常量)`
    );
  }
} catch (e) {
  fatal(`ABI 校驗失敗(rtl_spec_version 不可用): ${e?.message ?? e}`);
}

// ---- 語義入口(JSON 進出;parse 錯誤=結構化欄位,非工具級錯誤) ----

function langOf(args, fallbackName) {
  if (args.lang === 'sv' || args.lang === 'rtlil') return args.lang;
  const ext = extname(fallbackName).toLowerCase();
  if (ext === '.il') return 'rtlil';
  return 'sv'; // .v/.sv/.vh 及其餘 → sv
}

function loadSource(args) {
  // 至少一:皆給以 path 為準(R1-11);皆缺=工具級錯誤。
  if (args.path !== undefined && args.path !== null && args.path !== '') {
    const p = String(args.path);
    if (!existsSync(p)) return { error: `file not found: ${p}` };
    try {
      return { name: p, source: readFileSync(p, 'utf8') };
    } catch (e) {
      return { error: `cannot read ${p}: ${e?.message ?? e}` };
    }
  }
  if (typeof args.source === 'string') return { name: args.name ?? 'input.v', source: args.source };
  return { error: 'either `path` or `source` is required' };
}

// ---- 工具面(§6.1 四工具) ----

const TOOLS = [
  {
    name: 'rtl_analyze',
    description:
      'Structure projection report of an RTL source (Verilog/SV or RTLIL): modules (params/ports/nets/cells), instance edges, diagnostics count. First stop for RTL analysis.',
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'RTL source file path (relative to cwd)' },
        source: { type: 'string', description: 'RTL source text (used when path is absent; path wins when both given)' },
        name: { type: 'string', description: 'virtual file name for inline `source` (default input.v)' },
        lang: { type: 'string', enum: ['sv', 'rtlil'], description: 'default: by extension (.il → rtlil, else sv)' },
      },
      required: [],
    },
    call(args) {
      const loaded = loadSource(args);
      if (loaded.error) return toolError(loaded.error);
      const lang = langOf(args, loaded.name);
      const json =
        lang === 'rtlil'
          ? api.rtl_analyze_rtlil(loaded.name, loaded.source)
          : api.rtl_analyze_sv(loaded.name, loaded.source);
      return ok(JSON.parse(json), null);
    },
  },
  {
    name: 'rtl_extract_rules',
    description:
      'Extract a behavioural rulebook from RTL (reset/fsm/timing/arithmetic/counter/... categories; anti-doc-dump three locks). Not a mechanical doc conversion: every rule carries byte-checkable source evidence and a machine-checkable form.',
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'RTL source file path (relative to cwd)' },
        source: { type: 'string', description: 'RTL source text (used when path is absent)' },
        name: { type: 'string', description: 'virtual file name for inline `source`' },
        lang: { type: 'string', enum: ['sv', 'rtlil'] },
        format: { type: 'string', enum: ['json', 'markdown', 'both'], description: 'default both' },
        max_rules_per_module: { type: 'number', description: 'default 128; overflow marks truncated' },
      },
      required: [],
    },
    call(args) {
      const loaded = loadSource(args);
      if (loaded.error) return toolError(loaded.error);
      const lang = langOf(args, loaded.name);
      const format = ['json', 'markdown', 'both'].includes(args.format) ? args.format : 'both';
      const max = Number.isFinite(args.max_rules_per_module) && args.max_rules_per_module > 0
        ? Math.floor(args.max_rules_per_module)
        : 128;
      const json =
        lang === 'rtlil'
          ? api.rtl_extract_rules_rtlil(loaded.name, loaded.source)
          : api.rtl_extract_rules_sv(loaded.name, loaded.source, max);
      const rulebook = JSON.parse(json);
      const markdown =
        format === 'json' ? null : renderRulebookMarkdown(rulebook);
      return ok({ rulebook, markdown }, format === 'markdown' ? markdown : null);
    },
  },
  {
    name: 'rtl_scan_gaps',
    description:
      'Scan RTL for test gaps (latch inference / incomplete case / undriven read / missing reset / comb loop / multi-driver / blocking-in-seq). Each gap carries a suggested_test — the report IS the test-gap checklist.',
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'RTL source file path (relative to cwd)' },
        source: { type: 'string', description: 'RTL source text (used when path is absent)' },
        name: { type: 'string', description: 'virtual file name for inline `source`' },
        lang: { type: 'string', enum: ['sv', 'rtlil'] },
      },
      required: [],
    },
    call(args) {
      const loaded = loadSource(args);
      if (loaded.error) return toolError(loaded.error);
      const lang = langOf(args, loaded.name);
      const json =
        lang === 'rtlil'
          ? api.rtl_scan_gaps_rtlil(loaded.name, loaded.source)
          : api.rtl_scan_gaps_sv(loaded.name, loaded.source);
      const report = JSON.parse(json);
      const testPlan = (report.items ?? []).map((g) => g.suggested_test);
      return ok({ gaps: report.items ?? [], test_plan: testPlan }, renderGapsMarkdown(report));
    },
  },
  {
    name: 'rtl_extract_batch',
    description:
      'Batch rulebook+gap extraction over up to 64 RTL files; returns per-file ok/rule_count/gap_count summary (validation harness)',
    inputSchema: {
      type: 'object',
      properties: {
        paths: { type: 'array', items: { type: 'string' }, maxItems: MAX_BATCH_PATHS },
      },
      required: ['paths'],
    },
    call(args) {
      const paths = args.paths;
      if (!Array.isArray(paths)) {
        return toolError('`paths` must be an array of strings');
      }
      if (paths.length > MAX_BATCH_PATHS) {
        return toolError(`too many paths: ${paths.length} > ${MAX_BATCH_PATHS}`);
      }
      const results = [];
      for (const p of paths) {
        const loaded = loadSource({ path: p });
        if (loaded.error) {
          results.push({ path: p, ok: false, rule_count: 0, gap_count: 0, error: loaded.error });
          continue;
        }
        const lang = langOf({}, loaded.name);
        try {
          const rbJson =
            lang === 'rtlil'
              ? api.rtl_extract_rules_rtlil(loaded.name, loaded.source)
              : api.rtl_extract_rules_sv(loaded.name, loaded.source, 128);
          const gapJson =
            lang === 'rtlil'
              ? api.rtl_scan_gaps_rtlil(loaded.name, loaded.source)
              : api.rtl_scan_gaps_sv(loaded.name, loaded.source);
          const rb = JSON.parse(rbJson);
          const gr = JSON.parse(gapJson);
          results.push({
            path: p,
            ok: true,
            rule_count: sumRules(rb),
            gap_count: countGaps(gr),
          });
        } catch (e) {
          results.push({
            path: p,
            ok: false,
            rule_count: 0,
            gap_count: 0,
            error: `extraction failed: ${e?.message ?? e}`,
          });
        }
      }
      return ok({ results }, null);
    },
  },
];

function ok(structuredContent, textOverride) {
  const text =
    textOverride !== null && textOverride !== undefined
      ? textOverride
      : JSON.stringify(structuredContent);
  return { content: [{ type: 'text', text }], structuredContent, isError: false };
}

function toolError(message) {
  return { content: [{ type: 'text', text: message }], isError: true };
}

// ---- JSON-RPC 幀層(newline 分幀) ----

function writeFrame(obj) {
  process.stdout.write(JSON.stringify(obj) + '\n');
}

function reply(id, result) {
  writeFrame({ jsonrpc: '2.0', id, result });
}

function replyError(id, code, message) {
  writeFrame({ jsonrpc: '2.0', id, error: { code, message } });
}

function dispatch(msg) {
  const { id, method, params } = msg;
  if (method === 'initialize') {
    const requested = params?.protocolVersion;
    const version =
      typeof requested === 'string' && PROTOCOL_SUPPORTED.includes(requested)
        ? requested
        : PROTOCOL_LATEST;
    reply(id, {
      protocolVersion: version,
      capabilities: { tools: {} },
      serverInfo: { name: SERVER_NAME, version: SERVER_VERSION },
    });
    return;
  }
  if (method === 'notifications/initialized' || method === 'initialized') {
    return; // 通知:吞
  }
  if (method === 'ping') {
    reply(id, {});
    return;
  }
  if (method === 'tools/list') {
    reply(id, {
      tools: TOOLS.map((t) => ({
        name: t.name,
        description: t.description,
        inputSchema: t.inputSchema,
      })),
    });
    return;
  }
  if (method === 'tools/call') {
    const name = params?.name;
    const args = params?.arguments ?? {};
    const tool = TOOLS.find((t) => t.name === name);
    if (!tool) {
      replyError(id, -32601, `tool not found: ${name}`);
      return;
    }
    if (name === 'rtl_extract_batch' && !Array.isArray(args.paths)) {
      replyError(id, -32602, 'invalid params: `paths` (array) is required');
      return;
    }
    try {
      reply(id, tool.call(args));
    } catch (e) {
      // 導出面契約=JSON 進出,不拋;此層是防禦線。
      reply(id, toolError(`internal error: ${e?.message ?? e}`));
    }
    return;
  }
  if (id === undefined) {
    process.stderr.write(`rtl2spec: unknown notification ${method}\n`);
    return;
  }
  replyError(id, -32601, `method not found: ${method}`);
}

const rl = createInterface({ input: process.stdin, terminal: false });
rl.on('line', (line) => {
  const text = line.trim();
  if (!text) return;
  let msg;
  try {
    msg = JSON.parse(text);
  } catch (e) {
    process.stderr.write(`rtl2spec: bad frame: ${e?.message ?? e}\n`);
    return;
  }
  try {
    dispatch(msg);
  } catch (e) {
    if (msg?.id !== undefined) replyError(msg.id, -32603, `internal error: ${e?.message ?? e}`);
    process.stderr.write(`rtl2spec: dispatch failed: ${e?.message ?? e}\n`);
  }
});
rl.on('close', () => {
  process.exit(0);
});
