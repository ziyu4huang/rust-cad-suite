//! Deterministic VCD → SVG renderer (ADR-0005).
//!
//! Output contract (pinned — a golden byte-diff enforces it):
//! - Rows follow `opts.signals` order when given (exact full-dotted-name
//!   match; duplicate names → all matching rows), else declaration order
//!   (the reader's `$var` file order). For our own dumps declaration order
//!   equals the writer's flat-name sort; only third-party VCDs can differ.
//! - Integer pixel coordinates only: `x(t) = label_w + ((t-t0)*plot_w +
//!   span/2) / span` in u128 math (no float formatting, no overflow).
//! - No timestamps, no version strings, no environment data — byte output
//!   is a pure function of (VcdFile, RenderOptions).
//! - Pre-first-change policy: if no change exists at or before the window
//!   start, the row shows an x band until the first change (honest unknown);
//!   otherwise the last value extends left to t0.
//! - Band predicate for vectors: contains `x`/`X` → red band, else contains
//!   `z`/`Z` → gray band, else bus band with a thin transition stroke at
//!   each value change. Scalars: step polyline; x/z scalar → band + rails.
//! - Values live in `<title>` tooltips, never on the canvas (no bus value
//!   text in v1) — see ADR-0005 non-goals.
//! - Real rows map parseable values onto [min,max] of the window;
//!   unparseable values break the polyline (title still carries raw text).
//! - String rows render the current value as text, hard cap 24 `char`s,
//!   no ellipsis (determinism over prettiness).
//! - Zero-change rows: full-width x band + title "no changes".
//! - Errors: `VL-E105` (a `signals` name matches no var — message lists all
//!   missing names), `VL-E106` (invalid geometry: width < 320, explicit
//!   height below one row, t0 > t1). `VcdDiag.offset` = 0 (N/A) for both.
//! - XML safety: every data-derived text (var names, string/real values,
//!   timescale) goes through [`esc`]; names never appear in attributes.
//!
//! Non-goals (v1): pan/zoom (DOM layer), scope folding, downsampling,
//! delta cycles, FST, `$timezero` application (ADR-0004 stance).

use crate::reader::{VcdDiag, VcdFile, VcdValue};
use std::fmt::Write as _;

const GRID: &str = "#233442";
const TRACE: &str = "#4ec9b0";
const X_FILL: &str = "#b55757";
const Z_FILL: &str = "#66707a";
const BUS_BAND: &str = "#16303c";
const NAME_TEXT: &str = "#9cf3ff";
const BG: &str = "#101418";
const AXIS_TEXT: &str = "#8899aa";
const FONT: &str = "ui-monospace,SFMono-Regular,Menlo,monospace";

/// Render knobs. `Default` mirrors the demo's v2 look (960 wide, 28px rows).
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Canvas width in px (min 320).
    pub width: u32,
    /// Canvas height; `None` = auto (axis + rows).
    pub height: Option<u32>,
    /// Visible window start (default 0).
    pub t0: Option<u64>,
    /// Visible window end (default max(t_max, 1)). Window is used as given —
    /// space beyond t_max renders as empty grid (no silent clamping).
    pub t1: Option<u64>,
    /// Row subset + order (exact full-dotted-name match; duplicates → all
    /// matching rows). `None` = declaration order, all vars.
    pub signals: Option<Vec<String>>,
    pub row_height: u32,
    pub label_width: u32,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            width: 960,
            height: None,
            t0: None,
            t1: None,
            signals: None,
            row_height: 28,
            label_width: 120,
        }
    }
}

const AXIS_H: u32 = 18;
const MIN_WIDTH: u32 = 320;

/// Escape data-derived text for XML text nodes and titles: `&` first, then
/// `< > " '`; XML-1.0-illegal control characters are dropped (tab/newline/CR
/// kept). Data never lands in attributes, so quote escaping is belt-and-braces.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 => {} // illegal XML 1.0 control char
            c => out.push(c),
        }
    }
    out
}

/// Band fill for a raw vector text: contains x → red, else contains z →
/// gray, else bus band.
fn band_fill(raw: &str) -> &'static str {
    if raw.contains('x') || raw.contains('X') {
        X_FILL
    } else if raw.contains('z') || raw.contains('Z') {
        Z_FILL
    } else {
        BUS_BAND
    }
}

