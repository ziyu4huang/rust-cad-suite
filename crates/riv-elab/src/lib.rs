//! riv-elab — flat elaboration, Phase A of design §5: top auto-detection
//! (un-instantiated rule), decl/param constant evaluation via vl-values,
//! width/sign propagation baked into ops at lowering (IEEE 5.4/5.5,
//! self-determined subcontexts), display format compilation including the
//! StrLit→Bits path, and the milestone hard-error surface.
//!
//! Per design §1 the crate may use `vl-syntax` + `rcs-diag` + `riv-ir` +
//! `vl-values`; semantics are implemented per spec (IEEE 1364-2005 +
//! probed oracle behavior), not copied from vl-sim.

mod err;
mod expr;
mod fmt;
mod interp;

pub use err::ElabError;

use err::{err_at, ElabResult};
use expr::{Lower, NameKind};
use riv_ir::{Lval, Op, SensItem, Thread, ThreadKind, VarId, VarKind};
use rcs_diag::SourceMap;
use std::collections::HashMap;
use vl_syntax::ast::{self as ast, DeclKind, Expr, Module, ModuleItem, ParamType, Stmt};
use vl_values::Bits;

/// Per-source-file `` `timescale `` capture (the CLI driver collects these
/// from the preprocessor, in command-line order).
pub struct FileTs {
    pub path: String,
    pub timescale: Option<String>,
}

pub struct ElabInput<'a> {
    pub modules: &'a [Module],
    pub sm: &'a SourceMap,
    /// Explicit `-s` top; None → auto-detect (the un-instantiated module).
    pub top: Option<&'a str>,
    pub files: &'a [FileTs],
}

/// Internal var info (superset of what the IR carries).
#[derive(Clone)]
pub struct VarInfo {
    pub name: String,
    pub width: u32,
    pub signed: bool,
    pub kind: VarKind,
    pub desc: bool,
    pub msb: u32,
    pub lsb: u32,
}

pub struct Elab<'a> {
    pub sm: &'a SourceMap,
    pub vars: Vec<VarInfo>,
    pub params: HashMap<String, (Bits, bool)>,
    /// Task/function registry: name → (func idx, formals in declaration
    /// order with their frame slots). Frame layout: slot 0 = ret value
    /// (functions; unused for tasks), formals from 1.
    pub funcs: HashMap<String, (u32, Vec<(u32, ast::Direction, u32, bool)>)>,
    /// func idx → (ret width, ret signed) for call-site typing.
    pub ret_types: HashMap<u32, (u32, bool)>,
}

/// Top-module auto-detection: the module no other module instantiates
/// (mirrors the vl-difftest/vl-cli algorithm; re-implemented per the
/// crate-boundary rule — riv-elab cannot depend on vl-cli).
pub fn detect_top(modules: &[Module]) -> Result<String, Vec<String>> {
    let inst: std::collections::HashSet<String> = modules
        .iter()
        .flat_map(|m| m.items.iter())
        .filter_map(|it| match it {
            ModuleItem::Instance(i) => Some(i.mod_name.clone()),
            _ => None,
        })
        .collect();
    let cands: Vec<String> = modules
        .iter()
        .map(|m| m.name.clone())
        .filter(|n| !inst.contains(n))
        .collect();
    match cands.as_slice() {
        [only] => Ok(only.clone()),
        _ => Err(cands),
    }
}

/// Timescale halves parsed into a scale ratio (unit in precision ticks).
#[derive(Clone)]
struct TsInfo {
    unit: String,
    precision: String,
    num: u64,
    den: u64,
}

fn ts_mult(unit: &str) -> Option<u128> {
    Some(match unit {
        "s" => 1_000_000_000_000_000,
        "ms" => 1_000_000_000_000,
        "us" => 1_000_000_000,
        "ns" => 1_000_000,
        "ps" => 1_000,
        "fs" => 1,
        _ => return None,
    })
}

