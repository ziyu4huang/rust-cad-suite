// frames.mjs — 波形幀診斷腿純函數層(RTL2SPEC-FINISH-PLAN §4;CONFORMANCE-PROTOCOL §8)。零 IO。
//
// 幀診斷=輔助證據,永不進確定性閘(AGENTS 10.11):本檔只提供機械前置(統一窗、剝 title 畫布比較、modality 判定)
// 與工件驗證器(驗機械屬性,不驗 LLM 內容)。VCD 讀取/渲染一律走 vl-vcd(ADR-0004);本檔只讀 vcd2svg 產出的 SVG 文本,
// 不解析 VCD。值在 <title> tooltip、畫布無值文字(render.rs:18-19),esc 保留 \t\n\r(render.rs:95)→ title 可跨行。

export const PNG_MAGIC = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
export const CONFIDENCE = ['high', 'medium', 'low'];
export const MODALITIES = ['vision', 'text-fallback'];
export const LIMIT_CANVAS_SAME = '畫布層不可分(差異僅在 title tooltip 值)';
export const LIMIT_PNG_SAME = 'SVG 畫布有差但兩幀 PNG 相同(柵格化吞差)';
export const LIMIT_NO_RASTER = '柵格化鏈不可得(sips/qlmanage 皆未產出 PNG)';

/// 剝 title(程序化非貪婪跨行):title 元素 XML 不可嵌套,非貪婪必停在最近閉標籤;禁逐行 sed
/// (跨行 title 剝不淨會誤判「畫布不同」→ 偽放行 vision)。
export const stripTitles = (svg) => svg.replace(/<title>[\s\S]*?<\/title>/g, '');

export const isPngBytes = (buf) => PNG_MAGIC.every((b, k) => buf?.[k] === b);

/// 默認窗 SVG 的時間軸末標籤=t_max(render.rs 的 g=8 標籤=t0+span;五標籤 g=0,2,4,6,8)。不解析 VCD。
export function axisTmax(svg) {
  const labels = [...svg.matchAll(/ text-anchor="middle">([0-9]+)<\/text>/g)].map((m) => Number(m[1]));
  if (labels.length !== 5 || labels[0] !== 0) throw new Error(`時間軸標籤形非預期: ${JSON.stringify(labels)}`);
  return labels[4];
}

/// SVG 行解析(只讀 title 層):[{name,width,type,segs:[{t0,t1,value}]}];`<g>` 不嵌套(render.rs 每變數一個)。
export function parseSvgRows(svg) {
  const rows = [];
  for (const m of svg.matchAll(/<g><title>([\s\S]*?)<\/title>([\s\S]*?)<\/g>/g)) {
    const h = m[1].match(/^([\s\S]*) w=([0-9]+) type=([\s\S]*)$/);
    if (!h) throw new Error(`行 title 形非預期: ${m[1].slice(0, 60)}`);
    const segs = [...m[2].matchAll(/<title>t=([0-9]+)\.\.([0-9]+): ([\s\S]*?)<\/title>/g)].map((s) => ({ t0: Number(s[1]), t1: Number(s[2]), value: s[3] }));
    rows.push({ name: h[1], width: Number(h[2]), type: h[3], segs });
  }
  return rows;
}

/// 兩行首個分歧點(同窗;首段前=無變化資料,以起點差判)。null=全同。
export function firstRowDiff(a, b) {
  const A = a.segs;
  const B = b.segs;
  if (A.length === 0 && B.length === 0) return null;
  if (A.length === 0 || B.length === 0) {
    const s = (A.length ? A : B)[0];
    return { tick: s.t0, gold: A.length ? s.value : '<no changes>', mutant: B.length ? s.value : '<no changes>' };
  }
  if (A[0].t0 !== B[0].t0) {
    const goldFirst = A[0].t0 < B[0].t0;
    return { tick: Math.min(A[0].t0, B[0].t0), gold: goldFirst ? A[0].value : '<x>', mutant: goldFirst ? '<x>' : B[0].value };
  }
  let i = 0;
  let j = 0;
  while (i < A.length && j < B.length) {
    if (A[i].value !== B[j].value) return { tick: Math.max(A[i].t0, B[j].t0), gold: A[i].value, mutant: B[j].value };
    if (A[i].t1 < B[j].t1) i++;
    else if (B[j].t1 < A[i].t1) j++;
    else {
      i++;
      j++;
    }
  }
  return null;
}

