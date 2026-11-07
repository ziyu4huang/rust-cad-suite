// equiv.mjs — 等價裁決腿(測試側 harness;RTL2SPEC-FINISH-PLAN §3;CONFORMANCE-PROTOCOL §7)純函數層,零 IO。
// yosys oracle 僅測試用禁 ship;呼叫只在 --record 模式(conformance-eval.mjs 守衛),默認模式永不硬依賴該二進制。
//
// 雙向措辭(eqMeta/證據頁/§20 一律如實):proven≠無條件行為等價(弱等價 -seq 5+flatten/async2sync/memory_map
// 抽象),unproven≠不等價。eqverdict 三值:equivalent|unproven|adjudication-failed;現行 adjudication 兩值欄位保留
// (equivalent|unadjudicated),前者⇔eqverdict==equivalent。
//
// 守衛口徑(R3 訂正):proven 且 `select -count t:$equiv` 的 cells ≥ 輸出端口位數合計才算 equivalent;全端口位數
// 口徑會把組合設計誤判(signedexpr gg cells=18、全端口 23、輸出 18)。numbersOf 把 equivalent 併入 controlSet
// 恒真,Q8 接不住空真——本守衛是唯一防線。解析一律行形錨定正則,行形由 conformance-eval selfcheck 釘死。

/// 生產命令形(強形;無 blacklist 無 stash;flatten 必須在列;mut=top-only)。
export const EQ_COMMAND_TEMPLATE =
  'read_verilog <src>; rename <top> gold; read_verilog <mut-top-only>; rename <top> gate; proc; flatten; memory; async2sync; opt_clean; equiv_make gold gate equiv; hierarchy -top equiv; select -count t:$equiv; equiv_simple -seq 5; equiv_induct -seq 5; equiv_status';
/// 端口表(每設計一次;輸出位數=空真守衛錨)。
export const EQ_PORTS_TEMPLATE = 'read_verilog <src>; hierarchy -top <top>; proc; write_json <ports.json>';
/// 探針專用模板(唯一用途=製造空真,不屬生產管線):全名單 blacklist 構造對,必判 adjudication-failed(empty)。
export const EQ_EMPTY_NAMES_TEMPLATE = 'read_verilog <src>; hierarchy -top <top>; proc; flatten; write_json <names.json>';
export const EQ_EMPTY_PROBE_TEMPLATE =
  'read_verilog <src>; rename <top> gold; read_verilog <mut-top-only>; rename <top> gate; proc; flatten; memory; async2sync; opt_clean; equiv_make -blacklist <all-names.txt> gold gate equiv; hierarchy -top equiv; select -count t:$equiv; equiv_simple -seq 5; equiv_induct -seq 5; equiv_status';
export const EQ_PROVENANCE = '弱等價(-seq 5)+flatten/async2sync/memory_map 抽象';
export const EQ_VERDICTS = ['equivalent', 'unproven', 'adjudication-failed'];
export const EQ_REASONS = ['empty', 'no-sat-model', 'yosys-error', 'timeout', 'oracle-absent', 'no-count', 'count-mismatch', 'parse'];
/// 地面真值探針(斷言規格,手寫合法):正樣=OP-NOP 必 equivalent;反樣=golden killed 必非 equivalent。
export const EQ_GROUND_TRUTH = { fsm: { positive: 'm0045' }, i2c_master: { negative: 'm0001' } };
const UNPROVEN_NAMES_MAX = 32;

/// 模板實例化:每個占位必命中且不留殘餘(路徑一律相對 repo 根,record 零機器路徑,AGENTS 10.8)。
export function fillTemplate(tpl, vals) {
  let s = tpl;
  for (const [k, v] of Object.entries(vals)) {
    if (!s.includes(k)) throw new Error(`模板無占位 ${k}`);
    s = s.split(k).join(v);
  }
  if (/<[a-z-.]+>/.test(s)) throw new Error(`模板占位未盡: ${s}`);
  return s;
}

