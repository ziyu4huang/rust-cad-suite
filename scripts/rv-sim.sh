#!/usr/bin/env bash
# rv-sim.sh — compile + simulate Verilog with rverilator (agent toolbox).
# The one command an agent uses to run a design: stdout is the contract the
# e2e corpus gates byte-exact against the verilator-recorded goldens.
#
# usage: rv-sim.sh [-t] [-L] <file.v>... [plusargs...]
#   -t   record a VCD trace (writes the $dumpfile target, default dump.vcd,
#        into the current directory)
#   -L   force the legacy engine (RVERILATOR_LEGACY=1); default is the
#        unified engine (riv-elab -> riv-kernel, ADR-0009 U-1)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

trace=0; legacy=0
while getopts "tL" opt; do
  case $opt in
    t) trace=1 ;;
    L) legacy=1 ;;
    *) echo "usage: rv-sim.sh [-t] [-L] <file.v>... [plusargs...]" >&2; exit 2 ;;
  esac
done
shift $((OPTIND - 1))
[ $# -ge 1 ] || { echo "usage: rv-sim.sh [-t] [-L] <file.v>... [plusargs...]" >&2; exit 2; }

bin="${RV_BIN:-$root/target/debug/rverilator}"
if [ ! -x "$bin" ]; then
  echo "rv-sim: $bin not built — run: cargo build" >&2
  exit 3
fi

args=(--binary)
[ "$trace" -eq 1 ] && args+=(--trace)
if [ "$legacy" -eq 1 ]; then
  RVERILATOR_LEGACY=1 "$bin" "${args[@]}" "$@"
else
  "$bin" "${args[@]}" "$@"
fi
