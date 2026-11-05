//! markdown 渲染(確定性;validation-report.md 的規則書全文樣本資料源)。

use crate::schema::{CheckPayload, GapReport, Rulebook};

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
        out.push('\n');
        for m in &self.modules {
            out.push_str(&format!("## 模組 {}({} 條)\n\n", m.module, m.rules.len()));
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
                "trigger={trig}; when={}; effects={}",
                when.as_deref().unwrap_or("-"),
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