fn value_raw(v: &VcdValue) -> String {
    v.to_raw()
}

struct Seg<'a> {
    x0: i64,
    x1: i64,
    value: &'a VcdValue,
    t0: u64,
    t1: u64,
}

/// Collect window segments for one var: left-context value (last change at
/// or before `t0`, else x) through changes within the window.
fn segments<'a>(
    f: &'a VcdFile,
    var_idx: usize,
    t0: u64,
    t1: u64,
    x: &dyn Fn(u64) -> i64,
) -> Vec<Seg<'a>> {
    let mut out = Vec::new();
    let changes: Vec<&crate::reader::VcdChange> =
        f.changes.iter().filter(|c| c.var == var_idx).collect();
    // Left context.
    let mut cur: Option<&crate::reader::VcdChange> = None;
    for c in &changes {
        if c.time <= t0 {
            cur = Some(c);
        } else {
            break;
        }
    }
    let mut seg_start_t = t0;
    let mut seg_value: Option<&VcdValue> = cur.map(|c| &c.value);
    for c in &changes {
        let t = c.time;
        if t <= t0 {
            continue;
        }
        if t > t1 {
            break;
        }
        match seg_value.take() {
            Some(v) => out.push(Seg {
                x0: x(seg_start_t),
                x1: x(t),
                value: v,
                t0: seg_start_t,
                t1: t,
            }),
            None => {} // pre-first-change x stretch emits nothing; band drawn by caller
        }
        seg_start_t = t;
        seg_value = Some(&c.value);
    }
    match seg_value {
        Some(v) => out.push(Seg {
            x0: x(seg_start_t),
            x1: x(t1),
            value: v,
            t0: seg_start_t,
            t1,
        }),
        None => {}
    }
    out
}

