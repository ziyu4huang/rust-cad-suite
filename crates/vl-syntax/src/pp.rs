//! Preprocessor: `` `define `` expansion, conditionals, includes.
//!
//! v0 scope (per adversarial review of PLAN-DRAFT, finding #9): object-like
//! and single-level function-like macros, `` `` `` paste, `` `" `` stringify,
//! `` `ifdef/`ifndef/`elsif/`else/`endif ``, `` `include `` with `-I` search
//! (cycles = hard error), `` `timescale ``/`` `default_nettype `` capture,
//! `` `line ``/`` `pragma `` et al. accepted-and-ignored, `` `__FILE__ ``/
//! `` `__LINE__ ``. Macro bodies are re-lexed against a pseudo source file
//! `<macro:NAME>`, so diagnostics inside bodies show the body text.

use crate::lexer::{self, LexOptions};
use crate::token::{DirKind, Tok, Token};
use rcs_diag::{DiagSink, FileId, SourceMap, Span};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Default, Clone)]
pub struct PpOptions {
    pub include_dirs: Vec<PathBuf>,
    /// `-D NAME` / `-D NAME=VALUE` command-line defines.
    pub predefines: Vec<String>,
}

pub struct PpOutput {
    pub tokens: Vec<Token>,
    pub timescale: Option<String>,
    pub default_nettype: Option<String>,
}

/// Preprocess a real file from disk.
pub fn preprocess_file(
    path: &Path,
    sm: &mut SourceMap,
    diags: &mut DiagSink,
    opts: &PpOptions,
) -> Option<PpOutput> {
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            diags.error(
                "VL-E010",
                Span::default(),
                format!("cannot open input file `{}`: {e}", path.display()),
            );
            return None;
        }
    };
    let file = sm.add(path.display().to_string(), src.clone());
    let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut pp = Pp::new(sm, diags, opts);
    pp.run_file(file, src, dir);
    Some(pp.finish())
}

/// Preprocess an in-memory source (tests, future language-server use).
pub fn preprocess_source(
    name: &str,
    src: &str,
    sm: &mut SourceMap,
    diags: &mut DiagSink,
    opts: &PpOptions,
) -> PpOutput {
    let file = sm.add(name.to_string(), src.to_string());
    let mut pp = Pp::new(sm, diags, opts);
    pp.run_file(file, src.to_string(), PathBuf::from("."));
    pp.finish()
}

#[derive(Clone)]
struct MacroDef {
    params: Vec<String>,
    is_func: bool,
    body: Vec<Token>,
}

struct CondFrame {
    parent_active: bool,
    taken: bool,
    in_else: bool,
    active: bool,
}

struct Pp<'a> {
    sm: &'a mut SourceMap,
    diags: &'a mut DiagSink,
    include_dirs: Vec<PathBuf>,
    defines: HashMap<String, MacroDef>,
    out: Vec<Token>,
    timescale: Option<String>,
    default_nettype: Option<String>,
    cond: Vec<CondFrame>,
    depth: usize,
    include_depth: usize,
    cur_dir: PathBuf,
    cur_file: FileId,
}

impl<'a> Pp<'a> {
    fn new(sm: &'a mut SourceMap, diags: &'a mut DiagSink, opts: &PpOptions) -> Self {
        let mut pp = Pp {
            sm,
            diags,
            include_dirs: opts.include_dirs.clone(),
            defines: HashMap::new(),
            out: Vec::new(),
            timescale: None,
            default_nettype: None,
            cond: Vec::new(),
            depth: 0,
            include_depth: 0,
            cur_dir: PathBuf::from("."),
            cur_file: FileId(0),
        };
        for pd in &opts.predefines {
            let (name, value) = match pd.split_once('=') {
                Some((n, v)) => (n.to_string(), v.to_string()),
                None => (pd.clone(), "1".to_string()),
            };
            pp.store_macro(&name, Vec::new(), false, &value, Span::default());
        }
        pp
    }

    fn active(&self) -> bool {
        self.cond.iter().all(|f| f.active)
    }

    fn run_file(&mut self, file: FileId, src: String, dir: PathBuf) {
        let toks = lexer::lex(&src, file, self.diags);
        let saved_dir = std::mem::replace(&mut self.cur_dir, dir);
        let saved_file = std::mem::replace(&mut self.cur_file, file);
        self.run(&toks);
        self.cur_dir = saved_dir;
        self.cur_file = saved_file;
    }

