//! Statement parsing (procedural blocks, timing controls, case, loops).

use super::{PResult, Parser};
use crate::ast::{self, AssignPair, CaseItem, CaseKind, DelayVal, EdgeKind, EventCtrl, EventEdge, Expr, Stmt};
use crate::token::{Kw, Punct, Tok};

impl<'d> Parser<'d> {
    pub(crate) fn stmt(&mut self) -> PResult<Stmt> {
        self.skip_attrs();
        let t = self.peek().clone();
        match t.kind {
            Tok::Keyword(Kw::Begin) => self.block(),
            Tok::Keyword(Kw::If) => self.if_stmt(),
            Tok::Keyword(Kw::Case) => self.case_stmt(CaseKind::Case),
            Tok::Keyword(Kw::Casex) => self.case_stmt(CaseKind::Casex),
            Tok::Keyword(Kw::Casez) => self.case_stmt(CaseKind::Casez),
            Tok::Keyword(Kw::For) => self.for_stmt(),
            Tok::Keyword(Kw::While) => {
                self.pos += 1;
                self.expect_punct(Punct::LParen, "after `while`")?;
                let cond = self.expr()?;
                self.expect_punct(Punct::RParen, "after `while` condition")?;
                let body = self.stmt()?;
                Ok(Stmt::While {
                    span: t.span.merge(stmt_span(&body)),
                    cond,
                    body: Box::new(body),
                })
            }
            Tok::Keyword(Kw::Repeat) => {
                self.pos += 1;
                self.expect_punct(Punct::LParen, "after `repeat`")?;
                let count = self.expr()?;
                self.expect_punct(Punct::RParen, "after `repeat` count")?;
                let body = self.stmt()?;
                Ok(Stmt::Repeat {
                    span: t.span.merge(stmt_span(&body)),
                    count,
                    body: Box::new(body),
                })
            }
            Tok::Keyword(Kw::Forever) => {
                self.pos += 1;
                let body = self.stmt()?;
                Ok(Stmt::Forever {
                    span: t.span.merge(stmt_span(&body)),
                    body: Box::new(body),
                })
            }
            Tok::Keyword(Kw::Wait) | Tok::Keyword(Kw::Fork) => self.err(
                t.span,
                "VL-E900",
                format!(
                    "`{}` is not yet supported (milestone M8)",
                    if t.is_kw(Kw::Wait) { "wait" } else { "fork/join" }
                ),
            ),
            Tok::Keyword(Kw::Force) | Tok::Keyword(Kw::Release) | Tok::Keyword(Kw::Deassign) => self.err(
                t.span,
                "VL-E900",
                "force/release/deassign are not yet supported (milestone M7)",
            ),
            Tok::Keyword(Kw::Assign) => self.err(
                t.span,
                "VL-E900",
                "procedural continuous assignment is not yet supported (milestone M7)",
            ),
            Tok::Keyword(Kw::Disable) => {
                self.pos += 1;
                let (name, _) = self.expect_ident("block name after `disable`")?;
                let end = self.expect_punct(Punct::Semi, "after `disable`")?;
                Ok(Stmt::Disable {
                    span: t.span.merge(end),
                    name,
                })
            }
            Tok::Punct(Punct::Semi) => {
                self.pos += 1;
                Ok(Stmt::Null { span: t.span })
            }
            Tok::Punct(Punct::Hash) => {
                self.pos += 1;
                let delay = self.delay_val()?;
                let body = self.timing_body()?;
                let end = body.as_ref().map(|b| stmt_span(b)).unwrap_or(t.span);
                Ok(Stmt::DelayCtrl {
                    span: t.span.merge(end),
                    delay,
                    body,
                })
            }
            Tok::Punct(Punct::At) => {
                self.pos += 1;
                let ctrl = self.event_ctrl(t.span)?;
                let body = self.timing_body()?;
                let end = body.as_ref().map(|b| stmt_span(b)).unwrap_or(ctrl.span);
                Ok(Stmt::EventCtrl {
                    span: t.span.merge(end),
                    ctrl,
                    body,
                })
            }
            Tok::SysIdent => {
                self.pos += 1;
                let args = if self.at_punct(Punct::LParen) {
                    self.paren_expr_list()?
                } else {
                    Vec::new()
                };
                let end = self.expect_punct(Punct::Semi, "after system task call")?;
                Ok(Stmt::SysCall {
                    span: t.span.merge(end),
                    name: t.text,
                    args,
                })
            }
            Tok::Keyword(k) if !super::expr::starts_expr_stmt(&t) => self.err(
                t.span,
                "VL-E020",
                format!("expected statement, found keyword `{}`", k.as_str()),
            ),
            _ => self.expr_stmt(),
        }
    }

