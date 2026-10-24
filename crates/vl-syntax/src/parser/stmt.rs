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
            Tok::Keyword(Kw::Wait) => {
                // R3-B relocation (design §5 方言注): the parse-level M8
                // error arms moved to Sim::build verbatim (vl-sim side);
                // the parser now produces the AST and riverilog lowers it.
                self.pos += 1;
                self.expect_punct(Punct::LParen, "after `wait`")?;
                let cond = self.expr()?;
                self.expect_punct(Punct::RParen, "to close `wait` condition")?;
                let body = if self.at_punct(Punct::Semi) {
                    self.pos += 1;
                    None
                } else {
                    Some(Box::new(self.stmt()?))
                };
                Ok(Stmt::Wait {
                    span: t.span.merge(cond.span()),
                    cond,
                    body,
                })
            }
            Tok::Keyword(Kw::Fork) => {
                self.pos += 1;
                let name = if self.eat_punct(Punct::Colon) {
                    let (n, _) = self.expect_ident("block name after `fork :`")?;
                    Some(n)
                } else {
                    None
                };
                let mut branches = Vec::new();
                loop {
                    self.skip_attrs();
                    if self.at_kw(Kw::Join) {
                        self.pos += 1;
                        break;
                    }
                    if self.at_eof() {
                        return self.err(
                            t.span,
                            "VL-E020",
                            "unterminated `fork` (missing `join`)".to_string(),
                        );
                    }
                    if let Tok::Ident = self.peek().kind {
                        // join_any/join_none lex as identifiers (SV keywords);
                        // reject with the R8 pointer instead of a confusing
                        // statement error.
                        if self.peek().text == "join_any" || self.peek().text == "join_none" {
                            return self.err(
                                self.peek().span,
                                "VL-E900",
                                format!("`{}` is SystemVerilog (milestone R8)", self.peek().text),
                            );
                        }
                    }
                    let before = self.pos;
                    match self.stmt() {
                        Ok(b) => branches.push(b),
                        Err(_) => {
                            self.sync_stmt();
                            if self.pos == before {
                                self.pos += 1;
                            }
                        }
                    }
                }
                let end = self.peek().span;
                Ok(Stmt::Fork {
                    span: t.span.merge(end),
                    name,
                    branches,
                })
            }
            Tok::Keyword(Kw::Force) => {
                self.pos += 1;
                let lhs = self.expr()?;
                self.expect_punct(Punct::Eq, "in `force lhs = rhs`")?;
                let rhs = self.expr()?;
                let end = self.expect_punct(Punct::Semi, "after `force` statement")?;
                Ok(Stmt::Force {
                    span: t.span.merge(end),
                    lhs,
                    rhs,
                })
            }
            Tok::Keyword(Kw::Release) | Tok::Keyword(Kw::Deassign) => {
                self.pos += 1;
                let lhs = self.expr()?;
                let end = self.expect_punct(Punct::Semi, "after statement")?;
                Ok(if t.is_kw(Kw::Release) {
                    Stmt::Release { span: t.span.merge(end), lhs }
                } else {
                    Stmt::Deassign { span: t.span.merge(end), lhs }
                })
            }
            Tok::Keyword(Kw::Assign) => {
                // Procedural continuous assignment: `assign lhs = rhs;`
                self.pos += 1;
                let lhs = self.expr()?;
                self.expect_punct(Punct::Eq, "in procedural `assign lhs = rhs`")?;
                let rhs = self.expr()?;
                let end = self.expect_punct(Punct::Semi, "after procedural `assign`")?;
                Ok(Stmt::ProcContAssign {
                    span: t.span.merge(end),
                    lhs,
                    rhs,
                })
            }
            Tok::Keyword(Kw::Disable) => {
                self.pos += 1;
                let (name, _) = self.expect_ident("block name after `disable`")?;
                let end = self.expect_punct(Punct::Semi, "after `disable`")?;
                Ok(Stmt::Disable {
                    span: t.span.merge(end),
                    name,
                })
            }
            Tok::Punct(Punct::Arrow) => {
                self.pos += 1;
                let (name, _) = self.expect_ident("event name after `->`")?;
                let end = self.expect_punct(Punct::Semi, "after `-> event`")?;
                Ok(Stmt::Trigger {
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
        let mut decls = Vec::new();
        let mut body = Vec::new();
        loop {
            self.skip_attrs();
            if self.at_kw(Kw::End) {
                break;
            }
            if self.at_eof() {
                return self.err(start, "VL-E020", "unterminated `begin` block (missing `end`)");
            }
            // Block-local declarations (reg/integer/...) — legal in named
            // blocks (§9.8); scoped to the block for name resolution.
            if self.peek_is_var_decl_kw() {
                let t = self.peek().clone();
                let (kind, ty) = match t.kind {
                    Tok::Keyword(Kw::Integer) => (ast::DeclKind::Integer, Some(ast::ParamType::Integer)),
                    Tok::Keyword(Kw::Real) => (ast::DeclKind::Real, Some(ast::ParamType::Real)),
                    Tok::Keyword(Kw::Realtime) => (ast::DeclKind::Realtime, Some(ast::ParamType::Realtime)),
                    Tok::Keyword(Kw::Time) => (ast::DeclKind::Time, Some(ast::ParamType::Time)),
                    _ => (ast::DeclKind::Reg, None),
                };
                self.pos += 1;
                let decl = self.decl_tail(t.span, kind, ty, None)?;
                decls.push(decl);
                continue;
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
            decls,
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
        if self.at_punct(Punct::AttrOpen) {
            // `@(*)` lexes `(*` as an attribute-open token; at statement
            // level the star event control is the only reading.
            self.pos += 1;
            let end = self.expect_punct(Punct::RParen, "after `@(*`")?;
            return Ok(EventCtrl {
                span: at_span.merge(end),
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
    ///
    /// Assignments are lvalue-led (V2005 §9.2), so parse a unary (primary +
    /// postfix) first and dispatch on the `=` / `<=` that follows; a
    /// binary-leading form can never be an assignment (`a <= b ? c : d` and
    /// `mem[i] <= i == 0 ? x : y` both hide the `<=` under operators with
    /// lower precedence). Anything else rewinds to the full-expression paths
    /// (task calls, bare identifiers).
    fn expr_stmt(&mut self) -> PResult<Stmt> {
        let save = self.pos;
        let is_nb = if let Ok(lhs) = self.unary() {
            if self.at_punct(Punct::Eq) || self.at_punct(Punct::Le) {
                let nb = self.at_punct(Punct::Le);
                if !lhs.is_lvalue() {
                    return self.err(
                        lhs.span(),
                        "VL-E021",
                        "left side of assignment is not an lvalue",
                    );
                }
                self.pos += 1;
                if self.at_punct(Punct::Hash) {
                    // `a = #5 v;` — intra-assignment delay (R4-e).
                    self.pos += 1;
                    let delay = self.delay_val()?;
                    let rhs = self.expr()?;
                    let end = self.expect_punct(Punct::Semi, "after assignment")?;
                    return Ok(Stmt::IntraAssign {
                        span: lhs.span().merge(end),
                        lhs,
                        rhs,
                        delay,
                        nba: nb,
                    });
                }
                self.reject_intra_assign()?; // `= @(...) v` stays rejected
                let rhs = self.expr()?;
                let end = self.expect_punct(Punct::Semi, "after assignment")?;
                let span = lhs.span().merge(end);
                return Ok(if nb {
                    Stmt::NbAssign { span, lhs, rhs }
                } else {
                    Stmt::BAssign { span, lhs, rhs }
                });
            }
            false
        } else {
            false
        };
        let _ = is_nb;
        self.pos = save;
        let lhs = self.expr()?;
        let span = lhs.span();
        if self.eat_punct(Punct::Eq) {
            if !lhs.is_lvalue() {
                return self.err(span, "VL-E021", "left side of `=` is not an lvalue");
            }
            if self.at_punct(Punct::Hash) {
                self.pos += 1;
                let delay = self.delay_val()?;
                let rhs = self.expr()?;
                let end = self.expect_punct(Punct::Semi, "after assignment")?;
                return Ok(Stmt::IntraAssign {
                    span: span.merge(end),
                    lhs,
                    rhs,
                    delay,
                    nba: false,
                });
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
            Expr::Ternary {
                span,
                cond,
                then,
                els,
            } => {
                // `a <= b ? c : d` parses as a comparison-led ternary; at
                // statement level the `<=` is the nonblocking assignment and
                // the whole ternary is its rhs (V2005 has no expression
                // statements, so the reading is unambiguous).
                let nb = match &*cond {
                    Expr::Binary {
                        op: ast::BinOp::Le,
                        lhs: l,
                        rhs: r,
                        span: ls,
                    } => {
                        if !l.is_lvalue() {
                            return self.err(*ls, "VL-E021", "left side of `<=` is not an lvalue");
                        }
                        Some((
                            (**l).clone(),
                            Expr::Ternary {
                                span,
                                cond: r.clone(),
                                then,
                                els,
                            },
                            *ls,
                        ))
                    }
                    _ => None,
                };
                match nb {
                    Some((l, rhs, ls)) => {
                        let end = self.expect_punct(Punct::Semi, "after nonblocking assignment")?;
                        Ok(Stmt::NbAssign {
                            span: ls.merge(end),
                            lhs: l,
                            rhs,
                        })
                    }
                    None => self.err(
                        span,
                        "VL-E020",
                        "expected a statement (assignment or task call)",
                    ),
                }
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
        | Stmt::Trigger { span, .. }
        | Stmt::IntraAssign { span, .. }
        | Stmt::Disable { span, .. }
        | Stmt::Wait { span, .. }
        | Stmt::Fork { span, .. }
        | Stmt::Force { span, .. }
        | Stmt::Release { span, .. }
        | Stmt::Deassign { span, .. }
        | Stmt::ProcContAssign { span, .. }
        | Stmt::Null { span } => *span,
    }
}

#[allow(dead_code)]
fn _assign_pair_mark(_: AssignPair) {}
