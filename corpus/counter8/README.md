# counter8 — M5 acceptance fixture

Oracle-verified against Verilator 5.050 (`/opt/homebrew/bin/verilator`), 2026-10-21.

## Files
- `counter.v` — 8-bit counter, sync reset, parameterized width.
- `tb_counter.v` — self-checking TB. Reads q after `#1` post-edge: without it,
  IEEE 1364-2005 §11 region ordering reads the pre-NBA value (`FAIL q=4`,
  oracle-verified) — that subtlety is itself an M5 scheduler acceptance item.
- `expected.stdout` — normalized oracle stdout (`PASS`).
- `expected.oracle-stdout.txt` — raw oracle stdout (for normalization tests).

## Verified oracle invocations
```sh
# lint (BOTH files required — single-file lint dies with MODMISSING, exit 1)
verilator --lint-only --timing counter.v tb_counter.v          # exit 0
# simulate
verilator --binary --timing --Mdir obj_dir counter.v tb_counter.v -o sim
./obj_dir/sim                                                  # PASS, exit 0
```

## M5 byte-diff normalization rules (adversarial review finding #3)
1. Always run both sides with the same explicit `--timescale` (oracle default
   makes `$monitor("%0t")` print in precision units — internal time scale must
   match, not be guessed).
2. Filter from oracle stdout: lines starting `- ` (Simulation Report block)
   and `-Info:`/`-Warning`/`-Error` tool notes — never a bare `-` prefix
   (designs legitimately print `-7div2=-3`),
   any line containing `Verilator:`, `%Warning`, `%Error` (env-dependent).
3. Mixed-timescale modules are a hard error upstream (TIMESCALEMOD, exit 1) —
   replicate, don't silently pick a default.
4. Two runs of the same binary differ only in the `speed` line — covered by
   rule 2; keep that line filtered even if it moves.

## X-policy (adversarial finding #2 — ADR-0001 addendum)
Byte-diff corpus MUST stay X-clean: Verilator 5.050 is a 2-state evaluator
(`4'b1x00 == 4'b1x00` → 1, `1x00+1` → `1001`, `>>>` sign-extends x as 0), and
`--x-initial unique` makes results run-random. 4-state operator correctness is
validated separately against the IEEE 1364-2005 §5 truth tables (unit tests in
`vl-sim`), never against a Verilator byte-diff. See `oracle-fixtures/xprobe.sv`.
