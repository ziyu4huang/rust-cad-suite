# oracle-fixtures — adversarial-review experiment corpus (ours, committable)

Authored 2026-10-21 by the adversarial review of PLAN-DRAFT (all verified by
running Verilator 5.050 locally; see git history for the review report):

- `xprobe.sv` — X-semantics probe. Proves Verilator is 2-state: `1x00==1x00`→1,
  `1x00+1`→`1001`, `sbx101>>>1`→`0010`; with `--x-initial unique` results are
  run-random. Basis for the X-policy: byte-diff corpus stays X-clean; 4-state
  correctness is gated by IEEE §5 truth-table unit tests instead.
- `tb_race.sv` — two `initial` blocks racing at time 0. Oracle prints a
  deterministic `A first? a=1` / `B sees a=1` (declaration order). Our
  scheduler must replicate this determinism; M4 smoke fixture.
- `tb_counter.sv` / `counter.v` — earlier versions of the M5 fixture (kept for
  provenance; the canonical pair lives in `corpus/counter8/`).
- `preproc.v`, `preproc2.v`, `preproc3.v` — preprocessor probes (function-like
  macros, `` `` `` paste, `` `" `` stringify: all accepted by Verilator 5.050).

Verilator's own test_regress files are NOT committed here (LGPL/Artistic mix);
they are usable read-only as a local dev oracle only.
