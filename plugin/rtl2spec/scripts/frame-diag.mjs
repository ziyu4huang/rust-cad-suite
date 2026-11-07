// frame-diag.mjs — 波形幀診斷腿機械段(RTL2SPEC-FINISH-PLAN §4;CONFORMANCE-PROTOCOL §8)。輔助證據,**不進任何閘**。
//
// 用法(經 run-frame-diag.sh,守衛在 shell):
//   bun scripts/frame-diag.mjs [design…] [--mutant <design>=<id>]   產幀+counterexample+prompt/機械 text-fallback
//   bun scripts/frame-diag.mjs --validate [design…]                 驗 diag-vision/<design>.json(只驗工件機械屬性)
// 寫面:plugin/rtl2spec/build/conformance/{frames,diag-vision}/(gitignored);入庫 tb.v 零改(wavecap 第二 top 取波形)。
// VCD 讀取/渲染走 vl-vcd(target/debug/vcd2svg,腿內不構建);PNG 只是查看手段,SVG 才是交付形(ADR-0005)。
// exit:0=機械段過(text-fallback 不是失敗);1=工件紅(波形探針擾動 TB 輸出/驗證器紅);2=用法/缺件。

import { createHash } from 'node:crypto';
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

import { diffScenes, iverilogRun, normalizeIverilog, run } from './lib/tbproto.mjs';
import { instantiation } from './conformance/tblint.mjs';
import { applyMutant, planMutations } from './conformance/mutate.mjs';
import { diagnose } from './conformance/diagnose.mjs';
import { axisTmax, decideModality, divergingSignals, isPngBytes, LIMIT_NO_RASTER, parseSvgRows, validateDiag, zoomWindow } from './conformance/frames.mjs';

const pluginRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const repoRoot = dirname(dirname(pluginRoot));
const rel = (p) => relative(repoRoot, p);
const fixRoot = join(pluginRoot, 'fixtures', 'conformance');
const framesRoot = join(pluginRoot, 'build', 'conformance', 'frames');
const diagRoot = join(pluginRoot, 'build', 'conformance', 'diag-vision');
const VCD2SVG = join(repoRoot, 'target', 'debug', 'vcd2svg');
const ZOOM_SIGNALS = 8;
const sha256 = (b) => createHash('sha256').update(b).digest('hex');

const argv = process.argv.slice(2);
const validateMode = argv.includes('--validate');
const mutantOverride = new Map();
const designArgs = [];
for (let k = 0; k < argv.length; k++) {
  if (argv[k] === '--validate') continue;
  if (argv[k] === '--mutant') {
    const v = argv[++k] ?? '';
    const eq = v.indexOf('=');
    if (eq <= 0) {
      console.log('FAIL usage: --mutant <design>=<id>');
      process.exit(2);
    }
    mutantOverride.set(v.slice(0, eq), v.slice(eq + 1));
    continue;
  }
  designArgs.push(argv[k]);
}
const ALL = ['acc_pipe', 'fsm', 'i2c_master', 'signedexpr', 'sincos', 'subbytes'];
const selected = designArgs.length ? designArgs : ALL;
for (const d of selected) {
  if (!existsSync(join(fixRoot, d, 'case.json'))) {
    console.log(`FAIL usage: 無此設計 ${d}`);
    process.exit(2);
  }
}

let failures = 0;
const ck = (ok, name, detail = '') => {
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
  if (!ok) failures++;
  return ok;
};

// ---------- 驗證模式(不產幀,只驗工件) ----------
const framePaths = (design) => ({
  dir: join(framesRoot, design),
  manifest: join(framesRoot, design, 'counterexample.json'),
  diag: join(diagRoot, `${design}.json`),
});
if (validateMode) {
  console.log('== frame-diag --validate(只驗工件機械屬性,不驗 LLM 內容) ==');
  const io = { exists: (p) => existsSync(join(repoRoot, p)), isPng: (p) => isPngBytes(readFileSync(join(repoRoot, p))) };
  for (const d of selected) {
    const p = framePaths(d);
    if (!existsSync(p.manifest)) {
      console.log(`SKIP ${d} 無 counterexample.json(未產幀)`);
      continue;
    }
    if (!existsSync(p.diag)) {
      console.log(`SKIP ${d} 無 diag-vision/${d}.json(診斷未落)`);
      continue;
    }
    const manifest = JSON.parse(readFileSync(p.manifest, 'utf8'));
    let diag;
    try {
      diag = JSON.parse(readFileSync(p.diag, 'utf8'));
    } catch (e) {
      ck(false, `${d} diag JSON 可解析`, e.message);
      continue;
    }
    const f = validateDiag(diag, manifest, io);
    ck(f.length === 0, `${d} diag 工件`, f.length ? f.join(' | ') : `modality=${diag.modality} framesRead=${diag.framesRead} confidence=${diag.confidence} 雙檢 canvas=${manifest.dualCheck.canvasDiffer} png=${manifest.dualCheck.pngDiffer}`);
  }
  console.log(`== frame-diag --validate: ${failures ? 'FAIL' : 'PASS'} ==`);
  process.exit(failures ? 1 : 0);
}

