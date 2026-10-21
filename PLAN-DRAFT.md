# Rust CAD Suite — Phase 1 Plan (DRAFT from planner, pending adversarial review)

> Status: planner 出案稿(2026-10-21)。對抗審後合成為 PLAN.md 正典;本檔保留原案供對帳。

## 0. Scope restatement

- Rust-only reimplementation of the OSS CAD Suite toolset: no upstream C/C++ compiled or linked for core function; every shipped tool is a Rust binary.
- Phase 1 = a **Verilator-equivalent** Verilog/SystemVerilog simulator toolchain. Verilator is a *compiler to a cycle-based 2-state model* (not an IEEE event simulator) — we replicate that conceptual role, not its C++ codebase (clean-room: different language, no code copying).
- Later phases reuse Phase-1 frontend crates (yosys-equiv is the big consumer), so the workspace is split for sharing from day one.
- Verified on disk: rustc/cargo 1.95.0; oracle Verilator 5.050 at `/opt/homebrew/bin/verilator` (dev-time only, never shipped); **iverilog is NOT installed** (single oracle — difftest harness must auto-skip it). macOS arm64.

## 1. Research findings

### 1.1 Suite catalog (`vendor-reference/oss-cad-suite-build/default/scripts/`, 63 scripts)

| Group | Tools (scripts) | Upstream lang |
|---|---|---|
| Simulation | verilator, iverilog, slang (SV frontend), cocotb (python) | C++, C++, C++, Py |
| Synthesis | yosys, abc, ghdl, yosys-slang-plugin, ghdl-yosys-plugin, pyhdl | C++, C, VHDL-chain |
| Formal/SMT | sby(Py), eqy, aiger, avy, boolector, btor2tools, cadical, cvc4, cvc5, yices, z3, bitwuzla, lingeling, picosat, libpoly, smt-switch, suprove, pono, symfpu, imctk, ric3, formal | all C/C++ |
| PnR+bitstream | nextpnr×7 (+bba), icestorm, prjtrellis, **prjoxide (Rust!)**, apicula(Py), prjpeppercorn, gatemate-bba | C++/C/Rust/Py |
| Programming | openfpgaloader + (dfu-util, ecpprog, ecpdap, fujprog, openocd per `default/rules/programmers.py`) | C |
| Waveform | gtkwave, **surfer (Rust!)**, xdot, graphviz | C, Rust |
| Support | python2/3 (native+pkg), system-resources×3, utils, compatibility | Py/sh |

Upstream build flow (`default/scripts/verilator.sh`): autoconf→configure→make C++ with flex/bison; our port hand-writes all of that in Rust.

**Proposed phase order after verilator** (value × feasibility):
1. P2: yosys-equiv synthesis core — highest linchpin value; reuses `vl-syntax`/`vl-elab`.
2. P3: iverilog-equiv — event-driven scheduler + VPI subset; shares ~80% of Phase-1 crates.
3. P4: nextpnr-generic + FPGA families — adopt **prjoxide** (Rust, ISC) as-is; icestorm/prjtrellis/apicula = long tail.
4. P5: formal (sby-equiv driver) — **risky**: all serious SMT/SAT solvers upstream are C++; pure-Rust solver maturity is the weakest link.
5. P6: waveform viewer — adopt surfer (Rust, EUPL-1.2 — separate binary only) or write a Rust TUI viewer on our VCD/FST crates.
6. P7: cocotb-equivalent — "Rust-only" vs Python ecosystem conflict (open question).

### 1.2 Verilator architecture (docs/internals.rst @ verilator master)

