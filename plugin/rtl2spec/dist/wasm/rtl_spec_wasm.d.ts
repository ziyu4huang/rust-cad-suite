/* tslint:disable */
/* eslint-disable */

export function rtl_analyze_rtlil(name: string, source: string): string;

export function rtl_analyze_sv(name: string, source: string): string;

export function rtl_extract_rules_rtlil(name: string, source: string): string;

export function rtl_extract_rules_sv(name: string, source: string, max_rules: number): string;

export function rtl_scan_gaps_rtlil(name: string, source: string): string;

export function rtl_scan_gaps_sv(name: string, source: string): string;

/**
 * Package identity (matches plugin manifest `version`).
 */
export function rtl_spec_version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly rtl_analyze_rtlil: (a: number, b: number, c: number, d: number) => [number, number];
    readonly rtl_analyze_sv: (a: number, b: number, c: number, d: number) => [number, number];
    readonly rtl_extract_rules_rtlil: (a: number, b: number, c: number, d: number) => [number, number];
    readonly rtl_extract_rules_sv: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly rtl_scan_gaps_rtlil: (a: number, b: number, c: number, d: number) => [number, number];
    readonly rtl_scan_gaps_sv: (a: number, b: number, c: number, d: number) => [number, number];
    readonly rtl_spec_version: () => [number, number];
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
