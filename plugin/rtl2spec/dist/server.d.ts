// rtl2spec server.mjs 的型別契約面(手寫;D11——源=手寫 JS 零編譯步,
// 類型以 .d.ts 附注,禁引入 tsc/bun build 步)。

export interface RulebookJson {
  source: string;
  parse_errors: string[];
  skipped: { file: string; reason: 'tb_file' | 'unsupported' }[];
  modules: { module: string; rules: RuleJson[] }[];
  truncated: string[];
}

export interface RuleJson {
  id: string;
  category: 'reset' | 'fsm' | 'handshake' | 'timing' | 'arithmetic' | 'counter' | 'memory' | 'protocol' | 'param';
  module: string;
  statement: string;
  qualifier: 'derived' | 'inferred';
  evidence: EvidenceJson[];
  check: { form: 'guard' | 'transition' | 'sequence' | 'relation' | 'invariant' } & Record<string, unknown>;
  fingerprint: string;
}

export interface EvidenceJson {
  file: string;
  line: number;
  end_line: number;
  class: 'process' | 'assign' | 'case' | 'generate' | 'expr' | 'decl';
  snippet: string;
  path?: string;
}

export interface GapItemJson {
  id: string;
  class: 'latch' | 'incomplete_case' | 'undriven_read' | 'missing_reset' | 'comb_loop' | 'multi_driver' | 'blocking_in_seq';
  module: string;
  confidence: 'certain' | 'heuristic';
  message: string;
  evidence: EvidenceJson[];
  suggested_test: string;
}

export interface GapReportJson {
  source: string;
  parse_errors: string[];
  skipped: { file: string; reason: 'tb_file' | 'unsupported' }[];
  items: GapItemJson[];
}

export interface AnalysisSummaryJson {
  source: string;
  has_errors: boolean;
  diagnostics_count: number;
  modules: {
    name: string;
    params: { name: string; value: string }[];
    ports: { name: string; dir: string; width: number; signed: boolean }[];
    nets: { name: string; kind: string; width: number; is_port: boolean }[];
    cells: { name: string; ctype: string; port_count: number }[];
    proc_count: number;
    assign_count: number;
  }[];
  instance_edges: [string, string][];
}

/** rtl_spec_wasm.js glue 的顯式位元組 init(web 目標;§5 策略)。 */
export default function init(input: { module_or_path: Uint8Array }): Promise<InitOutput>;

export interface InitOutput {
  rtl_spec_version(): string;
  rtl_analyze_sv(name: string, source: string): string;
  rtl_extract_rules_sv(name: string, source: string, max_rules: number): string;
  rtl_scan_gaps_sv(name: string, source: string): string;
  rtl_analyze_rtlil(name: string, source: string): string;
  rtl_extract_rules_rtlil(name: string, source: string): string;
  rtl_scan_gaps_rtlil(name: string, source: string): string;
}