Pipeline a Rust port replicates conceptually:
1. Preprocess → lex → parse → AST.
2. Link cells, link identifiers, resolve params, elaborate.
3. X-elimination, inlining, const-prop, DCE.
4. Pseudo-flatten with Scope/VarScope.
5. Order (V3Order) + schedule (V3Sched) → `eval` function family; emit C++ (V3EmitC); `--binary` adds model+main+make and invokes the C++ compiler (measured 16–92 s pure C++ build latency for trivial designs on this machine — a cost interpreter mode avoids).
6. Scheduling semantics: cycle-based, eval regions `ico`/`act`/`nba` (+`inact`/`obs`/`react` under `--timing`), each iterated to convergence with a `didNotConverge` limit; combinational loops broken by hybrid logic (UNOPTFLAT); activity gating; NBAs committed only in the `nba` region; post-init settle pass.
7. `--timing`: processes with delays/event controls become suspendable C++20 coroutines (`VlDelayScheduler` time-heap, `VlTriggerScheduler`). Rust analog = async/await.
8. Not a full IEEE event simulator: mostly 2-state, X-eliminated at elaboration; `#delay` via `--timing` coroutine machinery.

### 1.3 Rust ecosystem survey

- dalance/sv-parser — MIT/Apache-2.0, IEEE 1800-2017 grammar-complete **CST**; CST→AST lowering ≈ half a parser; keep as dev-only cross-check oracle only.
- sv_sim (DMoore12) — MIT toy; not reusable. marlin-verilog — DPI bridge, not a simulator. rust-hdl/Veryl/Spade — DSLs, excluded. slang-rs — C++ bindings, violates Rust-only.
- CHIPS Alliance sv-tests — test suite ISC-licensed → corpus.