/// top-only 提取(行級狀態機,禁點星跨塊):module <top> 行起至其首個 endmodule 行止。塊註釋內與 `//` 行不計;
/// 定義數≠1 或無 endmodule 一律 throw(呼叫端記 adjudication-failed,不靜默)。
export function extractTopOnly(src, top) {
  const head = new RegExp(`^\\s*module\\s+${top.replace(/[$]/g, '\\$')}(?![A-Za-z0-9_$])`);
  const lines = src.split('\n');
  const code = [];
  let inBlock = false;
  for (const raw of lines) {
    let out = '';
    let k = 0;
    while (k < raw.length) {
      if (inBlock) {
        const e = raw.indexOf('*/', k);
        if (e < 0) {
          k = raw.length;
        } else {
          inBlock = false;
          k = e + 2;
        }
      } else {
        const s = raw.indexOf('/*', k);
        const c = raw.indexOf('//', k);
        if (c >= 0 && (s < 0 || c < s)) {
          out += raw.slice(k, c);
          k = raw.length;
        } else if (s >= 0) {
          out += raw.slice(k, s);
          inBlock = true;
          k = s + 2;
        } else {
          out += raw.slice(k);
          k = raw.length;
        }
      }
    }
    code.push(out);
  }
  const starts = code.flatMap((l, i) => (head.test(l) ? [i] : []));
  if (starts.length !== 1) throw new Error(`top-only: module ${top} 定義數=${starts.length}≠1`);
  const end = code.findIndex((l, i) => i > starts[0] && /^\s*endmodule\b/.test(l));
  if (end < 0) throw new Error(`top-only: module ${top} 無 endmodule`);
  return lines.slice(starts[0], end + 1).join('\n') + '\n';
}

/// write_json 的端口位數表(all/out;out=direction==='output' 的 bits 合計)。
export function portBits(json, top) {
  const m = json?.modules?.[top];
  if (!m?.ports) throw new Error(`ports: write_json 無模組 ${top}`);
  let all = 0;
  let out = 0;
  for (const p of Object.values(m.ports)) {
    all += p.bits.length;
    if (p.direction === 'output') out += p.bits.length;
  }
  return { all, out };
}

/// 空真探針全名單:flatten 後全部公開網名(`$` 起首=內部)。
export function publicNames(json, top) {
  const m = json?.modules?.[top];
  if (!m?.netnames) throw new Error(`names: write_json 無模組 ${top}`);
  return Object.keys(m.netnames).filter((n) => !n.startsWith('$')).sort();
}

// 真實行形(yosys 0.70+179 實跑;敵意面=equiv_simple/induct 的相近行不得誤中):
const RE_OBJECTS = /^([0-9]+) objects\.$/gm; // select -count
const RE_FOUND = /^Found ([0-9]+) \$equiv cells in [^ ]+:$/m; // equiv_status;不中「Found 40 unproven $equiv cells (6 groups) in equiv:」
const RE_OF = /^\s*Of those cells ([0-9]+) are proven and ([0-9]+) are unproven\.$/m;
const RE_SUCCESS = /^\s*Equivalence successfully proven!$/m;
const RE_NOCELLS = /^No \$equiv cells found in [^ ]+\.$/m; // 不中「No selected unproven $equiv cells found in equiv.」
const RE_UNPROVEN = /^\s*Unproven \$equiv [^ ]+: (\S+)(?: \[([0-9:]+)\])? /gm;
const RE_TOTAL = /^Found a total of ([0-9]+) unproven \$equiv cells\.$/m;
const RE_NOSAT = /No SAT model available/;
const RE_ERROR = /^ERROR: (.*)$/m;

/// 由單次 yosys 運行判 eqverdict。r={code,stdout,stderr,timedOut,spawnError};outBits=輸出端口位數合計。
/// 回傳 {eqverdict,eqCells,eqReason?,eqDetail?,eqUnproven?,unprovenCells?}。
export function eqVerdictOf(r, outBits) {
  const failed = (eqReason, eqCells = null, eqDetail) => ({ eqverdict: 'adjudication-failed', eqCells, eqReason, ...(eqDetail ? { eqDetail } : {}) });
  if (r.spawnError) return failed('oracle-absent', null);
  if (r.timedOut) return failed('timeout', null);
  const text = `${r.stdout ?? ''}\n${r.stderr ?? ''}`;
  const objects = [...(r.stdout ?? '').matchAll(RE_OBJECTS)].map((m) => Number(m[1]));
  const cells = objects.length === 1 ? objects[0] : null;
  if (r.code !== 0) {
    if (RE_NOSAT.test(text)) return failed('no-sat-model', cells);
    const e = text.match(RE_ERROR);
    return failed('yosys-error', cells, e ? e[1].slice(0, 160) : `exit ${r.code}`);
  }
  if (objects.length !== 1) return failed('no-count', null);
  if (RE_NOCELLS.test(r.stdout)) return failed('empty', cells);
  const found = r.stdout.match(RE_FOUND);
  const of = r.stdout.match(RE_OF);
  if (!found || !of) return failed('parse', cells);
  if (Number(found[1]) !== cells) return failed('count-mismatch', cells, `select ${cells}≠equiv_status ${found[1]}`);
  const nProven = Number(of[1]);
  const nUnproven = Number(of[2]);
  if (nProven + nUnproven !== cells) return failed('parse', cells);
  if (nUnproven === 0) {
    if (!RE_SUCCESS.test(r.stdout)) return failed('parse', cells);
    if (!(outBits >= 1) || cells < outBits) return failed('empty', cells, `cells ${cells}<輸出位數 ${outBits}`);
    return { eqverdict: 'equivalent', eqCells: cells };
  }
  const total = r.stdout.match(RE_TOTAL);
  const names = [...r.stdout.matchAll(RE_UNPROVEN)].map((m) => {
    const base = m[1].replace(/^\\/, '').replace(/_gold$/, '');
    return m[2] ? `${base}[${m[2]}]` : base;
  });
  if (!total || Number(total[1]) !== nUnproven || names.length !== nUnproven) return failed('parse', cells);
  const uniq = [...new Set(names)].sort();
  return { eqverdict: 'unproven', eqCells: cells, eqUnproven: nUnproven, unprovenCells: uniq.slice(0, UNPROVEN_NAMES_MAX) };
}

