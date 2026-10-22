//! Recursive-descent parser core: helpers, recovery, declarations.

pub mod expr;
pub mod module;
pub mod stmt;

use crate::ast;
use crate::token::{Kw, Punct, Tok, Token};
use rcs_diag::{DiagSink, Span};

/// Internal abort signal; diagnostics were already emitted.
pub(crate) struct ParseAbort;

pub(crate) type PResult<T> = Result<T, ParseAbort>;

pub fn parse_source_file(mut toks: Vec<Token>, diags: &mut DiagSink) -> ast::SourceFile {
    // Defensive: the parser's EOF handling assumes a single trailing Eof.
    if toks.last().map(|t| t.kind) != Some(Tok::Eof) {
        let span = toks
            .last()
            .map(|t| Span::new(t.span.file, t.span.end.0, t.span.end.0))
            .unwrap_or_default();
        toks.push(Token::new(Tok::Eof, span, String::new()));
    }
    let mut p = Parser {
        toks,
        pos: 0,
        diags,
    };
    p.source_file()
}

pub(crate) struct Parser<'d> {
    pub(crate) toks: Vec<Token>,
    pub(crate) pos: usize,
    pub(crate) diags: &'d mut DiagSink,
}

impl<'d> Parser<'d> {
    // ---- token access ----

    pub(crate) fn peek(&self) -> &Token {
        let i = self.pos.min(self.toks.len() - 1);
        &self.toks[i]
    }

    pub(crate) fn peek_at(&self, n: usize) -> &Token {
        let i = (self.pos + n).min(self.toks.len() - 1);
        &self.toks[i]
    }

    pub(crate) fn bump(&mut self) -> Token {
        let t = self.peek().clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }

    pub(crate) fn at_eof(&self) -> bool {
        self.peek().kind == Tok::Eof
    }

    pub(crate) fn at_punct(&self, p: Punct) -> bool {
        self.peek().is_punct(p)
    }

