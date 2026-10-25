#!/usr/bin/env bash
# riv-run.sh — the iverilog-face flow end to end: riverilog compiles the
# design to RIV-1, rivvp interprets it (ADR-0009: one engine, two CLI faces).
# Goldens on this face are recorded from iverilog 13.0.
#
# usage: riv-run.sh [rivvp flags] <file.v> [plusargs...]
#   rivvp flags (-n -N -q -l <file> ...) must come BEFORE the first .v;
#   everything after it is passed through as plusargs. The intermediate
#   out.riv lands in a scratch dir unless KEEP_RIV=1 (then ./out.riv).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

[ $# -ge 1 ] || {
  echo "usage: riv-run.sh [rivvp flags] <file.v> [plusargs...]" >&2
  exit 2
}

riverilog="${RIVERILOG_BIN:-$root/target/debug/riverilog}"
rivvp="${RIVVP_BIN:-$root/target/debug/rivvp}"
for b in "$riverilog" "$rivvp"; do
  [ -x "$b" ] || { echo "riv-run: $b not built — run: cargo build" >&2; exit 3; }
done

pre=(); post=(); src=""
for a in "$@"; do
  if [ -z "$src" ] && [[ "$a" == *.v ]]; then
    src="$a"
  elif [ -z "$src" ]; then
    pre+=("$a")
  else
    post+=("$a")
  fi
done
[ -n "$src" ] || { echo "riv-run: no .v file in arguments" >&2; exit 2; }

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
riv="$tmp/out.riv"
if [ -n "${KEEP_RIV:-}" ]; then
  riv="out.riv"
fi

"$riverilog" -o "$riv" "$src"
# bash 3.2 (macOS): "${arr[@]}" on an empty array is an unbound error under
# set -u — the ${arr[@]+…} guard keeps empty flag/plusarg lists legal.
exec "$rivvp" ${pre[@]+"${pre[@]}"} "$riv" ${post[@]+"${post[@]}"}