/// Render `f` to a deterministic SVG document.
pub fn render(f: &VcdFile, opts: &RenderOptions) -> Result<String, VcdDiag> {
    if opts.width < MIN_WIDTH {
        return Err(VcdDiag {
            code: "VL-E106",
            message: format!("width {} below minimum {MIN_WIDTH}", opts.width),
            offset: 0,
        });
    }
    if opts.row_height < 12 {
        return Err(VcdDiag {
            code: "VL-E106",
            message: format!("row_height {} below minimum 12", opts.row_height),
            offset: 0,
        });
    }
    let t0 = opts.t0.unwrap_or(0);
    let t1 = opts.t1.unwrap_or_else(|| f.t_max().max(1));
    if t0 > t1 {
        return Err(VcdDiag {
            code: "VL-E106",
            message: format!("window t0 {t0} > t1 {t1}"),
            offset: 0,
        });
    }
    // Row selection (exact full-name match; duplicates → all matching rows).
    let rows: Vec<usize> = match &opts.signals {
        None => (0..f.vars.len()).collect(),
        Some(names) => {
            let mut missing: Vec<&String> = names.iter().collect();
            let mut idxs = Vec::new();
            for n in names {
                let mut hit = false;
                for (i, v) in f.vars.iter().enumerate() {
                    if &v.name == n {
                        idxs.push(i);
                        hit = true;
                    }
                }
                if hit {
                    missing.retain(|m| *m != n);
                }
            }
            if !missing.is_empty() {
                let list: Vec<String> = missing.iter().map(|s| s.to_string()).collect();
                return Err(VcdDiag {
                    code: "VL-E105",
                    message: format!("signals not in dump: {}", list.join(", ")),
                    offset: 0,
                });
            }
            idxs
        }
    };

    let width = opts.width;
    let label_w = opts.label_width.min(width.saturating_sub(80)) as u64;
    let plot_w = (width as u64 - label_w).max(1);
    let span = (t1 - t0).max(1) as u128;
    let x = |t: u64| -> i64 {
        let d = (t.max(t0) - t0) as u128;
        label_w as i64 + ((d * plot_w as u128 + span / 2) / span) as i64
    };
    let rows_area = (rows.len() as u32) * opts.row_height;
    let auto_h = AXIS_H + rows_area;
    let required = AXIS_H + opts.row_height; // axis + at least one row
    let height = match opts.height {
        None => auto_h,
        Some(h) if h >= required.max(auto_h) => h,
        Some(h) => {
            return Err(VcdDiag {
                code: "VL-E106",
                message: format!("height {h} below required {}", required.max(auto_h)),
                offset: 0,
            })
        }
    };

    let mut s = String::with_capacity(16 * 1024);
    writeln!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width} {height}\" width=\"{width}\" height=\"{height}\" font-family=\"{FONT}\">"
    )
    .unwrap();
    writeln!(s, "<rect width=\"{width}\" height=\"{height}\" fill=\"{BG}\"/>").unwrap();
    // Grid: 8 vertical divisions + time axis (labels every other division).
    for g in 0..=8u32 {
        let gx = label_w as u64 + g as u64 * plot_w / 8;
        writeln!(
            s,
            "<line x1=\"{gx}\" y1=\"2\" x2=\"{gx}\" y2=\"{}\" stroke=\"{GRID}\" stroke-width=\"1\"/>",
            height - AXIS_H
        )
        .unwrap();
        if g % 2 == 0 {
            let t = t0 + (span as u64) * g as u64 / 8;
            writeln!(
                s,
                "<text x=\"{}\" y=\"{height}\" font-size=\"9\" fill=\"{AXIS_TEXT}\" text-anchor=\"middle\">{t}</text>",
                gx
            )
            .unwrap();
        }
    }
    if let Some(ts) = &f.timescale {
        let _ = write!(
            s,
            "<text x=\"{width}\" y=\"10\" font-size=\"9\" fill=\"{AXIS_TEXT}\" text-anchor=\"end\">"
        );
        let _ = write!(s, "timescale {}", esc(ts));
        s.push_str("</text>\n");
    }

    for (ri, &vi) in rows.iter().enumerate() {
        let var = &f.vars[vi];
        let y0 = (AXIS_H + ri as u32 * opts.row_height + 8) as i64; // 16px band inside the row
        let _ = write!(
            s,
            "<g><title>{} w={} type={}</title>",
            esc(&var.name),
            var.width,
            esc(&var.var_type)
        );
        writeln!(
            s,
            "<text x=\"4\" y=\"{}\" font-size=\"11\" fill=\"{NAME_TEXT}\">{}</text>",
            y0 + 8,
            esc(&var.name)
        )
        .unwrap();
        let segs = segments(f, vi, t0, t1, &x);
        if segs.is_empty() {
            // No changes in window (incl. zero-change vars): honest x band.
            writeln!(
                s,
                "<rect x=\"{label_w}\" y=\"{y0}\" width=\"{plot_w}\" height=\"16\" fill=\"{Z_FILL}\" opacity=\"0.5\"/><title>no changes</title>"
            )
            .unwrap();
            s.push_str("</g>\n");
            continue;
        }
        match &segs[0].value {
            VcdValue::Scalar(_) => render_scalar(&mut s, &segs, y0),
            VcdValue::Vector(_) => render_vector(&mut s, &segs, y0, var.width),
            VcdValue::Real(_) => render_real(&mut s, &segs, y0),
            VcdValue::Str(_) => render_str(&mut s, &segs, y0),
        }
        s.push_str("</g>\n");
    }
    s.push_str("</svg>\n");
    Ok(s)
}

fn title_seg(s: &mut String, seg: &Seg) {
    let _ = write!(
        s,
        "<title>t={}..{}: {}</title>",
        seg.t0,
        seg.t1,
        esc(&value_raw(seg.value))
    );
}