// ---------- 裝載 dist wasm(tb_plan;閘驗出貨物同源) ----------
const wasmDir = join(pluginRoot, 'dist', 'wasm');
const glue = await import(join(wasmDir, 'rtl_spec_wasm.js'));
await glue.default({ module_or_path: new Uint8Array(readFileSync(join(wasmDir, 'rtl_spec_wasm_bg.wasm'))) });

// ---------- 協議釘檔的視覺 prompt(逐字來源=CONFORMANCE-PROTOCOL §8 圍欄,不複製模板文本) ----------
function visionPromptTemplate() {
  const L = readFileSync(join(pluginRoot, 'CONFORMANCE-PROTOCOL.md'), 'utf8').split('\n');
  const b = L.indexOf('<!-- vision-prompt v1 begin -->');
  const s = L.indexOf('```text', b);
  const e = L.indexOf('```', s + 1);
  if (b < 0 || s < 0 || e < 0) throw new Error('CONFORMANCE-PROTOCOL §8 視覺 prompt 圍欄缺');
  return L.slice(s + 1, e).join('\n') + '\n';
}

// ---------- 柵格化(查看手段;sips 首選,qlmanage 備選;皆不可得=降級) ----------
function rasterize(svgPath, pngPath) {
  const r = run('sips', ['-s', 'format', 'png', svgPath, '--out', pngPath], { timeoutMs: 60000 });
  if (r.code === 0 && existsSync(pngPath) && isPngBytes(readFileSync(pngPath))) return 'sips';
  const outDir = dirname(pngPath);
  const q = run('qlmanage', ['-t', '-s', '960', '-o', outDir, svgPath], { timeoutMs: 60000 });
  const qlOut = join(outDir, `${basename(svgPath)}.png`);
  if (q.code === 0 && existsSync(qlOut) && isPngBytes(readFileSync(qlOut))) {
    cpSync(qlOut, pngPath);
    rmSync(qlOut, { force: true });
    return 'qlmanage';
  }
  return null;
}

function renderSvg(vcd, out, { from, to, signals } = {}) {
  const args = [];
  if (from !== undefined) args.push('--from', String(from));
  if (to !== undefined) args.push('--to', String(to));
  if (signals?.length) args.push('--signals', signals.join(','));
  args.push('-o', out, vcd);
  const r = run(VCD2SVG, args, { timeoutMs: 60000 });
  if (r.code !== 0) throw new Error(`vcd2svg 失敗 ${vcd}: ${(r.stderr || r.spawnError || '').slice(0, 160)}`);
  return readFileSync(out, 'utf8');
}

