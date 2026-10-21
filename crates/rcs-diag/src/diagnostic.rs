//! Diagnostic values and collection.

use crate::span::Span;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        }
    }
}

/// A single diagnostic: severity, stable machine-readable code, message,
/// primary span, and optional attached notes (with their own spans).
#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Stable code, e.g. `"VL-E001"`. Message text may change; codes must not.
    pub code: &'static str,
    pub message: String,
    pub span: Span,
    pub notes: Vec<(String, Option<Span>)>,
}

impl Diagnostic {
    pub fn error(code: &'static str, span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Error,
            code,
            message: message.into(),
            span,
            notes: Vec::new(),
        }
    }

    pub fn warning(code: &'static str, span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            code,
            message: message.into(),
            span,
            notes: Vec::new(),
        }
    }

    pub fn note(mut self, message: impl Into<String>, span: Option<Span>) -> Self {
        self.notes.push((message.into(), span));
        self
    }
}

/// Collects diagnostics from all pipeline stages.
#[derive(Default)]
pub struct DiagSink {
    pub diags: Vec<Diagnostic>,
}

impl DiagSink {
    pub fn new() -> Self {
        DiagSink { diags: Vec::new() }
    }

    pub fn push(&mut self, d: Diagnostic) {
        self.diags.push(d);
    }

    pub fn error(&mut self, code: &'static str, span: Span, message: impl Into<String>) {
        self.diags.push(Diagnostic::error(code, span, message));
    }

    pub fn warning(&mut self, code: &'static str, span: Span, message: impl Into<String>) {
        self.diags.push(Diagnostic::warning(code, span, message));
    }

    pub fn error_count(&self) -> usize {
        self.diags
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }

    pub fn has_errors(&self) -> bool {
        self.error_count() > 0
    }
}
