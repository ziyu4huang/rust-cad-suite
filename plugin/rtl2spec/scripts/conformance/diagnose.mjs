// diagnose.mjs — counterexample→結構化診斷(§19.6.1;D-C9;C-S5)。純函數,零 IO。
//
// 輸入:期望/實際正規化輸出、tb_plan(plan)、規則(該模組)、tb.v 的 COVERS 註釋、殺傷矩陣
// (record.mutants 形)、被測產物源碼(提 // ASSUMPTION: 行)。輸出=診斷 JSON v1(§19.6.1)。
//
// suspects 排序(確定性):①direct=規則主體恰為差異輸出端口;②covers=失配場景 COVERS 的規則
// (`*`=非定向,不折算);③mutant-sensitive=殺傷矩陣中「在失配場景集上殺死該規則」的變異計數
// (**留一法**:排除當前變異自身,caller 傳 excludeKey);同級按規則 ID 序。
// 分類:協議違例→tb-protocol;ASSUMPTION 提及 undefinedSignals 名、或 suspects 空、或
// ≥2 獨立樣本同場景同 firstDiff(由 caller 傳样本數)→rulebook-gap-candidate;其餘→rewrite-error。

import { parseProtocol, diffScenes } from '../lib/tbproto.mjs';

const sceneName = (s) => `SCENE${s.n} ${s.name}`;

/// 差異輸出端口集:失配 kv 行的 key 中屬於 plan.outputs 者(下標取基名)。
function differingPorts(aScene, bScene, outputs) {
  const parse = (lines) => {
    const m = new Map();
    for (const l of lines) {
      for (const item of l.trim().split(' ')) {
        const eq = item.indexOf('=');
        if (eq > 0) m.set(item.slice(0, eq), item.slice(eq + 1));
      }
    }
    return m;
  };
  const av = parse(aScene?.lines ?? []);
  const bv = parse(bScene?.lines ?? []);
  const keys = new Set([...av.keys(), ...bv.keys()]);
  const out = [];
  for (const k of keys) {
    if (av.get(k) !== bv.get(k)) {
      const base = k.split('[')[0];
      if (outputs.includes(base)) out.push(base);
    }
  }
  return [...new Set(out)].sort();
}

/// 逐失配場景的 firstDiff(contextBefore ≤3 行)。
function firstDiffs(expectedNorm, actualNorm) {
  const a = parseProtocol(expectedNorm);
  const b = parseProtocol(actualNorm);
  const failing = [];
  for (const s of a.scenes) {
    const bs = b.scenes.find((x) => x.n === s.n && x.name === s.name);
    if (!bs) {
      failing.push({ scene: sceneName(s), missing: true });
      continue;
    }
    const n = Math.max(s.lines.length, bs.lines.length);
    for (let k = 0; k < n; k++) {
      if (s.lines[k] !== bs.lines[k]) {
        failing.push({
          scene: sceneName(s),
          firstDiff: {
            lineNo: k + 1,
            expected: s.lines[k] ?? '<absent>',
            actual: bs.lines[k] ?? '<absent>',
          },
          keysDiffering: differingPorts(s, bs, []),
          contextBefore: s.lines.slice(Math.max(0, k - 3), k),
        });
        break;
      }
    }
  }
  const tailA = `${a.doneCount}|${a.doneLast}|${a.watchdog}|${a.timeouts.length}`;
  const tailB = `${b.doneCount}|${b.doneLast}|${b.watchdog}|${b.timeouts.length}`;
  if (tailA !== tailB) failing.push({ scene: '<tail>', firstDiff: { lineNo: 0, expected: tailA, actual: tailB }, keysDiffering: [], contextBefore: [] });
  return failing;
}

