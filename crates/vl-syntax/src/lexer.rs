//! Lexer: source text → raw tokens (including `` ` `` directives and macro
//! uses). Hand-written per PLAN §1.3: Verilog lexing is context-sensitive
//! (sized `'h` literals with x/z digits, directives sharing the `` ` `` sigil).

use crate::token::{DirKind, Kw, Punct, Tok, Token};
use rcs_diag::{DiagSink, FileId, Span};

#[derive(Clone, Copy)]
pub struct LexOptions {
    /// Byte offset of `src` within its logical file. Macro-body re-lexing
    /// re-uses the original file's coordinates so spans stay absolute.
    pub base: u32,
}

impl Default for LexOptions {
    fn default() -> Self {
        LexOptions { base: 0 }
    }
}

pub fn lex(src: &str, file: FileId, diags: &mut DiagSink) -> Vec<Token> {
    lex_with(src, file, diags, LexOptions::default())
}

pub fn lex_with(src: &str, file: FileId, diags: &mut DiagSink, opts: LexOptions) -> Vec<Token> {
    let mut lx = Lexer {
        src,
        pos: 0,
        file,
        diags,
        base: opts.base,
    };
    let mut out = Vec::new();
    loop {
        let t = lx.next_token();
        let eof = t.kind == Tok::Eof;
        out.push(t);
        if eof {
            break;
        }
    }
    out
}

struct Lexer<'a, 'd> {
    src: &'a str,
    pos: usize,
    file: FileId,
    diags: &'d mut DiagSink,
    base: u32,
}

impl<'a, 'd> Lexer<'a, 'd> {
    fn eof(&self) -> bool {
        self.pos >= self.src.len()
    }

    fn cur(&self) -> u8 {
        self.src.as_bytes()[self.pos]
    }

    fn peek(&self, n: usize) -> Option<u8> {
        self.src.as_bytes().get(self.pos + n).copied()
    }

    fn bump(&mut self) -> u8 {
        let b = self.cur();
        self.pos += 1;
        b
    }

    fn span_from(&self, start: usize) -> Span {
        Span::new(self.file, (start + self.base as usize) as u32, (self.pos + self.base as usize) as u32)
    }

    fn err(&mut self, start: usize, code: &'static str, msg: impl Into<String>) {
        let span = Span::new(
            self.file,
            (start + self.base as usize) as u32,
            (self.pos + self.base as usize) as u32,
        );
        self.diags.error(code, span, msg);
    }

    fn skip_trivia(&mut self) {
        loop {
            while !self.eof() && (self.cur() as char).is_whitespace() {
                self.pos += 1;
            }
            if self.eof() {
                return;
            }
            if self.cur() == b'/' && self.peek(1) == Some(b'/') {
                while !self.eof() && self.cur() != b'\n' {
                    self.pos += 1;
                }
                continue;
            }
            if self.cur() == b'/' && self.peek(1) == Some(b'*') {
                let start = self.pos;
                self.pos += 2;
                loop {
                    if self.eof() {
                        self.err(start, "VL-E003", "unterminated block comment");
                        return;
                    }
                    if self.cur() == b'*' && self.peek(1) == Some(b'/') {
                        self.pos += 2;
                        break;
                    }
                    self.pos += 1;
                }
                continue;
            }
            return;
        }
    }

    fn next_token(&mut self) -> Token {
        self.skip_trivia();
        let start = self.pos;
        if self.eof() {
            return Token::new(Tok::Eof, self.span_from(start), "");
        }
        let c = self.cur();
        let tok = match c {
            b'"' => self.lex_string(start),
            b'\'' => self.lex_based(start, None),
            b'$' => {
                self.bump();
                let name = self.take_ident_body();
                if name.is_empty() {
                    self.err(start, "VL-E004", "expected system identifier after `$`");
                }
                Token::new(Tok::SysIdent, self.span_from(start), format!("${name}"))
            }
            b'\\' => {
                self.bump();
                let name = self.take_until_ws();
                if name.is_empty() {
                    self.err(start, "VL-E004", "empty escaped identifier");
                }
                Token::new(Tok::Ident, self.span_from(start), name)
            }
            b'`' => self.lex_backtick(start),
            b'0'..=b'9' => self.lex_number(start),
            _ if is_ident_start(c) => {
                let name = self.take_ident_body();
                match Kw::from_str(&name) {
                    Some(kw) => Token::new(Tok::Keyword(kw), self.span_from(start), name),
                    None => Token::new(Tok::Ident, self.span_from(start), name),
                }
            }
            _ => self.lex_punct(start),
        };
        tok
    }

