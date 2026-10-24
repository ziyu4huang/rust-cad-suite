//! Display format-string compilation (design §5-4).
//!
//! The lexer has already decoded `\t`/`\\`/`\n` escapes (vl-syntax lexer),
//! so this pass works on decoded text: `%%` → literal `%`, `%<flags><spec>`
//! → a [`FmtSpec`] consuming the next argument. Spec surface = the
//! 11/45-display goldens plus the §5-4 list: `%0d/%d/%05d/%h/%x/%b/%o/
//! %c/%s/%t/%%`. `%m` (hierarchy name) belongs to R4.

use crate::err::{err_at, ElabResult};

use riv_ir::FmtPiece;

pub fn compile(fmt: &str, file: &str, line: u32) -> ElabResult<Vec<FmtPiece>> {
    let mut pieces: Vec<FmtPiece> = Vec::new();
    let mut text = String::new();
    let mut argn = 0u32;
    let flush = |text: &mut String, pieces: &mut Vec<FmtPiece>| {
        if !text.is_empty() {
            pieces.push(FmtPiece::Text(std::mem::take(text)));
        }
    };
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            text.push(c);
            continue;
        }
        match chars.next() {
            Some('%') => text.push('%'),
            Some(spec) => {
                // flags: [0]<width digits>
                let mut zero_pad = false;
                let mut width = 0u32;
                let mut spec = spec;
                if spec == '0' {
                    zero_pad = true;
                    spec = chars.next().ok_or_else(|| {
                        err_at(file, line, "display format ends after `%0`")
                    })?;
                }
                let mut digits = String::new();
                if spec.is_ascii_digit() {
                    loop {
                        digits.push(spec);
                        match chars.peek() {
                            Some(d) if d.is_ascii_digit() => digits.push(chars.next().unwrap()),
                            _ => break,
                        }
                        spec = chars.next().ok_or_else(|| {
                            err_at(file, line, "display format ends inside width")
                        })?;
                    }
                    spec = chars.next().ok_or_else(|| {
                        err_at(file, line, "display format ends after width")
                    })?;
                }
                if !digits.is_empty() {
                    width = digits.parse().map_err(|_| {
                        err_at(file, line, "display format width too large")
                    })?;
                }
                let ch = match spec {
                    'd' | 'D' => 'd',
                    'h' | 'H' => 'h',
                    'x' | 'X' => 'h',
                    'b' | 'B' => 'b',
                    'o' | 'O' => 'o',
                    'c' => 'c',
                    's' | 'S' => 's',
                    't' | 'T' => 't',
                    'm' | 'M' => {
                        return Err(err_at(
                            file,
                            line,
                            "`%m` (hierarchy name) needs instance paths (milestone R4)",
                        ))
                    }
                    other => {
                        return Err(err_at(
                            file,
                            line,
                            &format!("unsupported display format `%{other}` (see docs/RIVERILOG-PLAN.md §8)"),
                        ))
                    }
                };
                flush(&mut text, &mut pieces);
                pieces.push(FmtPiece::Spec { ch, width, zero_pad, arg: argn });
                argn += 1;
            }
            None => {
                return Err(err_at(file, line, "display format ends after `%`"))
            }
        }
    }
    flush(&mut text, &mut pieces);
    Ok(pieces)
}