/// 主入口。args={expectedNorm, actualNorm, plan, rules, covers:[{n,ids}], mutants(殺傷矩陣
/// record.mutants 形), excludeKey(留一法:排除該變異鍵), assumptionSrc(被測產物源碼;可 null),
/// sampleCount(同場景同 firstDiff 的獨立樣本數;留一法=1)}。
export function diagnose({ expectedNorm, actualNorm, plan, rules, covers, mutants, excludeKey = null, assumptionSrc = null, sampleCount = 1 }) {
  const failing = firstDiffs(expectedNorm, actualNorm);
  const actP = parseProtocol(actualNorm);
  const ruleById = new Map(rules.map((r) => [r.id, r]));
  // 失配場景名集(S)。
  const failScenes = new Set(failing.map((f) => f.scene));
  const failSceneNums = new Set(failing.map((f) => Number((f.scene.match(/^SCENE(\d+)/) ?? [])[1] ?? -1)));
  // 差異輸出端口(全 failing 聯集)。
  const aP = parseProtocol(expectedNorm);
  const ports = [];
  for (const f of failing) {
    const s = aP.scenes.find((x) => sceneName(x) === f.scene);
    const bs = actP.scenes.find((x) => sceneName(x) === f.scene);
    if (s && bs) ports.push(...differingPorts(s, bs, plan.outputs));
  }
  const diffPorts = [...new Set(ports)];
  // ②covers:失配場景號的 COVERS 規則(`*` 不折算)。
  const covered = new Set();
  for (const c of covers ?? []) {
    if (!failSceneNums.has(c.n)) continue;
    for (const id of c.ids) if (id !== '*') covered.add(id);
  }
  // ③mutant-sensitive(留一法):失配場景集上殺死該規則的變異計數(排除自身)。
  const sens = new Map();
  for (const m of mutants ?? []) {
    if (excludeKey !== null && m.key === excludeKey) continue;
    if (!(m.result === 'killed' || m.result === 'timeout')) continue;
    const hit = (m.killScenes ?? []).some((s) => failScenes.has(s));
    if (!hit) continue;
    for (const id of m.ruleIds ?? []) sens.set(id, (sens.get(id) ?? 0) + 1);
  }
  // 主體基名。
  const subjects = (r) => {
    const c = r.check;
    if (c.form === 'guard') return (c.effects ?? []).map((e) => e.lhs.split('[')[0]);
    if (c.form === 'sequence') return [c.signal.split('[')[0]];
    if (c.form === 'relation') return [c.lhs.split('[')[0]];
    if (c.form === 'transition') return [c.state_reg];
    return [];
  };
  const ranked = [];
  for (const r of rules) {
    let why = null;
    let rank = null;
    if (subjects(r).some((s) => diffPorts.includes(s))) {
      rank = 1;
      why = `direct:主體∈差異輸出端口(${diffPorts.join(',')})`;
    } else if (covered.has(r.id)) {
      rank = 2;
      why = 'covers:失配場景 COVERS 的規則';
    } else if ((sens.get(r.id) ?? 0) > 0) {
      rank = 3;
      why = `mutant-sensitive:失配場景上殺死該規則的變異 ×${sens.get(r.id)}(留一法後)`;
    }
    if (rank !== null) ranked.push({ ruleId: r.id, statement: r.statement, check: r.check, why, rank });
  }
  ranked.sort((x, y) => x.rank - y.rank || (x.ruleId < y.ruleId ? -1 : x.ruleId > y.ruleId ? 1 : 0));
  // undefinedSignals:實際輸出含 x/z 的 key。
  const undefinedSignals = [...new Set(actP.xz.map((x) => x.split(' ')[1].split('=')[0]))].map((name) => ({ name, class: 'xz-valued' }));
  const assumptions = assumptionSrc
    ? (assumptionSrc.match(/^\/\/ ASSUMPTION:.*$/gm) ?? []).map((l) => l.trim())
    : [];
  let classification;
  if (actP.violations.length > 0) classification = 'tb-protocol';
  else if (
    (assumptions.length > 0 && undefinedSignals.some((u) => assumptions.some((a) => a.includes(u.name)))) ||
    ranked.length === 0 ||
    (sampleCount >= 2 && failing.length > 0 && failing.every((f) => f.firstDiff && f.scene !== '<tail>'))
  ) {
    classification = 'rulebook-gap-candidate';
  } else {
    classification = 'rewrite-error';
  }
  return {
    failing,
    suspects: ranked.slice(0, 20).map((x, k) => ({ ...x, order: k + 1 })),
    undefinedSignals,
    assumptions,
    classification,
    diffPorts,
  };
}

/// 留一法自驗(§19.6.4):killed 變異=真值已知的缺陷實現;top-3 命中率=真值規則集(變異歸屬集)
/// 與 suspects 前 3 相交。回傳 {total, hit, rate, per:[{key, ruleIds, top3:[...], hit}]}。
export function leaveOneOut({ expectedNorm, outputsByKey, mutants, plan, rules, covers }) {
  const per = [];
  for (const m of mutants) {
    const actual = outputsByKey.get(m.key);
    if (!actual) continue;
    const d = diagnose({
      expectedNorm,
      actualNorm: actual,
      plan,
      rules,
      covers,
      mutants,
      excludeKey: m.key,
      sampleCount: 1,
    });
    const top3 = d.suspects.slice(0, 3).map((s) => s.ruleId);
    const truth = m.ruleIds ?? [];
    const hit = truth.some((id) => top3.includes(id));
    per.push({ key: m.key, op: m.op, line: m.line, ruleIds: truth, top3, hit });
  }
  const total = per.length;
  const hitN = per.filter((p) => p.hit).length;
  return { total, hit: hitN, rate: `${hitN}/${total}`, per };
}

export { diffScenes };
