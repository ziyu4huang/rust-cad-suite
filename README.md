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
| M3 elaboration + lint | next |
| M4–M6 simulation + VCD | planned; oracle fixtures recorded in `corpus/` |

30 tests green, zero warnings, zero external dependencies.

## Quick start
```sh
cargo build
./target/debug/rverilator --lint-only corpus/counter8/counter.v corpus/counter8/tb_counter.v
./target/debug/rverilator -E corpus/counter8/counter.v     # preprocess only
./target/debug/rverilator --dump-ast corpus/counter8/counter.v
cargo test
```

## Layout
- `crates/rcs-diag` — shared spans, source map, diagnostics renderer (suite-wide).
- `crates/vl-syntax` — preprocessor, lexer, recursive-descent parser, AST.
- `crates/vl-cli` — the `rverilator` binary.
- `corpus/` — self-written fixtures with Verilator-5.050-recorded expected outputs.
- `vendor-reference/` (gitignored) — upstream build scripts + binary release, reference only.

## Oracle
Development uses the real Verilator 5.050 (`brew install verilator`) purely as a
differential-test oracle. Normalization rules and the X-semantics policy are in
`corpus/counter8/README.md`; nothing from upstream ships.

## License
MIT OR Apache-2.0 (pending confirmation — see PLAN.md §8).
