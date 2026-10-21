//! Token model shared by the lexer, preprocessor and parser.

use rcs_diag::Span;

/// Verilog-2005 reserved words (IEEE 1364-2005 Annex B).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[allow(non_camel_case_types)]
pub enum Kw {
    Always, And, Assign, Automatic, Begin, Buf, Bufif0, Bufif1, Case, Casex, Casez,
    Cell, Cmos, Config, Deassign, Default, Defparam, Design, Disable, Edge, Else,
    End, Endcase, Endconfig, Endfunction, Endgenerate, Endmodule, Endprimitive,
    Endspecify, Endtable, Endtask, Event, For, Force, Forever, Fork, Function,
    Generate, Genvar, Highz0, Highz1, If, Ifnone, Incdir, Include, Initial, Inout,
    Input, Instance, Integer, Join, Large, Liblist, Library, Localparam,
    Macromodule, Medium, Module, Nand, Negedge, Nmos, Nor, Noshowcancelled, Not,
    Notif0, Notif1, Or, Output, Parameter, Pmos, Posedge, Primitive, Pull0, Pull1,
    Pulldown, Pullup, Pulsestyle_onevent, Pulsestyle_ondetect, Rcmos, Real,
    Realtime, Reg, Release, Repeat, Rnmos, Rpmos, Rtran, Rtranif0, Rtranif1,
    Scalared, Showcancelled, Signed, Small, Specify, Specparam, Strong0, Strong1,
    Supply0, Supply1, Table, Task, Time, Tran, Tranif0, Tranif1, Tri, Tri0, Tri1,
    Triand, Trior, Trireg, Unsigned, Use, Uwire, Vectored, Wait, Wand, Weak0,
    Weak1, While, Wire, Wor, Xnor, Xor,
}

/// Sorted by spelling so lookup can binary-search.
const KEYWORDS: &[(&str, Kw)] = &[
    ("always", Kw::Always), ("and", Kw::And), ("assign", Kw::Assign),
    ("automatic", Kw::Automatic), ("begin", Kw::Begin), ("buf", Kw::Buf),
    ("bufif0", Kw::Bufif0), ("bufif1", Kw::Bufif1), ("case", Kw::Case),
    ("casex", Kw::Casex), ("casez", Kw::Casez), ("cell", Kw::Cell), ("cmos", Kw::Cmos),
    ("config", Kw::Config), ("deassign", Kw::Deassign), ("default", Kw::Default),
    ("defparam", Kw::Defparam), ("design", Kw::Design), ("disable", Kw::Disable),
    ("edge", Kw::Edge), ("else", Kw::Else), ("end", Kw::End), ("endcase", Kw::Endcase),
    ("endconfig", Kw::Endconfig), ("endfunction", Kw::Endfunction),
    ("endgenerate", Kw::Endgenerate), ("endmodule", Kw::Endmodule),
    ("endprimitive", Kw::Endprimitive), ("endspecify", Kw::Endspecify),
    ("endtable", Kw::Endtable), ("endtask", Kw::Endtask), ("event", Kw::Event),
    ("for", Kw::For), ("force", Kw::Force), ("forever", Kw::Forever), ("fork", Kw::Fork),
    ("function", Kw::Function), ("generate", Kw::Generate), ("genvar", Kw::Genvar),
    ("highz0", Kw::Highz0), ("highz1", Kw::Highz1), ("if", Kw::If), ("ifnone", Kw::Ifnone),
    ("incdir", Kw::Incdir), ("include", Kw::Include), ("initial", Kw::Initial),
    ("inout", Kw::Inout), ("input", Kw::Input), ("instance", Kw::Instance),
    ("integer", Kw::Integer), ("join", Kw::Join), ("large", Kw::Large),
    ("liblist", Kw::Liblist), ("library", Kw::Library), ("localparam", Kw::Localparam),
    ("macromodule", Kw::Macromodule), ("medium", Kw::Medium), ("module", Kw::Module),
    ("nand", Kw::Nand), ("negedge", Kw::Negedge), ("nmos", Kw::Nmos), ("nor", Kw::Nor),
    ("noshowcancelled", Kw::Noshowcancelled), ("not", Kw::Not), ("notif0", Kw::Notif0),
    ("notif1", Kw::Notif1), ("or", Kw::Or), ("output", Kw::Output),
    ("parameter", Kw::Parameter), ("pmos", Kw::Pmos), ("posedge", Kw::Posedge),
    ("primitive", Kw::Primitive), ("pull0", Kw::Pull0), ("pull1", Kw::Pull1),
    ("pulldown", Kw::Pulldown), ("pullup", Kw::Pullup),
    ("pulsestyle_ondetect", Kw::Pulsestyle_ondetect),
    ("pulsestyle_onevent", Kw::Pulsestyle_onevent), ("rcmos", Kw::Rcmos),
    ("real", Kw::Real), ("realtime", Kw::Realtime), ("reg", Kw::Reg),
    ("release", Kw::Release), ("repeat", Kw::Repeat), ("rnmos", Kw::Rnmos),
    ("rpmos", Kw::Rpmos), ("rtran", Kw::Rtran), ("rtranif0", Kw::Rtranif0),
    ("rtranif1", Kw::Rtranif1), ("scalared", Kw::Scalared),
    ("showcancelled", Kw::Showcancelled), ("signed", Kw::Signed), ("small", Kw::Small),
    ("specify", Kw::Specify), ("specparam", Kw::Specparam), ("strong0", Kw::Strong0),
    ("strong1", Kw::Strong1), ("supply0", Kw::Supply0), ("supply1", Kw::Supply1),
    ("table", Kw::Table), ("task", Kw::Task), ("time", Kw::Time), ("tran", Kw::Tran),
    ("tranif0", Kw::Tranif0), ("tranif1", Kw::Tranif1), ("tri", Kw::Tri),
    ("tri0", Kw::Tri0), ("tri1", Kw::Tri1), ("triand", Kw::Triand), ("trior", Kw::Trior),
    ("trireg", Kw::Trireg), ("unsigned", Kw::Unsigned), ("use", Kw::Use),
    ("uwire", Kw::Uwire), ("vectored", Kw::Vectored), ("wait", Kw::Wait),
    ("wand", Kw::Wand), ("weak0", Kw::Weak0), ("weak1", Kw::Weak1), ("while", Kw::While),
    ("wire", Kw::Wire), ("wor", Kw::Wor), ("xnor", Kw::Xnor), ("xor", Kw::Xor),
];

