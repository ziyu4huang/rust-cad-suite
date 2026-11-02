//! ASP/1 負載編碼:手寫 JSON 子集(ANALYZER-SERVICE §5.2/§5.3;零 serde)。
//!
//! Reader 禁項(命中即拒,§5.2):浮點/指數字面量、越 i64 整數、前導零、
//! 對象重複鍵、尾逗號、註釋、NaN/字面量拼錯、原始控制字符(<0x20)、
//! 孤代理對;另設巢狀深度上限 128(防棧溢出,自建契約非 §5.2 列項)。
//! 寫律五條見 §5.3:緊湊形/Int 十進制無前導零/轉義七種+`<0x20`→`\u00xx`
//! 小寫/Obj 鍵不重排/無校驗和無壓縮。

use std::fmt;

/// JSON 子集值;Obj 保序(寫出序=插入序,§5.3 律 4)。
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodecError {
    pub msg: String,
}

impl CodecError {
    fn new(msg: impl Into<String>) -> Self {
        CodecError { msg: msg.into() }
    }
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "codec: {}", self.msg)
    }
}

impl std::error::Error for CodecError {}

/// 巢狀深度上限(reader 自衛;超出即拒)。
const MAX_DEPTH: usize = 128;

impl Json {
    /// 正典寫律(§5.3)寫入 `out`。
    pub fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Int(n) => out.push_str(&n.to_string()),
            Json::Str(s) => write_str(s, out),
            Json::Arr(items) => {
                out.push('[');
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write(out);
                }
                out.push(']');
            }
            Json::Obj(entries) => {
                out.push('{');
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_str(k, out);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }

    /// 正典序列化(緊湊形)。
    pub fn render(&self) -> String {
        let mut s = String::new();
        self.write(&mut s);
        s
    }

    /// 嚴格 reader:禁項全表 §5.2,未知鍵不拒(按鍵查找,§5.4 兼容律)、
    /// 重複鍵拒;token 間允許四種 JSON 空白。
    pub fn parse(s: &str) -> Result<Json, CodecError> {
        let chars: Vec<char> = s.chars().collect();
        let mut p = P { c: &chars, pos: 0, depth: 0 };
        p.ws();
        let v = p.value()?;
        p.ws();
        if p.pos != p.c.len() {
            return Err(p.err("trailing content after JSON value"));
        }
        Ok(v)
    }
}

fn write_str(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                // <0x20 → \u00xx 小寫十六進制(§5.3 律 3,同 JS stringify)。
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

struct P<'a> {
    c: &'a [char],
    pos: usize,
    depth: usize,
}

impl<'a> P<'a> {
    fn err(&self, msg: impl AsRef<str>) -> CodecError {
        CodecError::new(format!("{} at char offset {}", msg.as_ref(), self.pos))
    }

