//! The M2 vertical-slice interpreter: flat elaboration (union-find port
//! connections, parameter overrides), combinational-assign convergence, and
//! sequential `initial` execution with `$display`/`$finish`.
//!
//! Deliberate v0-slice limitations (each errors loudly, pointing at its
//! milestone): always/event controls/NBA (M5), generate (M3), functions/tasks
//! (M4), memories/reals/gates (M7). All initials run sequentially to
//! completion — correct for delay-free TBs, superseded by the M5 scheduler.

use crate::eval::{eval, Lookup, SimError, SResult, Val};
use crate::value::{low_bits, Bits};
use rcs_diag::Span;
use std::collections::HashMap;
use vl_syntax::ast::{self, CaseItem, CaseKind, DeclKind, Expr, Module, Stmt};

pub struct Cell {
    pub bits: Bits,
    pub signed: bool,
    pub msb: i64,
    pub lsb: i64,
}

struct Assign {
    lhs: Expr,
    rhs: Expr,
}

pub struct Sim {
    /// union-find over flat names (port connections alias child nets to the
    /// parent's nets); maps name → parent name (roots map to themselves).
    parent: HashMap<String, String>,
    cells: HashMap<String, Cell>,
    assigns: Vec<Assign>,
    initials: Vec<Stmt>,
    modules: HashMap<String, Module>,
    out: String,
    finished: bool,
}

#[derive(PartialEq)]
pub enum Flow {
    Normal,
    Finish,
}

impl Sim {
    /// Elaborate `top` from parsed modules (slice: top auto-detected upstream).
    pub fn build(modules: &[Module], top: &str) -> SResult<Sim> {
        let mut sim = Sim {
            parent: HashMap::new(),
            cells: HashMap::new(),
            assigns: Vec::new(),
            initials: Vec::new(),
            modules: modules.iter().map(|m| (m.name.clone(), m.clone())).collect(),
            out: String::new(),
            finished: false,
        };
        let m = sim
            .modules
            .get(top)
            .ok_or_else(|| SimError::new(format!("top module `{top}` not found"), None))?
            .clone();
        sim.elab_module(&m, "", &HashMap::new(), &HashMap::new())?;
        Ok(sim)
    }

    pub fn take_output(&mut self) -> String {
        std::mem::take(&mut self.out)
    }

    // ---- union-find ----

    fn find(&mut self, name: &str) -> String {
        match self.parent.get(name).cloned() {
            Some(p) => {
                let root = self.find(&p);
                self.parent.insert(name.to_string(), root.clone());
                root
            }
            None => name.to_string(),
        }
    }

