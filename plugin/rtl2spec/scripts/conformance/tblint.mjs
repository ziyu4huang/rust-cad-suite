// tblint.mjs — TB 靜態判據(§19.4.3 Q6/Q9;§19.3.3 TB-R1x;C-S3)。純函數,零 IO。
//
// Q6:contract 名實例化+全端口具名連接、零層次引用、零 $random/$urandom、看門狗在場、
//     TB-R1x 零違例(`// race-ok` 豁免)、TB 內部沿行程同檔 R1 零違例。
// Q9:COVERS/WAIVE 的規則 ID ⊆ 規則書;WAIVE 行文法與標籤集(WAIVE_LABELS);`// SCENES: N` 與 COVERS
//     場景號一致;D 腿另要求每條規則 COVERS 或 WAIVE(T 腿 COVERS 由 tb_plan 機械生成,非定向標 `*`)。
//
// 語句窗(復審 RV2-7 定案=corpus_lint 文檔語義,**非**其代碼行為):窗=事件控制右括號之後
// 的語句,止於首個 `#`、下一個 `@(` 或以 `end` 起首的行;**同行出現 `#` 即關窗,不再延伸**。
// 與 Rust corpus_lint.rs 的 parity 只聲明於其三個同行向量;換行形=已登記差異 D-c3。

/// WAIVE 標籤集(閘側唯一來源;CONFORMANCE-PROTOCOL 模板 v2 第 2 條枚舉與本表由 conformance-eval
/// selfcheck 逐集合核對)。unobservable=無 killed 且存活變異全部 equivalent(yosys 已證);
/// unadjudicated=無 killed 且存在未證等價的存活變異(中性,不帶可/不可觀測方向);
/// x-valued/tb-internal=靜態標籤。數據謂詞的交叉驗證落點=等價裁決腿(RTL2SPEC-FINISH-PLAN §3.5)。
export const WAIVE_LABELS = ['x-valued', 'unobservable', 'unadjudicated', 'tb-internal'];
const WAIVE_LINE = new RegExp(`^// WAIVE: (\\S+) (${WAIVE_LABELS.join('|')})$`);

const stripComments = (src) =>
  src
    .replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, ' '))
    .split('\n')
    .map((l) => {
      const i = l.indexOf('//');
      return i >= 0 ? l.slice(0, i) : l;
    })
    .join('\n');

const stripStrings = (l) => l.replace(/"(?:[^"\\]|\\.)*"/g, '""');

function idents(s) {
  return s.match(/[A-Za-z0-9_$]+/g) ?? [];
}

/// 語句窗內的阻塞寫目標(corpus_lint 同款判定:`=` 非 `<=`/`==`/`!=`/`>=` 位)。
function blockingTargets(windowText) {
  const out = [];
  for (const stmt of windowText.split(';')) {
    const s = stmt.trim();
    const eq = s.indexOf('=');
    if (eq < 0) continue;
    const before = s.slice(0, eq);
    const after = s[eq + 1] ?? ' ';
    const prev = before[before.length - 1] ?? ' ';
    if (after === '=' || '<>!='.includes(prev)) continue;
    const lhs = idents(before);
    if (lhs.length) out.push(lhs[lhs.length - 1]);
  }
  return out;
}

/// 事件控制枚舉:每個 `@(<edge> <net>)` 及其語句窗(文檔語義)。clean=剝註釋文本(窗內容
/// 用,同行數);raw=原文(`// race-ok` 豁免標記在註釋裡,只能從原文判)。
function eventWindows(clean, raw) {
  const lines = clean.split('\n');
  const rawLines = raw.split('\n');
  const out = [];
  const EV = /@\(\s*(posedge|negedge)\s+([A-Za-z_][A-Za-z0-9_$]*)\s*\)/g;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if ((rawLines[i] ?? '').includes('race-ok')) continue;
    if (line.trimStart().startsWith('always')) continue; // 行程頭不是 TB 驅動窗
    EV.lastIndex = 0;
    let m;
    while ((m = EV.exec(line))) {
      const tail = line.slice(m.index + m[0].length);
      const cuts = [tail.indexOf('#'), tail.indexOf('@(')].filter((x) => x >= 0);
      let win;
      if (cuts.length) {
        win = tail.slice(0, Math.min(...cuts)); // 同行 `#`/`@(` 即關窗
      } else {
        win = tail + '\n';
        for (let j = i + 1; j < lines.length; j++) {
          const lj = lines[j].trim();
          if (lj.includes('#') || lj.includes('@(') || lj.startsWith('end')) break;
          win += lj + '\n';
        }
      }
      out.push({ line: i + 1, edge: m[1], net: m[2], window: win });
    }
  }
  return out;
}

