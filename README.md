# rust-cad-suite

A from-scratch, **Rust-only** reimplementation of the
[YosysHQ oss-cad-suite](https://github.com/YosysHQ/oss-cad-suite-build) toolset.
No upstream C/C++ is compiled or linked for core function; every shipped tool is
a Rust binary.

**Phase 1 (in progress): `rverilator`** — a Verilator-equivalent Verilog
simulator toolchain. Roadmap, milestones and decisions live in [PLAN.md](PLAN.md)
and [docs/adr/](docs/adr/).

## Status (2026-10-25)
| Milestone | State |
|---|---|
| M0-M6 workspace / preprocessor / parser / elaboration / scheduler / VCD | done — see [PLAN.md](PLAN.md) |
| M7 memory arrays + `$readmemh`/`$readmemb` + 12 gate primitives | done — `mem[i]` VCD element vars, x-index writes ignored, out-of-range reads x |
| **E2E differential regression** | **59/59 = 100% vs Verilator 5.052** (`corpus/e2e`, expectations recorded from the oracle, exact-100% gate in `scripts/regress.sh`) |
| Lint-only differential track | **83/87 (95.4%)** over the vendored yosys `tests/simple` suite (`corpus-external/`, ISC); harness = `scripts/lint_diff.sh`; remaining divergences = 3 different-root-cause feature gaps + 1 registered divergence |
| Wide values (>128 bits) | done — two-arm value repr, full limb-aware op surface ([ADR-0008](docs/adr/0008-multi-limb-values.md)) |
| **riverilog** (iverilog-analog, R2-R4) | real compile→simulate pipeline — `riverilog tb.v -o out.riv` + `rivvp out.riv` runs hierarchical designs end-to-end (instantiation/params/generate/memories/gates/named events/delayed NBAs): **58/59 e2e + 19/20 iv-e2e byte-exact** (53 = rivD10 registered two-oracle divergence, ledgered; 09_udp → R9); three-way tri gate (iverilog legs committed in git); VCD crosscheck vs iverilog-recorded goldens; yosys-simple compile-parity leg (68/68 + 19 registered); $fopen/$dump-control/$random family; perf ~575k cycles/s release (vvp ÷2.2); design = [docs/riv-r2-design.md](docs/riv-r2-design.md), plan = [docs/RIVERILOG-PLAN.md](docs/RIVERILOG-PLAN.md) |
| **rVPI** (Rust-native VPI analog) | slice 1 done — typed handles, value get/put, value-change/after-delay/start/end callbacks, iteration, time ([ADR-0003](docs/adr/0003-rvpi.md)); verified by `vpi_gate` + a two-paths-vs-corpus check + an internal rVPI×VCD crosscheck |
| Frontend fuzz | done — 600-iteration deterministic mutation/random-bytes smoke in `cargo test` (caught 3 UTF-8 robustness bugs on first run) |

External corpora vendored under `corpus-external/` (ISC-licensed snapshots with
attribution): yosys `tests/simple` (87 files) and chipsalliance `sv-tests`
chapters 5/6/8/9/12 (260 files). Divergences and incidents are tracked in
[docs/REGISTRY.md](docs/REGISTRY.md); the mechanical gate is
`bash scripts/regress.sh` (E2E ≥ baseline, lint-parity ≥ baseline, report
regeneration).

## Quick start
```sh
cargo build
./target/debug/rverilator --binary corpus/fulladd/fulladd.v corpus/fulladd/tb_fulladd.v   # simulate
cargo test -p vl-difftest    # E2E differential regression gate (>=80%)
./target/debug/rverilator --lint-only corpus/counter8/counter.v corpus/counter8/tb_counter.v
./target/debug/rverilator -E corpus/counter8/counter.v     # preprocess only
./target/debug/rverilator --dump-ast corpus/counter8/counter.v
cargo test
```

## Layout
- `crates/rcs-diag` — shared spans, source map, diagnostics renderer (suite-wide).
- `crates/vl-syntax` — preprocessor, lexer, recursive-descent parser, AST.
- `crates/vl-sim` — 4-state values, evaluator, interpreter (event scheduler) and the rVPI interface (`src/vpi.rs`).
- `crates/vl-cli` — the `rverilator` binary.
- `crates/rv-xls-sim` — XLS `VerilogSimulator`-shaped process adapter (argv + stdin JSON) over the unified engine; contract tests pin `.sv` refusal, macro/include behaviour, and byte-exact replay of the harvested XLS samples ([docs/XLS-ADAPTER.md](docs/XLS-ADAPTER.md)).
- `crates/vl-values` + `crates/riv-*` (`riv-ir`, `riv-elab`, `riv-kernel`, `riv-cli`, `riv-difftest`) — the riverilog (iverilog-analog) line: `riverilog`/`rivvp` binaries over flat elaboration → RIV-1 (own versioned text IR) → event-driven kernel; see [docs/RIVERILOG-PLAN.md](docs/RIVERILOG-PLAN.md).
- `dsh-plugins/rverilator-wasm-playground/` — the wasm playground packaged as a DeepSeek Harness bundle (a Verilog panel in the Harness Web UI; Host route serves the `pkg` artifact, an allowlisted corpus slice, and the scanned XLS provenance manifest, Client half is the React panel). See its [README](dsh-plugins/rverilator-wasm-playground/README.md) and the [XLS shared-core plan](docs/plan/xls-shared-core.html).
- `crates/vl-wasm` — WebAssembly facade over `vl-sim` (`wasm-pack build crates/vl-wasm --target web`, ~500KB; differential-gated from Bun).
- `corpus/` — self-written fixtures with Verilator-5.050-recorded expected outputs.
- `vendor-reference/` (gitignored) — upstream build scripts + binary release, reference only.

## Oracle
Development uses the real Verilator 5.050 (`brew install verilator`) purely as a
differential-test oracle. Normalization rules and the X-semantics policy are in
`corpus/counter8/README.md`; nothing from upstream ships.

## License
MIT — see [LICENSE](LICENSE). The vendored corpus snapshots keep their own
ISC licenses and attribution under `corpus-external/`. Development only:
the real Verilator 5.050 is used as a differential oracle; nothing from
upstream ships.