    fn union(&mut self, a: &str, b: &str) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra != rb {
            self.parent.insert(ra, rb);
        }
    }

    // ---- elaboration ----

    fn elab_module(
        &mut self,
        m: &Module,
        prefix: &str,
        param_overrides: &HashMap<String, Bits>,
        param_override_signed: &HashMap<String, bool>,
    ) -> SResult<()> {
        // Header parameters first (later params may reference earlier ones).
        for p in &m.params {
            let full = format!("{prefix}{}", p.name);
            if let Some(v) = param_overrides.get(&p.name) {
                self.cells.insert(
                    full,
                    Cell {
                        bits: *v,
                        signed: *param_override_signed.get(&p.name).unwrap_or(&false),
                        msb: v.to_i64_lossy(),
                        lsb: 0,
                    },
                );
                continue;
            }
            let v = eval(&self.prefix_expr(&p.init, prefix)?, self)?;
            let (bits, signed) = match v {
                Val::Bits { bits, signed } => (bits, signed),
                Val::Str(_) => {
                    return Err(SimError::at("string parameter (M7)", p.span))
                }
            };
            self.cells.insert(
                full,
                Cell {
                    bits,
                    signed,
                    msb: bits.to_i64_lossy(),
                    lsb: 0,
                },
            );
        }
        for it in &m.items {
            self.elab_item(it, prefix, m)?;
        }
        Ok(())
    }

    fn elab_item(&mut self, it: &ast::ModuleItem, prefix: &str, m: &Module) -> SResult<()> {
        match it {
            ast::ModuleItem::Decl(d) => match d.kind {
                DeclKind::Parameter | DeclKind::Localparam => {
                    for e in &d.entries {
                        let v = eval(&self.prefix_expr(
                            e.init.as_ref().ok_or_else(|| {
                                SimError::at("parameter without initializer", e.span)
                            })?,
                            prefix,
                        )?, self)?;
                        let (bits, signed) = match v {
                            Val::Bits { bits, signed } => (bits, signed),
                            Val::Str(_) => {
                                return Err(SimError::at("string parameter (M7)", e.span))
                            }
                        };
                        self.cells.insert(
                            format!("{prefix}{}", e.name),
                            Cell {
                                bits,
                                signed,
                                msb: bits.to_i64_lossy(),
                                lsb: 0,
                            },
                        );
                    }
                }
                DeclKind::Genvar | DeclKind::Event => {}
                DeclKind::Real | DeclKind::Realtime => {
                    return Err(SimError::at(
                        "real variables are not yet supported (milestone M7)",
                        d.span,
                    ))
                }
                _ => {
                    let (width, signed, msb, lsb) = match &d.range {
                        Some(r) => {
                            let msv = self.const_i64(&r.msb, prefix)?;
                            let lsv = self.const_i64(&r.lsb, prefix)?;
                            let w = (msv - lsv).unsigned_abs() as u32 + 1;
                            (w, d.signed, msv, lsv)
                        }
                        None => match d.kind {
                            DeclKind::Integer => (32, true, 31, 0),
                            DeclKind::Time | DeclKind::Realtime => (64, false, 63, 0),
                            _ => (1, d.signed, 0, 0),
                        },
                    };
                    for e in &d.entries {
                        if !e.unpacked.is_empty() {
                            return Err(SimError::at(
                                "memory arrays are not yet supported (milestone M7)",
                                e.span,
                            ));
                        }
                        self.cells.insert(
                            format!("{prefix}{}", e.name),
                            Cell {
                                bits: Bits::zeros(width)?,
                                signed,
                                msb,
                                lsb,
                            },
                        );
                        if let Some(init) = &e.init {
                            let v = eval(&self.prefix_expr(init, prefix)?, self)?;
                            let (b, _) = match v {
                                Val::Bits { bits, signed } => (bits, signed),
                                Val::Str(_) => {
                                    return Err(SimError::at("string initializer (M7)", e.span))
                                }
                            };
                            let name = format!("{prefix}{}", e.name);
                            self.set_name(&name, b, signed)?;
                        }
                    }
                }
            },
            ast::ModuleItem::ContAssign(pairs) => {
                for p in pairs {
                    self.assigns.push(Assign {
                        lhs: self.prefix_expr(&p.lhs, prefix)?,
                        rhs: self.prefix_expr(&p.rhs, prefix)?,
                    });
                }
            }
            ast::ModuleItem::Initial { body, .. } => {
                self.initials.push(self.prefix_stmt(body, prefix)?);
            }
            ast::ModuleItem::Always { span, .. } => {
                return Err(SimError::at(
                    "always blocks need the event scheduler (milestone M5); \
                     this v0 slice runs delay-free combinational TBs only",
                    *span,
                ))
            }
            ast::ModuleItem::Instance(inst) => {
                self.elab_instance(inst, prefix, m)?;
            }
            ast::ModuleItem::GenerateRegion(_)
            | ast::ModuleItem::GenIf { .. }
            | ast::ModuleItem::GenFor { .. }
            | ast::ModuleItem::GenCase { .. }
            | ast::ModuleItem::GenBlock { .. } => {
                return Err(SimError::at(
                    "generate constructs are elaborated at milestone M3",
                    prefix_span(prefix),
                ))
            }
            ast::ModuleItem::Function(_) | ast::ModuleItem::Task(_) => {
                return Err(SimError::at(
                    "functions/tasks are supported from milestone M4",
                    prefix_span(prefix),
                ))
            }
            ast::ModuleItem::Defparam(_) => {
                return Err(SimError::at(
                    "defparam is elaborated at milestone M3",
                    prefix_span(prefix),
                ))
            }
            ast::ModuleItem::Empty => {}
        }
        Ok(())
    }

    fn elab_instance(
        &mut self,
        inst: &ast::Instance,
        prefix: &str,
        _m: &Module,
    ) -> SResult<()> {
        let child = self
            .modules
            .get(&inst.mod_name)
            .ok_or_else(|| {
                SimError::at(
                    format!("unknown module `{}` (gate primitives land at M7)", inst.mod_name),
                    inst.span,
                )
            })?
            .clone();
        for unit in &inst.insts {
            let child_prefix = format!("{prefix}{}.", unit.name);
            // Parameter overrides, evaluated in the *caller's* scope.
            let mut overrides: HashMap<String, Bits> = HashMap::new();
            let mut ovs_signed: HashMap<String, bool> = HashMap::new();
            if !inst.params.is_empty() {
                let named: Vec<&ast::ParamConn> =
                    inst.params.iter().filter(|p| p.name.is_some()).collect();
                if !named.is_empty() {
                    for p in named {
                        let name = p.name.clone().unwrap();
                        match &p.expr {
                            Some(e) => {
                                let v = eval(&self.prefix_expr(e, prefix)?, self)?;
                                if let Val::Bits { bits, signed } = v {
                                    overrides.insert(name.clone(), bits);
                                    ovs_signed.insert(name, signed);
                                }
                            }
                            None => {} // `.P()` — keep default
                        }
                    }
                } else {
                    for (i, p) in inst.params.iter().enumerate() {
                        let Some(formal) = child.params.get(i) else {
                            return Err(SimError::at(
                                "positional parameter override out of range",
                                p.span,
                            ));
                        };
                        if let Some(e) = &p.expr {
                            let v = eval(&self.prefix_expr(e, prefix)?, self)?;
                            if let Val::Bits { bits, signed } = v {
                                overrides.insert(formal.name.clone(), bits);
                                ovs_signed.insert(formal.name.clone(), signed);
                            }
                        }
                    }
                }
            }
            self.elab_module(&child, &child_prefix, &overrides, &ovs_signed)?;
            // Port connections.
            for (i, conn) in unit.conns.iter().enumerate() {
                let formal_name = match &conn.name {
                    Some(n) => n.clone(),
                    None => {
                        let Some(p) = child.ports.get(i) else {
                            return Err(SimError::at(
                                "positional port connection out of range",
                                conn.span,
                            ));
                        };
                        p.name.clone()
                    }
                };
                let formal = format!("{child_prefix}{formal_name}");
                let dir = self.port_direction(&child, &formal_name);
                match &conn.expr {
                    None => {} // unconnected port
                    Some(Expr::Ident { name, .. }) => {
                        // Simple net connection: alias (both directions).
                        let actual = format!("{prefix}{name}");
                        self.union(&formal, &actual);
                    }
                    Some(e) => match dir {
                        Some(ast::Direction::Output) => {
                            self.assigns.push(Assign {
                                lhs: self.prefix_expr(e, prefix)?,
                                rhs: Expr::Ident {
                                    span: conn.span,
                                    name: formal.clone(),
                                },
                            });
                        }
                        Some(ast::Direction::Inout) => {
                            return Err(SimError::at(
                                "inout ports connected to expressions are not yet supported (M7)",
                                conn.span,
                            ))
                        }
                        _ => {
                            // input (or unknown direction): drive the formal
                            self.assigns.push(Assign {
                                lhs: Expr::Ident {
                                    span: conn.span,
                                    name: formal.clone(),
                                },
                                rhs: self.prefix_expr(e, prefix)?,
                            });
                        }
                    },
                }
            }
        }
        Ok(())
    }

    fn port_direction(&self, m: &Module, port: &str) -> Option<ast::Direction> {
        for p in &m.ports {
            if p.name == port {
                return p.dir;
            }
        }
        // Non-ANSI: direction lives on a dir declaration item.
        for it in &m.items {
            if let ast::ModuleItem::Decl(d) = it {
                if let Some(dir) = d.dir {
                    for e in &d.entries {
                        if e.name == port {
                            return Some(dir);
                        }
                    }
                }
            }
        }
        None
    }

    // ---- const evaluation helper ----

    fn const_i64(&mut self, e: &Expr, prefix: &str) -> SResult<i64> {
        let pe = self.prefix_expr(e, prefix)?;
        let v = eval(&pe, self)?;
        match v {
            Val::Bits { bits, .. } if bits.is_known() => Ok(bits.to_i64_lossy()),
            Val::Bits { .. } => Err(SimError::at("constant expression contains x/z", e.span())),
            Val::Str(_) => Err(SimError::at("string where constant expected", e.span())),
        }
    }

    // ---- expression/statement prefixing (name rewriting into flat space) ----

    fn prefix_expr(&self, e: &Expr, prefix: &str) -> SResult<Expr> {
        Ok(match e {
            Expr::Ident { span, name } => Expr::Ident {
                span: *span,
                name: format!("{prefix}{name}"),
            },
            Expr::HierIdent { span, parts } => Expr::Ident {
                span: *span,
                name: format!("{prefix}{}", parts.join(".")),
            },
            Expr::NumLit(_) | Expr::RealLit { .. } | Expr::StrLit { .. } => e.clone(),
            Expr::Unary { span, op, expr } => Expr::Unary {
                span: *span,
                op: *op,
                expr: Box::new(self.prefix_expr(expr, prefix)?),
            },
            Expr::Binary {
                span,
                op,
                lhs,
                rhs,
            } => Expr::Binary {
                span: *span,
                op: *op,
                lhs: Box::new(self.prefix_expr(lhs, prefix)?),
                rhs: Box::new(self.prefix_expr(rhs, prefix)?),
            },
            Expr::Ternary {
                span,
                cond,
                then,
                els,
            } => Expr::Ternary {
                span: *span,
                cond: Box::new(self.prefix_expr(cond, prefix)?),
                then: Box::new(self.prefix_expr(then, prefix)?),
                els: Box::new(self.prefix_expr(els, prefix)?),
            },
            Expr::Concat { span, parts } => Expr::Concat {
                span: *span,
                parts: parts
                    .iter()
                    .map(|p| self.prefix_expr(p, prefix))
                    .collect::<SResult<_>>()?,
            },
            Expr::Repeat {
                span,
                count,
                value,
            } => Expr::Repeat {
                span: *span,
                count: Box::new(self.prefix_expr(count, prefix)?),
                value: Box::new(self.prefix_expr(value, prefix)?),
            },
            Expr::Index {
                span,
                base,
                index,
            } => Expr::Index {
                span: *span,
                base: Box::new(self.prefix_expr(base, prefix)?),
                index: Box::new(self.prefix_expr(index, prefix)?),
            },
            Expr::PartSelect {
                span,
                base,
                msb,
                lsb,
            } => Expr::PartSelect {
                span: *span,
                base: Box::new(self.prefix_expr(base, prefix)?),
                msb: Box::new(self.prefix_expr(msb, prefix)?),
                lsb: Box::new(self.prefix_expr(lsb, prefix)?),
            },
            Expr::IndexedPart { .. } => {
                return Err(SimError::at(
                    "indexed part-selects land at milestone M7",
                    e.span(),
                ))
            }
            Expr::Call {
                span,
                func,
                args,
            } => Expr::Call {
                span: *span,
                func: func.clone(),
                args: args
                    .iter()
                    .map(|a| self.prefix_expr(a, prefix))
                    .collect::<SResult<_>>()?,
            },
        })
    }

    fn prefix_stmt(&self, s: &Stmt, prefix: &str) -> SResult<Stmt> {
        Ok(match s {
            Stmt::Block { span, name, body } => Stmt::Block {
                span: *span,
                name: name.clone(),
                body: body
                    .iter()
                    .map(|b| self.prefix_stmt(b, prefix))
                    .collect::<SResult<_>>()?,
            },
            Stmt::BAssign { span, lhs, rhs } => Stmt::BAssign {
                span: *span,
                lhs: self.prefix_expr(lhs, prefix)?,
                rhs: self.prefix_expr(rhs, prefix)?,
            },
            Stmt::NbAssign { span, lhs, rhs } => Stmt::NbAssign {
                span: *span,
                lhs: self.prefix_expr(lhs, prefix)?,
                rhs: self.prefix_expr(rhs, prefix)?,
            },
            Stmt::If {
                span,
                cond,
                then,
                els,
            } => Stmt::If {
                span: *span,
                cond: self.prefix_expr(cond, prefix)?,
                then: Box::new(self.prefix_stmt(then, prefix)?),
                els: match els {
                    Some(e) => Some(Box::new(self.prefix_stmt(e, prefix)?)),
                    None => None,
                },
            },
            Stmt::Case {
                span,
                kind,
                expr,
                items,
            } => Stmt::Case {
                span: *span,
                kind: *kind,
                expr: self.prefix_expr(expr, prefix)?,
                items: items
                    .iter()
                    .map(|it| match it {
                        CaseItem::Items(labels, st) => Ok(CaseItem::Items(
                            labels
                                .iter()
                                .map(|l| self.prefix_expr(l, prefix))
                                .collect::<SResult<_>>()?,
                            self.prefix_stmt(st, prefix)?,
                        )),
                        CaseItem::Default(st) => Ok(CaseItem::Default(self.prefix_stmt(st, prefix)?)),
                    })
                    .collect::<SResult<_>>()?,
            },
            Stmt::For {
                span,
                init,
                cond,
                step,
                body,
            } => Stmt::For {
                span: *span,
                init: self.prefix_pairs(init, prefix)?,
                cond: self.prefix_expr(cond, prefix)?,
                step: self.prefix_pairs(step, prefix)?,
                body: Box::new(self.prefix_stmt(body, prefix)?),
            },
            Stmt::While {
                span,
                cond,
                body,
            } => Stmt::While {
                span: *span,
                cond: self.prefix_expr(cond, prefix)?,
                body: Box::new(self.prefix_stmt(body, prefix)?),
            },
            Stmt::Repeat {
                span,
                count,
                body,
            } => Stmt::Repeat {
                span: *span,
                count: self.prefix_expr(count, prefix)?,
                body: Box::new(self.prefix_stmt(body, prefix)?),
            },
            Stmt::Forever { span, body } => Stmt::Forever {
                span: *span,
                body: Box::new(self.prefix_stmt(body, prefix)?),
            },
            Stmt::DelayCtrl { span, delay, body } => Stmt::DelayCtrl {
                span: *span,
                delay: self.prefix_delay(delay, prefix)?,
                body: match body {
                    Some(b) => Some(Box::new(self.prefix_stmt(b, prefix)?)),
                    None => None,
                },
            },
            Stmt::EventCtrl { span, .. } => {
                return Err(SimError::at(
                    "event controls need the scheduler (milestone M5)",
                    *span,
                ))
            }
            Stmt::SysCall {
                span,
                name,
                args,
            } => Stmt::SysCall {
                span: *span,
                name: name.clone(),
                args: args
                    .iter()
                    .map(|a| self.prefix_expr(a, prefix))
                    .collect::<SResult<_>>()?,
            },
            Stmt::SubCall { span, .. } => {
                return Err(SimError::at(
                    "task calls land at milestone M4",
                    *span,
                ))
            }
            Stmt::Disable { span, .. } => {
                return Err(SimError::at(
                    "disable lands at milestone M4",
                    *span,
                ))
            }
            Stmt::Null { span } => Stmt::Null { span: *span },
        })
    }

    fn prefix_pairs(&self, pairs: &[ast::AssignPair], prefix: &str) -> SResult<Vec<ast::AssignPair>> {
        pairs
            .iter()
            .map(|p| {
                Ok(ast::AssignPair {
                    span: p.span,
                    lhs: self.prefix_expr(&p.lhs, prefix)?,
                    rhs: self.prefix_expr(&p.rhs, prefix)?,
                })
            })
            .collect()
    }

    fn prefix_delay(&self, d: &ast::DelayVal, prefix: &str) -> SResult<ast::DelayVal> {
        Ok(match d {
            ast::DelayVal::Lit(t) => ast::DelayVal::Lit(t.clone()),
            ast::DelayVal::Expr(e) => ast::DelayVal::Expr(self.prefix_expr(e, prefix)?),
        })
    }

    // ---- reading / writing through the union-find ----

    fn read_name(&mut self, name: &str, span: Span) -> SResult<(Bits, bool)> {
        let root = self.find(name);
        match self.cells.get(&root) {
            Some(c) => Ok((c.bits, c.signed)),
            None => Err(SimError::at(format!("undeclared identifier `{name}`"), span)),
        }
    }

    fn set_name(&mut self, name: &str, bits: Bits, signed: bool) -> SResult<()> {
        let root = self.find(name);
        match self.cells.get_mut(&root) {
            Some(c) => {
                // Adjust to the declared width (RHS signedness decides ext).
                c.bits = if c.signed && signed {
                    bits.sadjust(c.bits.width)?
                } else {
                    bits.uadjust(c.bits.width)?
                };
                Ok(())
            }
            None => Err(SimError::new(
                format!("undeclared identifier `{name}`"),
                None,
            )),
        }
    }

    fn assign_lvalue(&mut self, lhs: &Expr, v: Val) -> SResult<()> {
        let (bits, signed) = match v {
            Val::Bits { bits, signed } => (bits, signed),
            Val::Str(_) => return Err(SimError::at("cannot assign a string here", lhs.span())),
        };
        match lhs {
            Expr::Ident { name, .. } => self.set_name(name, bits, signed),
            Expr::HierIdent { parts, .. } => {
                self.set_name(&parts.join("."), bits, signed)
            }
            Expr::Index { base, index, .. } => {
                let (cur, cs) = self.read_lvalue_base(base)?;
                let (i, _) = match eval(index, self)? {
                    Val::Bits { bits, .. } => (bits, false),
                    Val::Str(_) => return Err(SimError::at("bad index", lhs.span())),
                };
                if i.has_unknown() {
                    return Ok(()); // x-indexed write: no effect (§5.2.1)
                }
                let pos = self.bit_position(base, i.to_i64_lossy());
                let cur2 = match pos {
                    Some(p) if p >= 0 && (p as u32) < cur.width => {
                        cur.set_part(p as u32, p as u32, &bits)?
                    }
                    _ => cur, // out-of-range write: no effect
                };
                self.write_base(base, cur2, cs)
            }
            Expr::PartSelect { base, msb, lsb, .. } => {
                let (cur, cs) = self.read_lvalue_base(base)?;
                let (m, _) = match eval(msb, self)? {
                    Val::Bits { bits, .. } => (bits, false),
                    Val::Str(_) => return Err(SimError::at("bad part-select", lhs.span())),
                };
                let (l, _) = match eval(lsb, self)? {
                    Val::Bits { bits, .. } => (bits, false),
                    Val::Str(_) => return Err(SimError::at("bad part-select", lhs.span())),
                };
                if m.has_unknown() || l.has_unknown() {
                    return Ok(());
                }
                let hi = self.bit_position(base, m.to_i64_lossy());
                let lo = self.bit_position(base, l.to_i64_lossy());
                let cur2 = match (hi, lo) {
                    (Some(h), Some(l))
                        if h >= 0 && l >= 0 && (h.max(l) as u32) < cur.width =>
                    {
                        cur.set_part(h as u32, l as u32, &bits)?
                    }
                    _ => cur,
                };
                self.write_base(base, cur2, cs)
            }
            Expr::Concat { parts, .. } => {
                // Split rhs MSB-first across the targets (§5.1.14 lvalue).
                let mut off = bits.width;
                for p in parts {
                    let w = self.lvalue_width(p)?;
                    if off < w {
                        return Err(SimError::at("concat lvalue wider than rhs", lhs.span()));
                    }
                    off -= w;
                    let slice = bits.part(off + w - 1, off)?;
                    self.assign_lvalue(p, Val::Bits { bits: slice, signed: false })?;
                }
                Ok(())
            }
            other => Err(SimError::at(
                "expression is not an assignable lvalue",
                other.span(),
            )),
        }
    }

    fn read_lvalue_base(&mut self, e: &Expr) -> SResult<(Bits, bool)> {
        match e {
            Expr::Ident { name, .. } => self.read_name(name, e.span()),
            Expr::HierIdent { parts, .. } => self.read_name(&parts.join("."), e.span()),
            other => Err(SimError::at(
                "unsupported selection base (M7)",
                other.span(),
            )),
        }
    }

    fn write_base(&mut self, e: &Expr, bits: Bits, signed: bool) -> SResult<()> {
        match e {
            Expr::Ident { name, .. } => self.set_name(name, bits, signed),
            Expr::HierIdent { parts, .. } => self.set_name(&parts.join("."), bits, signed),
            other => Err(SimError::at("unsupported selection base (M7)", other.span())),
        }
    }

    fn lvalue_width(&mut self, e: &Expr) -> SResult<u32> {
        match e {
            Expr::Ident { name, .. } => {
                let root = self.find(name);
                Ok(self.cells.get(&root).map(|c| c.bits.width).unwrap_or(1))
            }
            Expr::HierIdent { parts, .. } => {
                let key = parts.join(".");
                let root = self.find(&key);
                Ok(self.cells.get(&root).map(|c| c.bits.width).unwrap_or(1))
            }
            Expr::Index { .. } => Ok(1),
            Expr::PartSelect { msb, lsb, .. } => {
                let (m, _) = match eval(msb, self)? {
                    Val::Bits { bits, .. } => (bits, false),
                    Val::Str(_) => return Err(SimError::at("bad part-select", e.span())),
                };
                let (l, _) = match eval(lsb, self)? {
                    Val::Bits { bits, .. } => (bits, false),
                    Val::Str(_) => return Err(SimError::at("bad part-select", e.span())),
                };
                Ok((m.to_i64_lossy() - l.to_i64_lossy()).unsigned_abs() as u32 + 1)
            }
            Expr::Concat { parts, .. } => {
                let mut w = 0;
                for p in parts {
                    w += self.lvalue_width(p)?;
                }
                Ok(w)
            }
            other => Err(SimError::at("not an lvalue", other.span())),
        }
    }

    fn bit_position(&mut self, base: &Expr, index: i64) -> Option<i64> {
        let name = match base {
            Expr::Ident { name, .. } => Some(name.clone()),
            Expr::HierIdent { parts, .. } => Some(parts.join(".")),
            _ => None,
        };
        match name.and_then(|n| self.decl_range(&n)) {
            Some((msb, lsb)) => {
                if msb >= lsb {
                    Some(index - lsb)
                } else {
                    Some(lsb - index)
                }
            }
            None => Some(index),
        }
    }

    // ---- combinational convergence ----

    fn settle(&mut self) -> SResult<()> {
        for _ in 0..100 {
            let mut changed = false;
            let assigns: Vec<(Expr, Expr)> = self
                .assigns
                .iter()
                .map(|a| (a.lhs.clone(), a.rhs.clone()))
                .collect();
            for (lhs, rhs) in &assigns {
                let v = eval(rhs, self)?;
                let (bits, signed) = match v {
                    Val::Bits { bits, signed } => (bits, signed),
                    Val::Str(_) => {
                        return Err(SimError::at("assign produced a string", rhs.span()))
                    }
                };
                let cur = self.read_lvalue_base_quiet(lhs)?;
                // Continuous-assign extension follows the RHS's own signedness.
                let target = if signed { bits.sadjust(cur.0.width)? } else { bits.uadjust(cur.0.width)? };
                if cur.0 != target {
                    self.assign_lvalue(lhs, Val::Bits { bits, signed })?;
                    changed = true;
                }
            }
            if !changed {
                return Ok(());
            }
        }
        Err(SimError::new(
            "combinational network did not converge after 100 iterations \
             (combinational loop — upstream reports this as UNOPTFLAT)",
            None,
        ))
    }

    fn read_lvalue_base_quiet(&mut self, e: &Expr) -> SResult<(Bits, bool)> {
        match e {
            Expr::Ident { name, .. } => {
                let root = self.find(name);
                Ok(match self.cells.get(&root) {
                    Some(c) => (c.bits, c.signed),
                    None => (Bits::zeros(1)?, false),
                })
            }
            other => self.read_lvalue_base(other),
        }
    }

    // ---- statement execution ----

    pub fn run(&mut self) -> SResult<()> {
        self.settle()?;
        let initials = self.initials.clone();
        for stmt in &initials {
            if self.finished {
                break;
            }
            self.exec(stmt)?;
        }
        Ok(())
    }

    fn exec(&mut self, s: &Stmt) -> SResult<Flow> {
        const LOOP_CAP: u64 = 10_000_000;
        match s {
            Stmt::Block { body, .. } => {
                for st in body {
                    if self.exec(st)? == Flow::Finish {
                        return Ok(Flow::Finish);
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::BAssign { lhs, rhs, .. } => {
                let v = eval(rhs, self)?;
                self.assign_lvalue(lhs, v)?;
                self.settle()?;
                Ok(Flow::Normal)
            }
            Stmt::NbAssign { span, .. } => Err(SimError::at(
                "nonblocking assignments need the scheduler (milestone M5)",
                *span,
            )),
            Stmt::If {
                cond,
                then,
                els,
                ..
            } => {
                let (c, _) = match eval(cond, self)? {
                    Val::Bits { bits, .. } => (bits, false),
                    Val::Str(_) => {
                        return Err(SimError::at("string as condition", cond.span()))
                    }
                };
                // x/z condition takes the else branch (IEEE 1364-2005 §9.4).
                let take = matches!(c.truthy()?, Some(true));
                if take {
                    self.exec(then)
                } else if let Some(e) = els {
                    self.exec(e)
                } else {
                    Ok(Flow::Normal)
                }
            }
            Stmt::Case {
                kind,
                expr,
                items,
                ..
            } => {
                let (v, _) = match eval(expr, self)? {
                    Val::Bits { bits, .. } => (bits, false),
                    Val::Str(s) => {
                        return Err(SimError::at(format!("string in case expr `{s}`"), expr.span()))
                    }
                };
                for it in items {
                    match it {
                        CaseItem::Items(labels, body) => {
                            for l in labels {
                                let (lv, _) = match eval(l, self)? {
                                    Val::Bits { bits, .. } => (bits, false),
                                    Val::Str(_) => {
                                        return Err(SimError::at("string case label", l.span()))
                                    }
                                };
                                if self.case_matches(*kind, &v, &lv)? {
                                    return self.exec(body);
                                }
                            }
                        }
                        CaseItem::Default(_) => {}
                    }
                }
                for it in items {
                    if let CaseItem::Default(body) = it {
                        return self.exec(body);
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
                ..
            } => {
                for p in init {
                    let v = eval(&p.rhs, self)?;
                    self.assign_lvalue(&p.lhs, v)?;
                }
                self.settle()?;
                let mut n = 0u64;
                loop {
                    let (c, _) = match eval(cond, self)? {
                        Val::Bits { bits, .. } => (bits, false),
                        Val::Str(_) => {
                            return Err(SimError::at("string as for-condition", cond.span()))
                        }
                    };
                    if c.truthy()? != Some(true) {
                        break;
                    }
                    n += 1;
                    if n > LOOP_CAP {
                        return Err(SimError::at(
                            "for loop exceeded 10M iterations (slice has no time wheel)",
                            s_stmt_span(s),
                        ));
                    }
                    if self.exec(body)? == Flow::Finish {
                        return Ok(Flow::Finish);
                    }
                    for p in step {
                        let v = eval(&p.rhs, self)?;
                        self.assign_lvalue(&p.lhs, v)?;
                    }
                    self.settle()?;
                }
                Ok(Flow::Normal)
            }
            Stmt::While { cond, body, .. } => {
                let mut n = 0u64;
                loop {
                    let (c, _) = match eval(cond, self)? {
                        Val::Bits { bits, .. } => (bits, false),
                        Val::Str(_) => {
                            return Err(SimError::at("string as while-condition", cond.span()))
                        }
                    };
                    if c.truthy()? != Some(true) {
                        break;
                    }
                    n += 1;
                    if n > LOOP_CAP {
                        return Err(SimError::at("while loop exceeded cap", s_stmt_span(s)));
                    }
                    if self.exec(body)? == Flow::Finish {
                        return Ok(Flow::Finish);
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::Repeat { count, body, .. } => {
                let (c, _) = match eval(count, self)? {
                    Val::Bits { bits, .. } => (bits, false),
                    Val::Str(_) => return Err(SimError::at("string repeat count", count.span())),
                };
                if c.has_unknown() {
                    return Err(SimError::at("unknown repeat count", count.span()));
                }
                for _ in 0..c.to_u64_lossy().min(LOOP_CAP) {
                    if self.exec(body)? == Flow::Finish {
                        return Ok(Flow::Finish);
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::Forever { body, .. } => {
                let mut n = 0u64;
                loop {
                    n += 1;
                    if n > LOOP_CAP {
                        return Err(SimError::at(
                            "forever loop without $finish (slice has no time wheel)",
                            s_stmt_span(s),
                        ));
                    }
                    if self.exec(body)? == Flow::Finish {
                        return Ok(Flow::Finish);
                    }
                }
            }
            Stmt::DelayCtrl { body, .. } => {
                // No time model in the slice; a delay just settles comb logic.
                self.settle()?;
                match body {
                    Some(b) => self.exec(b),
                    None => Ok(Flow::Normal),
                }
            }
            Stmt::EventCtrl { span, .. } => Err(SimError::at(
                "event controls need the scheduler (milestone M5)",
                *span,
            )),
            Stmt::SysCall { name, args, .. } => self.syscall(name, args),
            Stmt::SubCall { span, name, .. } => Err(SimError::at(
                format!("task call `{name}` lands at milestone M4"),
                *span,
            )),
            Stmt::Disable { span, name } => Err(SimError::at(
                format!("disable `{name}` lands at milestone M4"),
                *span,
            )),
            Stmt::Null { .. } => Ok(Flow::Normal),
        }
    }

    fn case_matches(&self, kind: CaseKind, v: &Bits, label: &Bits) -> SResult<bool> {
        let w = v.width.max(label.width);
        let a = v.uadjust(w)?;
        let b = label.uadjust(w)?;
        match kind {
            CaseKind::Case => Ok(a.case_eq(&b)?.to_u64_lossy() == 1),
            CaseKind::Casez => {
                // x AND z in the case item are don't-care (IEEE 1364-2005 §9.5.2).
                let care = !b.mask & low_bits(w);
                Ok(((a.data ^ b.data) & care) == 0 && ((a.mask ^ b.mask) & care) == 0)
            }
            CaseKind::Casex => {
                // x/z in either value or item are don't-care.
                let care = !(a.mask | b.mask) & low_bits(w);
                Ok(((a.data ^ b.data) & care) == 0)
            }
        }
    }

    fn syscall(&mut self, name: &str, args: &[Expr]) -> SResult<Flow> {
        match name {
            "$display" | "$write" | "$error" | "$warning" | "$info" | "$fatal" => {
                let text = self.format_display(args)?;
                let prefix = match name {
                    "$error" | "$fatal" => "%Error: ",
                    "$warning" => "%Warning: ",
                    "$info" => "%Info: ",
                    _ => "",
                };
                self.out.push_str(prefix);
                self.out.push_str(&text);
                if name != "$write" {
                    self.out.push('\n');
                }
                if name == "$fatal" {
                    self.finished = true;
                    return Ok(Flow::Finish);
                }
                Ok(Flow::Normal)
            }
            "$finish" | "$stop" => {
                self.finished = true;
                Ok(Flow::Finish)
            }
            "$dumpfile" | "$dumpvars" | "$dumpoff" | "$dumpon" | "$dumpall" => Ok(Flow::Normal), // M6
            _ => Err(SimError::new(
                format!("system task `{name}` is not yet supported"),
                None,
            )),
        }
    }

    /// `$display("%b", x)` formatting per IEEE 1364-2005 §17.1.1.
    fn format_display(&mut self, args: &[Expr]) -> SResult<String> {
        let mut vals: Vec<Val> = Vec::with_capacity(args.len());
        for a in args {
            vals.push(eval(a, self)?);
        }
        let mut out = String::new();
        let fmt = match vals.first() {
            Some(Val::Str(s)) => Some(s.clone()),
            _ => None,
        };
        let Some(fmt) = fmt else {
            for v in &vals {
                match v {
                    Val::Bits { bits, .. } => out.push_str(&bits.fmt_dec()),
                    Val::Str(s) => out.push_str(s),
                }
            }
            return Ok(out);
        };
        let mut ai = 1usize;
        let chars: Vec<char> = fmt.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c != '%' {
                out.push(c);
                i += 1;
                continue;
            }
            i += 1;
            if i >= chars.len() {
                return Err(SimError::new("format string ends with `%`", None));
            }
            if chars[i] == '%' {
                out.push('%');
                i += 1;
                continue;
            }
            let mut zero_pad = false;
            let mut width: Option<usize> = None;
            while i < chars.len() {
                if chars[i] == '0' && width.is_none() && !zero_pad {
                    zero_pad = true;
                    i += 1;
                } else if let Some(d) = chars[i].to_digit(10) {
                    width = Some(width.unwrap_or(0) * 10 + d as usize);
                    i += 1;
                } else {
                    break;
                }
            }
            let Some(conv) = chars.get(i).copied() else {
                return Err(SimError::new("format string ends with `%`", None));
            };
            i += 1;
            let Some(arg) = vals.get(ai) else {
                return Err(SimError::new("not enough $display arguments", None));
            };
            ai += 1;
            let piece = match (conv, arg) {
                ('s', Val::Str(s)) => s.clone(),
                ('c', Val::Bits { bits, .. }) => {
                    char::from_u32(bits.to_u64_lossy() as u32).unwrap_or('?').to_string()
                }
                (
                    'd' | 'D',
                    Val::Bits { bits, signed },
                ) => {
                    let natural = if *signed {
                        bits.to_i64_lossy().abs().to_string().len()
                            + (bits.to_i64_lossy() < 0) as usize
                    } else {
                        let maxv = if bits.width >= 64 {
                            u64::MAX
                        } else {
                            low_bits(bits.width)
                        };
                        maxv.to_string().len()
                    };
                    let mut s = bits.fmt_dec();
                    // `%0d` = no padding at all; plain `%d` pads to the
                    // natural width (digits of the type's max value).
                    let pad = if zero_pad {
                        s.chars().count()
                    } else {
                        width.unwrap_or(natural).max(s.chars().count())
                    };
                    while s.chars().count() < pad {
                        s.insert(0, ' ');
                    }
                    s
                }
                (
                    'b' | 'B',
                    Val::Bits { bits, .. },
                ) => {
                    let mut s = bits.fmt_bin();
                    let pad = width.unwrap_or(bits.width as usize).max(s.len());
                    while s.len() < pad {
                        s.insert(0, if zero_pad { '0' } else { ' ' });
                    }
                    s
                }
                (
                    'h' | 'H' | 'x' | 'X',
                    Val::Bits { bits, .. },
                ) => {
                    let nib = bits.width.div_ceil(4) as usize;
                    let mut s = bits.fmt_hex();
                    let pad = width.unwrap_or(nib).max(s.len());
                    while s.len() < pad {
                        s.insert(0, if zero_pad { '0' } else { ' ' });
                    }
                    s
                }
                (
                    'o' | 'O',
                    Val::Bits { bits, .. },
                ) => {
                    if bits.has_unknown() {
                        "x".to_string()
                    } else {
                        format!("{:o}", bits.to_u64_lossy())
                    }
                }
                (
                    't' | 'T',
                    Val::Bits { bits, .. },
                ) => bits.fmt_dec(),
                ('s', Val::Bits { .. }) => {
                    return Err(SimError::new("%s needs a string argument", None))
                }
                (_, Val::Str(_)) => {
                    return Err(SimError::new(
                        format!("format `%{conv}` needs a value argument"),
                        None,
                    ))
                }
                (other, _) => {
                    return Err(SimError::new(
                        format!("unsupported format specifier `%{other}`"),
                        None,
                    ))
                }
            };
            out.push_str(&piece);
        }
        Ok(out)
    }
}

impl Lookup for Sim {
    fn lookup(&mut self, name: &str, span: Span) -> SResult<Val> {
        let (bits, signed) = self.read_name(name, span)?;
        Ok(Val::Bits { bits, signed })
    }

    fn decl_range(&mut self, name: &str) -> Option<(i64, i64)> {
        let root = self.find(name);
        self.cells.get(&root).map(|c| (c.msb, c.lsb))
    }
}

fn prefix_span(_prefix: &str) -> Span {
    Span::default()
}

fn s_stmt_span(s: &Stmt) -> Span {
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

#[allow(unused)]
fn _todo_span_marker() {}
