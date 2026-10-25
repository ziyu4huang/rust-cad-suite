#!/usr/bin/env bash
# rv-diff.sh — ours-vs-oracle side-by-side on one design (AGENTS §1/§9:
# the oracle's observable behavior IS the spec; a divergence needs either a
# fix or a REGISTRY divergence row, never a silent shrug).
#
# usage: rv-diff.sh <file.v> [plusargs...]
#   prints both engines' stdout labeled, then a MATCH/DIFF verdict line and
#   exit 0 (match) / 1 (diff) / 2 (usage) / 3 (either side failed to build).
set -u

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

f="${1:-}"
[ -n "$f" ] || { echo "usage: rv-diff.sh <file.v> [plusargs...]" >&2; exit 2; }
shift

ours="${RV_BIN:-$root/target/debug/rverilator}"
oracle="${ORACLE_BIN:-/opt/homebrew/bin/verilator}"
[ -x "$ours" ] || { echo "rv-diff: $ours not built — run: cargo build" >&2; exit 3; }
[ -x "$oracle" ] || { echo "rv-diff: oracle $oracle not found (verilator is test-only, never shipped)" >&2; exit 3; }

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

"$ours" --binary "$f" "$@" >"$tmp/ours.out" 2>"$tmp/ours.err"
ours_rc=$?

"$oracle" --binary --timing "$f" -o osim --Mdir "$tmp/odir" >"$tmp/oracle.build" 2>&1
if [ $? -ne 0 ] || [ ! -x "$tmp/odir/osim" ]; then
  echo "== oracle build failed =="
  tail -5 "$tmp/oracle.build"
  exit 3
fi
"$tmp/odir/osim" "$@" >"$tmp/oracle.out" 2>/dev/null
oracle_rc=$?

echo "== ours (exit $ours_rc) =="
cat "$tmp/ours.out"
echo "== oracle (exit $oracle_rc) =="
cat "$tmp/oracle.out"

# Normalize exactly like the difftest (AGENTS §10.5): the oracle's tool
# lines (`- Verilator: …`, `-Info/`, `-Warning/`) are report blocks, not
# design output — stripped from BOTH sides before comparing.
norm() { grep -v '^- ' "$1" || true; }

if diff -q <(norm "$tmp/ours.out") <(norm "$tmp/oracle.out") >/dev/null 2>&1 && [ "$ours_rc" -eq "$oracle_rc" ]; then
  echo "== MATCH =="
  exit 0
else
  echo "== DIFF =="
  diff <(norm "$tmp/oracle.out") <(norm "$tmp/ours.out") | head -20
  exit 1
fi
