/* tslint:disable */
/* eslint-disable */

export function rtl_analyze_rtlil(name: string, source: string): string;

export function rtl_analyze_sv(name: string, source: string): string;

export function rtl_extract_rules_rtlil(name: string, source: string): string;

export function rtl_extract_rules_sv(name: string, source: string, max_rules: number): string;

export function rtl_scan_gaps_rtlil(name: string, source: string): string;

export function rtl_scan_gaps_sv(name: string, source: string): string;

/**
 * spec 七查 lint 面(D-S3):findings 緊湊字串陣列(與 CLI `spec lint`
 * stdout byte-equal——run-spec-cli-parity spec 腿判據)。
 */
export function rtl_spec_spec_lint(name: string, source: string): string;

/**
 * spec 檔解析面(D-S3):name 沿導出形(解析零名消費;SpecDoc.name 自檔
 * 頭 `# spec:`);回傳=spec_json;Err(現無產出點)→ `{"error":…}` 幀。
 */
export function rtl_spec_spec_parse(name: string, source: string): string;

/**
 * Package identity (matches plugin manifest `version`).
 */
export function rtl_spec_version(): string;

/**
 * tb_plan rtlil 面(RV2-8:恆 TB1 錯誤幀;硬錯文案由核心產生,本面零語義)。
 */
export function rtl_tb_plan_rtlil(name: string, source: string, module: string, seed: number, cycles: number, flush: number): string;

/**
 * tb_plan sv 面(RV1-11/RV2-8):`module==""`=未指定(單模組自動選取);
 * seed/cycles/flush==0=核心默認(TbOpts::resolve,與 max_rules==0 同約定);
 * 回傳幀=`{"ok":true,"plan":{…},"template_v":"…"}` 或
 * `{"ok":false,"error":"…(milestone TB1)"}`(錯誤走資料,不 trap)。
 */
export function rtl_tb_plan_sv(name: string, source: string, module: string, seed: number, cycles: number, flush: number): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly rtl_analyze_rtlil: (a: number, b: number, c: number, d: number) => [number, number];
    readonly rtl_analyze_sv: (a: number, b: number, c: number, d: number) => [number, number];
    readonly rtl_extract_rules_rtlil: (a: number, b: number, c: number, d: number) => [number, number];
    readonly rtl_extract_rules_sv: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly rtl_scan_gaps_rtlil: (a: number, b: number, c: number, d: number) => [number, number];
    readonly rtl_scan_gaps_sv: (a: number, b: number, c: number, d: number) => [number, number];
    readonly rtl_spec_spec_lint: (a: number, b: number, c: number, d: number) => [number, number];
    readonly rtl_spec_spec_parse: (a: number, b: number, c: number, d: number) => [number, number];
    readonly rtl_spec_version: () => [number, number];
    readonly rtl_tb_plan_rtlil: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => [number, number];
    readonly rtl_tb_plan_sv: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => [number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
