//! 行導向詞法:逐字照抄 oracle `RTLILFrontendWorker` 的行緩衝語義
//! (vendor-reference/yosys frontends/rtlil/rtlil_frontend.cc:33-259)。
//!
//! 不變式:當前行剩餘 `rest` 永遠非空(至少含 `'\n'`);`#` 起頭的行是
//! 註釋(整行丟棄,包括 oracle `-readable` 輸出裡可能出現的 `# driver …`
//! 行——前端本來就不解析它們)。ID 詞法:`\` 或 `$` 開頭、吃到
//! ' '/'\t'/'\n'/'\r'——名字因此可含 `[]:'=.` 等字元,這是
//! `$paramod\submod\c=s32'111` 複合類型作為單 token 的根據。

use crate::ast::{Bit, Const};

/// RTLIL::WIDTH_LIMIT(kernel/rtlil.h:103)= 1 << 30。
pub const WIDTH_LIMIT: i64 = 1 << 30;

/// 解析錯誤:`Parser error in line {line}: {msg}`(oracle log_error 前綴
/// `ERROR: ` 由 CLI 層補,庫層持有核心訊息)。未支援構造的硬錯帶里程碑
/// 指針(鐵則 3:未實現構造禁靜默跑偏)。
#[derive(Clone, Debug, PartialEq)]
pub enum RyError {
    Parse { line: usize, msg: String },
    Unsupported { line: usize, what: &'static str, milestone: &'static str },
}

impl core::fmt::Display for RyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RyError::Parse { line, msg } => write!(f, "Parser error in line {line}: {msg}"),
            RyError::Unsupported { line, what, milestone } => write!(
                f,
                "Parser error in line {line}: {what} is not supported in ryosys (milestone {milestone})"
            ),
        }
    }
}

impl std::error::Error for RyError {}

/// 行遊標。`lines` 為按 `\n` 切分後的原始行(不含 `\n`;最後一行若無
/// 換行也照常);`rest` 是當前行未消費的部分(內部保證非空)。
pub struct Cursor<'a> {
    lines: Vec<&'a str>,
    row: usize,
    pub line_num: usize,
    rest: String,
    good: bool,
}