    fn run(&mut self, toks: &[Token]) {
        let mut i = 0;
        while i < toks.len() {
            let t = &toks[i];
            match t.kind {
                Tok::Directive(d) => {
                    self.directive(d, t);
                    i += 1;
                }
                Tok::MacroId => {
                    if self.active() {
                        match self.try_expand_at(toks, i) {
                            Some(next_i) => i = next_i,
                            None => {
                                // Undefined macro: diagnostic already emitted.
                                i += 1;
                            }
                        }
                    } else {
                        i += 1;
                    }
                }
                Tok::MacroQuote | Tok::MacroPaste => {
                    if self.active() {
                        self.diags.error(
                            "VL-E013",
                            t.span,
                            "`\" or `` outside a macro body",
                        );
                    }
                    i += 1;
                }
                Tok::Ident if t.text == "__LINE__" || t.text == "__FILE__" => {
                    // IEEE 1364-2005 §19.2: these appear WITHOUT a leading
                    // backtick but are preprocessor-interpreted.
                    if self.active() {
                        if t.text == "__LINE__" {
                            let (line, _) = self.sm.line_col(t.span.file, t.span.start);
                            self.out
                                .push(Token::new(Tok::IntLit, t.span, line.to_string()));
                        } else {
                            self.out.push(Token::new(
                                Tok::StrLit,
                                t.span,
                                self.sm.file_name(t.span.file).to_string(),
                            ));
                        }
                    }
                    i += 1;
                }
                Tok::Eof => i += 1, // each included file contributes one; only the outermost may stay — filtered here, finish() re-adds via the outermost stream
                Tok::Error => i += 1, // lexer already reported
                _ => {
                    if self.active() {
                        self.out.push(t.clone());
                    }
                    i += 1;
                }
            }
        }
    }

    fn directive(&mut self, d: DirKind, t: &Token) {
        match d {
            DirKind::Define => {
                if self.active() {
                    self.do_define(t);
                }
            }
            DirKind::Undef => {
                if self.active() {
                    let name = first_word(&t.text);
                    if name.is_empty() {
                        self.diags.error("VL-E012", t.span, "`undef requires a macro name");
                    } else {
                        self.defines.remove(name);
                    }
                }
            }
            DirKind::Ifdef | DirKind::Ifndef => {
                let name = first_word(&t.text).to_string();
                if name.is_empty() {
                    self.diags.error("VL-E012", t.span, "`ifdef requires a macro name");
                    self.cond.push(CondFrame {
                        parent_active: self.active(),
                        taken: false,
                        in_else: false,
                        active: false,
                    });
                    return;
                }
                let parent = self.cond.iter().all(|f| f.active);
                let mut on = self.defines.contains_key(&name);
                if d == DirKind::Ifndef {
                    on = !on;
                }
                self.cond.push(CondFrame {
                    parent_active: parent,
                    taken: parent && on,
                    in_else: false,
                    active: parent && on,
                });
            }
            DirKind::Elsif => {
                let name = first_word(&t.text).to_string();
                match self.cond.last_mut() {
                    Some(f) => {
                        if f.in_else {
                            self.diags.error("VL-E012", t.span, "`elsif after `else");
                        }
                        let on = f.parent_active && !f.taken && self.defines.contains_key(&name);
                        f.active = on;
                        f.taken |= on;
                    }
                    None => self.diags.error("VL-E012", t.span, "`elsif without `ifdef"),
                }
            }
            DirKind::Else => match self.cond.last_mut() {
                Some(f) => {
                    if f.in_else {
                        self.diags.error("VL-E012", t.span, "duplicate `else");
                    }
                    f.active = f.parent_active && !f.taken;
                    f.taken = true;
                    f.in_else = true;
                }
                None => self.diags.error("VL-E012", t.span, "`else without `ifdef"),
            },
            DirKind::Endif => {
                if self.cond.pop().is_none() {
                    self.diags.error("VL-E012", t.span, "`endif without `ifdef");
                }
            }
            DirKind::Include => {
                if self.active() {
                    self.do_include(t);
                }
            }
            DirKind::Timescale => {
                if self.active() {
                    let unit = t.text.trim().to_string();
                    if self.timescale.is_none() {
                        self.timescale = Some(unit);
                    }
                }
            }
            DirKind::DefaultNettype => {
                if self.active() {
                    self.default_nettype = Some(t.text.trim().to_string());
                }
            }
            // Accepted and ignored in v0: `resetall (timescale reset is a
            // documented v0 gap), `line (no span remapping yet), pragmas,
            // celldefine, keyword sets.
            _ => {}
        }
    }

