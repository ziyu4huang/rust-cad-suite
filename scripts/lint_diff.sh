#!/bin/bash
# Lint-parity differential vs verilator over a directory of .v files.
# Usage: scripts/lint_diff.sh <corpus_dir> <out.tsv>
# Class per file: match-accept / match-reject / OURS-ONLY-REJECT / ORACLE-ONLY-REJECT
set -u
DIR="${1:?corpus dir}"
OUT="${2:?out tsv}"
OURS="${RVERILATOR:-$PWD/target/debug/rverilator}"
ORACLE="${VERILATOR:-/opt/homebrew/bin/verilator}"
[ -x "$OURS" ] || { echo "ours binary missing: $OURS" >&2; exit 2; }
echo -e "file\tours_rc\toracle_rc\tclass" > "$OUT"
ma=0; mr=0; uor=0; oor=0
for f in "$DIR"/*."${EXT:-v}"; do
  n=$(basename "$f")
  "$OURS" --lint-only "$f" >/dev/null 2>&1; orc=$?
  # -Wno-fatal: verdict parity is on ERRORS; verilator warning classes
  # (WIDTH/LATCH...) are a separate future track, not part of this gate.
  "$ORACLE" --lint-only -Wno-fatal "$f" >/dev/null 2>&1; vrc=$?
  if [ "$orc" -eq 0 ] && [ "$vrc" -eq 0 ]; then
    cls=match-accept; ma=$((ma+1))
  elif [ "$orc" -ne 0 ] && [ "$vrc" -ne 0 ]; then
    cls=match-reject; mr=$((mr+1))
  elif [ "$orc" -ne 0 ]; then
    cls=OURS-ONLY-REJECT; uor=$((uor+1))
  else
    cls=ORACLE-ONLY-REJECT; oor=$((oor+1))
  fi
  printf '%s\t%s\t%s\t%s\n' "$n" "$orc" "$vrc" "$cls" >> "$OUT"
done
total=$((ma+mr+uor+oor))
echo "== lint parity: $((ma+mr))/$total match (accept $ma, reject $mr); OURS-ONLY-REJECT $uor; ORACLE-ONLY-REJECT $oor =="
echo "tsv → $OUT"
[ "$uor" -eq 0 ] || exit 1
