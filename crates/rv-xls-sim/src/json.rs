//! Minimal JSON reader/writer for the XLS adapter protocol.
//!
//! The repo keeps process/ABI surfaces serde-free (the host FFI manifest is
//! line-based for the same reason), so the adapter accepts a small, fixed JSON
//! subset — one object whose values are strings, booleans, arrays of strings,
//! or an object of string values — and parses it with a recursive-descent
//! reader whose whole surface is unit-tested below. This is not a general JSON
//! library; unknown request keys are ignored so a future XLS field does not
//! break the adapter.

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    /// Last write wins, like most JSON readers.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(pairs) => pairs.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

/// Parse one JSON value; trailing non-whitespace is an error.
pub fn parse(input: &str) -> Result<Json, String> {
    let mut parser = Parser { src: input, pos: 0 };
    let value = parser.value()?;
    parser.ws();
    if parser.pos != input.len() {
        return Err(format!("trailing characters at byte {}", parser.pos));
    }
    Ok(value)
}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\n' | '\r' | '\t')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, want: char) -> Result<(), String> {
        self.ws();
        match self.bump() {
            Some(c) if c == want => Ok(()),
            Some(c) => Err(format!("expected `{want}`, found `{c}` at byte {}", self.pos - c.len_utf8())),
            None => Err(format!("expected `{want}`, found end of input")),
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        match self.peek() {
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => Ok(Json::String(self.string()?)),
            Some('t') => self.literal("true", Json::Bool(true)),
            Some('f') => self.literal("false", Json::Bool(false)),
            Some('n') => self.literal("null", Json::Null),
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            Some(c) => Err(format!("unexpected character `{c}` at byte {}", self.pos)),
            None => Err("expected a value, found end of input".to_string()),
        }
    }

    fn literal(&mut self, text: &str, value: Json) -> Result<Json, String> {
        if self.src[self.pos..].starts_with(text) {
            self.pos += text.len();
            Ok(value)
        } else {
            Err(format!("invalid literal at byte {}", self.pos))
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E') {
                self.pos += 1;
            } else {
                break;
            }
        }
        self.src[start..self.pos]
            .parse::<f64>()
            .map(Json::Number)
            .map_err(|_| format!("invalid number `{}`", &self.src[start..self.pos]))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut out = String::new();
        loop {
            let c = self.bump().ok_or_else(|| "unterminated string".to_string())?;
            match c {
                '"' => return Ok(out),
                '\\' => {
                    let esc = self.bump().ok_or_else(|| "unterminated escape".to_string())?;
                    match esc {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'b' => out.push('\u{0008}'),
                        'f' => out.push('\u{000c}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let hi = self.hex4()?;
                            let ch = if (0xD800..=0xDBFF).contains(&hi) {
                                if self.bump() != Some('\\') || self.bump() != Some('u') {
                                    return Err("unpaired high surrogate".to_string());
                                }
                                let lo = self.hex4()?;
                                if !(0xDC00..=0xDFFF).contains(&lo) {
                                    return Err("invalid low surrogate".to_string());
                                }
                                let cp = 0x10000 + (((hi - 0xD800) as u32) << 10) + (lo - 0xDC00) as u32;
                                char::from_u32(cp).ok_or_else(|| "invalid surrogate pair".to_string())?
                            } else {
                                char::from_u32(hi as u32)
                                    .ok_or_else(|| format!("invalid unicode escape U+{hi:04X}"))?
                            };
                            out.push(ch);
                        }
                        other => return Err(format!("invalid escape `\\{other}`")),
                    }
                }
                c if (c as u32) < 0x20 => {
                    return Err(format!("raw control character U+{:04X} in string", c as u32));
                }
                c => out.push(c),
            }
        }
    }

    fn hex4(&mut self) -> Result<u16, String> {
        let mut value = 0u16;
        for _ in 0..4 {
            let c = self.bump().ok_or_else(|| "truncated \\u escape".to_string())?;
            let digit = c
                .to_digit(16)
                .ok_or_else(|| format!("invalid hex digit `{c}` in \\u escape"))?;
            value = value * 16 + digit as u16;
        }
        Ok(value)
    }

    fn array(&mut self) -> Result<Json, String> {
        self.expect('[')?;
        self.ws();
        if self.peek() == Some(']') {
            self.bump();
            return Ok(Json::Array(Vec::new()));
        }
        let mut items = Vec::new();
        loop {
            items.push(self.value()?);
            self.ws();
            match self.bump() {
                Some(',') => continue,
                Some(']') => return Ok(Json::Array(items)),
                Some(c) => return Err(format!("expected `,` or `]`, found `{c}`")),
                None => return Err("unterminated array".to_string()),
            }
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.expect('{')?;
        self.ws();
        if self.peek() == Some('}') {
            self.bump();
            return Ok(Json::Object(Vec::new()));
        }
        let mut pairs = Vec::new();
        loop {
            self.ws();
            let key = self.string()?;
            self.expect(':')?;
            let value = self.value()?;
            pairs.push((key, value));
            self.ws();
            match self.bump() {
                Some(',') => continue,
                Some('}') => return Ok(Json::Object(pairs)),
                Some(c) => return Err(format!("expected `,` or `}}`, found `{c}`")),
                None => return Err("unterminated object".to_string()),
            }
        }
    }
}

/// Quote one string as JSON (control characters escaped, non-ASCII kept UTF-8).
pub fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_request_shape() {
        let text = r#"{
            "text": "module tb; endmodule\n",
            "file_type": ".v",
            "defines": {"WIDTH": "8", "FLAG": ""},
            "includes": ["/a", "/b"],
            "syntax_only": false
        }"#;
        let json = parse(text).expect("valid json");
        assert_eq!(json.get("file_type").and_then(Json::as_str), Some(".v"));
        assert_eq!(json.get("syntax_only").and_then(Json::as_bool), Some(false));
        let defines = json.get("defines").expect("defines object");
        assert_eq!(defines.get("WIDTH").and_then(Json::as_str), Some("8"));
        assert_eq!(
            json.get("includes"),
            Some(&Json::Array(vec![
                Json::String("/a".to_string()),
                Json::String("/b".to_string())
            ]))
        );
    }

    #[test]
    fn string_escapes_and_unicode() {
        let json = parse(r#""a\"b\\c\nd\u00e9\uD83D\uDE00""#).expect("valid string");
        assert_eq!(
            json.as_str(),
            Some("a\"b\\c\ndé😀"),
            "escapes, BMP and surrogate pairs"
        );
    }

    #[test]
    fn rejects_malformed_input() {
        for (input, why) in [
            ("{", "unterminated object"),
            ("[1,]", "trailing comma"),
            ("\"unterminated", "unterminated string"),
            ("tru", "bad literal"),
            ("{} junk", "trailing characters"),
            ("", "empty input"),
            ("\"a\nb\"", "raw control char"),
            ("\"\\q\"", "invalid escape"),
        ] {
            assert!(parse(input).is_err(), "{why}: {input:?} should fail");
        }
    }

    #[test]
    fn duplicate_keys_last_wins() {
        let json = parse(r#"{"a": "1", "a": "2"}"#).expect("valid");
        assert_eq!(json.get("a").and_then(Json::as_str), Some("2"));
    }

    #[test]
    fn quote_round_trips() {
        let original = "line1\nline2\t\"q\"\\ \u{0001}é";
        let encoded = quote(original);
        let parsed = parse(&encoded).expect("quoted string parses");
        assert_eq!(parsed.as_str(), Some(original));
    }
}