    fn take_ident_body(&mut self) -> String {
        let start = self.pos;
        while !self.eof() && is_ident_cont(self.cur()) {
            self.pos += 1;
        }
        self.src[start..self.pos].to_string()
    }

    fn take_until_ws(&mut self) -> String {
        let start = self.pos;
        while !self.eof() {
            let c = self.cur();
            if c == b' ' || c == b'\t' || c == b'\n' || c == b'\r' {
                break;
            }
            self.pos += 1;
        }
        self.src[start..self.pos].to_string()
    }

    fn lex_string(&mut self, start: usize) -> Token {
        self.bump(); // opening quote
        let mut val = String::new();
        loop {
            if self.eof() || self.cur() == b'\n' {
                self.err(start, "VL-E002", "unterminated string literal");
                break;
            }
            match self.bump() {
                b'"' => break,
                b'\\' => {
                    if self.eof() {
                        self.err(start, "VL-E002", "unterminated string literal");
                        break;
                    }
                    let e = self.bump();
                    match e {
                        b'n' => val.push('\n'),
                        b't' => val.push('\t'),
                        b'\\' => val.push('\\'),
                        b'"' => val.push('"'),
                        b'v' => val.push('\x0b'),
                        b'f' => val.push('\x0c'),
                        b'a' => val.push('\x07'),
                        b'0'..=b'7' => {
                            let mut v = (e - b'0') as u32;
                            for _ in 0..2 {
                                match self.peek(0) {
                                    Some(d @ b'0'..=b'7') => {
                                        v = v * 8 + (d - b'0') as u32;
                                        self.pos += 1;
                                    }
                                    _ => break,
                                }
                            }
                            val.push(char::from_u32(v).unwrap_or('\u{fffd}'));
                        }
                        b'x' => {
                            let mut v: u32 = 0;
                            let mut n = 0;
                            while n < 2 {
                                match self.peek(0) {
                                    Some(d @ b'0'..=b'9') => {
                                        v = v * 16 + (d - b'0') as u32;
                                        self.pos += 1;
                                        n += 1;
                                    }
                                    Some(d @ b'a'..=b'f') => {
                                        v = v * 16 + (d - b'a' + 10) as u32;
                                        self.pos += 1;
                                        n += 1;
                                    }
                                    Some(d @ b'A'..=b'F') => {
                                        v = v * 16 + (d - b'A' + 10) as u32;
                                        self.pos += 1;
                                        n += 1;
                                    }
                                    _ => break,
                                }
                            }
                            if n == 0 {
                                self.err(self.pos, "VL-E006", "bad \\x escape in string");
                            }
                            val.push(char::from_u32(v).unwrap_or('\u{fffd}'));
                        }
                        other => val.push(other as char),
                    }
                }
                b => {
                    // Copy the full UTF-8 sequence.
                    let char_start = self.pos - 1;
                    while !self.eof() && (self.src.as_bytes()[self.pos] & 0xC0) == 0x80 {
                        self.pos += 1;
                    }
                    val.push_str(&self.src[char_start..self.pos]);
                    let _ = b;
                }
            }
        }
        Token::new(Tok::StrLit, self.span_from(start), val)
    }

    fn lex_backtick(&mut self, start: usize) -> Token {
        self.bump(); // `
        match self.peek(0) {
            Some(b'"') => {
                self.bump();
                Token::new(Tok::MacroQuote, self.span_from(start), "")
            }
            Some(b'`') => {
                self.bump();
                Token::new(Tok::MacroPaste, self.span_from(start), "")
            }
            Some(c) if is_ident_start(c) => {
                let name = self.take_ident_body();
                if let Some(dir) = DirKind::from_str(&name) {
                    let text = if dir.takes_line_arg() {
                        self.take_directive_line()
                    } else {
                        String::new()
                    };
                    Token::new(Tok::Directive(dir), self.span_from(start), text.trim().to_string())
                } else {
                    Token::new(Tok::MacroId, self.span_from(start), name)
                }
            }
            _ => {
                self.err(start, "VL-E004", "expected directive or macro name after `` ` ``");
                Token::new(Tok::MacroId, self.span_from(start), "")
            }
        }
    }

