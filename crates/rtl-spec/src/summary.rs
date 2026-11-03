//! 結構投影摘要(R2-1):`analyze_summary` = analyzer-core Analysis 公開
//! 查詢直讀組合(modules()/instance_edges()/diagnostics),**零新語義**
//! ——欄位=ModuleInfo view.rs 直投,兩輸入形(Sv|Rtlil)同一查詢形。
//! 這是 rtl_analyze(MCP 工具)與 wasm rtl_analyze_{sv,rtlil} 的資料源。

use analyzer_core::SourceInput;

use crate::schema::AnalysisSummary;

pub(crate) fn source_name(input: &SourceInput) -> String {
    match input {
        SourceInput::Sv(i) => i.name.clone(),
        SourceInput::Rtlil(i) => i.name.clone(),
    }
}

pub fn analyze_summary(input: &SourceInput) -> AnalysisSummary {
    let analysis = analyzer_core::analyze(input);
    let modules = analysis
        .modules()
        .iter()
        .map(|m| crate::schema::ModuleSummary {
            name: m.name.clone(),
            params: m
                .params
                .iter()
                .map(|p| (p.name.clone(), p.value.clone()))
                .collect(),
            ports: m
                .ports
                .iter()
                .map(|p| (p.name.clone(), p.dir.as_str().to_string(), p.width, p.signed))
                .collect(),
            nets: m
                .nets
                .iter()
                .map(|n| (n.name.clone(), n.kind.as_str().to_string(), n.width, n.is_port))
                .collect(),
            cells: m
                .cells
                .iter()
                .map(|c| (c.name.clone(), c.ctype.clone(), c.port_count))
                .collect(),
            proc_count: m.proc_count,
            assign_count: m.assign_count,
        })
        .collect();
    AnalysisSummary {
        source: source_name(input),
        has_errors: analysis.has_errors(),
        diagnostics_count: analysis.diagnostics().len(),
        modules,
        instance_edges: analysis.instance_edges(),
    }
}
