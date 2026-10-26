//! riv-elab — elaboration for the RIV design: top auto-detection,
//! module instantiation with parameter overrides and port binding (R4-a),
//! generate constructs (R4-b), unpacked arrays (R4-c), gate primitives
//! (R4-d), decl/param constant evaluation via vl-values, width/sign
//! propagation baked into ops at lowering (IEEE 5.4/5.5, self-determined
//! subcontexts), display format compilation including the StrLit→Bits
//! path, and the milestone hard-error surface.
//!
//! Per design §1 the crate may use `vl-syntax` + `rcs-diag` + `riv-ir` +
//! `vl-values`; semantics are implemented per spec (IEEE 1364-2005 +
//! probed oracle behavior), not copied from vl-sim.
//!
//! Scope model (R4): the top module, every module instance and every
//! generate-block iteration push a [`Scope`]; names/params/functions
//! resolve innermost-first. Design vars live in one flat arena with
//! dotted-path names (`u1.u2.a`), so hierarchical references resolve by
//! path. Same-width net↔net port connections collapse onto one storage
//! var; expression inputs and reg outputs become cont-assign
//! drivers/followers.

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
    /// `-i` (riverilog face, U-3b): skip instances of undefined modules
    /// silently — oracle-measured (iverilog 13.0): exit 0, no diagnostics.
    /// Other faces keep the hard error via `ElabInput::strict`.
    pub allow_missing_instances: bool,
}

impl<'a> ElabInput<'a> {
    /// Strict form (pre-U-3b behavior): hard error on undefined instances.
    pub fn strict(
        modules: &'a [Module],
        sm: &'a SourceMap,
        top: Option<&'a str>,
        files: &'a [FileTs],
    ) -> ElabInput<'a> {
        ElabInput {
            modules,
            sm,
            top,
            files,
            allow_missing_instances: false,
        }
    }
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
    /// Unpacked dims (empty = scalar). Storage lives in the kernel's array
    /// arena, indexed through the declared index space.
    pub unpacked: Vec<(u32, u32)>,
    /// Arrays of reg accept procedural element writes (wire arrays do
    /// not — cont-assign only).
    pub array_of_reg: bool,
}

/// One instantiation/generate scope (R4).
#[derive(Clone, Default)]
pub struct Scope {
    /// Instance-local simple name → design var id (ports included; a port
    /// bound to a parent net collapses onto the parent's id here).
    pub names: HashMap<String, VarId>,
    pub params: HashMap<String, (Bits, bool)>,
    /// Task/function registry: name → (func idx, formals in declaration
    /// order with their frame slots). Frame layout: slot 0 = ret value
    /// (functions; unused for tasks), formals from 1.
    pub funcs: HashMap<String, (u32, Vec<(u32, ast::Direction, u32, bool)>)>,
    /// Dotted hierarchical prefix of this scope, trailing dot included
    /// ("" for the top module).
    pub prefix: String,
    /// Unnamed generate-block counter (LRM genblkN, 1-based, per scope).
    pub genblk: u32,
    /// `#(.P(v))` overrides for a module instance being walked: consumed
    /// by decl_param as each header param is declared (the override
    /// REPLACES the default initializer; values were evaluated in the
    /// parent scope).
    pub param_overrides: HashMap<String, Bits>,
    /// Generate scopes are transparent: name resolution walks outward to
    /// the enclosing (module/instance) scope. Instance scopes are opaque.
    pub transparent: bool,
}

pub struct Elab<'a> {
    pub sm: &'a SourceMap,
    pub vars: Vec<VarInfo>,
    /// Scope stack; the LAST entry is current (module instances and
    /// generate iterations push/pop around their items).
    pub scopes: Vec<Scope>,
    /// func idx → (ret width, ret signed) for call-site typing.
    pub ret_types: HashMap<u32, (u32, bool)>,
    /// Collapsed-port aliases: child var id → the storage var it became.
    /// Name-based (hierarchical) resolution chases these; op rewrites make
    /// them invisible to already-lowered code.
    pub aliases: HashMap<VarId, VarId>,
}

impl<'a> Elab<'a> {
    pub fn cur(&self) -> &Scope {
        self.scopes.last().expect("scope stack")
    }
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
    let ElabInput { modules, sm, top, files, allow_missing_instances } = input;
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
    // file). A unit with NO directive anywhere runs at 1s/1s — measured on
    // iverilog 13.0 (nots.v probe: `#5` = 5 unit ticks, the $finish banner
    // prints "(1s)"); the R2-era unmeasured hard-error is lifted by it.
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
                per_file.insert(f.path.clone(), "1s/1s".to_string());
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
    // Files is one entry per source (the CLI loader always fills it); an
    // empty slice is a caller bug — error, not a panic across an FFI edge.
    let global_precision = min_precision.ok_or_else(|| ElabError {
        message: "riverilog: no source files reached elaboration".to_string(),
        exit: 2,
    })?
    .0;

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
        gates: Vec::new(),
        threads: Vec::new(),
        funcs: Vec::new(),
        strs: Vec::new(),
    };

    let mut elab = Elab {
        sm,
        vars: Vec::new(),
        scopes: vec![Scope { prefix: String::new(), ..Default::default() }],
        ret_types: HashMap::new(),
        aliases: HashMap::new(),
    };
    let mut walker = Walker {
        e: &mut elab,
        design: &mut design,
        per_file: &per_file,
        top_ts: top_info,
        modules,
        chain: Vec::new(),
        allow_missing: allow_missing_instances,
    };
    walker.walk_module(top_mod)?;

    Ok(design)
}

struct Walker<'a> {
    e: &'a mut Elab<'a>,
    design: &'a mut riv_ir::Design,
    per_file: &'a HashMap<String, String>,
    top_ts: TsInfo,
    modules: &'a [Module],
    /// Instantiation chain (module names) for recursion detection.
    chain: Vec<String>,
    /// `-i` (U-3b): undefined module instances are skipped silently.
    allow_missing: bool,
}