    /// Rest of line; `` \ ``+newline continues onto the next line (IEEE
    /// 1364-2005 §19.2). The backslash-newline pairs are preserved verbatim;
    /// consumers treat them as whitespace/continuation.
    fn take_directive_line(&mut self) -> String {
        let mut out = String::new();
        loop {
            let ls = self.pos;
            while !self.eof() && self.cur() != b'\n' {
                self.pos += 1;
            }
            let mut line = &self.src[ls..self.pos];
            if line.ends_with('\r') {
                line = &line[..line.len() - 1];
            }
            let line_trim = line.trim_end();
            if !self.eof() {
                self.pos += 1; // newline
            }
            if let Some(stripped) = line_trim.strip_suffix('\\') {
                out.push_str(stripped);
                out.push('\n');
                if self.eof() {
                    break;
                }
            } else {
                out.push_str(line);
                break;
            }
        }
        out
    }

    fn lex_number(&mut self, start: usize) -> Token {
        let digits = self.take_digits();
        if self.peek(0) == Some(b'\'') {
            let width: Option<u32> = digits.parse().ok();
            return self.lex_based(start, width);
        }
        // Real: digits '.' digits, or digits e[+-]digits (1364-2005 §2.5.2? — §3.5? — grammar `exp`).
        if self.peek(0) == Some(b'.') && self.peek(1).is_some_and(|b| b.is_ascii_digit()) {
            self.bump(); // .
            self.take_digits();
            self.take_exp();
            return Token::new(Tok::RealLit, self.span_from(start), no_underscores(&self.src[start..self.pos]));
        }
        if matches!(self.peek(0), Some(b'e') | Some(b'E'))
            && matches!(self.peek(1), Some(b'0'..=b'9'))
        {
            self.pos += 1;
            self.take_digits();
            return Token::new(Tok::RealLit, self.span_from(start), no_underscores(&self.src[start..self.pos]));
        }
        if matches!(self.peek(0), Some(b'e') | Some(b'E'))
            && matches!(self.peek(1), Some(b'+') | Some(b'-'))
            && matches!(self.peek(2), Some(b'0'..=b'9'))
        {
            self.pos += 2;
            self.take_digits();
            return Token::new(Tok::RealLit, self.span_from(start), no_underscores(&self.src[start..self.pos]));
        }
        Token::new(Tok::IntLit, self.span_from(start), digits)
    }

    fn take_exp(&mut self) {
        if matches!(self.peek(0), Some(b'e') | Some(b'E')) {
            let off = if matches!(self.peek(1), Some(b'+') | Some(b'-')) { 2 } else { 1 };
            if matches!(self.peek(off), Some(b'0'..=b'9')) {
                self.pos += off;
                self.take_digits();
            }
        }
    }

    fn take_digits(&mut self) -> String {
        let start = self.pos;
        while !self.eof() && (self.cur().is_ascii_digit() || self.cur() == b'_') {
            self.pos += 1;
        }
        no_underscores(&self.src[start..self.pos])
    }

    fn lex_based(&mut self, start: usize, width: Option<u32>) -> Token {
        self.bump(); // '
        let mut text = String::new();
        if let Some(w) = width {
            text.push_str(&w.to_string());
        }
        text.push('\'');
        if matches!(self.peek(0), Some(b's') | Some(b'S')) {
            self.bump();
            text.push('s');
        }
        let base_ch = if self.eof() { 0 } else { self.bump() };
        let class: &[u8] = match base_ch.to_ascii_lowercase() {
            b'd' => {
                text.push('d');
                b"0123456789_xXzZ?"
            }
            b'h' => {
                text.push('h');
                b"0123456789abcdefABCDEF_xXzZ?"
            }
            b'o' => {
                text.push('o');
                b"01234567_xXzZ?"
            }
            b'b' => {
                text.push('b');
                b"01_xXzZ?"
            }
            _ => {
                self.err(start, "VL-E005", "expected base letter (d/h/o/b) after `'`");
                return Token::new(Tok::BasedLit, self.span_from(start), text);
            }
        };
        let dstart = self.pos;
        while !self.eof() && class.contains(&self.cur()) {
            self.pos += 1;
        }
        if self.pos == dstart {
            self.err(start, "VL-E005", "no digits in based number literal");
        }
        text.push_str(&no_underscores(&self.src[dstart..self.pos]));
        Token::new(Tok::BasedLit, self.span_from(start), text)
    }

