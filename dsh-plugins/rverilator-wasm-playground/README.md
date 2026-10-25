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
| Verification cards | `51_vcd_counter`, `57_xls_add3`, `58_xls_acc_pipe` run in wasm and byte-diffed against the Verilator-recorded `expected.stdout` |
| Spec card | `i2c_spec_check` lockstep: pick a surgically broken RTL variant and read the named protocol violations |

## How the two halves fit

- **Host** (`index.js`) registers one prefix route on the Harness web carrier:
  - `/rverilator-wasm/pkg/*` → `crates/vl-wasm/pkg` (the wasm-bindgen artifact,
    rebuilt by `scripts/regress.sh` via `wasm-pack build crates/vl-wasm --target web`)
  - `/rverilator-wasm/corpus/e2e/<case>/<file>` → an **allowlisted** slice of
    `corpus/e2e` (the three cases above only)
  - `Cache-Control: no-store` everywhere (repo rule: local preview servers must
    not let a heuristic cache serve a stale wasm bundle), single-segment
    filenames only, no traversal, `GET`/`HEAD` only.
- **Client** (`client.js`) loads the artifact lazily
  (`import('/rverilator-wasm/pkg/vl_wasm.js')`), renders the panel, and
  registers the `main` key + the `sidebar.panellist` id that addresses it. All
  visible text goes through the Harness locale service (`rverilatorWasm` ns, EN
  and zh-TW).

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
traversal refusal, method refusal, the carrier's prefix-match rule (no trailing
slash — the bug this bundle first shipped with), the `main`/`panellist`
registrations, both locale dictionaries, SSR markup, and then — in jsdom, with
the **real** wasm artifact — Run (`q=8`), the riverilog face
(`$finish called at 76000`), a byte-exact corpus card, and a named spec
violation.

## Verified vs not verified

Verified in the running Desktop app (2026-10-25, screenshots in
`docs/e2e-report/dsh-playground-*.png`), by real HID clicks:

| Step | Result |
|---|---|
| sidebar row → panel | opens as the `main` panel, no crash, host theme applied |
| Run (rverilator face) | stdout `q=8`, status `done`, SVG waveform with `clk` + `q`, signal toggles |
| engine → riverilog | `RIV-1 bytes: 1094`, stdout carries the iverilog-face banner `$finish called at 76000 (1ps)` + `VCD info:` line |
| verification card | `✓ byte-exact against the recorded expectation` (corpus fetched through the Host route) |

Mechanically: `test/smoke.mjs` — 42 checks, ALL PASS (asset MIME/bytes/no-store,
corpus allowlist, traversal, method, the carrier prefix-match rule,
`main`/`panellist` registrations, three dictionaries, SSR markup, and in jsdom
with the real wasm artifact: Run `q=8`, riv face banner, byte-exact badge,
named spec violation). Live probes: `Slots` shows both occupants active, and the
asset route answers `200 text/javascript`, `200 application/wasm` (the 1.2 MB
artifact), `200` allowlisted corpus, `403` non-allowlisted/traversal.

Known cosmetic gap: the zh-CN dictionary shipped after the first bundle
publication, and a client bundle's combined body is cached per revision, so the
panel still renders its English strings until the app restarts (or the HMR
rebuild pipeline republishes the entry). Functionality is unaffected.