/// `1ns` → (mantissa 1, value in femtoseconds)
fn parse_ts_half(half: &str) -> Option<(u64, u128)> {
    let digits: String = half.chars().take_while(|c| c.is_ascii_digit()).collect();
    let rest = &half[digits.len()..];
    let m: u64 = digits.parse().ok()?;
    if !matches!(m, 1 | 10 | 100) {
        return None;
    }
    Some((m, m as u128 * ts_mult(rest)?))
}

fn parse_timescale(ts: &str) -> Option<TsInfo> {
    let (u, p) = ts.split_once('/')?;
    let (um, uv) = parse_ts_half(u.trim())?;
    let (pm, pv) = parse_ts_half(p.trim())?;
    let _ = (um, pm);
    if uv < pv {
        return None; // unit finer than precision — illegal
    }
    Some(TsInfo {
        unit: u.trim().to_string(),
        precision: p.trim().to_string(),
        num: (uv / pv).min(u64::MAX as u128) as u64,
        den: 1,
    })
}

pub fn elaborate(input: ElabInput) -> Result<riv_ir::Design, ElabError> {
    let ElabInput { modules, sm, top, files } = input;
    let top_name = match top {
        Some(t) => {
            if !modules.iter().any(|m| m.name == *t) {
                return Err(ElabError {
                    message: format!("riverilog: top module `{t}` not found"),
                    exit: 2,
                });
            }
            t.to_string()
        }
        None => match detect_top(modules) {
            Ok(t) => t,
            Err(cands) => {
                return Err(ElabError {
                    message: format!(
                        "riverilog: cannot auto-detect top module (candidates: {}); pass -s <module>",
                        if cands.is_empty() { "none".to_string() } else { cands.join(", ") }
                    ),
                    exit: 2,
                })
            }
        },
    };

    // Per-file timescales, sticky across the compilation unit until a new
    // directive (the preprocessor captures the directive seen in each
    // file). A unit with NO directive anywhere is the unmeasured default —
    // hard-error per design §4 (禁猜, PLAN §0.2 conflict unresolved).
    let mut last_ts: Option<String> = None;
    let mut per_file: HashMap<String, String> = HashMap::new();
    for f in files {
        if let Some(ts) = &f.timescale {
            last_ts = Some(ts.clone());
        }
        match &last_ts {
            Some(ts) => {
                per_file.insert(f.path.clone(), ts.clone());
            }
            None => {
                return Err(ElabError {
                    message: format!(
                        "riverilog: {0}: no `timescale directive and the no-timescale default is unmeasured on this oracle (see docs/riv-r2-design.md §4) (milestone R2)",
                        f.path
                    ),
                    exit: 2,
                })
            }
        }
    }

    // Global precision = min across files (design §4).
    let mut min_precision: Option<(String, u128)> = None;
    for f in files {
        let ts = &per_file[&f.path];
        let info = parse_timescale(ts).ok_or_else(|| ElabError {
            message: format!("riverilog: bad `timescale `{ts}`"),
            exit: 2,
        })?;
        let (_, rank) = parse_ts_half(&info.precision).ok_or_else(|| ElabError {
            message: format!("riverilog: bad precision `{}`", info.precision),
            exit: 2,
        })?;
        if min_precision.as_ref().map(|(_, r)| rank < *r).unwrap_or(true) {
            min_precision = Some((info.precision.clone(), rank));
        }
    }
    let global_precision = min_precision.unwrap().0;

    let top_mod = modules.iter().find(|m| m.name == top_name).unwrap();
    let top_file = sm.file_name(top_mod.span.file).to_string();
    let top_ts = per_file.get(&top_file).cloned().unwrap_or_default();
    let top_info = parse_timescale(&top_ts).ok_or_else(|| ElabError {
        message: format!("riverilog: bad `timescale `{top_ts}`"),
        exit: 2,
    })?;

    let mut design = riv_ir::Design {
        meta: riv_ir::Meta {
            top: top_name,
            timescale: top_ts.clone(),
            unit: top_info.unit.clone(),
            precision: global_precision,
            sources: files.iter().map(|f| f.path.clone()).collect(),
        },
        vars: Vec::new(),
        arrays: Vec::new(),
        assigns: Vec::new(),
        threads: Vec::new(),
        funcs: Vec::new(),
        strs: Vec::new(),
    };

    let mut elab = Elab {
        sm,
        vars: Vec::new(),
        params: HashMap::new(),
        funcs: HashMap::new(),
        ret_types: HashMap::new(),
    };
    let mut walker = Walker {
        e: &mut elab,
        design: &mut design,
        per_file: &per_file,
        top_ts: top_info,
    };
    walker.walk_module(top_mod)?;

    Ok(design)
}