    fn lex_punct(&mut self, start: usize) -> Token {
        use Punct::*;
        let b0 = self.bump();
        let two = |lx: &Self, c: u8| lx.peek(0) == Some(c);
        let p: Punct = match b0 {
            b'(' if two(self, b'*') => {
                self.pos += 1;
                AttrOpen
            }
            b'(' => LParen,
            b')' => RParen,
            b'[' => LBracket,
            b']' => RBracket,
            b'{' => LBrace,
            b'}' => RBrace,
            b';' => Semi,
            b',' => Comma,
            b'.' => Dot,
            b':' => Colon,
            b'?' => Question,
            b'@' => At,
            b'#' => Hash,
            b'+' => {
                if two(self, b':') {
                    self.pos += 1;
                    PlusColon
                } else {
                    Plus
                }
            }
            b'-' => {
                if two(self, b':') {
                    self.pos += 1;
                    MinusColon
                } else if two(self, b'>') {
                    self.pos += 1;
                    Arrow
                } else {
                    Minus
                }
            }
            b'*' => {
                if two(self, b')') {
                    self.pos += 1;
                    AttrClose
                } else if two(self, b'*') {
                    self.pos += 1;
                    Pow
                } else {
                    Star
                }
            }
            b'/' => Slash,
            b'%' => Percent,
            b'&' => {
                if two(self, b'&') {
                    self.pos += 1;
                    LogAnd
                } else {
                    Amp
                }
            }
            b'|' => {
                if two(self, b'|') {
                    self.pos += 1;
                    LogOr
                } else {
                    Pipe
                }
            }
            b'^' => {
                if two(self, b'~') {
                    self.pos += 1;
                    Xnor
                } else {
                    Caret
                }
            }
            b'~' => match self.peek(0) {
                Some(b'&') => {
                    self.pos += 1;
                    Nand
                }
                Some(b'|') => {
                    self.pos += 1;
                    Nor
                }
                Some(b'^') => {
                    self.pos += 1;
                    Xnor
                }
                _ => Tilde,
            },
            b'!' => {
                if two(self, b'=') {
                    self.pos += 1;
                    if two(self, b'=') {
                        self.pos += 1;
                        NotEqEq
                    } else {
                        NotEq
                    }
                } else {
                    Bang
                }
            }
            b'<' => match self.peek(0) {
                Some(b'<') => {
                    self.pos += 1;
                    if two(self, b'<') {
                        self.pos += 1;
                        AShl
                    } else {
                        Shl
                    }
                }
                Some(b'=') => {
                    self.pos += 1;
                    Le
                }
                _ => Lt,
            },
            b'>' => match self.peek(0) {
                Some(b'>') => {
                    self.pos += 1;
                    if two(self, b'>') {
                        self.pos += 1;
                        AShr
                    } else {
                        Shr
                    }
                }
                Some(b'=') => {
                    self.pos += 1;
                    Ge
                }
                _ => Gt,
            },
            b'=' => {
                if two(self, b'=') {
                    self.pos += 1;
                    if two(self, b'=') {
                        self.pos += 1;
                        EqEqEq
                    } else {
                        EqEq
                    }
                } else {
                    Eq
                }
            }
            _ => {
                self.pos = start + 1;
                let ch = self.src[start..].chars().next().unwrap_or('?');
                self.err(start, "VL-E001", format!("unexpected character `{ch}`"));
                return Token::new(
                    Tok::Error,
                    Span::new(
                        self.file,
                        (start + self.base as usize) as u32,
                        (start + 1 + self.base as usize) as u32,
                    ),
                    String::new(),
                );
            }
        };
        Token::new(Tok::Punct(p), self.span_from(start), "")
    }
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident_cont(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

fn no_underscores(s: &str) -> String {
    s.chars().filter(|&c| c != '_').collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcs_diag::SourceMap;

    fn lex_ok(src: &str) -> Vec<Token> {
        let mut sm = SourceMap::new();
        let mut diags = DiagSink::new();
        let f = sm.add("t.v", src.to_string());
        let toks = lex(src, f, &mut diags);
        assert!(!diags.has_errors(), "unexpected lex errors: {:#?}", diags.diags);
        toks
    }

    fn kinds(toks: &[Token]) -> Vec<Tok> {
        toks.iter().map(|t| t.kind).collect()
    }

    #[test]
    fn simple_module_tokens() {
        let toks = lex_ok("module counter(input clk); endmodule");
        assert_eq!(kinds(&toks)[0], Tok::Keyword(Kw::Module));
        assert_eq!(toks[1].text, "counter");
        assert_eq!(kinds(&toks)[2], Tok::Punct(Punct::LParen));
        assert_eq!(kinds(&toks)[3], Tok::Keyword(Kw::Input));
        assert_eq!(toks[4].text, "clk");
        assert_eq!(toks.last().unwrap().kind, Tok::Eof);
    }

    #[test]
    fn based_literals() {
        for (src, expect) in [
            ("8'sh3f", "8'sh3f"),
            ("4'b01x?", "4'b01x?"),
            ("'sd15", "'sd15"),
            ("12'hDEAD_BEEF", "12'hDEADBEEF"),
            ("'h3x", "'h3x"),
            ("1'b1", "1'b1"),
        ] {
            let toks = lex_ok(src);
            assert_eq!(toks[0].kind, Tok::BasedLit, "src={src}");
            assert_eq!(toks[0].text, expect, "src={src}");
        }
    }

    #[test]
    fn int_and_real() {
        assert_eq!(lex_ok("42")[0].text, "42");
        assert_eq!(lex_ok("1_000")[0].text, "1000");
        assert_eq!(lex_ok("3.14")[0].kind, Tok::RealLit);
        assert_eq!(lex_ok("1.5e-3")[0].text, "1.5e-3");
        assert_eq!(lex_ok("1e5")[0].kind, Tok::RealLit);
        // `1e` is not a real: IntLit + Ident.
        let t = lex_ok("1ex");
        assert_eq!(t[0].kind, Tok::IntLit);
        assert_eq!(t[1].kind, Tok::Ident);
    }

    #[test]
    fn string_escapes() {
        let t = lex_ok(r#""hi\n\t\101\x41""#);
        assert_eq!(t[0].kind, Tok::StrLit);
        assert_eq!(t[0].text, "hi\n\tAA");
    }

    #[test]
    fn operators_munch() {
        let toks = lex_ok("a <= b === c <<< d >>> e ** f && g || !h ~^ i ^~ j +: k");
        let got: Vec<String> = toks[..toks.len() - 1]
            .iter()
            .map(|t| match t.kind {
                Tok::Ident => t.text.clone(),
                Tok::Punct(p) => p.as_str().to_string(),
                _ => "?".to_string(),
            })
            .collect();
        assert_eq!(
            got,
            vec!["a", "<=", "b", "===", "c", "<<<", "d", ">>>", "e", "**", "f", "&&", "g", "||", "!", "h", "~^", "i", "~^", "j", "+:", "k"] // ^~ normalizes to Xnor.as_str()="~^"
        );
    }

    #[test]
    fn attribute_tokens() {
        let toks = lex_ok("(* full_case *) module m; endmodule");
        assert_eq!(toks[0].kind, Tok::Punct(Punct::AttrOpen));
        assert_eq!(kinds(&toks)[2], Tok::Punct(Punct::AttrClose));
    }

    #[test]
    fn directives_and_macros() {
        let toks = lex_ok("`timescale 1ns/1ps\n`define FOO 1\n`BAR(x)\n`ifdef X\n`else\n`endif");
        assert_eq!(toks[0].kind, Tok::Directive(DirKind::Timescale));
        assert_eq!(toks[0].text, "1ns/1ps");
        assert_eq!(toks[1].kind, Tok::Directive(DirKind::Define));
        assert_eq!(toks[1].text, "FOO 1");
        assert_eq!(toks[2].kind, Tok::MacroId);
        assert_eq!(toks[2].text, "BAR");
        assert_eq!(kinds(&toks)[6], Tok::Directive(DirKind::Ifdef));
        assert_eq!(kinds(&toks)[7], Tok::Directive(DirKind::Else));
        assert_eq!(kinds(&toks)[8], Tok::Directive(DirKind::Endif));
    }

    #[test]
    fn define_multiline_continuation() {
        let toks = lex_ok("`define ADD(a, b) \\\n  ((a) + (b))\nmodule m; endmodule");
        assert_eq!(toks[0].kind, Tok::Directive(DirKind::Define));
        assert_eq!(toks[0].text, "ADD(a, b) \n  ((a) + (b))");
        assert_eq!(toks[1].kind, Tok::Keyword(Kw::Module));
    }

    #[test]
    fn system_ident_and_comments() {
        let toks = lex_ok("$display // c1\n/* block */ $finish");
        assert_eq!(toks[0].kind, Tok::SysIdent);
        assert_eq!(toks[0].text, "$display");
        assert_eq!(toks[1].kind, Tok::SysIdent);
        assert_eq!(toks[1].text, "$finish");
    }

    #[test]
    fn escaped_identifier() {
        let toks = lex_ok("\\bus+width ");
        assert_eq!(toks[0].kind, Tok::Ident);
        assert_eq!(toks[0].text, "bus+width");
    }

    #[test]
    fn error_cases_report_not_crash() {
        let mut sm = SourceMap::new();
        let mut diags = DiagSink::new();
        let src = "'q5 \"unterminated";
        let f = sm.add("t.v", src.to_string());
        let _ = lex(src, f, &mut diags);
        assert!(diags.has_errors());
    }
}