impl Kw {
    pub fn from_str(s: &str) -> Option<Kw> {
        KEYWORDS
            .binary_search_by(|(k, _)| k.cmp(&s))
            .ok()
            .map(|i| KEYWORDS[i].1)
    }

    pub fn as_str(self) -> &'static str {
        KEYWORDS
            .iter()
            .find(|(_, k)| *k == self)
            .map(|(s, _)| *s)
            .unwrap_or("<kw>")
    }

    /// Net types (`wire`, `tri`, `wand`, ...).
    pub fn is_net_type(self) -> bool {
        matches!(
            self,
            Kw::Wire | Kw::Tri | Kw::Tri0 | Kw::Tri1 | Kw::Triand | Kw::Trior | Kw::Trireg
                | Kw::Wand | Kw::Wor | Kw::Supply0 | Kw::Supply1 | Kw::Uwire
        )
    }

    /// Gate primitive names usable as instance "module names".
    pub fn is_gate(self) -> bool {
        matches!(
            self,
            Kw::And | Kw::Nand | Kw::Or | Kw::Nor | Kw::Xor | Kw::Xnor | Kw::Buf | Kw::Not
                | Kw::Bufif0 | Kw::Bufif1 | Kw::Notif0 | Kw::Notif1
                | Kw::Nmos | Kw::Pmos | Kw::Cmos | Kw::Rnmos | Kw::Rpmos | Kw::Rcmos
                | Kw::Tran | Kw::Tranif0 | Kw::Tranif1 | Kw::Rtran | Kw::Rtranif0 | Kw::Rtranif1
                | Kw::Pullup | Kw::Pulldown
        )
    }
}

/// Operators and punctuation.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Punct {
    LParen, RParen, LBracket, RBracket, LBrace, RBrace,
    Semi, Comma, Dot, Colon, Question, At, Hash,
    Plus, Minus, Star, Slash, Percent, Pow,
    Amp, Pipe, Caret, Tilde, Bang,
    Lt, Gt, Eq, Le, Ge, EqEq, NotEq, EqEqEq, NotEqEq,
    LogAnd, LogOr, Shl, Shr, AShl, AShr,
    Nand, Nor, Xnor,        // ~&  ~|  ~^ (and ^~)
    PlusColon, MinusColon,  // +:  -:
    AttrOpen, AttrClose,    // (*  *)
    Arrow,                  // ->
}

impl Punct {
    pub fn as_str(self) -> &'static str {
        use Punct::*;
        match self {
            LParen => "(", RParen => ")", LBracket => "[", RBracket => "]",
            LBrace => "{", RBrace => "}", Semi => ";", Comma => ",", Dot => ".",
            Colon => ":", Question => "?", At => "@", Hash => "#",
            Plus => "+", Minus => "-", Star => "*", Slash => "/", Percent => "%", Pow => "**",
            Amp => "&", Pipe => "|", Caret => "^", Tilde => "~", Bang => "!",
            Lt => "<", Gt => ">", Eq => "=", Le => "<=", Ge => ">=", EqEq => "==",
            NotEq => "!=", EqEqEq => "===", NotEqEq => "!==",
            LogAnd => "&&", LogOr => "||", Shl => "<<", Shr => ">>", AShl => "<<<", AShr => ">>>",
            Nand => "~&", Nor => "~|", Xnor => "~^",
            PlusColon => "+:", MinusColon => "-:",
            AttrOpen => "(*", AttrClose => "*)", Arrow => "->",
        }
    }
}