/// title 層分歧信號表(序=首分歧 tick、再名字);機械歸因,不靠幀。
export function divergingSignals(goldSvg, mutantSvg) {
  const ga = parseSvgRows(goldSvg);
  const mb = new Map(parseSvgRows(mutantSvg).map((r) => [r.name, r]));
  const out = [];
  for (const r of ga) {
    const o = mb.get(r.name);
    if (!o) {
      out.push({ signal: r.name, width: r.width, firstTick: 0, gold: '<row>', mutant: '<absent>' });
      continue;
    }
    const d = firstRowDiff(r, o);
    if (d) out.push({ signal: r.name, width: r.width, firstTick: d.tick, gold: d.gold, mutant: d.mutant });
  }
  return out.sort((x, y) => x.firstTick - y.firstTick || (x.signal < y.signal ? -1 : x.signal > y.signal ? 1 : 0));
}

/// 剪幀窗(展示裁剪,不參與判定):分歧點前 1/16、後 1/8 全窗。
export function zoomWindow(firstTick, tmax) {
  const from = Math.max(0, firstTick - Math.floor(tmax / 16));
  const to = Math.min(tmax, firstTick + Math.floor(tmax / 8));
  return { from, to: Math.max(to, from + 1) };
}

/// 統一窗畫布雙檢 → modality。主判=剝 title 後 SVG byte 比較;輔判=PNG sha256;主判相同→強制 text-fallback。
export function decideModality({ goldSvg, mutantSvg, goldPngSha, mutantPngSha }) {
  const canvasDiffer = stripTitles(goldSvg) !== stripTitles(mutantSvg);
  const rastered = Boolean(goldPngSha && mutantPngSha);
  const pngDiffer = rastered ? goldPngSha !== mutantPngSha : null;
  const limitations = [];
  if (!canvasDiffer) limitations.push(LIMIT_CANVAS_SAME);
  if (!rastered) limitations.push(LIMIT_NO_RASTER);
  else if (canvasDiffer && !pngDiffer) limitations.push(LIMIT_PNG_SAME);
  return { canvasDiffer, pngDiffer, modalityAllowed: canvasDiffer && pngDiffer === true ? 'vision' : 'text-fallback', limitations };
}

/// diag 驗證器:只驗工件機械屬性,不驗 LLM 內容。io={exists(path),isPng(path)}。回傳 findings(空=過)。
/// 條款:frames 每項存在+PNG 魔數、framesRead==frames.length(實讀多少標多少)、modality=vision ⇒ 雙檢皆異且含兩張主幀、
/// confidence 三值;text-fallback 必須在 limitations 說明原因(含雙檢皆異但視覺會話不可用的降級)。
export function validateDiag(diag, manifest, io) {
  const f = [];
  if (!diag || typeof diag !== 'object') return ['diag 非物件'];
  if (diag.design !== manifest.design) f.push(`design ${diag.design}≠${manifest.design}`);
  if (diag.mutantId !== undefined && diag.mutantId !== manifest.mutant.id) f.push(`mutantId ${diag.mutantId}≠${manifest.mutant.id}`);
  if (!MODALITIES.includes(diag.modality)) f.push(`modality 不合法 ${diag.modality}`);
  if (!CONFIDENCE.includes(diag.confidence)) f.push(`confidence 不合法 ${diag.confidence}`);
  if (!Number.isInteger(diag.framesRead) || diag.framesRead < 0) f.push(`framesRead 不合法 ${diag.framesRead}`);
  if (!Array.isArray(diag.findings)) f.push('findings 非陣列');
  const frames = diag.frames ?? (diag.modality === 'vision' ? manifest.frames : []);
  if (!Array.isArray(frames)) f.push('frames 非陣列');
  else {
    if (diag.framesRead !== frames.length) f.push(`framesRead ${diag.framesRead}≠frames.length ${frames.length}`);
    for (const p of frames) {
      if (!io.exists(p)) f.push(`幀不存在 ${p}`);
      else if (!io.isPng(p)) f.push(`幀非 PNG ${p}`);
    }
  }
  const lim = Array.isArray(diag.limitations) ? diag.limitations.filter((x) => String(x).trim() !== '') : [];
  if (diag.modality === 'vision') {
    if (manifest.modalityAllowed !== 'vision') f.push(`標 vision 但雙檢未過(canvasDiffer=${manifest.dualCheck?.canvasDiffer} pngDiffer=${manifest.dualCheck?.pngDiffer})`);
    if (Array.isArray(frames)) for (const p of manifest.primaryFrames ?? []) if (!frames.includes(p)) f.push(`vision 未讀主幀 ${p}`);
  } else if (diag.modality === 'text-fallback') {
    if (lim.length === 0) f.push('text-fallback 必須在 limitations 說明原因');
  }
  return f;
}
