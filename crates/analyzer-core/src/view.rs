//! 統一視圖類型(ANALYZER-SERVICE §4.2):兩輸入形(Sv|Rtlil)共用的
//! 查詢投影。全部欄位=既有 IR 直讀,零新語義(name resolution / elaborate /
//! 表達式求值 = §11 禁區,不由本層承擔)。

use rcs_diag::{SourceMap, Span};

#[derive(Clone, Debug, PartialEq)]
pub struct ModuleInfo {
    pub name: String,
    /// SV:header `#(...)` ParamEntry + 體內 `parameter` 宣告(本弧裁定:
    /// localparam 是常數不是模組介面參數,對齊 RTLIL
    /// avail_parameters/parameter_defaults 語義,不收;§S1 實作記錄)。
    /// RTLIL:avail_parameters + parameter_defaults。
    pub params: Vec<ParamInfo>,
    pub ports: Vec<PortInfo>,
    /// SV:Decl 面(Net/Reg 族,含 unpacked 維數);RTLIL:wires 全量 +
    /// memories(kind="memory")。
    pub nets: Vec<NetInfo>,
    /// SV:Instance(閘原語與模組例化同形,一 InstUnit 一條);
    /// RTLIL:cells。
    pub cells: Vec<CellInfo>,
    /// SV:Always+Initial item 計數;RTLIL:processes 數。
    pub proc_count: usize,
    /// SV:ContAssign 對數;RTLIL:connections 數。
    pub assign_count: usize,
    /// SV:Module.span;RTLIL:恆 None(ry-ir AST 無 span,風險 R1/D10)。
    pub span: Option<SpanInfo>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParamInfo {
    pub name: String,
    /// 字面量形文本(§4.5):NumLit → `W'sdDIGITS`;StrLit → 帶引號;
    /// RealLit → 原文;非字面量 → `<expr>`。RTLIL:Bits → `N'h<hex>`
    /// (含 x/z 等非 01 位 → `N'b<逐位字符>`);Str → 帶引號;
    /// 無預設值參數 → 空串。
    pub value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PortInfo {
    pub name: String,
    pub dir: Dir,
    /// 字面量範圍派生(msb/lsb 直取);參數化寬度 → 0 + parametric;
    /// 真孤兒端口(無任何 dir 證據)→ 0。
    pub width: u32,
    /// 寬度非字面量(§4.2 派生規則:不求值參數,只標記)。
    pub parametric: bool,
    pub signed: bool,
    pub span: Option<SpanInfo>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Input,
    Output,
    Inout,
}

impl Dir {
    pub fn as_str(self) -> &'static str {
        match self {
            Dir::Input => "input",
            Dir::Output => "output",
            Dir::Inout => "inout",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetInfo {
    pub name: String,
    /// 詞表見 §13:SV=DeclKind 既有變體名(Net(x) → x.as_str());
    /// RTLIL:wires 恆 "wire"、memories 恆 "memory"。
    pub kind: NetKindTag,
    pub width: u32,
    pub is_port: bool,
    /// SV 記憶體維數(DeclEntry.unpacked.len());RTLIL 恆 0。
    pub unpacked_dims: u32,
    pub span: Option<SpanInfo>,
}

/// net/宣告種類標籤(§13 詞表的值載體)。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetKindTag(pub String);

impl NetKindTag {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn wire() -> Self {
        NetKindTag("wire".to_string())
    }

    pub(crate) fn memory() -> Self {
        NetKindTag("memory".to_string())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CellInfo {
    pub name: String,
    pub ctype: String,
    /// 連接數(SV:InstUnit.conns;RTLIL:cell.connections.len)。
    pub port_count: usize,
    pub span: Option<SpanInfo>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpanInfo {
    pub file: String,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// Span → SpanInfo 解析(1-based 行列,經既有 pub `SourceMap::line_col`,
/// source_map.rs:72-74——零接觸,禁追加同名存取器)。未註冊文件 → None。
pub(crate) fn span_info(sm: &SourceMap, span: Span) -> Option<SpanInfo> {
    if !sm.has(span.file) {
        return None;
    }
    let (start_line, start_col) = sm.line_col(span.file, span.start);
    let (end_line, end_col) = sm.line_col(span.file, span.end);
    Some(SpanInfo {
        file: sm.file_name(span.file).to_string(),
        start_line,
        start_col,
        end_line,
        end_col,
    })
}
