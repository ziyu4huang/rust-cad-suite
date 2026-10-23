//! Expression parsing: precedence climbing per IEEE 1364-2005 §A.8.2.

use super::{PResult, Parser};
use crate::ast::{Base, BinOp, Expr, NumLit, UnOp};
use crate::token::{Punct, Tok};

/// Binary operator precedence levels, loosest first (LRM Table 5-1... §A.8.2
/// associativity). `**` is right-associative; everything else left.
const LEVELS: usize = 11;

impl<'d> Parser<'d> {
    pub(crate) fn expr(&mut self) -> PResult<Expr> {
        self.ternary()
    }

    fn ternary(&mut self) -> PResult<Expr> {
        let cond = self.binary(0)?;
        if self.eat_punct(Punct::Question) {
            let start = cond.span();
            let t = self.expr()?;
            self.expect_punct(Punct::Colon, "in `?:` expression")?;
            let e = self.expr()?;
            return Ok(Expr::Ternary {
                span: start.merge(e.span()),
                cond: Box::new(cond),
                then: Box::new(t),
                els: Box::new(e),
            });
        }
        Ok(cond)
    }

    fn binary(&mut self, level: usize) -> PResult<Expr> {
        if level >= LEVELS {
            return self.unary();
        }
        let mut lhs = self.binary(level + 1)?;
        loop {
            let t = self.peek().clone();
            let Some((op, negate)) = op_at_level(level, &t) else {
                return Ok(lhs);
            };
            self.pos += 1;
            let rhs = if op == BinOp::Pow {
                self.binary(level)? // right-assoc
            } else {
                self.binary(level + 1)?
            };
            let span = lhs.span().merge(rhs.span());
            let core = Expr::Binary {
                span,
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
            // `~&` / `~|` / `~^`, `^~` share precedence with their base op and
            // mean "negated result" (LRM Table 5-1); keep exact via Unary::Not.
            lhs = if negate {
                Expr::Unary {
                    span,
                    op: UnOp::BitNot,
                    expr: Box::new(core),
                }
            } else {
                core
            };
        }
    }

    pub(crate) fn unary(&mut self) -> PResult<Expr> {
        let t = self.peek().clone();
        let op = match t.kind {
            Tok::Punct(Punct::Bang) => Some(UnOp::Not),
            Tok::Punct(Punct::Tilde) => Some(UnOp::BitNot),
            Tok::Punct(Punct::Plus) => Some(UnOp::Plus),
            Tok::Punct(Punct::Minus) => Some(UnOp::Minus),
            Tok::Punct(Punct::Amp) => Some(UnOp::And),
            Tok::Punct(Punct::Nand) => Some(UnOp::Nand),
            Tok::Punct(Punct::Pipe) => Some(UnOp::Or),
            Tok::Punct(Punct::Nor) => Some(UnOp::Nor),
            Tok::Punct(Punct::Caret) => Some(UnOp::Xor),
            Tok::Punct(Punct::Xnor) => Some(UnOp::Xnor),
            _ => None,
        };
        if let Some(op) = op {
            self.pos += 1;
            let e = self.unary()?;
            let span = t.span.merge(e.span());
            return Ok(Expr::Unary {
                span,
                op,
                expr: Box::new(e),
            });
        }
        let prim = self.primary()?;
        self.postfix(prim)
    }

    fn primary(&mut self) -> PResult<Expr> {
        self.skip_attrs();
        let t = self.peek().clone();
        match t.kind {
            Tok::IntLit => {
                self.pos += 1;
                // Unsized, unbased integers are signed (IEEE 1364-2005 §4.8).
                Ok(Expr::NumLit(NumLit {
                    span: t.span,
                    width: None,
                    signed: true,
                    base: Base::Dec,
                    digits: t.text,
                }))
            }
            Tok::BasedLit => {
                self.pos += 1;
                Ok(Expr::NumLit(parse_based(&t.text, t.span)))
            }
            Tok::RealLit => {
                self.pos += 1;
                Ok(Expr::RealLit {
                    span: t.span,
                    text: t.text,
                })
            }
            Tok::StrLit => {
                self.pos += 1;
                Ok(Expr::StrLit {
                    span: t.span,
                    value: t.text,
                })
            }
            Tok::SysIdent => {
                self.pos += 1;
                let args = if self.at_punct(Punct::LParen) {
                    self.paren_expr_list()?
                } else {
                    Vec::new()
                };
                Ok(Expr::Call {
                    span: t.span,
                    func: t.text,
                    args,
                })
            }
            Tok::Punct(Punct::LParen) => {
                self.pos += 1;
                let e = self.expr()?;
                self.expect_punct(Punct::RParen, "to close parenthesized expression")?;
                Ok(e)
            }
            Tok::Punct(Punct::LBrace) => self.concat_or_repeat(),
            Tok::Ident => {
                self.pos += 1;
                let mut parts = vec![t.text.clone()];
                while self.at_punct(Punct::Dot) && self.peek_at(1).kind == Tok::Ident {
                    self.pos += 2;
                    parts.push(self.toks[self.pos - 1].text.clone());
                }
                let end_span = self.toks[self.pos - 1].span;
                let span = t.span.merge(end_span);
                if self.at_punct(Punct::LParen) {
                    let args = self.paren_expr_list()?;
                    Ok(Expr::Call {
                        span,
                        func: parts.join("."),
                        args,
                    })
                } else if parts.len() == 1 {
                    Ok(Expr::Ident {
                        span,
                        name: t.text,
                    })
                } else {
                    Ok(Expr::HierIdent { span, parts })
                }
            }
            _ => self.err(
                t.span,
                "VL-E020",
                format!("expected expression, found {}", t.describe()),
            ),
        }
    }

    /// `( expr, expr, ... )` argument list.
    pub(crate) fn paren_expr_list(&mut self) -> PResult<Vec<Expr>> {
        self.expect_punct(Punct::LParen, "for argument list")?;
        let mut args = Vec::new();
        if !self.at_punct(Punct::RParen) {
            loop {
                args.push(self.expr()?);
                if !self.eat_punct(Punct::Comma) {
                    break;
                }
            }
        }
        self.expect_punct(Punct::RParen, "to close argument list")?;
        Ok(args)
    }

    fn concat_or_repeat(&mut self) -> PResult<Expr> {
        let open = self.expect_punct(Punct::LBrace, "for concatenation")?;
        if self.eat_punct(Punct::RBrace) {
            return self.err(open, "VL-E020", "empty concatenation `{}` is not allowed");
        }
        let first = self.expr()?;
        if self.at_punct(Punct::LBrace) {
            // replication {count{concat}} — concat_parts consumes the inner
            // braces, leaving one `}` for us.
            let inner = self.concat_parts()?;
            let close = self.expect_punct(Punct::RBrace, "to close replication")?;
            let value = if inner.len() == 1 {
                inner.into_iter().next().unwrap()
            } else {
                Expr::Concat {
                    span: open.merge(close),
                    parts: inner,
                }
            };
            return Ok(Expr::Repeat {
                span: open.merge(close),
                count: Box::new(first),
                value: Box::new(value),
            });
        }
        let mut parts = vec![first];
        while self.eat_punct(Punct::Comma) {
            parts.push(self.expr()?);
        }
        let close = self.expect_punct(Punct::RBrace, "to close concatenation")?;
        self.postfix(Expr::Concat {
            span: open.merge(close),
            parts,
        })
    }

    /// `{ expr, expr, ... }` used inside a replication.
    fn concat_parts(&mut self) -> PResult<Vec<Expr>> {
        self.expect_punct(Punct::LBrace, "for replication body")?;
        let mut parts = Vec::new();
        loop {
            parts.push(self.expr()?);
            if !self.eat_punct(Punct::Comma) {
                break;
            }
        }
        self.expect_punct(Punct::RBrace, "to close replication body")?;
        Ok(parts)
    }

    /// Indexing / part-select postfixes: `a[i]`, `a[3:0]`, `a[b+:8]`.
    fn postfix(&mut self, mut e: Expr) -> PResult<Expr> {
        loop {
            if self.at_punct(Punct::Dot)
                && matches!(
                    e,
                    Expr::Ident { .. }
                        | Expr::HierIdent { .. }
                        | Expr::Index { .. }
                        | Expr::PartSelect { .. }
                        | Expr::HierDot { .. }
                )
            {
                // `.name` after a (possibly indexed) identifier: hierarchical
                // reference into a generate block or instance
                // (`netgen[4].in`, `blk[I-1].z`, `A.B.x` tail).
                self.pos += 1;
                let (nm, nspan) = self.expect_ident("hierarchical name")?;
                let span = e.span().merge(nspan);
                e = Expr::HierDot {
                    span,
                    base: Box::new(e),
                    name: nm,
                };
                continue;
            }
            if !self.at_punct(Punct::LBracket) {
                return Ok(e);
            }
            self.pos += 1;
            let first = self.expr()?;
            if self.eat_punct(Punct::Colon) {
                let lsb = self.expr()?;
                let close = self.expect_punct(Punct::RBracket, "to close part-select")?;
                let span = e.span().merge(close);
                e = Expr::PartSelect {
                    span,
                    base: Box::new(e),
                    msb: Box::new(first),
                    lsb: Box::new(lsb),
                };
            } else if self.eat_punct(Punct::PlusColon) || self.eat_punct(Punct::MinusColon) {
                let plus = self.toks[self.pos - 1].is_punct(Punct::PlusColon);
                let width = self.expr()?;
                let close = self.expect_punct(Punct::RBracket, "to close indexed part-select")?;
                let span = e.span().merge(close);
                e = Expr::IndexedPart {
                    span,
                    base: Box::new(e),
                    offset: Box::new(first),
                    width: Box::new(width),
                    plus,
                };
            } else {
                let close = self.expect_punct(Punct::RBracket, "to close index")?;
                let span = e.span().merge(close);
                e = Expr::Index {
                    span,
                    base: Box::new(e),
                    index: Box::new(first),
                };
            }
        }
    }
}

/// `(BinOp, negate)` for a punct at a precedence level. `negate` marks
/// `~&`, `~|`, `~^`, `^~` (result-negated forms of the base op).
fn op_at_level(level: usize, t: &crate::token::Token) -> Option<(BinOp, bool)> {
    let p = match t.kind {
        Tok::Punct(p) => p,
        _ => return None,
    };
    use crate::token::Punct;
    let op = match (level, p) {
        (0, Punct::LogOr) => BinOp::LogOr,
        (1, Punct::LogAnd) => BinOp::LogAnd,
        (2, Punct::Pipe) => BinOp::BitOr,
        (3, Punct::Caret) => BinOp::BitXor,
        (3, Punct::Xnor) => BinOp::BitXor, // ^~ / ~^ (binary xnor is legal)
        // NOTE: binary `~&`/`~|` do NOT exist in IEEE 1364-2005 Annex A —
        // the oracle (Verilator) rejects them; `nand`/`nor` are unary only.
        (4, Punct::Amp) => BinOp::BitAnd,
        (5, Punct::EqEq) => BinOp::Eq,
        (5, Punct::NotEq) => BinOp::Ne,
        (5, Punct::EqEqEq) => BinOp::EqCase,
        (5, Punct::NotEqEq) => BinOp::NeCase,
        (6, Punct::Lt) => BinOp::Lt,
        (6, Punct::Le) => BinOp::Le,
        (6, Punct::Gt) => BinOp::Gt,
        (6, Punct::Ge) => BinOp::Ge,
        (7, Punct::Shl) => BinOp::Shl,
        (7, Punct::Shr) => BinOp::Shr,
        (7, Punct::AShl) => BinOp::AShl,
        (7, Punct::AShr) => BinOp::AShr,
        (8, Punct::Plus) => BinOp::Add,
        (8, Punct::Minus) => BinOp::Sub,
        (9, Punct::Star) => BinOp::Mul,
        (9, Punct::Slash) => BinOp::Div,
        (9, Punct::Percent) => BinOp::Mod,
        (10, Punct::Pow) => BinOp::Pow,
        _ => return None,
    };
    let negate = matches!((level, p), (3, Punct::Xnor));
    Some((op, negate))
}

/// Parse a `BasedLit` token text (`8'sh3f`, `'b01x`) into a NumLit.
pub fn parse_based(text: &str, span: rcs_diag::Span) -> NumLit {
    let Some(q) = text.find('\'') else {
        // Unreachable for lexer output; treat as decimal to stay total.
        return NumLit {
            span,
            width: None,
            signed: false,
            base: Base::Dec,
            digits: text.to_string(),
        };
    };
    let width = text[..q].parse::<u32>().ok();
    let rest = &text[q + 1..];
    let (signed, rest) = match rest.as_bytes().first() {
        Some(b's') | Some(b'S') => (true, &rest[1..]),
        _ => (false, rest),
    };
    // Fill literal `'0 `'1 `'x `'z`: 1-bit signed — sign extension fills
    // the context width (zeros/all-ones/all-x/all-z respectively).
    if width.is_none()
        && rest.len() == 1
        && matches!(rest, "0" | "1" | "x" | "X" | "z" | "Z")
    {
        return NumLit {
            span,
            width: None,
            signed: true,
            base: Base::Bin,
            digits: rest.to_ascii_lowercase(),
        };
    }
    let (base, digits) = match rest.chars().next() {
        Some(c) if c.to_ascii_lowercase() == 'd' => (Base::Dec, &rest[1..]),
        Some(c) if c.to_ascii_lowercase() == 'b' => (Base::Bin, &rest[1..]),
        Some(c) if c.to_ascii_lowercase() == 'o' => (Base::Oct, &rest[1..]),
        Some(c) if c.to_ascii_lowercase() == 'h' => (Base::Hex, &rest[1..]),
        _ => (Base::Dec, rest), // lexer guarantees this never happens
    };
    NumLit {
        span,
        width,
        signed,
        base,
        digits: digits.to_string(),
    }
}

/// Keywords that cannot start an expression (used by stmt dispatch).
pub(crate) fn starts_expr_stmt(t: &crate::token::Token) -> bool {
    matches!(
        t.kind,
        Tok::Ident
            | Tok::IntLit
            | Tok::BasedLit
            | Tok::RealLit
            | Tok::StrLit
            | Tok::SysIdent
            | Tok::Punct(Punct::LParen)
            | Tok::Punct(Punct::LBrace)
            | Tok::Punct(Punct::Plus)
            | Tok::Punct(Punct::Minus)
            | Tok::Punct(Punct::Bang)
            | Tok::Punct(Punct::Tilde)
            | Tok::Punct(Punct::Amp)
            | Tok::Punct(Punct::Nand)
            | Tok::Punct(Punct::Pipe)
            | Tok::Punct(Punct::Nor)
            | Tok::Punct(Punct::Caret)
            | Tok::Punct(Punct::Xnor)
    )
}
