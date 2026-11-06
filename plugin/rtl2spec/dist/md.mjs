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
      return `trigger=${t}; when=${p.when ?? '-'}; async=${p.async === true}; effects=${eff}`;
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

// 跳過注記標籤(detail=Some 時追加,與 rtl-spec md.rs 同一定義;
// None 時既有形 `${file}(${reason})` byte 不變)。
export function skipLabel(n) {
  return n.detail ? `${n.file}(${n.reason}: ${n.detail})` : `${n.file}(${n.reason})`;
}

// 接口契約塊渲染(S1;與 crates/rtl-spec/src/md.rs contract_markdown 同一
// 定義):參數 `名=值` 帶 `(local)` 標;端口 `名:dir[width]`(parametric→
// `[parametric]`;signed→` signed` 後綴);內部聲明 `名:kind[width]`(width
// =0=參數化範圍佔位;unpacked>0 追加 `x<dims>`);空面省略;contract 缺席
// (舊資料)回空串。
export function renderContractMarkdown(c) {
  if (!c) return '';
  const lines = [];
  lines.push('- 接口契約:');
  if (c.params?.length) {
    const items = c.params.map((p) =>
      p.local ? `${p.name}=${p.value} (local)` : `${p.name}=${p.value}`,
    );
    lines.push(`  - 參數: ${items.join('; ')}`);
  }
  if (c.ports?.length) {
    const items = c.ports.map((p) => {
      const width = p.parametric ? '[parametric]' : `[${p.width}]`;
      const signed = p.signed ? ' signed' : '';
      return `${p.name}:${p.dir}${width}${signed}`;
    });
    lines.push(`  - 端口: ${items.join('; ')}`);
  }
  if (c.signals?.length) {
    const items = c.signals.map((g) =>
      g.unpacked_dims > 0
        ? `${g.name}:${g.kind}[${g.width}]x${g.unpacked_dims}`
        : `${g.name}:${g.kind}[${g.width}]`,
    );
    lines.push(`  - 內部聲明: ${items.join('; ')}`);
  }
  return lines.join('\n');
}

export function renderRulebookMarkdown(rb) {
  const lines = [];
  const total = sumRules(rb);
  lines.push(`# 規則書:${rb.source}`);
  lines.push('');
  lines.push(`- 模組數:${(rb.modules ?? []).length};規則總數:${total}`);
  if (rb.parse_errors?.length) lines.push(`- 解析錯誤:${rb.parse_errors.length} 條`);
  if (rb.skipped?.length)
    lines.push(`- 跳過注記:${rb.skipped.map((n) => skipLabel(n)).join(', ')}`);
  if (rb.truncated?.length) lines.push(`- 截斷模組:${rb.truncated.join(', ')}(超過模組內上限)`);
  // N5 信號-聲明完備性 lint(S1;審讀 #6):與 rtl-spec md.rs 同一定義。
  const dangling = rb.decl_lint?.dangling_refs ?? [];
  if (dangling.length === 0) lines.push('- 聲明完備性 lint:懸空引用 0 條');
  else lines.push(`- 聲明完備性 lint:懸空引用 ${dangling.length} 條:${dangling.join(', ')}`);
  lines.push('');
  for (const m of rb.modules ?? []) {
    lines.push(`## 模組 ${m.module}(${m.rules.length} 條)`);
    lines.push('');
    lines.push(renderContractMarkdown(m.contract));
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
    lines.push(`- 跳過注記:${gr.skipped.map((n) => skipLabel(n)).join(', ')}`);
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
