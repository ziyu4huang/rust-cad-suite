//! Verilog-2005 (IEEE 1364-2005) frontend: preprocessor, lexer, parser, AST.
//!
//! Pipeline: source text → [`lexer::lex`] (raw tokens incl. `` ` `` directives)
//! → [`pp::Preprocessor`] (macro expansion, conditionals, includes) →
//! [`parser::parse_source_file`] → [`ast::SourceFile`].
//!
//! Zero external dependencies by policy (see PLAN §3).

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod pp;
pub mod token;

pub use rcs_diag;
