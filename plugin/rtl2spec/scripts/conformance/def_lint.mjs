// def_lint.mjs — 規則書定義完備性分類(§19.6.1;heuristic;閘側工具,RV1-14 定位聲明)。
// 只存在於 scripts/conformance/,只被 conformance-eval.mjs 調用,**不經任何面露出**(不進 wasm
// 導出、不進 MCP、不進 skills、不進規則書 JSON);結果只入 record.json 與缺陷報告。
//
// 口徑(§19.0-4 探針的正式化):定義完備性缺口=contract.signals∪輸出端口 中、未出現於任何規則
// 主體(lhs/signal/state_reg/effects.lhs 基名)的信號;對其「定義行形態」歸類:
//   U1 net 宣告賦值洞(`wire x = …;` 形;sv/mod.rs:686 `_ => {}` 臂只認 Always/Initial/ContAssign)
//   U2 直通別名(`assign x = y;` 且 RHS 為單一標識符=S2「直通別名不產」政策 → 標「政策排除」,不記缺陷)
//   U3 分支域外純常量(S2 政策 → 「政策排除」)
//   U4 Index/IndexedPart 非 canon(§18.4-3 待決:規則主體含非常量下標/Index 形被 canon 拒)
//   U5 其他(含內部聲明零規則引用=純存儲,不記缺陷但列明)
//
// 輸入:srcText、rulebook 該模組(mod)、contract。輸出:{items:[{signal,class,line,snippet,policy}], summary}。
// U2/U3 憑「未被規則覆蓋」與「定義行形態」可分:U2=連續 assign 鏈尾(單標識符 RHS);U3=常量賦值。
// 判「政策排除」後該項記 class 但 policy=true,缺陷清單只收 policy=false 的 U1/U4。

export function defLint(srcText, mod) {
  const lines = srcText.split('\n');
  const rules = mod.rules;
  const subjects = new Set();
  for (const r of rules) {
    const c = r.check;
    if (c.form === 'guard') for (const e of c.effects ?? []) subjects.add(e.lhs.split('[')[0]);
    else if (c.form === 'sequence') subjects.add(c.signal.split('[')[0]);
    else if (c.form === 'relation') subjects.add(c.lhs.split('[')[0]);
    else if (c.form === 'transition') subjects.add(c.state_reg);
    else if (c.form === 'invariant') subjects.add(c.subject.split('[')[0]);
  }
  const items = [];
  const covered = new Set([...(mod.contract.signals ?? []).map((s) => s.name), ...mod.contract.ports.filter((p) => p.dir === 'output').map((p) => p.name)]);
  for (const name of [...covered].sort()) {
    if (subjects.has(name)) continue;
    // 定義行:宣告行(assign/wire/reg 帶該名)。
    const defLine = lines.findIndex((l) => {
      const t = l.trim();
      if (t.startsWith('wire') || t.startsWith('reg')) {
        // `wire [...] name [= init];` 或 `reg [...] name;`
        return new RegExp(`(wire|reg)\\s+(signed\\s+)?(\\[[^\\]]*\\]\\s*)?${name}\\b\\s*(=|;|\\[)`).test(t);
      }
      return false;
    });
    const asgLine = lines.findIndex((l) => {
      const t = l.trim();
      if (!t.startsWith('assign')) return false;
      const m = t.match(/^assign\s+(?:[^=\s]+\s*=\s*)?([A-Za-z_][A-Za-z0-9_$]*(?:\s*\[[^\]]*\])*)\s*=\s*([^;]+);/);
      return m && m[1].split('[')[0].trim() === name;
    });
    let cls = 'U5';
    let policy = false;
    let line = defLine >= 0 ? defLine + 1 : asgLine >= 0 ? asgLine + 1 : 0;
    let snippet = line > 0 ? lines[line - 1].trim() : '';
    if (asgLine >= 0) {
      const rhs = lines[asgLine].match(/=\s*([^;]+);/)?.[1]?.trim() ?? '';
      if (/^[A-Za-z_][A-Za-z0-9_$]*$/.test(rhs)) {
        cls = 'U2';
        policy = true;
      } else if (/^[0-9']/.test(rhs)) {
        cls = 'U3';
        policy = true;
      } else {
        cls = 'U2';
        policy = true;
      }
    } else if (defLine >= 0) {
      // net 宣告帶 init → U1;無 init 的純宣告/內部存儲 → U5(列明不記缺陷)。
      if (/=/.test(lines[defLine])) {
        cls = 'U1';
      } else {
        cls = 'U5';
      }
    }
    items.push({ signal: name, class: cls, policy, line, snippet });
  }
  const defects = items.filter((i) => !i.policy && (i.class === 'U1' || i.class === 'U4'));
  return {
    items,
    summary: {
      uncovered: items.length,
      byClass: items.reduce((a, i) => ((a[i.class] = (a[i.class] ?? 0) + 1), a), {}),
      policyExcluded: items.filter((i) => i.policy).length,
      defects: defects.length,
    },
  };
}

/// P-unruled 賦值行形態二次標註(defects 報告用;與 mutate 的 UNRULED 池同一行集)。
export function classifyUnruled(line) {
  const t = line.trim();
  if (t.startsWith('wire') && /=/.test(t)) return 'U1';
  if (t.startsWith('assign')) {
    const rhs = t.match(/=\s*([^;]+);/)?.[1]?.trim() ?? '';
    if (/^[A-Za-z_][A-Za-z0-9_$]*$/.test(rhs)) return 'U2';
    return 'U2';
  }
  return 'U5';
}
