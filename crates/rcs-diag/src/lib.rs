//! Shared diagnostics infrastructure for the rust-cad-suite tools.
//!
//! Zero external dependencies by policy: everything downstream (frontend,
//! elaborator, simulator, future synthesis crates) renders errors through
//! this crate so output shape stays uniform across the suite.

pub mod diagnostic;
pub mod render;
pub mod source_map;
pub mod span;

pub use diagnostic::{Diagnostic, DiagSink, Severity};
pub use render::render_diags;
pub use source_map::{SourceFile, SourceMap};
pub use span::{BytePos, FileId, Span};