    /// Statement following `#delay` / `@(...)`: `;` means no body.
    fn timing_body(&mut self) -> PResult<Option<Box<Stmt>>> {
        if self.eat_punct(Punct::Semi) {
            return Ok(None);
        }
        Ok(Some(Box::new(self.stmt()?)))
    }

    fn block(&mut self) -> PResult<Stmt> {
        let start = self.expect_kw(Kw::Begin, "for block")?;
        let name = if self.eat_punct(Punct::Colon) {
            Some(self.expect_ident("block name")?.0)
        } else {
            None
        };
        let mut body = Vec::new();
        loop {
            self.skip_attrs();
            if self.at_kw(Kw::End) {
                break;
            }
            if self.at_eof() {
                return self.err(start, "VL-E020", "unterminated `begin` block (missing `end`)");
            }
            // Block-local declarations (reg/integer/...) are legal in named
            // blocks; v0 parses and discards them with a not-yet error.
            if self.peek_is_var_decl_kw() {
                let t = self.peek().clone();
                return self.err(
                    t.span,
                    "VL-E900",
                    "block-local declarations are not yet supported (milestone M7)",
                );
            }
            let before = self.pos;
            match self.stmt() {
                Ok(s) => body.push(s),
                Err(_) => {
                    self.sync_stmt();
                    if self.pos == before {
                        self.pos += 1;
                    }
                }
            }
        }
        let end = self.expect_kw(Kw::End, "to close block")?;
        if self.eat_punct(Punct::Colon) {
            let _ = self.expect_ident("block name after `end :`")?;
        }
        Ok(Stmt::Block {
            span: start.merge(end),
            name,
            body,
        })
    }

    pub(crate) fn peek_is_var_decl_kw(&self) -> bool {
        matches!(
            self.peek().kind,
            Tok::Keyword(Kw::Reg)
                | Tok::Keyword(Kw::Integer)
                | Tok::Keyword(Kw::Real)
                | Tok::Keyword(Kw::Realtime)
                | Tok::Keyword(Kw::Time)
                | Tok::Keyword(Kw::Parameter)
                | Tok::Keyword(Kw::Localparam)
                | Tok::Keyword(Kw::Event)
        )
    }

    fn if_stmt(&mut self) -> PResult<Stmt> {
        let start = self.expect_kw(Kw::If, "for if statement")?;
        self.expect_punct(Punct::LParen, "after `if`")?;
        let cond = self.expr()?;
        self.expect_punct(Punct::RParen, "after `if` condition")?;
        let then = self.stmt()?;
        let mut span = start.merge(stmt_span(&then));
        let els = if self.eat_kw(Kw::Else) {
            let e = self.stmt()?;
            span = span.merge(stmt_span(&e));
            Some(Box::new(e))
        } else {
            None
        };
        Ok(Stmt::If {
            span,
            cond,
            then: Box::new(then),
            els,
        })
    }

