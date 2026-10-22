# rust-cad-suite

A from-scratch, **Rust-only** reimplementation of the
[YosysHQ oss-cad-suite](https://github.com/YosysHQ/oss-cad-suite-build) toolset.
No upstream C/C++ is compiled or linked for core function; every shipped tool is
a Rust binary.

**Phase 1 (in progress): `rverilator`** — a Verilator-equivalent Verilog
simulator toolchain. Roadmap, milestones and decisions live in [PLAN.md](PLAN.md)
and [docs/adr/](docs/adr/).

## Status (2026-10-23)
| Milestone | State |
|---|---|
| M0-M6 workspace / preprocessor / parser / elaboration / scheduler / VCD | done — see [PLAN.md](PLAN.md) |
| M7 memory arrays + `$readmemh`/`$readmemb` + 12 gate primitives | done — `mem[i]` VCD element vars, x-index writes ignored, out-of-range reads x |
| **E2E differential regression** | **55/55 = 100% vs Verilator 5.050** (`corpus/e2e`, expectations recorded from the oracle, ≥80% gate in `cargo test`) |
| Lint-only differential track | **66/87 (75.9%)** over the vendored yosys `tests/simple` suite (`corpus-external/`, ISC); harness = `scripts/lint_diff.sh`; remaining 20 divergences are the M8 roadmap |
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