/// Build a fresh thread lowering session in the CURRENT scope. `Elab` is
/// only read from here on, so the borrow is independent of the walker's
/// design mutations.
fn make_lower<'x, 'y>(e: &'x Elab<'x>, file: &'y str, ts: &TsInfo) -> Lower<'x, 'y> {
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
fn commit(design: &mut riv_ir::Design, mut low: Lower<'_, '_>, kind: ThreadKind, sens: Vec<SensItem>) -> u32 {
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
    fn file_of_span(&self, span: rcs_diag::Span) -> String {
        self.e.sm.file_name(span.file).to_string()
    }

    fn ts_of_file(&self, file: &str) -> TsInfo {
        let ts = self.per_file.get(file).cloned().unwrap_or_default();
        parse_timescale(&ts).unwrap_or_else(|| self.top_ts.clone())
    }

    fn line_of(&self, span: rcs_diag::Span) -> u32 {
        self.e.sm.line_col(span.file, span.start).0
    }

    fn walk_module(&mut self, m: &Module) -> ElabResult<()> {
        let file = self.file_of_span(m.span);
        let file = &file;
        let ts = self.ts_of_file(file);

        // Header parameters (referenced by port ranges and decls); a child
        // instance's scope arrives with overrides already applied.
        let mut port_names: HashMap<String, ()> = HashMap::new();
        for pe in &m.params {
            self.decl_param(pe, file)?;
        }
        // ANSI header ports declare vars; non-ANSI ports stay names until
        // their body decl (implicit 1-bit wire if none arrives).
        for p in &m.ports {
            port_names.insert(p.name.clone(), ());
            if m.ansi {
                let kind = if p.is_reg { VarKind::Reg } else { VarKind::Net };
                let (w, hi, lo) = match &p.range {
                    Some(r) => {
                        let (hi, lo) = self.range_bounds(r, file)?;
                        (hi.abs_diff(lo) + 1, hi, lo)
                    }
                    None => (1, 0, 0),
                };
                self.declare_var(&p.name, w, p.signed, kind, file, p.span, hi, lo, Vec::new())?;
            }
        }

        // Pre-pass: register every body declaration's NAME (module items
        // are order-independent in Verilog — 59_i2c's `assign` references
        // a reg declared later). Initializers stay deferred to the walk.
        for item in &m.items {
            if let ModuleItem::Decl(d) = item {
                self.predeclare(d, file)?;
            }
        }

        for item in &m.items {
            match item {
                ModuleItem::Decl(d) => self.walk_decl(d, file, &ts)?,
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
                ModuleItem::Instance(i) => self.walk_instance(i, file, &ts)?,
                ModuleItem::Defparam(_) => {
                    return Err(err_at(
                        file,
                        self.line_of(m.span),
                        "defparam overrides are not planned (`#(.P(v))`/positional overrides are; see docs/RIVERILOG-PLAN.md §8.1)",
                    ))
                }
                ModuleItem::GenerateRegion(items) => {
                    for it in items {
                        self.walk_generate_item(it, file, &ts)?;
                    }
                }
                ModuleItem::GenIf { span, cond, then, els } => {
                    let taken = self.const_eval_bool(cond, *span, file)?;
                    self.walk_gen_scope(taken, then.as_slice(), els.as_deref().map(|v| &v[..]).unwrap_or(&[]), file, &ts)?;
                }
                ModuleItem::GenFor { span, var, init, cond, step, block, items } => {
                    self.walk_gen_for(*span, var, init, cond, step, block.as_deref(), items, file, &ts)?;
                }
                ModuleItem::GenCase { span, expr, items, default } => {
                    self.walk_gen_case(*span, expr, items, default.as_deref(), file)?;
                }
                ModuleItem::GenBlock { name, items, .. } => {
                    let blk = self.genblk_name(name.as_deref());
                    self.push_gen_scope(blk);
                    for it in items {
                        self.walk_generate_item(it, file, &ts)?;
                    }
                    self.pop_scope();
                }
                ModuleItem::Function(f) => self.build_func(f, file, &ts)?,
                ModuleItem::Task(f) => self.build_func(f, file, &ts)?,
                ModuleItem::Empty => {}
            }
        }

        // Non-ANSI ports with no body declaration: implicit 1-bit wire
        // (IEEE default_nettype wire; corpus does not change it). Ports
        // declared in the body (input/output decls) already registered in
        // the pre-pass — skip those.
        if !m.ansi {
            let names: Vec<String> = port_names.keys().cloned().collect();
            for name in names {
                if !self.e.cur().names.contains_key(&name) && !self.e.cur().params.contains_key(&name) {
                    self.declare_var(&name, 1, false, VarKind::Net, file, m.span, 0, 0, Vec::new())?;
                }
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Generate constructs (R4-b)
    // ------------------------------------------------------------------

    fn genblk_name(&mut self, label: Option<&str>) -> String {
        match label {
            Some(l) => l.to_string(),
            None => {
                let s = self.e.scopes.last_mut().unwrap();
                s.genblk += 1;
                format!("genblk{}", s.genblk)
            }
        }
    }

    /// Push a generate scope inheriting the current params (localparams of
    /// the enclosing module stay visible inside generate blocks).
    fn push_gen_scope(&mut self, name: String) {
        let prefix = format!("{}{name}.", self.e.cur().prefix);
        let params = self.e.cur().params.clone();
        self.e.scopes.push(Scope { prefix, params, transparent: true, ..Default::default() });
    }

    fn push_inst_scope(&mut self, inst_name: &str) {
        let prefix = format!("{}{inst_name}.", self.e.cur().prefix);
        self.e.scopes.push(Scope { prefix, transparent: false, ..Default::default() });
    }

    fn pop_scope(&mut self) {
        self.e.scopes.pop();
    }

    fn const_eval_bool(&mut self, e: &Expr, span: rcs_diag::Span, file: &str) -> ElabResult<bool> {
        let (b, _) = self.const_eval_at(e, file)?;
        if b.has_unknown() {
            return Err(err_at(file, self.line_of(span), "generate condition is unknown (x/z)"));
        }
        Ok(b.truthy().map_err(|er| err_at(file, self.line_of(span), er.0))? == Some(true))
    }

    fn walk_generate_item(&mut self, item: &ModuleItem, file: &str, ts: &TsInfo) -> ElabResult<()> {
        match item {
            ModuleItem::GenerateRegion(items) => {
                for it in items {
                    self.walk_generate_item(it, file, ts)?;
                }
                Ok(())
            }
            ModuleItem::GenIf { span, cond, then, els } => {
                let taken = self.const_eval_bool(cond, *span, file)?;
                self.walk_gen_scope(taken, then.as_slice(), els.as_deref().map(|v| &v[..]).unwrap_or(&[]), file, ts)
            }
            ModuleItem::GenFor { span, var, init, cond, step, block, items } => {
                self.walk_gen_for(*span, var, init, cond, step, block.as_deref(), items, file, ts)
            }
            ModuleItem::GenCase { span, expr, items, default } => {
                self.walk_gen_case(*span, expr, items, default.as_deref(), file)
            }
            ModuleItem::GenBlock { name, items, .. } => {
                let blk = self.genblk_name(name.as_deref());
                self.push_gen_scope(blk);
                for it in items {
                    self.walk_generate_item(it, file, ts)?;
                }
                self.pop_scope();
                Ok(())
            }
            other => self.walk_plain_item(other, file, ts),
        }
    }

    fn walk_plain_item(&mut self, item: &ModuleItem, file: &str, ts: &TsInfo) -> ElabResult<()> {
        match item {
            ModuleItem::Decl(d) => self.walk_decl(d, file, ts),
            ModuleItem::ContAssign(pairs) => {
                for pair in pairs {
                    self.lower_cont_assign(pair, file, ts)?;
                }
                Ok(())
            }
            ModuleItem::Always { span, ctrl, body } => self.lower_always(*span, ctrl.as_ref(), body, file, ts),
            ModuleItem::Initial { body, .. } => self.lower_thread(ThreadKind::Initial, Vec::new(), body, file, ts),
            ModuleItem::Instance(i) => self.walk_instance(i, file, ts),
            ModuleItem::Function(f) => self.build_func(f, file, ts),
            ModuleItem::Task(f) => self.build_func(f, file, ts),
            ModuleItem::Empty => Ok(()),
            ModuleItem::Defparam(_) => Err(err_at(
                file,
                0,
                "defparam overrides are not planned (`#(.P(v))`/positional overrides are; see docs/RIVERILOG-PLAN.md §8.1)",
            )),
            ModuleItem::GenIf { .. }
            | ModuleItem::GenFor { .. }
            | ModuleItem::GenCase { .. }
            | ModuleItem::GenBlock { .. }
            | ModuleItem::GenerateRegion(_) => {
                unreachable!("generate constructs dispatch through walk_generate_item")
            }
        }
    }

    fn walk_gen_scope(&mut self, taken: bool, then: &[ModuleItem], els: &[ModuleItem], file: &str, ts: &TsInfo) -> ElabResult<()> {
        let items = if taken { then } else { els };
        if items.is_empty() {
            return Ok(());
        }
        let blk = self.genblk_name(None);
        self.push_gen_scope(blk);
        for it in items {
            self.walk_generate_item(it, file, ts)?;
        }
        self.pop_scope();
        Ok(())
    }

    fn walk_gen_for(
        &mut self,
        span: rcs_diag::Span,
        var: &str,
        init: &Expr,
        cond: &Expr,
        step: &ast::AssignPair,
        block: Option<&str>,
        items: &[ModuleItem],
        file: &str,
        ts: &TsInfo,
    ) -> ElabResult<()> {
        let line = self.line_of(span);
        let (mut i, _) = self.const_eval_at(init, file)?;
        if i.has_unknown() {
            return Err(err_at(file, line, "genvar initializer is unknown (x/z)"));
        }
        let blk = self.genblk_name(block);
        let mut guard = 0u64;
        let one = Bits::from_u64(32, 1).unwrap();
        loop {
            self.push_gen_scope(format!("{blk}[{}]", i.to_u128_lossy()));
            self.e
                .scopes
                .last_mut()
                .unwrap()
                .params
                .insert(var.to_string(), (i.clone(), false));
            let go = self.const_eval_bool(cond, span, file)?;
            if !go {
                self.pop_scope();
                break;
            }
            for it in items {
                self.walk_generate_item(it, file, ts)?;
            }
            // Step RHS re-evaluated with the genvar bound to the current
            // iteration's value (`i = i + 1`).
            let (nv, _) = self.const_eval_at(&step.rhs, file)?;
            self.pop_scope();
            // Step advancing by 0 (or oscillating) never terminates.
            let advanced = nv
                .cmp_unsigned(&i)
                .map(|o| o.is_gt())
                .unwrap_or(true);
            if !advanced || nv.width > 64 || nv.to_u128_lossy() > 1 << 24 {
                return Err(err_at(
                    file,
                    line,
                    "generate loop step must monotonically increase the genvar",
                ));
            }
            let _ = one;
            i = nv;
            guard += 1;
            if guard > 1 << 16 {
                return Err(err_at(file, line, "generate loop exceeds 65536 iterations"));
            }
        }
        Ok(())
    }

    fn walk_gen_case(
        &mut self,
        span: rcs_diag::Span,
        expr: &Expr,
        items: &[(Vec<Expr>, Vec<ModuleItem>)],
        default: Option<&[ModuleItem]>,
        file: &str,
    ) -> ElabResult<()> {
        let line = self.line_of(span);
        let (v, _) = self.const_eval_at(expr, file)?;
        for (labels, body) in items {
            for l in labels {
                let (lv, _) = self.const_eval_at(l, file)?;
                if lv.case_eq(&v).map_err(|e| err_at(file, line, e.0))? == Bits::from_u64(1, 1).unwrap() {
                    let blk = self.genblk_name(None);
                    self.push_gen_scope(blk);
                    // The file/ts of the body items is the module's own; the
                    // caller threads it through walk_generate_item.
                    return self.walk_gen_items_body(body, file);
                }
            }
        }
        if let Some(body) = default {
            let blk = self.genblk_name(None);
            self.push_gen_scope(blk);
            return self.walk_gen_items_body(body, file);
        }
        Ok(())
    }

    fn walk_gen_items_body(&mut self, items: &[ModuleItem], file: &str) -> ElabResult<()> {
        // ts of the module file (items in a gen-case live in the module).
        let ts = self.ts_of_file(file);
        for it in items {
            self.walk_generate_item(it, file, &ts)?;
        }
        self.pop_scope();
        Ok(())
    }

    // ------------------------------------------------------------------
    // Module instantiation (R4-a) + gates (R4-d)
    // ------------------------------------------------------------------

    fn walk_instance(&mut self, i: &ast::Instance, file: &str, ts: &TsInfo) -> ElabResult<()> {
        if is_gate_name(&i.mod_name) {
            for unit in &i.insts {
                if unit.array.is_some() {
                    return Err(err_at(file, self.line_of(unit.span), "gate arrays are not supported yet"));
                }
                self.lower_gate(i, unit, file, ts)?;
            }
            return Ok(());
        }
        let Some(sub) = self.modules.iter().find(|m| m.name == i.mod_name) else {
            // -i (U-3b, oracle-measured): a missing module under `-i` is a
            // silent skip — no diagnostic, elaboration continues, exit 0.
            if self.allow_missing {
                return Ok(());
            }
            return Err(err_at(
                file,
                self.line_of(i.span),
                format!("module `{}` is not defined (searched the given sources and -y library dirs)", i.mod_name),
            ));
        };
        if self.chain.iter().any(|m| m == &sub.name) {
            return Err(err_at(
                file,
                self.line_of(i.span),
                format!("recursive instantiation of `{}` is not supported (chain: {})", sub.name, self.chain.join(" -> ")),
            ));
        }
        // Parameter overrides: named evaluated here (PARENT scope),
        // positional bound by header order (38_param_chain pins `.W(DW)`
        // referencing a parent localparam).
        let mut overrides: Vec<(String, Bits, bool)> = Vec::new();
        let mut positional_exprs: Vec<Option<&Expr>> = Vec::new();
        for pc in &i.params {
            match (&pc.name, &pc.expr) {
                (Some(n), Some(e)) => {
                    let (b, signed) = self.const_eval_at(e, file)?;
                    overrides.push((n.clone(), b, signed));
                }
                (Some(_), None) => {} // `.P()` = keep default
                (None, e) => positional_exprs.push(e.as_ref()),
            }
        }
        let all_param_names: Vec<String> = sub
            .params
            .iter()
            .map(|p| p.name.clone())
            .chain(sub.items.iter().flat_map(|it| match it {
                ModuleItem::Decl(d) if d.kind == DeclKind::Parameter => {
                    d.entries.iter().map(|e| e.name.clone()).collect()
                }
                _ => Vec::new(),
            }))
            .collect();
        for (idx, e) in positional_exprs.iter().enumerate() {
            let Some(e) = e else { continue };
            let Some(pname) = all_param_names.get(idx) else {
                return Err(err_at(
                    file,
                    self.line_of(i.span),
                    format!("module `{}` has no parameter #{}", sub.name, idx + 1),
                ));
            };
            let (b, signed) = self.const_eval_at(e, file)?;
            overrides.push((pname.clone(), b, signed));
        }
        for unit in &i.insts {
            // Instance arrays unroll into per-element scopes.
            let indices: Vec<Option<u64>> = match &unit.array {
                Some(r) => {
                    let (hi, lo) = self.range_bounds(r, file)?;
                    let (a, b) = (hi as u64, lo as u64);
                    let (lo_i, hi_i) = (a.min(b), a.max(b));
                    (lo_i..=hi_i).map(Some).collect()
                }
                None => vec![None],
            };
            for idx in indices {
                let inst_name = match idx {
                    Some(k) => format!("{}[{}]", unit.name, k),
                    None => unit.name.clone(),
                };
                self.instantiate(sub, &unit.conns, &overrides, &inst_name, file, i.span)?;
            }
        }
        Ok(())
    }

    fn instantiate(
        &mut self,
        sub: &Module,
        conns: &[ast::PortConn],
        overrides: &[(String, Bits, bool)],
        inst_name: &str,
        file: &str,
        span: rcs_diag::Span,
    ) -> ElabResult<()> {
        let line = self.line_of(span);
        let parent_scope = self.e.scopes.last().unwrap().clone();
        self.push_inst_scope(inst_name);
        // Header params are declared by walk_module; overrides (evaluated
        // in the PARENT scope) ride in the scope and replace each default
        // at declaration, re-typed at the declared width/sign (15_params).
        // Override targets include body-declared parameters too
        // (`module m(a); parameter W = 4;` — paramods pins it).
        let has_param = |m: &Module, name: &str| {
            m.params.iter().any(|p| p.name == name)
                || m.items.iter().any(|it| match it {
                    ModuleItem::Decl(d) => {
                        d.kind == DeclKind::Parameter
                            && d.entries.iter().any(|e| e.name == name)
                    }
                    _ => false,
                })
        };
        for (name, val, _) in overrides {
            if !has_param(sub, name) {
                return Err(err_at(file, line, format!("module `{}` has no parameter `{name}`", sub.name)));
            }
            self.e
                .scopes
                .last_mut()
                .unwrap()
                .param_overrides
                .insert(name.clone(), val.clone());
        }
        // Walk the child module body in its scope.
        self.chain.push(sub.name.clone());
        let r = self.walk_module(sub);
        self.chain.pop();
        if let Err(e) = r {
            self.pop_scope();
            return Err(e);
        }
        // ---- Port binding (parent exprs against child ports). ----
        let mut conns_by_name: HashMap<&str, &Expr> = HashMap::new();
        let mut positional: Vec<Option<&Expr>> = Vec::new();
        for c in conns {
            match (&c.name, &c.expr) {
                (Some(n), Some(e)) => {
                    conns_by_name.insert(n.as_str(), e);
                }
                (Some(_), None) => {} // `.p()` = unconnected
                (None, e) => positional.push(e.as_ref()),
            }
        }
        let mut bind_err: Option<ElabError> = None;
        for (pi, port) in sub.ports.iter().enumerate() {
            let conn: Option<&Expr> = conns_by_name
                .get(port.name.as_str())
                .copied()
                .or_else(|| positional.get(pi).copied().flatten());
            let Some(expr) = conn else { continue }; // unconnected: own var, undriven
            let Some(child_var) = self.e.cur().names.get(&port.name).copied() else {
                bind_err = Some(err_at(file, line, format!("port `{}` not declared in module `{}`", port.name, sub.name)));
                break;
            };
            let dir = port.dir.unwrap_or(ast::Direction::Input);
            let res = match (dir, expr) {
                (ast::Direction::Input, Expr::Ident { name, .. }) => {
                    let parent_var = parent_scope.names.get(name).copied();
                    match parent_var {
                        Some(pv)
                            if self.e.vars[pv as usize].kind == VarKind::Net
                                && self.e.vars[pv as usize].width == self.e.vars[child_var as usize].width =>
                        {
                            self.collapse_port(child_var, pv, &port.name)
                        }
                        _ => self.emit_port_driver(child_var, expr, &parent_scope, file),
                    }
                }
                (ast::Direction::Input, _) => self.emit_port_driver(child_var, expr, &parent_scope, file),
                (ast::Direction::Output, Expr::Ident { name, .. }) => {
                    let parent_var = parent_scope.names.get(name).copied();
                    let child_is_reg = self.e.vars[child_var as usize].kind == VarKind::Reg;
                    match parent_var {
                        Some(pv)
                            if !child_is_reg
                                && self.e.vars[pv as usize].kind == VarKind::Net
                                && self.e.vars[pv as usize].width == self.e.vars[child_var as usize].width =>
                        {
                            // Net output port: collapse the child port onto
                            // the parent var (both names = one storage; the
                            // child's internal drivers rebind to it).
                            self.collapse_port(child_var, pv, &port.name)
                        }
                        Some(_) => self.emit_port_follower(child_var, expr, &parent_scope, file),
                        None => Err(err_at(file, line, format!("port connection target `{name}` is not declared"))),
                    }
                }
                (ast::Direction::Output, _) => self.emit_port_follower(child_var, expr, &parent_scope, file),
                (ast::Direction::Inout, Expr::Ident { name, .. }) => {
                    let pv = parent_scope.names.get(name).copied().ok_or_else(|| {
                        err_at(file, line, format!("inout port target `{name}` is not declared"))
                    })?;
                    if self.e.vars[pv as usize].kind != VarKind::Net
                        || self.e.vars[child_var as usize].kind == VarKind::Reg
                        || self.e.vars[pv as usize].width != self.e.vars[child_var as usize].width
                    {
                        return Err(err_at(
                            file,
                            line,
                            "inout ports need same-width net-to-net connections (transactional merges land with the strength batch, milestone R9)",
                        ));
                    }
                    self.collapse_port(child_var, pv, &port.name)
                }
                (ast::Direction::Inout, _) => Err(err_at(file, line, "inout ports need plain identifier connections")),
            };
            if let Err(e) = res {
                bind_err = Some(e);
                break;
            }
        }
        self.pop_scope();
        match bind_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Same-width net↔net port connection: one storage var (the parent's).
    /// All already-emitted references to the child var id are rewritten.
    fn collapse_port(&mut self, child_var: VarId, parent_var: VarId, port_name: &str) -> ElabResult<()> {
        debug_assert_ne!(child_var, parent_var);
        // Design-side rewrite of every reference.
        let rewrite = |ops: &mut Vec<Op>| {
            for op in ops.iter_mut() {
                rewrite_var_ids(op, child_var, parent_var);
            }
        };
        for t in self.design.threads.iter_mut() {
            rewrite(&mut t.body);
            for s in t.sens.iter_mut() {
                if s.var == child_var {
                    s.var = parent_var;
                }
            }
        }
        for f in self.design.funcs.iter_mut() {
            rewrite(&mut f.body);
        }
        for g in self.design.gates.iter_mut() {
            if g.out == child_var {
                g.out = parent_var;
            }
            for t in g.inputs.iter_mut() {
                if *t == child_var {
                    *t = parent_var;
                }
            }
        }
        for (_, v) in self.design.assigns.iter_mut() {
            if *v == child_var {
                *v = parent_var;
            }
        }
        // Name maps in every scope that resolved to the child var (the
        // child's own port name, plus nested scopes that captured it).
        for s in self.e.scopes.iter_mut() {
            for (_, v) in s.names.iter_mut() {
                if *v == child_var {
                    *v = parent_var;
                }
            }
        }
        // The child port's var row is now dead — leave it in the arena
        // (ids are dense; the kernel tolerates unreferenced vars) but
        // record the alias so hierarchical NAME lookups follow it.
        self.e.aliases.insert(child_var, parent_var);
        self.e.scopes.last_mut().unwrap().names.insert(port_name.to_string(), parent_var);
        Ok(())
    }

    /// Input port driven by a parent expression: cont-assign-shaped thread
    /// lowered in the PARENT scope (reads parent vars, wakes on them). The
    /// child scope is swapped out for the parent's during lowering and
    /// restored after.
    fn emit_port_driver(&mut self, child_var: VarId, rhs: &Expr, parent_scope: &Scope, file: &str) -> ElabResult<()> {
        let w = self.e.vars[child_var as usize].width;
        let ts0 = self.ts_of_file(file);
        let saved = std::mem::replace(self.e.scopes.last_mut().unwrap(), parent_scope.clone());
        let r = (|| -> ElabResult<()> {
            let mut low = make_lower(&*self.e, file, &ts0);
            let v = low.lower(rhs, w)?;
            low.emit(Op::AssignBlk { lv: Lval::Var { var: child_var }, src: v.slot });
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
            self.design.assigns.push((tidx, child_var));
            Ok(())
        })();
        *self.e.scopes.last_mut().unwrap() = saved;
        r
    }

    /// Output port followed by the parent: cont-assign parent target =
    /// child port var (the child port can be a reg, procedurally driven).
    fn emit_port_follower(&mut self, child_var: VarId, target: &Expr, parent_scope: &Scope, file: &str) -> ElabResult<()> {
        let ts0 = self.ts_of_file(file);
        let saved = std::mem::replace(self.e.scopes.last_mut().unwrap(), parent_scope.clone());
        let r = (|| -> ElabResult<()> {
            let mut low = make_lower(&*self.e, file, &ts0);
            let lv = low.lower_lvalue(target)?;
            low.emit(Op::LoadVar { dst: 0, var: child_var });
            low.emit(Op::AssignBlk { lv: lv.clone(), src: 0 });
            low.emit(Op::WaitEdge {
                items: vec![SensItem { var: child_var, edge: riv_ir::EdgeKind::Any }],
            });
            low.emit(Op::Jump { to: 0 });
            low.emit(Op::EndThread);
            let first_var = lval_first_var(&lv);
            let tidx = commit(self.design, low, ThreadKind::ContAssign, Vec::new());
            self.design.assigns.push((tidx, first_var));
            Ok(())
        })();
        *self.e.scopes.last_mut().unwrap() = saved;
        r
    }

    /// Gate primitive instantiation (R4-d): `and #6 g(y, a, b)`.
    fn lower_gate(&mut self, i: &ast::Instance, unit: &ast::InstUnit, file: &str, ts: &TsInfo) -> ElabResult<()> {
        let line = self.line_of(unit.span);
        let kind = match i.mod_name.as_str() {
            "and" => riv_ir::GateKind::And,
            "nand" => riv_ir::GateKind::Nand,
            "or" => riv_ir::GateKind::Or,
            "nor" => riv_ir::GateKind::Nor,
            "xor" => riv_ir::GateKind::Xor,
            "xnor" => riv_ir::GateKind::Xnor,
            "buf" => riv_ir::GateKind::Buf,
            "not" => riv_ir::GateKind::Not,
            "bufif0" => riv_ir::GateKind::Bufif0,
            "bufif1" => riv_ir::GateKind::Bufif1,
            "notif0" => riv_ir::GateKind::Notif0,
            "notif1" => riv_ir::GateKind::Notif1,
            other => {
                return Err(err_at(
                    file,
                    line,
                    format!("gate primitive `{other}` is not supported (see docs/RIVERILOG-PLAN.md §8)"),
                ))
            }
        };
        // Delay: `#6` single value, or `#(r, f[, t])` (parser wraps the
        // paren list as a Concat). Plain decimals in the corpus; delay
        // literals scale by the module's unit/precision ratio.
        let lit_ticks = |e: &Expr| -> ElabResult<u64> {
            match e {
                Expr::NumLit(n) => {
                    let (num, den) = expr::parse_delay_rational(&n.digits).ok_or_else(|| {
                        err_at(file, line, format!("bad gate delay literal `{}`", n.digits))
                    })?;
                    Ok(expr::scale_rational(num, den, ts.num, ts.den))
                }
                _ => Err(err_at(file, line, "gate delays must be decimal literals here")),
            }
        };
        let (rise, fall, turnoff) = match i.params.first().map(|pc| &pc.expr) {
            None => (0u64, 0u64, 0u64),
            Some(Some(e)) => match e {
                Expr::Concat { parts, .. } if parts.len() == 2 || parts.len() == 3 => {
                    let r = lit_ticks(&parts[0])?;
                    let f = lit_ticks(&parts[1])?;
                    let t = match parts.get(2) {
                        Some(p) => lit_ticks(p)?,
                        None => 0,
                    };
                    (r, f, t)
                }
                other => {
                    let v = lit_ticks(other)?;
                    (v, v, v)
                }
            },
            Some(None) => (0, 0, 0),
        };
        // Terminals: output first, then inputs. buf/not may drive several
        // outputs (o1, o2, ..., in) — unrolled into one gate per output.
        let multi_out = matches!(kind, riv_ir::GateKind::Buf | riv_ir::GateKind::Not)
            && unit.conns.len() > 2;
        let mut terms = Vec::new();
        for c in &unit.conns {
            let Some(e) = &c.expr else {
                return Err(err_at(file, line, "gate terminal cannot be empty"));
            };
            let Expr::Ident { name, .. } = e else {
                return Err(err_at(
                    file,
                    line,
                    "gate terminals must be plain identifiers in R4 (expression terminals land with the strength batch, milestone R9)",
                ));
            };
            let Some(var) = self.e.cur().names.get(name).copied() else {
                return Err(err_at(file, line, format!("identifier `{name}` is not declared")));
            };
            terms.push(var);
        }
        // buf/not: (o1...oN, in) — multi-OUTPUT. Logic family:
        // (out, in1...inN) — multi-INPUT. bufif/notif: exactly 3.
        let (nin_min, nin_max) = match kind {
            riv_ir::GateKind::Buf | riv_ir::GateKind::Not => (2, usize::MAX),
            riv_ir::GateKind::Bufif0 | riv_ir::GateKind::Bufif1
            | riv_ir::GateKind::Notif0 | riv_ir::GateKind::Notif1 => (3, 3),
            _ => (3, usize::MAX),
        };
        if terms.len() < nin_min || terms.len() > nin_max {
            return Err(err_at(
                file,
                line,
                format!("gate `{}` takes {nin_min}+ terminals, got {}", i.mod_name, terms.len()),
            ));
        }
        for &t in terms.iter() {
            if self.e.vars[t as usize].width != 1 {
                return Err(err_at(
                    file,
                    line,
                    "vector gates are not supported yet (see docs/RIVERILOG-PLAN.md §8)",
                ));
            }
        }
        if multi_out {
            // (o1 ... oN, in): one gate per output, shared input.
            let input = *terms.last().unwrap();
            for &out in &terms[..terms.len() - 1] {
                self.net_driver_check(out, file, unit.span, true)?;
                self.design.gates.push(riv_ir::Gate {
                    kind,
                    out,
                    inputs: vec![input],
                    rise,
                    fall,
                    turnoff,
                });
            }
            return Ok(());
        }
        let out = terms[0];
        let inputs = terms[1..].to_vec();
        self.net_driver_check(out, file, unit.span, true)?;
        self.design.gates.push(riv_ir::Gate { kind, out, inputs, rise, fall, turnoff });
        Ok(())
    }

    // ------------------------------------------------------------------
    // Declarations
    // ------------------------------------------------------------------

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
        unpacked: Vec<(u32, u32)>,
    ) -> ElabResult<VarId> {
        let line = self.line_of(span);
        if self.e.cur().names.contains_key(name) {
            return Err(err_at(file, line, format!("`{name}` is already declared in this scope")));
        }
        if self.e.cur().params.contains_key(name) {
            return Err(err_at(file, line, format!("`{name}` is already declared as a parameter")));
        }
        let full = format!("{}{name}", self.e.cur().prefix);
        let id = self.design.vars.len() as u32;
        let array_of_reg = kind == VarKind::Reg;
        let kind = if !unpacked.is_empty() { VarKind::Array } else { kind };
        self.e.vars.push(VarInfo {
            name: full.clone(),
            width,
            signed,
            kind,
            desc: msb >= lsb,
            msb,
            lsb,
            unpacked: unpacked.clone(),
            array_of_reg,
        });
        self.design.vars.push(riv_ir::Var {
            id,
            name: full,
            width,
            signed,
            kind,
            init: None,
            msb,
            lsb,
        });
        if !unpacked.is_empty() {
            self.design.arrays.push(riv_ir::ArrayDim { var: id, dims: unpacked });
        }
        self.e.scopes.last_mut().unwrap().names.insert(name.to_string(), id);
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
        let (val, _) = match self.e.scopes.last_mut().unwrap().param_overrides.remove(&pe.name) {
            // Instance override (evaluated in the parent scope): replaces
            // the default initializer; still typed at the declared width.
            Some(v) => (v, false),
            None => self.const_eval_at(&pe.init, file)?,
        };
        let val = if signed {
            val.sadjust(w)
        } else {
            val.uadjust(w)
        }
        .map_err(|e| err_at(file, line, format!("parameter value: {}", e.0)))?;
        if self.e.cur().params.contains_key(&pe.name)
            || self.e.cur().names.contains_key(&pe.name)
        {
            return Err(err_at(file, line, format!("`{}` is already declared", pe.name)));
        }
        self.e
            .scopes
            .last_mut()
            .unwrap()
            .params
            .insert(pe.name.clone(), (val, signed));
        Ok(())
    }

    /// Register declaration names (no initializers) — the forward-ref
    /// pre-pass. Params/localparams are fully declared here (their values
    /// are order-significant: a width referencing a LATER localparam
    /// still errors, matching a conservative subset).
    fn predeclare(&mut self, d: &ast::Decl, file: &str) -> ElabResult<()> {
        match d.kind {
            DeclKind::Parameter | DeclKind::Localparam => self.walk_decl(d, file, &self.top_ts.clone()),
            DeclKind::Event => {
                for entry in &d.entries {
                    if !self.e.cur().names.contains_key(&entry.name) {
                        self.declare_var(&entry.name, 1, false, VarKind::Event, file, entry.span, 0, 0, Vec::new())?;
                    }
                }
                Ok(())
            }
            DeclKind::Genvar | DeclKind::Real | DeclKind::Realtime => Ok(()),
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
                    // `output [3:0] q; reg [3:0] q;` — the follow-up reg
                    // declaration PROMOTES the port net to a reg (Verilog
                    // port-completion rule; always01 pins it).
                    if d.kind == DeclKind::Reg {
                        if let Some(&id) = self.e.cur().names.get(&entry.name) {
                            if self.e.vars[id as usize].kind == VarKind::Net {
                                self.e.vars[id as usize].kind = VarKind::Reg;
                                self.design.vars[id as usize].kind = VarKind::Reg;
                                continue;
                            }
                        }
                    }
                    if self.e.cur().names.contains_key(&entry.name)
                        || self.e.cur().params.contains_key(&entry.name)
                    {
                        continue; // already registered (ANSI port or an earlier decl)
                    }
                    if entry.unpacked.len() > 1 {
                        return Err(err_at(file, self.line_of(entry.span), "multi-dimensional unpacked arrays are not supported yet (see docs/RIVERILOG-PLAN.md §8)"));
                    }
                    let unpacked: Vec<(u32, u32)> = entry
                        .unpacked
                        .iter()
                        .map(|r| self.range_bounds(r, file))
                        .collect::<Result<_, _>>()?;
                    self.declare_var(&entry.name, w, signed_default, kind, file, entry.span, hi, lo, unpacked)?;
                }
                Ok(())
            }
        }
    }

    fn walk_decl(&mut self, d: &ast::Decl, file: &str, ts: &TsInfo) -> ElabResult<()> {
        let line = self.line_of(d.span);
        match d.kind {
            DeclKind::Parameter | DeclKind::Localparam => {
                for entry in &d.entries {
                    if self.e.cur().params.contains_key(&entry.name)
                        || self.e.cur().names.contains_key(&entry.name)
                    {
                        continue; // pre-pass declared it (order-significant values)
                    }
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
            DeclKind::Event => {
                // Named events (R4-e): 1-bit event vars, no value storage.
                // The pre-pass already registered the names.
                for entry in &d.entries {
                    if !entry.unpacked.is_empty() {
                        return Err(err_at(file, line, "event arrays are not supported"));
                    }
                    if !self.e.cur().names.contains_key(&entry.name) {
                        self.declare_var(&entry.name, 1, false, VarKind::Event, file, entry.span, 0, 0, Vec::new())?;
                    }
                }
                Ok(())
            }
            DeclKind::Genvar => {
                // Genvars are elaboration-time constants bound by the
                // enclosing GenFor; declaring them just registers the name.
                for entry in &d.entries {
                    if self.e.cur().names.contains_key(&entry.name) {
                        return Err(err_at(file, line, format!("`{}` is already declared", entry.name)));
                    }
                }
                Ok(())
            }
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
                    if entry.unpacked.len() > 1 {
                        return Err(err_at(
                            file,
                            eline,
                            "multi-dimensional unpacked arrays are not supported yet (see docs/RIVERILOG-PLAN.md §8)",
                        ));
                    }
                    let unpacked: Vec<(u32, u32)> = entry
                        .unpacked
                        .iter()
                        .map(|r| self.range_bounds(r, file))
                        .collect::<Result<_, _>>()?;
                    let id = match self.e.cur().names.get(&entry.name).copied() {
                        Some(existing) => existing as VarId, // pre-registered
                        None => self.declare_var(
                            &entry.name,
                            w,
                            signed_default,
                            kind,
                            file,
                            entry.span,
                            hi,
                            lo,
                            unpacked,
                        )?,
                    };
                    let Some(init) = &entry.init else { continue };
                    if self.e.vars[id as usize].kind == VarKind::Array {
                        return Err(err_at(
                            file,
                            eline,
                            "array declaration initializers are not supported (initialize per-element or with $readmemh/b)",
                        ));
                    }
                    match self.e.vars[id as usize].kind {
                        VarKind::Reg => {
                            // Declaration initializer → an initial thread;
                            // its t0 change events wake @* waiters (the
                            // 21_always_comb golden depends on this).
                            let mut low = make_lower(&*self.e, file, ts);
                            let v = low.lower(init, w)?;
                            low.emit(Op::AssignBlk { lv: Lval::Var { var: id }, src: v.slot });
                            low.emit(Op::EndThread);
                            commit(self.design, low, ThreadKind::Initial, Vec::new());
                        }
                        VarKind::Net => {
                            // Net declaration assignment = continuous driver.
                            self.net_driver_check(id, file, entry.span, true)?;
                            let mut low = make_lower(&*self.e, file, ts);
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

    fn net_driver_check(&self, id: VarId, file: &str, span: rcs_diag::Span, whole_var: bool) -> ElabResult<()> {
        let vi = self.e.vars[id as usize].clone();
        if vi.kind == VarKind::Reg {
            return Err(err_at(
                file,
                self.line_of(span),
                format!("continuous assignment to reg `{}` (regs are procedural-only)", vi.name),
            ));
        }
        if whole_var && self.design.assigns.iter().any(|(_, v)| *v == id) {
            return Err(err_at(
                file,
                self.line_of(span),
                format!(
                    "multiple continuous drivers on `{}` need multi-driver arbitration (milestone R4 strength batch)",
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
        if let Lval::Var { var } = &lv {
            self.net_driver_check(*var, file, pair.span, true)?;
        } else if let Lval::Bit { var, .. } | Lval::Part { var, .. } = &lv {
            // Bit/part-level assigns from generate unrolls are legal when
            // disjoint; overlap arbitration stays with the strength batch.
            self.net_driver_check(*var, file, pair.span, false)?;
        }
        // Context = LHS width (probe: carry preservation in wire assigns).
        let v = low.lower(&pair.rhs, lw)?;
        low.emit(Op::AssignBlk { lv: lv.clone(), src: v.slot });
        let reads = std::mem::take(&mut low.reads);
        if reads.is_empty() {
            // Constant RHS (`assign y = 1'b0;`): a one-shot initial thread;
            // nothing to wait on.
            low.emit(Op::EndThread);
            commit(self.design, low, ThreadKind::Initial, Vec::new());
            return Ok(());
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
        if self.e.cur().funcs.contains_key(&f.name)
            || self.e.cur().names.contains_key(&f.name)
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
                        "formal unpacked arrays are not supported yet (see docs/RIVERILOG-PLAN.md §8)",
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
                        "local unpacked arrays are not supported yet (see docs/RIVERILOG-PLAN.md §8)",
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
            name: format!("{}{}", self.e.cur().prefix, f.name),
            is_task: f.is_task,
            automatic: f.automatic,
            frame_len: low.next_slot,
            ret: if f.is_task { None } else { Some((ret_w, f.ret_signed)) },
            body: low.ops,
        });
        self.e.ret_types.insert(idx, (ret_w, f.ret_signed));
        self.e
            .scopes
            .last_mut()
            .unwrap()
            .funcs
            .insert(f.name.clone(), (idx, formals));
        Ok(())
    }
}

fn is_gate_name(n: &str) -> bool {
    matches!(
        n,
        "and" | "nand" | "or" | "nor" | "xor" | "xnor" | "buf" | "not" | "bufif0" | "bufif1"
            | "notif0" | "notif1" | "pullup" | "pulldown"
    )
}

/// Rewrite every reference to var `from` → `to` inside one op (port
/// collapsing).
fn rewrite_var_ids(op: &mut Op, from: VarId, to: VarId) {
    let lv = |l: &mut Lval| rewrite_lval(l, from, to);
    let sens = |items: &mut Vec<SensItem>| {
        for s in items.iter_mut() {
            if s.var == from {
                s.var = to;
            }
        }
    };
    match op {
        Op::LoadVar { var, .. }
        | Op::LoadBit { var, .. }
        | Op::LoadPart { var, .. }
        | Op::LoadPartDyn { var, .. }
        | Op::LoadElem { var, .. } => {
            if *var == from {
                *var = to;
            }
        }
        Op::AssignBlk { lv: l, .. }
        | Op::AssignNba { lv: l, .. }
        | Op::AssignForce { lv: l, .. }
        | Op::Release { lv: l } => lv(l),
        Op::Call { outs, .. } => {
            for (l, _) in outs.iter_mut() {
                lv(l);
            }
        }
        Op::WaitEdge { items } => sens(items),
        _ => {}
    }
}

fn rewrite_lval(l: &mut Lval, from: VarId, to: VarId) {
    match l {
        Lval::Var { var }
        | Lval::Bit { var, .. }
        | Lval::Part { var, .. }
        | Lval::PartDyn { var, .. }
        | Lval::Elem { var, .. } => {
            if *var == from {
                *var = to;
            }
        }
        Lval::Concat { parts } => {
            for (p, _) in parts.iter_mut() {
                rewrite_lval(p, from, to);
            }
        }
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
        Lval::Elem { var, .. } => vars[*var as usize].width,
        Lval::Concat { parts } => parts.iter().map(|(_, w)| *w).sum(),
    }
}

fn lval_first_var(lv: &Lval) -> VarId {
    match lv {
        Lval::Var { var }
        | Lval::Bit { var, .. }
        | Lval::Part { var, .. }
        | Lval::PartDyn { var, .. }
        | Lval::Elem { var, .. } => *var,
        Lval::Concat { parts } => parts.first().map(|(p, _)| lval_first_var(p)).unwrap_or(0),
    }
}