/// TB 內部沿行程讀取的標識符(corpus_lint edge_read_vars 同款近似)。
function edgeReadVars(src) {
  const set = new Set();
  let inBlock = false;
  for (const line of src.split('\n')) {
    const t = line.trimStart();
    if (t.startsWith('always')) inBlock = t.includes('posedge') || t.includes('negedge');
    else if (/^(initial|endmodule|task|function)\b/.test(t)) inBlock = false;
    if (inBlock) for (const id of idents(line)) set.add(id);
  }
  return set;
}

/// 實例化區段:`<module> <inst> (` 起至首個 `);` 止;回傳 {found, inst, conns:Map(port→net)}。
export function instantiation(src, module) {
  const lines = src.split('\n');
  const head = new RegExp(`^\\s*${module.replace(/[$]/g, '\\$')}\\s+([A-Za-z_][A-Za-z0-9_$]*)\\s*\\(\\s*$`);
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i].match(head);
    if (!m) continue;
    const conns = new Map();
    for (let j = i + 1; j < lines.length; j++) {
      if (/^\s*\);\s*$/.test(lines[j])) return { found: true, inst: m[1], conns, startLine: i + 1, endLine: j + 1 };
      const c = lines[j].match(/^\s*\.([A-Za-z_][A-Za-z0-9_$]*)\s*\(\s*([A-Za-z_][A-Za-z0-9_$]*)\s*\)\s*,?\s*$/);
      if (c) conns.set(c[1], c[2]);
    }
  }
  return { found: false, inst: null, conns: new Map() };
}

/// TB-R1x(跨檔、極性感知;§19.3.3):E=DUT 沿集合 {(edge, port)}(取自 tb_plan.sens_edges,閘側
/// 不解析 DUT 文本);M=實例化 net→port。違例⇔`@(<e> <net>)` 窗內阻塞寫 TB 變量 v,v 經 M 連到
/// DUT 輸入端口,且 (e, M(net)) ∈ E。
export function tbR1x(src, sensEdges, inputPorts, conns) {
  const clean = stripComments(src);
  const E = new Set(sensEdges.map(([e, s]) => `${e} ${s}`));
  const netToPort = new Map([...conns].map(([port, net]) => [net, port]));
  const ins = new Set(inputPorts);
  const findings = [];
  for (const w of eventWindows(clean, src)) {
    const evPort = netToPort.get(w.net);
    if (!evPort || !E.has(`${w.edge} ${evPort}`)) continue;
    for (const v of blockingTargets(w.window)) {
      const vPort = netToPort.get(v);
      if (vPort && ins.has(vPort)) {
        findings.push(`line ${w.line}: 阻塞寫 \`${v}\`(→DUT 輸入 ${vPort})於 @(${w.edge} ${w.net}) 窗內,(${w.edge}, ${evPort})∈DUT 沿集合=同沿競爭`);
      }
    }
  }
  return findings;
}

/// 同檔 R1(TB 內部沿行程;corpus_lint 規則文本,文檔語義語句窗)。
export function r1SameFile(src) {
  const clean = stripComments(src);
  const vars = edgeReadVars(clean);
  const findings = [];
  for (const w of eventWindows(clean, src)) {
    for (const v of blockingTargets(w.window)) {
      if (vars.has(v)) findings.push(`line ${w.line}: 阻塞寫 \`${v}\` 於 @(${w.edge} ${w.net}) 窗內,被同檔沿行程讀取`);
    }
  }
  return findings;
}

/// WAIVE 行解析(序=源碼序):entries=[{id,tag}];invalid=文法外/標籤集外原行(lintTb 轉 Q9 硬紅)。
export function waiveEntriesOf(src) {
  const entries = [];
  const invalid = [];
  for (const [l] of src.matchAll(/^\/\/ WAIVE:.*$/gm)) {
    const m = l.match(WAIVE_LINE);
    if (m) entries.push({ id: m[1], tag: m[2] });
    else invalid.push(l);
  }
  return { entries, invalid };
}