/// Scalar row: one step polyline (0/1), x/z scalar = band + rails.
fn render_scalar(s: &mut String, segs: &[Seg], y0: i64) {
    // Split: x/z stretches as bands, 0/1 runs as one polyline.
    let mut poly = String::new();
    for seg in segs {
        title_seg(s, seg);
        let c = match seg.value {
            VcdValue::Scalar(c) => *c,
            _ => continue,
        };
        match c {
            '0' | '1' => {
                let y = y0 + if c == '1' { 0 } else { 16 };
                let _ = write!(poly, "{} {} ", seg.x0, y);
                let _ = write!(poly, "{} {} ", seg.x1, y);
            }
            other => {
                let fill = if other == 'z' || other == 'Z' { Z_FILL } else { X_FILL };
                let _ = write!(
                    s,
                    "<rect x=\"{}\" y=\"{y0}\" width=\"{}\" height=\"16\" fill=\"{fill}\"/>",
                    seg.x0.min(seg.x1),
                    (seg.x1 - seg.x0).abs().max(1)
                );
                // Dual rails mark the invalid stretch.
                let _ = write!(
                    s,
                    "<line x1=\"{}\" y1=\"{y0}\" x2=\"{}\" y2=\"{y0}\" stroke=\"{TRACE}\"/><line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{TRACE}\"/>",
                    seg.x0, seg.x1, seg.x0, y0 + 16, seg.x1, y0 + 16
                );
            }
        }
    }
    if poly.contains(' ') {
        let _ = write!(
            s,
            "<polyline points=\"{}\" fill=\"none\" stroke=\"{TRACE}\" stroke-width=\"2\"/>",
            poly.trim_end()
        );
    }
}

/// Vector row: band per segment (x/z predicate) + transition strokes between
/// consecutive segments; values only in <title>.
fn render_vector(s: &mut String, segs: &[Seg], y0: i64, width: u32) {
    let _ = width;
    for (i, seg) in segs.iter().enumerate() {
        title_seg(s, seg);
        let raw = value_raw(seg.value);
        let fill = band_fill(&raw);
        let _ = write!(
            s,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"8\" fill=\"{fill}\"/>",
            seg.x0.min(seg.x1),
            y0 + 4,
            (seg.x1 - seg.x0).abs().max(1)
        );
        if i > 0 {
            let px = seg.x0;
            let _ = write!(
                s,
                "<line x1=\"{px}\" y1=\"{}\" x2=\"{px}\" y2=\"{}\" stroke=\"{TRACE}\"/>",
                y0 + 2,
                y0 + 14
            );
        }
    }
}

/// Real row: step polyline over parseable values mapped to [min,max] of the
/// window; unparseable values break the polyline (title carries raw text).
fn render_real(s: &mut String, segs: &[Seg], y0: i64) {
    let vals: Vec<Option<f64>> = segs
        .iter()
        .map(|seg| match seg.value {
            VcdValue::Real(r) => r.parse::<f64>().ok().and_then(|v| {
                if v.is_finite() {
                    Some(v)
                } else {
                    None
                }
            }),
            _ => None,
        })
        .collect();
    let finite: Vec<f64> = vals.iter().filter_map(|v| *v).collect();
    let (min, max) = if finite.is_empty() {
        (0.0, 1.0)
    } else {
        (
            finite.iter().cloned().fold(f64::INFINITY, f64::min),
            finite.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        )
    };
    let y_of = |v: f64| -> i64 {
        if min == max {
            y0 + 8
        } else {
            y0 + 16 - ((v - min) / (max - min) * 16.0).round() as i64
        }
    };
    let mut poly = String::new();
    for (i, seg) in segs.iter().enumerate() {
        title_seg(s, seg);
        match vals[i] {
            Some(v) => {
                let y = y_of(v);
                let _ = write!(poly, "{} {y} ", seg.x0);
                let _ = write!(poly, "{} {y} ", seg.x1);
            }
            None => {
                // Break the polyline on unparseable values.
                if poly.contains(' ') {
                    let _ = write!(
                        s,
                        "<polyline points=\"{}\" fill=\"none\" stroke=\"{TRACE}\" stroke-width=\"2\"/>",
                        poly.trim_end()
                    );
                }
                poly.clear();
            }
        }
    }
    if poly.contains(' ') {
        let _ = write!(
            s,
            "<polyline points=\"{}\" fill=\"none\" stroke=\"{TRACE}\" stroke-width=\"2\"/>",
            poly.trim_end()
        );
    }
}