    fn case_stmt(&mut self, kind: CaseKind) -> PResult<Stmt> {
        let start = self.bump().span;
        self.expect_punct(Punct::LParen, "after `case`")?;
        let expr = self.expr()?;
        self.expect_punct(Punct::RParen, "after `case` expression")?;
        let mut items = Vec::new();
        loop {
            self.skip_attrs();
            if self.at_kw(Kw::Endcase) {
                break;
            }
            if self.at_eof() {
                return self.err(start, "VL-E020", "unterminated `case` (missing `endcase`)");
            }
            if self.eat_kw(Kw::Default) {
                self.eat_punct(Punct::Colon);
                let s = self.stmt()?;
                items.push(CaseItem::Default(s));
                continue;
            }
            let mut labels = vec![self.expr()?];
            while self.eat_punct(Punct::Comma) {
                labels.push(self.expr()?);
            }
            self.expect_punct(Punct::Colon, "after case item labels")?;
            let s = self.stmt()?;
            items.push(CaseItem::Items(labels, s));
        }
        let end = self.expect_kw(Kw::Endcase, "to close case")?;
        Ok(Stmt::Case {
            span: start.merge(end),
            kind,
            expr,
            items,
        })
    }

    fn for_stmt(&mut self) -> PResult<Stmt> {
        let start = self.expect_kw(Kw::For, "for loop")?;
        self.expect_punct(Punct::LParen, "after `for`")?;
        let init = self.assign_pairs()?;
        self.expect_punct(Punct::Semi, "after for-init")?;
        let cond = self.expr()?;
        self.expect_punct(Punct::Semi, "after for-condition")?;
        let step = self.assign_pairs()?;
        self.expect_punct(Punct::RParen, "after for-step")?;
        let body = self.stmt()?;
        Ok(Stmt::For {
            span: start.merge(stmt_span(&body)),
            init,
            cond,
            step,
            body: Box::new(body),
        })
    }

    /// `#` delay value: literal, identifier, or `( expr )`.
    pub(crate) fn delay_val(&mut self) -> PResult<DelayVal> {
        let t = self.peek().clone();
        match t.kind {
            Tok::IntLit | Tok::RealLit | Tok::BasedLit => {
                self.pos += 1;
                Ok(DelayVal::Lit(t.text))
            }
            Tok::Ident => {
                let e = self.primary_ident_only()?;
                Ok(DelayVal::Expr(e))
            }
            Tok::Punct(Punct::LParen) => {
                self.pos += 1;
                let e = self.expr()?;
                if self.at_punct(Punct::Colon) {
                    return self.err(
                        self.peek().span,
                        "VL-E900",
                        "min:typ:max delays are not yet supported (milestone M7)",
                    );
                }
                self.expect_punct(Punct::RParen, "to close delay expression")?;
                Ok(DelayVal::Expr(e))
            }
            _ => self.err(
                t.span,
                "VL-E020",
                format!("expected delay value after `#`, found {}", t.describe()),
            ),
        }
    }

    /// For `#ident` delays: a bare (possibly hierarchical) identifier.
    fn primary_ident_only(&mut self) -> PResult<Expr> {
        let t = self.bump();
        let mut parts = vec![t.text.clone()];
        while self.at_punct(Punct::Dot) && self.peek_at(1).kind == Tok::Ident {
            self.pos += 2;
            parts.push(self.toks[self.pos - 1].text.clone());
        }
        let span = t.span.merge(self.toks[self.pos - 1].span);
        if parts.len() == 1 {
            Ok(Expr::Ident { span, name: t.text })
        } else {
            Ok(Expr::HierIdent { span, parts })
        }
    }