impl<'a> Cursor<'a> {
    pub fn new(input: &'a str) -> Cursor<'a> {
        // getline 語義:空輸入=沒有任何行;`"a\n"`=一行 "a"。
        let lines: Vec<&str> = if input.is_empty() {
            Vec::new()
        } else {
            input.split('\n').collect()
        };
        let mut c = Cursor { lines, row: 0, line_num: 0, rest: String::new(), good: true };
        c.advance_to_next_nonempty_line();
        c
    }

    pub fn good(&self) -> bool {
        self.good
    }

    fn err(&self, msg: String) -> RyError {
        RyError::Parse { line: self.line_num, msg }
    }

    fn err_unsupported(&self, what: &'static str, milestone: &'static str) -> RyError {
        RyError::Unsupported { line: self.line_num, what, milestone }
    }

    /// oracle advance_to_next_nonempty_line:跳過空行與(剝前導空白後的)
    /// 純註釋行;流結束後 rest 固定 "\n"。
    pub fn advance_to_next_nonempty_line(&mut self) {
        if !self.good {
            self.rest = "\n".to_string();
            return;
        }
        loop {
            let raw = self.lines[self.row];
            self.row += 1;
            self.line_num += 1;
            if self.row >= self.lines.len() {
                self.good = false;
            }
            self.rest = raw.to_string();
            self.rest.push('\n');
            self.consume_whitespace_and_comments();
            if !self.rest.starts_with('\n') || !self.good {
                break;
            }
        }
    }

    /// 空白(' '/'\t')逐字元剝;`#` 起頭=註釋→整行丟棄。
    fn consume_whitespace_and_comments(&mut self) {
        loop {
            let mut chars = self.rest.chars();
            match chars.next() {
                Some(' ') | Some('\t') => {
                    self.rest.remove(0);
                }
                Some('#') => {
                    self.rest = "\n".to_string();
                    return;
                }
                _ => return,
            }
        }
    }

    /// 關鍵字匹配:前綴相等且下一字元非 'a'..'z'(`end1` 會匹配 `end`
    /// 然後在 expect_eol 報錯——oracle 同款)。
    pub fn try_parse_keyword(&mut self, keyword: &str) -> bool {
        let kw = keyword.as_bytes();
        if !self.rest.as_bytes().starts_with(kw) {
            return false;
        }
        // rest 永遠非空且不以 \n 前綴縮短到空,越界安全由 strip 前置判斷保證。
        let after = self.rest.as_bytes()[kw.len()];
        if (b'a'..=b'z').contains(&after) {
            return false;
        }
        self.rest.replace_range(..kw.len(), "");
        self.consume_whitespace_and_comments();
        true
    }

    /// error_token:當前位置到 ' '/'\t'/'\n' 的文字(oracle error_token,
    /// 可為空串)。
    pub fn error_token(&self) -> String {
        let mut result = String::new();
        for ch in self.rest.chars() {
            if ch == '\n' || ch == ' ' || ch == '\t' {
                break;
            }
            result.push(ch);
        }
        result
    }

    pub fn try_parse_char(&mut self, ch: char) -> bool {
        if !self.rest.starts_with(ch) {
            return false;
        }
        self.rest.remove(0);
        self.consume_whitespace_and_comments();
        true
    }

    pub fn try_parse_eol(&mut self) -> bool {
        if !self.rest.starts_with('\n') {
            return false;
        }
        self.advance_to_next_nonempty_line();
        true
    }

    pub fn expect_eol(&mut self) -> Result<(), RyError> {
        if self.try_parse_eol() {
            Ok(())
        } else {
            Err(self.err(format!("Expected EOL, got `{ }'.", self.error_token())))
        }
    }

    /// ID 詞法:`\`/`$` 開頭吃到 ' '/'\t'/'\n'/'\r'(其他控制字元**不是**
    /// 終止符——照抄 `ch <= ' ' && (ch == ' ' || ch == '\t' || ch == '\n'
    /// || ch == '\r')`)。`$priv@`/`$pub@` 前綴=twines 壓縮名,M0 硬錯
    /// (milestone Y1;oracle 對它們是靜默錯名,猜=假綠)。
    pub fn try_parse_id(&mut self) -> Result<Option<String>, RyError> {
        let first = self.rest.as_bytes().first().copied();
        if first != Some(b'\\') && first != Some(b'$') {
            return Ok(None);
        }
        let bytes = self.rest.as_bytes();
        let mut idx = 1;
        while idx < bytes.len() {
            let ch = bytes[idx];
            if ch == b' ' || ch == b'\t' || ch == b'\n' || ch == b'\r' {
                break;
            }
            idx += 1;
        }
        // idx < len 時必停在空白;idx == len 只可能是 rest 末尾的 '\n' 被剝——
        // 實際上 rest 必含 '\n' 尾,循環必 break。防禦無需。
        let id = self.rest[..idx].to_string();
        if id.starts_with("$priv@") || id.starts_with("$pub@") {
            return Err(self.err_unsupported(
                if id.starts_with("$priv@") { "`$priv@' name token" } else { "`$pub@' name token" },
                "Y1",
            ));
        }
        self.rest.replace_range(..idx, "");
        self.consume_whitespace_and_comments();
        Ok(Some(id))
    }

    pub fn parse_id(&mut self) -> Result<String, RyError> {
        match self.try_parse_id()? {
            Some(id) => Ok(id),
            None => Err(self.err(format!("Expected twine reference or ID, got `{ }'.", self.error_token()))),
        }
    }

    /// parse_integer:整數+消費空白(parse_integer + consume)。
    pub fn parse_integer(&mut self) -> Result<i64, RyError> {
        let v = self.parse_integer_alone()?;
        self.consume_whitespace_and_comments();
        Ok(v)
    }

    /// parse_integer_alone:可選 '-' + 十進位數字;無數字/溢位=
    /// "Invalid integer"(oracle from_chars 失敗路徑)。**不**消費後續空白。
    pub fn parse_integer_alone(&mut self) -> Result<i64, RyError> {
        let bytes = self.rest.as_bytes();
        let mut idx = 0;
        if idx < bytes.len() && bytes[idx] == b'-' {
            idx += 1;
        }
        let digits_start = idx;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        if idx == digits_start {
            return Err(self.err(format!("Invalid integer `{ }'.", self.error_token())));
        }
        let text = &self.rest[..idx];
        let Ok(v) = text.parse::<i64>() else {
            return Err(self.err(format!("Invalid integer `{ }'.", self.error_token())));
        };
        self.rest.replace_range(..idx, "");
        Ok(v)
    }

    /// 字串字面量:`"…"`,轉義 \n \t \ooo(1-3 位八進位)\ " \\;裸換行=
    /// Unterminated;NUL=Null byte。
    pub fn parse_string(&mut self) -> Result<String, RyError> {
        if !self.rest.starts_with('"') {
            return Err(self.err(format!("Expected string, got `{ }'.", self.error_token())));
        }
        let mut str_out = String::new();
        let mut idx = 1;
        loop {
            let bytes = self.rest.as_bytes();
            let start = idx;
            let mut ch: u8;
            loop {
                ch = bytes[idx];
                if ch == b'"' || ch == b'\n' || ch == b'\\' || ch == 0 {
                    break;
                }
                idx += 1;
            }
            str_out.push_str(&self.rest[start..idx]);
            idx += 1;
            if ch == b'"' {
                break;
            }
            if ch == 0 {
                return Err(self.err(format!("Null byte in string literal: `{ }'.", self.rest.trim_end_matches('\n'))));
            }
            if ch == b'\n' {
                return Err(self.err(format!("Unterminated string literal: `{ }'.", self.rest.trim_end_matches('\n'))));
            }
            // 反斜線轉義
            let bytes = self.rest.as_bytes();
            let c2 = bytes[idx];
            idx += 1;
            if c2 == b'n' {
                str_out.push('\n');
            } else if c2 == b't' {
                str_out.push('\t');
            } else if (b'0'..=b'7').contains(&c2) {
                let mut v = (c2 - b'0') as u32;
                let mut probe = idx;
                for _ in 0..2 {
                    if probe < bytes.len() && (b'0'..=b'7').contains(&bytes[probe]) {
                        v = v * 8 + (bytes[probe] - b'0') as u32;
                        probe += 1;
                    } else {
                        break;
                    }
                }
                idx = probe;
                str_out.push(v as u8 as char);
            } else {
                str_out.push(c2 as char);
            }
        }
        self.rest.replace_range(..idx, "");
        self.consume_whitespace_and_comments();
        Ok(str_out)
    }

    /// parse_const(rtlil_frontend.cc:261-322):字串 | 帶符號整數(32-bit
    /// 二補數展開)| `N'[s]bits`。補位:文字最後一個位元的值(S1 補 S0,
    /// 其餘補自身;無位元補 Sx)。
    pub fn parse_const(&mut self) -> Result<Const, RyError> {
        if self.rest.starts_with('"') {
            return Ok(Const::Str(self.parse_string()?));
        }

        let negative_value = self.rest.starts_with('-');
        let width = self.parse_integer_alone()?;
        // Can't test value<0 here(oracle 註:-0 需走整數路徑)。
        if negative_value || !self.rest.starts_with('\'') {
            if width < i32::MIN as i64 || width > i32::MAX as i64 {
                return Err(self.err(format!("Integer {width} out of range before `{ }'.", self.error_token())));
            }
            self.consume_whitespace_and_comments();
            return Ok(Const::from_i64_i32_width(width));
        }

        let mut idx = 1;
        let is_signed = self.rest.as_bytes().get(1) == Some(&b's');
        if is_signed {
            idx += 1;
        }

        let mut bits: Vec<Bit> = Vec::new();
        if width >= WIDTH_LIMIT {
            return Err(self.err(format!("Constant width {width} out of range before `{ }`.", self.error_token())));
        }
        bits.reserve(width.max(0) as usize);
        let bytes = self.rest.as_bytes();
        let start_idx = idx;
        loop {
            let bit = match bytes.get(idx) {
                Some(b'0') => Bit::S0,
                Some(b'1') => Bit::S1,
                Some(b'x') => Bit::Sx,
                Some(b'z') => Bit::Sz,
                Some(b'm') => Bit::Sm,
                Some(b'-') => Bit::Sa,
                _ => break,
            };
            bits.push(bit);
            idx += 1;
        }
        if start_idx < idx {
            bits.reverse();
        }

        if bits.len() as i64 > width {
            bits.truncate(width.max(0) as usize);
        } else if (bits.len() as i64) < width {
            let mut extbit = Bit::Sx;
            if let Some(&last) = bits.last() {
                extbit = if last == Bit::S1 { Bit::S0 } else { last };
            }
            bits.resize(width.max(0) as usize, extbit);
        }

        self.rest.replace_range(..idx, "");
        self.consume_whitespace_and_comments();
        Ok(Const::Bits { width, bits, signed: is_signed, is_unsized: false })
    }
}