export const isKill = (m) => m?.result === 'killed' || m?.result === 'timeout';

/// 變異的裁決態:eqverdict 優先;舊 record/--try 無 eq 資料時由 adjudication 回推,仍無=unjudged(保守=非等價)。
export function verdictOfMutant(m) {
  if (m.eqverdict) return m.eqverdict;
  if (m.adjudication === 'equivalent') return 'equivalent';
  if (m.adjudication === 'unadjudicated') return 'unproven';
  return 'unjudged';
}

/// 數據支持的標籤:killed>0=observable;無存活變異=no-data;存活全 equivalent=unobservable;否則 unadjudicated。
/// killed 兩腿都算(任一腿殺=可觀測差異)。
export function waiveRows(entries, mainRs, baseRs) {
  const bk = new Map((baseRs ?? []).map((m) => [m.key, m]));
  return entries.map(({ id, tag }) => {
    const ms = mainRs.filter((m) => m.pool === 'rule' && m.ruleIds.includes(id));
    const killed = ms.filter((m) => isKill(m) || isKill(bk.get(m.key)));
    const surv = ms.filter((m) => m.result === 'survived' && !isKill(bk.get(m.key)));
    const dist = { equivalent: 0, unproven: 0, 'adjudication-failed': 0, unjudged: 0 };
    for (const m of surv) dist[verdictOfMutant(m)]++;
    const dataTag = killed.length > 0 ? 'observable' : surv.length === 0 ? 'no-data' : dist.equivalent === surv.length ? 'unobservable' : 'unadjudicated';
    return { ruleId: id, tag, mutants: ms.length, killed: killed.length, dist, dataTag };
  });
}

/// WAIVE↔record 交叉驗證:數據標籤(unobservable/unadjudicated)必==數據謂詞;靜態標籤(x-valued/tb-internal)
/// 不判、只可見化(record-diff 打印使用數變化量,新增靜態標籤須在切片記錄理由)。
export const DATA_LABELS = ['unobservable', 'unadjudicated'];
export function waiveCrossFindings(rows) {
  return rows
    .filter((r) => DATA_LABELS.includes(r.tag) && r.tag !== r.dataTag)
    .map((r) => `${r.ruleId} 標 ${r.tag},數據=${r.dataTag}(變異 ${r.mutants}/killed ${r.killed}/equivalent ${r.dist.equivalent}/unproven ${r.dist.unproven}/failed ${r.dist['adjudication-failed']}/未裁決 ${r.dist.unjudged})`);
}

/// 每標籤使用數(record-diff 可見化)。
export function tagCounts(entries) {
  const c = {};
  for (const { tag } of entries) c[tag] = (c[tag] ?? 0) + 1;
  return c;
}

