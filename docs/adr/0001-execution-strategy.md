# ADR-0001: Phase 1 execution strategy

Status: accepted (2026-10-21). Planner proposal + adversarial review synthesis.

## Decision
v0 ships a **tree-walking interpreter (option A)** over an elaborated IR.
Tier B — Verilog → generated Rust model source → rustc (option B) — is the
M9 decision (ADR-0002), designed-for from day one:

- Every process lowers to **segments**: straight-line statement lists ending
  at explicit suspend points (`#delay`, `@(...)`, task calls with delays).
  The interpreter resumes via explicit frame state — VM-shaped, so tier B
  emits one async-fn per segment chain instead of a new IR.
- Timing constructs never become host-level concurrency. async/await is
  syntactic sugar over the segment machinery only; the scheduler is a single
  thread with a **deterministic process order (declaration order)** — the
  oracle (Verilator 5.050, `oracle-fixtures/tb_race.sv`) shows upstream also
  emits a fixed order, and byte-diff acceptance depends on replicating it.

## Delay-semantics → IR primitive table (fill in before M4)
| Source construct | IR primitives |
|---|---|
| `a = #5 b;` (intra-assignment) | sample-rhs node → delay node → writeback node (IEEE 1364-2005 ch.9) |
| `a <= b` (NBA) | evaluate-in-active, commit-in-nba region node |
| `@(a or b)` | one waiter, multiple source edges |
| `wait(expr)` | level+edge re-entry check (M8) |
| `#0` | active→NBA two-step update (§4.6/4.7) |

## Why not B now, why not C
- B pays emitter + build orchestration before semantics exist; its payoff
  (compiled-model speed) is validated against the interpreter-as-oracle later.
  Upstream's own C++ build costs 16–92 s even for trivial designs on this
  machine — the interpreter's "simulate now" `--binary` is a genuine UX win
  during development.
- C (bytecode VM) costs a VM without codegen payoff; the segment IR keeps the
  door open if corpus-scale simulation needs it before B lands.

## X-policy (adversarial finding #2)
The shipped value type is 4-state capable (`{data: u64, mask: u64}` per limb;
multi-limb layout = its own ADR before M1 value work), but the *policy* default
matches Verilator's 2-state behavior (init 0, X folds at elaboration).
Differential gates never feed X-propagating samples to a Verilator byte-diff;
4-state operator correctness is gated by IEEE 1364-2005 §5 truth-table unit
tests (evidence: `corpus/oracle-fixtures/xprobe.sv`).
