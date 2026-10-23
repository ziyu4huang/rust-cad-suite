//! Tolerant VCD reader.
//!
//! Error model: hard errors abort the parse (`VL-E1xx`), tolerated
//! conditions accumulate in [`VcdFile::warnings`] (`VL-W1xx`, capped at
//! 128 with a truncation note). Byte `offset` points at the offending
//! line. Codes:
//!
//! | code | meaning |
//! |---|---|
//! | VL-E100 | malformed value change |
//! | VL-E101 | malformed `#time` |
//! | VL-E102 | malformed `$var` |
//! | VL-E103 | malformed `$scope` |
//! | VL-E104 | `$dumpports` (EVCD body) — explicitly unsupported |
//! | VL-W100 | change for unknown identifier |
//! | VL-W101 | truncated EOF (section or token cut short) |
//! | VL-W102 | EVCD strength scalar (`h/u/w/l/-`) |
//! | VL-W103 | cross-form coercion (`b` on real/string var, `r`/`s` on bits var) |
//!
//! Tolerances adopted from GTKWave (see docs/GTKWAVE-VCD-STUDY.md): unknown
//! header sections skipped; `$timezero` recorded but NOT applied; `$vcdclose`
//! skipped; any `$scope` type accepted; duplicate ids become aliases; CRLF;
//! optional `[msb:lsb]` suffix after `$var` names; uppercase scalar/vector
//! forms. Rejected for v1: gzip input, `$dumpports` bodies, `$timezero`
//! offsetting, EVCD strength semantics, FST.

use std::collections::BTreeMap;

const WARN_CAP: usize = 128;

#[derive(Debug, Clone, PartialEq)]
pub enum VcdValue {
    /// Single character (`0/1/x/z`, or EVCD strength `h/u/w/l/-`).
    Scalar(char),
    /// 4-state bit vector, raw text MSB-first (width as declared).
    Vector(String),
    /// Real value, raw text.
    Real(String),
    /// String value.
    Str(String),
}

