//! markdown 渲染(確定性;validation-report.md 的規則書全文樣本資料源)。

use crate::schema::{CheckPayload, Contract, GapReport, Rulebook};

/// 接口契約塊渲染(S1;確定性;與 dist/md.mjs renderContract 同一定義):
/// 參數 `名=值` 帶 `(local)` 標;端口 `名:dir[width]`(parametric=true→
/// `[parametric]`;signed=true→` signed` 後綴);內部聲明 `名:kind[width]`
/// (width=0=參數化範圍佔位,§4.2;unpacked>0 追加 `x<dims>`);空面省略。
fn contract_markdown(c: &Contract) -> String {
    let mut out = String::new();
    out.push_str("- 接口契約:\n");
    if !c.params.is_empty() {
        let items: Vec<String> = c
            .params
            .iter()
            .map(|p| {
                if p.local {
                    format!("{}={} (local)", p.name, p.value)
                } else {
                    format!("{}={}", p.name, p.value)
                }
            })
            .collect();
        out.push_str(&format!("  - 參數: {}\n", items.join("; ")));
    }
    if !c.ports.is_empty() {
        let items: Vec<String> = c
            .ports
            .iter()
            .map(|p| {
                let width = if p.parametric {
                    "[parametric]".to_string()
                } else {
                    format!("[{}]", p.width)
                };
                let signed = if p.signed { " signed" } else { "" };
                format!("{}:{}{}{}", p.name, p.dir, width, signed)
            })
            .collect();
        out.push_str(&format!("  - 端口: {}\n", items.join("; ")));
    }
    if !c.signals.is_empty() {
        let items: Vec<String> = c
            .signals
            .iter()
            .map(|g| {
                if g.unpacked_dims > 0 {
                    format!("{}:{}[{}]x{}", g.name, g.kind, g.width, g.unpacked_dims)
                } else {
                    format!("{}:{}[{}]", g.name, g.kind, g.width)
                }
            })
            .collect();
        out.push_str(&format!("  - 內部聲明: {}\n", items.join("; ")));
    }
    out
}

impl Rulebook {
    pub fn render_markdown(&self) -> String {
        let mut out = String::new();
        let total = self.total_rules();
        out.push_str(&format!("# 規則書:{}\n\n", self.source));
        out.push_str(&format!(
            "- 模組數:{};規則總數:{}\n",
            self.modules.len(),
            total
        ));
        let cats = self.category_counts();
        if !cats.is_empty() {
            let joined: Vec<String> = cats
                .iter()
                .map(|(k, n)| format!("{}={}", k, n))
                .collect();
            out.push_str(&format!("- 類別分布:{}\n", joined.join(", ")));
        }
        if !self.parse_errors.is_empty() {
            out.push_str(&format!(
                "- 解析錯誤:{} 條\n",
                self.parse_errors.len()
            ));
            for e in &self.parse_errors {
                out.push_str(&format!("  - {e}\n"));
            }
        }
        if !self.skipped.is_empty() {
            let joined: Vec<String> = self
                .skipped
                .iter()
                .map(|n| match &n.detail {
                    Some(d) => format!("{}({}: {})", n.file, n.reason.as_str(), d),
                    None => format!("{}({})", n.file, n.reason.as_str()),
                })
                .collect();
            out.push_str(&format!("- 跳過注記:{}\n", joined.join(", ")));
        }
        if !self.truncated.is_empty() {
            out.push_str(&format!(
                "- 截斷模組:{}(超過模組內上限)\n",
                self.truncated.join(", ")
            ));
        }
        // N5 信號-聲明完備性 lint(S1;審讀 #6):隨提取器輸出每輪重算。
        if self.decl_lint.dangling_refs.is_empty() {
            out.push_str("- 聲明完備性 lint:懸空引用 0 條\n");
        } else {
            out.push_str(&format!(
                "- 聲明完備性 lint:懸空引用 {} 條:{}\n",
                self.decl_lint.dangling_refs.len(),
                self.decl_lint.dangling_refs.join(", ")
            ));
        }
        out.push('\n');
        for m in &self.modules {
            out.push_str(&format!("## 模組 {}({} 條)\n\n", m.module, m.rules.len()));
            // S1 接口契約章(N2):①端口/參數面+②內部聲明清單。
            out.push_str(&contract_markdown(&m.contract));
            for r in &m.rules {
                out.push_str(&format!(
                    "### {} [{}/{}]\n\n",
                    r.id,
                    r.category.as_str(),
                    r.qualifier.as_str()
                ));
                out.push_str(&format!("- 陳述:{}\n", r.statement));
                for e in &r.evidence {
                    out.push_str(&format!(
                        "- 證據:{}:{}-{}({})\n",
                        e.file,
                        e.line,
                        e.end_line,
                        e.class.as_str()
                    ));
                    out.push_str("  ```verilog\n");
                    for line in e.snippet.split('\n') {
                        out.push_str(&format!("  {line}\n"));
                    }
                    out.push_str("  ```\n");
                }
                out.push_str(&format!("- check({}):{}\n", r.check.form.as_str(), check_text(&r.check.payload)));
                out.push_str(&format!("- 指紋:`{}`\n\n", r.fingerprint));
            }
        }
        out
    }
}