    fn peek(&self) -> Option<char> {
        self.c.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek();
        if ch.is_some() {
            self.pos += 1;
        }
        ch
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.pos += 1;
        }
    }

    fn value(&mut self) -> Result<Json, CodecError> {
        match self.peek() {
            None => Err(self.err("unexpected end of input")),
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => Ok(Json::Str(self.string()?)),
            Some('t') => self.lit("true", Json::Bool(true)),
            Some('f') => self.lit("false", Json::Bool(false)),
            Some('n') => self.lit("null", Json::Null),
            Some('-') | Some('0'..='9') => self.number(),
            Some(ch) => Err(self.err(format!("unexpected character `{ch}`"))),
        }
    }

    fn lit(&mut self, word: &str, v: Json) -> Result<Json, CodecError> {
        for expected in word.chars() {
            if self.bump() != Some(expected) {
                return Err(self.err(format!("invalid literal, expected `{word}`")));
            }
        }
        Ok(v)
    }

    fn object(&mut self) -> Result<Json, CodecError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.err("nesting too deep"));
        }
        self.pos += 1; // '{'
        let mut entries: Vec<(String, Json)> = Vec::new();
        self.ws();
        if self.peek() == Some('}') {
            self.pos += 1;
            self.depth -= 1;
            return Ok(Json::Obj(entries));
        }
        loop {
            self.ws();
            if self.peek() != Some('"') {
                return Err(self.err("expected object key string"));
            }
            let key = self.string()?;
            self.ws();
            if self.peek() != Some(':') {
                return Err(self.err("expected `:` after object key"));
            }
            self.pos += 1;
            self.ws();
            let val = self.value()?;
            if entries.iter().any(|(k, _)| *k == key) {
                return Err(self.err(format!("duplicate object key `{key}`")));
            }
            entries.push((key, val));
            self.ws();
            match self.peek() {
                Some(',') => self.pos += 1, // 尾逗號:下一輪鍵檢查自然拒('}' 非 '"')
                Some('}') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err(self.err("expected `,` or `}` in object")),
            }
        }
        self.depth -= 1;
        Ok(Json::Obj(entries))
    }

    fn array(&mut self) -> Result<Json, CodecError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.err("nesting too deep"));
        }
        self.pos += 1; // '['
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(']') {
            self.pos += 1;
            self.depth -= 1;
            return Ok(Json::Arr(items));
        }
        loop {
            self.ws();
            items.push(self.value()?);
            self.ws();
            match self.peek() {
                Some(',') => self.pos += 1,
                Some(']') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err(self.err("expected `,` or `]` in array")),
            }
        }
        self.depth -= 1;
        Ok(Json::Arr(items))
    }

    fn string(&mut self) -> Result<String, CodecError> {
        self.pos += 1; // '"'
        let mut out = String::new();
        loop {
            match self.bump() {
                None => return Err(self.err("unterminated string")),
                Some('"') => return Ok(out),
                Some('\\') => match self.bump() {
                    None => return Err(self.err("unterminated escape")),
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('/') => out.push('/'),
                    Some('b') => out.push('\u{0008}'),
                    Some('f') => out.push('\u{000c}'),
                    Some('n') => out.push('\n'),
                    Some('r') => out.push('\r'),
                    Some('t') => out.push('\t'),
                    Some('u') => {
                        let hi = self.hex4()?;
                        let cp = if (0xD800..=0xDBFF).contains(&hi) {
                            // 高代理必須緊跟 \uDC00..\uDFFF(孤代理=拒)。
                            if self.bump() != Some('\\') || self.bump() != Some('u') {
                                return Err(self.err("lone high surrogate"));
                            }
                            let lo = self.hex4()?;
                            if !(0xDC00..=0xDFFF).contains(&lo) {
                                return Err(self.err("invalid low surrogate"));
                            }
                            0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                        } else if (0xDC00..=0xDFFF).contains(&hi) {
                            return Err(self.err("lone low surrogate"));
                        } else {
                            hi
                        };
                        match char::from_u32(cp) {
                            Some(c) => out.push(c),
                            None => return Err(self.err("invalid codepoint")),
                        }
                    }
                    Some(ch) => return Err(self.err(format!("invalid escape `\\{ch}`"))),
                },
                Some(ch) if (ch as u32) < 0x20 => {
                    return Err(self.err("raw control character in string"))
                }
                Some(ch) if (0xD800..=0xDFFF).contains(&(ch as u32)) => {
                    return Err(self.err("lone surrogate in text"))
                }
                Some(ch) => out.push(ch),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, CodecError> {
        let mut v = 0u32;
        for _ in 0..4 {
            let d = match self.bump() {
                Some(c) if c.is_ascii_hexdigit() => c.to_digit(16).unwrap(),
                _ => return Err(self.err("invalid `\\u` escape")),
            };
            v = v * 16 + d;
        }
        Ok(v)
    }

    fn number(&mut self) -> Result<Json, CodecError> {
        let start = self.pos;
        if self.peek() == Some('-') {
            self.pos += 1;
        }
        match self.peek() {
            Some('0') => {
                self.pos += 1;
                if matches!(self.peek(), Some('0'..='9')) {
                    return Err(self.err("leading zero in number"));
                }
            }
            Some('1'..='9') => {
                while matches!(self.peek(), Some('0'..='9')) {
                    self.pos += 1;
                }
            }
            _ => return Err(self.err("invalid number")),
        }
        // 浮點/指數禁項(§5.2):v1 負載無浮點,見到即拒。
        if matches!(self.peek(), Some('.') | Some('e') | Some('E')) {
            return Err(self.err("float/exponent numbers not in ASP/1 subset"));
        }
        let text: String = self.c[start..self.pos].iter().collect();
        let n: i64 = text.parse().map_err(|_| self.err("integer out of i64 range"))?;
        Ok(Json::Int(n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt(j: Json) -> String {
        let s = j.render();
        assert_eq!(Json::parse(&s).unwrap(), j, "roundtrip of {s}");
        s
    }

    // ---- 寫律五條(§5.3)逐條 ----

    #[test]
    fn write_law1_compact_no_whitespace() {
        let j = Json::Obj(vec![
            ("a".into(), Json::Int(1)),
            ("b".into(), Json::Arr(vec![Json::Null, Json::Bool(true)])),
        ]);
        assert_eq!(j.render(), r#"{"a":1,"b":[null,true]}"#);
    }

    #[test]
    fn write_law2_int_decimal_no_leading_zero() {
        assert_eq!(Json::Int(0).render(), "0");
        assert_eq!(Json::Int(-5).render(), "-5");
        assert_eq!(Json::Int(i64::MIN).render(), "-9223372036854775808");
        assert_eq!(Json::Int(i64::MAX).render(), "9223372036854775807");
    }

    #[test]
    fn write_law3_escapes_and_lowercase_u() {
        let j = Json::Str("a\"b\\c\u{8}d\u{c}e\nf\rg\th\u{1}\u{f}".into());
        assert_eq!(j.render(), "\"a\\\"b\\\\c\\bd\\fe\\nf\\rg\\th\\u0001\\u000f\"");
    }

    #[test]
    fn write_law4_obj_key_order_preserved() {
        let j = Json::Obj(vec![
            ("z".into(), Json::Int(1)),
            ("a".into(), Json::Int(2)),
        ]);
        assert_eq!(j.render(), r#"{"z":1,"a":2}"#, "插入序即寫出序,不重排");
        let rev = Json::Obj(vec![
            ("a".into(), Json::Int(2)),
            ("z".into(), Json::Int(1)),
        ]);
        assert_ne!(j.render(), rev.render());
    }

    #[test]
    fn write_law5_no_checksum_plain_payload() {
        // 幀層無校驗和/壓縮:frame.rs 負責;此處釘負載=純文本無包裝。
        assert_eq!(Json::Str("x".into()).render(), "\"x\"");
    }

    // ---- reader 禁項(§5.2)逐條 ----

    #[test]
    fn reject_float_and_exponent() {
        assert!(Json::parse("{\"x\":1.5}").is_err());
        assert!(Json::parse("{\"x\":1e5}").is_err());
        assert!(Json::parse("{\"x\":1E-2}").is_err());
        assert!(Json::parse("{\"x\":0.0}").is_err());
        assert!(Json::parse("1.5").is_err());
        assert!(Json::parse("1e5").is_err());
    }

    #[test]
    fn reject_out_of_i64_and_leading_zero() {
        assert!(Json::parse("{\"n\":9223372036854775808}").is_err());
        assert!(Json::parse("{\"n\":-9223372036854775809}").is_err());
        assert!(Json::parse("01").is_err());
        assert!(Json::parse("-01").is_err());
        assert_eq!(Json::parse("0").unwrap(), Json::Int(0));
        assert_eq!(Json::parse("-0").unwrap(), Json::Int(0));
        assert_eq!(Json::parse("-9223372036854775808").unwrap(), Json::Int(i64::MIN));
    }

    #[test]
    fn reject_duplicate_keys() {
        assert!(Json::parse("{\"a\":1,\"a\":2}").is_err());
        // 未知鍵不拒(§5.4 兼容律)。
        assert!(Json::parse("{\"a\":1,\"unknown\":2}").is_ok());
    }

    #[test]
    fn reject_trailing_comma_and_comments_and_nan() {
        assert!(Json::parse("{\"a\":[1,2,]}").is_err());
        assert!(Json::parse("{\"a\":1,}").is_err());
        assert!(Json::parse("[1,]").is_err());
        assert!(Json::parse("{\"a\":1} // hi").is_err());
        assert!(Json::parse("{\"a\":1} /* x */").is_err());
        assert!(Json::parse("NaN").is_err());
        assert!(Json::parse("{\"x\":NaN}").is_err());
        assert!(Json::parse("True").is_err());
        assert!(Json::parse("nulll").is_err());
    }

    #[test]
    fn reject_raw_control_char_and_bad_escape() {
        assert!(Json::parse("{\"s\":\"a\u{1}b\"}").is_err());
        assert!(Json::parse(r#"{"s":"\x"}"#).is_err(), "\\x 非法轉義");
        assert!(Json::parse("{\"s\":\"\\ud800\"}").is_err(), "孤高代理");
        assert!(Json::parse("{\"s\":\"\\udc00\"}").is_err(), "孤低代理");
        assert!(Json::parse("{\"s\":\"\\ud800x\"}").is_err(), "高代理未跟 \\u");
        // 合法代理對 = astral 字符(\ud83d\ude00 → 😀)。
        assert_eq!(
            Json::parse("{\"s\":\"\\ud83d\\ude00\"}").unwrap(),
            Json::Obj(vec![("s".into(), Json::Str("😀".into()))])
        );
    }

    // ---- 基本回讀與嚴格面 ----

    #[test]
    fn parse_accepts_whitespace_and_unknown_keys() {
        let j = Json::parse(" { \"a\" : [ 1 , 2 ] , \"b\" : true } \n").unwrap();
        assert_eq!(
            j,
            Json::Obj(vec![
                ("a".into(), Json::Arr(vec![Json::Int(1), Json::Int(2)])),
                ("b".into(), Json::Bool(true)),
            ])
        );
    }

    #[test]
    fn parse_rejects_trailing_and_depth() {
        assert!(Json::parse("{} {}").is_err());
        let deep = "[".repeat(MAX_DEPTH + 1);
        assert!(Json::parse(&deep).is_err(), "深度上限拒收");
        let deep_ok = "[".repeat(MAX_DEPTH) + &"]".repeat(MAX_DEPTH);
        assert!(Json::parse(&deep_ok).is_ok(), "恰在上限內收");
    }

    #[test]
    fn roundtrip_variants() {
        rt(Json::Null);
        rt(Json::Bool(true));
        rt(Json::Int(-123));
        rt(Json::Str("tab\tnew\nquote\"backslash\u{0}".into()));
        rt(Json::Arr(vec![Json::Null, Json::Int(3), Json::Str("s".into())]));
        rt(Json::Obj(vec![("k".into(), Json::Obj(vec![]))]));
    }
}