impl VcdValue {
    /// Raw interchange text (what a consumer normalizes with
    /// [`normalize_width`] for bit vectors).
    pub fn to_raw(&self) -> String {
        match self {
            VcdValue::Scalar(c) => c.to_string(),
            VcdValue::Vector(s) | VcdValue::Real(s) | VcdValue::Str(s) => s.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VcdVar {
    /// Identifier code as written in the file.
    pub id: String,
    /// Full dotted hierarchical name (scope path + leaf).
    pub name: String,
    /// Leaf name as written.
    pub leaf: String,
    /// Declared width (bits).
    pub width: u32,
    /// `$var` type token (`wire`, `reg`, `real`, `string`, ...).
    pub var_type: String,
    /// Parsed `[msb:lsb]` suffix, when present.
    pub index: Option<(i64, i64)>,
}

#[derive(Debug, Clone)]
pub struct VcdChange {
    pub time: u64,
    /// Index into [`VcdFile::vars`]. Aliases (same id, several `$var`
    /// entries) receive one change per entry, so every named view of the
    /// net sees the value.
    pub var: usize,
    pub value: VcdValue,
}

#[derive(Debug, Clone)]
pub struct VcdDiag {
    pub code: &'static str,
    pub message: String,
    /// Byte offset of the offending line.
    pub offset: usize,
}

#[derive(Debug, Default)]
pub struct VcdFile {
    pub timescale: Option<String>,
    /// `$timezero` value — recorded, never applied (ADR-0004).
    pub timezero: Option<i64>,
    pub vars: Vec<VcdVar>,
    pub changes: Vec<VcdChange>,
    pub warnings: Vec<VcdDiag>,
}

impl VcdFile {
    /// Largest change time seen (0 when there are no changes).
    pub fn t_max(&self) -> u64 {
        self.changes.iter().map(|c| c.time).max().unwrap_or(0)
    }
}

/// Left-extend (or left-truncate) a VCD vector to `w` bits. x/z extend
/// with themselves, 0 otherwise. Moved verbatim from vl-difftest's
/// `norm_val` when the reader was centralized — behavior is the oracle
/// crosscheck's contract, do not "fix" it.
pub fn normalize_width(v: &str, w: u32) -> String {
    let w = w as usize;
    if v.len() >= w {
        return v[v.len() - w..].to_string();
    }
    let fill = match v.chars().next() {
        Some('x') => 'x',
        Some('z') => 'z',
        _ => '0',
    };
    let mut s: String = std::iter::repeat(fill).take(w - v.len()).collect();
    s.push_str(v);
    s
}

pub fn parse(text: &str) -> Result<VcdFile, VcdDiag> {
    let mut f = VcdFile::default();
    let mut id_vars: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut scope: Vec<String> = Vec::new();
    let mut in_defs = true;
    let mut now: u64 = 0;
    let mut byte_off: usize = 0;

    for raw in text.lines() {
        let line_start = byte_off;
        byte_off += raw.len() + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if in_defs {
            match line.split_whitespace().next().unwrap_or("") {
                "$scope" => {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() < 3 {
                        return Err(VcdDiag {
                            code: "VL-E103",
                            message: format!("malformed $scope: `{line}`"),
                            offset: line_start,
                        });
                    }
                    // Scope type token (module/task/function/...) is accepted
                    // but not interpreted — GTKWave maps ~17 kinds; we keep
                    // the tree shape only.
                    scope.push(parts[2].to_string());
                }
                "$upscope" => {
                    scope.pop();
                }
                "$var" => {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() < 5 {
                        return Err(VcdDiag {
                            code: "VL-E102",
                            message: format!("malformed $var: `{line}`"),
                            offset: line_start,
                        });
                    }
                    let Ok(width) = parts[2].parse::<u32>() else {
                        return Err(VcdDiag {
                            code: "VL-E102",
                            message: format!("malformed $var width `{}`", parts[2]),
                            offset: line_start,
                        });
                    };
                    let id = parts[3].to_string();
                    let leaf = parts[4].to_string();
                    let mut index = None;
                    if let Some(range) = parts.get(5) {
                        if range.starts_with('[') && !range.starts_with("$") {
                            index = parse_range(range);
                        }
                    }
                    let mut name = scope.clone();
                    name.push(leaf.clone());
                    let var = VcdVar {
                        id: id.clone(),
                        name: name.join("."),
                        leaf,
                        width,
                        var_type: parts[1].to_string(),
                        index,
                    };
                    let idx = f.vars.len();
                    // Aliases (same id, several $var) share the change stream.
                    id_vars.entry(id).or_default().push(idx);
                    f.vars.push(var);
                }
                "$enddefinitions" => in_defs = false,
                "$timescale" => {
                    let ts: Vec<&str> = line
                        .split_whitespace()
                        .skip(1)
                        .take_while(|t| *t != "$end")
                        .collect();
                    if !ts.is_empty() {
                        f.timescale = Some(ts.join(" "));
                    }
                }
                "$timezero" => {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(tz) = parts[1].parse::<i64>() {
                            f.timezero = Some(tz);
                        }
                    }
                }
                "$date" | "$version" | "$comment" | "$vcdclose" | "$dumpfile" | "$dumpvars"
                | "$dumpall" | "$dumpoff" | "$dumpon" => {
                    // Skip the section body (to `$end`) unless it terminates
                    // on the same line.
                    if !line.ends_with("$end") {
                        skip_to_end(text, &mut byte_off);
                    }
                }
                _ => { /* unknown header section: tolerated, skipped */ }
            }
            continue;
        }

        // ---- change section ----
        if let Some(t) = line.strip_prefix('#') {
            match t.trim().parse::<u64>() {
                Ok(v) => now = v,
                Err(_) => {
                    return Err(VcdDiag {
                        code: "VL-E101",
                        message: format!("malformed #time: `{t}`"),
                        offset: line_start,
                    })
                }
            }
        } else if line.starts_with('$') {
            let head = line.split_whitespace().next().unwrap_or("");
            match head {
                "$dumpports" | "$dumpportsoff" | "$dumpportson" | "$dumpportsall" => {
                    return Err(VcdDiag {
                        code: "VL-E104",
                        message: format!("{head} (EVCD) is not supported"),
                        offset: line_start,
                    })
                }
                "$comment" => {
                    if !line.ends_with("$end") {
                        skip_to_end(text, &mut byte_off);
                    }
                }
                _ => { /* $end / $dumpvars / $dumpall / ... — no-op */ }
            }
        } else {
            let first = line.as_bytes()[0];
            let is_strength = matches!(first, b'h' | b'u' | b'w' | b'l' | b'-');
            match first {
                b'b' | b'B' | b'r' | b'R' => {
                    let form = first as char;
                    let rest = &line[1..];
                    let mut it = rest.split_whitespace();
                    let (Some(v), Some(id)) = (it.next(), it.next()) else {
                        return Err(VcdDiag {
                            code: "VL-E100",
                            message: format!("malformed value change: `{line}`"),
                            offset: line_start,
                        });
                    };
                    if it.next().is_some() {
                        return Err(VcdDiag {
                            code: "VL-E100",
                            message: format!("malformed value change: `{line}`"),
                            offset: line_start,
                        });
                    }
                    let (value, warn) = match form {
                        'b' | 'B' => {
                            if !v.chars().all(|c| matches!(c, '0'..='9' | 'x' | 'X' | 'z' | 'Z')) {
                                return Err(VcdDiag {
                                    code: "VL-E100",
                                    message: format!("bad vector digits `{v}`"),
                                    offset: line_start,
                                });
                            }
                            coerce_vector(&f, v.to_string(), id, &id_vars)
                        }
                        _ => coerce_real(&f, v.to_string(), id, &id_vars),
                    };
                    if let Some(w) = warn {
                        note_warn(
                            &mut f.warnings,
                            VcdDiag {
                                code: "VL-W103",
                                message: w,
                                offset: line_start,
                            },
                        );
                    }
                    push_changes(&mut f, &mut id_vars, id, now, value, line_start);
                }
                b's' | b'S' => {
                    let rest = &line[1..];
                    let Some((v, id)) = rest.rsplit_once(' ') else {
                        return Err(VcdDiag {
                            code: "VL-E100",
                            message: format!("malformed string change: `{line}`"),
                            offset: line_start,
                        });
                    };
                    push_changes(&mut f, &mut id_vars, id, now, VcdValue::Str(v.to_string()), line_start);
                }
                b'0' | b'1' | b'x' | b'X' | b'z' | b'Z' => {
                    let id = &line[1..];
                    if id.is_empty() {
                        return Err(VcdDiag {
                            code: "VL-E100",
                            message: format!("malformed scalar change: `{line}`"),
                            offset: line_start,
                        });
                    }
                    let c = line.chars().next().unwrap();
                    push_changes(
                        &mut f,
                        &mut id_vars,
                        id,
                        now,
                        VcdValue::Scalar(c),
                        line_start,
                    );
                }
                _ if is_strength => {
                    note_warn(
                        &mut f.warnings,
                        VcdDiag {
                            code: "VL-W102",
                            message: format!("EVCD strength scalar `{line}`"),
                            offset: line_start,
                        },
                    );
                    let id = &line[1..];
                    if id.is_empty() {
                        return Err(VcdDiag {
                            code: "VL-E100",
                            message: format!("malformed scalar change: `{line}`"),
                            offset: line_start,
                        });
                    }
                    let c = line.chars().next().unwrap();
                    push_changes(
                        &mut f,
                        &mut id_vars,
                        id,
                        now,
                        VcdValue::Scalar(c),
                        line_start,
                    );
                }
                _ => {
                    return Err(VcdDiag {
                        code: "VL-E100",
                        message: format!("malformed value change: `{line}`"),
                        offset: line_start,
                    })
                }
            }
        }
    }
    if in_defs {
        note_warn(
            &mut f.warnings,
            VcdDiag {
                code: "VL-W101",
                message: "truncated: EOF before $enddefinitions".to_string(),
                offset: text.len().saturating_sub(1),
            },
        );
    }
    Ok(f)
}

/// b/B form: bits vars keep the raw vector; real/string vars coerce to a
/// string value with a VL-W103 warning (GTKWave recoder does the inverse
/// normalization). Returns (value, optional warning message).
fn coerce_vector(
    f: &VcdFile,
    v: String,
    id: &str,
    id_vars: &BTreeMap<String, Vec<usize>>,
) -> (VcdValue, Option<String>) {
    let ty = var_type_of(f, id, id_vars);
    if ty == "real" || ty == "string" || ty == "real_parameter" {
        return (
            VcdValue::Str(v),
            Some(format!("b-form on {ty} var `{id}` — kept as string")),
        );
    }
    (VcdValue::Vector(v), None)
}

/// r/R form: real vars keep raw text; bits vars coerce to vector text.
fn coerce_real(
    f: &VcdFile,
    v: String,
    id: &str,
    id_vars: &BTreeMap<String, Vec<usize>>,
) -> (VcdValue, Option<String>) {
    let ty = var_type_of(f, id, id_vars);
    match ty {
        "real" | "real_parameter" => (VcdValue::Real(v), None),
        "string" => (
            VcdValue::Str(v),
            Some(format!("r-form on {ty} var `{id}` — kept as string")),
        ),
        _ => (
            VcdValue::Vector(v),
            Some(format!("r-form on bits var `{id}` — kept as vector text")),
        ),
    }
}

fn var_type_of(
    f: &VcdFile,
    id: &str,
    id_vars: &BTreeMap<String, Vec<usize>>,
) -> &'static str {
    id_vars
        .get(id)
        .and_then(|idxs| idxs.first())
        .and_then(|i| f.vars.get(*i))
        .map(|v| match v.var_type.as_str() {
            "real" => "real",
            "real_parameter" => "real_parameter",
            "string" => "string",
            _ => "bits",
        })
        .unwrap_or("bits")
}

fn push_changes(
    f: &mut VcdFile,
    id_vars: &mut BTreeMap<String, Vec<usize>>,
    id: &str,
    now: u64,
    value: VcdValue,
    offset: usize,
) {
    let Some(idxs) = id_vars.get(id).cloned() else {
        note_warn(
            &mut f.warnings,
            VcdDiag {
                code: "VL-W100",
                message: format!("change for unknown identifier `{id}`"),
                offset,
            },
        );
        return;
    };
    for i in idxs {
        f.changes.push(VcdChange {
            time: now,
            var: i,
            value: value.clone(),
        });
    }
}

fn note_warn(warnings: &mut Vec<VcdDiag>, d: VcdDiag) {
    if warnings.len() >= WARN_CAP {
        if warnings.len() == WARN_CAP {
            warnings.push(VcdDiag {
                code: "VL-W101",
                message: "warning cap reached; further warnings suppressed".to_string(),
                offset: d.offset,
            });
        }
        return;
    }
    warnings.push(d);
}

/// `[msb:lsb]` / `[i]` suffix on a `$var` name.
fn parse_range(tok: &str) -> Option<(i64, i64)> {
    let inner = tok.strip_prefix('[')?.strip_suffix(']')?;
    if let Some((a, b)) = inner.split_once(':') {
        Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
    } else {
        let i: i64 = inner.trim().parse().ok()?;
        Some((i, i))
    }
}

/// Consume input until the `$end` terminator line (used for multi-line
/// `$date`/`$version`/`$comment`/... sections). EOF simply stops.
fn skip_to_end(text: &str, byte_off: &mut usize) {
    while *byte_off < text.len() {
        let rest = &text[*byte_off..];
        let Some(nl) = rest.find('\n') else {
            let line = rest.trim();
            *byte_off = text.len();
            if line == "$end" {
                return;
            }
            break;
        };
        let line = rest[..nl].trim();
        *byte_off += nl + 1;
        if line == "$end" {
            return;
        }
    }
}
