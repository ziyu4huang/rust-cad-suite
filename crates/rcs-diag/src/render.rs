//! Human-facing diagnostic rendering (rustc-style carets).

use crate::diagnostic::Diagnostic;
use crate::source_map::SourceMap;
use std::fmt::Write;

/// Render diagnostics to `out`:
///
/// ```text
/// error[VL-E001]: unexpected token `end`
///   --> tb.v:4:9
///    |
///  4 |     end else begin
///    |         ^^^
/// ```
pub fn render_diags(out: &mut String, sm: &SourceMap, diags: &[Diagnostic]) {
    for d in diags {
        let _ = writeln!(out, "{}[{}]: {}", d.severity.as_str(), d.code, d.message);
        render_location(out, sm, d);
        let _ = writeln!(out);
    }
}

fn render_location(out: &mut String, sm: &SourceMap, d: &Diagnostic) {
    if !sm.has(d.span.file) {
        // Default/unmapped span (e.g. a driver-level failure before any
        // source was registered): render the message without a location.
        for (msg, _) in &d.notes {
            let _ = writeln!(out, "  = note: {msg}");
        }
        return;
    }
    let (line, col) = sm.line_col(d.span.file, d.span.start);
    let _ = writeln!(
        out,
        "  --> {}:{}:{}",
        sm.file_name(d.span.file),
        line,
        col
    );
    let line_text = sm.line_text(d.span.file, line);
    let line_no = format!("{:>4}", line);
    let gutter = " ".repeat(line_no.len());
    let _ = writeln!(out, "{} |", gutter);
    let _ = writeln!(out, "{} | {}", line_no, line_text);

    // Caret run, clipped to the visible line (multi-line spans show their
    // first line only).
    let line_start = sm.line_start_offset(d.span.file, line);
    let start = d.span.start.to_usize().saturating_sub(line_start);
    let end = d
        .span
        .end
        .to_usize()
        .saturating_sub(line_start)
        .max(start + 1)
        .min(line_text.len().max(start + 1));
    let _ = writeln!(
        out,
        "{} | {}{}",
        gutter,
        " ".repeat(start),
        "^".repeat(end - start)
    );

    for (msg, span) in &d.notes {
        match span {
            Some(s) => {
                let (nl, nc) = sm.line_col(s.file, s.start);
                let _ = writeln!(
                    out,
                    "{} = note: {}:{}:{}: {}",
                    gutter,
                    sm.file_name(s.file),
                    nl,
                    nc,
                    msg
                );
            }
            None => {
                let _ = writeln!(out, "{} = note: {}", gutter, msg);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::Diagnostic;
    use crate::span::Span;

    #[test]
    fn renders_caret_under_token() {
        let mut sm = SourceMap::new();
        let src = "module m;\n    end else begin\nendmodule\n";
        let f = sm.add("tb.v", src.to_string());
        // "    end" -> `end` starts at offset 10+4 = 14, len 3
        let d = Diagnostic::error("VL-E001", Span::new(f, 14, 17), "unexpected token `end`")
            .note("expected a module item", None);
        let mut out = String::new();
        render_diags(&mut out, &sm, &[d]);
        let expected = "\
error[VL-E001]: unexpected token `end`
  --> tb.v:2:5
     |
   2 |     end else begin
     |     ^^^
     = note: expected a module item

";
        assert_eq!(out, expected);
    }
}