fn check_text(p: &CheckPayload) -> String {
    match p {
        CheckPayload::Guard {
            trigger,
            when,
            r#async,
            effects,
        } => {
            let trig = trigger
                .as_ref()
                .map(|t| {
                    format!(
                        "{} {}",
                        t.signal.as_deref().unwrap_or("?"),
                        t.edge.as_deref().unwrap_or("?")
                    )
                })
                .unwrap_or_else(|| "-".to_string());
            let eff: Vec<String> = effects
                .iter()
                .map(|e| format!("{}←{}", e.lhs, e.value))
                .collect();
            format!(
                "trigger={trig}; when={}; async={}; effects={}",
                when.as_deref().unwrap_or("-"),
                r#async,
                eff.join("; ")
            )
        }
        CheckPayload::Transition {
            state_reg,
            from,
            to,
            when,
        } => format!(
            "{state_reg}: {} → {} when {}",
            from,
            to,
            when.as_deref().unwrap_or("-")
        ),
        CheckPayload::Sequence {
            signal,
            becomes,
            anchor,
            latency,
            when,
        } => format!(
            "anchor({anchor}) +{latency}拍 → {signal}={becomes} when {}",
            when.as_deref().unwrap_or("-")
        ),
        CheckPayload::Relation {
            lhs,
            rhs,
            when,
            width_note,
        } => format!(
            "{lhs} ≡ {rhs} when {}{}",
            when.as_deref().unwrap_or("-"),
            width_note
                .as_ref()
                .map(|n| format!(" ({n})"))
                .unwrap_or_default()
        ),
        CheckPayload::Invariant {
            subject,
            kind,
            values,
            expr,
        } => format!(
            "{subject} {} {}",
            kind.as_str(),
            if values.is_empty() {
                expr.as_deref().unwrap_or("-").to_string()
            } else {
                values.join(" | ")
            }
        ),
    }
}

impl GapReport {
    pub fn render_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# 破口報告:{}\n\n", self.source));
        out.push_str(&format!("- 破口數:{}\n", self.items.len()));
        if !self.parse_errors.is_empty() {
            out.push_str(&format!("- 解析錯誤:{} 條\n", self.parse_errors.len()));
        }
        if !self.skipped.is_empty() {
            let joined: Vec<String> = self
                .skipped
                .iter()
                .map(|n| match &n.detail {
                    Some(d) => format!("{}({}: {})", n.file, n.reason.as_str(), d),
                    None => format!("{}({})", n.file, n.reason.as_str()),
                })
                .collect();
            out.push_str(&format!("- 跳過注記:{}\n", joined.join(", ")));
        }
        out.push('\n');
        for g in &self.items {
            out.push_str(&format!(
                "### {} [{} {}] {}\n\n",
                g.id,
                g.class.as_str(),
                g.confidence.as_str(),
                g.message
            ));
            for e in &g.evidence {
                out.push_str(&format!(
                    "- 證據:{}:{}-{}({})\n",
                    e.file, e.line, e.end_line, e.class.as_str()
                ));
            }
            out.push_str(&format!("- 建議測試:{}\n\n", g.suggested_test));
        }
        out
    }
}