    pub(crate) fn eat_punct(&mut self, p: Punct) -> bool {
        if self.at_punct(p) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    pub(crate) fn at_kw(&self, k: Kw) -> bool {
        self.peek().is_kw(k)
    }

    pub(crate) fn eat_kw(&mut self, k: Kw) -> bool {
        if self.at_kw(k) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    // ---- diagnostics ----

    pub(crate) fn err<T>(&mut self, span: Span, code: &'static str, msg: impl Into<String>) -> PResult<T> {
        self.diags.error(code, span, msg);
        Err(ParseAbort)
    }

    pub(crate) fn expect_punct(&mut self, p: Punct, ctx: &str) -> PResult<Span> {
        let t = self.peek().clone();
        if t.is_punct(p) {
            self.pos += 1;
            Ok(t.span)
        } else {
            self.err(
                t.span,
                "VL-E020",
                format!("expected `{}` {}found {}", p.as_str(), ctx, t.describe()),
            )
        }
    }

    pub(crate) fn expect_ident(&mut self, ctx: &str) -> PResult<(String, Span)> {
        let t = self.peek().clone();
        if t.kind == Tok::Ident {
            self.pos += 1;
            Ok((t.text, t.span))
        } else {
            self.err(
                t.span,
                "VL-E020",
                format!("expected identifier ({}), found {}", ctx, t.describe()),
            )
        }
    }

    pub(crate) fn expect_kw(&mut self, k: Kw, ctx: &str) -> PResult<Span> {
        let t = self.peek().clone();
        if t.is_kw(k) {
            self.pos += 1;
            Ok(t.span)
        } else {
            self.err(
                t.span,
                "VL-E020",
                format!("expected `{}` {}, found {}", k.as_str(), ctx, t.describe()),
            )
        }
    }

    /// `(* ... *)` attribute instances are skipped in v0 (contents dropped).
    pub(crate) fn skip_attrs(&mut self) {
        while self.at_punct(Punct::AttrOpen) {
            while !self.at_eof() && !self.eat_punct(Punct::AttrClose) {
                self.bump();
            }
        }
    }

    // ---- recovery ----

    /// After a failed statement: skip to just past the next `;`, or stop
    /// before a structural closer. Guarantees forward progress.
    pub(crate) fn sync_stmt(&mut self) {
        let start = self.pos;
        let mut depth = 0usize;
        while !self.at_eof() {
            match self.peek().kind {
                Tok::Punct(Punct::LParen) | Tok::Punct(Punct::LBrace) | Tok::Punct(Punct::LBracket) => {
                    depth += 1
                }
                Tok::Punct(Punct::RParen) | Tok::Punct(Punct::RBrace) | Tok::Punct(Punct::RBracket) => {
                    if depth == 0 {
                        return; // let caller's structure handle it
                    }
                    depth -= 1
                }
                Tok::Punct(Punct::Semi) if depth == 0 => {
                    self.pos += 1;
                    return;
                }
                Tok::Keyword(Kw::End)
                | Tok::Keyword(Kw::Endcase)
                | Tok::Keyword(Kw::Endmodule)
                | Tok::Keyword(Kw::Endfunction)
                | Tok::Keyword(Kw::Endtask)
                | Tok::Keyword(Kw::Endgenerate)
                | Tok::Keyword(Kw::Else) => {
                    if depth == 0 {
                        return;
                    }
                }
                Tok::Eof => return,
                _ => {}
            }
            self.pos += 1;
        }
        if self.pos == start {
            self.pos += 1; // absolute progress guarantee
        }
    }

    /// After a failed module item: skip to past the next top-level `;` or
    /// stop before a module/construct keyword.
    pub(crate) fn sync_item(&mut self) {
        let start = self.pos;
        while !self.at_eof() {
            match self.peek().kind {
                Tok::Punct(Punct::Semi) => {
                    self.pos += 1;
                    return;
                }
                Tok::Keyword(Kw::Endmodule)
                | Tok::Keyword(Kw::Endgenerate)
                | Tok::Keyword(Kw::Endfunction)
                | Tok::Keyword(Kw::Endtask)
                | Tok::Keyword(Kw::Module)
                | Tok::Keyword(Kw::Macromodule)
                | Tok::Keyword(Kw::Initial)
                | Tok::Keyword(Kw::Always)
                | Tok::Keyword(Kw::Assign)
                | Tok::Keyword(Kw::Generate)
                | Tok::Keyword(Kw::Function)
                | Tok::Keyword(Kw::Task) => return,
                Tok::Eof => return,
                _ => {}
            }
            self.pos += 1;
        }
        if self.pos == start {
            self.pos += 1;
        }
    }

    fn sync_top(&mut self) {
        while !self.at_eof() {
            if self.at_kw(Kw::Module) || self.at_kw(Kw::Macromodule) {
                return;
            }
            self.pos += 1;
        }
    }

    // ---- top level ----

    fn source_file(&mut self) -> ast::SourceFile {
        let mut modules = Vec::new();
        while !self.at_eof() {
            self.skip_attrs();
            if self.at_eof() {
                break;
            }
            if self.at_kw(Kw::Module) || self.at_kw(Kw::Macromodule) {
                match self.module() {
                    Ok(m) => modules.push(m),
                    Err(ParseAbort) => {
                        self.sync_top();
                    }
                }
            } else {
                let t = self.peek().clone();
                let _ = self.err::<()>(
                    t.span,
                    "VL-E020",
                    format!("expected `module` at top level, found {}", t.describe()),
                );
                self.sync_top();
            }
        }
        ast::SourceFile { modules }
    }

    // ---- declarations ----

    /// Range `[msb:lsb]`.
    pub(crate) fn range(&mut self) -> PResult<ast::Range> {
        let start = self.expect_punct(Punct::LBracket, "for a range")?;
        let msb = self.expr()?;
        self.expect_punct(Punct::Colon, "in range")?;
        let lsb = self.expr()?;
        let end = self.expect_punct(Punct::RBracket, "to close range")?;
        Ok(ast::Range {
            span: start.merge(end),
            msb,
            lsb,
        })
    }

    /// `assign_pairs`: `lhs = rhs {, lhs = rhs}` (for/defparam/assign).
    pub(crate) fn assign_pairs(&mut self) -> PResult<Vec<ast::AssignPair>> {
        let mut out = Vec::new();
        loop {
            let lhs = self.expr()?;
            let span = lhs.span();
            if self.eat_punct(Punct::Eq) {
                self.reject_intra_assign()?;
                let rhs = self.expr()?;
                out.push(ast::AssignPair {
                    span: span.merge(rhs.span()),
                    lhs,
                    rhs,
                });
            } else if let ast::Expr::Binary {
                op: ast::BinOp::Le,
                lhs: l,
                rhs: r,
                span,
            } = lhs
            {
                if !l.is_lvalue() {
                    return self.err(span, "VL-E021", "left side of assignment is not an lvalue");
                }
                out.push(ast::AssignPair {
                    span,
                    lhs: *l,
                    rhs: *r,
                });
            } else {
                return self.err(
                    lhs.span(),
                    "VL-E021",
                    "expected `=` assignment",
                );
            }
            if !self.eat_punct(Punct::Comma) {
                return Ok(out);
            }
        }
    }

    /// Intra-assignment delay/repeat (`a = #5 b;`) — explicit not-yet error.
    pub(crate) fn reject_intra_assign(&mut self) -> PResult<()> {
        let t = self.peek().clone();
        if t.is_punct(Punct::Hash) || t.is_punct(Punct::At) {
            return self.err(
                t.span,
                "VL-E900",
                "intra-assignment delay/event control is not yet supported (milestone M8)",
            );
        }
        Ok(())
    }

    /// Declaration after its leading keyword(s) are consumed:
    /// `[signed] [range] name {unpacked_range} [= init] {, ...} ;`
    pub(crate) fn decl_tail(
        &mut self,
        span: Span,
        kind: ast::DeclKind,
        ty: Option<ast::ParamType>,
        dir: Option<ast::Direction>,
    ) -> PResult<ast::Decl> {
        let mut signed = false;
        let mut range: Option<ast::Range> = None;
        loop {
            if self.eat_kw(Kw::Signed) {
                signed = true;
            } else if self.at_punct(Punct::LBracket) {
                if range.is_some() {
                    // packed multi-dim arrays are v0-unimplemented
                    return self.err(
                        self.peek().span,
                        "VL-E900",
                        "multi-dimensional packed arrays are not yet supported (milestone M8)",
                    );
                }
                range = Some(self.range()?);
            } else {
                break;
            }
        }
        let mut entries = Vec::new();
        loop {
            let espan = self.peek().span;
            let (name, _) = self.expect_ident("declaration name")?;
            let mut unpacked = Vec::new();
            while self.at_punct(Punct::LBracket) {
                unpacked.push(self.range()?);
            }
            let init = if self.eat_punct(Punct::Eq) {
                if dir.is_some() {
                    return self.err(
                        espan,
                        "VL-E020",
                        "initial values on port declarations are SystemVerilog (not yet supported)",
                    );
                }
                Some(self.expr()?)
            } else {
                None
            };
            entries.push(ast::DeclEntry {
                span: espan,
                name,
                unpacked,
                init,
            });
            if !self.eat_punct(Punct::Comma) {
                break;
            }
        }
        let end = self.expect_punct(Punct::Semi, "to end declaration")?;
        Ok(ast::Decl {
            span: span.merge(end),
            kind,
            ty,
            signed,
            range,
            dir,
            entries,
        })
    }

    /// Non-ANSI `input/output/inout` declaration: optional net/reg type, then
    /// `decl_tail` with the direction attached.
    pub(crate) fn dir_decl(&mut self, dir: ast::Direction, span: Span) -> PResult<ast::Decl> {
        let mut kind: Option<ast::DeclKind> = None;
        let mut ty: Option<ast::ParamType> = None;
        if let Tok::Keyword(k) = self.peek().kind {
            if let Some(nk) = ast::NetKind::from_kw(k) {
                self.pos += 1;
                kind = Some(ast::DeclKind::Net(nk));
            } else if k == Kw::Reg {
                self.pos += 1;
                kind = Some(ast::DeclKind::Reg);
            } else if k == Kw::Integer {
                self.pos += 1;
                kind = Some(ast::DeclKind::Integer);
                ty = Some(ast::ParamType::Integer);
            } else if k == Kw::Time {
                self.pos += 1;
                kind = Some(ast::DeclKind::Time);
                ty = Some(ast::ParamType::Time);
            } else if k == Kw::Real {
                self.pos += 1;
                kind = Some(ast::DeclKind::Real);
                ty = Some(ast::ParamType::Real);
            } else if k == Kw::Realtime {
                self.pos += 1;
                kind = Some(ast::DeclKind::Realtime);
                ty = Some(ast::ParamType::Realtime);
            }
        }
        let kind = kind.unwrap_or(ast::DeclKind::Net(ast::NetKind::Wire));
        self.decl_tail(span, kind, ty, Some(dir))
    }

    /// True when the current token can start a new ANSI port group
    /// (direction / net type / reg / signed / range).
    pub(crate) fn peek_starts_port_spec(&self) -> bool {
        match self.peek().kind {
            Tok::Keyword(k) => {
                matches!(
                    k,
                    Kw::Input | Kw::Output | Kw::Inout | Kw::Reg | Kw::Signed
                ) || ast::NetKind::from_kw(k).is_some()
            }
            Tok::Punct(Punct::LBracket) => true,
            _ => false,
        }
    }

    /// A `net type` / `reg` keyword at the current position, if any.
    pub(crate) fn take_net_or_reg_kind(&mut self) -> Option<ast::DeclKind> {
        if let Tok::Keyword(k) = self.peek().kind {
            if let Some(nk) = ast::NetKind::from_kw(k) {
                self.pos += 1;
                return Some(ast::DeclKind::Net(nk));
            }
            if k == Kw::Reg {
                self.pos += 1;
                return Some(ast::DeclKind::Reg);
            }
        }
        None
    }
}