// ---------- 逐設計 ----------
function frameDesign(design) {
  const cs = JSON.parse(readFileSync(join(fixRoot, design, 'case.json'), 'utf8'));
  const rec = JSON.parse(readFileSync(join(fixRoot, design, 'record.json'), 'utf8'));
  const tbPath = join(fixRoot, design, 'tb.v');
  const tbText = readFileSync(tbPath, 'utf8');
  const expected = readFileSync(join(fixRoot, design, 'expected.txt'), 'utf8');
  const srcPath = join(repoRoot, cs.src);
  const srcText = readFileSync(srcPath, 'utf8');
  const spec = JSON.parse(readFileSync(join(repoRoot, cs.specPath), 'utf8'));
  const rules = spec.modules.find((m) => m.module === cs.top).rules;

  // counterexample=指定變異,缺省=record 主腿首個 killed 的 P-rule 變異(確定性選取)。
  const pick = mutantOverride.get(design);
  const mrec = pick ? rec.mutants.find((m) => m.id === pick) : rec.mutants.find((m) => m.pool === 'rule' && m.result === 'killed');
  if (!mrec) return ck(false, `${design} 無可用 counterexample 變異`, pick ?? '(主腿無 killed)');
  if (mrec.result !== 'killed') return ck(false, `${design} ${mrec.id} 非 killed(${mrec.result}),無 counterexample`);
  const pm = planMutations(srcText, rules).mutants.find((m) => m.id === mrec.id);
  if (!pm || pm.key !== mrec.key) return ck(false, `${design} ${mrec.id} 重放與 record 不符`, pm?.key);

  const out = join(framesRoot, design);
  const work = join(out, 'work');
  const svgDir = join(out, 'svg');
  rmSync(out, { recursive: true, force: true });
  rmSync(join(diagRoot, `${design}.json`), { force: true }); // 幀重產,舊診斷作廢
  rmSync(join(diagRoot, `${design}.prompt.md`), { force: true });
  mkdirSync(work, { recursive: true });
  mkdirSync(svgDir, { recursive: true });
  mkdirSync(diagRoot, { recursive: true });

  // 波形探針=第二 top,tb.v 零改;$dumpvars 必須層次實例路徑。
  const tbTop = `tb_${cs.top}`;
  const inst = instantiation(tbText.replace(/\/\/.*$/gm, ''), cs.top);
  if (!inst.found) return ck(false, `${design} tb.v 內找不到 ${cs.top} 實例`);
  const capName = `wavecap_${cs.top}`;
  const capture = (role, dutPath) => {
    const cap = join(work, `wavecap_${role}.v`);
    writeFileSync(cap, `\`timescale 1ns/1ps\nmodule ${capName};\n  initial begin\n    $dumpfile("${role}.vcd");\n    $dumpvars(0, ${tbTop}.${inst.inst});\n  end\nendmodule\n`);
    const r = iverilogRun({ flags: [...(cs.iverilog ?? ['-g2005']), '-s', capName], top: tbTop, files: [tbPath, dutPath, cap], workDir: work, outName: `${role}.vvp`, timeoutMs: 30000 });
    if (!r.compiled) throw new Error(`${role} 編譯失敗: ${(r.compErr ?? '').slice(0, 160)}`);
    if (r.timedOut) throw new Error(`${role} 模擬逾時`);
    writeFileSync(join(work, `${role}.stdout`), r.stdout);
    return { norm: normalizeIverilog(r.stdout), vcd: join(work, `${role}.vcd`) };
  };
  const mutPath = join(work, `${mrec.id}.v`);
  writeFileSync(mutPath, applyMutant(srcText, pm));
  const gold = capture('gold', srcPath);
  const mut = capture('mutant', mutPath);

  // 波形探針不得擾動 TB 判別力:原版輸出==入庫 expected.txt;變異殺傷場景==record.killScenes。
  const goldSame = gold.norm === expected;
  const scenes = diffScenes(expected, mut.norm);
  const scenesSame = JSON.stringify(scenes) === JSON.stringify(mrec.killScenes);
  ck(goldSame && scenesSame, `${design} 波形探針零擾動(原版==expected.txt,${mrec.id} 殺傷場景==record)`, goldSame && scenesSame ? `${scenes.length} 場景` : `gold=${goldSame} scenes=${JSON.stringify(scenes).slice(0, 120)} record=${JSON.stringify(mrec.killScenes).slice(0, 120)}`);

  const frame = JSON.parse(glue.rtl_tb_plan_sv(cs.src, srcText, cs.top, cs.seed ?? 0, cs.cycles ?? 0, cs.flush ?? 0));

  // 統一窗:TMAX=兩 VCD t_max 較大者(取自 vl-vcd 默認窗軸標籤);兩幀一律 --from 0 --to TMAX。
  const tGold = axisTmax(renderSvg(gold.vcd, join(work, 'gold.default.svg')));
  const tMut = axisTmax(renderSvg(mut.vcd, join(work, 'mutant.default.svg')));
  const tmax = Math.max(tGold, tMut);
  const goldSvg = renderSvg(gold.vcd, join(svgDir, 'gold.svg'), { from: 0, to: tmax });
  const mutSvg = renderSvg(mut.vcd, join(svgDir, 'mutant.svg'), { from: 0, to: tmax });
  const gp = join(out, 'gold.png');
  const mp = join(out, 'mutant.png');
  const rg = rasterize(join(svgDir, 'gold.svg'), gp);
  const rm = rasterize(join(svgDir, 'mutant.svg'), mp);
  const shaOf = (p, ok) => (ok && existsSync(p) ? sha256(readFileSync(p)) : null);
  const dec = decideModality({ goldSvg, mutantSvg: mutSvg, goldPngSha: shaOf(gp, rg), mutantPngSha: shaOf(mp, rm) });

  // title 層機械歸因(不靠幀);TB 協議不打印模擬時間,tick↔SCENE 無機械對應(如實聲明)。
  const div = divergingSignals(goldSvg, mutSvg);
  const frames = [];
  const primaryFrames = [];
  if (rg) primaryFrames.push(rel(gp));
  if (rm) primaryFrames.push(rel(mp));
  frames.push(...primaryFrames);

  // 剪幀(展示裁剪):分歧點附近窗+--signals(時鐘+前 N 個分歧信號)。只在主判可分時產出。
  let zoom = null;
  if (dec.modalityAllowed === 'vision' && div.length > 0) {
    const rowNames = new Set(parseSvgRows(goldSvg).map((r) => r.name));
    const clocks = (frame.ok ? frame.plan.clocks ?? [] : []).map((c) => `${tbTop}.${inst.inst}.${c.signal}`).filter((n) => rowNames.has(n));
    const rows = div.slice(0, ZOOM_SIGNALS).map((d) => d.signal);
    const sigs = [...clocks.filter((c) => !rows.includes(c)), ...rows];
    const w = zoomWindow(div[0].firstTick, tmax);
    const za = renderSvg(gold.vcd, join(svgDir, 'gold_zoom.svg'), { ...w, signals: sigs });
    const zb = renderSvg(mut.vcd, join(svgDir, 'mutant_zoom.svg'), { ...w, signals: sigs });
    const gz = join(out, 'gold_zoom.png');
    const mz = join(out, 'mutant_zoom.png');
    const rzg = rasterize(join(svgDir, 'gold_zoom.svg'), gz);
    const rzm = rasterize(join(svgDir, 'mutant_zoom.svg'), mz);
    if (rzg && rzm) frames.push(rel(gz), rel(mz));
    const zdec = decideModality({ goldSvg: za, mutantSvg: zb, goldPngSha: shaOf(gz, rzg), mutantPngSha: shaOf(mz, rzm) });
    zoom = { from: w.from, to: w.to, signals: sigs, canvasDiffer: zdec.canvasDiffer, pngDiffer: zdec.pngDiffer };
  }

  // stdout 層機械診斷(diagnose.mjs 同款;留一法排除自身)。
  const covers = [...tbText.matchAll(/^\/\/ COVERS: ([0-9]+): (.+)$/gm)].map((m) => ({ n: Number(m[1]), ids: m[2].split(',').map((x) => x.trim()) }));
  const dg = frame.ok ? diagnose({ expectedNorm: expected, actualNorm: mut.norm, plan: frame.plan, rules, covers, mutants: rec.mutants, excludeKey: mrec.key }) : null;

  const manifest = {
    schema: 'frame-counterexample-v1',
    readMe: '波形幀診斷的機械前置(輔助證據,不進閘)。值不在畫布上,只有沿與帶形。vision=確實逐張讀了 frames 列出的 PNG;只讀本 JSON/SVG 文本=text-fallback。diag-vision/<design>.json 必含 design、mutantId、modality、frames(實讀 PNG 路徑)、framesRead(=frames 長度,SVG/JSON 不計)、findings、confidence(high|medium|low)、limitations、verdict。modalityAllowed=text-fallback 時不得標 vision。titleLayer/stdoutLayer 是機械歸因,禁把它們改寫成「從幀上看到」。',
    design,
    top: cs.top,
    src: cs.src,
    tb: { path: rel(tbPath), leg: rec.tb.leg, sha256: rec.tb.sha256 },
    mutant: { id: mrec.id, op: mrec.op, line: mrec.line, before: mrec.before.trim(), after: mrec.after.trim(), ruleIds: mrec.ruleIds, killScenes: mrec.killScenes },
    window: { from: 0, to: tmax, goldTmax: tGold, mutantTmax: tMut, unified: true },
    zoom,
    frames,
    primaryFrames,
    svg: [rel(join(svgDir, 'gold.svg')), rel(join(svgDir, 'mutant.svg')), ...(zoom ? [rel(join(svgDir, 'gold_zoom.svg')), rel(join(svgDir, 'mutant_zoom.svg'))] : [])],
    dualCheck: { canvasDiffer: dec.canvasDiffer, pngDiffer: dec.pngDiffer, rasterizer: rg && rm ? (rg === rm ? rg : `${rg}+${rm}`) : null, goldPngSha: shaOf(gp, rg), mutantPngSha: shaOf(mp, rm) },
    modalityAllowed: dec.modalityAllowed,
    limitations: dec.limitations,
    titleLayer: { divergingCount: div.length, divergingSignals: div.slice(0, 16) },
    stdoutLayer: dg ? { classification: dg.classification, failing: dg.failing.slice(0, 6), diffPorts: dg.diffPorts, suspectsTop3: dg.suspects.slice(0, 3).map((s) => ({ ruleId: s.ruleId, why: s.why })) } : null,
    notes: ['TB 協議不打印模擬時間,tick↔SCENE 無機械對應;tick 只能由 title 層取得', 'label 欄 120px 是 render 契約,長信號名會蓋到畫布前段,不代表波形差異'],
    perturbation: { goldEqExpected: goldSame, killScenesEqRecord: scenesSame },
  };
  writeFileSync(join(out, 'counterexample.json'), JSON.stringify(manifest, null, 2) + '\n');

  // 視覺可讀=寫 prompt,交 LLM 腿;強制 fallback=寫機械 text-fallback 診斷(不經 LLM)。
  if (dec.modalityAllowed === 'vision') {
    const prompt = visionPromptTemplate()
      .replace('{DESIGN}', design)
      .replace('{FRAMES}', frames.join('\n'))
      .replace('{COUNTEREXAMPLE}', rel(join(out, 'counterexample.json')))
      .replace('{OUT}', rel(join(diagRoot, `${design}.json`)));
    writeFileSync(join(diagRoot, `${design}.prompt.md`), prompt);
  } else {
    const firsts = div.slice(0, 8).map((d) => ({ signal: d.signal, tickWindow: [d.firstTick, d.firstTick], observation: `title 層首分歧:gold=${d.gold} mutant=${d.mutant}`, hypothesis: `已知變異 ${mrec.op}@${mrec.line} 歸屬 ${mrec.ruleIds.join(',')}(取自 record,非推斷)` }));
    const diag = {
      design,
      mutantId: mrec.id,
      modality: 'text-fallback',
      frames: [],
      framesRead: 0,
      findings: firsts,
      confidence: 'low',
      limitations: dec.limitations.length ? dec.limitations : [LIMIT_NO_RASTER],
      verdict: `${dec.limitations[0] ?? LIMIT_NO_RASTER};只讀 SVG title 文本與 counterexample JSON,未讀圖,未做視覺判斷。`,
      producedBy: 'frame-diag.mjs(機械,未經 LLM)',
    };
    writeFileSync(join(diagRoot, `${design}.json`), JSON.stringify(diag, null, 2) + '\n');
  }
  console.log(`INFO ${design} ${mrec.id} ${mrec.op}@${mrec.line} tmax ${tGold}/${tMut} canvas異=${dec.canvasDiffer} png異=${dec.pngDiffer} raster=${manifest.dualCheck.rasterizer} 分歧信號=${div.length} frames=${frames.length} zoom=${zoom ? `${zoom.from}..${zoom.to}/${zoom.signals.length}信號` : '-'} → modality=${dec.modalityAllowed}`);
  return manifest;
}

console.log('== frame-diag(機械段;輔助證據,不進閘) ==');
const rowsOut = [];
for (const d of selected) {
  try {
    const m = frameDesign(d);
    if (m && m !== true) rowsOut.push(m);
  } catch (e) {
    ck(false, `${d} 幀鏈`, e.message);
  }
}
const vis = rowsOut.filter((m) => m.modalityAllowed === 'vision').map((m) => m.design);
const fb = rowsOut.filter((m) => m.modalityAllowed !== 'vision').map((m) => m.design);
console.log(`SUMMARY frame-diag designs=${rowsOut.length} vision=${vis.length}[${vis.join(',')}] text-fallback=${fb.length}[${fb.join(',')}]`);
console.log(`== frame-diag: ${failures ? 'FAIL' : 'PASS'} ==`);
process.exit(failures ? 1 : 0);
