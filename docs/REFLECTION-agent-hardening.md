# Self-reflection: what building the verilator-equivalent taught about
# agent-developed EDA/IC tooling (2026-10-21, arc M0→M5 + E2E gate)

Scope of evidence: commits 2f3d43f → c757a7a. 32-case E2E differential suite
vs Verilator 5.050 at 96.9% (the one failure is a declared M7 hard-error).
Every failure below is one that *shipped* in a commit before being caught —
that is the point of recording them.

## 1. The dominant failure class: hand-authored expectations vs the oracle

Seven times in one arc, a wrong expectation reached the tree, and **every
time the oracle was right and my reasoning was wrong**:

| # | My belief | Oracle truth |
|---|---|---|
| 1 | TB reads `q=5` after 5th posedge | `FAIL q=4` — NBA region commits after the active-region read |
| 2 | `%0d` pads like `%d` | `%0d` = minimal; plain `%d` pads to type width |
| 3 | `4'hABCD` truncates (LRM §2.5.1) | Verilator errors "too many digits" |
| 4 | binary `a ~& b` is an operator | Annex A has no binary `~&`/`~|`; Verilator rejects |
| 5 | `12'hDEAD_BEEF` truncation unit test | error (same as #3) |
| 6 | `'hx` → 32'h0000000x | unknown fills the full 32 bits |
| 7 | `always @*` waits at t0 like a simulator | level-sensitive processes evaluate once at t0 |

Root cause: I treated the LRM as the spec. For a *compatibility* product the
spec is the reference tool's observable behavior; the LRM only explains it.
A hand-written golden file is a guess with a file extension.

**Rule (now AGENTS §2):** expected outputs are *recorded* from the oracle,
never authored. Hand-written expectations are allowed only for error-path
shape (exit codes, milestone pointers), not value semantics.

## 2. Two acceptance tracks, or the X-policy trap

The adversarial audit proved Verilator is 2-state (`1x00+1=1001`). A
4-state-correct implementation would *fail* a naive byte-diff against it on
X-propagating samples — correctness punished as regression. Corollary found
in practice: `z` literals and same-edge scheduling races (FSM case) are the
same trap in disguise — case 26 had two processes racing on one edge, where
IEEE itself is nondeterministic and only Verilator's internal order made it
"pass".

**Rule (AGENTS §3):** byte-diff corpus is X-clean and race-free by policy;
4-state operator correctness and scheduler determinism are gated by their own
unit tests (IEEE §5 tables, declaration-order contract). A parity test that
depends on unresolved order is not a test, it's a coin.

## 3. Error recovery must guarantee progress — with a test

The worst bug of the arc: `task one(input integer n);` hung the parser
forever. `sync_item` stops on structural keywords (`endtask`, `end`…) without
consuming them; the module loop didn't treat them as terminators; the error
path re-derived the same diagnostic forever. It survived because no fixture
combined "task + ANSI typed port". Found by `sample`-ing the live process,
not by reading code.

**Rule (AGENTS §4):** every recovery loop carries a `pos-unchanged → bump`
guard, and the corpus contains one hang-shaped fixture per recovery site.
A watchdog is part of the test harness, not an ops afterthought.

## 4. Semantics bugs cluster where two representations meet

`~x` flipped x→z (bit_not inverted the unknown *kind*); `>>>` filled sign
bits at bit 64 instead of the value's width; ternary-merge used the AND
forcing rule; `%d` dropped the signed flag between Val and Bits. None of
these are parser problems — they live exactly where a bit-pair encoding
`(data, mask)` meets an operator table or a format contract.

**Rule (AGENTS §5):** every 4-state operator gets its x/z-kind-preserving
unit test written *with* the operator, from the IEEE §5 table, before it is
used anywhere. Formatting (`%d/%b/%t`) is a contract too — pin it with
golden tests, the oracle arbitrates.

## 5. Strict resolution surfaced what blind prefixing masked

The first implementation "worked" because it blindly string-prefixed names;
the rewrite to strict scope-chain resolution immediately exposed that ANSI
port declarations never created cells — a real elaboration hole that had been
passing by coincidence (flat-namespace collision). Lesson: making illegal
states *error loudly* converts silent miscompiles into fixable failures; the
same pass turned every unsupported feature into a `milestone Mx` pointer, so
E2E failures self-classify into "bug" vs "declared scope".

**Rule (AGENTS §6):** unresolved = hard error with a named milestone.
Silent fallbacks are banned; a `not yet supported` error is a feature.

## 6. Process debt that paid off (keep doing)

- Adversarial review with *execution* evidence (it ran verilator before
  reviewing) set the X-policy, the normalization spec, and the flag-error
  contract before any of this was written — zero rework attributed to it.
- Oracle-driven corpus authoring: verilator's warnings-as-errors reviewed my
  testbench style for free (WIDTH/ASCRANGE/CASEX warnings).
- Surgical edits via scripts with hit-count assertions; re-read after any
  script mutation of a file. Two Edit-tool staleness rejections and one
  match-arm splice broke a build — the assertion discipline caught what
  confidence would have shipped.

## 7. What is still weak (next hardening)

- No fuzzing yet (M2 gate item) — the recovery-hang bug class is exactly
  what fuzz_parse would have caught in minutes.
- Coverage is pass-rate, not line coverage — a case can pass while never
  exercising a branch. Add per-case coverage instrumentation when the
  scheduler stabilizes.
- VCD parity (M6) still open; the alias-folding comparison rule is written
  down but not implemented.
- Single-oracle risk: everything is pinned to Verilator 5.050 behavior;
  iverilog as a second oracle would catch Verilator-specific quirks being
  copied as "spec".
