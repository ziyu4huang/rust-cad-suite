// mutate.mjs — 規則錨定變異器(§19.4.2 算子目錄 v1;C-S3)。純函數,零 IO。
//
// 兩池:P-rule(規則錨定,入殺傷率分母)/P-unruled(源檔中含賦值、但不在任何規則 evidence
// 區間內的行;不入分母,被殺=TB 可觀測而規則書未描述=規則書缺陷的確定性證據,餵 F3)。
// 控制組:OP-NOP(行尾追加註釋,必存活)。
//
// 錨定(復審 RV2-5:按規則主體定行):候選區=evidence[0] 行區間;錨定行=區內「賦值左值經 norm
// 後==規則主體」的唯一一行(guard 每個 effect.lhs 一錨/sequence signal/relation lhs/transition
// state_reg);零或多匹配=ambiguous(附原因)。手術=錨定行上目標 token count==1 才替換;RHS 一律
// 取源碼原文(不用規則渲染值)。
//
// v1 選用序(規格只定「按 form 選用;每規則每算子 ≤1、每規則 ≤2」,未定兩個取哪兩個——此處
// 以「規則專屬算子先於行程共享算子」定序並錄入 record.json):
//   guard=RVAL→ASYNC(async 才適用)→POL;transition=STATE→CONST;
//   sequence=HOLD→ARITH→CMP→CONST→SIGN→SLICE;relation=ARITH→CMP→CONST→SIGN→SLICE。
// 歸屬集=全部「該算子對其 form 適用且其錨定行==變異所在行」的規則(與該規則自身預算無關)。

export const OPCAT_VERSION = 'opcat-v1';

export const PRIORITY = {
  guard: ['OP-RVAL', 'OP-ASYNC', 'OP-POL'],
  transition: ['OP-STATE', 'OP-CONST'],
  sequence: ['OP-HOLD', 'OP-ARITH', 'OP-CMP', 'OP-CONST', 'OP-SIGN', 'OP-SLICE'],
  relation: ['OP-ARITH', 'OP-CMP', 'OP-CONST', 'OP-SIGN', 'OP-SLICE'],
  invariant: [],
};
const UNRULED_PRIORITY = ['OP-HOLD', 'OP-ARITH', 'OP-CMP', 'OP-CONST', 'OP-SIGN', 'OP-SLICE'];
const PER_RULE_BUDGET = 2;
const NOP_TAG = ' // conformance-nop';

// ---------- 文本工具 ----------

/// norm(單一定義;只用於渲染文本↔源碼比對):去空白+無位寬十進制字面量 N→'sdN。
export function norm(s) {
  const t = s.replace(/\s+/g, '');
  return t.replace(/(^|[^0-9A-Za-z_$'])([0-9]+)(?=[^0-9A-Za-z_$']|$)/g, (_, p, d) => `${p}'sd${d}`);
}

const codeOf = (line) => {
  // 剝行註釋(字串內 // 不在本語料面出現;保守處理)。
  const i = line.indexOf('//');
  return i >= 0 ? line.slice(0, i) : line;
};

function matchParen(s, open) {
  let depth = 0;
  for (let k = open; k < s.length; k++) {
    if (s[k] === '(') depth++;
    else if (s[k] === ')') {
      depth--;
      if (depth === 0) return k;
    }
  }
  return -1;
}

/// 賦值行解析:剝 begin/end/else/assign/if(…)/case 標簽/net 宣告前綴後,取行首
/// `LHS (<=|=) RHS;`。回傳 {lhs, op, rhs, rhsStart, rhsEnd}(絕對偏移,供原位手術)或 null。
export function parseAssign(line) {
  const code = codeOf(line);
  let k = 0;
  const skipWs = () => {
    while (k < code.length && /\s/.test(code[k])) k++;
  };
  for (let guard = 0; guard < 16; guard++) {
    skipWs();
    const rest = code.slice(k);
    let m;
    if ((m = rest.match(/^(begin|end|else|assign)\b/))) {
      k += m[0].length;
      continue;
    }
    if ((m = rest.match(/^if\s*\(/))) {
      const open = k + m[0].length - 1;
      const close = matchParen(code, open);
      if (close < 0) return null;
      k = close + 1;
      continue;
    }
    if ((m = rest.match(/^(wire|reg|logic)\b(\s+signed\b)?\s*(\[[^\]]*\]\s*)?/))) {
      k += m[0].length;
      continue;
    }
    // case 標簽(可多標簽):無括號/方括號/=/;/< > 的前綴後接單個 `:`。
    if ((m = rest.match(/^([^:;=<>()[\]{}?]+):(?!:)/))) {
      k += m[0].length;
      continue;
    }
    break;
  }
  const rest = code.slice(k);
  const m = rest.match(/^(\{[^}]*\}|[A-Za-z_][A-Za-z0-9_$]*(?:\s*\[[^\]]*\])*)\s*(<=|=)(?!=)\s*/);
  if (!m) return null;
  const lhs = m[1];
  const op = m[2];
  const rhsStart = k + m[0].length;
  const semi = code.lastIndexOf(';');
  if (semi < rhsStart) return null;
  const rhs = code.slice(rhsStart, semi).trim();
  if (rhs === '') return null;
  return { lhs, op, rhs, rhsStart, rhsEnd: semi };
}