    fn do_define(&mut self, t: &Token) {
        let text = t.text.as_str();
        let mut chars = text.char_indices();
        // Macro name.
        let name_end = match chars.next() {
            Some((_, c)) if c.is_ascii_alphabetic() || c == '_' => {
                let mut end = text.len();
                for (i, c) in chars {
                    if !(c.is_ascii_alphanumeric() || c == '_' || c == '$') {
                        end = i;
                        break;
                    }
                }
                end
            }
            _ => {
                self.diags
                    .error("VL-E012", t.span, "`define requires a macro name");
                return;
            }
        };
        let name = text[..name_end].to_string();
        let rest = &text[name_end..];
        // Function-like iff `(` follows the name with no whitespace
        // (IEEE 1800-2017 §22.5.1).
        if rest.starts_with('(') {
            let (params, after) = match parse_macro_params(rest) {
                Some(p) => p,
                None => {
                    self.diags.error("VL-E012", t.span,
                        format!("unbalanced parameter list in `define {name}"),
                    );
                    return;
                }
            };
            let body = &after[after
                .find(|c: char| !c.is_whitespace())
                .unwrap_or(after.len())..];
            self.store_macro(&name, params, true, body, t.span);
        } else {
            let body = &rest[rest
                .find(|c: char| !c.is_whitespace())
                .unwrap_or(rest.len())..];
            self.store_macro(&name, Vec::new(), false, body, t.span);
        }
    }

    fn store_macro(
        &mut self,
        name: &str,
        params: Vec<String>,
        is_func: bool,
        body_src: &str,
        def_span: Span,
    ) {
        let body = if body_src.is_empty() {
            Vec::new()
        } else {
            let fid = self.sm.add(format!("<macro:{name}>"), body_src.to_string());
            let mut toks = lexer::lex_with(
                body_src,
                fid,
                self.diags,
                LexOptions::default(),
            );
            toks.retain(|t| t.kind != Tok::Eof);
            toks
        };
        if let Some(prev) = self.defines.get(name) {
            if prev.body != body || prev.params != params {
                self.diags.warning(
                    "VL-W001",
                    def_span,
                    format!("redefinition of macro `{name}"),
                );
            }
        }
        self.defines.insert(
            name.to_string(),
            MacroDef {
                params,
                is_func,
                body,
            },
        );
    }

    fn do_include(&mut self, t: &Token) {
        let spec = t.text.trim();
        let fname = spec
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .or_else(|| spec.strip_prefix('<').and_then(|s| s.strip_suffix('>')));
        let Some(fname) = fname else {
            self.diags.error("VL-E012", t.span,
                format!("`include expects \"file\" or <file>, got `{spec}`"),
            );
            return;
        };
        if self.include_depth >= 64 {
            self.diags
                .error("VL-E012", t.span, "`include nesting too deep (include cycle?)");
            return;
        }
        let mut candidates: Vec<PathBuf> = vec![self.cur_dir.join(fname)];
        for d in &self.include_dirs {
            candidates.push(d.join(fname));
        }
        for cand in &candidates {
            if let Ok(src) = std::fs::read_to_string(cand) {
                let dir = cand.parent().unwrap_or(Path::new(".")).to_path_buf();
                let fid = self.sm.add(cand.display().to_string(), src.clone());
                self.include_depth += 1;
                self.run_file(fid, src, dir);
                self.include_depth -= 1;
                return;
            }
        }
        let searched: Vec<String> = candidates.iter().map(|p| p.display().to_string()).collect();
        self.diags.error("VL-E010", t.span,
            format!("cannot find `include file `{fname}` (searched: {})", searched.join(", ")),
        );
    }

