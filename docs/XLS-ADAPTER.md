# XLS adapter (`rv-xls-sim`) — contract v1

`rv-xls-sim` is the process-shaped answer to Google XLS's pluggable simulator
interface. XLS's `xls/simulation/verilog_simulator.h` is a pure-virtual class:

```
Run(text, file_type, macro_definitions, includes) -> (stdout, stderr)
RunSyntaxChecking(text, file_type, macro_definitions, includes) -> Status
```

Its only built-in implementation shells out to `iverilog`/`vvp`. This adapter
is the same shape for rverilator: Verilog in (memory or files), the **unified
engine** (`vl_sim::USim`, the same one `rverilator --binary` drives) underneath,
design output and diagnostics returned separately.

- Crate: `crates/rv-xls-sim`
- Binary: `cargo build -p rv-xls-sim` → `target/debug/rv-xls-sim`
- Plan/context: `docs/plan/xls-shared-core.html` (S2)
- Tests: `cargo test -p rv-xls-sim` (10 unit + 11 contract)

## Two entry shapes, one request model

### 1. argv (XLS's iverilog simulator shape)

```sh
rv-xls-sim [--syntax-check] [--top-module M] [-D NAME[=VAL]] [-I DIR] file.v...
```

- Design `$display` output → **stdout**.
- Diagnostics → **stderr**.
- Exit `0` clean, `1` compile/simulation error, `2` bad invocation.

The CLI (`rverilator`) intentionally keeps Verilator's all-on-stdout convention;
this face exists because XLS's interface separates the two streams. The adapter
is what separates them.

### 2. JSON (one request on stdin, one response on stdout)

```sh
printf '%s' '{"text":"module tb; initial begin $display(\"q=%0d\", 8); $finish; end endmodule\n"}' \
  | rv-xls-sim --json
```

```json
{"status":"ok","stdout":"q=8\n","stderr":"","exit":0}
```

Request fields (`text` is required; unknown keys are ignored for forward
compatibility):

| Field | Type | Default | Meaning |
|---|---|---|---|
| `text` | string | — | Verilog source (written to a scratch `top.v`, then compiled) |
| `file_type` | string | `".v"` | `".v"` accepted; `".sv"` refused explicitly |
| `top` | string \| null | auto-detect | top module |
| `defines` | object of strings | `{}` | `{"NAME":"VALUE"}`; an empty string means `-DNAME` |
| `includes` | array of strings | `[]` | include search directories |
| `syntax_only` | boolean | `false` | preprocess + parse + elaborate, no simulation |

Response fields: `status` (`"ok"`/`"error"`), `stdout`, `stderr`, `exit`.
The process exit code mirrors `exit` (`0`/`1`); a malformed request prints a
`status:"error"` response and exits `2`, so a caller can always parse the body.

## Semantics pinned by tests

`crates/rv-xls-sim/tests/contract.rs` is the acceptance gate:

| Fixture | Assertion |
|---|---|
| `57_xls_add3` (design + TB as one text) | stdout is **byte-equal** to the Verilator-recorded `expected.stdout`; no scratch path leaks |
| `58_xls_acc_pipe` (same) | byte-equal; the TB has no `$dumpvars` (the adapter does not need one) |
| argv mode on the same corpus | same bytes as the JSON/text path — one core, two entry shapes |
| `-D BIG` + `` `ifdef BIG `` | macro actually changes stdout (`big` vs `small`), not just accepted |
| `-I <dir>` + `` `include "defs.vh" `` | include resolves from the directory |
| broken design | `status:"error"`, `exit:1`, stdout empty, stderr carries the diagnostics |
| `.sv` | refused with a "SystemVerilog … milestone" message — never silently downgraded |
| `--syntax-check` on a good design | `ok`, stdout and stderr empty (no `%Info` noise) |
| JSON response | round-trips; `status`/`stdout`/`stderr`/`exit` typed correctly |

The regression gate runs the binary on the add3 corpus as well
(`scripts/regress.sh`, "xls adapter" line), so a broken release build cannot
skip the contract tests.

## Implementation notes

- **One engine.** The adapter calls `vl_sim::USim` (riv-elab → riv-kernel) with
  per-file timescales collected during preprocessing, exactly like
  `rverilator --binary` (ADR-0009). It does not spawn the CLI, so there is no
  second process to keep in sync, and no binary-path discovery.
