// markdown 渲染(與 crates/rtl-spec/src/md.rs 同形;展示層,非閘面——
// V1..V6 機械判據全在 Rust JSON 上,兩渲染器不設 byte-equal 閘)。
// server.mjs / check-rulebook.mjs / gen-report.mjs 共用本件(單一渲染器)。

export function sumRules(rulebook) {
  return (rulebook.modules ?? []).reduce((n, m) => n + (m.rules?.length ?? 0), 0);
}

export function countGaps(gapReport) {
  return (gapReport.items ?? []).length;
}

export function checkText(check) {
  const p = check;
  switch (p.form) {
    case 'guard': {
      const t = p.trigger ? `${p.trigger.signal ?? '?'} ${p.trigger.edge ?? '?'}` : '-';
      const eff = (p.effects ?? []).map((e) => `${e.lhs}←${e.value}`).join('; ');
      return `trigger=${t}; when=${p.when ?? '-'}; effects=${eff}`;
    }
    case 'transition':
      return `${p.state_reg}: ${p.from} → ${p.to} when ${p.when ?? '-'}`;
    case 'sequence':
      return `anchor(${p.anchor?.expr}) +${p.latency}拍 → ${p.signal}=${p.becomes} when ${p.when ?? '-'}`;
    case 'relation':
      // 適用域(R3-②):分支域恆等式攜 when;全域=null/-。
      return `${p.lhs} ≡ ${p.rhs} when ${p.when ?? '-'}${p.width_note ? ` (${p.width_note})` : ''}`;
    case 'invariant':
      return `${p.subject} ${p.kind} ${(p.values ?? []).length ? p.values.join(' | ') : (p.expr ?? '-')}`;
    default:
      return JSON.stringify(p);
  }
}

export function renderRulebookMarkdown(rb) {
  const lines = [];
  const total = sumRules(rb);
  lines.push(`# 規則書:${rb.source}`);
  lines.push('');
  lines.push(`- 模組數:${(rb.modules ?? []).length};規則總數:${total}`);
  if (rb.parse_errors?.length) lines.push(`- 解析錯誤:${rb.parse_errors.length} 條`);
  if (rb.skipped?.length)
    lines.push(`- 跳過注記:${rb.skipped.map((n) => `${n.file}(${n.reason})`).join(', ')}`);
  if (rb.truncated?.length) lines.push(`- 截斷模組:${rb.truncated.join(', ')}(超過模組內上限)`);
  lines.push('');
  for (const m of rb.modules ?? []) {
    lines.push(`## 模組 ${m.module}(${m.rules.length} 條)`);
    lines.push('');
    for (const r of m.rules) {
      lines.push(`### ${r.id} [${r.category}/${r.qualifier}]`);
      lines.push('');
      lines.push(`- 陳述:${r.statement}`);
      for (const e of r.evidence) {
        const loc =
          e.line > 0
            ? `${e.file}:${e.line}-${e.end_line}`
            : `${e.file}${e.path ? ` \`${e.path}\`` : ''}`;
        lines.push(`- 證據:${loc}(${e.class})`);
        if (e.snippet) {
          lines.push('  ```verilog');
          for (const l of e.snippet.split('\n')) lines.push(`  ${l}`);
          lines.push('  ```');
        } else if (e.path) {
          lines.push(`  - 結構路徑:\`${e.path}\``);
        }
      }
      lines.push(`- check(${r.check.form}):${checkText(r.check)}`);
      lines.push(`- 指紋:\`${r.fingerprint}\``);
      lines.push('');
    }
  }
  return lines.join('\n');
}

export function renderGapsMarkdown(gr) {
  const lines = [];
  lines.push(`# 破口報告:${gr.source}`);
  lines.push('');
  lines.push(`- 破口數:${(gr.items ?? []).length}`);
  if (gr.parse_errors?.length) lines.push(`- 解析錯誤:${gr.parse_errors.length} 條`);
  if (gr.skipped?.length)
    lines.push(`- 跳過注記:${gr.skipped.map((n) => `${n.file}(${n.reason})`).join(', ')}`);
  lines.push('');
  for (const g of gr.items ?? []) {
    lines.push(`### ${g.id} [${g.class} ${g.confidence}] ${g.message}`);
    lines.push('');
    for (const e of g.evidence) {
      const loc =
        e.line > 0
          ? `${e.file}:${e.line}-${e.end_line}`
          : `${e.file}${e.path ? ` \`${e.path}\`` : ''}`;
      lines.push(`- 證據:${loc}(${e.class})`);
    }
    lines.push(`- 建議測試:${g.suggested_test}`);
    lines.push('');
  }
  return lines.join('\n');
}
