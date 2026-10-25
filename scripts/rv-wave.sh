#!/usr/bin/env bash
# rv-wave.sh — simulate with a VCD trace, then render the deterministic SVG
# waveform (ADR-0005: vl-vcd is the single renderer; the SVG is byte-stable
# and XML-escaped, so an agent can diff it like any other golden).
#
# usage: rv-wave.sh [-o out.svg] <file.v>...
#   default output: wave.svg next to the first source file's directory is
#   NOT used — the SVG lands where -o says (default ./wave.svg); the VCD
#   stays in a scratch dir unless KEEP_VCD=1.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

out="wave.svg"
while getopts "o:" opt; do
  case $opt in
    o) out="$OPTARG" ;;
    *) echo "usage: rv-wave.sh [-o out.svg] <file.v>..." >&2; exit 2 ;;
  esac
done
shift $((OPTIND - 1))
[ $# -ge 1 ] || { echo "usage: rv-wave.sh [-o out.svg] <file.v>..." >&2; exit 2; }

sim="${RV_BIN:-$root/target/debug/rverilator}"
vcd2svg="${VCD2SVG_BIN:-$root/target/debug/vcd2svg}"
for b in "$sim" "$vcd2svg"; do
  [ -x "$b" ] || { echo "rv-wave: $b not built — run: cargo build" >&2; exit 3; }
done

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# The sim runs inside the scratch dir (the VCD lands there), so every
# argument that names an existing file must be absolutized first.
args=()
for a in "$@"; do
  if [ ! -e "$a" ]; then
    args+=("$a")
  elif [[ "$a" == /* ]]; then
    args+=("$a")
  else
    args+=("$PWD/$a")
  fi
done

(cd "$tmp" && "$sim" --binary --trace "${args[@]}" >/dev/null)
vcdfile="$(ls "$tmp"/*.vcd 2>/dev/null | head -1 || true)"
if [ -z "$vcdfile" ]; then
  echo "rv-wave: the run produced no VCD (does the TB call \$dumpvars? try: \$dumpfile(\"dump.vcd\"); \$dumpvars;)" >&2
  exit 4
fi
"$vcd2svg" "$vcdfile" -o "$out"
echo "waveform: $out ($(wc -c <"$out" | tr -d ' ') bytes)${KEEP_VCD:+ vcd: $vcdfile}"
