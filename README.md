# rust-cad-suite

A from-scratch, **Rust-only** reimplementation of the
[YosysHQ oss-cad-suite](https://github.com/YosysHQ/oss-cad-suite-build) toolset.
No upstream C/C++ is compiled or linked for core function; every shipped tool is
a Rust binary.

**Phase 1 (in progress): `rverilator`** — a Verilator-equivalent Verilog
simulator toolchain. Roadmap, milestones and decisions live in [PLAN.md](PLAN.md)
and [docs/adr/](docs/adr/).

## Status (2026-10-21)
| Milestone | State |
|---|---|
| M0 workspace + diagnostics + lexer | done — 124 keywords, sized/based/real literals, directives |
| M1 preprocessor | core done — object/function macros, `` `` `` paste, `` `" `` stringify, ifdef family, `` `include `` + `-I`, `` `timescale ``, `__FILE__/__LINE__` |
| M2 parser | core done — Verilog-2005 modules (ANSI + non-ANSI ports), decls, assign, always/initial, if/case/casez/for/while/repeat/forever, timing controls, instances (named/positional/arrays), generate (if/for/case/blocks), functions/tasks, multi-error recovery |
| M2 vertical slice (`--binary`) | done — 4-state values, evaluator, union-find mini-elaboration, combinational convergence, `initial` + `$display`/`$finish`; `corpus/fulladd` **byte-identical to Verilator 5.050** (test `sim_gate`) |
| M3/M4 elaboration, functions/tasks, generate | done — scope-chain resolution, parameter overrides, static functions, inlined tasks, generate for/if/case |
| M5 event scheduler | done — process VM with suspend points, time wheel, NBA region, edge wakes, `$time`/`%t`; `always`/`#delay`/NBA/`@*` all live |
| E2E differential regression | **50/51 = 98.0% vs Verilator 5.050** (`corpus/e2e`, ≥80% gate in `cargo test`; sole FAIL = memory arrays, declared M7 hard-error) |
| M6 VCD tracing | done — `$dumpfile/$dumpvars` writer; 4 cases cross-checked against Verilator-recorded VCDs (16/16 signals, 0 mismatches; `vcd_gate` in `cargo test`) |
| Frontend fuzz | done — 600-iteration deterministic mutation/random-bytes smoke in `cargo test` (caught 3 UTF-8 robustness bugs on first run) |
| Lint track (WIDTH family) | next |

47 tests green (incl. E2E 98.0%, VCD cross-check, fuzz), zero warnings, zero external dependencies.

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
- `crates/vl-sim` — 4-state values, evaluator, slice interpreter (scheduler at M5).
- `crates/vl-cli` — the `rverilator` binary.
- `corpus/` — self-written fixtures with Verilator-5.050-recorded expected outputs.
- `vendor-reference/` (gitignored) — upstream build scripts + binary release, reference only.

## Oracle
Development uses the real Verilator 5.050 (`brew install verilator`) purely as a
differential-test oracle. Normalization rules and the X-semantics policy are in
`corpus/counter8/README.md`; nothing from upstream ships.

## License
MIT OR Apache-2.0 (pending confirmation — see PLAN.md §8).