struct Walker<'a> {
    e: &'a mut Elab<'a>,
    design: &'a mut riv_ir::Design,
    per_file: &'a HashMap<String, String>,
    top_ts: TsInfo,
}

impl<'a> Walker<'a> {
    fn file_of(&self, m: &Module) -> (String, TsInfo) {
        let name = self.e.sm.file_name(m.span.file).to_string();
        let ts = self.per_file.get(&name).cloned().unwrap_or_default();
        let info = parse_timescale(&ts).unwrap_or_else(|| self.top_ts.clone());
        (name, info)
    }

    fn line_of(&self, span: rcs_diag::Span) -> u32 {
        self.e.sm.line_col(span.file, span.start).0
    }

}

/// Build a fresh thread lowering session. `Elab` is only read from here
/// on (vars/params fully registered), so the borrow is independent of the
/// walker's design mutations.
fn make_lower<'x, 'y>(
    e: &'x Elab<'x>,
    file: &'y str,
    ts: &TsInfo,
) -> Lower<'x, 'y> {
    Lower {
        e,
        strs: Vec::new(),
        ops: Vec::new(),
        next_slot: 0,
        scopes: Vec::new(),
        reads: Vec::new(),
        file,
        scale_num: ts.num,
        scale_den: ts.den,
    }
}

/// Append a finished thread; merges its local format pool into the design
/// pool (rebasing SysCall fmt indices); returns the thread index.
fn commit(
    design: &mut riv_ir::Design,
    mut low: Lower<'_, '_>,
    kind: ThreadKind,
    sens: Vec<SensItem>,
) -> u32 {
    let off = design.strs.len() as u32;
    if off > 0 {
        for op in low.ops.iter_mut() {
            if let Op::SysCall { fmt: Some(i), .. } = op {
                *i += off;
            }
        }
    }
    design.strs.extend(low.strs);
    let idx = design.threads.len() as u32;
    design.threads.push(Thread {
        kind,
        sens,
        frame_len: low.next_slot,
        body: low.ops,
    });
    idx
}

impl<'a> Walker<'a> {

    fn walk_module(&mut self, m: &Module) -> ElabResult<()> {
        let (file, ts) = self.file_of(m);
        let file = &file;

        // Header parameters (referenced by port ranges and decls).
        let mut port_names: HashMap<String, ()> = HashMap::new();
        for pe in &m.params {
            self.decl_param(pe, file)?;
        }
        // ANSI header ports declare vars; non-ANSI ports stay names until
        // their body decl (implicit 1-bit wire if none arrives).
        for p in &m.ports {
            port_names.insert(p.name.clone(), ());
            if m.ansi {
                let kind = if p.is_reg {
                    VarKind::Reg
                } else {
                    VarKind::Net
                };
                let (w, hi, lo) = match &p.range {
                    Some(r) => {
                        let (hi, lo) = self.range_bounds(r, file)?;
                        (hi.abs_diff(lo) + 1, hi, lo)
                    }
                    None => (1, 0, 0),
                };
                self.declare_var(&p.name, w, p.signed, kind, file, p.span, hi, lo)?;
            }
        }

        for item in &m.items {
            match item {
                ModuleItem::Decl(d) => self.walk_decl(d, file)?,
                ModuleItem::ContAssign(pairs) => {
                    for pair in pairs {
                        self.lower_cont_assign(pair, file, &ts)?;
                    }
                }
                ModuleItem::Always { span, ctrl, body } => {
                    self.lower_always(*span, ctrl.as_ref(), body, file, &ts)?
                }
                ModuleItem::Initial { body, .. } => {
                    self.lower_thread(ThreadKind::Initial, Vec::new(), body, file, &ts)?
                }
                ModuleItem::Instance(i) => {
                    return Err(err_at(
                        file,
                        self.line_of(i.span),
                        "module instantiation needs the structural batch (milestone R4)",
                    ))
                }
                ModuleItem::Defparam(_) => {
                    return Err(err_at(
                        file,
                        self.line_of(m.span),
                        "defparam overrides need the structural batch (milestone R4)",
                    ))
                }
                ModuleItem::GenerateRegion(_)
                | ModuleItem::GenIf { .. }
                | ModuleItem::GenFor { .. }
                | ModuleItem::GenCase { .. }
                | ModuleItem::GenBlock { .. } => {
                    return Err(err_at(
                        file,
                        self.line_of(m.span),
                        "generate constructs need the structural batch (milestone R4)",
                    ))
                }
                ModuleItem::Function(f) => self.build_func(f, file, &ts)?,
                ModuleItem::Task(f) => self.build_func(f, file, &ts)?,
                ModuleItem::Empty => {}
            }
        }

        // Non-ANSI ports with no body declaration: implicit 1-bit wire
        // (IEEE default_nettype wire; corpus does not change it).
        if !m.ansi {
            let names: Vec<String> = port_names.keys().cloned().collect();
            for name in names {
                self.declare_var(&name, 1, false, VarKind::Net, file, m.span, 0, 0)?;
            }
        }
        Ok(())
    }