/// Q6+Q9 全量 lint。plan=tb_plan JSON;ruleIds=規則書全 ID(序);leg=template|directed。
export function lintTb(src, { plan, ruleIds, leg = 'template' }) {
  const q6 = [];
  const q9 = [];
  const clean = stripComments(src);
  const inst = instantiation(clean, plan.module);
  if (!inst.found) q6.push(`未以 contract 模組名 \`${plan.module}\` 實例化`);
  for (const p of plan.ports) {
    if (inst.found && !inst.conns.has(p.name)) q6.push(`端口 \`${p.name}\` 未具名連接`);
  }
  if (inst.found) {
    for (const port of inst.conns.keys()) {
      if (!plan.ports.some((p) => p.name === port)) q6.push(`連接了 contract 外端口 \`${port}\``);
    }
  }
  if (/\$(random|urandom)\b/.test(clean)) q6.push('使用 $random/$urandom(跨模擬器序列不保證一致)');
  // 層次引用:剝字串後 `ident.ident` 形(實例化連接 `.port(` 行除外)。
  clean.split('\n').forEach((l, k) => {
    if (inst.found && k + 1 > inst.startLine && k + 1 < inst.endLine) return;
    const hit = stripStrings(l).match(/\b[A-Za-z_][A-Za-z0-9_$]*\.[A-Za-z_][A-Za-z0-9_$]*/);
    if (hit) q6.push(`line ${k + 1}: 層次引用 \`${hit[0]}\``);
  });
  if (!/\$display\("WATCHDOG"\);\s*\n?\s*\$finish;/.test(clean)) q6.push('缺看門狗 `$display("WATCHDOG"); $finish;`');
  const inputs = plan.ports.filter((p) => p.dir === 'input').map((p) => p.name);
  for (const f of tbR1x(src, plan.sens_edges, inputs, inst.conns)) q6.push(`TB-R1x ${f}`);
  for (const f of r1SameFile(src)) q6.push(`R1 ${f}`);

  // Q9 覆蓋聲明。
  // D 腿場景宣告可用 `// SCENE LIST: <name>[,<name>…]`(顯式枚舉,作者維護);
  // 數字 N 取其長度。`// SCENES: N` 仍可選(兩者都缺=缺宣告)。
  const list = src.match(/^\/\/ SCENE LIST: ([a-z0-9_, ]+)$/m);
  const declared = src.match(/^\/\/ SCENES: ([0-9]+)$/m);
  const covers = [...src.matchAll(/^\/\/ COVERS: ([0-9]+): (.+)$/gm)].map((m) => ({ n: Number(m[1]), ids: m[2].split(',').map((x) => x.trim()) }));
  // WAIVE 行=一行一規則+標籤∈WAIVE_LABELS;文法外/標籤集外一律硬紅(收尾 S0-C:舊正則靜默略過,
  // 規則另有 COVERS 時標籤錯字會無聲通過)。
  const { entries: waiveEntries, invalid: waiveInvalid } = waiveEntriesOf(src);
  const waives = waiveEntries.map((w) => w.id);
  for (const l of waiveInvalid) q9.push(`WAIVE 行文法外(一行一規則,標籤∈${WAIVE_LABELS.join('|')}):${l.slice(0, 80)}`);
  const known = new Set(ruleIds);
  let declN = declared ? Number(declared[1]) : null;
  if (list) {
    const names = list[1].split(',').map((x) => x.trim()).filter(Boolean);
    if (declN !== null && declN !== names.length) q9.push(`SCENE LIST 長度 ${names.length} 與 SCENES: ${declN} 不一致`);
    declN = names.length;
  }
  if (declN === null) q9.push('缺 `// SCENES: N` 或 `// SCENE LIST: …`');
  for (const c of covers) {
    if (declN !== null && (c.n < 1 || c.n > declN)) q9.push(`COVERS 場景號 ${c.n} 越界`);
    for (const id of c.ids) if (id !== '*' && !known.has(id)) q9.push(`COVERS 未知規則 ${id}`);
  }
  for (const id of waives) if (!known.has(id)) q9.push(`WAIVE 未知規則 ${id}`);
  const named = new Set(covers.flatMap((c) => c.ids).filter((x) => x !== '*'));
  if (leg === 'directed') {
    for (const id of ruleIds) if (!named.has(id) && !waives.includes(id)) q9.push(`D 腿規則 ${id} 既無 COVERS 亦無 WAIVE`);
  }
  return {
    q6,
    q9,
    declaredScenes: declN,
    coversNamed: [...named].filter((id) => known.has(id)).length,
    hasStar: covers.some((c) => c.ids.includes('*')),
    waived: waives.length,
    waiveEntries,
  };
}
