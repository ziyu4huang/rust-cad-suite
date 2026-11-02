//! analyzer-core — 統一分析器核心(ANALYZER-SERVICE.md 正典;ADR-0011)。
//!
//! 兩輸入形(Sv|Rtlil)→ [`analyze`] → [`Analysis`] 投影視圖 + 原生 IR
//! 逃逸口。零新語義:SV 側=vl-syntax pp→lex→parse 全管線直呼;RTLIL 側=
//! ry-ir::parse_rtlil 直呼;查詢集=既有 IR 欄位投影,求值/解析=禁區。
//! 10.7 core-first:語義能力全在本 crate;analyzer-server 只是新 face,
//! 未來 wasm/FFI/其它 client 直呼本 crate 零複製。

pub mod proto;
pub mod report;
pub mod rtlil;
pub mod sv;
pub mod view;

pub use rcs_diag;

use std::collections::HashSet;

use view::ModuleInfo;

/// 輸入判別(入口枚舉,§4.1)。
#[derive(Clone, Debug)]
pub enum SourceInput {
    /// Verilog/SystemVerilog 源碼:pp → lex → parse 全管線。
    Sv(SvInput),
    /// RTLIL 文本(oracle 產 -readable 形;ry-ir::parse_rtlil)。
    Rtlil(RtlilInput),
}

#[derive(Clone, Debug, Default)]
pub struct SvInput {
    /// 檔名(診斷渲染/SourceMap 註冊用,可非磁碟路徑)。
    pub name: String,
    pub source: String,
    /// include 目錄與 `-D` 預定義,直通 vl_syntax::pp::PpOptions。
    pub include_dirs: Vec<std::path::PathBuf>,
    pub defines: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct RtlilInput {
    pub name: String,
    pub source: String,
}

/// 統一分析結果:投影視圖 + 原生 IR 逃逸口 + 診斷(§4.1)。
pub struct Analysis {
    pub kind: InputKind,
    pub diagnostics: Vec<rcs_diag::Diagnostic>,
    /// render_diags 產物(帶 caret;零診斷時為空串)。
    pub diag_text: String,
    pub modules: Vec<ModuleInfo>,
    /// 原生逃逸口(跨進程協議只送投影視圖;此二者僅 Rust API 可及)。
    pub sv: Option<vl_syntax::ast::SourceFile>,
    pub rtlil: Option<ry_ir::Design>,
    /// SV 側 SourceMap(診斷渲染與行列查詢);RTLIL 側 None。
    pub(crate) sm: Option<rcs_diag::SourceMap>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputKind {
    Sv,
    Rtlil,
}

/// 唯一入口:分派 + 投影。永不 panic;語法錯誤進 diagnostics 不中止
/// (SV 累積語義;RTLIL 首錯硬錯=單條 error 診斷,§4.3 記錄的不對稱)。
pub fn analyze(input: &SourceInput) -> Analysis {
    match input {
        SourceInput::Sv(i) => sv::analyze_sv(i),
        SourceInput::Rtlil(i) => rtlil::analyze_rtlil(i),
    }
}

impl Analysis {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == rcs_diag::Severity::Error)
    }

    pub fn diagnostics(&self) -> &[rcs_diag::Diagnostic] {
        &self.diagnostics
    }

    pub fn modules(&self) -> &[ModuleInfo] {
        &self.modules
    }

    pub fn module(&self, name: &str) -> Option<&ModuleInfo> {
        self.modules.iter().find(|m| m.name == name)
    }

    /// 跨模組實例邊 `(from_module, to_module)`:目標須命中本輸入的模組名
    /// 集合(閘原語/`$cell` 不成邊;兩側同一查詢形=「一致 API」的直接體現)。
    /// SV:一 InstUnit 一條邊(實例圖的忠實形);RTLIL:一 cell 一條邊。
    /// RTLIL 名字逐字保留(ry-ir 不做 unescape,`\` 前綴在場)。
    pub fn instance_edges(&self) -> Vec<(String, String)> {
        let mut edges = Vec::new();
        if let Some(ast_file) = &self.sv {
            let names: HashSet<&str> =
                ast_file.modules.iter().map(|m| m.name.as_str()).collect();
            for m in &ast_file.modules {
                let c = sv::collect(&m.items);
                for inst in c.instances {
                    if names.contains(inst.mod_name.as_str()) {
                        for _u in &inst.insts {
                            edges.push((m.name.clone(), inst.mod_name.clone()));
                        }
                    }
                }
            }
        } else if let Some(design) = &self.rtlil {
            let names: HashSet<&str> =
                design.modules.iter().map(|m| m.name.as_str()).collect();
            for m in &design.modules {
                for c in &m.cells {
                    if names.contains(c.ctype.as_str()) {
                        edges.push((m.name.clone(), c.ctype.clone()));
                    }
                }
            }
        }
        edges
    }

    /// 行列查詢(SV 側直通既有 pub `rcs_diag::SourceMap::line_col`,
    /// source_map.rs:72-74——零接觸,禁追加同名存取器;RTLIL=Err)。
    /// **file 維度必帶(R2-3)**:pp 的 include/宏/paste 向同一 SourceMap
    /// add 多文件,單 pos 無所屬文件語義;入參=(FileId, byte offset),
    /// 出參=1-based (line, col)。跨進程協議不暴露此查詢(Rust API only)。
    pub fn line_col(&self, file: rcs_diag::FileId, pos: u32) -> Result<(u32, u32), String> {
        match &self.sm {
            Some(sm) if sm.has(file) => Ok(sm.line_col(file, rcs_diag::BytePos(pos))),
            Some(_) => Err(format!("unknown file id {}", file.0)),
            None => Err("no span info".to_string()),
        }
    }
}