    fn range_bounds(&self, r: &ast::Range, file: &str) -> ElabResult<(u32, u32)> {
        let (mh, _) = self.const_eval_at(&r.msb, file)?;
        let (ml, _) = self.const_eval_at(&r.lsb, file)?;
        if mh.has_unknown() || ml.has_unknown() {
            return Err(err_at(
                file,
                self.line_of(r.span),
                "range bounds must be constant",
            ));
        }
        Ok((mh.to_u128_lossy() as u32, ml.to_u128_lossy() as u32))
    }

    fn const_eval_at(&self, e: &Expr, file: &str) -> ElabResult<(Bits, bool)> {
        let low = Lower {
            e: self.e,
            strs: Vec::new(),
            ops: Vec::new(),
            next_slot: 0,
            scopes: Vec::new(),
            reads: Vec::new(),
            file,
            scale_num: self.top_ts.num,
            scale_den: self.top_ts.den,
        };
        low.const_eval(e)
    }

    fn declare_var(
        &mut self,
        name: &str,
        width: u32,
        signed: bool,
        kind: VarKind,
        file: &str,
        span: rcs_diag::Span,
        msb: u32,
        lsb: u32,
    ) -> ElabResult<VarId> {
        if let Some(v) = self.e.vars.iter().find(|v| v.name == name) {
            return Err(err_at(
                file,
                self.line_of(span),
                format!("`{name}` is already declared (width {})", v.width),
            ));
        }
        if self.e.params.contains_key(name) {
            return Err(err_at(
                file,
                self.line_of(span),
                format!("`{name}` is already declared as a parameter"),
            ));
        }
        let id = self.design.vars.len() as u32;
        self.e.vars.push(VarInfo {
            name: name.to_string(),
            width,
            signed,
            kind,
            desc: msb >= lsb,
            msb,
            lsb,
        });
        self.design.vars.push(riv_ir::Var {
            id,
            name: name.to_string(),
            width,
            signed,
            kind,
            init: None,
            msb,
            lsb,
        });
        Ok(id)
    }

