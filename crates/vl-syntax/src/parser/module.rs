//! Module-level parsing: headers, port lists, items, instances, generate
//! constructs, functions/tasks.

use super::stmt::stmt_span;
use super::{PResult, Parser};
use crate::ast::{self, AssignPair, Direction, Expr, ModuleItem, ParamType};
use crate::token::{Kw, Punct, Tok};
use rcs_diag::Span;

impl<'d> Parser<'d> {
    pub(crate) fn module(&mut self) -> PResult<ast::Module> {
        self.skip_attrs();
        let start = self.bump().span; // `module`
        let (name, _) = self.expect_ident("module name")?;

        // Header parameter list: #( parameter [type] [signed] [range] N = e , ... )
        let mut params = Vec::new();
        if self.eat_punct(Punct::Hash) {
            self.expect_punct(Punct::LParen, "after `#`")?;
            if !self.at_punct(Punct::RParen) {
                loop {
                    let pspan = self.peek().span;
                    if self.at_kw(Kw::Parameter) || self.at_kw(Kw::Localparam) {
                        self.pos += 1;
                    }
                    let mut ty: Option<ParamType> = None;
                    match self.peek().kind {
                        Tok::Keyword(Kw::Integer) => {
                            self.pos += 1;
                            ty = Some(ParamType::Integer);
                        }
                        Tok::Keyword(Kw::Real) => {
                            self.pos += 1;
                            ty = Some(ParamType::Real);
                        }
                        Tok::Keyword(Kw::Time) => {
                            self.pos += 1;
                            ty = Some(ParamType::Time);
                        }
                        Tok::Keyword(Kw::Realtime) => {
                            self.pos += 1;
                            ty = Some(ParamType::Realtime);
                        }
                        _ => {}
                    }
                    let mut signed = false;
                    if self.eat_kw(Kw::Signed) {
                        signed = true;
                    }
                    let range = if self.at_punct(Punct::LBracket) {
                        Some(self.range()?)
                    } else {
                        None
                    };
                    let (pname, _) = self.expect_ident("parameter name")?;
                    self.expect_punct(Punct::Eq, "after parameter name")?;
                    let init = self.expr()?;
                    params.push(ast::ParamEntry {
                        span: pspan,
                        name: pname,
                        ty,
                        signed,
                        range,
                        init,
                    });
                    if !self.eat_punct(Punct::Comma) {
                        break;
                    }
                }
            }
            self.expect_punct(Punct::RParen, "to close parameter port list")?;
        }

        let (ports, ansi) = self.port_list()?;
        self.expect_punct(Punct::Semi, "after module header")?;

        let mut items = Vec::new();
        let end_span = loop {
            self.skip_attrs();
            let es = self.peek().span;
            if self.eat_kw(Kw::Endmodule) {
                if self.eat_punct(Punct::Colon) {
                    let _ = self.expect_ident("name after `endmodule :`")?;
                }
                break es;
            }
            if self.at_eof() {
                return self.err(
                    start,
                    "VL-E020",
                    format!("unterminated module `{name}` (missing `endmodule`)"),
                );
            }
            let before = self.pos;
            match self.module_item() {
                Ok(Some(it)) => items.push(it),
                Ok(None) => {}
                Err(super::ParseAbort) => {
                    self.sync_item();
                    if self.pos == before {
                        // sync stopped on a structural keyword it won't
                        // consume — force progress (INC hang fix).
                        self.pos += 1;
                    }
                }
            }
        };

        Ok(ast::Module {
            span: start.merge(end_span),
            name,
            params,
            ports,
            ansi,
            items,
        })
    }