/// 默認模式 eq-verify:eqMeta/eqverdict schema 完整性+counts 一致性(不跑 yosys)。回傳 findings。
export function eqSchemaFindings(rec) {
  const f = [];
  const M = rec.eqMeta;
  if (!M) return ['eqMeta 缺'];
  for (const k of ['yosysVersion', 'commandTemplate', 'portsTemplate', 'emptyProbeTemplate', 'provenance']) {
    if (typeof M[k] !== 'string' || M[k] === '') f.push(`eqMeta.${k} 缺`);
  }
  if (M.commandTemplate !== EQ_COMMAND_TEMPLATE) f.push('eqMeta.commandTemplate≠生產命令形');
  if (M.emptyProbeTemplate !== EQ_EMPTY_PROBE_TEMPLATE) f.push('eqMeta.emptyProbeTemplate≠探針模板');
  if (!(M.ports?.out >= 1) || !(M.ports?.all >= M.ports?.out)) f.push(`eqMeta.ports 不合法 ${JSON.stringify(M.ports)}`);
  if (M.goldVsGold?.eqverdict !== 'equivalent' || !(M.goldVsGold?.eqCells >= M.ports?.out)) f.push(`gold-vs-gold 非 equivalent 或 cells<輸出位數 ${JSON.stringify(M.goldVsGold)}`);
  if (M.emptyProbe?.eqverdict !== 'adjudication-failed' || M.emptyProbe?.eqReason !== 'empty') f.push(`空真探針非 adjudication-failed(empty) ${JSON.stringify(M.emptyProbe)}`);
  const truth = EQ_GROUND_TRUTH[rec.design] ?? {};
  if (truth.positive && (M.groundTruth?.positive?.id !== truth.positive || M.groundTruth.positive.eqverdict !== 'equivalent')) f.push(`正樣探針 ${truth.positive} 缺或非 equivalent`);
  if (truth.negative && (M.groundTruth?.negative?.id !== truth.negative || !['unproven', 'adjudication-failed'].includes(M.groundTruth.negative.eqverdict))) f.push(`反樣探針 ${truth.negative} 缺或被判 equivalent`);
  const P = (rec.mutants ?? []).filter((m) => m.pool === 'rule');
  const judged = P.filter((m) => m.adjudication);
  const n = { equivalent: 0, unproven: 0, 'adjudication-failed': 0 };
  for (const m of judged) {
    if (!EQ_VERDICTS.includes(m.eqverdict)) {
      f.push(`${m.id} eqverdict 缺或不合法(${m.eqverdict})`);
      continue;
    }
    n[m.eqverdict]++;
    if ((m.adjudication === 'equivalent') !== (m.eqverdict === 'equivalent')) f.push(`${m.id} adjudication=${m.adjudication} 與 eqverdict=${m.eqverdict} 矛盾`);
    if (m.eqverdict === 'equivalent') {
      if (m.result !== 'survived') f.push(`${m.id} equivalent 卻 result=${m.result}`);
      if (!(m.eqCells >= M.ports?.out)) f.push(`${m.id} equivalent 但 cells ${m.eqCells}<輸出位數 ${M.ports?.out}(空真)`);
    }
    if (m.eqverdict === 'unproven' && !(m.eqUnproven >= 1)) f.push(`${m.id} unproven 缺 eqUnproven`);
    if (m.eqverdict === 'adjudication-failed' && !EQ_REASONS.includes(m.eqReason)) f.push(`${m.id} adjudication-failed 缺 eqReason`);
  }
  for (const m of P) if (!m.adjudication && m.eqverdict) f.push(`${m.id} 無 adjudication 卻帶 eqverdict`);
  const c = M.counts ?? {};
  if (c.adjudicated !== judged.length) f.push(`counts.adjudicated ${c.adjudicated}≠${judged.length}`);
  if (c.equivalent !== n.equivalent) f.push(`counts.equivalent ${c.equivalent}≠${n.equivalent}`);
  if (c.unproven !== n.unproven) f.push(`counts.unproven ${c.unproven}≠${n.unproven}`);
  if (c.adjudicationFailed !== n['adjudication-failed']) f.push(`counts.adjudicationFailed ${c.adjudicationFailed}≠${n['adjudication-failed']}`);
  if (rec.numbers?.equivalent !== n.equivalent) f.push(`numbers.equivalent ${rec.numbers?.equivalent}≠eqverdict equivalent ${n.equivalent}`);
  const ks = M.killedSelfcheck ?? {};
  const bm = new Map((rec.baselineTemplate?.mutants ?? []).map((m) => [m.key, m]));
  const anyKilled = P.filter((m) => isKill(m) || isKill(bm.get(m.key))).length;
  if (ks.total !== anyKilled) f.push(`killedSelfcheck.total ${ks.total}≠任一腿 killed ${anyKilled}`);
  if (ks.equivalent !== 0) f.push(`killedSelfcheck.equivalent=${ks.equivalent}≠0`);
  if ((ks.unproven ?? 0) + (ks.adjudicationFailed ?? 0) + (ks.equivalent ?? 0) !== ks.total) f.push('killedSelfcheck 分項和≠total');
  return f;
}
