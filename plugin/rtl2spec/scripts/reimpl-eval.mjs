// reimpl-eval.mjs — 復原對拍評測執行器(S5;§5.2 S5(b)(d);直調不進 regress)。
// 兩模式:
//   --trim <design>:從當前提取器 rules.json 生成裁剪 evidence 的規則書 md
//     (僅陳述+check 渲染行+接口契約——B 組輸入;確定性,寫只落 build/reimpl-eval/)。
//   默認(對拍):23 場景逐腿 iverilog 編譯+運行+逐場景 diff(對照=原版 corpus
//     DUT 同 TB 輸出);verdict JSON+報告 md 寫 build/reimpl-eval/;按組分列
//     (A/B 禁混報)+消融點按切片分段(S1/S2/S3+抄寫通道)。
// 盲寫腿=輔助證據,永不進確定性閘;機械面=編譯成敗+TB 塊 diff(REIMPL-PROTOCOL.md §2)。
// 用法: bun scripts/reimpl-eval.mjs [--trim <counter8|acc_pipe|i2c_master>]
import { readFileSync, writeFileSync, mkdirSync, existsSync, rmSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

const pluginRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const repoRoot = dirname(dirname(pluginRoot));
const outDir = join(pluginRoot, 'build', 'reimpl-eval');
const F = (p) => join(pluginRoot, 'fixtures', p);

const argv = process.argv.slice(2);
const trimIdx = argv.indexOf('--trim');
if (trimIdx >= 0) {
  const design = argv[trimIdx + 1];
  const SRC = {
    counter8: 'corpus/counter8/counter.v',
    acc_pipe: 'corpus/e2e/58_xls_acc_pipe/acc_pipe.v',
    i2c_master: 'corpus/e2e/59_i2c_master/i2c_master.v',
  };
  if (!SRC[design]) {
    console.error(`usage: --trim <counter8|acc_pipe|i2c_master> — 未知 ${design}`);
    process.exit(2);
  }
  const wasmDir = join(pluginRoot, 'dist', 'wasm');
  const glue = await import(join(wasmDir, 'rtl_spec_wasm.js'));
  await glue.default({ module_or_path: new Uint8Array(readFileSync(join(wasmDir, 'rtl_spec_wasm_bg.wasm'))).slice() });
  const srcPath = join(repoRoot, SRC[design]);
  const rb = JSON.parse(glue.rtl_extract_rules_sv(srcPath, readFileSync(srcPath, 'utf8'), 256));
  // 裁剪:rules 去 evidence 塊(僅陳述+check 渲染);接口契約/lint/契約頭保留
  // (契約章=E1 失敗點 N2 的修復面,屬規則文本非證據塊——協議 §1 B 組定義)。
  const lines = [`# 規則書(裁剪 evidence 版):${rb.source}`, '', '- 模組數:' + rb.modules.length, ''];
  for (const m of rb.modules) {
    const c = m.contract;
    lines.push(`## 模組 ${m.module}(${m.rules.length} 條)`, '');
    if (c) {
      if (c.params?.length) lines.push('- 參數: ' + c.params.map((p) => `${p.name}=${p.value}${p.local ? ' (local)' : ''}`).join('; '));
      if (c.ports?.length) lines.push('- 端口: ' + c.ports.map((p) => `${p.name}:${p.dir}${p.parametric ? '[parametric]' : '[' + p.width + ']'}${p.signed ? ' signed' : ''}`).join('; '));
      if (c.signals?.length) lines.push('- 內部聲明: ' + c.signals.map((g) => `${g.name}:${g.kind}[${g.width}]${g.unpacked_dims > 0 ? 'x' + g.unpacked_dims : ''}`).join('; '));
    }
    for (const r of m.rules) {
      const c2 = r.check;
      const detail =
        c2.form === 'guard' ? `trigger=${c2.trigger?.signal ?? '-'} ${c2.trigger?.edge ?? '-'}; when=${c2.when ?? '-'}; async=${c2.async}; effects=${(c2.effects ?? []).map((e) => `${e.lhs}←${e.value}`).join(';')}`
        : c2.form === 'transition' ? `${c2.state_reg}: ${c2.from} → ${c2.to} when ${c2.when ?? '-'}`
        : c2.form === 'sequence' ? `+${c2.latency}拍 → ${c2.signal}=${c2.becomes} when ${c2.when ?? '-'}`
        : c2.form === 'relation' ? `${c2.lhs} ≡ ${c2.rhs} when ${c2.when ?? '-'}`
        : `${c2.subject} ${c2.kind}`;
      lines.push(`- ${r.id}[${r.category}] 陳述:${r.statement}`);
      lines.push(`  check(${c2.form}): ${detail}`);
    }
    lines.push('');
  }
  mkdirSync(outDir, { recursive: true });
  const dst = join(outDir, `rulebook-trimmed-${design}.md`);
  writeFileSync(dst, lines.join('\n') + '\n');
  console.log(`== trim: ${design} → ${dst}(${lines.length} 行) ==`);
  process.exit(0);
}

// ---------- 對拍模式 ----------
const clock = (label, fn) => {
  const t0 = Date.now();
  const r = fn();
  return { label, ms: Date.now() - t0, ...r };
};
const sh = (cmd, args, cwd) => execFileSync(cmd, args, { cwd, encoding: 'utf8' });
const trySh = (cmd, args, cwd) => {
  try {
    return { code: 0, out: sh(cmd, args, cwd) };
  } catch (e) {
    return { code: e.status ?? 1, out: e.stdout ?? '', err: e.stderr ?? String(e.message).slice(0, 200) };
  }
};

// 逐場景切塊:TB 輸出協議=`SCENE<n> <name>` 前缀行+塊內行;塊 diff=任一行差異即失配。
const scenes = (txt) => {
  const blocks = new Map();
  let cur = null;
  for (const l of txt.split('\n')) {
    const m = l.match(/^SCENE(\d+)\b/);
    if (m) {
      cur = `SCENE${m[1]}`;
      blocks.set(cur, []);
    } else if (cur && l.trim() !== '') {
      blocks.get(cur).push(l.trimEnd());
    }
  }
  return blocks;
};

const LEGS = [
  { leg: 'counter8-A1', design: 'counter8', group: 'A', n: 1, tb: 'e1/tb_counter8.v', origDut: ['corpus/counter8/counter.v'], reimplDut: ['e1-reimpl/counter8-a1/top.v'], scenes: 6, point: 'control' },
  { leg: 'i2c-A0', design: 'i2c_master', group: 'A', n: 1, tb: 'e1/tb_i2c.v', origDut: ['corpus/e2e/59_i2c_master/i2c_master.v'], reimplDut: ['e1-reimpl/i2c-a0/top.v'], wrapper: 'e1/dut_i2c', wrapperO: 'e1/dut_orig_i2c.v', wrapperR: 'e1/dut_reimpl_i2c.v', scenes: 11, point: 'S1-before' },
  { leg: 'acc-A0', design: 'acc_pipe', group: 'A', n: 1, tb: 'e1/tb_acc_pipe.v', origDut: ['corpus/e2e/58_xls_acc_pipe/acc_pipe.v'], reimplDut: ['e1-reimpl/acc-a0/top.v'], wrapperO: 'e1/dut_orig_accpipe.v', wrapperR: 'e1/dut_reimpl_accpipe.v', scenes: 6, point: 'S2-before' },
  { leg: 'i2c-A0f', design: 'i2c_master', group: 'A', n: 1, tb: 'e1/tb_i2c.v', origDut: ['corpus/e2e/59_i2c_master/i2c_master.v'], reimplDut: ['e1-reimpl/i2c-a0/top_fixed.v'], wrapperO: 'e1/dut_orig_i2c.v', wrapperR: 'e1/dut_reimpl_i2c.v', scenes: 11, point: 'S3-before' },
  { leg: 'i2c-A1', design: 'i2c_master', group: 'A', n: 1, tb: 'e1/tb_i2c.v', origDut: ['corpus/e2e/59_i2c_master/i2c_master.v'], reimplDut: ['reimpl/i2c-A1/top.v'], wrapperO: 'e1/dut_orig_i2c.v', scenes: 11, point: 'S1+S3' },
  { leg: 'i2c-A2', design: 'i2c_master', group: 'A', n: 2, tb: 'e1/tb_i2c.v', origDut: ['corpus/e2e/59_i2c_master/i2c_master.v'], reimplDut: ['reimpl/i2c-A2/top.v'], wrapperO: 'e1/dut_orig_i2c.v', scenes: 11, point: 'S1+S3' },
  { leg: 'i2c-A3', design: 'i2c_master', group: 'A', n: 3, tb: 'e1/tb_i2c.v', origDut: ['corpus/e2e/59_i2c_master/i2c_master.v'], reimplDut: ['reimpl/i2c-A3/top.v'], wrapperO: 'e1/dut_orig_i2c.v', scenes: 11, point: 'S1+S3' },
  { leg: 'acc-A1', design: 'acc_pipe', group: 'A', n: 1, tb: 'e1/tb_acc_pipe.v', origDut: ['corpus/e2e/58_xls_acc_pipe/acc_pipe.v'], reimplDut: ['reimpl/acc-A1/top.v'], wrapperO: 'e1/dut_orig_accpipe.v', scenes: 6, point: 'S2' },
  { leg: 'acc-A2', design: 'acc_pipe', group: 'A', n: 2, tb: 'e1/tb_acc_pipe.v', origDut: ['corpus/e2e/58_xls_acc_pipe/acc_pipe.v'], reimplDut: ['reimpl/acc-A2/top.v'], wrapperO: 'e1/dut_orig_accpipe.v', scenes: 6, point: 'S2' },
  { leg: 'acc-A3', design: 'acc_pipe', group: 'A', n: 3, tb: 'e1/tb_acc_pipe.v', origDut: ['corpus/e2e/58_xls_acc_pipe/acc_pipe.v'], reimplDut: ['reimpl/acc-A3/top.v'], wrapperO: 'e1/dut_orig_accpipe.v', scenes: 6, point: 'S2' },
  { leg: 'i2c-B1', design: 'i2c_master', group: 'B', n: 1, tb: 'e1/tb_i2c.v', origDut: ['corpus/e2e/59_i2c_master/i2c_master.v'], reimplDut: ['reimpl/i2c-B1/top.v'], wrapperO: 'e1/dut_orig_i2c.v', scenes: 11, point: 'copy-channel' },
  { leg: 'acc-B1', design: 'acc_pipe', group: 'B', n: 1, tb: 'e1/tb_acc_pipe.v', origDut: ['corpus/e2e/58_xls_acc_pipe/acc_pipe.v'], reimplDut: ['reimpl/acc-B1/top.v'], wrapperO: 'e1/dut_orig_accpipe.v', scenes: 6, point: 'copy-channel' },
];

mkdirSync(outDir, { recursive: true });
const runDir = join(outDir, 'runs');
rmSync(runDir, { recursive: true, force: true });
mkdirSync(runDir, { recursive: true });

// 原版輸出(每設計一份,TB+wrapper+corpus DUT)。wrapper 選擇契約:E1 舊產物
// 腿(A0)接專用適配 wrapper(接口缺口補齊);S5 新腿按接口契約對齊原版簽名,
// 一律接原版直連 wrapper(wrapperO=雙方共用——接口對齊本身即 S1 契約章的對拍面)。
const origOut = {};
const origFail = [];
for (const design of ['counter8', 'acc_pipe', 'i2c_master']) {
  const leg = LEGS.find((l) => l.design === design);
  const args = ['-g2001', '-o', join(runDir, `sim_${design}_orig`), F(leg.tb)];
  if (leg.wrapperO) args.push(F(leg.wrapperO));
  for (const d of leg.origDut) args.push(join(repoRoot, d));
  const c = trySh('iverilog', args, repoRoot);
  if (c.code !== 0) { origFail.push(`${design}: iverilog ${c.code} ${c.err?.slice(0, 120)}`); continue; }
  const r = trySh(join(runDir, `sim_${design}_orig`), [], runDir);
  origOut[design] = r.out;
  writeFileSync(join(runDir, `out_${design}_orig.txt`), r.out);
}
if (origFail.length > 0) {
  console.error(`FAIL 原版腿編譯/運行: ${origFail.join('; ')}`);
  process.exit(1);
}

// 逐腿對拍。
const verdict = { tool: 'iverilog', legs: [], generatedAt: new Date().toISOString() };
for (const leg of LEGS) {
  const files = leg.reimplDut.map((d) => (d.startsWith('e1-reimpl/') || d.startsWith('reimpl/')) ? (d.startsWith('e1-reimpl/') ? F(d) : join(outDir, d)) : join(repoRoot, d));
  const missing = files.filter((f) => !existsSync(f));
  if (missing.length > 0) {
    verdict.legs.push({ leg: leg.leg, design: leg.design, group: leg.group, n: leg.n, point: leg.point, present: false, reason: `盲寫產物缺: ${missing.join(',')}` });
    console.log(`SKIP ${leg.leg}(產物缺)`);
    continue;
  }
  const args = ['-g2001', '-o', join(runDir, `sim_${leg.leg}`), F(leg.tb)];
  const wrap = leg.wrapperR ?? leg.wrapperO; // 新腿=契約對齊接口,雙方共用原版直連
  if (wrap) args.push(F(wrap));
  for (const f of files) args.push(f);
  const comp = trySh('iverilog', args, repoRoot);
  const compiled = comp.code === 0;
  const res = { leg: leg.leg, design: leg.design, group: leg.group, n: leg.n, point: leg.point, present: true, compiled, scenesTotal: leg.scenes, scenesMatched: null, mismatched: null, warn: compiled ? (comp.err ?? '').split('\n').filter((l) => l.includes('warning:')).slice(0, 4) : (comp.err ?? '').split('\n').slice(0, 4) };
  if (compiled) {
    const run = trySh(join(runDir, `sim_${leg.leg}`), [], runDir);
    writeFileSync(join(runDir, `out_${leg.leg}.txt`), run.out);
    const a = scenes(origOut[leg.design]);
    const b = scenes(run.out);
    const mismatched = [];
    for (const k of a.keys()) {
      if (JSON.stringify(a.get(k)) !== JSON.stringify(b.get(k) ?? null)) mismatched.push(k);
    }
    res.scenesMatched = leg.scenes - mismatched.length;
    res.mismatched = mismatched;
  }
  verdict.legs.push(res);
  console.log(`${compiled ? 'RUN ' : 'COMP-FAIL'} ${leg.leg}${compiled ? ` matched=${res.scenesMatched}/${leg.scenes}${(res.mismatched ?? []).length ? ' 失配:' + res.mismatched.join(',') : ''}` : ' ' + (comp.err ?? '').split('\n')[0]?.slice(0, 80)}`);
}

// 分段輸出(按組分列禁混報;消融點按切片)。
const by = (pred) => verdict.legs.filter(pred);
const matchedOf = (l) => (l.compiled ? `${l.scenesMatched}/${l.scenesTotal}` : 'compiled=false');
const segments = {
  S1_point: {
    desc: 'i2c compiled(A0=pre-S1 對照 vs A1..A3=post-contract;判準 n≥3 中 ≥2 compiled=true)',
    before: by((l) => l.leg === 'i2c-A0').map((l) => matchedOf(l)),
    after: by((l) => l.leg.startsWith('i2c-A') && l.point === 'S1+S3').map((l) => matchedOf(l)),
  },
  S2_point: {
    desc: 'acc_pipe matched(A0=pre-S2 對照 vs A1..A3=post-combo-rules)',
    before: by((l) => l.leg === 'acc-A0').map((l) => matchedOf(l)),
    after: by((l) => l.leg.startsWith('acc-A') && l.point === 'S2').map((l) => matchedOf(l)),
  },
  S3_point: {
    desc: 'i2c SCENE10(A0f=top_fixed 最小修復=pre-S3 對照 vs A1..A3 post-async;判準 ≥2/3 得 11/11)',
    before: by((l) => l.leg === 'i2c-A0f').map((l) => (!l.compiled ? 'compiled=false' : (l.mismatched ?? []).includes('SCENE10') ? 'SCENE10 失配(' + l.scenesMatched + '/' + l.scenesTotal + ')' : l.scenesMatched + '/' + l.scenesTotal)),
    after: by((l) => l.leg.startsWith('i2c-A') && l.point === 'S1+S3').map((l) => (l.compiled ? ((l.mismatched ?? []).includes('SCENE10') ? 'SCENE10 失配(' + l.scenesMatched + '/' + l.scenesTotal + ')' : l.scenesMatched + '/' + l.scenesTotal) : 'compiled=false')),
  },
  copy_channel: {
    desc: '抄寫通道(A 組 vs B 組 matched 差;按組分列禁混報)',
    groupA: by((l) => l.group === 'A' && (l.design === 'acc_pipe' || l.design === 'i2c_master')).map((l) => `${l.leg}=${matchedOf(l)}`),
    groupB: by((l) => l.group === 'B').map((l) => `${l.leg}=${matchedOf(l)}`),
  },
};
verdict.segments = segments;

const vPath = join(outDir, 'verdict.json');
writeFileSync(vPath, JSON.stringify(verdict, null, 2));
console.log(`== verdict: ${vPath} ==`);
console.log(JSON.stringify(segments, null, 2));
const missingLegs = verdict.legs.filter((l) => !l.present).length;
process.exit(missingLegs > 0 ? 3 : 0);