- **In-memory text** is written to a scratch directory (named after the source
  file; only the directory is random) and compiled from disk, so relative
  `` `include `` resolution and `$readmem` behave like a real file. The scratch
  directory is removed on drop; `$readmem` resolves against it.
- **VCD** is not written: XLS's interface only consumes stdout/stderr. A
  design's `$dumpvars` still parses and runs; the trace text is discarded.
- **Multi-top syntax check** mirrors `--lint-only`: every un-instantiated root
  is elaborated, so a generated design does not need an explicit top.
- **No serde.** The JSON reader (`src/json.rs`) is a ~250-line recursive-descent
  subset with unit tests for escapes/surrogates/malformed input; the repo's ABI
  surfaces stay dependency-free.

## Divergences / non-goals (v1)

| Item | v1 behaviour | Why |
|---|---|---|
| `.sv` input | explicit hard error | rverilator targets Verilog-2005; XLS's iverilog path also refuses SV |
| stderr wording vs iverilog | diagnostic text differs (our spans/codes) | callers must not assert iverilog's wording; only "non-empty on failure" |
| `$finish` | exit `0`, no finish banner | rverilator face suppresses the iverilog banner (ADR-0009/rivD8) |
| VCD output | discarded | not part of XLS's `SimulationResult` |
| file injection | not in v1 (`$readmem` reads the scratch/cwd) | XLS does not pass data files through `Run`; the wasm face has `inject_file` when needed |

## Upstream subclass (implemented, compiled against the real header)

The XLS side lives in `crates/rv-xls-sim/cpp/`:

- `xls/simulation/simulators/rverilator_simulator.{h,cc}` — the
  `VerilogSimulator` subclass. Like `IcarusVerilogSimulator` it creates a temp
  dir, writes `top.v` and every `VerilogInclude` (text, not a path), passes
  `-I <tempdir>` plus the always-on `-D SIMULATION -D ASSERT_ON`, invokes
  `rv-xls-sim`, and maps a non-zero exit to an absl error carrying the
  diagnostics. `.sv` is refused with `UnimplementedError` before spawning.
- `xls/simulation/simulators/BUILD` — drop-in Bazel target. Add
  `//xls/simulation/simulators:rverilator_simulator` to the deps of
  `//xls/simulation:verilog_simulators` (next to the iverilog target).
- Registration: `XLS_REGISTER_MODULE_INITIALIZER(... "rverilator" ...)` lives in
  the same TU and is excluded by `-DRV_XLS_SIM_HARNESS` for the local harness.

Local verification — it downloads the pinned XLS headers, compiles and runs:

```sh
bash scripts/xls-cpp-gate.sh
```

The harness compiles against upstream `v0.0.0-10898-g8ddae8bf4` and links a
local `InvokeSubprocess` double (`cpp/harness/`). It proves the subclass shape,
in-memory includes, macros, error mapping and add3 byte-equality. The Bazel
in-tree link is proven by CI (2026-10-26): the `in-tree` job of
`.github/workflows/xls-harvest.yml` checks out google/xls at the harvested tag,
overlays the subclass, and `bazel build`s it green (first run 635s / 2171
actions; artifact measured as `librverilator_simulator.lo`). XLS's own test
suite and the `verilog_simulators` registration land with the upstream PR.
This host has no Bazel and the XLS release is linux-x64 only — CI is the
linux oracle for this arc.

## Try it

```sh
cargo build -p rv-xls-sim
cargo test  -p rv-xls-sim

# byte-exact replay of the harvested XLS sample
./target/debug/rv-xls-sim corpus/e2e/57_xls_add3/add3.v corpus/e2e/57_xls_add3/tb.v

# the same design through the JSON protocol (syntax only)
printf '%s' '{"text":"module tb; endmodule","syntax_only":true}' \
  | ./target/debug/rv-xls-sim --json

# XLS-side subclass against the real upstream header (downloads it once)
bash scripts/xls-cpp-gate.sh

# CI artifact -> snapshot (verify sha256 + normalize + print next steps)
python3 scripts/xls_adopt.py /tmp/xls-artifact --dry-run
```
