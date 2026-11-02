// 協議與 Analysis 視圖 TS 型別(ANALYZER-SERVICE §6.2/§13)。
// 視圖層整數一律 number(線路側 Int=bigint,client 解碼時 Number() 轉換;
// 寬度/計數域遠小於 2^53,無精度面)。

export interface Span {
  file: string;
  start_line: number;
  start_col: number;
  end_line: number;
  end_col: number;
}

export interface ParamInfo {
  name: string;
  value: string;
}

export interface PortInfo {
  name: string;
  dir: "input" | "output" | "inout";
  width: number;
  /** 寬度非字面量(參數化)標記(§4.2/§12-R8)。 */
  parametric?: boolean;
  signed: boolean;
  span: Span | null;
}

export interface NetInfo {
  name: string;
  kind: string;
  width: number;
  is_port: boolean;
  unpacked_dims: number;
  span: Span | null;
}

export interface CellInfo {
  name: string;
  ctype: string;
  port_count: number;
  span: Span | null;
}

export interface ModuleInfo {
  name: string;
  params: ParamInfo[];
  ports: PortInfo[];
  nets: NetInfo[];
  cells: CellInfo[];
  proc_count: number;
  assign_count: number;
  span: Span | null;
}

export interface DiagnosticNote {
  message: string;
  span: Span | null;
}

export interface Diagnostic {
  severity: "error" | "warning" | "note";
  code: string;
  message: string;
  span: Span | null;
  notes: DiagnosticNote[];
}

export interface AnalyzeResult {
  has_errors: boolean;
  modules: ModuleInfo[];
  diagnostics: Diagnostic[];
  diags_text: string;
}

export interface AnalyzeRequest {
  kind: "sv" | "rtlil";
  name: string;
  source: string;
  include_dirs?: string[];
  defines?: string[];
}

export interface ServerInfo {
  proto: number;
  server: string;
  version: string;
  ops: string[];
}

/** 錯誤幀(§5.6 字典)對應的異常。 */
export class AspError extends Error {
  readonly code: string;
  constructor(code: string, message: string) {
    super(`[${code}] ${message}`);
    this.code = code;
  }
}