    /// Port list after the module name. Returns ports + ANSI flag.
    fn port_list(&mut self) -> PResult<(Vec<ast::Port>, bool)> {
        if !self.at_punct(Punct::LParen) {
            return Ok((Vec::new(), true));
        }
        self.pos += 1;
        if self.eat_punct(Punct::RParen) {
            return Ok((Vec::new(), true));
        }
        self.skip_attrs(); // a leading attribute does not make the list non-ANSI
        let ansi = self.peek_starts_port_spec();
        let mut ports = Vec::new();
        if ansi {
            let mut cur_dir: Option<Direction> = None;
            let mut cur_net: Option<ast::NetKind> = None;
            let mut cur_reg = false;
            let mut cur_signed = false;
            let mut cur_range: Option<ast::Range> = None;
            'groups: loop {
                self.skip_attrs(); // attributes may precede each port group
                // Group specifiers; omitted ones inherit the previous group
                // (LRM §12.3.3).
                loop {
                    match self.peek().kind {
                        Tok::Keyword(Kw::Input) => {
                            self.pos += 1;
                            cur_dir = Some(Direction::Input);
                        }
                        Tok::Keyword(Kw::Output) => {
                            self.pos += 1;
                            cur_dir = Some(Direction::Output);
                        }
                        Tok::Keyword(Kw::Inout) => {
                            self.pos += 1;
                            cur_dir = Some(Direction::Inout);
                        }
                        Tok::Keyword(k) if ast::NetKind::from_kw(k).is_some() => {
                            self.pos += 1;
                            cur_net = ast::NetKind::from_kw(k);
                            cur_reg = false;
                        }
                        Tok::Keyword(Kw::Reg) => {
                            self.pos += 1;
                            cur_reg = true;
                            cur_net = None;
                        }
                        Tok::Keyword(Kw::Signed) => {
                            self.pos += 1;
                            cur_signed = true;
                        }
                        Tok::Punct(Punct::LBracket) => {
                            cur_range = Some(self.range()?);
                        }
                        _ => break,
                    }
                }
                let Some(dir) = cur_dir else {
                    return self.err(
                        self.peek().span,
                        "VL-E020",
                        "first ANSI port must declare a direction",
                    );
                };
                // One or more names in this group.
                loop {
                    self.skip_attrs();
                    let (name, nspan) = self.expect_ident("port name")?;
                    ports.push(ast::Port {
                        span: nspan,
                        name,
                        dir: Some(dir),
                        net: cur_net,
                        is_reg: cur_reg,
                        signed: cur_signed,
                        range: cur_range.clone(),
                    });
                    if self.eat_punct(Punct::Comma) {
                        if self.peek_starts_port_spec() {
                            continue 'groups;
                        }
                        continue;
                    }
                    break 'groups;
                }
            }
        } else {
            loop {
                if self.at_punct(Punct::Dot) {
                    return self.err(
                        self.peek().span,
                        "VL-E900",
                        "port expression connections `.name(expr)` are not yet supported (milestone M10)",
                    );
                }
                let (name, nspan) = self.expect_ident("port name")?;
                ports.push(ast::Port {
                    span: nspan,
                    name,
                    dir: None,
                    net: None,
                    is_reg: false,
                    signed: false,
                    range: None,
                });
                if !self.eat_punct(Punct::Comma) {
                    break;
                }
            }
        }
        self.expect_punct(Punct::RParen, "to close port list")?;
        Ok((ports, ansi))
    }

    pub(crate) fn module_item(&mut self) -> PResult<Option<ModuleItem>> {
        self.skip_attrs();
        let t = self.peek().clone();
        match t.kind {
            Tok::Keyword(k) => {
                if let Some(nk) = ast::NetKind::from_kw(k) {
                    self.pos += 1;
                    return Ok(Some(ModuleItem::Decl(
                        self.decl_tail(t.span, ast::DeclKind::Net(nk), None, None)?,
                    )));
                }
                match k {
                    Kw::Reg => {
                        self.pos += 1;
                        Ok(Some(ModuleItem::Decl(
                            self.decl_tail(t.span, ast::DeclKind::Reg, None, None)?,
                        )))
                    }
                    Kw::Integer => {
                        self.pos += 1;
                        Ok(Some(ModuleItem::Decl(
                            self.decl_tail(t.span, ast::DeclKind::Integer, Some(ParamType::Integer), None)?,
                        )))
                    }
                    Kw::Real => {
                        self.pos += 1;
                        Ok(Some(ModuleItem::Decl(
                            self.decl_tail(t.span, ast::DeclKind::Real, Some(ParamType::Real), None)?,
                        )))
                    }
                    Kw::Realtime => {
                        self.pos += 1;
                        Ok(Some(ModuleItem::Decl(
                            self.decl_tail(t.span, ast::DeclKind::Realtime, Some(ParamType::Realtime), None)?,
                        )))
                    }
                    Kw::Time => {
                        self.pos += 1;
                        Ok(Some(ModuleItem::Decl(
                            self.decl_tail(t.span, ast::DeclKind::Time, Some(ParamType::Time), None)?,
                        )))
                    }
                    Kw::Genvar => {
                        self.pos += 1;
                        Ok(Some(ModuleItem::Decl(
                            self.decl_tail(t.span, ast::DeclKind::Genvar, None, None)?,
                        )))
                    }
                    Kw::Event => {
                        self.pos += 1;
                        Ok(Some(ModuleItem::Decl(
                            self.decl_tail(t.span, ast::DeclKind::Event, None, None)?,
                        )))
                    }
                    Kw::Parameter | Kw::Localparam => {
                        self.pos += 1;
                        let kind = if k == Kw::Parameter {
                            ast::DeclKind::Parameter
                        } else {
                            ast::DeclKind::Localparam
                        };
                        let mut ty: Option<ParamType> = None;
                        match self.peek().kind {
                            Tok::Keyword(Kw::Integer) => {
                                self.pos += 1;
                                ty = Some(ParamType::Integer);
                            }
                            Tok::Keyword(Kw::Real) => {
                                self.pos += 1;
                                ty = Some(ParamType::Real);
                            }
                            Tok::Keyword(Kw::Time) => {
                                self.pos += 1;
                                ty = Some(ParamType::Time);
                            }
                            Tok::Keyword(Kw::Realtime) => {
                                self.pos += 1;
                                ty = Some(ParamType::Realtime);
                            }
                            _ => {}
                        }
                        Ok(Some(ModuleItem::Decl(
                            self.decl_tail(t.span, kind, ty, None)?,
                        )))
                    }
                    Kw::Input | Kw::Output | Kw::Inout => {
                        self.pos += 1;
                        let dir = match k {
                            Kw::Input => Direction::Input,
                            Kw::Output => Direction::Output,
                            _ => Direction::Inout,
                        };
                        Ok(Some(ModuleItem::Decl(self.dir_decl(dir, t.span)?)))
                    }
                    Kw::Assign => {
                        self.pos += 1;
                        self.cont_assign(t.span)
                    }
                    Kw::Defparam => {
                        self.pos += 1;
                        let pairs = self.assign_pairs()?;
                        self.expect_punct(Punct::Semi, "after defparam")?;
                        Ok(Some(ModuleItem::Defparam(pairs)))
                    }
                    Kw::Initial => {
                        self.pos += 1;
                        let body = self.stmt()?;
                        Ok(Some(ModuleItem::Initial {
                            span: t.span.merge(stmt_span(&body)),
                            body,
                        }))
                    }
                    Kw::Always => {
                        self.pos += 1;
                        let ctrl = if self.at_punct(Punct::At) {
                            let at_span = self.peek().span;
                            self.pos += 1;
                            Some(self.event_ctrl(at_span)?)
                        } else {
                            None
                        };
                        let body = self.stmt()?;
                        Ok(Some(ModuleItem::Always {
                            span: t.span.merge(stmt_span(&body)),
                            ctrl,
                            body,
                        }))
                    }
                    Kw::Generate => {
                        self.pos += 1;
                        self.generate_region(t.span)
                    }
                    // V2005 allows if/for/case generate constructs directly
                    // as module items (module_or_generate_item, §12.3).
                    Kw::If | Kw::For | Kw::Case | Kw::Casex | Kw::Casez => {
                        self.generate_body_item()
                    }
                    Kw::Begin => {
                        // A (named) begin block at module level is a
                        // module_or_generate_item — gen_items parses
                        // begin..end and constructs the GenBlock itself.
                        let mut its = self.gen_items()?;
                        match its.len() {
                            0 => Ok(Some(ast::ModuleItem::Empty)),
                            1 => Ok(Some(its.remove(0))),
                            _ => self.err(
                                t.span,
                                "VL-E020",
                                "multiple items in module-level begin block",
                            ),
                        }
                    }
                    Kw::Function => Ok(Some(ModuleItem::Function(Box::new(
                        self.function_or_task(false)?,
                    )))),
                    Kw::Task => Ok(Some(ModuleItem::Task(Box::new(
                        self.function_or_task(true)?,
                    )))),
                    Kw::Specify => {
                        // Specify blocks are ignored for simulation purposes
                        // (upstream lint tolerates them; contents dropped).
                        self.pos += 1;
                        let mut depth = 1usize;
                        while depth > 0 && !self.at_eof() {
                            if self.at_kw(Kw::Specify) {
                                depth += 1;
                            }
                            if self.eat_kw(Kw::Endspecify) {
                                depth -= 1;
                            } else {
                                self.pos += 1;
                            }
                        }
                        Ok(Some(ast::ModuleItem::Empty))
                    }
                    Kw::Specparam => {
                        // SDF timing params are simulation-inert here: skip
                        // the declaration up to and including the `;`.
                        while !self.at_eof() && !self.eat_punct(Punct::Semi) {
                            self.pos += 1;
                        }
                        Ok(Some(ast::ModuleItem::Empty))
                    }
                    Kw::Primitive | Kw::Table | Kw::Config
                    | Kw::Library => self.err(
                        t.span,
                        "VL-E900",
                        format!("`{}` constructs are not yet supported", k.as_str()),
                    ),
                    _ if k.is_gate() => self.gate_instance(k),
                    _ => self.err(
                        t.span,
                        "VL-E020",
                        format!("expected module item, found keyword `{}`", k.as_str()),
                    ),
                }
            }
            Tok::Ident => Ok(Some(ModuleItem::Instance(self.instance()?))),
            Tok::Punct(Punct::Semi) => {
                self.pos += 1;
                Ok(None)
            }
            Tok::Error => {
                self.pos += 1;
                Ok(None)
            }
            _ => self.err(
                t.span,
                "VL-E020",
                format!("expected module item, found {}", t.describe()),
            ),
        }
    }

    fn cont_assign(&mut self, _span: Span) -> PResult<Option<ModuleItem>> {
        if self.at_punct(Punct::Hash) {
            return self.err(
                self.peek().span,
                "VL-E900",
                "assign delays are not yet supported (milestone M8)",
            );
        }
        // Net strengths: accepted and dropped in v0.
        while matches!(
            self.peek().kind,
            Tok::Keyword(Kw::Supply0)
                | Tok::Keyword(Kw::Supply1)
                | Tok::Keyword(Kw::Strong0)
                | Tok::Keyword(Kw::Strong1)
                | Tok::Keyword(Kw::Pull0)
                | Tok::Keyword(Kw::Pull1)
                | Tok::Keyword(Kw::Weak0)
                | Tok::Keyword(Kw::Weak1)
                | Tok::Keyword(Kw::Highz0)
                | Tok::Keyword(Kw::Highz1)
                | Tok::Keyword(Kw::Small)
                | Tok::Keyword(Kw::Medium)
                | Tok::Keyword(Kw::Large)
        ) {
            self.pos += 1;
        }
        let pairs = self.assign_pairs()?;
        self.expect_punct(Punct::Semi, "after assign")?;
        Ok(Some(ModuleItem::ContAssign(pairs)))
    }

    /// Module/cell instance: `name [#(params)] inst [range] (conns) {, ...} ;`
    fn instance(&mut self) -> PResult<ast::Instance> {
        let start = self.peek().span;
        let (mod_name, _) = self.expect_ident("module name")?;
        let mut params = Vec::new();
        if self.eat_punct(Punct::Hash) {
            self.expect_punct(Punct::LParen, "after `#`")?;
            if !self.at_punct(Punct::RParen) {
                loop {
                    let pspan = self.peek().span;
                    if self.at_punct(Punct::Dot) {
                        self.pos += 1;
                        let (n, _) = self.expect_ident("parameter name")?;
                        self.expect_punct(Punct::LParen, "after `.`param")?;
                        let e = if self.at_punct(Punct::RParen) {
                            None
                        } else {
                            Some(self.expr()?)
                        };
                        self.expect_punct(Punct::RParen, "to close parameter value")?;
                        params.push(ast::ParamConn {
                            span: pspan,
                            name: Some(n),
                            expr: e,
                        });
                    } else {
                        let e = self.expr()?;
                        params.push(ast::ParamConn {
                            span: pspan,
                            name: None,
                            expr: Some(e),
                        });
                    }
                    if !self.eat_punct(Punct::Comma) {
                        break;
                    }
                }
            }
            self.expect_punct(Punct::RParen, "to close parameter list")?;
        }
        let insts = self.instance_units()?;
        let end = self.expect_punct(Punct::Semi, "after instance")?;
        Ok(ast::Instance {
            span: start.merge(end),
            mod_name,
            params,
            insts,
        })
    }

    fn gate_instance(&mut self, k: Kw) -> PResult<Option<ModuleItem>> {
        let start = self.bump().span;
        let insts = self.instance_units()?;
        let end = self.expect_punct(Punct::Semi, "after gate instance")?;
        Ok(Some(ModuleItem::Instance(ast::Instance {
            span: start.merge(end),
            mod_name: k.as_str().to_string(),
            params: Vec::new(),
            insts,
        })))
    }

    /// `inst_name [range] (port_conns) {, inst_name ... }` — the trailing `;`
    /// is consumed by the caller.
    fn instance_units(&mut self) -> PResult<Vec<ast::InstUnit>> {
        let mut insts = Vec::new();
        loop {
            let ispan = self.peek().span;
            let name = if self.peek().kind == Tok::Ident {
                self.bump().text
            } else {
                return self.err(
                    self.peek().span,
                    "VL-E020",
                    format!("expected instance name, found {}", self.peek().describe()),
                );
            };
            let array = if self.at_punct(Punct::LBracket) {
                Some(self.range()?)
            } else {
                None
            };
            self.expect_punct(Punct::LParen, "after instance name")?;
            let mut conns = Vec::new();
            if !self.at_punct(Punct::RParen) {
                loop {
                    let cspan = self.peek().span;
                    if self.at_punct(Punct::Dot) {
                        self.pos += 1;
                        let (n, _) = self.expect_ident("port name")?;
                        self.expect_punct(Punct::LParen, "after `.`port")?;
                        let e = if self.at_punct(Punct::RParen) {
                            None
                        } else {
                            Some(self.expr()?)
                        };
                        self.expect_punct(Punct::RParen, "to close port connection")?;
                        conns.push(ast::PortConn {
                            span: cspan,
                            name: Some(n),
                            expr: e,
                        });
                    } else if self.at_punct(Punct::Comma) {
                        // empty positional slot: `f(a, , c)`
                        conns.push(ast::PortConn {
                            span: cspan,
                            name: None,
                            expr: None,
                        });
                    } else if self.at_punct(Punct::RParen) {
                        conns.push(ast::PortConn {
                            span: cspan,
                            name: None,
                            expr: None,
                        });
                        break;
                    } else {
                        let e = self.expr()?;
                        conns.push(ast::PortConn {
                            span: cspan,
                            name: None,
                            expr: Some(e),
                        });
                    }
                    if !self.eat_punct(Punct::Comma) {
                        break;
                    }
                }
            }
            let iend = self.expect_punct(Punct::RParen, "to close port connections")?;
            insts.push(ast::InstUnit {
                span: ispan.merge(iend),
                name,
                array,
                conns,
            });
            if !self.eat_punct(Punct::Comma) {
                break;
            }
        }
        Ok(insts)
    }

    // ---- generate constructs ----

    fn generate_region(&mut self, start: Span) -> PResult<Option<ModuleItem>> {
        let mut items = Vec::new();
        let end_span = loop {
            self.skip_attrs();
            let es = self.peek().span;
            if self.eat_kw(Kw::Endgenerate) {
                break es;
            }
            if self.at_eof() {
                return self.err(
                    start,
                    "VL-E020",
                    "unterminated `generate` (missing `endgenerate`)",
                );
            }
            match self.generate_body_item()? {
                Some(it) => items.push(it),
                None => {}
            }
        };
        let _ = end_span; // kept for span-accurate diagnostics later
        Ok(Some(ModuleItem::GenerateRegion(items)))
    }

    /// One item in a generate scope: gen constructs or ordinary module items.
    fn generate_body_item(&mut self) -> PResult<Option<ModuleItem>> {
        let t = self.peek().clone();
        match t.kind {
            Tok::Keyword(Kw::Genvar) => {
                self.pos += 1;
                Ok(Some(ModuleItem::Decl(
                    self.decl_tail(t.span, ast::DeclKind::Genvar, None, None)?,
                )))
            }
            Tok::Keyword(Kw::If) => Ok(Some(self.gen_if()?)),
            Tok::Keyword(Kw::For) => Ok(Some(self.gen_for()?)),
            Tok::Keyword(Kw::Case) | Tok::Keyword(Kw::Casex) | Tok::Keyword(Kw::Casez) => {
                Ok(Some(self.gen_case()?))
            }
            _ => self.module_item(),
        }
    }

    /// `if (c) begin ... end` / `for (..) begin ... end`: gen_items wraps
    /// the begin-block as a GenBlock, but the construct's implicit genblk
    /// already names that scope — splice a single unnamed block's items
    /// through so only one level exists.
    fn splice_unnamed_block(items: Vec<ModuleItem>) -> Vec<ModuleItem> {
        if items.len() == 1 {
            if let ModuleItem::GenBlock { name: None, items: inner, .. } = &items[0] {
                return inner.clone();
            }
        }
        items
    }

    fn gen_if(&mut self) -> PResult<ModuleItem> {
        let start = self.expect_kw(Kw::If, "for generate-if")?;
        self.expect_punct(Punct::LParen, "after `if`")?;
        let cond = self.expr()?;
        self.expect_punct(Punct::RParen, "after generate-if condition")?;
        let then = Self::splice_unnamed_block(self.gen_items()?);
        let els = if self.eat_kw(Kw::Else) {
            if self.at_kw(Kw::If) {
                Some(vec![self.gen_if()?])
            } else {
                Some(Self::splice_unnamed_block(self.gen_items()?))
            }
        } else {
            None
        };
        let end = then
            .last()
            .map(item_span)
            .or_else(|| els.as_ref().and_then(|v| v.last()).map(item_span))
            .unwrap_or(cond.span());
        Ok(ModuleItem::GenIf {
            span: start.merge(end),
            cond,
            then,
            els,
        })
    }

    fn gen_for(&mut self) -> PResult<ModuleItem> {
        let start = self.expect_kw(Kw::For, "for generate-for")?;
        self.expect_punct(Punct::LParen, "after `for`")?;
        let (var, _) = self.expect_ident("genvar name")?;
        self.expect_punct(Punct::Eq, "after genvar name")?;
        let init = self.expr()?;
        self.expect_punct(Punct::Semi, "after generate-for init")?;
        let cond = self.expr()?;
        self.expect_punct(Punct::Semi, "after generate-for condition")?;
        let (svar, sspan) = self.expect_ident("genvar name in step")?;
        self.expect_punct(Punct::Eq, "after genvar name in step")?;
        let rhs = self.expr()?;
        self.expect_punct(Punct::RParen, "to close generate-for header")?;
        let step = AssignPair {
            span: sspan.merge(rhs.span()),
            lhs: Expr::Ident {
                span: sspan,
                name: svar,
            },
            rhs,
        };
        let items = self.gen_items()?;
        let block = match items.first() {
            Some(ModuleItem::GenBlock { name, .. }) => name.clone(),
            _ => None,
        };
        let items = Self::splice_unnamed_block(items);
        let end = items.last().map(item_span).unwrap_or(init.span());
        Ok(ModuleItem::GenFor {
            span: start.merge(end),
            var,
            init,
            cond,
            step,
            block,
            items,
        })
    }

    fn gen_case(&mut self) -> PResult<ModuleItem> {
        let start = self.bump().span;
        self.expect_punct(Punct::LParen, "after `case`")?;
        let expr = self.expr()?;
        self.expect_punct(Punct::RParen, "after case expression")?;
        let mut items = Vec::new();
        let mut default: Option<Vec<ModuleItem>> = None;
        loop {
            self.skip_attrs();
            if self.at_kw(Kw::Endcase) {
                break;
            }
            if self.at_eof() {
                return self.err(start, "VL-E020", "unterminated generate case");
            }
            if self.eat_kw(Kw::Default) {
                self.eat_punct(Punct::Colon);
                default = Some(Self::splice_unnamed_block(self.gen_items()?));
                continue;
            }
            let mut labels = vec![self.expr()?];
            while self.eat_punct(Punct::Comma) {
                labels.push(self.expr()?);
            }
            self.expect_punct(Punct::Colon, "after generate case labels")?;
            let its = Self::splice_unnamed_block(self.gen_items()?);
            items.push((labels, its));
        }
        let end = self.expect_kw(Kw::Endcase, "to close generate case")?;
        Ok(ModuleItem::GenCase {
            span: start.merge(end),
            expr,
            items,
            default,
        })
    }

    /// Generate scope items: a `begin : name ... end` block or a single item.
    fn gen_items(&mut self) -> PResult<Vec<ModuleItem>> {
        self.skip_attrs();
        if self.at_kw(Kw::Begin) {
            let start = self.bump().span;
            let name = if self.eat_punct(Punct::Colon) {
                Some(self.expect_ident("generate block name")?.0)
            } else {
                None
            };
            let mut items = Vec::new();
            loop {
                self.skip_attrs();
                if self.at_kw(Kw::End) {
                    break;
                }
                if self.at_eof() {
                    return self.err(start, "VL-E020", "unterminated generate block");
                }
                match self.generate_body_item()? {
                    Some(it) => items.push(it),
                    None => {}
                }
            }
            let end = self.expect_kw(Kw::End, "to close generate block")?;
            if self.eat_punct(Punct::Colon) {
                let _ = self.expect_ident("name after `end :`")?;
            }
            return Ok(vec![ModuleItem::GenBlock {
                span: start.merge(end),
                name,
                items,
            }]);
        }
        match self.module_item()? {
            Some(it) => Ok(vec![it]),
            None => Ok(Vec::new()),
        }
    }

    // ---- functions / tasks ----

    fn function_or_task(&mut self, is_task: bool) -> PResult<ast::Function> {
        let kwname = if is_task { "task" } else { "function" };
        let start = self.bump().span; // `function` / `task`
        self.skip_attrs();
        if self.eat_kw(Kw::Automatic) {}
        let mut ret_signed = false;
        let mut ret_range: Option<ast::Range> = None;
        let mut ret_type: Option<ParamType> = None;
        if !is_task {
            if self.eat_kw(Kw::Signed) {
                ret_signed = true;
            }
            match self.peek().kind {
                Tok::Keyword(Kw::Integer) => {
                    self.pos += 1;
                    ret_type = Some(ParamType::Integer);
                }
                Tok::Keyword(Kw::Real) => {
                    self.pos += 1;
                    ret_type = Some(ParamType::Real);
                }
                Tok::Keyword(Kw::Time) => {
                    self.pos += 1;
                    ret_type = Some(ParamType::Time);
                }
                Tok::Keyword(Kw::Realtime) => {
                    self.pos += 1;
                    ret_type = Some(ParamType::Realtime);
                }
                _ => {}
            }
            if self.at_punct(Punct::LBracket) {
                ret_range = Some(self.range()?);
            }
        }
        let (name, _) = self.expect_ident(&format!("{kwname} name"))?;
        let mut ports: Vec<ast::Decl> = Vec::new();
        if self.at_punct(Punct::LParen) {
            self.pos += 1;
            if !self.at_punct(Punct::RParen) {
                loop {
                    let dspan = self.peek().span;
                    let dir = if self.eat_kw(Kw::Input) {
                        Direction::Input
                    } else if self.eat_kw(Kw::Output) {
                        Direction::Output
                    } else if self.eat_kw(Kw::Inout) {
                        Direction::Inout
                    } else {
                        return self.err(
                            dspan,
                            "VL-E020",
                            format!(
                                "expected `input`/`output`/`inout` in ANSI {kwname} port list, found {}",
                                self.peek().describe()
                            ),
                        );
                    };
                    let _ = self.take_net_or_reg_kind();
                    // Type keywords are legal in task/function ports
                    // (`input integer n`); carry them as Decl.ty.
                    let mut ty: Option<ParamType> = None;
                    match self.peek().kind {
                        Tok::Keyword(Kw::Integer) => {
                            self.pos += 1;
                            ty = Some(ParamType::Integer);
                        }
                        Tok::Keyword(Kw::Real) => {
                            self.pos += 1;
                            ty = Some(ParamType::Real);
                        }
                        Tok::Keyword(Kw::Time) => {
                            self.pos += 1;
                            ty = Some(ParamType::Time);
                        }
                        Tok::Keyword(Kw::Realtime) => {
                            self.pos += 1;
                            ty = Some(ParamType::Realtime);
                        }
                        _ => {}
                    }
                    let mut signed = false;
                    if self.eat_kw(Kw::Signed) {
                        signed = true;
                    }
                    let range = if self.at_punct(Punct::LBracket) {
                        Some(self.range()?)
                    } else {
                        None
                    };
                    let (pname, _) = self.expect_ident("port name")?;
                    ports.push(ast::Decl {
                        span: dspan,
                        kind: ast::DeclKind::Reg,
                        ty,
                        signed,
                        range: range.clone(),
                        dir: Some(dir),
                        entries: vec![ast::DeclEntry {
                            span: dspan,
                            name: pname,
                            unpacked: Vec::new(),
                            init: None,
                        }],
                    });
                    if !self.eat_punct(Punct::Comma) {
                        break;
                    }
                    // Bare names after a group (`input [3:0] s1, s2, s3`)
                    // share the group's direction/range.
                    let mut list_done = false;
                    loop {
                        if self.peek().kind != Tok::Ident {
                            break;
                        }
                        let (nm, nspan) = self.expect_ident("port name")?;
                        ports.push(ast::Decl {
                            span: nspan,
                            kind: ast::DeclKind::Reg,
                            ty,
                            signed,
                            range: range.clone(),
                            dir: Some(dir),
                            entries: vec![ast::DeclEntry {
                                span: nspan,
                                name: nm,
                                unpacked: Vec::new(),
                                init: None,
                            }],
                        });
                        if !self.eat_punct(Punct::Comma) {
                            list_done = true;
                            break;
                        }
                    }
                    if list_done {
                        break;
                    }
                }
            }
            self.expect_punct(Punct::RParen, "to close port list")?;
        }
        self.expect_punct(Punct::Semi, &format!("after {kwname} header"))?;

        let mut decls: Vec<ast::Decl> = Vec::new();
        let mut body = Vec::new();
        let closer = if is_task { Kw::Endtask } else { Kw::Endfunction };
        loop {
            self.skip_attrs();
            if self.eat_kw(closer) {
                if self.eat_punct(Punct::Colon) {
                    let _ = self.expect_ident("name after `end` closer")?;
                }
                break;
            }
            if self.at_eof() {
                return self.err(
                    start,
                    "VL-E020",
                    format!("unterminated {kwname} `{name}` (missing `{}`)", closer.as_str()),
                );
            }
            if self.peek_is_var_decl_kw() || matches!(
                self.peek().kind,
                Tok::Keyword(Kw::Input) | Tok::Keyword(Kw::Output) | Tok::Keyword(Kw::Inout)
            ) {
                let t = self.peek().clone();
                let decl = if matches!(
                    t.kind,
                    Tok::Keyword(Kw::Input) | Tok::Keyword(Kw::Output) | Tok::Keyword(Kw::Inout)
                ) {
                    self.pos += 1;
                    let dir = match t.kind {
                        Tok::Keyword(Kw::Input) => Direction::Input,
                        Tok::Keyword(Kw::Output) => Direction::Output,
                        _ => Direction::Inout,
                    };
                    self.dir_decl(dir, t.span)?
                } else {
                    let (kind, ty) = match t.kind {
                        Tok::Keyword(Kw::Reg) => (ast::DeclKind::Reg, None),
                        Tok::Keyword(Kw::Integer) => (ast::DeclKind::Integer, Some(ParamType::Integer)),
                        Tok::Keyword(Kw::Real) => (ast::DeclKind::Real, Some(ParamType::Real)),
                        Tok::Keyword(Kw::Realtime) => (ast::DeclKind::Realtime, Some(ParamType::Realtime)),
                        Tok::Keyword(Kw::Time) => (ast::DeclKind::Time, Some(ParamType::Time)),
                        Tok::Keyword(Kw::Parameter) => (ast::DeclKind::Parameter, None),
                        _ => (ast::DeclKind::Localparam, None),
                    };
                    self.pos += 1;
                    self.decl_tail(t.span, kind, ty, None)?
                };
                decls.push(decl);
            } else {
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
        }
        Ok(ast::Function {
            span: start.merge(self.toks[(self.pos - 1).min(self.toks.len() - 1)].span),
            is_task,
            name,
            ret_signed,
            ret_range,
            ret_type,
            ports,
            decls,
            body,
        })
    }
}

fn item_span(it: &ModuleItem) -> Span {
    match it {
        ModuleItem::Decl(d) => d.span,
        ModuleItem::ContAssign(pairs) | ModuleItem::Defparam(pairs) => pairs
            .first()
            .map(|p| p.span)
            .unwrap_or_default(),
        ModuleItem::Always { span, .. } | ModuleItem::Initial { span, .. } => *span,
        ModuleItem::Instance(i) => i.span,
        ModuleItem::GenerateRegion(items) => items
            .last()
            .map(item_span)
            .unwrap_or_default(),
        ModuleItem::GenIf { span, .. }
        | ModuleItem::GenFor { span, .. }
        | ModuleItem::GenCase { span, .. }
        | ModuleItem::GenBlock { span, .. } => *span,
        ModuleItem::Function(f) | ModuleItem::Task(f) => f.span,
        ModuleItem::Empty => Span::default(),
    }
}