/// String row: current value as text, hard cap 24 chars, per-segment title.
fn render_str(s: &mut String, segs: &[Seg], y0: i64) {
    for seg in segs {
        title_seg(s, seg);
        let raw = value_raw(seg.value);
        let capped: String = raw.chars().take(24).collect();
        let _ = write!(
            s,
            "<text x=\"{}\" y=\"{}\" font-size=\"10\" fill=\"{TRACE}\">{}</text>",
            seg.x0 + 2,
            y0 + 12,
            esc(&capped)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::parse;

    const HOSTILE: &str = "\
$scope module tb $end
$var wire 4 ! <img src=x onerror=alert(1)> $end
$var string 64 $ s $end
$upscope $end
$enddefinitions $end
#0
b0101 !
s<script>alert(1)</script> $
#10
bXX01 !
";

    #[test]
    fn hostile_data_is_escaped() {
        let f = parse(HOSTILE).expect("hostile fixture parses");
        let svg = render(&f, &RenderOptions::default()).expect("renders");
        assert!(!svg.contains("<script>"), "raw script tag leaked");
        assert!(!svg.contains("<img"), "raw img tag leaked");
        assert!(svg.contains("&lt;script&gt;"), "escaped string value missing");
        assert!(svg.contains("&lt;img"), "escaped name missing");
        // Data-derived names must never appear inside an attribute.
        for line in svg.lines() {
            for attr_start in ["id=\"", "class=\"", "data-name=\""] {
                assert!(!line.contains(attr_start), "attribute from data: {line}");
            }
        }
    }

    #[test]
    fn render_is_deterministic() {
        let f = parse(HOSTILE).expect("parses");
        let a = render(&f, &RenderOptions::default()).expect("ok");
        let b = render(&f, &RenderOptions::default()).expect("ok");
        assert_eq!(a, b);
    }

    #[test]
    fn errors_e105_e106() {
        let f = parse(HOSTILE).expect("parses");
        let e = render(
            &f,
            &RenderOptions {
                signals: Some(vec!["nope.sig".into()]),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(e.code, "VL-E105");
        assert_eq!(e.offset, 0);
        let e = render(
            &f,
            &RenderOptions {
                width: 200,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(e.code, "VL-E106");
        let e = render(
            &f,
            &RenderOptions {
                t0: Some(100),
                t1: Some(5),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(e.code, "VL-E106");
    }

    #[test]
    fn shapes_scalar_vector_real_str() {
        let src = "\
$scope module tb $end
$var wire 1 ! c $end
$var wire 4 \" q [3:0] $end
$var real 64 # r $end
$var string 64 $ m $end
$upscope $end
$enddefinitions $end
#0
0!
b0101 !
r1.5 #
shello $
#10
1!
b1100 !
r2.5 #
#20
bzz11 !
";
        let f = parse(src).expect("parses");
        let svg = render(&f, &RenderOptions::default()).expect("ok");
        // Scalar: one polyline with a step at t=10.
        assert!(svg.contains("<polyline"), "scalar polyline missing");
        // Vector: bus band + transition strokes + x-band for bzz11.
        assert!(svg.contains(BUS_BAND), "bus band missing");
        assert!(svg.contains(Z_FILL), "z band missing for bzz11");
        // Real: mapped polyline.
        assert!(svg.contains("<title>t=0..10: 1.5</title>"), "real title missing");
        // String: text row, capped.
        assert!(svg.contains("hello"), "string text missing");
        // Titles on rows carry name/width/type.
        assert!(svg.contains("<title>tb.q w=4 type=wire</title>"));
    }

    #[test]
    fn zero_change_row_is_x_band() {
        let src = "$scope module t $end\n$var wire 4 ! a $end\n$enddefinitions $end\n#0\nb0101 !\n";
        let f = parse(src).expect("parses");
        // Row 2 vars: one changes, one not.
        let src2 = "$scope module t $end\n$var wire 4 ! a $end\n$var wire 4 \" b $end\n$enddefinitions $end\n#0\nb0101 !\n";
        let f2 = parse(src2).expect("parses");
        let svg = render(&f2, &RenderOptions::default()).expect("ok");
        assert!(svg.contains("no changes"), "zero-change row marker missing");
        let _ = f;
    }
}