**Recommendation: hand-write lexer, preprocessor, recursive-descent parser** (context-sensitive lexing: unsized `'h` literals with x/z, `` ` `` macro expansion across token boundaries, `` `" ``).

### 1.4 Corpus licenses
- ISC: picorv32, serv, sv-tests, prjoxide, oss-cad-suite-build → safe to commit.
- Verilator test_regress: LGPL-3/Artistic mix → **never commit**; local read-only oracle use only.

## 2. CLI & naming proposal
- Binary `rverilator` (phase 1): PATH safety vs oracle; honesty (not bug-compatible day one); `rv*` family. `verilator` alias deferred ≥M9.
- Flags honored v0: `--lint-only`, `-Wall`, `-Wno-<CODE>`, `-E`, `-D`, `-I`, `--top-module`, `--timescale`, `--binary` (interpret mode = simulate now), `--timing` (accepted, always-on), `--trace`, `--Mdir`, `-o`, `--build-jobs` (noop), `--version`. Unknown flags: warn, don't hard-fail.
- Exit codes: 0 ok / 1 diagnostics-fatal; match Verilator observed codes at M2.

## 3. Workspace layout
```
rust-cad-suite/
├── Cargo.toml
├── crates/
│   ├── rcs-diag/     spans, source files, diagnostics, caret renderer   [0 deps]
│   ├── vl-syntax/    preprocessor + lexer + parser + AST                [rcs-diag]
│   ├── vl-ir/        elaborated design IR (sim ↔ future codegen)       [rcs-diag]
│   ├── vl-elab/      link/params/generate/width → lower to vl-ir      [vl-syntax, vl-ir]
│   ├── vl-sim/       scheduler + interpreter + values + VCD            [vl-ir]
│   ├── vl-cli/       bin `rverilator`                                  [above + clap]
│   └── vl-difftest/  dev-only oracle harness                           [publish=false]
├── corpus/  corpus-external/  fuzz/  docs/adr/
```
Dependency policy: core crates zero external deps; `vl-cli` may use clap; dev-only cargo-fuzz/insta.
Value representation: 4-state capable `{data: u64, mask: u64}` bit-slices + vectors; policy default = Verilator 2-state behavior (init 0).

## 4. Verilog subset ladder
**IN v0 (V2005 core)**: modules; ANSI+non-ANSI ports; wire/reg/integer/time; vectors/ranges; assign; always @(posedge|negedge|*); blocking/nonblocking; if/case/casez/casex; for/while/repeat; parameter/localparam (+`#(.P())`); defparam parse+warn; generate/genvar; instantiation named+positional; functions (no delays), tasks; $display/$write/$finish/$time/$stime/$realtime/$monitor/$dumpfile/$dumpvars/$dumpoff/$dumpon/$random/$clog2/$bits; `timescale/`define/`undef/`ifdef family/`include/`default_nettype; #delay in TBs; initial; literals incl x/z; concat/replication; all operators.
**OUT v0 (explicit "not yet supported, Mx")**: specify, UDP, VPI/PLI/DPI, force/release (M7+), wait/fork (M8), real corners (M7), FST (M9), coverage, SVA.
**SV ladder**: M8 = logic, always_ff/comb/latch, typedef/enum/struct packed, packages. Interfaces M10.

## 5. Milestones
| M | Scope | Acceptance | Effort |
|---|---|---|---|
| M0 skeleton | workspace, rcs-diag, lexer+tokens | lexer literal tests; diag golden; `--version` | 2–3 d |
| M1 preprocessor | define obj+func (`` `` ``, `` `" ``), include+-I, ifdef family, timescale, __FILE__/__LINE__ | golden token dumps; diff vs `verilator -E` | ~1 wk |
| M2 parser | full V2005 grammar RD, AST dump, multi-error recovery | 100% own corpus + sv-tests subset; ≥2 errors w/ spans; fuzz 10 min | ~2 wk |
| M3 elab+lint | linking, params, generate, width check, -Wall subset, --top-module | picorv32+serv elaborate zero spurious WIDTH | 2–3 wk |
| M4 interp core (untimed) | 4-state values, expr eval, assign, initial, $display/$finish | adders TB byte-identical to oracle | ~2 wk |
| M5 timing+sched | #delay wheel, edges, NBA region, comb convergence, UNOPTFLAT, $monitor | **counter8 + self-check TB → `q=5`,`PASS` byte-exact** | 2–3 wk |
| M6 VCD | $dumpfile/$dumpvars/off/on, timescale | normalized VCD byte-equal to oracle on 4 designs | ~1 wk |
| M7 breadth | functions/tasks+delay, generate full, memories+$readmem, reals, force/release | picorv32 smoke; sv-tests % | 3–4 wk |
| M8 SV ladder 1 | logic, always_ff/comb, typedef/enum/struct, packages | enum FSM lint+sim | ~3 wk |
| M9 perf gate | activity gating; ADR-0002 tier B `--gen-rust` | ≥10× vs M8; tier-B diff vs interpreter | 2 wk+ADR |

Cumulative to M9 ≈ 4–6 months; first useful tool at M3 (lint-only).

## 6. Testing
Unit per crate; golden `corpus/<case>/{design.v,tb.v,expected.stdout,expected.exit,expected.vcd}`; differential vs oracle (`RCS_ORACLE`, pinned 5.050; stdout normalized; VCD normalized with canonical id remap by (scope,name)); `verilator -E` for pp; fuzzing from M2; CI ubuntu+macos.

## 7. ADR-0001 execution strategy
| | A interpreter | B Verilog→Rust→rustc | C bytecode VM |
|---|---|---|---|
| Correctness/debug | best | medium | medium |
| Speed | slowest | fastest | middle |
| Runtime deps | none | rustc/cargo | none |
| Compile latency | none | seconds→minutes | none |
| Timing | explicit resume frames | async/await ≈ C++20 coroutines | natural |
| Cost | lowest | high | high |
**Decision: A for v0**; processes lower to **segments** (straight-line stmt lists ending at suspend points) so IR is VM-shaped for B/C reuse. Path to B at M9 (`--gen-rust`). C rejected v0.

## 8. Risks & open questions
Risks: semantic drift from Verilator specifics ($display padding, timescale rounding, zero-init); SV creep; drop-in flag surface; tier-B rustc requirement; solo schedule; P5 pure-Rust solver gap.
Open: (1) 4-state headroom vs 2-state parity; (2) naming; (3) tier B wanted?; (4) perf targets; (5) license MIT OR Apache-2.0; (6) SV interfaces timing; (7) cocotb/Python.