    fn decl_param(&mut self, pe: &ast::ParamEntry, file: &str) -> ElabResult<()> {
        let line = self.line_of(pe.span);
        match pe.ty {
            Some(ParamType::Real) | Some(ParamType::Realtime) => {
                return Err(err_at(
                    file,
                    line,
                    "`real` parameters are not supported (real types are not in the planned R2-R5 surface; see docs/RIVERILOG-PLAN.md §8)",
                ))
            }
            _ => {}
        }
        let ty_wh = match pe.ty {
            Some(ParamType::Integer) => Some((32u32, true)),
            Some(ParamType::Time) => Some((64u32, false)),
            _ => None,
        };
        let (w, signed) = match (&pe.range, ty_wh) {
            (Some(r), _) => (
                {
                    let (hi, lo) = self.range_bounds(r, file)?;
                    hi.abs_diff(lo) + 1
                },
                pe.signed || matches!(pe.ty, Some(ParamType::Integer)),
            ),
            (None, Some((w, s))) => (w, s),
            (None, None) => (32, pe.signed),
        };
        let (val, _) = self.const_eval_at(&pe.init, file)?;
        let val = if signed {
            val.sadjust(w)
        } else {
            val.uadjust(w)
        }
        .map_err(|e| err_at(file, line, format!("parameter value: {}", e.0)))?;
        if self.e.params.contains_key(&pe.name)
            || self.e.vars.iter().any(|v| v.name == pe.name)
        {
            return Err(err_at(file, line, format!("`{}` is already declared", pe.name)));
        }
        self.e.params.insert(pe.name.clone(), (val, signed));
        Ok(())
    }

    fn walk_decl(&mut self, d: &ast::Decl, file: &str) -> ElabResult<()> {
        let line = self.line_of(d.span);
        match d.kind {
            DeclKind::Parameter | DeclKind::Localparam => {
                for entry in &d.entries {
                    let Some(init) = &entry.init else {
                        return Err(err_at(
                            file,
                            self.line_of(entry.span),
                            format!("parameter `{}` has no value", entry.name),
                        ));
                    };
                    let pe = ast::ParamEntry {
                        span: entry.span,
                        name: entry.name.clone(),
                        ty: d.ty,
                        signed: d.signed,
                        range: d.range.clone(),
                        init: init.clone(),
                    };
                    self.decl_param(&pe, file)?;
                }
                Ok(())
            }
            DeclKind::Real | DeclKind::Realtime => Err(err_at(
                file,
                line,
                "`real` variables are not supported (real types are not in the planned R2-R5 surface; see docs/RIVERILOG-PLAN.md §8)",
            )),
            DeclKind::Event => Err(err_at(
                file,
                line,
                "named events land at the behavioral batch (milestone R3)",
            )),
            DeclKind::Genvar => Err(err_at(
                file,
                line,
                "genvars belong to generate constructs (milestone R4)",
            )),
            DeclKind::Integer | DeclKind::Time | DeclKind::Reg | DeclKind::Net(_) => {
                let (kind, w_default, signed_default) = match d.kind {
                    DeclKind::Integer => (VarKind::Reg, 32u32, true),
                    DeclKind::Time => (VarKind::Reg, 64u32, false),
                    DeclKind::Reg => (VarKind::Reg, 1u32, d.signed),
                    DeclKind::Net(_) => (VarKind::Net, 1u32, d.signed),
                    _ => unreachable!(),
                };
                let (w, hi, lo) = match &d.range {
                    Some(r) => {
                        let (hi, lo) = self.range_bounds(r, file)?;
                        (hi.abs_diff(lo) + 1, hi, lo)
                    }
                    None => (w_default, w_default - 1, 0),
                };
                for entry in &d.entries {
                    let eline = self.line_of(entry.span);
                    if !entry.unpacked.is_empty() {
                        return Err(err_at(
                            file,
                            eline,
                            "unpacked arrays (memories) land with the structural batch (milestone R4)",
                        ));
                    }
                    let id = match self.e.vars.iter().position(|v| v.name == entry.name) {
                        Some(existing) => {
                            // Re-declaration of an ANSI port by a body decl
                            // (legacy style) — keep the existing var.
                            existing as VarId
                        }
                        None => {
                            self.declare_var(&entry.name, w, signed_default, kind, file, entry.span, hi, lo)?
                        }
                    };
                    let Some(init) = &entry.init else { continue };
                    match kind {
                        VarKind::Reg => {
                            // Declaration initializer → an initial thread;
                            // its t0 change events wake @* waiters (the
                            // 21_always_comb golden depends on this).
                            let ts0 = self.top_ts.clone();
                            let mut low = make_lower(&*self.e, file, &ts0);
                            let v = low.lower(init, w)?;
                            low.emit(Op::AssignBlk { lv: Lval::Var { var: id }, src: v.slot });
                            low.emit(Op::EndThread);
                            commit(self.design, low, ThreadKind::Initial, Vec::new());
                        }
                        VarKind::Net => {
                            // Net declaration assignment = continuous driver.
                            self.net_driver_check(id, file, entry.span)?;
                            let ts0 = self.top_ts.clone();
                            let mut low = make_lower(&*self.e, file, &ts0);
                            let v = low.lower(init, w)?;
                            low.emit(Op::AssignBlk { lv: Lval::Var { var: id }, src: v.slot });
                            let reads = std::mem::take(&mut low.reads);
                            low.emit(Op::WaitEdge {
                                items: reads
                                    .into_iter()
                                    .map(|var| SensItem { var, edge: riv_ir::EdgeKind::Any })
                                    .collect(),
                            });
                            low.emit(Op::Jump { to: 0 });
                            low.emit(Op::EndThread);
                            let tidx = commit(self.design, low, ThreadKind::ContAssign, Vec::new());
                            self.design.assigns.push((tidx, id));
                        }
                        _ => unreachable!(),
                    }
                }
                Ok(())
            }
        }
    }