    /// Event control after `@`: `*`, `(*)`, or `( [posedge|negedge] expr {or|, ...} )`.
    pub(crate) fn event_ctrl(&mut self, at_span: rcs_diag::Span) -> PResult<EventCtrl> {
        if self.eat_punct(Punct::Star) {
            return Ok(EventCtrl {
                span: at_span,
                edges: Vec::new(),
            });
        }
        if self.peek().kind == Tok::Ident {
            // `@ident` (event variable or signal) without parentheses.
            let e = self.primary_ident_only()?;
            let span = at_span.merge(e.span());
            return Ok(EventCtrl {
                span,
                edges: vec![EventEdge {
                    kind: EdgeKind::Any,
                    expr: e,
                }],
            });
        }
        self.expect_punct(Punct::LParen, "after `@`")?;
        if self.at_punct(Punct::Star) {
            self.pos += 1;
            let end = self.expect_punct(Punct::RParen, "after `@(*`")?;
            return Ok(EventCtrl {
                span: at_span.merge(end),
                edges: Vec::new(),
            });
        }
        let mut edges = Vec::new();
        loop {
            let kind = if self.eat_kw(Kw::Posedge) {
                EdgeKind::Posedge
            } else if self.eat_kw(Kw::Negedge) {
                EdgeKind::Negedge
            } else if self.at_kw(Kw::Edge) {
                let t = self.peek().clone();
                return self.err(
                    t.span,
                    "VL-E900",
                    "`edge` event control is not yet supported",
                );
            } else {
                EdgeKind::Any
            };
            let expr = self.expr()?;
            edges.push(EventEdge { kind, expr });
            if self.eat_kw(Kw::Or) || self.eat_punct(Punct::Comma) {
                continue;
            }
            break;
        }
        let end = self.expect_punct(Punct::RParen, "to close event control")?;
        Ok(EventCtrl {
            span: at_span.merge(end),
            edges,
        })
    }

    /// Expression-led statements: blocking/nonblocking assignment or task call.
    fn expr_stmt(&mut self) -> PResult<Stmt> {
        let lhs = self.expr()?;
        let span = lhs.span();
        if self.eat_punct(Punct::Eq) {
            if !lhs.is_lvalue() {
                return self.err(span, "VL-E021", "left side of `=` is not an lvalue");
            }
            self.reject_intra_assign()?;
            let rhs = self.expr()?;
            let end = self.expect_punct(Punct::Semi, "after assignment")?;
            return Ok(Stmt::BAssign {
                span: span.merge(end),
                lhs,
                rhs,
            });
        }
        match lhs {
            // `a <= b` parses as a `<=` comparison; at statement level it is
            // the nonblocking assignment (LRM §9.2.2).
            Expr::Binary {
                op: ast::BinOp::Le,
                lhs: l,
                rhs: r,
                span,
            } => {
                if !l.is_lvalue() {
                    return self.err(span, "VL-E021", "left side of `<=` is not an lvalue");
                }
                let end = self.expect_punct(Punct::Semi, "after nonblocking assignment")?;
                Ok(Stmt::NbAssign {
                    span: span.merge(end),
                    lhs: *l,
                    rhs: *r,
                })
            }
            Expr::Call { span, func, args } => {
                let end = self.expect_punct(Punct::Semi, "after task call")?;
                Ok(Stmt::SubCall {
                    span: span.merge(end),
                    name: func,
                    args,
                })
            }
            Expr::Ident { span, name } => {
                let end = self.expect_punct(Punct::Semi, "after task call")?;
                Ok(Stmt::SubCall {
                    span: span.merge(end),
                    name,
                    args: Vec::new(),
                })
            }
            Expr::HierIdent { span, parts } => {
                let end = self.expect_punct(Punct::Semi, "after task call")?;
                Ok(Stmt::SubCall {
                    span: span.merge(end),
                    name: parts.join("."),
                    args: Vec::new(),
                })
            }
            other => self.err(
                other.span(),
                "VL-E020",
                "expected a statement (assignment or task call)",
            ),
        }
    }
}

pub(crate) fn stmt_span(s: &Stmt) -> rcs_diag::Span {
    match s {
        Stmt::Block { span, .. }
        | Stmt::BAssign { span, .. }
        | Stmt::NbAssign { span, .. }
        | Stmt::If { span, .. }
        | Stmt::Case { span, .. }
        | Stmt::For { span, .. }
        | Stmt::While { span, .. }
        | Stmt::Repeat { span, .. }
        | Stmt::Forever { span, .. }
        | Stmt::DelayCtrl { span, .. }
        | Stmt::EventCtrl { span, .. }
        | Stmt::SysCall { span, .. }
        | Stmt::SubCall { span, .. }
        | Stmt::Disable { span, .. }
        | Stmt::Null { span } => *span,
    }
}

#[allow(dead_code)]
fn _assign_pair_mark(_: AssignPair) {}
