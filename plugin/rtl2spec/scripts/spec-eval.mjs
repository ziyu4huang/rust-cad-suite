// spec-eval.mjs — spec 前端評測床執行器(D-S4;RTL2SPEC-D-PLAN §4 D-S4/§5.2;直調不進 regress)。
// 形沿 reimpl-eval(§1 基線):iverilog -g2001 編譯(tb+wrapperO+DUT)+tbproto.sceneBlocks
// 逐場景塊 diff(語義逐字移植);腿表=fixtures/d-spec/legs.json(斷言規格,手寫合法)。
// 接線契約(R1-2):DUT 檔含 `module <spec.module>`=啟動斷言(spec.module 由 wasm
// rtl_spec_spec_parse 從 spec 檔取=SSOT);不過=exit 1(錯誤帶 spec.module 與 wrapperO
// 路徑=憲法①下一步提示)。
// 守衛:iverilog 缺場=exit 2;腿產物缺=exit 3。寫只落 build/spec-eval/
// (普通運行;--record --slice <D-Sn> 才寫已提交證據頁 spec-eval-report.md——頁內零牆鐘,
// R2-1;保鮮形 D-S6 收編)。mutprobe=每設計一條確定性變異(anchor count==1 運行期斷言),
// 必紅(matched<expected 或 compiled=false)=TB 判別力自證(10.12);measured 實測對拍
// legs.json 錄入數字,漂移=紅。
// 用法: bun scripts/spec-eval.mjs [--record --slice <D-Sn>]
import { readFileSync, writeFileSync, mkdirSync, existsSync, rmSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { run as runWatched, sceneBlocks, parseProtocol } from './lib/tbproto.mjs';

const pluginRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const repoRoot = dirname(dirname(pluginRoot));
const outDir = join(pluginRoot, 'build', 'spec-eval');
const recordMode = process.argv.includes('--record');
const sliceIdx = process.argv.indexOf('--slice');
const slice = sliceIdx >= 0 ? process.argv[sliceIdx + 1] : null;
if (recordMode && !slice) {
  console.error('FAIL usage: --record 必帶 --slice <D-Sn>(錄前快照對賬的切片標籤)');
  process.exit(2);
}

const legsSpec = JSON.parse(readFileSync(join(repoRoot, 'plugin/rtl2spec/fixtures/d-spec/legs.json'), 'utf8'));
const R = (p) => join(repoRoot, p);

// 守衛:iverilog 缺場=exit 2(降級);腿表/DUT/TB/wrapper/spec 缺=exit 3(產物缺)。
const ivProbe = runWatched('iverilog', ['-V'], { cwd: repoRoot, timeoutMs: 10000 });
if (ivProbe.code !== 0 || ivProbe.spawnError) {
  console.log('SKIP guard: iverilog not in PATH');
  process.exit(2);
}
const iverilogVersion = (ivProbe.stdout || '').split('\n')[0] || 'iverilog';
const missing = [];
for (const leg of legsSpec.legs) {
  for (const p of [leg.spec, leg.tb, ...(leg.wrapperO ? [leg.wrapperO] : []), ...leg.origDut, ...leg.reimplDut]) {
    if (!existsSync(R(p))) missing.push(p);
  }
}
for (const d of legsSpec.designs ?? []) {} // (保留形;設計清單由 legs 推導)
if (missing.length > 0) {
  console.log(`FAIL guard: 腿產物缺: ${missing.join(', ')}`);
  process.exit(3);
}

// wasm 裝載(顯式位元組 init,golden-check 同形):spec.module SSOT 解析面。
const wasmDir = join(pluginRoot, 'dist', 'wasm');
const glue = await import(join(wasmDir, 'rtl_spec_wasm.js'));
await glue.default({ module_or_path: new Uint8Array(readFileSync(join(wasmDir, 'rtl_spec_wasm_bg.wasm'))) });
const wasmSha = (await import('node:crypto')).createHash('sha256')
  .update(readFileSync(join(wasmDir, 'rtl_spec_wasm_bg.wasm'))).digest('hex');

// 啟動接線斷言(R1-2):每腿 DUT 檔含 `module <spec.module>`;不過=exit 1(帶下一步提示)。
const specModule = {};
{
  const fails = [];
  for (const leg of legsSpec.legs) {
    const src = readFileSync(R(leg.spec), 'utf8');
    const doc = JSON.parse(glue.rtl_spec_spec_parse(leg.spec, src));
    const mod = doc.module;
    if (!mod) {
      fails.push(`${leg.leg}: spec 檔無 module 行(${leg.spec})`);
      continue;
    }
    specModule[leg.design] = mod;
    for (const d of [...leg.origDut, ...leg.reimplDut]) {
      const re = new RegExp(`\\bmodule\\s+${mod.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\b`);
      if (!re.test(readFileSync(R(d), 'utf8'))) {
        fails.push(
          `${leg.leg}: DUT 檔 ${d} 不含 \`module ${mod}\`(spec.module=${mod};wrapperO=${leg.wrapperO ?? '無'})` +
            ` — 核對盲寫頂模組名或 wrapper 接線(run: bash plugin/rtl2spec/run-spec-eval.sh)`,
        );
      }
    }
  }
  if (fails.length > 0) {
    console.log('FAIL 接線斷言:');
    for (const f of fails) console.log(`  - ${f}`);
    process.exit(1);
  }
  console.log(`PASS 接線斷言(spec.module=腿表逐腿對拍: ${[...new Set(Object.values(specModule))].join(', ')})`);
}

const SIM_TIMEOUT_MS = 30000;
const trySh = (cmd, args, cwd) => {
  const r = runWatched(cmd, args, { cwd, timeoutMs: SIM_TIMEOUT_MS });
  if (r.code === 0 && !r.timedOut) return { code: 0, out: r.stdout };
  return {
    code: r.timedOut ? 124 : r.code,
    out: r.stdout ?? '',
    err: r.timedOut ? `TIMEOUT ${SIM_TIMEOUT_MS}ms` : r.spawnError ? String(r.spawnError).slice(0, 200) : r.stderr,
  };
};

mkdirSync(outDir, { recursive: true });
const runDir = join(outDir, 'runs');
rmSync(runDir, { recursive: true, force: true });
mkdirSync(runDir, { recursive: true });
const mutantDir = join(outDir, 'mutants');
rmSync(mutantDir, { recursive: true, force: true });
mkdirSync(mutantDir, { recursive: true });

// 路徑契約:tb/wrapper/duts 一律 push 原值(相對=repo 根相對,cwd=repoRoot;
// 絕對=mutant 產物),禁二次 R()——R(R(rel)) 雙前綴實錄(D-S4 首跑)。
const compileArgs = (outName, tb, wrapper, duts) => {
  const args = ['-g2001', '-o', join(runDir, outName), R(tb)];
  if (wrapper) args.push(R(wrapper));
  for (const d of duts) args.push(d);
  return args;
};

// 原版輸出(每設計一份)。
const designs = [...new Set(legsSpec.legs.map((l) => l.design))];
const origOut = {};
const origFail = [];
for (const design of designs) {
  const leg = legsSpec.legs.find((l) => l.design === design);
  const c = trySh('iverilog', compileArgs(`sim_${design}_orig`, leg.tb, leg.wrapperO, leg.origDut), repoRoot);
  if (c.code !== 0) {
    origFail.push(`${design}: iverilog ${c.code} ${c.err?.slice(0, 120)}`);
    continue;
  }
  const r = trySh(join(runDir, `sim_${design}_orig`), [], runDir);
  origOut[design] = r.out;
  writeFileSync(join(runDir, `out_${design}_orig.txt`), r.out);
}
if (origFail.length > 0) {
  console.log(`FAIL 原版腿編譯/運行: ${origFail.join('; ')}`);
  process.exit(1);
}

const diffVs = (design, out) => {
  const a = sceneBlocks(origOut[design]);
  const b = sceneBlocks(out);
  const mismatched = [];
  for (const k of a.keys()) {
    if (JSON.stringify(a.get(k)) !== JSON.stringify(b.get(k) ?? null)) mismatched.push(k);
  }
  return mismatched;
};

// 逐腿對拍+斷言(對 legs.json expected 逐字;任何不符=紅)。
const failures = [];
const legs = [];
for (const leg of legsSpec.legs) {
  const files = leg.reimplDut;
  const absent = files.filter((d) => !existsSync(R(d)));
  if (absent.length > 0) {
    // 腿產物缺=exit 3(D-S5 盲寫腿未凍結時的形)。
    console.log(`FAIL guard: 腿 ${leg.leg} 產物缺: ${absent.join(', ')}(凍結入庫=D-S5 --record)`);
    process.exit(3);
  }
  const comp = trySh('iverilog', compileArgs(`sim_${leg.leg}`, leg.tb, leg.wrapperO, files), repoRoot);
  const compiled = comp.code === 0;
  const res = { leg: leg.leg, design: leg.design, compiled, scenesTotal: leg.scenes, scenesMatched: null, mismatched: null };
  if (compiled) {
    const run = trySh(join(runDir, `sim_${leg.leg}`), [], runDir);
    writeFileSync(join(runDir, `out_${leg.leg}.txt`), run.out);
    const parsed = parseProtocol(run.out).scenes.length;
    res.scenesParsed = parsed;
    if (parsed !== leg.scenes) {
      failures.push(`${leg.leg}: 場景協議數 ${parsed} != 腿表 ${leg.scenes}(TB 輸出協議漂移)`);
    }
    res.mismatched = diffVs(leg.design, run.out);
    res.scenesMatched = leg.scenes - res.mismatched.length;
  }
  if (compiled !== leg.expected.compiled) {
    failures.push(`${leg.leg}: compiled=${compiled} != expected.compiled=${leg.expected.compiled}`);
  } else if (compiled && res.scenesMatched !== leg.expected.matched) {
    failures.push(`${leg.leg}: matched=${res.scenesMatched}/${leg.scenes} != expected.matched=${leg.expected.matched}(失配:${(res.mismatched ?? []).join(',') || '無'})`);
  }
  legs.push(res);
  console.log(`${compiled ? 'RUN ' : 'COMP-FAIL'} ${leg.leg} matched=${compiled ? `${res.scenesMatched}/${leg.scenes}` : '-'} expected=${leg.expected.compiled ? leg.expected.matched : 'compile-fail'}${compiled ? '' : ' ' + (comp.err ?? '').split('\n')[0]?.slice(0, 120)}`);
}

// mutprobe(負向探針=TB 判別力自證,10.12):每設計一條確定性變異,必紅。
const probes = [];
for (const mp of legsSpec.mutprobe ?? []) {
  const leg = legsSpec.legs.find((l) => l.design === mp.design);
  if (!leg) {
    failures.push(`mutprobe ${mp.design}: 腿表無此設計`);
    continue;
  }
  const origText = readFileSync(R(leg.origDut[0]), 'utf8');
  const n = origText.split(mp.anchor).length - 1;
  if (n !== 1) {
    failures.push(`mutprobe ${mp.design}: anchor 在 ${leg.origDut[0]} 命中 ${n}!=1(變異錨失準,禁施)`);
    continue;
  }
  const mutantPath = join(mutantDir, `${mp.design}.v`);
  writeFileSync(mutantPath, origText.replace(mp.anchor, mp.replacement));
  const comp = trySh('iverilog', compileArgs(`sim_${mp.design}_mut`, leg.tb, leg.wrapperO, [mutantPath]), repoRoot);
  const compiled = comp.code === 0;
  const probe = { design: mp.design, variant: mp.variant, compiled, matched: null, mismatched: null, red: false };
  if (compiled) {
    const run = trySh(join(runDir, `sim_${mp.design}_mut`), [], runDir);
    writeFileSync(join(runDir, `out_${mp.design}_mut.txt`), run.out);
    probe.mismatched = diffVs(mp.design, run.out);
    probe.matched = leg.scenes - probe.mismatched.length;
    probe.red = probe.matched < leg.expected.matched;
  } else {
    probe.red = true; // compiled=false 亦=紅(判據二擇一)
  }
  if (!probe.red) {
    failures.push(`mutprobe ${mp.design}/${mp.variant}: 變異腿未紅(matched=${probe.matched}/${leg.scenes})=TB 無判別力(10.12 無效測試)`);
  } else if (mp.measured && (mp.measured.matched !== probe.matched || mp.measured.compiled !== probe.compiled)) {
    failures.push(`mutprobe ${mp.design}/${mp.variant}: 實測 matched=${probe.matched} compiled=${probe.compiled} != legs.json 錄入 matched=${mp.measured.matched} compiled=${mp.measured.compiled}(判別力數字漂移)`);
  }
  probes.push(probe);
  console.log(`MUT ${mp.design}/${mp.variant} ${compiled ? `matched=${probe.matched}/${leg.scenes}` : 'compiled=false ' + (comp.err ?? '').split('\n')[0]?.slice(0, 120)} → ${probe.red ? '紅(判別力成立)' : '未紅'}`);
}

const verdict = {
  tool: 'iverilog',
  iverilog: iverilogVersion,
  wasmSha256: wasmSha,
  slice: slice ?? null,
  legs,
  mutprobe: probes,
  generatedAt: new Date().toISOString(),
};
writeFileSync(join(outDir, 'verdict.json'), JSON.stringify(verdict, null, 2) + '\n');

// 證據頁(零牆鐘,R2-1;默認模式落 build/;--record 才寫已提交頁,D-S6 收編保鮮)。
const page = [];
page.push('# rtl2spec spec-eval 證據頁(D 弧評測床;機械生成禁手改)');
page.push('');
page.push(`- 命令行:\`bash plugin/rtl2spec/run-spec-eval.sh\`(默認模式重生成本頁到 build/ 並 byte 比對;\`--record --slice <D-Sn>\` 寫本頁;直調不進 regress,D-PLAN §5.2)`);
page.push(`- 口徑:iverilog -g2001 編譯(tb+wrapperO+DUT)+tbproto.sceneBlocks 逐場景塊 diff(形沿 reimpl-eval);首批腿=自對拍(reimplDut==origDut),盲寫腿 D-S5 凍結後增補;mutprobe=每設計一條確定性變異必紅(10.12 判別力)`);
page.push(`- 工具鏈(錄製時):${iverilogVersion};wasm sha256 \`${wasmSha.slice(0, 16)}\``);
page.push(`- 接線契約(R1-2):spec.module SSOT=spec 檔(rtl_spec_spec_parse);DUT 檔含 \`module <spec.module>\`=啟動斷言`);
page.push('');
page.push('| 腿 | 設計 | compiled | matched | expected |');
page.push('|---|---|---|---|---|');
for (const [i, l] of legs.entries()) {
  const exp = legsSpec.legs[i].expected;
  page.push(`| ${l.leg} | ${l.design} | ${l.compiled} | ${l.compiled ? `${l.scenesMatched}/${l.scenesTotal}` : '-'} | ${exp.compiled ? exp.matched : 'compile-fail'}/${exp.compiled} |`);
}
page.push('');
page.push('| mutprobe | 變異 | compiled | matched | 判別 |');
page.push('|---|---|---|---|---|');
for (const p of probes) {
  page.push(`| ${p.design} | ${p.variant} | ${p.compiled} | ${p.compiled ? `${p.matched}/${legsSpec.legs.find((l) => l.design === p.design).scenes}` : '-'} | ${p.red ? '紅' : '未紅'} |`);
}
page.push('');
const pageText = page.join('\n') + '\n';
writeFileSync(join(outDir, 'spec-eval-report.md'), pageText);
const committedPage = join(pluginRoot, 'spec-eval-report.md');
if (recordMode) {
  writeFileSync(committedPage, pageText);
  console.log('INFO 已提交證據頁已寫 plugin/rtl2spec/spec-eval-report.md(--record)');
} else {
  // 保鮮形(conformance-report.md 同形,R1-5/R2-1):默認模式與已提交頁 byte 比對。
  const stored = existsSync(committedPage) ? readFileSync(committedPage, 'utf8') : null;
  if (stored !== pageText) {
    failures.push(`證據頁 spec-eval-report.md 與重生成 byte 不等(${stored === null ? '缺' : '陳舊或手改'})— 重錄:bash plugin/rtl2spec/run-spec-eval.sh --record --slice <D-Sn>`);
  } else {
    console.log('PASS 證據頁 spec-eval-report.md 與重生成 byte 相等');
  }
}

if (failures.length > 0) {
  console.log('FAIL spec-eval:');
  for (const f of failures) console.log(`  - ${f}`);
  process.exit(1);
}
console.log(`== spec-eval: ${legs.length} 腿對 expected 逐字+mutprobe ${probes.length} 條全紅 ==`);