    fn net_driver_check(&self, id: VarId, file: &str, span: rcs_diag::Span) -> ElabResult<()> {
        let vi = self.e.vars[id as usize].clone();
        if vi.kind == VarKind::Reg {
            return Err(err_at(
                file,
                self.line_of(span),
                format!("continuous assignment to reg `{}` (regs are procedural-only)", vi.name),
            ));
        }
        if self.design.assigns.iter().any(|(_, v)| *v == id) {
            return Err(err_at(
                file,
                self.line_of(span),
                format!(
                    "multiple continuous drivers on `{}` need multi-driver arbitration (milestone R4)",
                    vi.name
                ),
            ));
        }
        Ok(())
    }

    fn lower_cont_assign(&mut self, pair: &ast::AssignPair, file: &str, ts: &TsInfo) -> ElabResult<()> {
        let mut low = make_lower(&*self.e, file, ts);
        // Lvalue + driver check + RHS all in ONE lowering session (dynamic
        // index lvalues emit ops).
        let lv = low.lower_lvalue(&pair.lhs)?;
        let lw = lval_width(&lv, &self.e.vars);
        if let Lval::Var { var } | Lval::Bit { var, .. } | Lval::Part { var, .. } = &lv {
            self.net_driver_check(*var, file, pair.span)?;
        }
        // Context = LHS width (probe: carry preservation in wire assigns).
        let v = low.lower(&pair.rhs, lw)?;
        low.emit(Op::AssignBlk { lv: lv.clone(), src: v.slot });
        let reads = std::mem::take(&mut low.reads);
        if reads.is_empty() {
            return Err(err_at(
                file,
                low.line_of(pair.span),
                "continuous assignment with an empty RHS read set",
            ));
        }
        low.emit(Op::WaitEdge {
            items: reads
                .into_iter()
                .map(|var| SensItem { var, edge: riv_ir::EdgeKind::Any })
                .collect(),
        });
        low.emit(Op::Jump { to: 0 });
        low.emit(Op::EndThread);
        let first_var = lval_first_var(&lv);
        let tidx = commit(self.design, low, ThreadKind::ContAssign, Vec::new());
        self.design.assigns.push((tidx, first_var));
        Ok(())
    }