/// Compiler directives recognized by the lexer (`` `name ``). Anything else
/// after a backtick is a [`Tok::MacroId`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum DirKind {
    Define, Undef, Ifdef, Ifndef, Elsif, Else, Endif, Include,
    Timescale, Resetall, DefaultNettype, Line, Celldefine, Endcelldefine,
    UnconnectedDrive, NounconnectedDrive, Pragma, BeginKeywords, EndKeywords,
}

impl DirKind {
    pub fn from_str(s: &str) -> Option<DirKind> {
        Some(match s {
            "define" => DirKind::Define,
            "undef" => DirKind::Undef,
            "ifdef" => DirKind::Ifdef,
            "ifndef" => DirKind::Ifndef,
            "elsif" => DirKind::Elsif,
            "else" => DirKind::Else,
            "endif" => DirKind::Endif,
            "include" => DirKind::Include,
            "timescale" => DirKind::Timescale,
            "resetall" => DirKind::Resetall,
            "default_nettype" => DirKind::DefaultNettype,
            "line" => DirKind::Line,
            "celldefine" => DirKind::Celldefine,
            "endcelldefine" => DirKind::Endcelldefine,
            "unconnected_drive" => DirKind::UnconnectedDrive,
            "nounconnected_drive" => DirKind::NounconnectedDrive,
            "pragma" => DirKind::Pragma,
            "begin_keywords" => DirKind::BeginKeywords,
            "end_keywords" => DirKind::EndKeywords,
            _ => return None,
        })
    }

    /// Directives whose argument is the rest of the line (joined over `\`
    /// continuations). The others take no argument.
    pub fn takes_line_arg(self) -> bool {
        !matches!(
            self,
            DirKind::Else
                | DirKind::Endif
                | DirKind::Resetall
                | DirKind::Celldefine
                | DirKind::Endcelldefine
                | DirKind::NounconnectedDrive
                | DirKind::EndKeywords
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Tok {
    /// Simple or escaped identifier; `text` = name (backslash stripped).
    Ident,
    /// `$display` etc.; `text` includes the `$`.
    SysIdent,
    Keyword(Kw),
    /// Unsized decimal integer; `text` = digits, underscores removed.
    IntLit,
    /// Real literal; `text` = source spelling, underscores removed.
    RealLit,
    /// String literal; `text` = decoded contents (quotes and escapes resolved).
    StrLit,
    /// Based literal, optionally sized/signed; `text` normalized like `8'sh3f`.
    BasedLit,
    Punct(Punct),
    /// Compiler directive; `text` = raw argument (rest of line), may be empty.
    Directive(DirKind),
    /// `` `NAME `` macro use (or macro parameter inside a body); `text` = NAME.
    MacroId,
    /// `` `" `` inside a macro body.
    MacroQuote,
    /// `` ` `` token-paste inside a macro body.
    MacroPaste,
    /// A character with no Verilog meaning; a lexer diagnostic was emitted.
    Error,
    Eof,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Token {
    pub kind: Tok,
    pub span: Span,
    pub text: String,
}

impl Token {
    pub fn new(kind: Tok, span: Span, text: impl Into<String>) -> Self {
        Token {
            kind,
            span,
            text: text.into(),
        }
    }

    pub fn is_punct(&self, p: Punct) -> bool {
        self.kind == Tok::Punct(p)
    }

    pub fn is_kw(&self, k: Kw) -> bool {
        self.kind == Tok::Keyword(k)
    }

    /// Human-readable description for diagnostics.
    pub fn describe(&self) -> String {
        match self.kind {
            Tok::Ident => format!("identifier `{}`", self.text),
            Tok::SysIdent => format!("`{}`", self.text),
            Tok::Keyword(k) => format!("keyword `{}`", k.as_str()),
            Tok::IntLit | Tok::RealLit | Tok::BasedLit => format!("number `{}`", self.text),
            Tok::StrLit => "string literal".to_string(),
            Tok::Punct(p) => format!("`{}`", p.as_str()),
            Tok::Directive(_) => format!("directive `{}`", self.text),
            Tok::MacroId => format!("macro `{}`", self.text),
            Tok::MacroQuote => "`\"".to_string(),
            Tok::MacroPaste => "``".to_string(),
            Tok::Error => "invalid character".to_string(),
            Tok::Eof => "end of file".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_table_is_sorted_and_complete() {
        for w in KEYWORDS.windows(2) {
            assert!(w[0].0 < w[1].0, "KEYWORDS not sorted at {:?}", w[1].0);
        }
        // IEEE 1364-2005 Annex B lists 124 reserved words.
        assert_eq!(KEYWORDS.len(), 124);
        assert_eq!(Kw::from_str("endmodule"), Some(Kw::Endmodule));
        assert_eq!(Kw::from_str("Endmodule"), None, "keywords are case-sensitive");
        assert_eq!(Kw::Pulsestyle_onevent.as_str(), "pulsestyle_onevent");
    }
}