const replaceAt = (s, start, end, text) => s.slice(0, start) + text + s.slice(end);

function countToken(line, re) {
  return [...codeOf(line).matchAll(re)].length;
}

// ---------- 字面量 ----------

const SIZED = /([0-9]+)\s*'\s*([sS]?)([bBoOdDhH])\s*([0-9a-fA-F_xXzZ?]+)/g;

function bumpSized(width, signedFlag, base, digits) {
  const clean = digits.replace(/_/g, '');
  if (/[xXzZ?]/.test(clean)) return null;
  const radix = { b: 2, o: 8, d: 10, h: 16 }[base.toLowerCase()];
  let v;
  try {
    v = BigInt(radix === 10 ? clean : `0${{ 2: 'b', 8: 'o', 16: 'x' }[radix]}${clean}`);
  } catch {
    return null;
  }
  const w = BigInt(Number(width));
  if (w <= 0n || w > 4096n) return null;
  const mod = 1n << w;
  const nv = (v + 1n) % mod;
  let txt = nv.toString(radix);
  if (radix !== 10) txt = txt.padStart(clean.length, '0');
  if (/[A-F]/.test(clean)) txt = txt.toUpperCase();
  return `${width}'${signedFlag}${base}${txt}`;
}

/// RHS 首個可加一字面量(方括號外;有位寬無 x/z 或無位寬十進制)。回傳 {start,end,text,next}(RHS 內偏移)。
function firstBumpable(rhs) {
  let depth = 0;
  const masks = [];
  for (let k = 0; k < rhs.length; k++) {
    if (rhs[k] === '[') depth++;
    else if (rhs[k] === ']') depth = Math.max(0, depth - 1);
    masks.push(depth > 0);
  }
  const cands = [];
  for (const m of rhs.matchAll(SIZED)) {
    if (masks[m.index]) continue;
    const next = bumpSized(m[1], m[2], m[3], m[4]);
    if (next) cands.push({ start: m.index, end: m.index + m[0].length, text: m[0], next });
  }
  for (const m of rhs.matchAll(/(^|[^0-9A-Za-z_$'])([0-9]+)(?=[^0-9A-Za-z_$']|$)/g)) {
    const start = m.index + m[1].length;
    if (masks[start]) continue;
    // 排除有位寬字面量的寬度段(後隨 ')
    if (/^\s*'/.test(rhs.slice(start + m[2].length))) continue;
    const next = (BigInt(m[2]) + 1n).toString();
    cands.push({ start, end: start + m[2].length, text: m[2], next });
  }
  cands.sort((a, b) => a.start - b.start);
  return cands[0] ?? null;
}

/// RHS 是否「單一字面量」(OP-RVAL 前提)。
function singleLiteral(rhs) {
  const t = rhs.trim();
  const sized = t.match(/^([0-9]+)\s*'\s*([sS]?)([bBoOdDhH])\s*([0-9a-fA-F_xXzZ?]+)$/);
  if (sized) {
    const next = bumpSized(sized[1], sized[2], sized[3], sized[4]);
    return next ? { next, xz: false } : { next: null, xz: /[xXzZ?]/.test(sized[4]) };
  }
  if (/^'[sS]?[bBoOdDhH][xXzZ?]+$/.test(t) || /^'[01xXzZ]$/.test(t)) return { next: null, xz: /[xXzZ?]/.test(t) };
  if (/^[0-9]+$/.test(t)) return { next: (BigInt(t) + 1n).toString(), xz: false };
  return null;
}

// ---------- RHS 算子手術(只動 RHS;count==1 於整行) ----------

const ARITH_SWAPS = [
  // [正則(RHS 內), 替換, 整行計數正則]——長者先。
  [/>>>/, '>>', />>>/g],
  [/(?<![<>])<<(?![<=])/, '>>', /(?<![<>])<<(?![<=])/g],
  [/(?<![<>])>>(?![>=])/, '<<', /(?<![<>])>>(?![>=])/g],
  [/(?<![+])\+(?![+:=])/, '-', /(?<![+])\+(?![+:=])/g],
  [/(?<![-])-(?![-:>=])/, '+', /(?<![-])-(?![-:>=])/g],
  [/(?<![&])&(?![&=])/, '|', /(?<![&])&(?![&=])/g],
  [/(?<![|])\|(?![|=])/, '&', /(?<![|])\|(?![|=])/g],
  [/(?<![~])\^(?![~=])/, '|', /(?<![~])\^(?![~=])/g],
];

function rhsSwap(line, pa, swaps) {
  // 按 RHS 內出現位置取最先者;整行 count==1 才替換。
  let best = null;
  for (const [re, rep, lineRe] of swaps) {
    const m = pa.rhs.match(re);
    if (!m) continue;
    if (!best || m.index < best.idx) best = { idx: m.index, len: m[0].length, rep, lineRe, tok: m[0] };
  }
  if (!best) return { status: 'n/a', reason: 'RHS 無目標算子' };
  if (countToken(line, best.lineRe) !== 1) return { status: 'ambiguous', reason: `算子 ${best.tok} 行內計數≠1` };
  const rhsAbs = line.indexOf(pa.rhs, pa.rhsStart);
  if (rhsAbs < 0) return { status: 'ambiguous', reason: 'RHS 定位失敗' };
  const at = rhsAbs + best.idx;
  return { status: 'ok', after: replaceAt(line, at, at + best.len, best.rep), detail: `${best.tok}→${best.rep}` };
}

const CMP_SWAPS = [
  [/>=/, '>', />=/g],
  [/(?<![=!])==(?!=)/, '!=', /(?<![=!])==(?!=)/g],
];

function opRhsConst(line, pa, kind) {
  const rhsAbs = line.indexOf(pa.rhs, pa.rhsStart);
  if (rhsAbs < 0) return { status: 'ambiguous', reason: 'RHS 定位失敗' };
  if (kind === 'OP-CONST') {
    const c = firstBumpable(pa.rhs);
    if (!c) return { status: 'n/a', reason: 'RHS 無可加一字面量' };
    const re = new RegExp(`(?<![0-9A-Za-z_$'])${c.text.replace(/[.*+?^${}()|[\]\\']/g, '\\$&')}(?![0-9A-Za-z_$'])`, 'g');
    if (countToken(line, re) !== 1) return { status: 'ambiguous', reason: `字面量 ${c.text} 行內計數≠1` };
    const at = rhsAbs + c.start;
    return { status: 'ok', after: replaceAt(line, at, at + c.text.length, c.next), detail: `${c.text}→${c.next}` };
  }
  if (kind === 'OP-SIGN') {
    const m = [...pa.rhs.matchAll(SIZED)].find((x) => !/[xXzZ?]/.test(x[4]));
    if (!m) return { status: 'n/a', reason: 'RHS 無有位寬字面量' };
    const next = m[2] ? `${m[1]}'${m[3]}${m[4]}` : `${m[1]}'s${m[3]}${m[4]}`;
    const re = new RegExp(m[0].replace(/[.*+?^${}()|[\]\\']/g, '\\$&'), 'g');
    if (countToken(line, re) !== 1) return { status: 'ambiguous', reason: `字面量 ${m[0]} 行內計數≠1` };
    const at = rhsAbs + m.index;
    return { status: 'ok', after: replaceAt(line, at, at + m[0].length, next), detail: `${m[0]}→${next}` };
  }
  if (kind === 'OP-SLICE') {
    const m = pa.rhs.match(/\[\s*([0-9]+)\s*:\s*([0-9]+)\s*\]/);
    if (!m) return { status: 'n/a', reason: 'RHS 無數值部分選擇' };
    const hi = Number(m[1]);
    const lo = Number(m[2]);
    const [nh, nl] = lo >= 1 ? [hi - 1, lo - 1] : [hi + 1, lo + 1];
    const re = new RegExp(m[0].replace(/[.*+?^${}()|[\]\\']/g, '\\$&'), 'g');
    if (countToken(line, re) !== 1) return { status: 'ambiguous', reason: `部分選擇 ${m[0]} 行內計數≠1` };
    const at = rhsAbs + m.index;
    const rep = `[${nh}:${nl}]`;
    return { status: 'ok', after: replaceAt(line, at, at + m[0].length, rep), detail: `${m[0]}→${rep}` };
  }
  return { status: 'n/a', reason: 'unknown' };
}

/// 賦值行上的 RHS 類算子(HOLD/ARITH/CMP/CONST/SIGN/SLICE)。
export function mutateAssignLine(line, op) {
  const pa = parseAssign(line);
  if (!pa) return { status: 'ambiguous', reason: '錨定行非賦值形' };
  switch (op) {
    case 'OP-HOLD': {
      if (norm(pa.rhs) === norm(pa.lhs)) return { status: 'n/a', reason: 'RHS 已是自身' };
      const rhsAbs = line.indexOf(pa.rhs, pa.rhsStart);
      if (rhsAbs < 0) return { status: 'ambiguous', reason: 'RHS 定位失敗' };
      return { status: 'ok', after: replaceAt(line, rhsAbs, rhsAbs + pa.rhs.length, pa.lhs.trim()), detail: `rhs→${pa.lhs.trim()}` };
    }
    case 'OP-ARITH':
      return rhsSwap(line, pa, ARITH_SWAPS);
    case 'OP-CMP':
      return rhsSwap(line, pa, CMP_SWAPS);
    case 'OP-CONST':
    case 'OP-SIGN':
    case 'OP-SLICE':
      return opRhsConst(line, pa, op);
    default:
      return { status: 'n/a', reason: `${op} 非賦值行算子` };
  }
}

// ---------- 規則錨定 ----------

const subjectsOf = (rule) => {
  const c = rule.check;
  switch (c.form) {
    case 'guard':
      return (c.effects ?? []).map((e) => ({ subject: e.lhs, value: e.value }));
    case 'sequence':
      return [{ subject: c.signal }];
    case 'relation':
      return [{ subject: c.lhs }];
    case 'transition':
      return [{ subject: c.state_reg }];
    default:
      return [];
  }
};

/// 區間內左值==主體的唯一行(1-based);{line} 或 {ambiguous:原因}。rhsWant(transition 的 `to`)
/// =多匹配時以「RHS 經 norm 相等」消歧(同一 case 臂內多條 state 賦值的 R1-5 歸屬口徑)。
function anchorLine(lines, ev, subject, rhsWant) {
  if (!ev) return { ambiguous: '無 evidence[0]' };
  const want = norm(subject);
  let hits = [];
  for (let n = ev.line; n <= ev.end_line; n++) {
    const pa = parseAssign(lines[n - 1] ?? '');
    if (pa && norm(pa.lhs) === want) hits.push({ n, rhs: pa.rhs });
  }
  if (hits.length > 1 && rhsWant !== undefined) hits = hits.filter((h) => norm(h.rhs) === norm(rhsWant));
  if (hits.length === 1) return { line: hits[0].n };
  return { ambiguous: hits.length === 0 ? `區間 ${ev.line}-${ev.end_line} 無左值==${subject}${rhsWant !== undefined ? `(且 RHS==${rhsWant})` : ''}` : `區間內 ${hits.length} 行左值==${subject}` };
}

/// 規則的主錨定行(transition 帶 `to` 消歧)。
const mainAnchor = (lines, r, subj) =>
  anchorLine(lines, r.evidence?.[0], subj.subject, r.check.form === 'transition' ? r.check.to : undefined);

/// guard 的復位條件行(evidence[0] 首行起向上至多 2 行,`if (<when>)` 形)。
function polLine(lines, rule) {
  const ev = rule.evidence?.[0];
  const when = rule.check.when;
  if (!ev || !when) return { ambiguous: '無 evidence/when' };
  for (let n = ev.line; n >= Math.max(1, ev.line - 2); n--) {
    const code = codeOf(lines[n - 1] ?? '');
    const m = code.match(/\bif\s*\(/);
    if (!m) continue;
    const open = m.index + m[0].length - 1;
    const close = matchParen(code, open);
    if (close < 0) continue;
    const cond = code.slice(open + 1, close);
    if (norm(cond) === norm(when)) return { line: n, open, close, cond };
  }
  return { ambiguous: `evidence[0] 首行附近無 if (${when})` };
}

function opPol(lines, rule) {
  const p = polLine(lines, rule);
  if (p.ambiguous) return { status: 'ambiguous', reason: p.ambiguous };
  const line = lines[p.line - 1];
  const c = p.cond.trim();
  const flipped = c.startsWith('!') ? c.replace(/^!\s*/, '') : `!${c}`;
  const after = replaceAt(line, p.open + 1, p.close, flipped);
  return { status: 'ok', line: p.line, after, detail: `if (${c})→if (${flipped})` };
}

function opAsync(lines, rule) {
  if (!rule.check.async) return { status: 'n/a', reason: 'async=false' };
  const sens = rule.evidence?.[1];
  const when = rule.check.when ?? '';
  const sig = when.replace(/^!\s*/, '');
  if (!sens || !/^[A-Za-z_][A-Za-z0-9_$]*$/.test(sig)) return { status: 'ambiguous', reason: '無敏感列表證或復位訊號非單標識符' };
  const line = lines[sens.line - 1] ?? '';
  const e = `(?:posedge|negedge)\\s+${sig}\\b`;
  // 四形(R1-6):` or <e>`/`, <e>`/`<e> or `/`<e>, `。
  for (const re of [new RegExp(`\\s+or\\s+${e}`, 'g'), new RegExp(`\\s*,\\s*${e}`, 'g'), new RegExp(`\\b${e}\\s+or\\s+`, 'g'), new RegExp(`\\b${e}\\s*,\\s*`, 'g')]) {
    const hits = [...line.matchAll(re)];
    if (hits.length === 1) {
      const h = hits[0];
      return { status: 'ok', line: sens.line, after: replaceAt(line, h.index, h.index + h[0].length, ''), detail: `刪 ${h[0].trim()}` };
    }
    if (hits.length > 1) return { status: 'ambiguous', reason: '復位沿項行內計數≠1' };
  }
  return { status: 'ambiguous', reason: `敏感列表行無 ${sig} 沿項` };
}

function opRval(lines, rule, effectIdx) {
  const eff = rule.check.effects?.[effectIdx];
  const a = anchorLine(lines, rule.evidence?.[0], eff.lhs);
  if (a.ambiguous) return { status: 'ambiguous', reason: a.ambiguous };
  const line = lines[a.line - 1];
  const pa = parseAssign(line);
  const lit = pa ? singleLiteral(pa.rhs) : null;
  if (!lit) return { status: 'n/a', reason: '源碼 RHS 非單一字面量', line: a.line };
  if (!lit.next) return { status: 'n/a', reason: lit.xz ? 'x/z 值(WAIVE x-valued)' : '字面量不可加一', line: a.line };
  const rhsAbs = line.indexOf(pa.rhs, pa.rhsStart);
  return { status: 'ok', line: a.line, after: replaceAt(line, rhsAbs, rhsAbs + pa.rhs.length, lit.next), detail: `${pa.rhs}→${lit.next}` };
}

/// OP-STATE 枚舉表(R1-5):同模組同 state_reg 全部 transition 規則 evidence 區間內,`^\s*<reg>\s*<=\s*(RHS);\s*$`
/// 行的 RHS 原文,按行號升序首次出現去重=有序表 C;每行 L 的替換值=C 中 r 的下一個(循環,跳過同值)。
function stateTable(lines, rules) {
  const tables = new Map();
  for (const r of rules) {
    if (r.check.form !== 'transition') continue;
    const reg = r.check.state_reg;
    const ev = r.evidence?.[0];
    if (!ev) continue;
    const re = new RegExp(`^\\s*${reg.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\s*<=\\s*([^;]+);\\s*$`);
    if (!tables.has(reg)) tables.set(reg, new Map());
    const t = tables.get(reg);
    for (let n = ev.line; n <= ev.end_line; n++) {
      const m = codeOf(lines[n - 1] ?? '').match(re);
      if (m && !t.has(n)) t.set(n, m[1].trim());
    }
  }
  const out = new Map();
  for (const [reg, byLine] of tables) {
    const order = [...byLine.entries()].sort((a, b) => a[0] - b[0]);
    const C = [];
    for (const [, rhs] of order) if (!C.includes(rhs)) C.push(rhs);
    out.set(reg, { C, byLine: new Map(order) });
  }
  return out;
}

/// 單規則×單算子嘗試(歸屬計算與生成共用)。
function attempt(lines, rule, op, states) {
  const form = rule.check.form;
  if (op === 'OP-POL') return opPol(lines, rule);
  if (op === 'OP-ASYNC') return opAsync(lines, rule);
  if (op === 'OP-RVAL') {
    // guard 每個 effect 一錨:取首個可生成者(其餘 effect 經歸屬計入)。
    let last = { status: 'n/a', reason: '無 effect' };
    for (let k = 0; k < (rule.check.effects ?? []).length; k++) {
      const r = opRval(lines, rule, k);
      if (r.status === 'ok') return r;
      last = r;
    }
    return last;
  }
  const subj = subjectsOf(rule)[0];
  if (!subj) return { status: 'n/a', reason: `${form} 無主體` };
  const a = mainAnchor(lines, rule, subj);
  if (a.ambiguous) return { status: 'ambiguous', reason: a.ambiguous };
  const r = mutateAssignLine(lines[a.line - 1], op);
  return { ...r, line: a.line };
}

/// 規則序全部變異(P-rule+控制組+P-unruled)。rules=規則書該模組規則(序);srcText=DUT 原文。
export function planMutations(srcText, rules) {
  const lines = srcText.split('\n');
  const states = stateTable(lines, rules);
  const byKey = new Map();
  const add = (pool, op, line, after, detail) => {
    const key = `${line}|${after}`;
    if (!byKey.has(key)) byKey.set(key, { pool, op, line, before: lines[line - 1], after, detail, ruleIds: [] });
    return byKey.get(key);
  };
  // OP-STATE=行驅動(R1-5):每個 state_reg 枚舉表 C 的每一行 L 各一個變異(|C|<2=n/a);歸屬=
  // `to` 規範相等的**唯一** transition 規則,否則=區間含 L 的全部 transition 規則(attribution:"span")。
  const stateMut = [];
  for (const [reg, t] of states) {
    if (t.C.length < 2) continue;
    for (const [L, r] of t.byLine) {
      const i = t.C.indexOf(r);
      let j = (i + 1) % t.C.length;
      while (t.C[j] === r) j = (j + 1) % t.C.length;
      const line = lines[L - 1];
      const pa = parseAssign(line);
      if (!pa) continue;
      const rhsAbs = line.indexOf(pa.rhs, pa.rhsStart);
      const m = add('rule', 'OP-STATE', L, replaceAt(line, rhsAbs, rhsAbs + pa.rhs.length, t.C[j]), `${r}→${t.C[j]}`);
      const byTo = rules.filter((x) => x.check.form === 'transition' && x.check.state_reg === reg && norm(x.check.to) === norm(r));
      if (byTo.length === 1) {
        m.ruleIds = [byTo[0].id];
      } else {
        m.ruleIds = rules
          .filter((x) => x.check.form === 'transition' && x.check.state_reg === reg && x.evidence?.[0] && L >= x.evidence[0].line && L <= x.evidence[0].end_line)
          .map((x) => x.id);
        m.attribution = 'span';
      }
      stateMut.push(m);
    }
  }
  const attempts = [];
  for (const r of rules) {
    const pri = PRIORITY[r.check.form] ?? [];
    let made = 0;
    for (const op of pri) {
      if (made >= PER_RULE_BUDGET) break;
      if (op === 'OP-STATE') {
        const hit = stateMut.some((m) => m.ruleIds.includes(r.id));
        const t = states.get(r.check.state_reg);
        attempts.push({
          ruleId: r.id,
          op,
          status: hit ? 'ok' : t && t.C.length < 2 ? 'n/a' : 'ambiguous',
          reason: hit ? null : t && t.C.length < 2 ? '|C|<2' : '無歸屬 OP-STATE 行(區間內無 ^reg <= RHS;$ 行)',
          line: null,
        });
        if (hit) made++;
        continue;
      }
      const a = attempt(lines, r, op, states);
      attempts.push({ ruleId: r.id, op, status: a.status, reason: a.reason ?? null, line: a.line ?? null });
      if (a.status === 'ok') {
        add('rule', op, a.line, a.after, a.detail);
        made++;
      }
    }
  }
  // 歸屬集(RV2-5):op 對其 form 適用且錨定行==變異行的全部規則(規則序)。OP-STATE 按 to 歸屬,
  // 無唯一匹配→區間歸屬(attribution:"span")。
  const anchorCache = new Map();
  const anchorsFor = (r, op) => {
    const k = `${r.id}|${op}`;
    if (!anchorCache.has(k)) {
      let ls = [];
      if (op === 'OP-POL') {
        const p = polLine(lines, r);
        if (!p.ambiguous) ls = [p.line];
      } else if (op === 'OP-ASYNC') {
        if (r.check.async && r.evidence?.[1]) ls = [r.evidence[1].line];
      } else if (op === 'OP-RVAL') {
        for (const e of r.check.effects ?? []) {
          const a = anchorLine(lines, r.evidence?.[0], e.lhs);
          if (!a.ambiguous) ls.push(a.line);
        }
      } else {
        const subj = subjectsOf(r)[0];
        const a = subj ? mainAnchor(lines, r, subj) : { ambiguous: 1 };
        if (!a.ambiguous) ls = [a.line];
      }
      anchorCache.set(k, ls);
    }
    return anchorCache.get(k);
  };
  for (const m of byKey.values()) {
    if (m.op === 'OP-STATE') continue; // 行驅動段已定歸屬
    m.ruleIds = rules.filter((r) => (PRIORITY[r.check.form] ?? []).includes(m.op) && anchorsFor(r, m.op).includes(m.line)).map((r) => r.id);
  }
  // 控制組 OP-NOP:每個在場 form 的首條規則 evidence[0] 首行(≤4;「任一證據行」的確定性取法)。
  const controls = [];
  const seenForm = new Set();
  for (const r of rules) {
    const f = r.check.form;
    const ev = r.evidence?.[0];
    if (seenForm.has(f) || !ev) continue;
    seenForm.add(f);
    const line = ev.line;
    if (controls.some((c) => c.line === line)) continue;
    controls.push({ pool: 'control', op: 'OP-NOP', line, before: lines[line - 1], after: lines[line - 1] + NOP_TAG, detail: 'append nop', ruleIds: [r.id] });
  }
  // P-unruled:含賦值、不在任何規則任何 evidence 區間內、非參數/迴圈頭的行。
  const covered = new Set();
  for (const r of rules) for (const e of r.evidence ?? []) for (let n = e.line; n <= e.end_line; n++) covered.add(n);
  const unruled = [];
  lines.forEach((l, i) => {
    const n = i + 1;
    if (covered.has(n)) return;
    const code = codeOf(l).trim();
    if (code === '' || /^(parameter|localparam|input|output|inout|integer|genvar|function|task|for\b)/.test(code) || /\bfor\s*\(/.test(code)) return;
    if (!parseAssign(l)) return;
    let made = 0;
    for (const op of UNRULED_PRIORITY) {
      if (made >= PER_RULE_BUDGET) break;
      const r = mutateAssignLine(l, op);
      if (r.status === 'ok') {
        unruled.push({ pool: 'unruled', op, line: n, before: l, after: r.after, detail: r.detail, ruleIds: [] });
        made++;
      }
    }
  });
  const all = [...byKey.values(), ...controls, ...unruled];
  all.forEach((m, k) => {
    m.id = `m${String(k + 1).padStart(4, '0')}`;
    m.key = `${m.line}|${m.after}`;
  });
  return { mutants: all, attempts, stateTables: Object.fromEntries([...states].map(([k, v]) => [k, v.C])) };
}

export function applyMutant(srcText, m) {
  const lines = srcText.split('\n');
  if (lines[m.line - 1] !== m.before) throw new Error(`mutant ${m.id} 錨定行內容漂移(line ${m.line})`);
  lines[m.line - 1] = m.after;
  return lines.join('\n');
}

/// 錨定統計(規格實測數對照:按主體定行六設計全部唯一)。
export function anchorStats(srcText, rules) {
  const lines = srcText.split('\n');
  const out = {};
  for (const r of rules) {
    const f = r.check.form;
    out[f] ??= { total: 0, unique: 0 };
    for (const s of subjectsOf(r)) {
      out[f].total++;
      const a = f === 'guard' ? anchorLine(lines, r.evidence?.[0], s.subject) : mainAnchor(lines, r, s);
      if (!a.ambiguous) out[f].unique++;
    }
  }
  return out;
}