    fn lower_always(
        &mut self,
        span: rcs_diag::Span,
        ctrl: Option<&ast::EventCtrl>,
        body: &Stmt,
        file: &str,
        ts: &TsInfo,
    ) -> ElabResult<()> {
        let line = self.line_of(span);
        match ctrl {
            None => {
                // `always` without event control: legal only with a delay
                // inside; a zero-delay loop would spin the kernel.
                if !stmt_has_timing(body) {
                    return Err(err_at(
                        file,
                        line,
                        "`always` block has no event control or delay (zero-delay loop)",
                    ));
                }
                self.lower_thread(ThreadKind::Always, Vec::new(), body, file, ts)
            }
            Some(ev) => {
                let items = {
                    let mut low = make_lower(&*self.e, file, ts);
                    low.sens_items(ev)?
                };
                // `@*` parses to an empty edge list — level-sensitive like
                // `@(a or b)`; both take the do-while (t0 first evaluation).
                let pure_level = items.iter().all(|i| i.edge == riv_ir::EdgeKind::Any);
                if pure_level {
                    // Combinational: execute once at t0, then wait on the
                    // body's read set (do-while shape; the 21_always_comb
                    // golden pins the t0 evaluation).
                    let mut low = make_lower(&*self.e, file, ts);
                    low.lower_stmt(body)?;
                    let reads = std::mem::take(&mut low.reads);
                    if reads.is_empty() {
                        return Err(err_at(file, line, "`@*` block has an empty sensitivity list"));
                    }
                    low.emit(Op::WaitEdge {
                        items: reads
                            .into_iter()
                            .map(|var| SensItem { var, edge: riv_ir::EdgeKind::Any })
                            .collect(),
                    });
                    low.emit(Op::Jump { to: 0 });
                    low.emit(Op::EndThread);
                    commit(self.design, low, ThreadKind::Always, Vec::new());
                    Ok(())
                } else {
                    // Edge-triggered: wait-first, sens armed by the kernel.
                    self.lower_thread(ThreadKind::Always, items, body, file, ts)
                }
            }
        }
    }

    fn lower_thread(
        &mut self,
        kind: ThreadKind,
        sens: Vec<SensItem>,
        body: &Stmt,
        file: &str,
        ts: &TsInfo,
    ) -> ElabResult<()> {
        let mut low = make_lower(&*self.e, file, ts);
        low.lower_stmt(body)?;
        if kind == ThreadKind::Always && sens.is_empty() {
            // Delay-driven `always` (`always #5 clk = ~clk;`): the body IS
            // the loop — wrap around forever. Edge-triggered always re-arm
            // through their thread sens at the kernel instead.
            low.emit(Op::Jump { to: 0 });
        } else {
            low.emit(Op::EndThread);
        }
        commit(self.design, low, kind, sens);
        Ok(())
    }
}

