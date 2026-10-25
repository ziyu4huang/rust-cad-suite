# rverilator.wasm playground — DSH bundle

The repo's browser playground (`crates/vl-wasm/wasm-demo`) delivered as a
persistent DeepSeek Harness plugin: a **Verilog 模擬台 / Verilog Playground**
page inside the Harness Web UI.

It is not the standalone page in an iframe. The panel is a React component
registered in the Harness `main` slot (reached from its own icon in
`sidebar.panellist`), so it inherits the host theme tokens, light/dark
switching, locale, and layout instead of living in a separate document.

## What it does

| Area | Content |
|---|---|
| Playground | Verilog source, optional `top`, `$readmem` files (JSON), engine selector, stdout, VCD waveform |
| Engines | `rverilator` → `WasmSim` (vl-sim) · `riverilog` → `riv_build` + `riv_run` (riv-elab + riv-kernel), the same two-step CLI contract, RIV-1 byte count shown |
| Waveforms | `vcd_parse` → `to_svg` from the shared Rust `vl-vcd` renderer (ADR-0005: no canvas), signal toggles, deterministic SVG |
| XLS bridge | Per harvested XLS sample: the provenance chain (DSLX source → XLS IR → normalized Verilog → race-safe TB) in four panes, then run in wasm and byte-diff stdout against the Verilator-recorded expectation. `force_dump` supplies the waveform because XLS TBs carry no `$dumpvars` |
| Four-faces note | Under the XLS cards: the core's four faces (rverilator CLI · wasm · Bun FFI · `rv-xls-sim`) and why this panel is the zero-install review face while XLS itself calls the adapter |
| Verification card | `51_vcd_counter` (reference case) runs in wasm and byte-diffs against the recorded expectation |
| Spec card | `i2c_spec_check` lockstep: pick a surgically broken RTL variant and read the named protocol violations |

The XLS bridge is the first slice of the "share one core with XLS" plan:
`docs/plan/xls-shared-core.html` (why XLS needs no Verilator, the interface
contract, S1 done / S2 adapter / S3 upstream subclass).

## How the two halves fit

- **Host** (`index.js`) registers one prefix route on the Harness web carrier:
  - `/rverilator-wasm/pkg/*` → `crates/vl-wasm/pkg` (the wasm-bindgen artifact,
    rebuilt by `scripts/regress.sh` via `wasm-pack build crates/vl-wasm --target web`)
  - `/rverilator-wasm/corpus/e2e/<case>/<file>` → an **allowlisted** slice of
    `corpus/e2e` (counter + the two XLS e2e cases only)
  - `/rverilator-wasm/xls/manifest.json` → scanned live from
    `corpus-external/xls` + the matching e2e case (design stem → case dir);
    adding a harvested sample is data, not client code
  - `/rverilator-wasm/xls/dslx/<id>.x`, `/rverilator-wasm/xls/generated/<id>.{v,ir}`
    → the XLS snapshot (extension-checked, traversal-refused), plus the
    Apache-2.0 licence/notice files
  - `Cache-Control: no-store` everywhere (repo rule: local preview servers must
    not let a heuristic cache serve a stale wasm bundle), single-segment
    filenames only, no traversal, `GET`/`HEAD` only.
- **Client** (`client.js`) loads the artifact lazily
  (`import('/rverilator-wasm/pkg/vl_wasm.js')`), renders the panel, and
  registers the `main` key + the `sidebar.panellist` id that addresses it. All
  visible text goes through the Harness locale service (`rverilatorWasm` ns,
  registered as `en` + `zh` — the shipped built-in ids — plus `zh-CN`/`zh-TW`
  aliases for language packs).

The panel reads no repo files directly; the Host route is the only bridge, and
it cannot become a repo-wide file reader.

## Install

```
plugin_manager install_bundle /Users/huangziyu/proj/rust-cad-suite/dsh-plugins/rverilator-wasm-playground
```

The bundle is linked (not copied), so the panel always serves the current
`crates/vl-wasm/pkg` build. After editing `client.js`, refresh the page; the
client bundle is served fresh. After editing `index.js`, the Host module is
cached per generation: toggle the row
(`plugin_manager set_plugin include:rverilator-wasm-playground enabled false|true`)
or restart the app to mount the new code.

## Test

```
# smoke: Host route + registration + React render + real wasm clicks
mkdir -p /tmp/plugintest && cd /tmp/plugintest
npm install react@19 react-dom@19 jsdom@27
cd /Users/huangziyu/proj/rust-cad-suite/dsh-plugins/rverilator-wasm-playground
NODE_PATH=/tmp/plugintest/node_modules node test/smoke.mjs
```

`test/smoke.mjs` covers: asset MIME/bytes/no-store, the corpus allowlist and
traversal refusal, the XLS manifest scan (DSLX/IR/Verilog joined to the e2e case,
`forceDump`), the XLS asset allowlist (extension + traversal), method refusal,
the carrier's prefix-match rule (no trailing slash — the bug this bundle first
shipped with), the `main`/`panellist` registrations, the locale dictionaries, SSR
markup, and then — in jsdom, with the **real** wasm artifact — Run (`q=8`), the
riverilog face (`$finish called at 76000`), both XLS provenance panes and an XLS
byte-exact run (`add3=6`, forced-dump SVG), a byte-exact corpus card, and a named
spec violation.

## Verified vs not verified

Verified in the running Desktop app (2026-10-26, screenshots in
`docs/e2e-report/dsh-xls-bridge-*.png`), by real HID clicks through the Host
route:

| Step | Result |
|---|---|
| sidebar row → panel | opens as the `main` panel, no crash, host theme applied |
| Run (rverilator face) | stdout `q=8`, status `done`, SVG waveform with `clk` + `q`, signal toggles |
| engine → riverilog | `RIV-1 bytes: 1094`, stdout carries the iverilog-face banner `$finish called at 76000 (1ps)` + `VCD info:` line |
| XLS add3 card | four provenance panes loaded; `✓` byte-exact; stdout `add3=6` / `add3=355`; waveform from the forced dump |
| XLS acc_pipe card | `✓` byte-exact; stdout `pipe=24` / `pipe=780`; pipeline waveform with time axis |
| four-faces note | renders under the XLS cards, naming `rv-xls-sim` as the face XLS calls |
| verification card | `✓ byte-exact against the recorded expectation` (corpus fetched through the Host route) |

Mechanically: `test/smoke.mjs` — 66 checks, ALL PASS (asset MIME/bytes/no-store,
corpus allowlist, traversal, method, the carrier prefix-match rule, XLS manifest
and asset surface, `main`/`panellist` registrations, four locale registrations,
SSR markup, and in jsdom with the real wasm artifact: Run `q=8`, riv face banner,
XLS four-pane card + byte-exact + the four-faces note, byte-exact corpus badge, named spec violation).
Live probes: the asset route answers `200 text/javascript`,
`200 application/wasm` (the 1.2 MB artifact), `200` allowlisted corpus,
`200` XLS manifest/DSLX/IR, `403` non-allowlisted/traversal/extension.

Locale note (one real trap): `@deepseek-ai/dsh-client-locale` ships exactly two
built-in ids, `en` and `zh`. Registering only `zh-CN`/`zh-TW` compiles, runs, and
silently falls back to English while the UI around it is visibly Chinese — the
first version of this plugin did exactly that. Register under `zh`. A client
bundle edit then reaches the page on the next reload (revision is derived from
the file's mtime/size): no app restart is needed for the Client half, unlike the
Host half (`index.js`), whose module is cached per generation until the row is
toggled or the app restarts.
