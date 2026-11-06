//! N5 信號-聲明完備性 lint(§5.2 S1;審讀 #6 可執行形)。對每檔計算
//! A(規則書全部 evidence/check 行引用的標識符集)∖ B(原檔宣告集)——
//! 非空即報(規則書引用了原檔不存在的信號=提取器 bug)。防回歸閘:結果
//! 隨提取器輸出(rulebook JSON `decl_lint`)出場,隨 golden 74 檔每輪重算,
//! 不另立閘;基線預期全綠(evidence 攜源碼原文行,行內標識符天然有聲明)。
//!
//! A 采集面:evidence.snippet(去註釋/字串/屬性)+ check 自由文本欄位
//! (Check::referenced_texts);RTLIL 證形(path)與 statement 模板不掃
//! ——前者非「行」(D2 路徑證形),後者與 lhs/rhs 同源冗餘。
//! 詞法契約:識別符=[A-Za-z_][A-Za-z0-9_$]*;前綴 `$`(系統任務)/
//! `'`(sized 字面量基字母)/`` ` ``(巨集引用)的識別符跳過——巨集不是
//! 原檔宣告面;IEEE 1364-2005 關鍵字濾除;RTLIL 跳脫名 `\x` 歸一為 x
//! (B 側同律,見 assemble_decl_b_set)。
//! B 采集面=呼叫端組裝(sv/rtlil 各自把投影宣告集+檔內具名宣告彙入),
//! 本模組只提供詞法與集合差;dangling 序= BTreeSet 迭代序(字典序,
//! 確定性契約,禁 HashSet 序——F1a 教訓)。

use std::collections::BTreeSet;

use crate::schema::Rule;

/// IEEE 1364-2005 關鍵字全表(詞法濾除;不含 SystemVerilog-only 詞)。
const KEYWORDS: &[&str] = &[
    "always", "and", "assign", "automatic", "begin", "buf", "bufif0", "bufif1", "case", "casex",
    "casez", "cell", "cmos", "config", "deassign", "default", "defparam", "design", "disable",
    "edge", "else", "end", "endcase", "endfunction", "endgenerate", "endmodule", "endprimitive",
    "endspecify", "endtable", "endtask", "event", "for", "force", "forever", "fork", "function",
    "generate", "genvar", "highz0", "highz1", "if", "ifnone", "incdir", "include", "initial",
    "inout", "input", "instance", "integer", "join", "large", "liblist", "library", "localparam",
    "macromodule", "medium", "module", "nand", "negedge", "nmos", "nor", "noshowcancelled", "not",
    "notif0", "notif1", "or", "output", "parameter", "pmos", "posedge", "primitive", "pull0",
    "pull1", "pulldown", "pullup", "pulsestyle_onevent", "pulsestyle_ondetect", "rcmos", "real",
    "realtime", "reg", "release", "repeat", "rnmos", "rpmos", "rtran", "rtranif0", "rtranif1",
    "scalared", "showcancelled", "signed", "small", "specify", "specparam", "strong0", "strong1",
    "supply0", "supply1", "table", "task", "time", "tran", "tranif0", "tranif1", "tri", "tri0",
    "tri1", "triand", "trior", "trireg", "unsigned", "use", "uwire", "vectored", "wait", "wand",
    "weak0", "weak1", "while", "wire", "wor", "xnor", "xor",
];

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