impl<'a> Walker<'a> {
    /// Task/function elaboration (design §3 frame model): slot 0 = the
    /// return value (functions; the function name IS this variable),
    /// formals from 1 in declaration order (ANSI ports, then old-style
    /// `input/output/inout` body decls), locals after. Static (default)
    /// binds to one shared frame at load; `automatic` gets per-activation
    /// frames — both expressed by Func.automatic + frame_len.
    fn build_func(&mut self, f: &ast::Function, file: &str, ts: &TsInfo) -> ElabResult<()> {
        if self.e.funcs.contains_key(&f.name)
            || self.e.vars.iter().any(|v| v.name == f.name)
        {
            return Err(err_at(
                file,
                self.line_of(f.span),
                format!("`{}` is already declared", f.name),
            ));
        }
        let ret_w = match (&f.ret_range, f.ret_type) {
            (Some(r), _) => {
                let (hi, lo) = self.range_bounds(r, file)?;
                hi.abs_diff(lo) + 1
            }
            (None, Some(ParamType::Integer)) => 32,
            (None, Some(ParamType::Time)) => 64,
            (None, _) => 1,
        };
        let mut low = make_lower(&*self.e, file, ts);
        let mut next_slot = 1u32; // slot 0 = ret
        let mut formals: Vec<(u32, ast::Direction, u32, bool)> = Vec::new();
        if !f.is_task {
            // The function name IS the return-value variable (IEEE §10.3).
            low.scopes.push((
                f.name.clone(),
                NameKind::Local { slot: 0, w: ret_w, signed: f.ret_signed },
            ));
        }
        // Formals: ANSI header ports first, then old-style dir decls.
        let mut formal_decls: Vec<(ast::Direction, &ast::Decl)> = Vec::new();
        for d in &f.ports {
            let dir = d.dir.unwrap_or(ast::Direction::Input);
            formal_decls.push((dir, d));
        }
        for d in &f.decls {
            if let Some(dir) = d.dir {
                formal_decls.push((dir, d));
            }
        }
        for (dir, d) in &formal_decls {
            let (w, _hi, _lo) = match &d.range {
                Some(r) => {
                    let (hi, lo) = self.range_bounds(r, file)?;
                    (hi.abs_diff(lo) + 1, hi, lo)
                }
                None => (1, 0, 0),
            };
            let signed = d.signed || matches!(d.ty, Some(ParamType::Integer));
            for entry in &d.entries {
                if !entry.unpacked.is_empty() {
                    return Err(err_at(
                        file,
                        self.line_of(entry.span),
                        "formal unpacked arrays land with the structural batch (milestone R4)",
                    ));
                }
                low.scopes.push((
                    entry.name.clone(),
                    NameKind::Local { slot: next_slot, w, signed },
                ));
                formals.push((next_slot, *dir, w, signed));
                next_slot += 1;
            }
        }
        // Locals: body decls without a direction.
        for d in &f.decls {
            if d.dir.is_some() {
                continue;
            }
            let (w_default, signed_default) = match d.kind {
                ast::DeclKind::Integer => (32u32, true),
                ast::DeclKind::Time => (64u32, false),
                ast::DeclKind::Real | ast::DeclKind::Realtime => {
                    return Err(err_at(
                        file,
                        self.line_of(d.span),
                        "`real` locals are not supported (real types are not in the planned R2-R5 surface; see docs/RIVERILOG-PLAN.md §8)",
                    ))
                }
                _ => (1u32, d.signed),
            };
            let (w, _hi, _lo) = match &d.range {
                Some(r) => {
                    let (hi, lo) = self.range_bounds(r, file)?;
                    (hi.abs_diff(lo) + 1, hi, lo)
                }
                None => (w_default, w_default - 1, 0),
            };
            for entry in &d.entries {
                if !entry.unpacked.is_empty() {
                    return Err(err_at(
                        file,
                        self.line_of(entry.span),
                        "local unpacked arrays land with the structural batch (milestone R4)",
                    ));
                }
                low.scopes.push((
                    entry.name.clone(),
                    NameKind::Local { slot: next_slot, w, signed: signed_default },
                ));
                next_slot += 1;
            }
        }
        low.next_slot = next_slot;
        for st in &f.body {
            low.lower_stmt(st)?;
        }
        low.emit(Op::Ret { src: 0 });
        let idx = self.design.funcs.len() as u32;
        self.design.funcs.push(riv_ir::Func {
            name: f.name.clone(),
            is_task: f.is_task,
            automatic: f.automatic,
            frame_len: low.next_slot,
            ret: if f.is_task { None } else { Some((ret_w, f.ret_signed)) },
            body: low.ops,
        });
        self.e.ret_types.insert(idx, (ret_w, f.ret_signed));
        self.e.funcs.insert(f.name.clone(), (idx, formals));
        Ok(())
    }
}

fn stmt_has_timing(s: &Stmt) -> bool {
    match s {
        Stmt::DelayCtrl { .. } | Stmt::EventCtrl { .. } => true,
        Stmt::Block { body, .. } => body.iter().any(stmt_has_timing),
        Stmt::If { then, els, .. } => {
            stmt_has_timing(then) || els.as_ref().map(|e| stmt_has_timing(e)).unwrap_or(false)
        }
        _ => false,
    }
}

fn lval_width(lv: &Lval, vars: &[VarInfo]) -> u32 {
    match lv {
        Lval::Var { var } => vars[*var as usize].width,
        Lval::Bit { .. } => 1,
        Lval::Part { hi, lo, .. } => hi.abs_diff(*lo) + 1,
        Lval::PartDyn { .. } => 1,
        Lval::Concat { parts } => parts.iter().map(|(_, w)| *w).sum(),
    }
}

fn lval_first_var(lv: &Lval) -> VarId {
    match lv {
        Lval::Var { var }
        | Lval::Bit { var, .. }
        | Lval::Part { var, .. }
        | Lval::PartDyn { var, .. } => *var,
        Lval::Concat { parts } => parts.first().map(|(p, _)| lval_first_var(p)).unwrap_or(0),
    }
}