    /// If this is a function-like macro use, read its argument list starting
    /// at `i` in `toks`. Returns `(args, index_after)` or `None` when `toks[i]`
    /// is not `(` (with a diagnostic emitted only if the macro requires args).
    fn read_macro_args<'t>(
        &mut self,
        def: &MacroDef,
        name: &str,
        toks: &'t [Token],
        i: usize,
        use_span: Span,
    ) -> Option<(Vec<Vec<Token>>, usize)> {
        if i >= toks.len() || toks[i].kind != Tok::Punct(crate::token::Punct::LParen) {
            if def.is_func {
                self.diags.error("VL-E012", use_span,
                    format!("macro `{name} used with too few arguments (expected `(...)` )"),
                );
            }
            return None;
        }
        let mut args: Vec<Vec<Token>> = Vec::new();
        let mut cur: Vec<Token> = Vec::new();
        let mut depth = 0usize;
        let mut j = i + 1;
        loop {
            if j >= toks.len() {
                self.diags.error("VL-E012", use_span, "unterminated macro argument list");
                return None;
            }
            let t = &toks[j];
            match t.kind {
                Tok::Punct(crate::token::Punct::LParen) => {
                    depth += 1;
                    cur.push(t.clone());
                }
                Tok::Punct(crate::token::Punct::RParen) => {
                    if depth == 0 {
                        if !(cur.is_empty() && args.is_empty()) {
                            args.push(std::mem::take(&mut cur));
                        }
                        return Some((args, j + 1));
                    }
                    depth -= 1;
                    cur.push(t.clone());
                }
                Tok::Punct(crate::token::Punct::Comma) if depth == 0 => {
                    args.push(std::mem::take(&mut cur));
                }
                Tok::Eof => {
                    self.diags.error("VL-E012", use_span, "unterminated macro argument list");
                    return None;
                }
                _ => cur.push(t.clone()),
            }
            j += 1;
        }
    }

    /// Expand a macro use at `toks[i]` (which must be a `MacroId`). Returns
    /// the index just past the use (and its arguments). Emits diagnostics and
    /// returns `None` for undefined macros.
    fn try_expand_at(&mut self, toks: &[Token], i: usize) -> Option<usize> {
        let t = &toks[i];
        let name = t.text.clone();
        if name == "__FILE__" {
            self.out.push(Token::new(
                Tok::StrLit,
                t.span,
                self.sm.file_name(self.cur_file).to_string(),
            ));
            return Some(i + 1);
        }
        if name == "__LINE__" {
            let (line, _) = self.sm.line_col(t.span.file, t.span.start);
            self.out
                .push(Token::new(Tok::IntLit, t.span, line.to_string()));
            return Some(i + 1);
        }
        let Some(def) = self.defines.get(&name).cloned() else {
            self.diags
                .error("VL-E011", t.span, format!("undefined macro `{name}`"));
            return None;
        };
        self.depth += 1;
        if self.depth > 200 {
            self.diags
                .error("VL-E012", t.span, "macro expansion too deep (recursive `define?)");
            self.depth -= 1;
            return Some(i + 1);
        }
        if def.is_func {
            let (args, next) = match self.read_macro_args(&def, &name, toks, i + 1, t.span) {
                Some(x) => x,
                None => {
                    self.depth -= 1;
                    return None;
                }
            };
            if args.len() != def.params.len() {
                self.diags.error("VL-E012", t.span,
                    format!(
                        "macro `{name} expects {} argument(s), got {}",
                        def.params.len(),
                        args.len()
                    ),
                );
                self.depth -= 1;
                return Some(next);
            }
            let expanded = self.substitute(&def, &args);
            let mut res = self.expand_all(expanded);
            self.out.append(&mut res);
            self.depth -= 1;
            Some(next)
        } else {
            let expanded = self.substitute(&def, &[]);
            let mut res = self.expand_all(expanded);
            self.out.append(&mut res);
            self.depth -= 1;
            Some(i + 1)
        }
    }

    fn substitute(&self, def: &MacroDef, args: &[Vec<Token>]) -> Vec<Token> {
        let mut out = Vec::new();
        for tok in &def.body {
            // Macro parameters appear in the body as plain identifiers (they
            // were lexed before the parameter list was known); `` `p `` is
            // accepted too (lexes as MacroId).
            if matches!(tok.kind, Tok::Ident | Tok::MacroId) {
                if let Some(idx) = def.params.iter().position(|p| p == &tok.text) {
                    out.extend(args.get(idx).cloned().unwrap_or_default());
                    continue;
                }
            }
            out.push(tok.clone());
        }
        out
    }

    /// Rescan: expand nested macro uses; handle `` `" `` stringify and
    /// `` `` `` paste. Never recurses through `self.out`.
    fn expand_all(&mut self, toks: Vec<Token>) -> Vec<Token> {
        let mut out: Vec<Token> = Vec::new();
        let mut i = 0;
        while i < toks.len() {
            let t = &toks[i];
            match t.kind {
                Tok::MacroId if t.text == "__LINE__" || t.text == "__FILE__" => {
                    if t.text == "__LINE__" {
                        let (line, _) = self.sm.line_col(t.span.file, t.span.start);
                        out.push(Token::new(Tok::IntLit, t.span, line.to_string()));
                    } else {
                        out.push(Token::new(
                            Tok::StrLit,
                            t.span,
                            self.sm.file_name(t.span.file).to_string(),
                        ));
                    }
                    i += 1;
                }
                Tok::MacroId => {
                    let Some(def) = self.defines.get(&t.text).cloned() else {
                        // Unknown name inside an expanded body: keep as plain
                        // identifier (matches how the LRM treats token pasting
                        // results that are not macros).
                        out.push(Token::new(Tok::Ident, t.span, t.text.clone()));
                        i += 1;
                        continue;
                    };
                    self.depth += 1;
                    if self.depth > 200 {
                        self.diags.error("VL-E012", t.span,
                            "macro expansion too deep (recursive `define?)",
                        );
                        self.depth -= 1;
                        i += 1;
                        continue;
                    }
                    let args = if def.is_func {
                        match self.read_macro_args(&def, &t.text, &toks, i + 1, t.span) {
                            Some((a, next)) => {
                                i = next;
                                if a.len() != def.params.len() {
                                    self.diags.error("VL-E012", t.span,
                                        format!(
                                            "macro `{}` expects {} argument(s), got {}",
                                            t.text,
                                            def.params.len(),
                                            a.len()
                                        ),
                                    );
                                    self.depth -= 1;
                                    continue;
                                }
                                a
                            }
                            None => {
                                self.diags.error("VL-E012", t.span,
                                    format!(
                                        "macro `{}` used with too few arguments",
                                        t.text
                                    ),
                                );
                                self.depth -= 1;
                                i += 1;
                                continue;
                            }
                        }
                    } else {
                        i += 1;
                        Vec::new()
                    };
                    let expanded = self.substitute(&def, &args);
                    let mut res = self.expand_all(expanded);
                    out.append(&mut res);
                    self.depth -= 1;
                }
                Tok::MacroQuote => {
                    // `` `"token `` → stringified token; trailing `` `" ``
                    // closes and emits nothing.
                    if let Some(next) = toks.get(i + 1) {
                        out.push(Token::new(Tok::StrLit, t.span.merge(next.span), next.text.clone()));
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                Tok::MacroPaste => {
                    if let Some(last) = out.pop() {
                        if let Some(next) = toks.get(i + 1) {
                            let joined = format!("{}{}", last.text, next.text);
                            let fid = self.sm.add("<paste>".to_string(), joined.clone());
                            let mut relexed =
                                lexer::lex(&joined, fid, self.diags);
                            relexed.retain(|t| t.kind != Tok::Eof);
                            if relexed.is_empty() {
                                out.push(Token::new(Tok::Error, t.span, ""));
                            } else {
                                out.extend(relexed);
                            }
                            i += 2;
                        } else {
                            out.push(last);
                            i += 1;
                        }
                    } else {
                        i += 1;
                    }
                }
                _ => {
                    out.push(t.clone());
                    i += 1;
                }
            }
        }
        out
    }

    fn finish(mut self) -> PpOutput {
        // All per-file Eofs were dropped in run(); append exactly one.
        let eof_span = self
            .out
            .last()
            .map(|t| Span::new(t.span.file, t.span.end.0, t.span.end.0))
            .unwrap_or_default();
        self.out.push(Token::new(Tok::Eof, eof_span, String::new()));
        PpOutput {
            tokens: self.out,
            timescale: self.timescale,
            default_nettype: self.default_nettype,
        }
    }
}

fn first_word(s: &str) -> &str {
    let s = s.trim_start();
    let end = s
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$'))
        .unwrap_or(s.len());
    &s[..end]
}

fn parse_macro_params(s: &str) -> Option<(Vec<String>, &str)> {
    debug_assert!(s.starts_with('('));
    let mut params = Vec::new();
    let mut cur = String::new();
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '(' => {
                depth += 1;
                if depth == 1 {
                    continue;
                }
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    let p = cur.trim();
                    if !p.is_empty() {
                        params.push(p.to_string());
                    }
                    return Some((params, &s[i + 1..]));
                }
                cur.push(c);
            }
            ',' if depth == 1 => {
                params.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    None
}