/// 去註釋(`//` 行註釋與 `/* */` 塊註釋)/字串("…",轉義感知)/屬性實例
/// (`(* … *)`——屬性鍵如 keep 非宣告面);其餘原文保留(空白規整無契約,
/// 後續只做詞法採集)。
pub(crate) fn strip_comments_strings_attrs(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let b: Vec<char> = src.chars().collect();
    let n = b.len();
    let mut i = 0usize;
    while i < n {
        if b[i] == '"' {
            // 字串:跳過內容(轉義感知),不落盤。
            i += 1;
            while i < n && b[i] != '"' {
                if b[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(n);
            out.push(' ');
        } else if b[i] == '/' && i + 1 < n && b[i + 1] == '/' {
            // 行註釋:吃到行尾(不含換行本身)。
            while i < n && b[i] != '\n' {
                i += 1;
            }
        } else if b[i] == '/' && i + 1 < n && b[i + 1] == '*' {
            // 塊註釋。
            i += 2;
            while i + 1 < n && !(b[i] == '*' && b[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(n);
            out.push(' ');
        } else if b[i] == '(' && i + 1 < n && b[i + 1] == '*' {
            // 屬性實例((* k = v *));內含字串粗剝即可(屬性鍵值不進 A)。
            i += 2;
            while i + 1 < n && !(b[i] == '*' && b[i + 1] == ')') {
                i += 1;
            }
            i = (i + 2).min(n);
            out.push(' ');
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

/// 識別符採集(詞法契約見模組註釋;RTLIL `\x` 的 `\` 非跳過前綴,x 進集,
/// 與 B 側歸一律對齊)。
pub(crate) fn collect_idents(text: &str, out: &mut BTreeSet<String>) {
    let b: Vec<char> = text.chars().collect();
    let n = b.len();
    let mut i = 0usize;
    while i < n {
        if is_ident_start(b[i]) {
            if i > 0 && (b[i - 1] == '$' || b[i - 1] == '\'' || b[i - 1] == '`') {
                // $x / 'x(sized 字面量基字母)/ `x(巨集引用):跳過整詞。
                while i < n && is_ident_char(b[i]) {
                    i += 1;
                }
                continue;
            }
            let start = i;
            while i < n && is_ident_char(b[i]) {
                i += 1;
            }
            let word: String = b[start..i].iter().collect();
            if !KEYWORDS.contains(&word.as_str()) {
                out.insert(word);
            }
        } else {
            i += 1;
        }
    }
}

/// 規則書引用標識符集 A:evidence.snippet(去註釋/字串/屬性)+ check 自由
/// 文本欄位(RTLIL path 證形與 statement 模板不掃,見模組註釋)。
pub(crate) fn rulebook_ref_idents<'a>(rules: impl IntoIterator<Item = &'a Rule>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for r in rules {
        for e in &r.evidence {
            collect_idents(&strip_comments_strings_attrs(&e.snippet), &mut out);
        }
        for t in r.check.referenced_texts() {
            collect_idents(t, &mut out);
        }
    }
    out
}

/// 名字入 B 集(RTLIL 跳脫名歸一律:剝前綴 `\` 後雙形齊入,與 A 側詞法對齊)。
pub(crate) fn insert_decl_name(out: &mut BTreeSet<String>, name: &str) {
    out.insert(name.to_string());
    if let Some(stripped) = name.strip_prefix('\\') {
        out.insert(stripped.to_string());
    }
}

/// A∖B(字典序;空差=lint 綠)。
pub(crate) fn diff_sets(a: &BTreeSet<String>, b: &BTreeSet<String>) -> Vec<String> {
    a.difference(b).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_removes_comments_strings_attrs() {
        let s = "assign a = b; // tail note c1\n/* block x */ assign c = \"str d\"; (* keep = \"true\" *) e = f;";
        let stripped = strip_comments_strings_attrs(s);
        let mut ids = BTreeSet::new();
        collect_idents(&stripped, &mut ids);
        for noise in ["c1", "block", "x", "d", "keep", "true"] {
            assert!(!ids.contains(noise), "noise {noise} leaked: {ids:?}");
        }
        for want in ["a", "b", "c", "e", "f"] {
            assert!(ids.contains(want), "ident {want} missing: {ids:?}");
        }
        // 關鍵字(assign)正確濾除。
        assert!(!ids.contains("assign"), "keyword leaked: {ids:?}");
    }

    #[test]
    fn literal_base_and_macro_and_sys_prefixes_skipped() {
        let mut ids = BTreeSet::new();
        collect_idents("cnt <= 16'd1; y = 2'b01 + 'sd7; `$M; $fdisplay(w); `WIDTH", &mut ids);
        for noise in ["d1", "b01", "sd7", "M", "fdisplay", "WIDTH"] {
            assert!(!ids.contains(noise), "noise {noise} leaked: {ids:?}");
        }
        assert!(ids.contains("cnt"), "{ids:?}");
        assert!(ids.contains("w"), "{ids:?}");
    }

    #[test]
    fn keywords_filtered_and_rtlil_escape_normalized() {
        let mut ids = BTreeSet::new();
        collect_idents("posedge clk begin state endcase", &mut ids);
        assert_eq!(ids, BTreeSet::from(["clk".to_string(), "state".to_string()]));
        let mut b = BTreeSet::new();
        insert_decl_name(&mut b, "\\state");
        assert!(b.contains("state") && b.contains("\\state"));
    }
}
